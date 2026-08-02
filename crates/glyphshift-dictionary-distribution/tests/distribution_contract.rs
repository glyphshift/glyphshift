use glyphshift_dictionary_distribution::{
    ArtifactPresentation, ArtifactStatement, CatalogQuery, CatalogRelease,
    DictionaryArtifactDescriptor, DictionaryDistribution, DictionaryDistributionError,
    DictionaryInstallationState, DictionaryReleaseKey, DictionaryReplacementPolicy,
    FixedInstallationClock, InMemoryDictionaryCatalog, InMemoryDictionaryInstallStore,
    InMemoryTrustVerifier, InstallRequest, PublisherIdentity, Sha256Digest, SignatureEnvelope,
};
use glyphshift_dictionary_package::{DictionaryCreate, DictionaryEntryCreate, DictionaryPackage};
use sha2::{Digest, Sha256};

const PRIMARY_URL: &str = "https://catalog.example/artifacts/dictionary-ui.json";
const MIRROR_URL: &str = "https://mirror.example/artifacts/dictionary-ui.json";

#[test]
fn distribution_query_selects_parent_then_declared_default_presentation() {
    let payload = dictionary_payload("dictionary.ui", "1.2.0");
    let release = release(&payload, [PRIMARY_URL], "publisher.example");
    let mut distribution = distribution(
        InMemoryDictionaryCatalog::new().with_release(release.clone()),
        InMemoryTrustVerifier::new(),
        InMemoryDictionaryInstallStore::new(),
    );

    let parent = distribution
        .query(&CatalogQuery::new("menu"), "zh-Hans-CN")
        .expect("query parent locale");
    assert_eq!(parent.releases().len(), 1);
    assert_eq!(
        parent.releases()[0].effective_presentation_locale(),
        "zh-Hans"
    );
    assert_eq!(parent.releases()[0].presentation().name(), "菜单中文");

    let default = distribution
        .query(&CatalogQuery::new("menu"), "ja-JP")
        .expect("query default locale");
    assert_eq!(
        default.releases()[0].effective_presentation_locale(),
        "en-US"
    );
    assert_eq!(default.releases()[0].presentation().name(), "Menu English");
}

#[test]
fn distribution_install_verifies_and_is_idempotent_with_mirror_fallback() {
    let payload = dictionary_payload("dictionary.ui", "1.2.0");
    let release = release(
        &payload,
        ["https://missing.example/dictionary.json", MIRROR_URL],
        "publisher.example",
    );
    let statement = ArtifactStatement::for_release(&release);
    let publisher = PublisherIdentity::new("publisher.example").expect("publisher");
    let trust = InMemoryTrustVerifier::new().with_trusted_artifact(
        statement,
        release.artifact().signature().clone(),
        publisher,
    );
    let catalog = InMemoryDictionaryCatalog::new()
        .with_release(release.clone())
        .with_unavailable_url("https://missing.example/dictionary.json")
        .with_artifact(MIRROR_URL, payload.clone());
    let store = InMemoryDictionaryInstallStore::new();
    let store_handle = store.clone();
    let mut distribution = distribution(catalog, trust, store);
    let request = InstallRequest::new(
        release.key().clone(),
        DictionaryReplacementPolicy::RejectExisting,
    );

    let installed = distribution.install(&request).expect("install release");
    let installed_again = distribution.install(&request).expect("idempotent install");

    assert_eq!(installed.state(), DictionaryInstallationState::Verified);
    assert_eq!(installed, installed_again);
    assert_eq!(
        installed
            .source()
            .expect("installation source")
            .installed_at_unix_ms(),
        1_700_000_000_000
    );
    assert_eq!(
        store_handle
            .active_payload("dictionary.ui")
            .expect("active payload"),
        Some(payload)
    );
}

