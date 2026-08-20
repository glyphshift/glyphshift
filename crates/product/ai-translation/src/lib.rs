//! AI-assisted translation planning and execution.

mod codex;
mod history;
mod http;
mod job;
mod profile;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub use codex::*;
pub use history::*;
pub use http::*;
pub use job::*;
pub use profile::*;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    EmptySource,
    AlreadyTranslated,
    Ignored,
    PureNumberOrSymbols,
    NumericMeasurement,
    SingleCharacter,
    ContainsDigit,
    Url,
    Email,
    FilePath,
    Shortcut,
    TooLong,
    CustomPattern,
    DuplicateSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    InvalidExcludedPattern { index: usize },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct FilterPolicy {
    skip_pure_numbers_or_symbols: bool,
    skip_numeric_measurements: bool,
    skip_single_character: bool,
    skip_text_containing_digits: bool,
    skip_urls: bool,
    skip_emails: bool,
    skip_file_paths: bool,
    skip_shortcuts: bool,
    max_source_chars: Option<usize>,
    excluded_patterns: Vec<Box<str>>,
}

impl Default for FilterPolicy {
    fn default() -> Self {
        Self {
            skip_pure_numbers_or_symbols: true,
            skip_numeric_measurements: true,
            skip_single_character: true,
            skip_text_containing_digits: false,
            skip_urls: true,
            skip_emails: true,
            skip_file_paths: true,
            skip_shortcuts: true,
            max_source_chars: None,
            excluded_patterns: Vec::new(),
        }
    }
}

