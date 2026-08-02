use crate::{
    ArtifactStatement, CatalogQuery, CatalogRelease, CatalogSourcePage, DictionaryInstallationView,
    DictionaryReleaseKey, DictionaryReplacementPolicy, PublisherIdentity, SignatureEnvelope,
    VerifiedDictionaryArtifact,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogPortError {
    Unavailable,
    NotFound,
    TooLarge,
    InvalidResponse,
}

pub trait DictionaryDistributionPort: Send {
    fn query(&mut self, query: &CatalogQuery) -> Result<CatalogSourcePage, CatalogPortError>;

    fn release(&mut self, key: &DictionaryReleaseKey) -> Result<CatalogRelease, CatalogPortError>;

    fn fetch(&mut self, download_url: &str, byte_limit: u64) -> Result<Vec<u8>, CatalogPortError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustVerifierError {
    InvalidSignature,
    UntrustedPublisher,
    Unavailable,
}

pub trait ArtifactTrustVerifier: Send {
    fn verify(
        &mut self,
        statement: &ArtifactStatement,
        signature: &SignatureEnvelope,
    ) -> Result<PublisherIdentity, TrustVerifierError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallStoreError {
    LocalChangesConflict,
    StorageFailure,
}

pub trait DictionaryInstallStore: Send {
    fn install(
        &mut self,
        artifact: VerifiedDictionaryArtifact,
        replacement: DictionaryReplacementPolicy,
    ) -> Result<DictionaryInstallationView, InstallStoreError>;

    fn installations(&mut self) -> Result<Vec<DictionaryInstallationView>, InstallStoreError>;
}

pub trait InstallationClock: Send {
    fn now_unix_ms(&mut self) -> u64;
}

#[derive(Default)]
pub struct SystemInstallationClock;

impl InstallationClock for SystemInstallationClock {
    fn now_unix_ms(&mut self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as u64)
    }
}
