use crate::{
    model::DictionaryInstallationSourceParts, DictionaryInstallStore, DictionaryInstallationSource,
    DictionaryInstallationState, DictionaryInstallationView, DictionaryReleaseKey,
    DictionaryReplacementPolicy, InstallStoreError, PublisherIdentity, Sha256Digest,
    SignatureEnvelope, VerifiedDictionaryArtifact, DICTIONARY_MEDIA_TYPE,
};
use glyphshift_dictionary_package::DictionaryPackage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const INSTALLATION_SCHEMA: &str = "glyphshift.dictionary-installation/1";
const TRANSACTION_SCHEMA: &str = "glyphshift.dictionary-install-transaction/1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredSignature {
    scheme: Box<str>,
    key_id: Box<str>,
    value: Box<str>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallationRecord {
    schema: Box<str>,
    catalog_id: Box<str>,
    dictionary_id: Box<str>,
    release_version: Box<str>,
    media_type: Box<str>,
    size: u64,
    digest: Box<str>,
    publisher_identity: Box<str>,
    signature: StoredSignature,
    installed_at_unix_ms: u64,
    payload_revision: u64,
}

impl InstallationRecord {
    fn from_source(source: &DictionaryInstallationSource) -> Self {
        Self {
            schema: INSTALLATION_SCHEMA.into(),
            catalog_id: source.release().catalog_id().into(),
            dictionary_id: source.release().dictionary_id().into(),
            release_version: source.release().release_version().into(),
            media_type: source.media_type().into(),
            size: source.size(),
            digest: source.digest().to_hex(),
            publisher_identity: source.publisher_identity().as_str().into(),
            signature: StoredSignature {
                scheme: source.signature().scheme().into(),
                key_id: source.signature().key_id().into(),
                value: source.signature().value().into(),
            },
            installed_at_unix_ms: source.installed_at_unix_ms(),
            payload_revision: source.payload_revision(),
        }
    }

    fn source(
        &self,
        expected_dictionary_id: &str,
    ) -> Result<DictionaryInstallationSource, InstallStoreError> {
        if self.schema.as_ref() != INSTALLATION_SCHEMA
            || self.dictionary_id.as_ref() != expected_dictionary_id
            || self.media_type.as_ref() != DICTIONARY_MEDIA_TYPE
        {
            return Err(InstallStoreError::StorageFailure);
        }
        let release = DictionaryReleaseKey::new(
            self.catalog_id.clone(),
            self.dictionary_id.clone(),
            self.release_version.clone(),
        )
        .map_err(|_| InstallStoreError::StorageFailure)?;
        let publisher_identity = PublisherIdentity::new(self.publisher_identity.clone())
            .map_err(|_| InstallStoreError::StorageFailure)?;
        let signature = SignatureEnvelope::new(
            self.signature.scheme.clone(),
            self.signature.key_id.clone(),
            self.signature.value.clone(),
        )
        .map_err(|_| InstallStoreError::StorageFailure)?;
        DictionaryInstallationSource::restore(DictionaryInstallationSourceParts {
            release,
            media_type: self.media_type.clone(),
            size: self.size,
            digest: decode_digest(&self.digest)?,
            publisher_identity,
            signature,
            installed_at_unix_ms: self.installed_at_unix_ms,
            payload_revision: self.payload_revision,
        })
        .map_err(|_| InstallStoreError::StorageFailure)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingInstallation {
    schema: Box<str>,
    installation: InstallationRecord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InstallCheckpoint {
    Artifact,
    Transaction,
    Active,
    Record,
}

type ActivePayloads = BTreeMap<Box<str>, Vec<u8>>;
type InstallationSources = BTreeMap<Box<str>, DictionaryInstallationSource>;

/// Crash-recoverable storage for verified Dictionary packages.
pub struct FileDictionaryInstallStore {
    root: PathBuf,
    #[cfg(test)]
    fail_after: Option<InstallCheckpoint>,
}

impl FileDictionaryInstallStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, InstallStoreError> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(InstallStoreError::StorageFailure);
        }
        let mut store = Self {
            root,
            #[cfg(test)]
            fail_after: None,
        };
        store.ensure_layout()?;
        store.recover_pending()?;
        Ok(store)
    }

    #[cfg(test)]
    fn active_payload(&self, dictionary_id: &str) -> Result<Option<Vec<u8>>, InstallStoreError> {
        read_optional(&self.active_path(dictionary_id))
    }

