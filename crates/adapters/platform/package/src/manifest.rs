use crate::{MAX_FILE, MAX_TOTAL, PackageError, valid_hash};
use glyphshift_adapter_native_host::NativeAdapterMetadata;
use glyphshift_domain::Placement;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const PACKAGE_SCHEMA: &str = "glyphshift.plugin/1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub package_id: String,
    pub version: [u16; 3],
    /// Exact supported bundle contract, independent of the package release version.
    pub runtime_bundle_schema: String,
    pub license_file: String,
    pub variants: Vec<Variant>,
    pub files: Vec<PackageFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub platform: String,
    pub architecture: String,
    pub adapters: Vec<Adapter>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adapter {
    pub file: String,
    pub native_metadata: NativeAdapterMetadata,
    pub name: String,
    pub summary: String,
    pub technology: String,
    pub process_resident_after_deactivate: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub role: FileRole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRole {
    NativeAdapter,
    Support,
    License,
}

pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-".contains(&b))
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id.as_bytes()[id.len() - 1].is_ascii_alphanumeric()
}

/// Portable archive paths exclude Windows aliases, devices, ADS and case collisions.
pub(crate) fn valid_path(path: &str) -> bool {
    if path.is_empty() || path.len() > 200 || path.split('/').count() > 4 {
        return false;
    }
    path.split('/').all(|part| {
        let stem = part.split('.').next().unwrap_or("");
        !part.is_empty()
            && !part.starts_with('.')
            && !part.ends_with('.')
            && part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_.".contains(&b))
            && !matches!(stem, "con" | "prn" | "aux" | "nul")
            && !(stem.len() == 4
                && (stem.starts_with("com") || stem.starts_with("lpt"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    })
}

impl Manifest {
    pub fn validate(&self) -> Result<(), PackageError> {
        let invalid = PackageError::InvalidManifest;
        if self.schema != PACKAGE_SCHEMA
            || !valid_id(&self.package_id)
            || !self.package_id.starts_with("glyphshift-adapter-")
            || self.variants.is_empty()
            || self.variants.len() > 2
            || self.files.is_empty()
            || self.files.len() > 256
        {
            return Err(invalid);
        }
        if self.runtime_bundle_schema != "glyphshift.runtime-bundle/4" {
            return Err(PackageError::Incompatible);
        }
        let mut paths = BTreeSet::new();
        let mut total = 0u64;
        for file in &self.files {
            if !valid_path(&file.path) || file.path == "manifest.json" {
                return Err(PackageError::UnsafePath);
            }
            if !paths.insert(&file.path) || !valid_hash(&file.sha256) {
                return Err(invalid);
            }
            if file.size > MAX_FILE {
                return Err(PackageError::SizeLimit);
            }
            total = total
                .checked_add(file.size)
                .ok_or(PackageError::SizeLimit)?;
            if total > MAX_TOTAL {
                return Err(PackageError::SizeLimit);
            }
        }
        // No entry may be both a file and a parent directory.
        if paths.iter().any(|path| {
            path.match_indices('/')
                .any(|(i, _)| paths.contains(&path[..i].to_owned()))
        }) {
            return Err(PackageError::UnsafePath);
        }
        if !self
            .files
            .iter()
            .any(|f| f.path == self.license_file && f.role == FileRole::License && f.size > 0)
        {
            return Err(invalid);
        }
        let mut architectures = BTreeSet::new();
        let mut libraries = BTreeSet::new();
        let mut identities = std::collections::BTreeMap::new();
        for variant in &self.variants {
            if variant.platform != "windows"
                || !matches!(variant.architecture.as_str(), "x86" | "x86_64")
            {
                return Err(PackageError::Incompatible);
            }
            if !architectures.insert(&variant.architecture)
                || variant.adapters.is_empty()
                || variant.adapters.len() > 64
            {
                return Err(invalid);
            }
            let mut ids = BTreeSet::new();
            for adapter in &variant.adapters {
                let metadata = &adapter.native_metadata;
                if !valid_id(&metadata.adapter_id)
                    || !ids.insert(&metadata.adapter_id)
                    || adapter.name.trim().is_empty()
                    || adapter.name.len() > 256
                    || adapter.summary.len() > 4096
                    || adapter.technology.len() > 256
                {
                    return Err(invalid);
                }
                if metadata.abi != [1, 0] {
                    return Err(PackageError::Incompatible);
                }
                let descriptor = metadata.descriptor().map_err(|_| invalid)?;
                metadata.source_policy().map_err(|_| invalid)?;
                if descriptor.placement() != Placement::TargetProcess
                    || !descriptor.platforms().any(|p| p == variant.platform)
                    || !descriptor
                        .architectures()
                        .any(|a| a == variant.architecture)
                {
                    return Err(PackageError::Incompatible);
                }
                if identities
                    .insert(&metadata.adapter_id, metadata)
                    .is_some_and(|previous| previous != metadata)
                {
                    return Err(PackageError::Conflict);
                }
                if !adapter.file.ends_with(".dll")
                    || !libraries.insert(&adapter.file)
                    || !self.files.iter().any(|f| {
                        f.path == adapter.file && f.role == FileRole::NativeAdapter && f.size > 0
                    })
                {
                    return Err(invalid);
                }
            }
        }
        if self
            .files
            .iter()
            .any(|f| f.role == FileRole::NativeAdapter && !libraries.contains(&f.path))
        {
            return Err(invalid);
        }
        Ok(())
    }
}
