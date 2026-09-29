//! Exact-version Registry consumption with independently pinned trust.
//! Installation does not select or execute plugins, and never replaces user-edited dictionaries.
mod dictionary;
mod discovery;
pub use discovery::{DictionaryEntry, DictionaryPage, DictionarySearch};
#[cfg(test)]
mod fixtures;
mod http;
mod proof;
mod strict_json;
#[cfg(test)]
mod tests;

pub use http::RegistryClient;
pub use proof::{Artifact, ReleaseRequest, Statement, TrustStore, VerifiedProof};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Configuration,
    InvalidProof,
    UntrustedKey,
    IdentityMismatch,
    InvalidArtifact,
    Unavailable,
    NotFound,
    TooLarge,
    LocalChangesConflict,
    Storage,
}

pub const MAX_PROOF: u64 = 2 * 1024 * 1024 + 1024;

pub fn read_bounded(path: &std::path::Path, limit: u64) -> Result<Vec<u8>, Error> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| Error::Storage)?;
    let meta = file.metadata().map_err(|_| Error::Storage)?;
    if !meta.is_file() || meta.len() > limit {
        return Err(Error::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Storage)?;
    if bytes.len() as u64 > limit {
        return Err(Error::TooLarge);
    }
    Ok(bytes)
}