    fn ensure_layout(&self) -> Result<(), InstallStoreError> {
        for directory in [
            self.active_directory(),
            self.artifact_directory(),
            self.installation_directory(),
            self.transaction_directory(),
        ] {
            fs::create_dir_all(directory).map_err(storage_failure)?;
        }
        Ok(())
    }

    fn active_directory(&self) -> PathBuf {
        self.root.join("dictionaries")
    }

    fn artifact_directory(&self) -> PathBuf {
        self.root.join("dictionary-artifacts").join("sha256")
    }

    fn installation_directory(&self) -> PathBuf {
        self.root.join("dictionary-installations")
    }

    fn transaction_directory(&self) -> PathBuf {
        self.installation_directory().join(".transactions")
    }

    fn active_path(&self, dictionary_id: &str) -> PathBuf {
        self.active_directory()
            .join(format!("{dictionary_id}.json"))
    }

    fn artifact_path(&self, digest: Sha256Digest) -> PathBuf {
        self.artifact_directory()
            .join(format!("{}.json", digest.to_hex()))
    }

    fn installation_path(&self, dictionary_id: &str) -> PathBuf {
        self.installation_directory()
            .join(format!("{dictionary_id}.json"))
    }

    fn transaction_path(&self, dictionary_id: &str) -> PathBuf {
        self.transaction_directory()
            .join(format!("{dictionary_id}.json"))
    }

    fn publish_artifact(
        &self,
        digest: Sha256Digest,
        payload: &[u8],
    ) -> Result<(), InstallStoreError> {
        let path = self.artifact_path(digest);
        if let Some(existing) = read_optional(&path)? {
            return digest_matches(&existing, digest)
                .then_some(())
                .ok_or(InstallStoreError::StorageFailure);
        }
        persist_new(&path, payload)?;
        let published = fs::read(path).map_err(storage_failure)?;
        digest_matches(&published, digest)
            .then_some(())
            .ok_or(InstallStoreError::StorageFailure)
    }

    fn read_installation(
        &self,
        dictionary_id: &str,
    ) -> Result<Option<DictionaryInstallationSource>, InstallStoreError> {
        let Some(bytes) = read_optional(&self.installation_path(dictionary_id))? else {
            return Ok(None);
        };
        let record: InstallationRecord =
            serde_json::from_slice(&bytes).map_err(|_| InstallStoreError::StorageFailure)?;
        record.source(dictionary_id).map(Some)
    }

    fn recover_pending(&mut self) -> Result<(), InstallStoreError> {
        for path in json_files(&self.transaction_directory())? {
            let dictionary_id = file_stem(&path)?;
            let bytes = fs::read(&path).map_err(storage_failure)?;
            let Ok(pending) = serde_json::from_slice::<PendingInstallation>(&bytes) else {
                quarantine_invalid_transaction(&path);
                continue;
            };
            if pending.schema.as_ref() != TRANSACTION_SCHEMA {
                quarantine_invalid_transaction(&path);
                continue;
            }
            let Ok(source) = pending.installation.source(&dictionary_id) else {
                quarantine_invalid_transaction(&path);
                continue;
            };
            let active = read_optional(&self.active_path(&dictionary_id))?;
            let artifact = read_optional(&self.artifact_path(source.digest()))?;
            let can_complete =
                active
                    .as_deref()
                    .zip(artifact.as_deref())
                    .is_some_and(|(active, artifact)| {
                        active == artifact
                            && active.len() as u64 == source.size()
                            && digest_matches(active, source.digest())
                            && package_matches(active, &dictionary_id, source.payload_revision())
                    });
            if can_complete {
                atomic_json_write(
                    &self.installation_path(&dictionary_id),
                    &pending.installation,
                )?;
            }
            fs::remove_file(path).map_err(storage_failure)?;
        }
        Ok(())
    }