#[test]
fn distribution_rejects_size_digest_signature_publisher_and_payload_identity_mismatch() {
    let valid_payload = dictionary_payload("dictionary.ui", "1.2.0");
    let catalog_release = release(&valid_payload, [PRIMARY_URL], "publisher.example");

    assert_install_error(
        catalog_release.clone(),
        [0_u8; 1].to_vec(),
        trusted_verifier(&catalog_release, "publisher.example"),
        DictionaryDistributionError::SizeMismatch,
    );

    let same_size_wrong_payload = vec![b'x'; valid_payload.len()];
    assert_install_error(
        catalog_release.clone(),
        same_size_wrong_payload,
        trusted_verifier(&catalog_release, "publisher.example"),
        DictionaryDistributionError::DigestMismatch,
    );

    assert_install_error(
        catalog_release.clone(),
        valid_payload.clone(),
        InMemoryTrustVerifier::new(),
        DictionaryDistributionError::InvalidSignature,
    );

    assert_install_error(
        catalog_release.clone(),
        valid_payload.clone(),
        trusted_verifier(&catalog_release, "publisher.other"),
        DictionaryDistributionError::PublisherIdentityMismatch,
    );

    let other_payload = dictionary_payload("dictionary.other", "1.2.0");
    let other_release = release(&other_payload, [PRIMARY_URL], "publisher.example");
    let forged_key = DictionaryReleaseKey::new("glyphshift.official", "dictionary.ui", "1.2.0")
        .expect("release key");
    let forged_release = CatalogRelease::new(
        forged_key,
        other_release.source_locale(),
        other_release.target_locale(),
        other_release.default_presentation_locale(),
        other_release.presentations().to_vec(),
        other_release.artifact().clone(),
    )
    .expect("forged catalog release");
    assert_install_error(
        forged_release.clone(),
        other_payload,
        trusted_verifier(&forged_release, "publisher.example"),
        DictionaryDistributionError::ReleaseIdentityMismatch,
    );
}

#[test]
fn installation_views_derive_modified_missing_and_unmanaged_from_active_content() {
    let payload = dictionary_payload("dictionary.ui", "1.2.0");
    let release = release(&payload, [PRIMARY_URL], "publisher.example");
    let store = InMemoryDictionaryInstallStore::new();
    let store_handle = store.clone();
    let mut distribution = distribution(
        InMemoryDictionaryCatalog::new()
            .with_release(release.clone())
            .with_artifact(PRIMARY_URL, payload),
        trusted_verifier(&release, "publisher.example"),
        store,
    );
    distribution
        .install(&InstallRequest::new(
            release.key().clone(),
            DictionaryReplacementPolicy::RejectExisting,
        ))
        .expect("install release");

    store_handle
        .put_active(dictionary_payload("dictionary.ui", "1.2.1"))
        .expect("modify active");
    assert_eq!(
        distribution.installations().expect("modified views")[0].state(),
        DictionaryInstallationState::Modified
    );

    store_handle
        .remove_active("dictionary.ui")
        .expect("remove active");
    assert_eq!(
        distribution.installations().expect("missing views")[0].state(),
        DictionaryInstallationState::Missing
    );

    store_handle
        .put_active(dictionary_payload("dictionary.local", "0.1.0"))
        .expect("add unmanaged");
    let views = distribution.installations().expect("installation views");
    assert_eq!(views.len(), 2);
    assert_eq!(
        views
            .iter()
            .find(|view| view.dictionary_id() == "dictionary.local")
            .expect("unmanaged view")
            .state(),
        DictionaryInstallationState::Unmanaged
    );
}

#[test]
fn modified_installation_requires_an_explicit_replacement_policy() {
    let first_payload = dictionary_payload("dictionary.ui", "1.2.0");
    let next_payload = dictionary_payload("dictionary.ui", "1.3.0");
    let first_release = release(&first_payload, [PRIMARY_URL], "publisher.example");
    let next_release = release(&next_payload, [MIRROR_URL], "publisher.example");
    let trust = InMemoryTrustVerifier::new()
        .with_trusted_artifact(
            ArtifactStatement::for_release(&first_release),
            first_release.artifact().signature().clone(),
            PublisherIdentity::new("publisher.example").expect("publisher"),
        )
        .with_trusted_artifact(
            ArtifactStatement::for_release(&next_release),
            next_release.artifact().signature().clone(),
            PublisherIdentity::new("publisher.example").expect("publisher"),
        );
    let store = InMemoryDictionaryInstallStore::new();
    let store_handle = store.clone();
    let mut distribution = distribution(
        InMemoryDictionaryCatalog::new()
            .with_release(first_release.clone())
            .with_release(next_release.clone())
            .with_artifact(PRIMARY_URL, first_payload)
            .with_artifact(MIRROR_URL, next_payload.clone()),
        trust,
        store,
    );
    distribution
        .install(&InstallRequest::new(
            first_release.key().clone(),
            DictionaryReplacementPolicy::RejectExisting,
        ))
        .expect("install first release");
    let local_edit = dictionary_payload("dictionary.ui", "1.2.1");
    store_handle
        .put_active(local_edit.clone())
        .expect("modify active");

    assert_eq!(
        distribution
            .install(&InstallRequest::new(
                next_release.key().clone(),
                DictionaryReplacementPolicy::ReplaceVerified,
            ))
            .expect_err("protect local changes"),
        DictionaryDistributionError::LocalChangesConflict
    );
    assert_eq!(
        store_handle
            .active_payload("dictionary.ui")
            .expect("active payload"),
        Some(local_edit)
    );

    let replaced = distribution
        .install(&InstallRequest::new(
            next_release.key().clone(),
            DictionaryReplacementPolicy::ReplaceAny,
        ))
        .expect("explicitly replace local changes");
    assert_eq!(replaced.state(), DictionaryInstallationState::Verified);
    assert_eq!(
        replaced
            .source()
            .expect("installation source")
            .release()
            .release_version(),
        "1.3.0"
    );
    assert_eq!(
        store_handle
            .active_payload("dictionary.ui")
            .expect("active payload"),
        Some(next_payload)
    );
}

