use super::super::{safe_identifier, CaptureError};
use super::MAX_PROBE_QUERY_PAGE_SIZE;
use crate::CaptureTranslationContext;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeRunError {
    InvalidInput,
    NotFound,
    AlreadyExists,
    InvalidState,
    InvalidRun,
    Storage,
    Observation,
    Export,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeRunStatus {
    Ready,
    Running,
    Paused,
    Interrupted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeRunCreate {
    pub(super) id: Box<str>,
    pub(super) workflow_id: Option<Box<str>>,
    pub(super) name: Box<str>,
    pub(super) software_id: Box<str>,
    pub(super) dictionary_id: Box<str>,
    pub(super) excluded_dictionary_ids: Vec<Box<str>>,
    pub(super) adapter_ids: Vec<Box<str>>,
    pub(super) live_preview_enabled: bool,
}

impl ProbeRunCreate {
    pub fn with_workflow(
        mut self,
        workflow_id: impl Into<Box<str>>,
    ) -> Result<Self, ProbeRunError> {
        let workflow_id = workflow_id.into();
        if !safe_identifier(&workflow_id) {
            return Err(ProbeRunError::InvalidInput);
        }
        self.workflow_id = Some(workflow_id);
        Ok(self)
    }

    pub fn with_excluded_dictionaries(mut self, ids: Vec<Box<str>>) -> Result<Self, ProbeRunError> {
        if ids
            .iter()
            .any(|id| !safe_identifier(id) || *id == self.dictionary_id)
            || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
        {
            return Err(ProbeRunError::InvalidInput);
        }
        self.excluded_dictionary_ids = ids;
        Ok(self)
    }

    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        software_id: impl Into<Box<str>>,
        dictionary_id: impl Into<Box<str>>,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
        live_preview_enabled: bool,
    ) -> Result<Self, ProbeRunError> {
        let id = id.into();
        let name = name.into();
        let software_id = software_id.into();
        let dictionary_id = dictionary_id.into();
        let adapter_ids = adapter_ids.into_iter().map(Into::into).collect::<Vec<_>>();
        if !safe_identifier(&id)
            || name.trim().is_empty()
            || software_id.trim().is_empty()
            || !safe_identifier(&dictionary_id)
            || adapter_ids.is_empty()
            || adapter_ids.iter().any(|id| !safe_identifier(id))
            || adapter_ids.iter().collect::<BTreeSet<_>>().len() != adapter_ids.len()
        {
            return Err(ProbeRunError::InvalidInput);
        }
        Ok(Self {
            id,
            workflow_id: None,
            name: name.trim().into(),
            software_id,
            dictionary_id,
            excluded_dictionary_ids: Vec::new(),
            adapter_ids,
            live_preview_enabled,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeRunUpdate {
    pub(super) name: Box<str>,
    pub(super) dictionary_id: Box<str>,
    pub(super) excluded_dictionary_ids: Vec<Box<str>>,
    pub(super) adapter_ids: Vec<Box<str>>,
    pub(super) live_preview_enabled: bool,
}

impl ProbeRunUpdate {
    pub fn with_excluded_dictionaries(mut self, ids: Vec<Box<str>>) -> Result<Self, ProbeRunError> {
        if ids
            .iter()
            .any(|id| !safe_identifier(id) || *id == self.dictionary_id)
            || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
        {
            return Err(ProbeRunError::InvalidInput);
        }
        self.excluded_dictionary_ids = ids;
        Ok(self)
    }

    pub fn new(
        name: impl Into<Box<str>>,
        dictionary_id: impl Into<Box<str>>,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
        live_preview_enabled: bool,
    ) -> Result<Self, ProbeRunError> {
        let name = name.into();
        let dictionary_id = dictionary_id.into();
        let adapter_ids = adapter_ids.into_iter().map(Into::into).collect::<Vec<_>>();
        if name.trim().is_empty()
            || !safe_identifier(&dictionary_id)
            || adapter_ids.is_empty()
            || adapter_ids.iter().any(|id| !safe_identifier(id))
            || adapter_ids.iter().collect::<BTreeSet<_>>().len() != adapter_ids.len()
        {
            return Err(ProbeRunError::InvalidInput);
        }
        Ok(Self {
            name: name.trim().into(),
            dictionary_id,
            excluded_dictionary_ids: Vec::new(),
            adapter_ids,
            live_preview_enabled,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProbeRunSummary {
    pub(super) id: Box<str>,
    #[serde(default)]
    pub(super) workflow_id: Option<Box<str>>,
    pub(super) name: Box<str>,
    pub(super) software_id: Box<str>,
    pub(super) dictionary_id: Box<str>,
    #[serde(default)]
    pub(super) excluded_dictionary_ids: Vec<Box<str>>,
    pub(super) adapter_ids: Vec<Box<str>>,
    pub(super) status: ProbeRunStatus,
    pub(super) live_preview_enabled: bool,
    pub(super) observation_revision: u64,
    pub(super) observed_count: usize,
    pub(super) ignored_count: usize,
    pub(super) dropped_observations: u64,
    pub(super) preview_generation: u64,
    pub(super) created_at_ms: u64,
    pub(super) updated_at_ms: u64,
}

impl ProbeRunSummary {
    #[must_use]
    pub fn workflow_id(&self) -> Option<&str> {
        self.workflow_id.as_deref()
    }

    pub fn excluded_dictionary_ids(&self) -> &[Box<str>] {
        &self.excluded_dictionary_ids
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn software_id(&self) -> &str {
        &self.software_id
    }

    #[must_use]
    pub fn dictionary_id(&self) -> &str {
        &self.dictionary_id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn adapter_ids(&self) -> &[Box<str>] {
        &self.adapter_ids
    }

    #[must_use]
    pub const fn status(&self) -> ProbeRunStatus {
        self.status
    }

    #[must_use]
    pub const fn live_preview_enabled(&self) -> bool {
        self.live_preview_enabled
    }

    #[must_use]
    pub const fn observation_revision(&self) -> u64 {
        self.observation_revision
    }

    #[must_use]
    pub const fn preview_generation(&self) -> u64 {
        self.preview_generation
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ProbeDictionaryEntry {
    pub(super) source: Box<str>,
    pub(super) translation: Box<str>,
}

impl ProbeDictionaryEntry {
    #[must_use]
    pub fn new(source: impl Into<Box<str>>, translation: impl Into<Box<str>>) -> Self {
        Self {
            source: source.into(),
            translation: translation.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeDictionarySnapshot {
    pub(super) revision: u64,
    pub(super) excluded_sources: Arc<BTreeSet<String>>,
    pub(super) entries: Arc<Vec<ProbeDictionaryEntry>>,
}

impl ProbeDictionarySnapshot {
    pub fn with_excluded_sources(mut self, sources: BTreeSet<String>) -> Self {
        self.excluded_sources = Arc::new(sources);
        self
    }

    pub fn new(
        revision: u64,
        entries: impl IntoIterator<Item = ProbeDictionaryEntry>,
    ) -> Result<Self, ProbeRunError> {
        let mut entries = entries.into_iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| left.source.cmp(&right.source));
        let valid = entries.iter().all(|entry| !entry.source.trim().is_empty());
        let unique = entries
            .iter()
            .map(|entry| &entry.source)
            .collect::<BTreeSet<_>>()
            .len()
            == entries.len();
        if revision == 0 || !valid || !unique {
            return Err(ProbeRunError::InvalidInput);
        }
        Ok(Self {
            revision,
            entries: Arc::new(entries),
            excluded_sources: Arc::default(),
        })
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeQuery {
    pub(super) search: Box<str>,
    pub(super) adapter_ids: Vec<Box<str>>,
    pub(super) translation_filter: ProbeTranslationFilter,
    pub(super) page: usize,
    pub(super) page_size: usize,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeTranslationFilter {
    #[default]
    All,
    Untranslated,
    Translated,
    Skipped,
    RuleMatched,
    OtherDictionary,
}

impl ProbeQuery {
    pub fn new(
        search: impl Into<Box<str>>,
        page: usize,
        page_size: usize,
    ) -> Result<Self, ProbeRunError> {
        if page == 0 || page_size == 0 || page_size > MAX_PROBE_QUERY_PAGE_SIZE {
            return Err(ProbeRunError::InvalidInput);
        }
        Ok(Self {
            search: search.into(),
            adapter_ids: Vec::new(),
            translation_filter: ProbeTranslationFilter::All,
            page,
            page_size,
        })
    }

    pub fn with_adapter_ids(
        mut self,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Result<Self, ProbeRunError> {
        let mut adapter_ids = adapter_ids.into_iter().map(Into::into).collect::<Vec<_>>();
        if adapter_ids.iter().any(|id| !safe_identifier(id))
            || adapter_ids.iter().collect::<BTreeSet<_>>().len() != adapter_ids.len()
        {
            return Err(ProbeRunError::InvalidInput);
        }
        adapter_ids.sort();
        self.adapter_ids = adapter_ids;
        Ok(self)
    }

    #[must_use]
    pub const fn with_translation_filter(
        mut self,
        translation_filter: ProbeTranslationFilter,
    ) -> Self {
        self.translation_filter = translation_filter;
        self
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeEntryState {
    Pending,
    Translated,
    Unobserved,
    Ignored,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProbeEntryRow {
    pub(super) source: Box<str>,
    pub(super) translation: Box<str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) translation_context: Option<CaptureTranslationContext>,
    pub(super) state: ProbeEntryState,
    pub(super) adapter_ids: Vec<Box<str>>,
    pub(super) count: u64,
    pub(super) first_seen_ms: u64,
    pub(super) last_seen_ms: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) translation_variants: Vec<ProbeDictionaryEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) resolution: Option<ProbeEntryResolution>,
}

/// Current configuration provenance; not evidence of a target pixel change.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProbeEntryResolution {
    pub kind: Box<str>,
    pub skip_reason: Option<Box<str>>,
    pub dictionary_ids: Vec<Box<str>>,
    pub rule_index: Option<usize>,
    pub editable: bool,
    pub edit_source: Option<Box<str>>,
    pub edit_translation: Option<Box<str>>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProbeEntryPage {
    pub(super) observation_revision: u64,
    pub(super) dictionary_revision: u64,
    pub(super) page: usize,
    pub(super) page_size: usize,
    pub(super) total: usize,
    pub(super) rows: Vec<ProbeEntryRow>,
}

impl ProbeEntryRow {
    pub fn set_resolution(&mut self, resolution: ProbeEntryResolution, translation: Option<&str>) {
        if let Some(translation) = translation {
            self.translation = translation.into();
        }
        self.resolution = Some(resolution);
    }
    pub fn has_translation_conflict(&self) -> bool {
        !self.translation_variants.is_empty()
    }
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translation(&self) -> &str {
        &self.translation
    }

    #[must_use]
    pub const fn translation_context(&self) -> Option<&CaptureTranslationContext> {
        self.translation_context.as_ref()
    }

    #[must_use]
    pub const fn state(&self) -> ProbeEntryState {
        self.state
    }
}

impl ProbeEntryPage {
    #[must_use]
    pub const fn observation_revision(&self) -> u64 {
        self.observation_revision
    }

    #[must_use]
    pub const fn dictionary_revision(&self) -> u64 {
        self.dictionary_revision
    }

    #[must_use]
    pub const fn total(&self) -> usize {
        self.total
    }

    #[must_use]
    pub fn rows(&self) -> &[ProbeEntryRow] {
        &self.rows
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewEntry {
    pub(super) source: Box<str>,
    pub(super) translation: Box<str>,
}

impl PreviewEntry {
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translation(&self) -> &str {
        &self.translation
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeExportFormat {
    EntriesJson,
    ObservationsJson,
    ObservationsCsv,
    EntriesCsv,
    DictionaryJson,
}

impl From<CaptureError> for ProbeRunError {
    fn from(_: CaptureError) -> Self {
        Self::Observation
    }
}