    fn installation_maps(
        &self,
    ) -> Result<(ActivePayloads, InstallationSources), InstallStoreError> {
        let mut active = BTreeMap::new();
        for path in json_files(&self.active_directory())? {
            let dictionary_id = file_stem(&path)?;
            let payload = fs::read(path).map_err(storage_failure)?;
            if decode_package(&payload, &dictionary_id).is_err() {
                continue;
            }
            active.insert(dictionary_id.into(), payload);
        }

        let mut installations = BTreeMap::new();
        for path in json_files(&self.installation_directory())? {
            let dictionary_id = file_stem(&path)?;
            let bytes = fs::read(path).map_err(storage_failure)?;
            let Ok(record) = serde_json::from_slice::<InstallationRecord>(&bytes) else {
                continue;
            };
            let Ok(source) = record.source(&dictionary_id) else {
                continue;
            };
            installations.insert(dictionary_id.clone().into(), source);
        }
        Ok((active, installations))
    }

    fn checkpoint(&mut self, checkpoint: InstallCheckpoint) -> Result<(), InstallStoreError> {
        #[cfg(test)]
        if self.fail_after == Some(checkpoint) {
            self.fail_after = None;
            return Err(InstallStoreError::StorageFailure);
        }
        let _ = checkpoint;
        Ok(())
    }

    #[cfg(test)]
    fn fail_after(&mut self, checkpoint: InstallCheckpoint) {
        self.fail_after = Some(checkpoint);
    }
}

fn quarantine_invalid_transaction(path: &Path) {
    let destination = path.with_extension("invalid");
    if !destination.exists() {
        let _ = fs::rename(path, destination);
    }
}

impl DictionaryInstallStore for FileDictionaryInstallStore {
    fn install(
        &mut self,
        artifact: VerifiedDictionaryArtifact,
        replacement: DictionaryReplacementPolicy,
    ) -> Result<DictionaryInstallationView, InstallStoreError> {
        self.recover_pending()?;
        let dictionary_id = artifact.package().id();
        let active = read_optional(&self.active_path(dictionary_id))?;
        let installed = self.read_installation(dictionary_id)?;
        let is_same_verified_release =
            active
                .as_deref()
                .zip(installed.as_ref())
                .is_some_and(|(active, installed)| {
                    digest_matches(active, installed.digest())
                        && installed.release() == artifact.release()
                        && installed.digest() == artifact.statement().digest()
                });
        if is_same_verified_release {
            self.publish_artifact(artifact.statement().digest(), artifact.payload())?;
            return Ok(DictionaryInstallationView::new(
                dictionary_id.into(),
                DictionaryInstallationState::Verified,
                installed,
            ));
        }

        if let Some(active) = active.as_deref() {
            let replacement_allowed = match replacement {
                DictionaryReplacementPolicy::RejectExisting => false,
                DictionaryReplacementPolicy::ReplaceVerified => installed
                    .as_ref()
                    .is_some_and(|source| digest_matches(active, source.digest())),
                DictionaryReplacementPolicy::ReplaceAny => true,
            };
            if !replacement_allowed {
                return Err(InstallStoreError::LocalChangesConflict);
            }
        }

        let source = DictionaryInstallationSource::from_verified(&artifact);
        let record = InstallationRecord::from_source(&source);
        self.publish_artifact(source.digest(), artifact.payload())?;
        self.checkpoint(InstallCheckpoint::Artifact)?;

        atomic_json_write(
            &self.transaction_path(dictionary_id),
            &PendingInstallation {
                schema: TRANSACTION_SCHEMA.into(),
                installation: record.clone(),
            },
        )?;
        self.checkpoint(InstallCheckpoint::Transaction)?;

        atomic_write(&self.active_path(dictionary_id), artifact.payload())?;
        self.checkpoint(InstallCheckpoint::Active)?;

        atomic_json_write(&self.installation_path(dictionary_id), &record)?;
        self.checkpoint(InstallCheckpoint::Record)?;

        fs::remove_file(self.transaction_path(dictionary_id)).map_err(storage_failure)?;
        Ok(DictionaryInstallationView::new(
            dictionary_id.into(),
            DictionaryInstallationState::Verified,
            Some(source),
        ))
    }

