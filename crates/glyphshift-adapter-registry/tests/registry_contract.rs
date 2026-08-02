use glyphshift_adapter_registry::{
    AdapterBinding, AdapterDescriptor, AdapterHostBinding, AdapterPackage, AdapterPackageSet,
    AdapterRegistry, AdapterRequirement, AdapterTrustPolicy, AdapterVersion,
    AdapterVersionRequirement, ArtifactHash, PackageArtifactId, RegistryError, SignerId,
};
use glyphshift_domain::AbiVersion;
use glyphshift_domain::{AdapterId, ApplyModel, Feature, Placement, RegistryRevision, TargetFacts};

const TRUSTED_SIGNER: &str = "example.signer.trusted";

const fn foundation_version() -> AdapterVersion {
    AdapterVersion::new(1, 0, 0)
}

fn default_artifact_id() -> PackageArtifactId {
    PackageArtifactId::new("synthetic.default.artifact")
}

fn package(descriptor: AdapterDescriptor) -> AdapterPackage {
    let hash = ArtifactHash::sha256([0xA5; 32]);
    AdapterPackage::new(
        descriptor,
        default_artifact_id(),
        SignerId::new(TRUSTED_SIGNER),
        hash,
        hash,
    )
}

fn registry_for(adapter_id: &str) -> AdapterRegistry {
    AdapterRegistry::new(AdapterTrustPolicy::new(
        [SignerId::new(TRUSTED_SIGNER)],
        [AdapterId::new(adapter_id)],
    ))
}

fn requirement(
    adapter_id: AdapterId,
    features: impl IntoIterator<Item = Feature>,
) -> AdapterRequirement {
    AdapterRequirement::new(
        adapter_id,
        AdapterVersionRequirement::Exact(foundation_version()),
        features,
    )
}

