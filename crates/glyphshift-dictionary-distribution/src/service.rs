use crate::model::valid_locale;
use crate::{
    ArtifactStatement, ArtifactTrustVerifier, CatalogContractError, CatalogPage, CatalogPortError,
    CatalogQuery, CatalogRelease, CatalogReleaseSummary, DictionaryDistributionPort,
    DictionaryInstallStore, DictionaryInstallationView, InstallRequest, InstallStoreError,
    InstallationClock, TrustVerifierError, VerifiedDictionaryArtifact, DEFAULT_MAX_ARTIFACT_BYTES,
};
use glyphshift_dictionary_package::DictionaryPackage;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DictionaryDistributionError {
    InvalidCatalog,
    CatalogUnavailable,
    ReleaseMissing,
    ArtifactTooLarge,
    SizeMismatch,
    DigestMismatch,
    InvalidSignature,
    UntrustedPublisher,
    PublisherIdentityMismatch,
    TrustUnavailable,
    InvalidPayload,
    ReleaseIdentityMismatch,
    LocalChangesConflict,
    StorageFailure,
}

pub struct DictionaryDistribution {
    catalog: Box<dyn DictionaryDistributionPort>,
    trust: Box<dyn ArtifactTrustVerifier>,
    store: Box<dyn DictionaryInstallStore>,
    clock: Box<dyn InstallationClock>,
    max_artifact_bytes: u64,
}

impl DictionaryDistribution {
    #[must_use]
    pub fn new(
        catalog: Box<dyn DictionaryDistributionPort>,
        trust: Box<dyn ArtifactTrustVerifier>,
        store: Box<dyn DictionaryInstallStore>,
        clock: Box<dyn InstallationClock>,
    ) -> Self {
        Self {
            catalog,
            trust,
            store,
            clock,
            max_artifact_bytes: DEFAULT_MAX_ARTIFACT_BYTES,
        }
    }

    pub fn with_max_artifact_bytes(
        mut self,
        max_artifact_bytes: u64,
    ) -> Result<Self, CatalogContractError> {
        if max_artifact_bytes == 0 {
            return Err(CatalogContractError::InvalidArtifact);
        }
        self.max_artifact_bytes = max_artifact_bytes;
        Ok(self)
    }

