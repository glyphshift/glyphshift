use crate::FilterPolicy;
use crate::DEFAULT_MAX_ITEMS_PER_REQUEST;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const PROFILE_SCHEMA: &str = "glyphshift.ai-profiles/4";
pub const DEFAULT_TIMEOUT_MS: u64 = 1_800_000;
pub const MAX_TIMEOUT_MS: u64 = 3_600_000;
pub const DEFAULT_MAX_RETRIES: u16 = 2;
pub const MAX_MAX_RETRIES: u16 = 10;

const fn default_max_retries() -> u16 {
    DEFAULT_MAX_RETRIES
}

const fn default_max_items_per_request() -> u16 {
    DEFAULT_MAX_ITEMS_PER_REQUEST
}
const PROFILE_FILE_NAME: &str = "ai-profiles.json";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AiProviderProtocol {
    CodexSubscription,
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
            Self::CodexSubscription => "codex://local",
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
            Self::CodexSubscription | Self::OllamaChat => 1,
            _ => 2,
        }
    }

    #[must_use]
    pub const fn credential_required(self) -> bool {
        !matches!(
            self,
            Self::CodexSubscription | Self::OllamaChat | Self::OpenAiCompatible
        )
    }

    #[must_use]
    pub const fn supports_reasoning_control(self) -> bool {
        matches!(
            self,
            Self::CodexSubscription
                | Self::OpenAiResponses
                | Self::OpenAiChatCompletions
                | Self::OpenAiCompatible
        )
    }

    #[must_use]
    pub const fn default_reasoning_effort(self) -> AiReasoningEffort {
        if self.supports_reasoning_control() {
            AiReasoningEffort::Disabled
        } else {
            AiReasoningEffort::Automatic
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiReasoningEffort {
    Disabled,
    Automatic,
    Low,
    Medium,
    High,
    Maximum,
}

impl Default for AiReasoningEffort {
    fn default() -> Self {
        Self::Disabled
    }
}

impl AiReasoningEffort {
    #[must_use]
    pub const fn responses_value(self) -> Option<&'static str> {
        match self {
            Self::Disabled => Some("none"),
            Self::Automatic => None,
            Self::Low => Some("low"),
            Self::Medium => Some("medium"),
            Self::High => Some("high"),
            Self::Maximum => Some("max"),
        }
    }

    #[must_use]
    pub const fn chat_value(self) -> Option<&'static str> {
        self.responses_value()
    }

    #[must_use]
    pub const fn codex_value(self) -> Option<&'static str> {
        match self {
            Self::Disabled => Some("none"),
            Self::Automatic => None,
            Self::Low => Some("low"),
            Self::Medium => Some("medium"),
            Self::High => Some("high"),
            Self::Maximum => Some("xhigh"),
        }
    }
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
    #[serde(default)]
    reasoning_effort: AiReasoningEffort,
    timeout_ms: u64,
    #[serde(default = "default_max_items_per_request")]
    max_items_per_request: u16,
    max_concurrency: u16,
    #[serde(default = "default_max_retries")]
    max_retries: u16,
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
            reasoning_effort: protocol.default_reasoning_effort(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_items_per_request: DEFAULT_MAX_ITEMS_PER_REQUEST,
            max_concurrency: protocol.default_concurrency(),
            max_retries: DEFAULT_MAX_RETRIES,
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

    #[must_use]
    pub const fn with_reasoning_effort(mut self, reasoning_effort: AiReasoningEffort) -> Self {
        self.reasoning_effort = reasoning_effort;
        self
    }

    #[must_use]
    pub const fn with_max_retries(mut self, max_retries: u16) -> Self {
        self.max_retries = max_retries;
        self
    }

    #[must_use]
    pub const fn with_max_items_per_request(mut self, max_items_per_request: u16) -> Self {
        self.max_items_per_request = max_items_per_request;
        self
    }

    #[must_use]
    pub const fn with_max_concurrency(mut self, max_concurrency: u16) -> Self {
        self.max_concurrency = max_concurrency;
        self
    }

    #[must_use]
    pub const fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct StoredAiProfile {
    id: Box<str>,
    name: Box<str>,
    protocol: AiProviderProtocol,
    base_url: Box<str>,
    model_id: Box<str>,
    #[serde(default)]
    reasoning_effort: AiReasoningEffort,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    credential: Option<Box<str>>,
    timeout_ms: u64,
    #[serde(default = "default_max_items_per_request")]
    max_items_per_request: u16,
    max_concurrency: u16,
    #[serde(default = "default_max_retries")]
    max_retries: u16,
    filter_policy: FilterPolicy,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
    reasoning_effort: AiReasoningEffort,
    timeout_ms: u64,
    max_items_per_request: u16,
    max_concurrency: u16,
    max_retries: u16,
    filter_policy: FilterPolicy,
    credential: Option<Box<str>>,
    has_credential: bool,
    credential_required: bool,
}

pub struct ResolvedAiProfile {
    id: Box<str>,
    name: Box<str>,
    protocol: AiProviderProtocol,
    base_url: Box<str>,
    model_id: Box<str>,
    reasoning_effort: AiReasoningEffort,
    credential: Option<Box<str>>,
    timeout_ms: u64,
    max_items_per_request: u16,
    max_concurrency: u16,
    max_retries: u16,
}

impl ResolvedAiProfile {
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
    pub const fn reasoning_effort(&self) -> AiReasoningEffort {
        self.reasoning_effort
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

    #[must_use]
    pub const fn max_items_per_request(&self) -> u16 {
        self.max_items_per_request
    }

    #[must_use]
    pub const fn max_retries(&self) -> u16 {
        self.max_retries
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
    pub const fn reasoning_effort(&self) -> AiReasoningEffort {
        self.reasoning_effort
    }

    #[must_use]
    pub const fn has_credential(&self) -> bool {
        self.has_credential
    }

    #[must_use]
    pub fn credential(&self) -> Option<&str> {
        self.credential.as_deref()
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
    MissingCredential,
}

pub struct AiProfileCatalog {
    path: PathBuf,
    artifact: ProfileArtifact,
}

impl AiProfileCatalog {
    pub fn open(data_root: impl AsRef<Path>) -> Result<Self, AiProfileError> {
        let path = data_root.as_ref().join(PROFILE_FILE_NAME);
        let mut artifact = if path.exists() {
            let reader = BufReader::new(File::open(&path).map_err(|_| AiProfileError::Storage)?);
            let value = match serde_json::from_reader::<_, serde_json::Value>(reader) {
                Ok(value) => value,
                Err(_) => {
                    preserve_invalid_profile_artifact(&path);
                    serde_json::Value::Null
                }
            };
            profile_artifact_from_value(&value)
        } else {
            ProfileArtifact::default()
        };
        let mut migrated = artifact.schema.as_ref() != PROFILE_SCHEMA;
        for profile in &mut artifact.profiles {
            let normalized = normalized_model_id(profile.protocol, &profile.model_id);
            if normalized.as_ref() != profile.model_id.as_ref() {
                profile.model_id = normalized;
                migrated = true;
            }
        }
        normalize_artifact(&mut artifact);
        validate_artifact(&artifact)?;
        let catalog = Self { path, artifact };
        if migrated {
            catalog.persist()?;
        }
        Ok(catalog)
    }

    #[must_use]
    pub fn default_profile_id(&self) -> Option<&str> {
        self.artifact.default_profile_id.as_deref()
    }

    pub fn profiles(&self) -> Result<Vec<AiProfileView>, AiProfileError> {
        Ok(self.artifact.profiles.iter().map(profile_view).collect())
    }

    pub fn profile(&self, profile_id: &str) -> Result<AiProfileView, AiProfileError> {
        let profile = self
            .artifact
            .profiles
            .iter()
            .find(|profile| profile.id.as_ref() == profile_id)
            .ok_or_else(|| AiProfileError::UnknownProfile(profile_id.into()))?;
        Ok(profile_view(profile))
    }

    pub fn resolve_profile(&self, profile_id: &str) -> Result<ResolvedAiProfile, AiProfileError> {
        let profile = self
            .artifact
            .profiles
            .iter()
            .find(|profile| profile.id.as_ref() == profile_id)
            .ok_or_else(|| AiProfileError::UnknownProfile(profile_id.into()))?;
        let has_credential = profile.credential.is_some();
        if profile.protocol.credential_required() && !has_credential {
            return Err(AiProfileError::MissingCredential);
        }
        Ok(ResolvedAiProfile {
            id: profile.id.clone(),
            name: profile.name.clone(),
            protocol: profile.protocol,
            base_url: profile.base_url.clone(),
            model_id: profile.model_id.clone(),
            reasoning_effort: profile.reasoning_effort,
            credential: profile.credential.clone(),
            timeout_ms: profile.timeout_ms,
            max_items_per_request: profile.max_items_per_request,
            max_concurrency: profile.max_concurrency,
            max_retries: profile.max_retries,
        })
    }

    pub fn save_profile(&mut self, draft: AiProfileDraft) -> Result<AiProfileView, AiProfileError> {
        let (mut profile, credential) = stored_profile(draft)?;
        let index = self
            .artifact
            .profiles
            .iter()
            .position(|candidate| candidate.id == profile.id);
        profile.credential = match credential {
            CredentialUpdate::Keep => {
                index.and_then(|index| self.artifact.profiles[index].credential.clone())
            }
            CredentialUpdate::Replace(secret) => {
                let secret = secret.trim();
                if secret.is_empty() {
                    return Err(AiProfileError::InvalidProfile("credential"));
                }
                Some(secret.into())
            }
            CredentialUpdate::Clear => None,
        };
        if profile.protocol.credential_required() && profile.credential.is_none() {
            return Err(AiProfileError::InvalidProfile("credential"));
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
        Ok(profile_view(&profile))
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
        self.artifact.profiles.remove(index);
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
        reasoning_effort,
        timeout_ms,
        max_items_per_request,
        max_concurrency,
        max_retries,
        filter_policy,
        credential,
    } = draft;
    let name = name.trim();
    let base_url = base_url.trim().trim_end_matches('/');
    let model_id = normalized_model_id(protocol, model_id.trim());
    if !safe_identifier(&id) {
        return Err(AiProfileError::InvalidProfile("id"));
    }
    if name.is_empty() || name.chars().count() > 128 {
        return Err(AiProfileError::InvalidProfile("name"));
    }
    if model_id.is_empty() || model_id.chars().count() > 256 {
        return Err(AiProfileError::InvalidProfile("model"));
    }
    let valid_base_url = if protocol == AiProviderProtocol::CodexSubscription {
        base_url == protocol.default_base_url()
    } else {
        !base_url.is_empty()
            && (base_url.starts_with("https://") || base_url.starts_with("http://"))
    };
    if !valid_base_url {
        return Err(AiProfileError::InvalidProfile("base-url"));
    }
    if !(1_000..=MAX_TIMEOUT_MS).contains(&timeout_ms)
        || !(1..=1_000).contains(&max_items_per_request)
        || max_concurrency == 0
        || max_retries > MAX_MAX_RETRIES
    {
        return Err(AiProfileError::InvalidProfile("request-policy"));
    }
    Ok((
        StoredAiProfile {
            id,
            name: name.into(),
            protocol,
            base_url: base_url.into(),
            model_id,
            reasoning_effort,
            credential: None,
            timeout_ms,
            max_items_per_request,
            max_concurrency,
            max_retries,
            filter_policy,
        },
        credential,
    ))
}

fn profile_view(profile: &StoredAiProfile) -> AiProfileView {
    AiProfileView {
        id: profile.id.clone(),
        name: profile.name.clone(),
        protocol: profile.protocol,
        base_url: profile.base_url.clone(),
        model_id: profile.model_id.clone(),
        reasoning_effort: profile.reasoning_effort,
        timeout_ms: profile.timeout_ms,
        max_items_per_request: profile.max_items_per_request,
        max_concurrency: profile.max_concurrency,
        max_retries: profile.max_retries,
        filter_policy: profile.filter_policy.clone(),
        credential: profile.credential.clone(),
        has_credential: profile.credential.is_some(),
        credential_required: profile.protocol.credential_required(),
    }
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
                || profile
                    .credential
                    .as_deref()
                    .is_some_and(|credential| credential.trim().is_empty())
                || profile.base_url.trim().is_empty()
                || (profile.protocol == AiProviderProtocol::CodexSubscription
                    && profile.base_url.as_ref()
                        != AiProviderProtocol::CodexSubscription.default_base_url())
                || !(1_000..=MAX_TIMEOUT_MS).contains(&profile.timeout_ms)
                || profile.max_concurrency == 0
                || !(1..=1_000).contains(&profile.max_items_per_request)
                || profile.max_retries > MAX_MAX_RETRIES
        })
    {
        return Err(AiProfileError::InvalidArtifact);
    }
    Ok(())
}

fn profile_artifact_from_value(value: &serde_json::Value) -> ProfileArtifact {
    let Some(object) = value.as_object() else {
        return ProfileArtifact::default();
    };
    ProfileArtifact {
        schema: object
            .get("schema")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(PROFILE_SCHEMA)
            .into(),
        default_profile_id: object
            .get("defaultProfileId")
            .and_then(serde_json::Value::as_str)
            .map(Into::into),
        profiles: object
            .get("profiles")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(stored_profile_from_value)
            .collect(),
    }
}

fn stored_profile_from_value(value: &serde_json::Value) -> Option<StoredAiProfile> {
    let object = value.as_object()?;
    let id = object.get("id")?.as_str()?;
    let protocol: AiProviderProtocol = object
        .get("protocol")
        .and_then(|value| serde_json::from_value(value.clone()).ok())?;
    let name = object
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(id);
    let model_id = object.get("modelId")?.as_str()?;
    let base_url = object
        .get("baseUrl")
        .and_then(serde_json::Value::as_str)
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| protocol.default_base_url());
    let reasoning_effort = object
        .get("reasoningEffort")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_else(|| protocol.default_reasoning_effort());
    let timeout_ms = object
        .get("timeoutMs")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| (1_000..=MAX_TIMEOUT_MS).contains(value))
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    let max_items_per_request = object
        .get("maxItemsPerRequest")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| (1..=1_000).contains(value))
        .unwrap_or(DEFAULT_MAX_ITEMS_PER_REQUEST);
    let max_concurrency = object
        .get("maxConcurrency")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| protocol.default_concurrency());
    let max_retries = object
        .get("maxRetries")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| *value <= MAX_MAX_RETRIES)
        .unwrap_or(DEFAULT_MAX_RETRIES);
    Some(StoredAiProfile {
        id: id.into(),
        name: name.into(),
        protocol,
        base_url: base_url.into(),
        model_id: model_id.into(),
        reasoning_effort,
        credential: object
            .get("credential")
            .and_then(serde_json::Value::as_str)
            .filter(|credential| !credential.trim().is_empty())
            .map(Into::into),
        timeout_ms,
        max_items_per_request,
        max_concurrency,
        max_retries,
        filter_policy: FilterPolicy::from_persisted_value(object.get("filterPolicy")),
    })
}

