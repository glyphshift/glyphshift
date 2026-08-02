//! Verified discovery and installation of portable Dictionary packages.

mod memory;
mod model;
mod ports;
mod service;

pub use memory::{
    FixedInstallationClock, InMemoryDictionaryCatalog, InMemoryDictionaryInstallStore,
    InMemoryTrustVerifier,
};
pub use model::{
    ArtifactPresentation, ArtifactStatement, CatalogContractError, CatalogPage, CatalogQuery,
    CatalogRelease, CatalogReleaseSummary, CatalogSourcePage, DictionaryArtifactDescriptor,
    DictionaryInstallationSource, DictionaryInstallationState, DictionaryInstallationView,
    DictionaryReleaseKey, DictionaryReplacementPolicy, InstallRequest, PublisherIdentity,
    Sha256Digest, SignatureEnvelope, VerifiedDictionaryArtifact,
    DICTIONARY_ARTIFACT_STATEMENT_SCHEMA, DICTIONARY_MEDIA_TYPE,
};
pub use ports::{
    ArtifactTrustVerifier, CatalogPortError, DictionaryDistributionPort, DictionaryInstallStore,
    InstallStoreError, InstallationClock, SystemInstallationClock, TrustVerifierError,
};
pub use service::{DictionaryDistribution, DictionaryDistributionError};

pub const DEFAULT_MAX_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;
