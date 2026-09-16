use crate::ResolvedAiProfile;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const DEFAULT_MAX_ITEMS_PER_REQUEST: u16 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranslationBatchPolicy {
    pub(super) max_items_per_request: u16,
}

impl TranslationBatchPolicy {
    #[must_use]
    pub const fn new(max_items_per_request: u16) -> Option<Self> {
        if max_items_per_request == 0 {
            None
        } else {
            Some(Self {
                max_items_per_request,
            })
        }
    }

    #[must_use]
    pub const fn max_items_per_request(self) -> u16 {
        self.max_items_per_request
    }
}

impl Default for TranslationBatchPolicy {
    fn default() -> Self {
        Self {
            max_items_per_request: DEFAULT_MAX_ITEMS_PER_REQUEST,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorCategory {
    Network,
    Timeout,
    Cancelled,
    Authentication,
    Permission,
    InvalidRequest,
    ModelNotFound,
    RateLimited,
    QuotaOrBilling,
    Overloaded,
    SafetyOrRefusal,
    MalformedOutput,
    ProviderInternal,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    pub(super) category: ProviderErrorCategory,
    pub(super) retryable: bool,
    pub(super) retry_after_ms: Option<u64>,
    pub(super) provider_code: Option<Box<str>>,
    pub(super) request_id: Option<Box<str>>,
    pub(super) http_status: Option<u16>,
    pub(super) safe_message: Box<str>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub(super) input_tokens: u64,
    pub(super) output_tokens: u64,
    pub(super) reasoning_tokens: u64,
    pub(super) cached_input_tokens: u64,
    pub(super) total_tokens: u64,
}

impl ProviderUsage {
    #[must_use]
    pub const fn new(
        input_tokens: u64,
        output_tokens: u64,
        reasoning_tokens: u64,
        cached_input_tokens: u64,
        total_tokens: u64,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            reasoning_tokens,
            cached_input_tokens,
            total_tokens,
        }
    }

    #[must_use]
    pub const fn input_tokens(self) -> u64 {
        self.input_tokens
    }

    #[must_use]
    pub const fn output_tokens(self) -> u64 {
        self.output_tokens
    }

    #[must_use]
    pub const fn reasoning_tokens(self) -> u64 {
        self.reasoning_tokens
    }

    #[must_use]
    pub const fn cached_input_tokens(self) -> u64 {
        self.cached_input_tokens
    }

    #[must_use]
    pub const fn total_tokens(self) -> u64 {
        self.total_tokens
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(other.reasoning_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(other.cached_input_tokens);
        self.total_tokens = self.total_tokens.saturating_add(other.total_tokens);
    }
}

impl ProviderError {
    #[must_use]
    pub fn new(
        category: ProviderErrorCategory,
        retryable: bool,
        safe_message: impl Into<Box<str>>,
    ) -> Self {
        Self {
            category,
            retryable,
            retry_after_ms: None,
            provider_code: None,
            request_id: None,
            http_status: None,
            safe_message: safe_message.into(),
        }
    }

    #[must_use]
    pub const fn category(&self) -> ProviderErrorCategory {
        self.category
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    #[must_use]
    pub const fn retry_after_ms(&self) -> Option<u64> {
        self.retry_after_ms
    }

    #[must_use]
    pub fn safe_message(&self) -> &str {
        &self.safe_message
    }

    #[must_use]
    pub(crate) fn with_http_status(mut self, status: u16) -> Self {
        self.http_status = Some(status);
        self
    }

    #[must_use]
    pub(crate) fn with_request_id(mut self, request_id: Option<&str>) -> Self {
        self.request_id = request_id.map(Into::into);
        self
    }

    #[must_use]
    pub(crate) fn with_retry_after_ms(mut self, retry_after_ms: Option<u64>) -> Self {
        self.retry_after_ms = retry_after_ms;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderItem {
    pub(super) item_id: Box<str>,
    pub(super) source: Box<str>,
    pub(super) protected_tokens: Vec<Box<str>>,
    pub(super) context: Option<Box<str>>,
    pub(super) disambiguation: Option<Box<str>>,
}

impl ProviderItem {
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

    #[must_use]
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    #[must_use]
    pub fn disambiguation(&self) -> Option<&str> {
        self.disambiguation.as_deref()
    }
}

pub struct ProviderRequest<'a> {
    pub(super) profile: &'a ResolvedAiProfile,
    pub(super) source_locale: &'a str,
    pub(super) target_locale: &'a str,
    pub(super) items: Vec<ProviderItem>,
}

impl<'a> ProviderRequest<'a> {
    #[must_use]
    pub const fn profile(&self) -> &ResolvedAiProfile {
        self.profile
    }

    #[must_use]
    pub const fn source_locale(&self) -> &str {
        self.source_locale
    }

    #[must_use]
    pub const fn target_locale(&self) -> &str {
        self.target_locale
    }

    #[must_use]
    pub fn items(&self) -> &[ProviderItem] {
        &self.items
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderTranslation {
    pub(super) item_id: Box<str>,
    pub(super) text: Box<str>,
}

impl ProviderTranslation {
    #[must_use]
    pub fn new(item_id: impl Into<Box<str>>, text: impl Into<Box<str>>) -> Self {
        Self {
            item_id: item_id.into(),
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderBatchResult {
    pub(super) translations: Vec<ProviderTranslation>,
    pub(super) usage: Option<ProviderUsage>,
}

impl ProviderBatchResult {
    #[must_use]
    pub fn new(translations: impl IntoIterator<Item = ProviderTranslation>) -> Self {
        Self {
            translations: translations.into_iter().collect(),
            usage: None,
        }
    }

    #[must_use]
    pub const fn with_usage(mut self, usage: ProviderUsage) -> Self {
        self.usage = Some(usage);
        self
    }
}

#[derive(Clone)]
pub struct CancellationToken {
    pub(super) cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub(crate) fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }

    #[cfg(test)]
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

pub trait TranslationProvider: Send + Sync + 'static {
    fn translate(
        &self,
        request: &ProviderRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, ProviderError>;
}