    fn installations(&mut self) -> Result<Vec<DictionaryInstallationView>, InstallStoreError> {
        self.recover_pending()?;
        let (active, installations) = self.installation_maps()?;
        let dictionary_ids = active
            .keys()
            .chain(installations.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        Ok(dictionary_ids
            .into_iter()
            .map(|dictionary_id| {
                let payload = active.get(&dictionary_id);
                let source = installations.get(&dictionary_id);
                let state = match (payload, source) {
                    (Some(payload), Some(source)) if digest_matches(payload, source.digest()) => {
                        DictionaryInstallationState::Verified
                    }
                    (Some(_), Some(_)) => DictionaryInstallationState::Modified,
                    (None, Some(_)) => DictionaryInstallationState::Missing,
                    (Some(_), None) => DictionaryInstallationState::Unmanaged,
                    (None, None) => unreachable!("dictionary id came from active or records"),
                };
                DictionaryInstallationView::new(dictionary_id, state, source.cloned())
            })
            .collect())
    }
}

fn atomic_json_write(path: &Path, value: &impl Serialize) -> Result<(), InstallStoreError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| InstallStoreError::StorageFailure)?;
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), InstallStoreError> {
    let parent = path.parent().ok_or(InstallStoreError::StorageFailure)?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(storage_failure)?;
    temporary.write_all(bytes).map_err(storage_failure)?;
    temporary.flush().map_err(storage_failure)?;
    temporary.as_file().sync_all().map_err(storage_failure)?;
    temporary
        .persist(path)
        .map_err(|_| InstallStoreError::StorageFailure)?;
    Ok(())
}

fn persist_new(path: &Path, bytes: &[u8]) -> Result<(), InstallStoreError> {
    let parent = path.parent().ok_or(InstallStoreError::StorageFailure)?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(storage_failure)?;
    temporary.write_all(bytes).map_err(storage_failure)?;
    temporary.flush().map_err(storage_failure)?;
    temporary.as_file().sync_all().map_err(storage_failure)?;
    match temporary.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err(InstallStoreError::StorageFailure),
    }
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, InstallStoreError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(storage_failure(error)),
    }
}

fn json_files(directory: &Path) -> Result<Vec<PathBuf>, InstallStoreError> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory).map_err(storage_failure)? {
        let entry = entry.map_err(storage_failure)?;
        if entry.file_type().map_err(storage_failure)?.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|value| value == "json")
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn file_stem(path: &Path) -> Result<String, InstallStoreError> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .ok_or(InstallStoreError::StorageFailure)
}

fn decode_digest(value: &str) -> Result<Sha256Digest, InstallStoreError> {
    if value.len() != 64 {
        return Err(InstallStoreError::StorageFailure);
    }
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| InstallStoreError::StorageFailure)?;
    }
    Ok(Sha256Digest::new(digest))
}

fn digest_matches(payload: &[u8], expected: Sha256Digest) -> bool {
    let digest: [u8; 32] = Sha256::digest(payload).into();
    Sha256Digest::new(digest) == expected
}

fn decode_package(
    payload: &[u8],
    expected_dictionary_id: &str,
) -> Result<DictionaryPackage, InstallStoreError> {
    let source = std::str::from_utf8(payload).map_err(|_| InstallStoreError::StorageFailure)?;
    DictionaryPackage::decode_json(source, Some(expected_dictionary_id))
        .map_err(|_| InstallStoreError::StorageFailure)
}

fn package_matches(payload: &[u8], dictionary_id: &str, revision: u64) -> bool {
    decode_package(payload, dictionary_id).is_ok_and(|package| package.revision() == revision)
}

