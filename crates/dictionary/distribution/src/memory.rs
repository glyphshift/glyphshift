use crate::{
    ArtifactStatement, ArtifactTrustVerifier, CatalogPortError, CatalogQuery, CatalogRelease,
    CatalogSourcePage, DictionaryDistributionPort, DictionaryInstallStore,
    DictionaryInstallationSource, DictionaryInstallationState, DictionaryInstallationView,
    DictionaryReleaseKey, DictionaryReplacementPolicy, InstallStoreError, InstallationClock,
    PublisherIdentity, Sha256Digest, SignatureEnvelope, TrustVerifierError,
    VerifiedDictionaryArtifact,
};
use glyphshift_dictionary_package::DictionaryPackage;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
enum ArtifactResponse {
    Bytes(Vec<u8>),
    Unavailable,
}

#[derive(Default)]
pub struct InMemoryDictionaryCatalog {
    releases: Vec<CatalogRelease>,
    artifacts: BTreeMap<Box<str>, ArtifactResponse>,
    unavailable: bool,
}

impl InMemoryDictionaryCatalog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_release(mut self, release: CatalogRelease) -> Self {
        self.releases.push(release);
        self
    }

    #[must_use]
    pub fn with_artifact(mut self, download_url: impl Into<Box<str>>, payload: Vec<u8>) -> Self {
        self.artifacts
            .insert(download_url.into(), ArtifactResponse::Bytes(payload));
        self
    }

    #[must_use]
    pub fn with_unavailable_url(mut self, download_url: impl Into<Box<str>>) -> Self {
        self.artifacts
            .insert(download_url.into(), ArtifactResponse::Unavailable);
        self
    }

    #[must_use]
    pub const fn unavailable(mut self) -> Self {
        self.unavailable = true;
        self
    }
}

impl DictionaryDistributionPort for InMemoryDictionaryCatalog {
    fn query(&mut self, query: &CatalogQuery) -> Result<CatalogSourcePage, CatalogPortError> {
        if self.unavailable {
            return Err(CatalogPortError::Unavailable);
        }
        let text = query.text().to_ascii_lowercase();
        let releases = self
            .releases
            .iter()
            .filter(|release| {
                (text.is_empty()
                    || release
                        .key()
                        .dictionary_id()
                        .to_ascii_lowercase()
                        .contains(&text)
                    || release.presentations().iter().any(|presentation| {
                        presentation.name().to_ascii_lowercase().contains(&text)
                            || presentation.summary().to_ascii_lowercase().contains(&text)
                            || presentation
                                .tags()
                                .iter()
                                .any(|tag| tag.to_ascii_lowercase().contains(&text))
                    }))
                    && query
                        .source_locale()
                        .is_none_or(|locale| release.source_locale().eq_ignore_ascii_case(locale))
                    && query
                        .target_locale()
                        .is_none_or(|locale| release.target_locale().eq_ignore_ascii_case(locale))
                    && query.tag().is_none_or(|expected| {
                        release.presentations().iter().any(|presentation| {
                            presentation
                                .tags()
                                .iter()
                                .any(|tag| tag.eq_ignore_ascii_case(expected))
                        })
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        let start = query
            .cursor()
            .map_or(Ok(0), str::parse::<usize>)
            .map_err(|_| CatalogPortError::InvalidResponse)?;
        if start > releases.len() {
            return Err(CatalogPortError::InvalidResponse);
        }
        let end = (start + usize::from(query.page_size())).min(releases.len());
        let next_cursor = (end < releases.len()).then(|| end.to_string().into_boxed_str());
        Ok(CatalogSourcePage::new(
            releases[start..end].to_vec(),
            next_cursor,
        ))
    }

    fn release(&mut self, key: &DictionaryReleaseKey) -> Result<CatalogRelease, CatalogPortError> {
        if self.unavailable {
            return Err(CatalogPortError::Unavailable);
        }
        self.releases
            .iter()
            .find(|release| release.key() == key)
            .cloned()
            .ok_or(CatalogPortError::NotFound)
    }

    fn fetch(&mut self, download_url: &str, byte_limit: u64) -> Result<Vec<u8>, CatalogPortError> {
        if self.unavailable {
            return Err(CatalogPortError::Unavailable);
        }
        match self.artifacts.get(download_url) {
            Some(ArtifactResponse::Bytes(payload)) if payload.len() as u64 <= byte_limit => {
                Ok(payload.clone())
            }
            Some(ArtifactResponse::Bytes(_)) => Err(CatalogPortError::TooLarge),
            Some(ArtifactResponse::Unavailable) => Err(CatalogPortError::Unavailable),
            None => Err(CatalogPortError::NotFound),
        }
    }
}

#[derive(Clone, Debug)]
struct TrustedFixture {
    statement: ArtifactStatement,
    signature: SignatureEnvelope,
    publisher_identity: PublisherIdentity,
}

#[derive(Default)]
pub struct InMemoryTrustVerifier {
    fixtures: Vec<TrustedFixture>,
    unavailable: bool,
}

impl InMemoryTrustVerifier {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_trusted_artifact(
        mut self,
        statement: ArtifactStatement,
        signature: SignatureEnvelope,
        publisher_identity: PublisherIdentity,
    ) -> Self {
        self.fixtures.push(TrustedFixture {
            statement,
            signature,
            publisher_identity,
        });
        self
    }

    #[must_use]
    pub const fn unavailable(mut self) -> Self {
        self.unavailable = true;
        self
    }
}

impl ArtifactTrustVerifier for InMemoryTrustVerifier {
    fn verify(
        &mut self,
        statement: &ArtifactStatement,
        signature: &SignatureEnvelope,
    ) -> Result<PublisherIdentity, TrustVerifierError> {
        if self.unavailable {
            return Err(TrustVerifierError::Unavailable);
        }
        self.fixtures
            .iter()
            .find(|fixture| &fixture.signature == signature && &fixture.statement == statement)
            .map(|fixture| fixture.publisher_identity.clone())
            .ok_or(TrustVerifierError::InvalidSignature)
    }
}

#[derive(Default)]
struct InstallState {
    active: BTreeMap<Box<str>, Vec<u8>>,
    artifacts: BTreeMap<Sha256Digest, Vec<u8>>,
    installations: BTreeMap<Box<str>, DictionaryInstallationSource>,
}

#[derive(Clone, Default)]
pub struct InMemoryDictionaryInstallStore {
    state: Arc<Mutex<InstallState>>,
}

impl InMemoryDictionaryInstallStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put_active(&self, payload: Vec<u8>) -> Result<(), InstallStoreError> {
        let source =
            std::str::from_utf8(&payload).map_err(|_| InstallStoreError::StorageFailure)?;
        let package = DictionaryPackage::decode_json(source, None)
            .map_err(|_| InstallStoreError::StorageFailure)?;
        self.state
            .lock()
            .map_err(|_| InstallStoreError::StorageFailure)?
            .active
            .insert(package.id().into(), payload);
        Ok(())
    }

    pub fn remove_active(&self, dictionary_id: &str) -> Result<(), InstallStoreError> {
        self.state
            .lock()
            .map_err(|_| InstallStoreError::StorageFailure)?
            .active
            .remove(dictionary_id);
        Ok(())
    }

    pub fn active_payload(
        &self,
        dictionary_id: &str,
    ) -> Result<Option<Vec<u8>>, InstallStoreError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| InstallStoreError::StorageFailure)?
            .active
            .get(dictionary_id)
            .cloned())
    }

