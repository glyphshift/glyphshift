use crate::manifest::valid_path;
use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub struct Package {
    pub(crate) manifest: Manifest,
    pub(crate) manifest_bytes: Vec<u8>,
    pub(crate) files: BTreeMap<String, Vec<u8>>,
    pub(crate) digest: String,
}

impl Package {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PackageError> {
        Self::from_bytes(&read_bounded(path.as_ref(), MAX_ARCHIVE)?)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PackageError> {
        if bytes.len() as u64 > MAX_ARCHIVE {
            return Err(PackageError::SizeLimit);
        }
        let mut zip = ZipArchive::new(Cursor::new(bytes))?;
        if zip.len() > 257 {
            return Err(PackageError::SizeLimit);
        }
        let mut files = BTreeMap::new();
        let mut total = 0u64;
        for index in 0..zip.len() {
            let entry = zip.by_index(index)?;
            let name = entry.name().to_owned();
            if !valid_path(&name)
                || entry.name_raw() != name.as_bytes()
                || entry.is_dir()
                || entry
                    .unix_mode()
                    .is_some_and(|mode| !matches!(mode & 0o170000, 0 | 0o100000))
            {
                return Err(PackageError::UnsafePath);
            }
            if files.contains_key(&name) {
                return Err(PackageError::InvalidArchive);
            }
            let limit = if name == "manifest.json" {
                MAX_MANIFEST
            } else {
                MAX_FILE
            };
            let size = entry.size();
            if size > limit {
                return Err(PackageError::SizeLimit);
            }
            total = total.checked_add(size).ok_or(PackageError::SizeLimit)?;
            if total > MAX_TOTAL + MAX_MANIFEST {
                return Err(PackageError::SizeLimit);
            }
            let mut data = Vec::new();
            entry.take(limit + 1).read_to_end(&mut data)?;
            if data.len() as u64 != size {
                return Err(PackageError::InvalidArchive);
            }
            files.insert(name, data);
        }
        let manifest_bytes = files
            .remove("manifest.json")
            .ok_or(PackageError::InvalidManifest)?;
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
        manifest.validate()?;
        if manifest.files.len() != files.len() {
            return Err(PackageError::InvalidArchive);
        }
        for file in &manifest.files {
            let data = files.get(&file.path).ok_or(PackageError::InvalidArchive)?;
            if file.size != data.len() as u64 || file.sha256 != sha256(data) {
                return Err(PackageError::HashMismatch);
            }
        }
        // Inspect only bounded PE bytes; no LoadLibrary/DllMain here.
        for variant in &manifest.variants {
            for adapter in &variant.adapters {
                if glyphshift_adapter_native_host::inspect_pe_architecture_bytes(
                    &files[&adapter.file],
                )
                .map_err(|_| PackageError::Incompatible)?
                    != variant.architecture
                {
                    return Err(PackageError::Incompatible);
                }
            }
        }
        Ok(Self {
            manifest,
            manifest_bytes,
            files,
            digest: sha256(bytes),
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn sha256(&self) -> &str {
        &self.digest
    }
}

/// Canonical lower-case paths, exact inventory, no links or unexpected files.
pub(crate) fn verified_tree(
    root: &Path,
    manifest: &Manifest,
) -> Result<BTreeMap<String, PathBuf>, PackageError> {
    manifest.validate()?;
    let root = root.canonicalize()?;
    let mut result = BTreeMap::new();
    for file in &manifest.files {
        let mut path = root.clone();
        for component in file.path.split('/') {
            path.push(component);
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(PackageError::UnsafePath);
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(PackageError::UnsafePath);
                }
            }
        }
        if !path.canonicalize()?.starts_with(&root) {
            return Err(PackageError::UnsafePath);
        }
        let bytes = read_bounded(&path, MAX_FILE)?;
        if bytes.len() as u64 != file.size || sha256(&bytes) != file.sha256 {
            return Err(PackageError::HashMismatch);
        }
        result.insert(file.path.clone(), path);
    }
    // Installed trees must not contain undeclared DLLs in the dependency search directory.
    fn inventory(
        root: &Path,
        current: &Path,
        found: &mut BTreeSet<String>,
        entries: &mut usize,
    ) -> Result<(), PackageError> {
        for entry in std::fs::read_dir(current)? {
            *entries += 1;
            if *entries > 1024 {
                return Err(PackageError::SizeLimit);
            }
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|_| PackageError::UnsafePath)?
                    .to_string_lossy()
                    .replace('\\', "/");
                if !valid_path(&relative) {
                    return Err(PackageError::UnsafePath);
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if entry.metadata()?.file_attributes() & 0x400 != 0 {
                        return Err(PackageError::UnsafePath);
                    }
                }
                inventory(root, &entry.path(), found, entries)?;
            } else if kind.is_file() {
                found.insert(
                    entry
                        .path()
                        .strip_prefix(root)
                        .map_err(|_| PackageError::UnsafePath)?
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
                if found.len() > 257 {
                    return Err(PackageError::SizeLimit);
                }
            } else {
                return Err(PackageError::UnsafePath);
            }
        }
        Ok(())
    }
    let mut found = BTreeSet::new();
    inventory(&root, &root, &mut found, &mut 0)?;
    found.remove("manifest.json");
    if found != manifest.files.iter().map(|f| f.path.clone()).collect() {
        return Err(PackageError::InvalidArchive);
    }
    Ok(result)
}

pub fn build_package(manifest: &Manifest, source: &Path) -> Result<Vec<u8>, PackageError> {
    let files = verified_tree(source, manifest)?;
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    writer.start_file("manifest.json", options)?;
    writer.write_all(&serde_json::to_vec_pretty(manifest)?)?;
    for (name, path) in files {
        writer.start_file(name, options)?;
        writer.write_all(&read_bounded(&path, MAX_FILE)?)?;
    }
    let bytes = writer.finish()?.into_inner();
    Package::from_bytes(&bytes)?;
    Ok(bytes)
}
