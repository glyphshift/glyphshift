use crate::{Error, ReleaseRequest, TrustStore, VerifiedProof, MAX_PROOF};
use glyphshift_dictionary_distribution::{DictionaryInstallationView, DictionaryReplacementPolicy};
use glyphshift_plugin_package::{Package, PluginStore, Selection};
use reqwest::blocking::Client;
use reqwest::{header, redirect, Certificate, StatusCode};
use std::io::Read;
use std::path::Path;
use std::time::Duration;
use url::Url;

mod discovery;

pub struct RegistryClient {
    origin: Url,
    http: Client,
    trust: TrustStore,
}

struct Fetched {
    proof: VerifiedProof,
    body: Vec<u8>,
    download_url: String,
}

impl RegistryClient {
    pub fn new(origin: &str, trust: TrustStore) -> Result<Self, Error> {
        Self::with_ca(origin, trust, None)
    }

    /// Optional CA augments system roots for explicitly configured deployments; TLS verification stays enabled.
    pub fn with_ca(origin: &str, trust: TrustStore, ca_pem: Option<&[u8]>) -> Result<Self, Error> {
        let origin = Url::parse(origin).map_err(|_| Error::Configuration)?;
        if origin.scheme() != "https"
            || origin.host_str().is_none()
            || origin.path() != "/"
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
        {
            return Err(Error::Configuration);
        }
        let mut builder = Client::builder()
            .https_only(true)
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .user_agent("Glyphshift-Registry/1");
        if let Some(pem) = ca_pem {
            if pem.len() > 65536 {
                return Err(Error::Configuration);
            }
            builder = builder.add_root_certificate(
                Certificate::from_pem(pem).map_err(|_| Error::Configuration)?,
            );
        }
        Ok(Self {
            origin,
            http: builder.build().map_err(|_| Error::Configuration)?,
            trust,
        })
    }

    fn endpoint(&self, request: &ReleaseRequest, operation: &str) -> Result<Url, Error> {
        let mut url = self.origin.clone();
        url.path_segments_mut()
            .map_err(|_| Error::Configuration)?
            .clear()
            .extend([
                "v1",
                "packages",
                request.kind(),
                request.package_id(),
                "releases",
                request.version(),
                operation,
            ]);
        Ok(url)
    }

    fn get(&self, url: Url, limit: u64, expected_type: &str) -> Result<Vec<u8>, Error> {
        let response = self
            .http
            .get(url)
            .header(header::ACCEPT, expected_type)
            .header(header::ACCEPT_ENCODING, "identity")
            .header(header::CACHE_CONTROL, "no-cache")
            .send()
            .map_err(|_| Error::Unavailable)?;
        match response.status() {
            StatusCode::OK => {}
            StatusCode::NOT_FOUND => return Err(Error::NotFound),
            _ => return Err(Error::Unavailable),
        }
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .ok_or(Error::InvalidArtifact)?;
        // Only ASCII whitespace around media-type separators is semantically irrelevant here.
        if content_type.replace(' ', "") != expected_type.replace(' ', "")
            || response
                .headers()
                .get(header::CONTENT_ENCODING)
                .is_some_and(|v| v != "identity")
        {
            return Err(Error::InvalidArtifact);
        }
        if response.content_length().is_some_and(|size| size > limit) {
            return Err(Error::TooLarge);
        }
        let mut body = Vec::new();
        response
            .take(limit + 1)
            .read_to_end(&mut body)
            .map_err(|_| Error::Unavailable)?;
        if body.len() as u64 > limit {
            return Err(Error::TooLarge);
        }
        Ok(body)
    }

    fn fetch_current(&self, request: &ReleaseRequest) -> Result<Fetched, Error> {
        let proof_url = self.endpoint(request, "proof")?;
        let envelope = self.get(proof_url.clone(), MAX_PROOF, "application/json")?;
        let proof = self.trust.verify(&envelope)?;
        if !request.matches(proof.statement()) {
            return Err(Error::IdentityMismatch);
        }
        let artifact = &proof.statement().artifact;
        let download = self.endpoint(request, "download")?;
        let body = self.get(download.clone(), artifact.size, &artifact.media_type)?;
        self.trust.verify_artifact(&envelope, request, &body)?;
        // A previously valid cached proof must not install a version withdrawn during download.
        let current = self.get(proof_url, MAX_PROOF, "application/json")?;
        let current = self.trust.verify(&current)?;
        if current.jws() != proof.jws() {
            return Err(Error::InvalidProof);
        }
        Ok(Fetched {
            proof,
            body,
            download_url: download.to_string(),
        })
    }

    /// Explicitly stages a selected version; it never selects/loads a DLL or changes a running target.
    pub fn install_adapter(
        &self,
        request: &ReleaseRequest,
        root: &Path,
    ) -> Result<Selection, Error> {
        if request.kind() != "adapter" {
            return Err(Error::Configuration);
        }
        let fetched = self.fetch_current(request)?;
        let package = Package::from_bytes(&fetched.body).map_err(|_| Error::InvalidArtifact)?;
        PluginStore::new(root)
            .install(&package, &fetched.proof.statement().artifact.sha256)
            .map_err(|e| match e {
                glyphshift_plugin_package::PackageError::Conflict => Error::LocalChangesConflict,
                _ => Error::Storage,
            })
    }

    /// Updates may replace only an unmodified verified installation; no force-overwrite path is exposed.
    pub fn install_dictionary(
        &self,
        request: &ReleaseRequest,
        catalog_id: &str,
        root: &Path,
        replacement: DictionaryReplacementPolicy,
    ) -> Result<DictionaryInstallationView, Error> {
        if request.kind() != "dictionary" || replacement == DictionaryReplacementPolicy::ReplaceAny
        {
            return Err(Error::Configuration);
        }
        self.prepare_dictionary(request)?
            .install(self.trust.clone(), catalog_id, root, replacement)
    }

    /// Fetch and verify without holding the desktop's mutation lock.
    pub fn prepare_dictionary(
        &self,
        request: &ReleaseRequest,
    ) -> Result<PreparedDictionary, Error> {
        if request.kind() != "dictionary" {
            return Err(Error::Configuration);
        }
        Ok(PreparedDictionary {
            fetched: self.fetch_current(request)?,
            prepared_at: std::time::Instant::now(),
        })
    }
}

#[cfg(test)]
mod tests;

/// Short-lived verified bytes; cannot be constructed by callers or serialized as approval.
pub struct PreparedDictionary {
    fetched: Fetched,
    prepared_at: std::time::Instant,
}
impl PreparedDictionary {
    /// Commit under the caller's editing lock, using freshly loaded trusted keys.
    pub fn install(
        self,
        trust: TrustStore,
        catalog_id: &str,
        root: &Path,
        replacement: DictionaryReplacementPolicy,
    ) -> Result<DictionaryInstallationView, Error> {
        if replacement == DictionaryReplacementPolicy::ReplaceAny {
            return Err(Error::Configuration);
        }
        if self.prepared_at.elapsed() > Duration::from_secs(30) {
            return Err(Error::Unavailable);
        }
        let fetched = self.fetched;
        trust.verify_jws(fetched.proof.jws())?;
        crate::dictionary::install(
            trust,
            fetched.proof,
            fetched.body,
            fetched.download_url,
            catalog_id,
            root,
            replacement,
        )
    }
}
