use crate::archive::verified_tree;
use crate::manifest::valid_id;
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    package_id: String,
    version: [u16; 3],
    manifest_sha256: String,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    releases: BTreeMap<String, Receipt>,
    selected: BTreeMap<String, String>,
}

/// Immutable verified files, approved by exact archive digest for local development.
/// This receipt is not a publisher signature or a network trust policy.
#[derive(Clone)]
pub struct InstalledPackage {
    root: PathBuf,
    manifest_sha256: String,
    manifest: Manifest,
    digest: String,
    files: BTreeMap<String, PathBuf>,
}
impl InstalledPackage {
    pub fn verify(&self) -> Result<(), PackageError> {
        if self.root.canonicalize()? != self.root {
            return Err(PackageError::UnsafePath);
        }
        if sha256(&read_bounded(
            &self.root.join("manifest.json"),
            MAX_MANIFEST,
        )?) != self.manifest_sha256
        {
            return Err(PackageError::HashMismatch);
        }
        verified_tree(&self.root, &self.manifest).map(|_| ())
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn sha256(&self) -> &str {
        &self.digest
    }
    pub fn file(&self, name: &str) -> Result<&Path, PackageError> {
        self.files
            .get(name)
            .map(PathBuf::as_path)
            .ok_or(PackageError::UnsafePath)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Selection {
    pub package_id: String,
    pub version: [u16; 3],
    pub sha256: String,
    pub selected: bool,
}

pub struct PluginStore {
    root: PathBuf,
}

impl PluginStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn lock(&self) -> Result<File, PackageError> {
        fs::create_dir_all(&self.root)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join("store.lock"))?;
        file.try_lock().map_err(|_| PackageError::Busy)?;
        Ok(file)
    }

    fn state(&self) -> Result<State, PackageError> {
        let path = self.root.join("state.json");
        let bytes = match read_bounded(&path, MAX_MANIFEST) {
            Ok(bytes) => bytes,
            Err(PackageError::Io) if !path.try_exists()? => return Ok(State::default()),
            Err(error) => return Err(error),
        };
        let state: State = serde_json::from_slice(&bytes)?;
        if state.releases.len() > 512 || state.selected.len() > 128 {
            return Err(PackageError::SizeLimit);
        }
        for (digest, receipt) in &state.releases {
            if !valid_hash(digest)
                || !valid_hash(&receipt.manifest_sha256)
                || !valid_id(&receipt.package_id)
            {
                return Err(PackageError::InvalidManifest);
            }
        }
        for (id, hash) in &state.selected {
            if !state
                .releases
                .get(hash)
                .is_some_and(|r| &r.package_id == id)
            {
                return Err(PackageError::InvalidManifest);
            }
        }
        Ok(state)
    }

    fn save(&self, state: &State) -> Result<(), PackageError> {
        let bytes = serde_json::to_vec_pretty(state)?;
        if bytes.len() as u64 > MAX_MANIFEST
            || state.releases.len() > 512
            || state.selected.len() > 128
        {
            return Err(PackageError::SizeLimit);
        }
        let mut temp = tempfile::NamedTempFile::new_in(&self.root)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(self.root.join("state.json"))
            .map_err(|_| PackageError::Io)?;
        Ok(())
    }

    fn installed(&self, digest: &str, receipt: &Receipt) -> Result<InstalledPackage, PackageError> {
        if !valid_hash(digest) {
            return Err(PackageError::InvalidManifest);
        }
        let objects = self.root.join("objects").canonicalize()?;
        let root = objects.join(digest);
        if root.canonicalize()? != root {
            return Err(PackageError::UnsafePath);
        }
        let manifest_bytes = read_bounded(&root.join("manifest.json"), MAX_MANIFEST)?;
        if sha256(&manifest_bytes) != receipt.manifest_sha256 {
            return Err(PackageError::HashMismatch);
        }
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
        if manifest.package_id != receipt.package_id || manifest.version != receipt.version {
            return Err(PackageError::Conflict);
        }
        let files = verified_tree(&root, &manifest)?;
        Ok(InstalledPackage {
            root,
            manifest_sha256: receipt.manifest_sha256.clone(),
            manifest,
            digest: digest.into(),
            files,
        })
    }

    /// Stages approved bytes without changing the active selection or executing code.
    pub fn install(
        &self,
        package: &Package,
        approved_sha256: &str,
    ) -> Result<Selection, PackageError> {
        if !valid_hash(approved_sha256) || package.digest != approved_sha256 {
            return Err(PackageError::ApprovalRequired);
        }
        let _lock = self.lock()?;
        let mut state = self.state()?;
        for (digest, receipt) in &state.releases {
            if receipt.package_id == package.manifest.package_id
                && receipt.version == package.manifest.version
                && digest != &package.digest
            {
                return Err(PackageError::Conflict);
            }
        }
        let objects = self.root.join("objects");
        fs::create_dir_all(&objects)?;
        let destination = objects.join(&package.digest);
        let receipt = Receipt {
            package_id: package.manifest.package_id.clone(),
            version: package.manifest.version,
            manifest_sha256: sha256(&package.manifest_bytes),
        };
        if destination.try_exists()? {
            self.installed(&package.digest, &receipt)?;
        } else {
            let stage = tempfile::tempdir_in(&objects)?;
            for (path, bytes) in &package.files {
                let path = stage.path().join(path);
                fs::create_dir_all(path.parent().ok_or(PackageError::UnsafePath)?)?;
                let mut file = File::create(path)?;
                file.write_all(bytes)?;
                file.sync_all()?;
            }
            let mut manifest = File::create(stage.path().join("manifest.json"))?;
            manifest.write_all(&package.manifest_bytes)?;
            manifest.sync_all()?;
            drop(manifest);
            verified_tree(stage.path(), &package.manifest)?;
            fs::rename(stage.path(), destination)?;
        }
        state.releases.insert(package.digest.clone(), receipt);
        self.save(&state)?;
        Ok(Selection {
            package_id: package.manifest.package_id.clone(),
            version: package.manifest.version,
            sha256: package.digest.clone(),
            selected: state.selected.get(&package.manifest.package_id) == Some(&package.digest),
        })
    }

    fn resolve(&self, state: &State) -> Result<Vec<InstalledPackage>, PackageError> {
        let mut packages = Vec::new();
        let mut owners = BTreeMap::new();
        for (id, digest) in &state.selected {
            let receipt = state
                .releases
                .get(digest)
                .ok_or(PackageError::NotInstalled)?;
            let package = self.installed(digest, receipt)?;
            // Ownership is by Adapter ID, not architecture: two packages cannot own
            // different variants of one capability and accidentally mix release versions.
            let ids = package
                .manifest
                .variants
                .iter()
                .flat_map(|v| &v.adapters)
                .map(|a| &a.native_metadata.adapter_id)
                .collect::<BTreeSet<_>>();
            for adapter in ids {
                if owners.insert(adapter.clone(), id).is_some() {
                    return Err(PackageError::Conflict);
                }
            }
            packages.push(package);
        }
        Ok(packages)
    }

    pub fn selected(&self) -> Result<Vec<InstalledPackage>, PackageError> {
        self.resolve(&self.state()?)
    }

    pub fn list(&self) -> Result<Vec<Selection>, PackageError> {
        let state = self.state()?;
        Ok(state
            .releases
            .iter()
            .map(|(digest, r)| Selection {
                package_id: r.package_id.clone(),
                version: r.version,
                sha256: digest.clone(),
                selected: state.selected.get(&r.package_id) == Some(digest),
            })
            .collect())
    }

    /// Reads and verifies an installed release, including unselected versions.
    /// This performs static file checks only; it never loads native code.
    pub fn get(&self, digest: &str) -> Result<InstalledPackage, PackageError> {
        let state = self.state()?;
        let receipt = state
            .releases
            .get(digest)
            .ok_or(PackageError::NotInstalled)?;
        self.installed(digest, receipt)
    }

    /// Preflight runs before the atomic selection change. Existing RuntimeBundle
    /// instances retain their old immutable paths; this never hot-reloads a target.
    pub fn select<F>(&self, digest: &str, preflight: F) -> Result<(), PackageError>
    where
        F: FnOnce(&[InstalledPackage]) -> Result<(), PackageError>,
    {
        let _lock = self.lock()?;
        let mut state = self.state()?;
        let receipt = state
            .releases
            .get(digest)
            .ok_or(PackageError::NotInstalled)?;
        state
            .selected
            .insert(receipt.package_id.clone(), digest.into());
        let packages = self.resolve(&state)?;
        preflight(&packages)?;
        self.save(&state)
    }

    /// Removes the selection only. Immutable files are retained for loaded targets
    /// and rollback; physical garbage collection needs target lifetime accounting.
    pub fn deselect_release<F>(&self, digest: &str, preflight: F) -> Result<(), PackageError>
    where
        F: FnOnce(&[InstalledPackage]) -> Result<(), PackageError>,
    {
        let _lock = self.lock()?;
        let mut state = self.state()?;
        let receipt = state
            .releases
            .get(digest)
            .ok_or(PackageError::NotInstalled)?;
        let id = receipt.package_id.clone();
        if state.selected.get(&id).map(String::as_str) != Some(digest) {
            return Err(PackageError::Conflict);
        }
        state.selected.remove(&id);
        let packages = self.resolve(&state)?;
        preflight(&packages)?;
        self.save(&state)
    }

    pub fn deselect<F>(&self, package_id: &str, preflight: F) -> Result<(), PackageError>
    where
        F: FnOnce(&[InstalledPackage]) -> Result<(), PackageError>,
    {
        let _lock = self.lock()?;
        let mut state = self.state()?;
        state.selected.remove(package_id);
        let packages = self.resolve(&state)?;
        preflight(&packages)?;
        self.save(&state)
    }
}
