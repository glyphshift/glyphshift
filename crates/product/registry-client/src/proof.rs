use crate::{strict_json, Error, MAX_PROOF};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Artifact {
    pub schema: String,
    pub kind: String,
    pub package_id: String,
    pub version: String,
    pub media_type: String,
    pub sha256: String,
    pub size: u64,
    pub compatibility: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Statement {
    pub schema: String,
    pub issuer: String,
    pub publisher_user_key: String,
    pub artifact: Artifact,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    jws: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    alg: String,
    kid: String,
    typ: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Trust {
    schema: String,
    issuer: String,
    keys: Vec<Key>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Key {
    id: String,
    public_key: String,
    state: String,
}

#[derive(Clone)]
pub struct TrustStore {
    issuer: String,
    keys: BTreeMap<String, VerifyingKey>,
}

#[derive(Clone, Debug)]
pub struct VerifiedProof {
    statement: Statement,
    key_id: String,
    jws: String,
}
impl VerifiedProof {
    pub fn statement(&self) -> &Statement {
        &self.statement
    }
    pub fn key_id(&self) -> &str {
        &self.key_id
    }
    pub fn jws(&self) -> &str {
        &self.jws
    }
}

#[derive(Clone, Debug)]
pub struct ReleaseRequest {
    kind: String,
    package: String,
    version: String,
    publisher: String,
}
impl ReleaseRequest {
    pub fn new(kind: &str, package: &str, version: &str, publisher: &str) -> Result<Self, Error> {
        if !matches!(kind, "dictionary" | "adapter")
            || !identifier(package)
            || !valid_version(version)
            || !user_key(publisher)
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            kind: kind.into(),
            package: package.into(),
            version: version.into(),
            publisher: publisher.into(),
        })
    }
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn package_id(&self) -> &str {
        &self.package
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn publisher(&self) -> &str {
        &self.publisher
    }
    pub fn byte_limit(&self) -> u64 {
        if self.kind == "adapter" {
            64 * 1024 * 1024
        } else {
            16 * 1024 * 1024
        }
    }
    pub(crate) fn matches(&self, s: &Statement) -> bool {
        s.artifact.kind == self.kind
            && s.artifact.package_id == self.package
            && s.artifact.version == self.version
            && s.publisher_user_key == self.publisher
    }
}

impl TrustStore {
    pub fn from_json(data: &[u8]) -> Result<Self, Error> {
        if data.len() > 65536 {
            return Err(Error::Configuration);
        }
        let config: Trust = strict_json::parse(data).map_err(|_| Error::Configuration)?;
        if config.schema != "glyphshift.release-trust/1"
            || config.issuer.is_empty()
            || config.issuer.len() > 256
            || config.issuer.trim() != config.issuer
            || config.keys.is_empty()
            || config.keys.len() > 64
        {
            return Err(Error::Configuration);
        }
        let mut keys = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for key in config.keys {
            if !identifier(&key.id)
                || !seen.insert(key.id.clone())
                || !matches!(key.state.as_str(), "active" | "revoked")
            {
                return Err(Error::Configuration);
            }
            let raw: [u8; 32] = URL_SAFE_NO_PAD
                .decode(&key.public_key)
                .map_err(|_| Error::Configuration)?
                .try_into()
                .map_err(|_| Error::Configuration)?;
            let public = VerifyingKey::from_bytes(&raw).map_err(|_| Error::Configuration)?;
            if public.is_weak() {
                return Err(Error::Configuration);
            }
            if key.state == "active" {
                keys.insert(key.id, public);
            }
        }
        Ok(Self {
            issuer: config.issuer,
            keys,
        })
    }

    pub fn verify(&self, envelope: &[u8]) -> Result<VerifiedProof, Error> {
        if envelope.len() as u64 > MAX_PROOF {
            return Err(Error::TooLarge);
        }
        let proof: Envelope = strict_json::parse(envelope)?;
        if proof.schema != "glyphshift.release-proof/1" {
            return Err(Error::InvalidProof);
        }
        self.verify_jws(&proof.jws)
    }

    pub(crate) fn verify_jws(&self, jws: &str) -> Result<VerifiedProof, Error> {
        if jws.len() > 2 * 1024 * 1024 {
            return Err(Error::TooLarge);
        }
        let parts: Vec<_> = jws.split('.').collect();
        let [head, payload, signature] = parts.as_slice() else {
            return Err(Error::InvalidProof);
        };
        if head.len() > 1368 || signature.len() != 86 {
            return Err(Error::InvalidProof);
        }
        let header: Header = strict_json::parse(
            &URL_SAFE_NO_PAD
                .decode(head)
                .map_err(|_| Error::InvalidProof)?,
        )?;
        if header.alg != "EdDSA" || header.typ != "glyphshift-release+jws" {
            return Err(Error::InvalidProof);
        }
        let key = self.keys.get(&header.kid).ok_or(Error::UntrustedKey)?;
        let signature = Signature::from_slice(
            &URL_SAFE_NO_PAD
                .decode(signature)
                .map_err(|_| Error::InvalidProof)?,
        )
        .map_err(|_| Error::InvalidProof)?;
        key.verify_strict(format!("{head}.{payload}").as_bytes(), &signature)
            .map_err(|_| Error::InvalidProof)?;
        let statement: Statement = strict_json::parse(
            &URL_SAFE_NO_PAD
                .decode(payload)
                .map_err(|_| Error::InvalidProof)?,
        )?;
        let a = &statement.artifact;
        let request = ReleaseRequest::new(
            &a.kind,
            &a.package_id,
            &a.version,
            &statement.publisher_user_key,
        )
        .map_err(|_| Error::InvalidProof)?;
        let media = if a.kind == "adapter" {
            "application/vnd.glyphshift.plugin+zip;version=1"
        } else {
            "application/vnd.glyphshift.dictionary+json;version=3"
        };
        if statement.schema != "glyphshift.release-statement/1"
            || statement.issuer != self.issuer
            || a.schema != "glyphshift.resource-inspection/1"
            || a.size == 0
            || a.size > request.byte_limit()
            || a.media_type != media
            || !valid_hash(&a.sha256)
            || !a.compatibility.is_object()
        {
            return Err(Error::InvalidProof);
        }
        Ok(VerifiedProof {
            statement,
            key_id: header.kid,
            jws: jws.into(),
        })
    }

    /// Offline cryptographic/static validation. This is not a current-availability check or execution authorization.
    pub fn verify_artifact(
        &self,
        proof: &[u8],
        request: &ReleaseRequest,
        bytes: &[u8],
    ) -> Result<VerifiedProof, Error> {
        let verified = self.verify(proof)?;
        if !request.matches(verified.statement()) {
            return Err(Error::IdentityMismatch);
        }
        if bytes.len() as u64 != verified.statement.artifact.size
            || glyphshift_plugin_package::sha256(bytes) != verified.statement.artifact.sha256
        {
            return Err(Error::InvalidArtifact);
        }
        // Reuse the authoritative Rust inspector; never replace GSP/dictionary validation with a loose JSON parser.
        let actual = glyphshift_resource_inspector::inspect(request.kind(), bytes)
            .map_err(|_| Error::InvalidArtifact)?;
        if actual
            != serde_json::to_value(&verified.statement.artifact)
                .map_err(|_| Error::InvalidProof)?
        {
            return Err(Error::InvalidArtifact);
        }
        Ok(verified)
    }
}

fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 96
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn valid_version(s: &str) -> bool {
    s.len() <= 128 && semver::Version::parse(s).is_ok_and(|v| v.build.is_empty())
}
fn user_key(s: &str) -> bool {
    s.len() == 8
        && s.bytes()
            .all(|b| b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(&b))
}
fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