fn distribution(
    catalog: InMemoryDictionaryCatalog,
    trust: InMemoryTrustVerifier,
    store: InMemoryDictionaryInstallStore,
) -> DictionaryDistribution {
    DictionaryDistribution::new(
        Box::new(catalog),
        Box::new(trust),
        Box::new(store),
        Box::new(FixedInstallationClock::new(1_700_000_000_000)),
    )
}

fn dictionary_payload(dictionary_id: &str, release_version: &str) -> Vec<u8> {
    DictionaryPackage::create(
        DictionaryCreate::new(dictionary_id, "Dictionary", "en-US", "zh-CN")
            .with_release_version(release_version)
            .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
    )
    .expect("dictionary package")
    .encode_json()
    .expect("encode dictionary")
    .into_bytes()
}

fn release(
    payload: &[u8],
    urls: impl IntoIterator<Item = &'static str>,
    publisher: &str,
) -> CatalogRelease {
    let package =
        DictionaryPackage::decode_json(std::str::from_utf8(payload).expect("utf8 payload"), None)
            .expect("decode payload");
    let metadata = package.view();
    let digest: [u8; 32] = Sha256::digest(payload).into();
    let descriptor = DictionaryArtifactDescriptor::new(
        payload.len() as u64,
        Sha256Digest::new(digest),
        urls,
        PublisherIdentity::new(publisher).expect("publisher"),
        SignatureEnvelope::new("fixture", "test-key", "signed-statement").expect("signature"),
    )
    .expect("descriptor");
    CatalogRelease::new(
        DictionaryReleaseKey::new(
            "glyphshift.official",
            metadata.id(),
            metadata.metadata().release_version(),
        )
        .expect("release key"),
        metadata.metadata().source_locale(),
        metadata.metadata().target_locale(),
        "en-US",
        vec![
            ArtifactPresentation::new("en-US", "Menu English", "menu strings")
                .expect("English presentation"),
            ArtifactPresentation::new("zh-Hans", "菜单中文", "menu strings")
                .expect("Chinese presentation"),
        ],
        descriptor,
    )
    .expect("catalog release")
}

fn trusted_verifier(release: &CatalogRelease, publisher: &str) -> InMemoryTrustVerifier {
    InMemoryTrustVerifier::new().with_trusted_artifact(
        ArtifactStatement::for_release(release),
        release.artifact().signature().clone(),
        PublisherIdentity::new(publisher).expect("publisher"),
    )
}

fn assert_install_error(
    release: CatalogRelease,
    payload: Vec<u8>,
    trust: InMemoryTrustVerifier,
    expected: DictionaryDistributionError,
) {
    let mut distribution = distribution(
        InMemoryDictionaryCatalog::new()
            .with_release(release.clone())
            .with_artifact(PRIMARY_URL, payload),
        trust,
        InMemoryDictionaryInstallStore::new(),
    );
    let error = distribution
        .install(&InstallRequest::new(
            release.key().clone(),
            DictionaryReplacementPolicy::RejectExisting,
        ))
        .expect_err("reject invalid artifact");
    assert_eq!(error, expected);
    assert!(distribution
        .installations()
        .expect("installation views")
        .is_empty());
}
