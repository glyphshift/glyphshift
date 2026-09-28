//! GSP native plugin packages. Parsing and installation never load executable code.
mod archive;
mod manifest;
mod store;

pub use archive::{Package, build_package};
pub use manifest::{Adapter, FileRole, Manifest, PACKAGE_SCHEMA, PackageFile, Variant};
pub use store::{InstalledPackage, PluginStore, Selection};

use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageError {
    Io,
    InvalidArchive,
    InvalidManifest,
    UnsafePath,
    SizeLimit,
    HashMismatch,
    ApprovalRequired,
    Incompatible,
    Conflict,
    Busy,
    NotInstalled,
}

impl From<std::io::Error> for PackageError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}
impl From<serde_json::Error> for PackageError {
    fn from(_: serde_json::Error) -> Self {
        Self::InvalidManifest
    }
}
impl From<zip::result::ZipError> for PackageError {
    fn from(_: zip::result::ZipError) -> Self {
        Self::InvalidArchive
    }
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(crate) const MAX_MANIFEST: u64 = 1024 * 1024;
pub(crate) const MAX_FILE: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_TOTAL: u64 = 128 * 1024 * 1024;
pub(crate) const MAX_ARCHIVE: u64 = 64 * 1024 * 1024;

pub(crate) fn read_bounded(path: &std::path::Path, limit: u64) -> Result<Vec<u8>, PackageError> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(PackageError::SizeLimit);
    }
    Ok(bytes)
}
