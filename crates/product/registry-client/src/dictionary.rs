use crate::{Error, TrustStore, VerifiedProof};
use glyphshift_dictionary_distribution::*;
use glyphshift_dictionary_package::DictionaryPackage;
use sha2::{Digest, Sha256};
use std::path::Path;

const SCHEME: &str = "glyphshift-registry-jws-v1";

// One exact verified release, not a fake global search/catalog implementation.
struct ExactRelease {
    release: CatalogRelease,
    bytes: Vec<u8>,
    url: String,
}
impl DictionaryDistributionPort for ExactRelease {
    fn query(&mut self, _: &CatalogQuery) -> Result<CatalogSourcePage, CatalogPortError> {
        Err(CatalogPortError::Unavailable)
    }
    fn release(&mut self, key: &DictionaryReleaseKey) -> Result<CatalogRelease, CatalogPortError> {
        if key != self.release.key() {
            return Err(CatalogPortError::NotFound);
        }
        Ok(self.release.clone())
    }
    fn fetch(&mut self, url: &str, limit: u64) -> Result<Vec<u8>, CatalogPortError> {
        if url != self.url {
            return Err(CatalogPortError::InvalidResponse);
        }
        if self.bytes.len() as u64 > limit {
            return Err(CatalogPortError::TooLarge);
        }
        Ok(self.bytes.clone())
    }
}

struct DictionaryTrust(TrustStore);
impl ArtifactTrustVerifier for DictionaryTrust {
    fn verify(
        &mut self,
        s: &ArtifactStatement,
        signature: &SignatureEnvelope,
    ) -> Result<PublisherIdentity, TrustVerifierError> {
        if signature.scheme() != SCHEME
            || s.schema() != DICTIONARY_ARTIFACT_STATEMENT_SCHEMA
            || s.digest_algorithm() != "sha256"
        {
            return Err(TrustVerifierError::InvalidSignature);
        }
        let proof = self
            .0
            .verify_jws(signature.value())
            .map_err(|_| TrustVerifierError::InvalidSignature)?;
        let actual = proof.statement();
        let a = &actual.artifact;
        if signature.key_id() != proof.key_id()
            || a.kind != "dictionary"
            || a.package_id != s.dictionary_id()
            || a.version != s.release_version()
            || a.media_type != s.media_type()
            || a.size != s.size()
            || a.sha256 != s.digest().to_hex().as_ref()
            || actual.publisher_user_key != s.publisher_identity().as_str()
        {
            return Err(TrustVerifierError::InvalidSignature);
        }
        PublisherIdentity::new(actual.publisher_user_key.as_str())
            .map_err(|_| TrustVerifierError::UntrustedPublisher)
    }
}

pub(crate) fn install(
    trust: TrustStore,
    proof: VerifiedProof,
    bytes: Vec<u8>,
    url: String,
    catalog_id: &str,
    root: &Path,
    replacement: DictionaryReplacementPolicy,
) -> Result<DictionaryInstallationView, Error> {
    let package = DictionaryPackage::decode_publication_json(
        std::str::from_utf8(&bytes).map_err(|_| Error::InvalidArtifact)?,
        None,
    )
    .map_err(|_| Error::InvalidArtifact)?;
    let view = package.view();
    let meta = view.metadata();
    let key = DictionaryReleaseKey::new(catalog_id, meta.id(), meta.release_version())
        .map_err(|_| Error::Configuration)?;
    let descriptor = DictionaryArtifactDescriptor::new(
        bytes.len() as u64,
        Sha256Digest::new(Sha256::digest(&bytes).into()),
        [url.as_str()],
        PublisherIdentity::new(proof.statement().publisher_user_key.as_str())
            .map_err(|_| Error::InvalidProof)?,
        SignatureEnvelope::new(SCHEME, proof.key_id(), proof.jws())
            .map_err(|_| Error::InvalidProof)?,
    )
    .map_err(|_| Error::InvalidArtifact)?;
    let presentation =
        ArtifactPresentation::new(meta.source_locale(), meta.name(), meta.description())
            .map_err(|_| Error::InvalidArtifact)?;
    let release = CatalogRelease::new(
        key.clone(),
        meta.source_locale(),
        meta.target_locale(),
        meta.source_locale(),
        vec![presentation],
        descriptor,
    )
    .map_err(|_| Error::InvalidArtifact)?;
    let store = FileDictionaryInstallStore::open(root).map_err(|_| Error::Storage)?;
    let mut distribution = DictionaryDistribution::new(
        Box::new(ExactRelease {
            release,
            bytes,
            url,
        }),
        Box::new(DictionaryTrust(trust)),
        Box::new(store),
        Box::new(SystemInstallationClock),
    );
    distribution
        .install(&InstallRequest::new(key, replacement))
        .map_err(|e| match e {
            DictionaryDistributionError::LocalChangesConflict => Error::LocalChangesConflict,
            DictionaryDistributionError::StorageFailure => Error::Storage,
            _ => Error::InvalidArtifact,
        })
}