    pub fn query(
        &mut self,
        query: &CatalogQuery,
        requested_presentation_locale: &str,
    ) -> Result<CatalogPage, DictionaryDistributionError> {
        query
            .validate()
            .map_err(|_| DictionaryDistributionError::InvalidCatalog)?;
        if !valid_locale(requested_presentation_locale) {
            return Err(DictionaryDistributionError::InvalidCatalog);
        }
        let source_page = self.catalog.query(query).map_err(map_catalog_query_error)?;
        let (releases, next_cursor) = source_page.into_parts();
        let releases = releases
            .into_iter()
            .map(|release| {
                validate_release(&release, self.max_artifact_bytes)?;
                let presentation = release
                    .presentation_for(requested_presentation_locale)
                    .ok_or(DictionaryDistributionError::InvalidCatalog)?;
                Ok(CatalogReleaseSummary::from_release(&release, presentation))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CatalogPage::new(releases, next_cursor))
    }

    pub fn install(
        &mut self,
        request: &InstallRequest,
    ) -> Result<DictionaryInstallationView, DictionaryDistributionError> {
        let release = self
            .catalog
            .release(request.release())
            .map_err(map_release_error)?;
        validate_release(&release, self.max_artifact_bytes)?;
        if release.key() != request.release() {
            return Err(DictionaryDistributionError::InvalidCatalog);
        }

        let payload = self.fetch_artifact(&release)?;
        if payload.len() as u64 != release.artifact().size() {
            return Err(DictionaryDistributionError::SizeMismatch);
        }
        let observed_digest = Sha256::digest(&payload);
        if observed_digest
            .as_slice()
            .ct_eq(&release.artifact().digest().as_bytes())
            .unwrap_u8()
            != 1
        {
            return Err(DictionaryDistributionError::DigestMismatch);
        }

        let statement = ArtifactStatement::for_release(&release);
        let publisher_identity = self
            .trust
            .verify(&statement, release.artifact().signature())
            .map_err(map_trust_error)?;
        if &publisher_identity != release.artifact().publisher_identity() {
            return Err(DictionaryDistributionError::PublisherIdentityMismatch);
        }

        let source = std::str::from_utf8(&payload)
            .map_err(|_| DictionaryDistributionError::InvalidPayload)?;
        let package = DictionaryPackage::decode_json(source, None)
            .map_err(|_| DictionaryDistributionError::InvalidPayload)?;
        let metadata = package.view();
        if metadata.id() != request.release().dictionary_id()
            || metadata.metadata().release_version() != request.release().release_version()
        {
            return Err(DictionaryDistributionError::ReleaseIdentityMismatch);
        }

        self.store
            .install(
                VerifiedDictionaryArtifact {
                    release: request.release().clone(),
                    payload,
                    package,
                    statement,
                    signature: release.artifact().signature().clone(),
                    publisher_identity,
                    installed_at_unix_ms: self.clock.now_unix_ms(),
                },
                request.replacement(),
            )
            .map_err(map_store_error)
    }

    pub fn installations(
        &mut self,
    ) -> Result<Vec<DictionaryInstallationView>, DictionaryDistributionError> {
        self.store.installations().map_err(map_store_error)
    }

    fn fetch_artifact(
        &mut self,
        release: &CatalogRelease,
    ) -> Result<Vec<u8>, DictionaryDistributionError> {
        let byte_limit = release.artifact().size().min(self.max_artifact_bytes);
        let mut last_error = CatalogPortError::Unavailable;
        for download_url in release.artifact().download_urls() {
            match self.catalog.fetch(download_url, byte_limit) {
                Ok(payload) => return Ok(payload),
                Err(CatalogPortError::TooLarge) => {
                    return Err(DictionaryDistributionError::SizeMismatch);
                }
                Err(error) => last_error = error,
            }
        }
        Err(match last_error {
            CatalogPortError::InvalidResponse => DictionaryDistributionError::InvalidCatalog,
            CatalogPortError::TooLarge => DictionaryDistributionError::SizeMismatch,
            CatalogPortError::Unavailable | CatalogPortError::NotFound => {
                DictionaryDistributionError::CatalogUnavailable
            }
        })
    }
}

fn validate_release(
    release: &CatalogRelease,
    max_artifact_bytes: u64,
) -> Result<(), DictionaryDistributionError> {
    release
        .validate()
        .map_err(|_| DictionaryDistributionError::InvalidCatalog)?;
    if release.artifact().size() > max_artifact_bytes {
        return Err(DictionaryDistributionError::ArtifactTooLarge);
    }
    Ok(())
}

fn map_catalog_query_error(error: CatalogPortError) -> DictionaryDistributionError {
    match error {
        CatalogPortError::InvalidResponse => DictionaryDistributionError::InvalidCatalog,
        CatalogPortError::Unavailable | CatalogPortError::NotFound | CatalogPortError::TooLarge => {
            DictionaryDistributionError::CatalogUnavailable
        }
    }
}

fn map_release_error(error: CatalogPortError) -> DictionaryDistributionError {
    match error {
        CatalogPortError::NotFound => DictionaryDistributionError::ReleaseMissing,
        CatalogPortError::InvalidResponse => DictionaryDistributionError::InvalidCatalog,
        CatalogPortError::Unavailable | CatalogPortError::TooLarge => {
            DictionaryDistributionError::CatalogUnavailable
        }
    }
}

fn map_trust_error(error: TrustVerifierError) -> DictionaryDistributionError {
    match error {
        TrustVerifierError::InvalidSignature => DictionaryDistributionError::InvalidSignature,
        TrustVerifierError::UntrustedPublisher => DictionaryDistributionError::UntrustedPublisher,
        TrustVerifierError::Unavailable => DictionaryDistributionError::TrustUnavailable,
    }
}

fn map_store_error(error: InstallStoreError) -> DictionaryDistributionError {
    match error {
        InstallStoreError::LocalChangesConflict => {
            DictionaryDistributionError::LocalChangesConflict
        }
        InstallStoreError::StorageFailure => DictionaryDistributionError::StorageFailure,
    }
}