fn storage_failure(_: std::io::Error) -> InstallStoreError {
    InstallStoreError::StorageFailure
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactPresentation, ArtifactStatement, CatalogRelease, DictionaryArtifactDescriptor,
    };
    use glyphshift_dictionary_package::{DictionaryCreate, DictionaryEntryCreate};
    use tempfile::tempdir;

    #[test]
    fn reopen_resolves_each_installation_interruption_to_a_complete_release() {
        let checkpoints = [
            (InstallCheckpoint::Artifact, "1.0.0"),
            (InstallCheckpoint::Transaction, "1.0.0"),
            (InstallCheckpoint::Active, "2.0.0"),
            (InstallCheckpoint::Record, "2.0.0"),
        ];

        for (checkpoint, expected_release) in checkpoints {
            let directory = tempdir().expect("temporary installation root");
            let mut store =
                FileDictionaryInstallStore::open(directory.path()).expect("open file store");
            store
                .install(
                    verified_artifact("dictionary.ui", "1.0.0"),
                    DictionaryReplacementPolicy::RejectExisting,
                )
                .expect("install initial release");
            store.fail_after(checkpoint);
            assert_eq!(
                store
                    .install(
                        verified_artifact("dictionary.ui", "2.0.0"),
                        DictionaryReplacementPolicy::ReplaceVerified,
                    )
                    .expect_err("simulate interruption"),
                InstallStoreError::StorageFailure
            );
            drop(store);

            let mut reopened =
                FileDictionaryInstallStore::open(directory.path()).expect("recover file store");
            let views = reopened.installations().expect("installation views");
            assert_eq!(views.len(), 1);
            assert_eq!(views[0].state(), DictionaryInstallationState::Verified);
            assert_eq!(
                views[0]
                    .source()
                    .expect("verified source")
                    .release()
                    .release_version(),
                expected_release
            );
            assert!(json_files(&reopened.transaction_directory())
                .expect("transaction directory")
                .is_empty());
            let active = reopened
                .active_payload("dictionary.ui")
                .expect("read active")
                .expect("active dictionary");
            assert_eq!(
                decode_package(&active, "dictionary.ui")
                    .expect("decode active")
                    .view()
                    .metadata()
                    .release_version(),
                expected_release
            );
        }
    }

    #[test]
    fn file_views_derive_modified_missing_and_unmanaged_states() {
        let directory = tempdir().expect("temporary installation root");
        let mut store =
            FileDictionaryInstallStore::open(directory.path()).expect("open file store");
        store
            .install(
                verified_artifact("dictionary.ui", "1.0.0"),
                DictionaryReplacementPolicy::RejectExisting,
            )
            .expect("install release");
        atomic_write(
            &store.active_path("dictionary.ui"),
            &dictionary_payload("dictionary.ui", "1.1.0"),
        )
        .expect("modify active");
        assert_eq!(
            store.installations().expect("modified view")[0].state(),
            DictionaryInstallationState::Modified
        );

        fs::remove_file(store.active_path("dictionary.ui")).expect("remove active");
        assert_eq!(
            store.installations().expect("missing view")[0].state(),
            DictionaryInstallationState::Missing
        );

        atomic_write(
            &store.active_path("dictionary.local"),
            &dictionary_payload("dictionary.local", "0.1.0"),
        )
        .expect("write unmanaged dictionary");
        let views = store.installations().expect("installation views");
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
    fn invalid_installation_record_does_not_hide_other_installations() {
        let directory = tempdir().expect("temporary installation root");
        let mut store =
            FileDictionaryInstallStore::open(directory.path()).expect("open file store");
        store
            .install(
                verified_artifact("dictionary.valid", "1.0.0"),
                DictionaryReplacementPolicy::RejectExisting,
            )
            .expect("install valid dictionary");
        fs::write(
            store.installation_path("dictionary.invalid"),
            br#"{"schema":"glyphshift.dictionary-installation/1","dictionaryId":42}"#,
        )
        .expect("write invalid installation record");

        let views = store
            .installations()
            .expect("list remaining valid installations");

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].dictionary_id(), "dictionary.valid");
    }

    fn verified_artifact(dictionary_id: &str, release_version: &str) -> VerifiedDictionaryArtifact {
        let payload = dictionary_payload(dictionary_id, release_version);
        let package = decode_package(&payload, dictionary_id).expect("dictionary package");
        let digest: [u8; 32] = Sha256::digest(&payload).into();
        let publisher_identity = PublisherIdentity::new("publisher.example").expect("publisher");
        let signature =
            SignatureEnvelope::new("fixture", "test-key", "signed-statement").expect("signature");
        let descriptor = DictionaryArtifactDescriptor::new(
            payload.len() as u64,
            Sha256Digest::new(digest),
            ["https://catalog.example/dictionary.json"],
            publisher_identity.clone(),
            signature.clone(),
        )
        .expect("artifact descriptor");
        let release = CatalogRelease::new(
            DictionaryReleaseKey::new("glyphshift.official", dictionary_id, release_version)
                .expect("release key"),
            "en-US",
            "zh-CN",
            "en-US",
            vec![
                ArtifactPresentation::new("en-US", "Dictionary", "Test dictionary")
                    .expect("presentation"),
            ],
            descriptor,
        )
        .expect("catalog release");
        VerifiedDictionaryArtifact {
            release: release.key().clone(),
            payload,
            package,
            statement: ArtifactStatement::for_release(&release),
            signature,
            publisher_identity,
            installed_at_unix_ms: 1_700_000_000_000,
        }
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
}