fn preserve_invalid_profile_artifact(path: &Path) {
    let backup = path.with_extension("invalid.json");
    if !backup.exists() {
        let _ = fs::copy(path, backup);
    }
}

fn stored_profile_is_valid(profile: &StoredAiProfile) -> bool {
    safe_identifier(&profile.id)
        && !profile.name.trim().is_empty()
        && !profile.model_id.trim().is_empty()
        && profile
            .credential
            .as_deref()
            .is_none_or(|credential| !credential.trim().is_empty())
        && !profile.base_url.trim().is_empty()
        && (profile.protocol != AiProviderProtocol::CodexSubscription
            || profile.base_url.as_ref()
                == AiProviderProtocol::CodexSubscription.default_base_url())
        && (1_000..=MAX_TIMEOUT_MS).contains(&profile.timeout_ms)
        && profile.max_concurrency > 0
        && (1..=1_000).contains(&profile.max_items_per_request)
        && profile.max_retries <= MAX_MAX_RETRIES
}

fn normalize_artifact(artifact: &mut ProfileArtifact) {
    artifact.schema = PROFILE_SCHEMA.into();
    let mut ids = BTreeSet::new();
    artifact
        .profiles
        .retain(|profile| stored_profile_is_valid(profile) && ids.insert(profile.id.clone()));
    if artifact
        .default_profile_id
        .as_ref()
        .is_some_and(|profile_id| !ids.contains(profile_id))
    {
        artifact.default_profile_id = None;
    }
}

fn normalized_model_id(protocol: AiProviderProtocol, model_id: &str) -> Box<str> {
    if protocol == AiProviderProtocol::CodexSubscription
        && model_id.eq_ignore_ascii_case("gpt-sol-5.6")
    {
        "gpt-5.6-sol".into()
    } else {
        model_id.into()
    }
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}