#[test]
fn adr_003_resolves_an_adapter_added_at_runtime() {
    let descriptor = AdapterDescriptor::new(
        AdapterId::new("example.synthetic.writeback"),
        foundation_version(),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let packages = AdapterPackageSet::new([package(descriptor.clone())]);
    let mut registry = registry_for("example.synthetic.writeback");

    let revision = registry.reload(packages).expect("reload should succeed");

    assert_eq!(revision, RegistryRevision::new(1));

    let binding = registry
        .resolve(
            &requirement(
                AdapterId::new("example.synthetic.writeback"),
                [Feature::TextReplace],
            ),
            &TargetFacts::new("windows", "x86_64"),
        )
        .expect("runtime adapter should resolve");

    assert_eq!(
        binding,
        AdapterBinding {
            descriptor,
            adapter_id: AdapterId::new("example.synthetic.writeback"),
            version: foundation_version(),
            apply_model: ApplyModel::InlineRender,
            artifact_hash: ArtifactHash::sha256([0xA5; 32]),
            host: AdapterHostBinding::TargetProcess {
                library: default_artifact_id(),
            },
            features: vec![Feature::TextReplace],
        }
    );
}

#[test]
fn adr_010_rejects_an_invalid_apply_model_and_placement() {
    let descriptor = AdapterDescriptor::new(
        AdapterId::new("example.synthetic.invalid-placement"),
        foundation_version(),
        ApplyModel::InlineRender,
        Placement::IsolatedWorker,
        [Feature::TextReplace],
    );
    let mut registry = registry_for("example.synthetic.invalid-placement");
    registry
        .reload(AdapterPackageSet::new([package(descriptor)]))
        .expect("descriptor identity should load before target resolution");

    let result = registry.resolve(
        &requirement(
            AdapterId::new("example.synthetic.invalid-placement"),
            [Feature::TextReplace],
        ),
        &TargetFacts::new("windows", "x86_64"),
    );

    assert_eq!(
        result,
        Err(RegistryError::UnsupportedHostModel {
            apply_model: ApplyModel::InlineRender,
            placement: Placement::IsolatedWorker,
        })
    );
}

#[test]
fn adr_011_rejects_writeback_features_on_observe_only_adapters() {
    let descriptor = AdapterDescriptor::new(
        AdapterId::new("example.synthetic.observe"),
        foundation_version(),
        ApplyModel::ObserveOnly,
        Placement::IsolatedWorker,
        [Feature::TextObserve, Feature::TextReplace],
    );
    let mut registry = registry_for("example.synthetic.observe");

    let result = registry.reload(AdapterPackageSet::new([package(descriptor)]));

    assert_eq!(
        result,
        Err(RegistryError::ObserveOnlyWriteback {
            adapter_id: AdapterId::new("example.synthetic.observe"),
            feature: Feature::TextReplace,
        })
    );
}

#[test]
fn adr_008_rejects_a_target_architecture_mismatch() {
    let descriptor = AdapterDescriptor::new(
        AdapterId::new("example.synthetic.x64-only"),
        foundation_version(),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    )
    .with_architectures(["x86_64"]);
    let mut registry = registry_for("example.synthetic.x64-only");
    registry
        .reload(AdapterPackageSet::new([package(descriptor)]))
        .expect("descriptor should load");

    let result = registry.resolve(
        &requirement(
            AdapterId::new("example.synthetic.x64-only"),
            [Feature::TextReplace],
        ),
        &TargetFacts::new("windows", "aarch64"),
    );

    assert_eq!(
        result,
        Err(RegistryError::UnsupportedArchitecture {
            adapter_id: AdapterId::new("example.synthetic.x64-only"),
            architecture: "aarch64".into(),
        })
    );
}

#[test]
fn adr_014_rejects_a_target_platform_mismatch() {
    let adapter_id = AdapterId::new("example.synthetic.windows-only");
    let descriptor = AdapterDescriptor::new(
        adapter_id.clone(),
        foundation_version(),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    )
    .with_platforms(["windows"])
    .with_architectures(["x86_64"]);
    let mut registry = registry_for("example.synthetic.windows-only");
    registry
        .reload(AdapterPackageSet::new([package(descriptor)]))
        .expect("descriptor should load before target compatibility is checked");

    let result = registry.resolve(
        &requirement(adapter_id.clone(), [Feature::TextReplace]),
        &TargetFacts::new("macos", "x86_64"),
    );

    assert_eq!(
        result,
        Err(RegistryError::UnsupportedPlatform {
            adapter_id,
            platform: "macos".into(),
        })
    );
}

#[test]
fn adr_007_rejects_an_incompatible_abi_major() {
    let descriptor = AdapterDescriptor::new(
        AdapterId::new("example.synthetic.future-abi"),
        foundation_version(),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    )
    .with_abi(AbiVersion::new(2, 0));
    let mut registry = registry_for("example.synthetic.future-abi");
    registry
        .reload(AdapterPackageSet::new([package(descriptor)]))
        .expect("descriptor identity should load before ABI resolution");

    let result = registry.resolve(
        &requirement(
            AdapterId::new("example.synthetic.future-abi"),
            [Feature::TextReplace],
        ),
        &TargetFacts::new("windows", "x86_64"),
    );

    assert_eq!(
        result,
        Err(RegistryError::IncompatibleAbi {
            adapter_id: AdapterId::new("example.synthetic.future-abi"),
            host: AbiVersion::new(1, 0),
            adapter: AbiVersion::new(2, 0),
        })
    );
}

#[test]
fn adr_005_rejects_an_artifact_hash_mismatch() {
    let descriptor = AdapterDescriptor::new(
        AdapterId::new("example.synthetic.tampered"),
        foundation_version(),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let declared_hash = ArtifactHash::sha256([0x11; 32]);
    let observed_hash = ArtifactHash::sha256([0x22; 32]);
    let package = AdapterPackage::new(
        descriptor,
        default_artifact_id(),
        SignerId::new(TRUSTED_SIGNER),
        declared_hash,
        observed_hash,
    );
    let mut registry = registry_for("example.synthetic.tampered");
    registry
        .reload(AdapterPackageSet::new([package]))
        .expect("package identity should load before integrity resolution");

    let result = registry.resolve(
        &requirement(
            AdapterId::new("example.synthetic.tampered"),
            [Feature::TextReplace],
        ),
        &TargetFacts::new("windows", "x86_64"),
    );

    assert_eq!(
        result,
        Err(RegistryError::ArtifactHashMismatch {
            adapter_id: AdapterId::new("example.synthetic.tampered"),
            declared: declared_hash,
            observed: observed_hash,
        })
    );
}

#[test]
fn adr_006_rejects_an_untrusted_signer_with_an_explanation() {
    let adapter_id = AdapterId::new("example.synthetic.untrusted");
    let descriptor = AdapterDescriptor::new(
        adapter_id.clone(),
        foundation_version(),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let hash = ArtifactHash::sha256([0x33; 32]);
    let signer = SignerId::new("example.signer.unknown");
    let package = AdapterPackage::new(
        descriptor,
        default_artifact_id(),
        signer.clone(),
        hash,
        hash,
    );
    let policy = AdapterTrustPolicy::new(
        [SignerId::new("example.signer.trusted")],
        [adapter_id.clone()],
    );
    let mut registry = AdapterRegistry::new(policy);
    registry
        .reload(AdapterPackageSet::new([package]))
        .expect("package identity should load before trust resolution");

    let result = registry.resolve(
        &requirement(adapter_id.clone(), [Feature::TextReplace]),
        &TargetFacts::new("windows", "x86_64"),
    );

    assert_eq!(
        result,
        Err(RegistryError::UntrustedSigner { adapter_id, signer })
    );
}

#[test]
fn adr_006_rejects_missing_user_authorization_with_an_explanation() {
    let adapter_id = AdapterId::new("example.synthetic.not-authorized");
    let descriptor = AdapterDescriptor::new(
        adapter_id.clone(),
        foundation_version(),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let hash = ArtifactHash::sha256([0x44; 32]);
    let signer = SignerId::new("example.signer.trusted");
    let package = AdapterPackage::new(
        descriptor,
        default_artifact_id(),
        signer.clone(),
        hash,
        hash,
    );
    let policy = AdapterTrustPolicy::new([signer], std::iter::empty::<AdapterId>());
    let mut registry = AdapterRegistry::new(policy);
    registry
        .reload(AdapterPackageSet::new([package]))
        .expect("package identity should load before authorization resolution");

    let result = registry.resolve(
        &requirement(adapter_id.clone(), [Feature::TextReplace]),
        &TargetFacts::new("windows", "x86_64"),
    );

    assert_eq!(
        result,
        Err(RegistryError::AuthorizationMissing { adapter_id })
    );
}

#[test]
fn adr_009_rejects_a_feature_not_declared_by_the_adapter() {
    let adapter_id = AdapterId::new("example.synthetic.text-only");
    let descriptor = AdapterDescriptor::new(
        adapter_id.clone(),
        foundation_version(),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let mut registry = registry_for("example.synthetic.text-only");
    registry
        .reload(AdapterPackageSet::new([package(descriptor)]))
        .expect("package should load");

    let result = registry.resolve(
        &requirement(adapter_id.clone(), [Feature::FontSubstitute]),
        &TargetFacts::new("windows", "x86_64"),
    );

    assert_eq!(
        result,
        Err(RegistryError::FeatureUnavailable {
            adapter_id,
            feature: Feature::FontSubstitute,
        })
    );
}

#[test]
fn adr_012_resolves_the_exact_requested_version_deterministically() {
    let adapter_id = AdapterId::new("example.synthetic.versioned");
    let version_one = AdapterVersion::new(1, 0, 0);
    let version_two = AdapterVersion::new(2, 0, 0);
    let descriptor_one = AdapterDescriptor::new(
        adapter_id.clone(),
        version_one,
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let descriptor_two = AdapterDescriptor::new(
        adapter_id.clone(),
        version_two,
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let hash_one = ArtifactHash::sha256([0x51; 32]);
    let hash_two = ArtifactHash::sha256([0x52; 32]);
    let package_one = AdapterPackage::new(
        descriptor_one.clone(),
        default_artifact_id(),
        SignerId::new(TRUSTED_SIGNER),
        hash_one,
        hash_one,
    );
    let package_two = AdapterPackage::new(
        descriptor_two,
        default_artifact_id(),
        SignerId::new(TRUSTED_SIGNER),
        hash_two,
        hash_two,
    );
    let mut registry = registry_for("example.synthetic.versioned");
    registry
        .reload(AdapterPackageSet::new([package_two, package_one]))
        .expect("both versions should coexist");

    let binding = registry
        .resolve(
            &AdapterRequirement::new(
                adapter_id.clone(),
                AdapterVersionRequirement::Exact(version_one),
                [Feature::TextReplace],
            ),
            &TargetFacts::new("windows", "x86_64"),
        )
        .expect("the exact requested version should resolve");

    assert_eq!(
        binding,
        AdapterBinding {
            descriptor: descriptor_one,
            adapter_id,
            version: version_one,
            apply_model: ApplyModel::InlineRender,
            artifact_hash: hash_one,
            host: AdapterHostBinding::TargetProcess {
                library: default_artifact_id(),
            },
            features: vec![Feature::TextReplace],
        }
    );
}

#[test]
fn adr_013_rejects_conflicting_content_without_replacing_the_registry() {
    let stable_id = AdapterId::new("example.synthetic.stable");
    let conflict_id = AdapterId::new("example.synthetic.conflict");
    let version = foundation_version();
    let stable_descriptor = AdapterDescriptor::new(
        stable_id.clone(),
        version,
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let conflict_descriptor = AdapterDescriptor::new(
        conflict_id.clone(),
        version,
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let stable_hash = ArtifactHash::sha256([0x60; 32]);
    let first_hash = ArtifactHash::sha256([0x61; 32]);
    let second_hash = ArtifactHash::sha256([0x62; 32]);
    let stable_package = AdapterPackage::new(
        stable_descriptor,
        default_artifact_id(),
        SignerId::new(TRUSTED_SIGNER),
        stable_hash,
        stable_hash,
    );
    let first_conflict = AdapterPackage::new(
        conflict_descriptor.clone(),
        default_artifact_id(),
        SignerId::new(TRUSTED_SIGNER),
        first_hash,
        first_hash,
    );
    let second_conflict = AdapterPackage::new(
        conflict_descriptor,
        default_artifact_id(),
        SignerId::new(TRUSTED_SIGNER),
        second_hash,
        second_hash,
    );
    let policy = AdapterTrustPolicy::new(
        [SignerId::new(TRUSTED_SIGNER)],
        [stable_id.clone(), conflict_id.clone()],
    );
    let mut registry = AdapterRegistry::new(policy);

    assert_eq!(
        registry
            .reload(AdapterPackageSet::new([stable_package.clone()]))
            .expect("initial package should load"),
        RegistryRevision::new(1)
    );

    let rejection = registry.reload(AdapterPackageSet::new([first_conflict, second_conflict]));

    assert_eq!(
        rejection,
        Err(RegistryError::ConflictingAdapterContent {
            adapter_id: conflict_id,
            version,
            registered: first_hash,
            incoming: second_hash,
        })
    );
    registry
        .resolve(
            &requirement(stable_id, [Feature::TextReplace]),
            &TargetFacts::new("windows", "x86_64"),
        )
        .expect("failed reload must preserve the previous registry");
    assert_eq!(
        registry
            .reload(AdapterPackageSet::new([stable_package]))
            .expect("the next successful reload should advance once"),
        RegistryRevision::new(2)
    );
}

#[test]
fn adr_001_returns_a_target_process_host_binding() {
    let adapter_id = AdapterId::new("example.synthetic.inline-host");
    let artifact_id = PackageArtifactId::new("synthetic.inline.library");
    let descriptor = AdapterDescriptor::new(
        adapter_id.clone(),
        foundation_version(),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let hash = ArtifactHash::sha256([0x71; 32]);
    let package = AdapterPackage::new(
        descriptor,
        artifact_id.clone(),
        SignerId::new(TRUSTED_SIGNER),
        hash,
        hash,
    );
    let mut registry = registry_for("example.synthetic.inline-host");
    registry
        .reload(AdapterPackageSet::new([package]))
        .expect("target-process package should load");

    let binding = registry
        .resolve(
            &requirement(adapter_id.clone(), [Feature::TextReplace]),
            &TargetFacts::new("windows", "x86_64"),
        )
        .expect("target-process package should resolve");

    assert_eq!(
        binding,
        AdapterBinding {
            descriptor: AdapterDescriptor::new(
                adapter_id.clone(),
                foundation_version(),
                ApplyModel::InlineRender,
                Placement::TargetProcess,
                [Feature::TextReplace],
            ),
            adapter_id,
            version: foundation_version(),
            apply_model: ApplyModel::InlineRender,
            artifact_hash: hash,
            host: AdapterHostBinding::TargetProcess {
                library: artifact_id,
            },
            features: vec![Feature::TextReplace],
        }
    );
}

#[test]
fn adr_002_returns_an_isolated_worker_binding_without_catalog_location() {
    let adapter_id = AdapterId::new("example.synthetic.worker-host");
    let artifact_id = PackageArtifactId::new("synthetic.worker.executable");
    let descriptor = AdapterDescriptor::new(
        adapter_id.clone(),
        foundation_version(),
        ApplyModel::ExternalProtocol,
        Placement::IsolatedWorker,
        [Feature::TextReplace],
    );
    let hash = ArtifactHash::sha256([0x72; 32]);
    let package = AdapterPackage::new(
        descriptor,
        artifact_id.clone(),
        SignerId::new(TRUSTED_SIGNER),
        hash,
        hash,
    );
    let mut registry = registry_for("example.synthetic.worker-host");
    registry
        .reload(AdapterPackageSet::new([package]))
        .expect("isolated-worker package should load");

    let binding = registry
        .resolve(
            &requirement(adapter_id.clone(), [Feature::TextReplace]),
            &TargetFacts::new("windows", "x86_64"),
        )
        .expect("isolated-worker package should resolve");

    assert_eq!(
        binding,
        AdapterBinding {
            descriptor: AdapterDescriptor::new(
                adapter_id.clone(),
                foundation_version(),
                ApplyModel::ExternalProtocol,
                Placement::IsolatedWorker,
                [Feature::TextReplace],
            ),
            adapter_id,
            version: foundation_version(),
            apply_model: ApplyModel::ExternalProtocol,
            artifact_hash: hash,
            host: AdapterHostBinding::IsolatedWorker {
                executable: artifact_id,
            },
            features: vec![Feature::TextReplace],
        }
    );
}