impl FilterPolicy {
    pub(crate) fn from_persisted_value(value: Option<&serde_json::Value>) -> Self {
        let mut policy = Self::default();
        let Some(object) = value.and_then(serde_json::Value::as_object) else {
            return policy;
        };
        macro_rules! restore_bool {
            ($field:ident, $key:literal) => {
                if let Some(value) = object.get($key).and_then(serde_json::Value::as_bool) {
                    policy.$field = value;
                }
            };
        }
        restore_bool!(skip_pure_numbers_or_symbols, "skipPureNumbersOrSymbols");
        restore_bool!(skip_numeric_measurements, "skipNumericMeasurements");
        restore_bool!(skip_single_character, "skipSingleCharacter");
        restore_bool!(skip_text_containing_digits, "skipTextContainingDigits");
        restore_bool!(skip_urls, "skipUrls");
        restore_bool!(skip_emails, "skipEmails");
        restore_bool!(skip_file_paths, "skipFilePaths");
        restore_bool!(skip_shortcuts, "skipShortcuts");
        policy.max_source_chars = object
            .get("maxSourceChars")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        policy.excluded_patterns = object
            .get("excludedPatterns")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(Into::into))
            .collect();
        policy
    }

    #[must_use]
    pub const fn with_skip_text_containing_digits(mut self, enabled: bool) -> Self {
        self.skip_text_containing_digits = enabled;
        self
    }

    #[must_use]
    pub const fn with_max_source_chars(mut self, maximum: Option<usize>) -> Self {
        self.max_source_chars = maximum;
        self
    }

    #[must_use]
    pub fn with_excluded_patterns(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.excluded_patterns = patterns.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TranslationItem {
    item_id: Box<str>,
    source: Box<str>,
    translation: Option<Box<str>>,
    ignored: bool,
}

impl TranslationItem {
    #[must_use]
    pub fn untranslated(item_id: impl Into<Box<str>>, source: impl Into<Box<str>>) -> Self {
        Self {
            item_id: item_id.into(),
            source: source.into(),
            translation: None,
            ignored: false,
        }
    }

    #[must_use]
    pub fn translated(
        item_id: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        translation: impl Into<Box<str>>,
    ) -> Self {
        Self {
            item_id: item_id.into(),
            source: source.into(),
            translation: Some(translation.into()),
            ignored: false,
        }
    }

    #[must_use]
    pub const fn ignored(mut self) -> Self {
        self.ignored = true;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationPlanRequest {
    scope_id: Box<str>,
    snapshot_revision: u64,
    source_locale: Box<str>,
    target_locale: Box<str>,
    items: Vec<TranslationItem>,
    filter_policy: FilterPolicy,
}

impl TranslationPlanRequest {
    #[must_use]
    pub fn new(
        scope_id: impl Into<Box<str>>,
        snapshot_revision: u64,
        source_locale: impl Into<Box<str>>,
        target_locale: impl Into<Box<str>>,
        items: impl IntoIterator<Item = TranslationItem>,
    ) -> Self {
        Self {
            scope_id: scope_id.into(),
            snapshot_revision,
            source_locale: source_locale.into(),
            target_locale: target_locale.into(),
            items: items.into_iter().collect(),
            filter_policy: FilterPolicy::default(),
        }
    }

    #[must_use]
    pub fn with_filter_policy(mut self, filter_policy: FilterPolicy) -> Self {
        self.filter_policy = filter_policy;
        self
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationCandidate {
    item_id: Box<str>,
    source: Box<str>,
    protected_tokens: Vec<Box<str>>,
}

impl TranslationCandidate {
    #[must_use]
    pub fn item_id(&self) -> &str {
        &self.item_id
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn protected_tokens(&self) -> &[Box<str>] {
        &self.protected_tokens
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkippedTranslationItem {
    item_id: Box<str>,
    source: Box<str>,
    reason: SkipReason,
}

impl SkippedTranslationItem {
    #[must_use]
    pub fn item_id(&self) -> &str {
        &self.item_id
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub const fn reason(&self) -> SkipReason {
        self.reason
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationPlan {
    token: Box<str>,
    scope_id: Box<str>,
    snapshot_revision: u64,
    source_locale: Box<str>,
    target_locale: Box<str>,
    candidates: Vec<TranslationCandidate>,
    skipped: Vec<SkippedTranslationItem>,
}

impl TranslationPlan {
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    #[must_use]
    pub const fn snapshot_revision(&self) -> u64 {
        self.snapshot_revision
    }

    #[must_use]
    pub fn candidates(&self) -> &[TranslationCandidate] {
        &self.candidates
    }

    #[must_use]
    pub fn skipped(&self) -> &[SkippedTranslationItem] {
        &self.skipped
    }

    #[must_use]
    pub fn skip_reason(&self, item_id: &str) -> Option<SkipReason> {
        self.skipped
            .iter()
            .find(|item| item.item_id.as_ref() == item_id)
            .map(|item| item.reason)
    }

    #[must_use]
    pub const fn eligible_count(&self) -> usize {
        self.candidates.len()
    }

    #[must_use]
    pub const fn skipped_count(&self) -> usize {
        self.skipped.len()
    }
}

#[derive(Default)]
pub struct AiTranslation {
    next_plan_id: u64,
    plans: BTreeMap<Box<str>, TranslationPlan>,
    next_job_id: u64,
    jobs: BTreeMap<Box<str>, std::sync::Arc<job::JobCell>>,
    providers: BTreeMap<AiProviderProtocol, std::sync::Arc<dyn TranslationProvider>>,
}

impl AiTranslation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn plan_translation(
        &mut self,
        request: TranslationPlanRequest,
    ) -> Result<TranslationPlan, PlanError> {
        let excluded_patterns = request
            .filter_policy
            .excluded_patterns
            .iter()
            .enumerate()
            .map(|(index, pattern)| {
                Regex::new(pattern).map_err(|_| PlanError::InvalidExcludedPattern { index })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.next_plan_id = self.next_plan_id.saturating_add(1);
        let token: Box<str> = format!("plan-{}", self.next_plan_id).into();
        let mut candidates = Vec::new();
        let mut skipped = Vec::new();
        let mut candidate_sources = BTreeSet::new();
        for item in request.items {
            let source = item.source.trim();
            let reason = skip_reason(&item, source, &request.filter_policy, &excluded_patterns)
                .or_else(|| {
                    candidate_sources
                        .contains(source)
                        .then_some(SkipReason::DuplicateSource)
                });
            if let Some(reason) = reason {
                skipped.push(SkippedTranslationItem {
                    item_id: item.item_id,
                    source: source.into(),
                    reason,
                });
            } else {
                candidate_sources.insert(Box::<str>::from(source));
                candidates.push(TranslationCandidate {
                    item_id: item.item_id,
                    source: source.into(),
                    protected_tokens: protected_tokens(source),
                });
            }
        }
        let plan = TranslationPlan {
            token: token.clone(),
            scope_id: request.scope_id,
            snapshot_revision: request.snapshot_revision,
            source_locale: request.source_locale,
            target_locale: request.target_locale,
            candidates,
            skipped,
        };
        self.plans.insert(token, plan.clone());
        Ok(plan)
    }
}

fn skip_reason(
    item: &TranslationItem,
    source: &str,
    policy: &FilterPolicy,
    excluded_patterns: &[Regex],
) -> Option<SkipReason> {
    if item
        .translation
        .as_deref()
        .is_some_and(|translation| !translation.trim().is_empty())
    {
        return Some(SkipReason::AlreadyTranslated);
    }
    if item.ignored {
        return Some(SkipReason::Ignored);
    }
    if source.is_empty() {
        return Some(SkipReason::EmptySource);
    }
    if policy.skip_urls && is_url(source) {
        return Some(SkipReason::Url);
    }
    if policy.skip_emails && is_email(source) {
        return Some(SkipReason::Email);
    }
    if policy.skip_shortcuts && is_shortcut(source) {
        return Some(SkipReason::Shortcut);
    }
    if policy.skip_numeric_measurements && is_numeric_measurement(source) {
        return Some(SkipReason::NumericMeasurement);
    }
    if policy.skip_pure_numbers_or_symbols && is_pure_number_or_symbols(source) {
        return Some(SkipReason::PureNumberOrSymbols);
    }
    if policy.skip_file_paths && is_file_path(source) {
        return Some(SkipReason::FilePath);
    }
    if policy.skip_single_character && source.chars().count() == 1 {
        return Some(SkipReason::SingleCharacter);
    }
    if policy.skip_text_containing_digits && source.chars().any(|character| character.is_numeric())
    {
        return Some(SkipReason::ContainsDigit);
    }
    if policy
        .max_source_chars
        .is_some_and(|maximum| source.chars().count() > maximum)
    {
        return Some(SkipReason::TooLong);
    }
    if excluded_patterns
        .iter()
        .any(|pattern| pattern.is_match(source))
    {
        return Some(SkipReason::CustomPattern);
    }
    None
}

fn is_url(source: &str) -> bool {
    let lowercase = source.to_ascii_lowercase();
    lowercase.starts_with("http://")
        || lowercase.starts_with("https://")
        || lowercase.starts_with("ftp://")
        || lowercase.starts_with("www.")
}

fn is_email(source: &str) -> bool {
    let Some((local, domain)) = source.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !source.chars().any(char::is_whitespace)
}

fn is_file_path(source: &str) -> bool {
    !source.chars().any(char::is_whitespace)
        && (source.contains('/')
            || source.contains('\\')
            || source.rsplit_once('.').is_some_and(|(stem, extension)| {
                !stem.is_empty()
                    && extension.len() <= 10
                    && extension
                        .chars()
                        .next()
                        .is_some_and(|character| character.is_ascii_alphabetic())
                    && extension.chars().all(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                    })
            }))
}

fn is_shortcut(source: &str) -> bool {
    let parts = source.split('+').map(str::trim).collect::<Vec<_>>();
    parts.len() >= 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        && parts.iter().any(|part| {
            matches!(
                part.to_ascii_lowercase().as_str(),
                "ctrl" | "alt" | "shift" | "win" | "cmd" | "command"
            )
        })
}

fn is_pure_number_or_symbols(source: &str) -> bool {
    !source.chars().any(char::is_alphabetic)
        && (source.chars().any(char::is_numeric) || !source.chars().any(char::is_alphanumeric))
}

fn is_numeric_measurement(source: &str) -> bool {
    if !source.chars().any(char::is_numeric) {
        return false;
    }
    if source
        .chars()
        .any(|character| matches!(character, '/' | ':' | '×' | '*'))
    {
        return true;
    }
    if source.ends_with('%')
        && source[..source.len() - 1].chars().all(|character| {
            character.is_numeric() || character.is_whitespace() || ".,+-".contains(character)
        })
    {
        return true;
    }
    let letters = source
        .chars()
        .filter(|character| character.is_alphabetic())
        .collect::<String>()
        .to_lowercase();
    matches!(
        letters.as_str(),
        "x" | "fps" | "px" | "pt" | "dpi" | "hz" | "khz" | "mhz" | "ms" | "gb" | "mb" | "kb"
    )
}

fn protected_tokens(source: &str) -> Vec<Box<str>> {
    let mut tokens = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'$' && bytes.get(index + 1) == Some(&b'{') {
            if let Some(end) = source[index + 2..].find('}') {
                let end = index + 2 + end + 1;
                tokens.push(source[index..end].into());
                index = end;
                continue;
            }
        }
        if bytes[index] == b'{' {
            if let Some(end) = source[index + 1..].find('}') {
                let end = index + 1 + end + 1;
                tokens.push(source[index..end].into());
                index = end;
                continue;
            }
        }
        if bytes[index] == b'%' {
            let mut end = index + 1;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            if end > index + 1 {
                tokens.push(source[index..end].into());
                index = end;
                continue;
            }
        }
        index += 1;
    }
    tokens
}
