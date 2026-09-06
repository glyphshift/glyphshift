use super::{
    safe_identifier, unix_time_millis, CaptureCatalog, CaptureConfiguration, CaptureError,
    CaptureSessionId,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

pub const PROBE_RUN_SCHEMA: &str = "glyphshift.probe-run/1";
pub const MAX_PROBE_QUERY_PAGE_SIZE: usize = 100;

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
    id: Box<str>,
    name: Box<str>,
    software_id: Box<str>,
    dictionary_id: Box<str>,
    adapter_ids: Vec<Box<str>>,
    live_preview_enabled: bool,
}

impl ProbeRunCreate {
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
            name: name.trim().into(),
            software_id,
            dictionary_id,
            adapter_ids,
            live_preview_enabled,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeRunUpdate {
    name: Box<str>,
    dictionary_id: Box<str>,
    adapter_ids: Vec<Box<str>>,
    live_preview_enabled: bool,
}

impl ProbeRunUpdate {
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
            adapter_ids,
            live_preview_enabled,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProbeRunSummary {
    id: Box<str>,
    name: Box<str>,
    software_id: Box<str>,
    dictionary_id: Box<str>,
    adapter_ids: Vec<Box<str>>,
    status: ProbeRunStatus,
    live_preview_enabled: bool,
    observation_revision: u64,
    observed_count: usize,
    ignored_count: usize,
    dropped_observations: u64,
    preview_generation: u64,
    created_at_ms: u64,
    updated_at_ms: u64,
}

impl ProbeRunSummary {
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeRunDocument {
    schema: Box<str>,
    storage_revision: u64,
    catalog_revision: u64,
    summary: ProbeRunSummary,
    ignored_sources: Vec<Box<str>>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ProbeDictionaryEntry {
    source: Box<str>,
    translation: Box<str>,
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
    revision: u64,
    entries: Vec<ProbeDictionaryEntry>,
}

impl ProbeDictionarySnapshot {
    pub fn new(
        revision: u64,
        entries: impl IntoIterator<Item = ProbeDictionaryEntry>,
    ) -> Result<Self, ProbeRunError> {
        let mut entries = entries.into_iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| left.source.cmp(&right.source));
        let valid = entries
            .iter()
            .all(|entry| !entry.source.trim().is_empty() && !entry.translation.trim().is_empty());
        let unique = entries
            .iter()
            .map(|entry| &entry.source)
            .collect::<BTreeSet<_>>()
            .len()
            == entries.len();
        if revision == 0 || !valid || !unique {
            return Err(ProbeRunError::InvalidInput);
        }
        Ok(Self { revision, entries })
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeQuery {
    search: Box<str>,
    adapter_ids: Vec<Box<str>>,
    translation_filter: ProbeTranslationFilter,
    page: usize,
    page_size: usize,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeTranslationFilter {
    #[default]
    All,
    Untranslated,
    Translated,
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
    source: Box<str>,
    translation: Box<str>,
    state: ProbeEntryState,
    adapter_ids: Vec<Box<str>>,
    count: u64,
    first_seen_ms: u64,
    last_seen_ms: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    translation_variants: Vec<ProbeDictionaryEntry>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProbeEntryPage {
    observation_revision: u64,
    dictionary_revision: u64,
    page: usize,
    page_size: usize,
    total: usize,
    rows: Vec<ProbeEntryRow>,
}

impl ProbeEntryRow {
    pub fn has_translation_conflict(&self) -> bool { !self.translation_variants.is_empty() }
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translation(&self) -> &str {
        &self.translation
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
    source: Box<str>,
    translation: Box<str>,
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
    ObservationsJson,
    ObservationsCsv,
    EntriesCsv,
    DictionaryJson,
}

pub struct ProbeRunStore {
    root: PathBuf,
    source_policies: BTreeMap<Box<str>, glyphshift_domain::SourceTextPolicy>,
}

struct ProbeSourceKeys {
    common: glyphshift_domain::SourceTextPolicy,
    normalized: BTreeMap<glyphshift_domain::SourceTextPolicy, BTreeSet<String>>,
    exact: BTreeSet<String>,
}
impl ProbeSourceKeys {
    fn key(&self, source: &str) -> String {
        use glyphshift_domain::SourceTextPolicy;
        if self.common != SourceTextPolicy::Exact { return self.common.key(source); }
        if self.exact.contains(source) { return source.to_owned(); }
        let candidates = self.normalized.iter().filter_map(|(policy, observed)| {
            let key = policy.key(source); observed.contains(&key).then_some(key)
        }).collect::<BTreeSet<_>>();
        if candidates.len() == 1 { candidates.into_iter().next().unwrap() } else { source.to_owned() }
    }
}

impl ProbeRunStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, ProbeRunError> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(ProbeRunError::InvalidInput);
        }
        fs::create_dir_all(&root).map_err(|_| ProbeRunError::Storage)?;
        let mut store = Self { root, source_policies: BTreeMap::new() };
        store.recover_disconnected()?;
        Ok(store)
    }

    pub fn set_source_policy(&mut self, adapter_id: impl Into<Box<str>>, policy: glyphshift_domain::SourceTextPolicy) {
        self.source_policies.insert(adapter_id.into(), policy);
    }

    fn run_policy(&self, document: &ProbeRunDocument) -> glyphshift_domain::SourceTextPolicy {
        let policies = document.summary.adapter_ids.iter().map(|id| self.source_policies.get(id).copied().unwrap_or_default()).collect::<BTreeSet<_>>();
        if policies.len() == 1 { *policies.first().unwrap() } else { glyphshift_domain::SourceTextPolicy::Exact }
    }

    fn source_keys(&self, document: &ProbeRunDocument, observations: Option<&CaptureCatalog>) -> ProbeSourceKeys {
        use glyphshift_domain::SourceTextPolicy;
        let mut keys = ProbeSourceKeys { common: self.run_policy(document), normalized: BTreeMap::new(), exact: BTreeSet::new() };
        if let Some(observations) = observations {
            for entry in observations.entries() {
                let policy = self.source_policies.get(entry.adapter_id()).copied().unwrap_or_default();
                if policy == SourceTextPolicy::Exact { keys.exact.insert(entry.source().to_owned()); }
                else { keys.normalized.entry(policy).or_default().insert(entry.source().to_owned()); }
            }
        }
        keys
    }

    pub fn create(&mut self, create: ProbeRunCreate) -> Result<ProbeRunSummary, ProbeRunError> {
        let directory = self.run_directory(&create.id);
        if directory.exists() {
            return Err(ProbeRunError::AlreadyExists);
        }
        fs::create_dir_all(&directory).map_err(|_| ProbeRunError::Storage)?;
        let now = unix_time_millis();
        let document = ProbeRunDocument {
            schema: PROBE_RUN_SCHEMA.into(),
            storage_revision: 1,
            catalog_revision: 0,
            summary: ProbeRunSummary {
                id: create.id,
                name: create.name,
                software_id: create.software_id,
                dictionary_id: create.dictionary_id,
                adapter_ids: create.adapter_ids,
                status: ProbeRunStatus::Ready,
                live_preview_enabled: create.live_preview_enabled,
                observation_revision: 0,
                observed_count: 0,
                ignored_count: 0,
                dropped_observations: 0,
                preview_generation: 0,
                created_at_ms: now,
                updated_at_ms: now,
            },
            ignored_sources: Vec::new(),
        };
        self.write_document(&document)?;
        Ok(document.summary)
    }

    pub fn delete(&mut self, run_id: &str) -> Result<(), ProbeRunError> {
        let document = self.read_document(run_id)?;
        if matches!(
            document.summary.status,
            ProbeRunStatus::Running | ProbeRunStatus::Paused
        ) {
            return Err(ProbeRunError::InvalidState);
        }
        let directory = self.run_directory(run_id);
        fs::remove_dir_all(directory).map_err(|_| ProbeRunError::Storage)
    }

    pub fn update(
        &mut self,
        run_id: &str,
        update: ProbeRunUpdate,
    ) -> Result<ProbeRunSummary, ProbeRunError> {
        let mut document = self.synchronized_document(run_id)?;
        let configuration_changed = document.summary.adapter_ids != update.adapter_ids
            || document.summary.dictionary_id != update.dictionary_id
            || document.summary.live_preview_enabled != update.live_preview_enabled;
        if configuration_changed
            && matches!(
                document.summary.status,
                ProbeRunStatus::Running | ProbeRunStatus::Paused
            )
        {
            return Err(ProbeRunError::InvalidState);
        }
        if document.summary.name == update.name
            && document.summary.dictionary_id == update.dictionary_id
            && document.summary.adapter_ids == update.adapter_ids
            && document.summary.live_preview_enabled == update.live_preview_enabled
        {
            return Ok(document.summary);
        }
        document.summary.name = update.name;
        document.summary.dictionary_id = update.dictionary_id;
        document.summary.adapter_ids = update.adapter_ids;
        document.summary.live_preview_enabled = update.live_preview_enabled;
        self.touch(&mut document);
        self.write_document(&document)?;
        Ok(document.summary)
    }

    pub fn clear_observations(&mut self, run_id: &str) -> Result<ProbeRunSummary, ProbeRunError> {
        let mut document = self.synchronized_document(run_id)?;
        if matches!(
            document.summary.status,
            ProbeRunStatus::Running | ProbeRunStatus::Paused
        ) {
            return Err(ProbeRunError::InvalidState);
        }
        let observation_path = self.observation_path(run_id);
        for slot in [
            observation_path.with_extension("a.json"),
            observation_path.with_extension("b.json"),
        ] {
            for path in [slot.clone(), slot.with_extension("pending")] {
                if path.exists() {
                    fs::remove_file(path).map_err(|_| ProbeRunError::Storage)?;
                }
            }
        }
        document.catalog_revision = 0;
        document.ignored_sources.clear();
        document.summary.observation_revision =
            document.summary.observation_revision.saturating_add(1);
        document.summary.observed_count = 0;
        document.summary.ignored_count = 0;
        document.summary.dropped_observations = 0;
        self.touch(&mut document);
        self.write_document(&document)?;
        Ok(document.summary)
    }

    pub fn list(&mut self) -> Result<Vec<ProbeRunSummary>, ProbeRunError> {
        let mut summaries = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|_| ProbeRunError::Storage)? {
            let entry = entry.map_err(|_| ProbeRunError::Storage)?;
            if !entry
                .file_type()
                .map_err(|_| ProbeRunError::Storage)?
                .is_dir()
            {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            if safe_identifier(&id) {
                if let Ok(summary) = self.synchronize(&id) {
                    summaries.push(summary);
                }
            }
        }
        summaries.sort_by(|left, right| {
            right
                .updated_at_ms
                .cmp(&left.updated_at_ms)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(summaries)
    }

    pub fn summary(&mut self, run_id: &str) -> Result<ProbeRunSummary, ProbeRunError> {
        self.synchronize(run_id)
    }

    pub fn capture_configuration(
        &self,
        run_id: &str,
        max_entries: u32,
    ) -> Result<CaptureConfiguration, ProbeRunError> {
        let document = self.read_document(run_id)?;
        CaptureConfiguration::new(
            CaptureSessionId::new(document.summary.id.clone())
                .map_err(|_| ProbeRunError::InvalidRun)?,
            self.observation_path(run_id),
            max_entries,
        )
        .map_err(|_| ProbeRunError::InvalidRun)
    }

    pub fn set_status(
        &mut self,
        run_id: &str,
        status: ProbeRunStatus,
    ) -> Result<ProbeRunSummary, ProbeRunError> {
        let mut document = self.read_document(run_id)?;
        document.summary.status = status;
        self.touch(&mut document);
        self.write_document(&document)?;
        Ok(document.summary)
    }

    pub fn set_preview_generation(
        &mut self,
        run_id: &str,
        generation: u64,
    ) -> Result<ProbeRunSummary, ProbeRunError> {
        let mut document = self.read_document(run_id)?;
        document.summary.preview_generation = generation;
        self.touch(&mut document);
        self.write_document(&document)?;
        Ok(document.summary)
    }

    pub fn query_entries(
        &mut self,
        run_id: &str,
        query: &ProbeQuery,
        dictionary: &ProbeDictionarySnapshot,
    ) -> Result<ProbeEntryPage, ProbeRunError> {
        let document = self.synchronized_document(run_id)?;
        if query
            .adapter_ids
            .iter()
            .any(|id| !document.summary.adapter_ids.contains(id))
        {
            return Err(ProbeRunError::InvalidInput);
        }
        let needle = query.search.trim().to_lowercase();
        let adapter_filter = query.adapter_ids.iter().collect::<BTreeSet<_>>();
        let mut rows = self
            .combined_rows(&document, dictionary)?
            .into_iter()
            .filter(|row| {
                let matches_adapter = adapter_filter.is_empty()
                    || row
                        .adapter_ids
                        .iter()
                        .any(|adapter| adapter_filter.contains(adapter));
                let matches_search = needle.is_empty()
                    || row.source.to_lowercase().contains(&needle)
                    || row.translation.to_lowercase().contains(&needle)
                    || row
                        .adapter_ids
                        .iter()
                        .any(|adapter| adapter.to_lowercase().contains(&needle));
                let matches_translation = match query.translation_filter {
                    ProbeTranslationFilter::All => true,
                    ProbeTranslationFilter::Untranslated => row.translation.trim().is_empty(),
                    ProbeTranslationFilter::Translated => !row.translation.trim().is_empty(),
                };
                matches_adapter && matches_search && matches_translation
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            right
                .last_seen_ms
                .cmp(&left.last_seen_ms)
                .then_with(|| left.source.cmp(&right.source))
        });
        let total = rows.len();
        let rows = paged(rows, query);
        Ok(ProbeEntryPage {
            observation_revision: document.summary.observation_revision,
            dictionary_revision: dictionary.revision(),
            page: query.page,
            page_size: query.page_size,
            total,
            rows,
        })
    }

    pub fn set_ignored(
        &mut self,
        run_id: &str,
        sources: &[Box<str>],
        ignored: bool,
    ) -> Result<ProbeRunSummary, ProbeRunError> {
        if sources.is_empty() {
            return Err(ProbeRunError::InvalidInput);
        }
        let mut document = self.synchronized_document(run_id)?;
        let observations = self.read_observations(run_id)?;
        let observed = observations
            .entries()
            .iter()
            .map(|entry| entry.source())
            .collect::<BTreeSet<_>>();
        if sources
            .iter()
            .any(|source| !observed.contains(source.as_ref()))
        {
            return Err(ProbeRunError::InvalidInput);
        }
        let mut ignored_sources = document
            .ignored_sources
            .into_iter()
            .collect::<BTreeSet<_>>();
        let before = ignored_sources.clone();
        for source in sources {
            if ignored {
                ignored_sources.insert(source.clone());
            } else {
                ignored_sources.remove(source);
            }
        }
        if ignored_sources == before {
            document.ignored_sources = ignored_sources.into_iter().collect();
            return Ok(document.summary);
        }
        document.ignored_sources = ignored_sources.into_iter().collect();
        document.summary.ignored_count = document.ignored_sources.len();
        document.summary.observation_revision =
            document.summary.observation_revision.saturating_add(1);
        self.touch(&mut document);
        self.write_document(&document)?;
        Ok(document.summary)
    }

    pub fn preview_entries(
        &mut self,
        run_id: &str,
        dictionary: &ProbeDictionarySnapshot,
    ) -> Result<Vec<PreviewEntry>, ProbeRunError> {
        let document = self.synchronized_document(run_id)?;
        let observations = self.read_observations(run_id).ok();
        let keys = self.source_keys(&document, observations.as_ref());
        let ignored = document
            .ignored_sources
            .into_iter()
            .collect::<BTreeSet<_>>();
        Ok(dictionary
            .entries
            .iter()
            .filter(|entry| !ignored.contains(&Box::<str>::from(keys.key(&entry.source))))
            .map(|entry| PreviewEntry {
                source: entry.source.clone(),
                translation: entry.translation.clone(),
            })
            .collect())
    }

    /// Expands an explicit clear action on a projected row to its saved keys.
    /// Merely reading or selecting a translation never deletes these variants.
    pub fn dictionary_sources_for_rows(&self, run_id: &str, sources: &[Box<str>], dictionary: &ProbeDictionarySnapshot) -> Result<Vec<Box<str>>, ProbeRunError> {
        let document = self.read_document(run_id)?;
        let observations = self.read_observations(run_id).ok();
        let keys = self.source_keys(&document, observations.as_ref());
        let selected = sources.iter().map(AsRef::as_ref).collect::<BTreeSet<&str>>();
        Ok(dictionary.entries.iter().filter(|entry| selected.contains(keys.key(&entry.source).as_str())).map(|entry| entry.source.clone()).collect())
    }

    pub fn export(
        &mut self,
        run_id: &str,
        format: ProbeExportFormat,
        dictionary: &ProbeDictionarySnapshot,
    ) -> Result<Vec<u8>, ProbeRunError> {
        let document = self.synchronized_document(run_id)?;
        match format {
            ProbeExportFormat::ObservationsJson => self
                .read_observations(run_id)?
                .encode_json()
                .map(String::into_bytes)
                .map_err(|_| ProbeRunError::Export),
            ProbeExportFormat::ObservationsCsv => {
                let observations = self.read_observations(run_id)?;
                let mut output =
                    String::from("\u{feff}source,adapterId,count,firstSeenMs,lastSeenMs\r\n");
                for entry in observations.entries() {
                    push_csv_row(
                        &mut output,
                        [
                            entry.source().to_owned(),
                            entry.adapter_id().to_owned(),
                            entry.count().to_string(),
                            entry.first_seen_ms().to_string(),
                            entry.last_seen_ms().to_string(),
                        ],
                    );
                }
                Ok(output.into_bytes())
            }
            ProbeExportFormat::EntriesCsv => {
                let mut output = String::from(
                    "\u{feff}source,translation,state,adapterIds,count,firstSeenMs,lastSeenMs\r\n",
                );
                for row in self.combined_rows(&document, dictionary)? {
                    push_csv_row(
                        &mut output,
                        [
                            row.source.to_string(),
                            row.translation.to_string(),
                            state_name(row.state).to_owned(),
                            row.adapter_ids.join(" | "),
                            row.count.to_string(),
                            row.first_seen_ms.to_string(),
                            row.last_seen_ms.to_string(),
                        ],
                    );
                }
                Ok(output.into_bytes())
            }
            ProbeExportFormat::DictionaryJson => Err(ProbeRunError::InvalidInput),
        }
    }

    fn combined_rows(
        &self,
        document: &ProbeRunDocument,
        dictionary: &ProbeDictionarySnapshot,
    ) -> Result<Vec<ProbeEntryRow>, ProbeRunError> {
        let ignored = document.ignored_sources.iter().collect::<BTreeSet<_>>();
        let translations = dictionary
            .entries
            .iter()
            .map(|entry| (entry.source.as_ref(), entry.translation.as_ref()))
            .collect::<BTreeMap<_, _>>();
        let mut aggregate = BTreeMap::<Box<str>, ProbeEntryRow>::new();
        let observations = self.read_observations(document.summary.id()).ok();
        if let Some(observations) = &observations {
            for entry in observations.entries() {
                let row = aggregate
                    .entry(entry.source().into())
                    .or_insert_with(|| ProbeEntryRow {
                        source: entry.source().into(),
                        translation: "".into(),
                        state: ProbeEntryState::Pending,
                        adapter_ids: Vec::new(),
                        count: 0,
                        first_seen_ms: entry.first_seen_ms(),
                        last_seen_ms: entry.last_seen_ms(),
                        translation_variants: Vec::new(),
                    });
                row.adapter_ids.push(entry.adapter_id().into());
                row.count = row.count.saturating_add(entry.count());
                row.first_seen_ms = row.first_seen_ms.min(entry.first_seen_ms());
                row.last_seen_ms = row.last_seen_ms.max(entry.last_seen_ms());
            }
        }
        let priority = document
            .summary
            .adapter_ids
            .iter()
            .enumerate()
            .map(|(index, adapter_id)| (adapter_id.as_ref(), index))
            .collect::<BTreeMap<_, _>>();
        for row in aggregate.values_mut() {
            row.adapter_ids.sort_by(|left, right| {
                priority
                    .get(left.as_ref())
                    .copied()
                    .unwrap_or(usize::MAX)
                    .cmp(&priority.get(right.as_ref()).copied().unwrap_or(usize::MAX))
                    .then_with(|| left.cmp(right))
            });
            row.adapter_ids.dedup();
            if let Some(translation) = translations.get(row.source.as_ref()) {
                row.translation = (*translation).into();
                row.state = ProbeEntryState::Translated;
            }
            if ignored.contains(&row.source) {
                row.state = ProbeEntryState::Ignored;
            }
        }
        for entry in &dictionary.entries {
            aggregate
                .entry(entry.source.clone())
                .or_insert_with(|| ProbeEntryRow {
                    source: entry.source.clone(),
                    translation: entry.translation.clone(),
                    state: ProbeEntryState::Unobserved,
                    adapter_ids: Vec::new(),
                    count: 0,
                    first_seen_ms: 0,
                    last_seen_ms: 0,
                    translation_variants: Vec::new(),
                });
        }
        let keys = self.source_keys(document, observations.as_ref());
        if keys.common == glyphshift_domain::SourceTextPolicy::Exact && keys.normalized.is_empty() { return Ok(aggregate.into_values().collect()); }
        let mut grouped = BTreeMap::<Box<str>, ProbeEntryRow>::new();
        for mut row in aggregate.into_values() {
            row.source = keys.key(&row.source).into();
            let key = row.source.clone();
            grouped.entry(key).and_modify(|previous| {
                if row.count != 0 {
                    previous.first_seen_ms = if previous.count == 0 { row.first_seen_ms } else { previous.first_seen_ms.min(row.first_seen_ms) };
                    previous.last_seen_ms = previous.last_seen_ms.max(row.last_seen_ms);
                    previous.count = previous.count.saturating_add(row.count);
                    previous.adapter_ids.extend(row.adapter_ids.iter().cloned());
                    previous.adapter_ids.sort(); previous.adapter_ids.dedup();
                }
            }).or_insert(row);
        }
        let mut dictionary_groups = BTreeMap::<String, Vec<&ProbeDictionaryEntry>>::new();
        for entry in &dictionary.entries { dictionary_groups.entry(keys.key(&entry.source)).or_default().push(entry); }
        let ignored = ignored.iter().map(|source| keys.key(source)).collect::<BTreeSet<_>>();
        for row in grouped.values_mut() {
            let candidates = dictionary_groups.get(row.source.as_ref()).cloned().unwrap_or_default();
            let explicit = candidates.iter().find(|entry| entry.source == row.source);
            let translations = candidates.iter().map(|entry| entry.translation.as_ref()).collect::<BTreeSet<_>>();
            row.translation = if let Some(entry) = explicit { entry.translation.clone() }
                else if translations.len() == 1 { (*translations.first().unwrap()).into() } else { "".into() };
            row.translation_variants = if explicit.is_none() && translations.len() > 1 {
                candidates.into_iter().map(|entry| (entry.translation.as_ref(), entry)).collect::<BTreeMap<_, _>>().into_values().cloned().collect()
            } else { Vec::new() };
            row.state = if ignored.contains(row.source.as_ref()) { ProbeEntryState::Ignored }
                else if row.count == 0 { ProbeEntryState::Unobserved }
                else if row.translation.is_empty() { ProbeEntryState::Pending } else { ProbeEntryState::Translated };
        }
        Ok(grouped.into_values().collect())
    }

    fn recover_disconnected(&mut self) -> Result<(), ProbeRunError> {
        let ids = fs::read_dir(&self.root)
            .map_err(|_| ProbeRunError::Storage)?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .filter_map(|entry| entry.file_name().to_str().map(ToOwned::to_owned))
            .filter(|id| safe_identifier(id))
            .collect::<Vec<_>>();
        for id in ids {
            let Ok(mut document) = self.read_document(&id) else {
                continue;
            };
            if matches!(
                document.summary.status,
                ProbeRunStatus::Running | ProbeRunStatus::Paused | ProbeRunStatus::Interrupted
            ) {
                document.summary.status = ProbeRunStatus::Ready;
                self.touch(&mut document);
                self.write_document(&document)?;
            }
        }
        Ok(())
    }

    fn synchronize(&mut self, run_id: &str) -> Result<ProbeRunSummary, ProbeRunError> {
        Ok(self.synchronized_document(run_id)?.summary)
    }

    fn synchronized_document(&self, run_id: &str) -> Result<ProbeRunDocument, ProbeRunError> {
        let mut document = self.read_document(run_id)?;
        let Ok(observations) = self.read_observations(run_id) else {
            return Ok(document);
        };
        let keys = self.source_keys(&document, Some(&observations));
        document.ignored_sources = document.ignored_sources.iter().map(|source| keys.key(source).into()).collect::<BTreeSet<Box<str>>>().into_iter().collect();
        let observed_count = observations.entries().iter().map(|entry| entry.source()).collect::<BTreeSet<_>>().len();
        if observations.revision() <= document.catalog_revision && observed_count == document.summary.observed_count && document.ignored_sources.len() == document.summary.ignored_count {
            return Ok(document);
        }
        document.catalog_revision = observations.revision();
        document.summary.observation_revision =
            document.summary.observation_revision.saturating_add(1);
        document.summary.observed_count = observations
            .entries()
            .iter()
            .map(|entry| entry.source())
            .collect::<BTreeSet<_>>()
            .len();
        document.summary.dropped_observations = observations.dropped_observations();
        let observed = observations
            .entries()
            .iter()
            .map(|entry| entry.source())
            .collect::<BTreeSet<_>>();
        document
            .ignored_sources
            .retain(|source| observed.contains(source.as_ref()));
        document.summary.ignored_count = document.ignored_sources.len();
        self.touch(&mut document);
        self.write_document(&document)?;
        Ok(document)
    }

    fn touch(&self, document: &mut ProbeRunDocument) {
        document.storage_revision = document.storage_revision.saturating_add(1);
        document.summary.updated_at_ms = unix_time_millis();
    }

    fn run_directory(&self, run_id: &str) -> PathBuf {
        self.root.join(run_id)
    }

    fn observation_path(&self, run_id: &str) -> PathBuf {
        self.run_directory(run_id).join("observations.json")
    }

    fn document_paths(&self, run_id: &str) -> [PathBuf; 2] {
        let directory = self.run_directory(run_id);
        [directory.join("run.a.json"), directory.join("run.b.json")]
    }

    fn read_observations(&self, run_id: &str) -> Result<CaptureCatalog, ProbeRunError> {
        CaptureCatalog::read_current(&self.observation_path(run_id))
            .map(|catalog| catalog.map_sources(|adapter, source| self.source_policies.get(adapter).copied().unwrap_or_default().key(source)))
            .map_err(|_| ProbeRunError::Observation)
    }

    fn read_document(&self, run_id: &str) -> Result<ProbeRunDocument, ProbeRunError> {
        if !safe_identifier(run_id) {
            return Err(ProbeRunError::InvalidInput);
        }
        self.document_paths(run_id)
            .iter()
            .filter_map(|path| fs::read_to_string(path).ok())
            .filter_map(|source| serde_json::from_str::<serde_json::Value>(&source).ok())
            .filter_map(|value| probe_document_from_value(&value))
            .filter(|document| {
                document.schema.as_ref() == PROBE_RUN_SCHEMA
                    && document.summary.id.as_ref() == run_id
            })
            .max_by_key(|document| document.storage_revision)
            .ok_or(ProbeRunError::NotFound)
    }

    fn write_document(&self, document: &ProbeRunDocument) -> Result<(), ProbeRunError> {
        let paths = self.document_paths(document.summary.id());
        let destination = &paths[(document.storage_revision % 2) as usize];
        let pending = destination.with_extension("pending");
        let source = serde_json::to_vec(document).map_err(|_| ProbeRunError::Storage)?;
        fs::write(&pending, source).map_err(|_| ProbeRunError::Storage)?;
        if destination.exists() {
            fs::remove_file(destination).map_err(|_| ProbeRunError::Storage)?;
        }
        fs::rename(pending, destination).map_err(|_| ProbeRunError::Storage)
    }
}

fn probe_document_from_value(value: &serde_json::Value) -> Option<ProbeRunDocument> {
    let object = value.as_object()?;
    let summary = object.get("summary")?.as_object()?;
    let id = summary.get("id")?.as_str()?;
    let software_id = summary.get("softwareId")?.as_str()?;
    let dictionary_id = summary.get("dictionaryId")?.as_str()?;
    let adapter_ids = summary
        .get("adapterIds")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|adapter| adapter.as_str().map(Into::into))
        .collect::<Vec<Box<str>>>();
    if adapter_ids.is_empty() {
        return None;
    }
    let status = summary
        .get("status")
        .and_then(|status| serde_json::from_value(status.clone()).ok())
        .unwrap_or(ProbeRunStatus::Ready);
    let number = |key: &str| {
        summary
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    };
    let created_at_ms = number("createdAtMs");
    Some(ProbeRunDocument {
        schema: PROBE_RUN_SCHEMA.into(),
        storage_revision: object
            .get("storageRevision")
            .and_then(serde_json::Value::as_u64)
            .filter(|revision| *revision > 0)
            .unwrap_or(1),
        catalog_revision: object
            .get("catalogRevision")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        summary: ProbeRunSummary {
            id: id.into(),
            name: summary
                .get("name")
                .and_then(serde_json::Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .unwrap_or(id)
                .into(),
            software_id: software_id.into(),
            dictionary_id: dictionary_id.into(),
            adapter_ids,
            status,
            live_preview_enabled: summary
                .get("livePreviewEnabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            observation_revision: number("observationRevision"),
            observed_count: usize::try_from(number("observedCount")).unwrap_or(0),
            ignored_count: usize::try_from(number("ignoredCount")).unwrap_or(0),
            dropped_observations: number("droppedObservations"),
            preview_generation: number("previewGeneration"),
            created_at_ms,
            updated_at_ms: number("updatedAtMs").max(created_at_ms),
        },
        ignored_sources: object
            .get("ignoredSources")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|source| source.as_str().map(Into::into))
            .collect(),
    })
}

fn paged<T>(rows: Vec<T>, query: &ProbeQuery) -> Vec<T> {
    let start = (query.page - 1).saturating_mul(query.page_size);
    rows.into_iter().skip(start).take(query.page_size).collect()
}

fn state_name(state: ProbeEntryState) -> &'static str {
    match state {
        ProbeEntryState::Pending => "pending",
        ProbeEntryState::Translated => "translated",
        ProbeEntryState::Unobserved => "unobserved",
        ProbeEntryState::Ignored => "ignored",
    }
}

fn push_csv_row<const N: usize>(output: &mut String, fields: [String; N]) {
    for (index, field) in fields.into_iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push('"');
        output.push_str(&field.replace('"', "\"\""));
        output.push('"');
    }
    output.push_str("\r\n");
}

impl From<CaptureError> for ProbeRunError {
    fn from(_: CaptureError) -> Self {
        Self::Observation
    }
}

#[cfg(test)]
#[path = "workspace/tests/mod.rs"]
mod tests;