    fn views(state: &InstallState) -> Vec<DictionaryInstallationView> {
        let dictionary_ids = state
            .active
            .keys()
            .chain(state.installations.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        dictionary_ids
            .into_iter()
            .map(|dictionary_id| {
                let active = state.active.get(&dictionary_id);
                let source = state.installations.get(&dictionary_id);
                let installation_state = match (active, source) {
                    (Some(payload), Some(source)) => {
                        let digest: [u8; 32] = Sha256::digest(payload).into();
                        if Sha256Digest::new(digest) == source.digest() {
                            DictionaryInstallationState::Verified
                        } else {
                            DictionaryInstallationState::Modified
                        }
                    }
                    (None, Some(_)) => DictionaryInstallationState::Missing,
                    (Some(_), None) => DictionaryInstallationState::Unmanaged,
                    (None, None) => unreachable!("dictionary id came from the state maps"),
                };
                DictionaryInstallationView::new(dictionary_id, installation_state, source.cloned())
            })
            .collect()
    }
}

impl DictionaryInstallStore for InMemoryDictionaryInstallStore {
    fn install(
        &mut self,
        artifact: VerifiedDictionaryArtifact,
        replacement: DictionaryReplacementPolicy,
    ) -> Result<DictionaryInstallationView, InstallStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| InstallStoreError::StorageFailure)?;
        let dictionary_id: Box<str> = artifact.package.id().into();
        let source = DictionaryInstallationSource::from_verified(&artifact);
        let is_same_verified_release = state
            .active
            .get(&dictionary_id)
            .zip(state.installations.get(&dictionary_id))
            .is_some_and(|(active, installed)| {
                let digest: [u8; 32] = Sha256::digest(active).into();
                Sha256Digest::new(digest) == installed.digest()
                    && installed.release() == artifact.release()
                    && installed.digest() == artifact.statement().digest()
            });
        if is_same_verified_release {
            return Ok(DictionaryInstallationView::new(
                dictionary_id,
                DictionaryInstallationState::Verified,
                state.installations.get(artifact.package.id()).cloned(),
            ));
        }

        if state.active.contains_key(&dictionary_id) {
            let replacement_allowed = match replacement {
                DictionaryReplacementPolicy::RejectExisting => false,
                DictionaryReplacementPolicy::ReplaceVerified => state
                    .installations
                    .get(&dictionary_id)
                    .is_some_and(|installed| {
                        state.active.get(&dictionary_id).is_some_and(|active| {
                            let digest: [u8; 32] = Sha256::digest(active).into();
                            Sha256Digest::new(digest) == installed.digest()
                        })
                    }),
                DictionaryReplacementPolicy::ReplaceAny => true,
            };
            if !replacement_allowed {
                return Err(InstallStoreError::LocalChangesConflict);
            }
        }

        state
            .artifacts
            .entry(artifact.statement().digest())
            .or_insert_with(|| artifact.payload.clone());
        state.active.insert(dictionary_id.clone(), artifact.payload);
        state
            .installations
            .insert(dictionary_id.clone(), source.clone());
        Ok(DictionaryInstallationView::new(
            dictionary_id,
            DictionaryInstallationState::Verified,
            Some(source),
        ))
    }

    fn installations(&mut self) -> Result<Vec<DictionaryInstallationView>, InstallStoreError> {
        let state = self
            .state
            .lock()
            .map_err(|_| InstallStoreError::StorageFailure)?;
        Ok(Self::views(&state))
    }
}

pub struct FixedInstallationClock {
    now_unix_ms: u64,
}

impl FixedInstallationClock {
    #[must_use]
    pub const fn new(now_unix_ms: u64) -> Self {
        Self { now_unix_ms }
    }
}

impl InstallationClock for FixedInstallationClock {
    fn now_unix_ms(&mut self) -> u64 {
        self.now_unix_ms
    }
}
