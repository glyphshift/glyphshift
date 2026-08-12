use crate::FilterPolicy;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const PROFILE_SCHEMA: &str = "glyphshift.ai-profiles/2";
const PROFILE_FILE_NAME: &str = "ai-profiles.json";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AiProviderProtocol {
    OpenAiResponses,
    OpenAiChatCompletions,
    OpenAiCompatible,
    AnthropicMessages,
    GeminiGenerateContent,
    OllamaChat,
}

impl AiProviderProtocol {
    #[must_use]
    pub const fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenAiResponses | Self::OpenAiChatCompletions => "https://api.openai.com/v1",
            Self::OpenAiCompatible => "",
            Self::AnthropicMessages => "https://api.anthropic.com",
            Self::GeminiGenerateContent => "https://generativelanguage.googleapis.com/v1beta",
            Self::OllamaChat => "http://127.0.0.1:11434/api",
        }
    }

    #[must_use]
    pub const fn default_concurrency(self) -> u16 {
        match self {
            Self::OllamaChat => 1,
            _ => 2,
        }
    }

    #[must_use]
    pub const fn credential_required(self) -> bool {
        !matches!(self, Self::OllamaChat | Self::OpenAiCompatible)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialVaultError {
    Missing,
    Unavailable,
    Rejected,
}

pub trait CredentialVault: Send + Sync {
    fn replace(&self, credential_ref: &str, secret: &str) -> Result<(), CredentialVaultError>;
    fn contains(&self, credential_ref: &str) -> Result<bool, CredentialVaultError>;
    fn delete(&self, credential_ref: &str) -> Result<(), CredentialVaultError>;
    fn expose(&self, credential_ref: &str) -> Result<Box<str>, CredentialVaultError>;
}

#[derive(Deserialize)]
#[serde(tag = "action", content = "secret", rename_all = "snake_case")]
pub enum CredentialUpdate {
    Keep,
    Replace(Box<str>),
    Clear,
}

impl Default for CredentialUpdate {
    fn default() -> Self {
        Self::Keep
    }
}

impl CredentialUpdate {
    #[must_use]
    pub fn replace(secret: impl Into<Box<str>>) -> Self {
        Self::Replace(secret.into())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AiProfileDraft {
    id: Box<str>,
    name: Box<str>,
    protocol: AiProviderProtocol,
    base_url: Box<str>,
    model_id: Box<str>,
    timeout_ms: u64,
    max_concurrency: u16,
    filter_policy: FilterPolicy,
    #[serde(default)]
    credential: CredentialUpdate,
}

impl AiProfileDraft {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        protocol: AiProviderProtocol,
        model_id: impl Into<Box<str>>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            protocol,
            base_url: protocol.default_base_url().into(),
            model_id: model_id.into(),
            timeout_ms: 60_000,
            max_concurrency: protocol.default_concurrency(),
            filter_policy: FilterPolicy::default(),
            credential: CredentialUpdate::Keep,
        }
    }

    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<Box<str>>) -> Self {
        self.base_url = base_url.into();
        self
    }

    #[must_use]
    pub fn with_credential(mut self, credential: CredentialUpdate) -> Self {
        self.credential = credential;
        self
    }

    #[must_use]
    pub fn with_filter_policy(mut self, filter_policy: FilterPolicy) -> Self {
        self.filter_policy = filter_policy;
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredAiProfile {
    id: Box<str>,
    name: Box<str>,
    protocol: AiProviderProtocol,
    base_url: Box<str>,
    model_id: Box<str>,
    credential_ref: Box<str>,
    timeout_ms: u64,
    max_concurrency: u16,
    filter_policy: FilterPolicy,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ProfileArtifact {
    schema: Box<str>,
    default_profile_id: Option<Box<str>>,
    profiles: Vec<StoredAiProfile>,
}

impl Default for ProfileArtifact {
    fn default() -> Self {
        Self {
            schema: PROFILE_SCHEMA.into(),
            default_profile_id: None,
            profiles: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiProfileView {
    id: Box<str>,
    name: Box<str>,
    protocol: AiProviderProtocol,
    base_url: Box<str>,
    model_id: Box<str>,
    timeout_ms: u64,
    max_concurrency: u16,
    filter_policy: FilterPolicy,
    has_credential: bool,
    credential_required: bool,
}

pub struct ResolvedAiProfile {
    id: Box<str>,
    protocol: AiProviderProtocol,
    base_url: Box<str>,
    model_id: Box<str>,
    credential: Option<Box<str>>,
    timeout_ms: u64,
    max_concurrency: u16,
}

impl ResolvedAiProfile {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn protocol(&self) -> AiProviderProtocol {
        self.protocol
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    #[must_use]
    pub fn credential(&self) -> Option<&str> {
        self.credential.as_deref()
    }

    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    #[must_use]
    pub const fn max_concurrency(&self) -> u16 {
        self.max_concurrency
    }
}

impl AiProfileView {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn protocol(&self) -> AiProviderProtocol {
        self.protocol
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    #[must_use]
    pub const fn has_credential(&self) -> bool {
        self.has_credential
    }

    #[must_use]
    pub const fn credential_required(&self) -> bool {
        self.credential_required
    }

    #[must_use]
    pub const fn filter_policy(&self) -> &FilterPolicy {
        &self.filter_policy
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiProfileError {
    Storage,
    InvalidArtifact,
    InvalidProfile(&'static str),
    UnknownProfile(Box<str>),
    Credential(CredentialVaultError),
}

pub struct AiProfileCatalog {
    path: PathBuf,
    artifact: ProfileArtifact,
    vault: Box<dyn CredentialVault>,
}

impl AiProfileCatalog {
    pub fn open(
        data_root: impl AsRef<Path>,
        vault: Box<dyn CredentialVault>,
    ) -> Result<Self, AiProfileError> {
        let path = data_root.as_ref().join(PROFILE_FILE_NAME);
        let artifact = if path.exists() {
            let reader = BufReader::new(File::open(&path).map_err(|_| AiProfileError::Storage)?);
            let artifact = serde_json::from_reader::<_, ProfileArtifact>(reader)
                .map_err(|_| AiProfileError::InvalidArtifact)?;
            validate_artifact(&artifact)?;
            artifact
        } else {
            ProfileArtifact::default()
        };
        Ok(Self {
            path,
            artifact,
            vault,
        })
    }

    #[must_use]
    pub fn default_profile_id(&self) -> Option<&str> {
        self.artifact.default_profile_id.as_deref()
    }

    pub fn profiles(&self) -> Result<Vec<AiProfileView>, AiProfileError> {
        self.artifact
            .profiles
            .iter()
            .map(|profile| profile_view(profile, self.vault.as_ref()))
            .collect()
    }

    pub fn profile(&self, profile_id: &str) -> Result<AiProfileView, AiProfileError> {
        let profile = self
            .artifact
            .profiles
            .iter()
            .find(|profile| profile.id.as_ref() == profile_id)
            .ok_or_else(|| AiProfileError::UnknownProfile(profile_id.into()))?;
        profile_view(profile, self.vault.as_ref())
    }

    pub fn resolve_profile(&self, profile_id: &str) -> Result<ResolvedAiProfile, AiProfileError> {
        let profile = self
            .artifact
            .profiles
            .iter()
            .find(|profile| profile.id.as_ref() == profile_id)
            .ok_or_else(|| AiProfileError::UnknownProfile(profile_id.into()))?;
        let has_credential = self
            .vault
            .contains(&profile.credential_ref)
            .map_err(AiProfileError::Credential)?;
        if profile.protocol.credential_required() && !has_credential {
            return Err(AiProfileError::Credential(CredentialVaultError::Missing));
        }
        let credential = has_credential
            .then(|| self.vault.expose(&profile.credential_ref))
            .transpose()
            .map_err(AiProfileError::Credential)?;
        Ok(ResolvedAiProfile {
            id: profile.id.clone(),
            protocol: profile.protocol,
            base_url: profile.base_url.clone(),
            model_id: profile.model_id.clone(),
            credential,
            timeout_ms: profile.timeout_ms,
            max_concurrency: profile.max_concurrency,
        })
    }

    pub fn save_profile(&mut self, draft: AiProfileDraft) -> Result<AiProfileView, AiProfileError> {
        let (profile, credential) = stored_profile(draft)?;
        let credential_ref = profile.credential_ref.clone();
        let index = self
            .artifact
            .profiles
            .iter()
            .position(|candidate| candidate.id == profile.id);
        match credential {
            CredentialUpdate::Keep => {}
            CredentialUpdate::Replace(secret) => {
                let secret = secret.trim();
                if secret.is_empty() {
                    return Err(AiProfileError::InvalidProfile("credential"));
                }
                self.vault
                    .replace(&credential_ref, secret)
                    .map_err(AiProfileError::Credential)?;
            }
            CredentialUpdate::Clear => self
                .vault
                .delete(&credential_ref)
                .map_err(AiProfileError::Credential)?,
        }
        if let Some(index) = index {
            self.artifact.profiles[index] = profile.clone();
        } else {
            self.artifact.profiles.push(profile.clone());
        }
        self.artifact
            .profiles
            .sort_by(|left, right| left.id.cmp(&right.id));
        if self.artifact.default_profile_id.is_none() {
            self.artifact.default_profile_id = Some(profile.id.clone());
        }
        self.persist()?;
        profile_view(&profile, self.vault.as_ref())
    }

    pub fn set_default_profile(&mut self, profile_id: &str) -> Result<(), AiProfileError> {
        if !self
            .artifact
            .profiles
            .iter()
            .any(|profile| profile.id.as_ref() == profile_id)
        {
            return Err(AiProfileError::UnknownProfile(profile_id.into()));
        }
        self.artifact.default_profile_id = Some(profile_id.into());
        self.persist()
    }

    pub fn delete_profile(&mut self, profile_id: &str) -> Result<(), AiProfileError> {
        let index = self
            .artifact
            .profiles
            .iter()
            .position(|profile| profile.id.as_ref() == profile_id)
            .ok_or_else(|| AiProfileError::UnknownProfile(profile_id.into()))?;
        let profile = self.artifact.profiles.remove(index);
        self.vault
            .delete(&profile.credential_ref)
            .map_err(AiProfileError::Credential)?;
        if self.artifact.default_profile_id.as_deref() == Some(profile_id) {
            self.artifact.default_profile_id = self
                .artifact
                .profiles
                .first()
                .map(|profile| profile.id.clone());
        }
        self.persist()
    }

    fn persist(&self) -> Result<(), AiProfileError> {
        let parent = self.path.parent().ok_or(AiProfileError::Storage)?;
        fs::create_dir_all(parent).map_err(|_| AiProfileError::Storage)?;
        let mut temporary = NamedTempFile::new_in(parent).map_err(|_| AiProfileError::Storage)?;
        {
            let mut writer = BufWriter::new(temporary.as_file_mut());
            serde_json::to_writer_pretty(&mut writer, &self.artifact)
                .map_err(|_| AiProfileError::Storage)?;
            writer
                .write_all(b"\n")
                .map_err(|_| AiProfileError::Storage)?;
            writer.flush().map_err(|_| AiProfileError::Storage)?;
        }
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| AiProfileError::Storage)?;
        temporary
            .persist(&self.path)
            .map_err(|_| AiProfileError::Storage)?;
        Ok(())
    }
}

fn stored_profile(
    draft: AiProfileDraft,
) -> Result<(StoredAiProfile, CredentialUpdate), AiProfileError> {
    let AiProfileDraft {
        id,
        name,
        protocol,
        base_url,
        model_id,
        timeout_ms,
        max_concurrency,
        filter_policy,
        credential,
    } = draft;
    let name = name.trim();
    let base_url = base_url.trim().trim_end_matches('/');
    let model_id = model_id.trim();
    if !safe_identifier(&id) {
        return Err(AiProfileError::InvalidProfile("id"));
    }
    if name.is_empty() || name.chars().count() > 128 {
        return Err(AiProfileError::InvalidProfile("name"));
    }
    if model_id.is_empty() || model_id.chars().count() > 256 {
        return Err(AiProfileError::InvalidProfile("model"));
    }
    if base_url.is_empty() || !(base_url.starts_with("https://") || base_url.starts_with("http://"))
    {
        return Err(AiProfileError::InvalidProfile("base-url"));
    }
    if !(1_000..=600_000).contains(&timeout_ms) || max_concurrency == 0 {
        return Err(AiProfileError::InvalidProfile("request-policy"));
    }
    let credential_ref: Box<str> = format!("glyphshift.ai-profile/{id}").into();
    Ok((
        StoredAiProfile {
            id,
            name: name.into(),
            protocol,
            base_url: base_url.into(),
            model_id: model_id.into(),
            credential_ref,
            timeout_ms,
            max_concurrency,
            filter_policy,
        },
        credential,
    ))
}

fn profile_view(
    profile: &StoredAiProfile,
    vault: &dyn CredentialVault,
) -> Result<AiProfileView, AiProfileError> {
    let has_credential = vault
        .contains(&profile.credential_ref)
        .map_err(AiProfileError::Credential)?;
    Ok(AiProfileView {
        id: profile.id.clone(),
        name: profile.name.clone(),
        protocol: profile.protocol,
        base_url: profile.base_url.clone(),
        model_id: profile.model_id.clone(),
        timeout_ms: profile.timeout_ms,
        max_concurrency: profile.max_concurrency,
        filter_policy: profile.filter_policy.clone(),
        has_credential,
        credential_required: profile.protocol.credential_required(),
    })
}

fn validate_artifact(artifact: &ProfileArtifact) -> Result<(), AiProfileError> {
    let ids = artifact
        .profiles
        .iter()
        .map(|profile| &profile.id)
        .collect::<BTreeSet<_>>();
    if artifact.schema.as_ref() != PROFILE_SCHEMA
        || ids.len() != artifact.profiles.len()
        || artifact
            .default_profile_id
            .as_ref()
            .is_some_and(|profile_id| !ids.contains(profile_id))
        || artifact.profiles.iter().any(|profile| {
            !safe_identifier(&profile.id)
                || profile.name.trim().is_empty()
                || profile.model_id.trim().is_empty()
                || profile.base_url.trim().is_empty()
                || profile.timeout_ms == 0
                || profile.max_concurrency == 0
        })
    {
        return Err(AiProfileError::InvalidArtifact);
    }
    Ok(())
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}
