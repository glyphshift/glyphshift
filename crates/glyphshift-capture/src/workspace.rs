use super::{
    safe_identifier, unix_time_millis, CaptureCatalog, CaptureConfiguration, CaptureError,
    CaptureSessionId,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

pub const PROBE_RUN_SCHEMA: &str = "glyphshift.probe-run/1";
const MAX_PAGE_SIZE: usize = 100;

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
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ProbeRunDocument {
    schema: Box<str>,
    storage_revision: u64,
    catalog_revision: u64,
    summary: ProbeRunSummary,
    ignored_sources: Vec<Box<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
    page: usize,
    page_size: usize,
}

impl ProbeQuery {
    pub fn new(
        search: impl Into<Box<str>>,
        page: usize,
        page_size: usize,
    ) -> Result<Self, ProbeRunError> {
        if page == 0 || page_size == 0 || page_size > MAX_PAGE_SIZE {
            return Err(ProbeRunError::InvalidInput);
        }
        Ok(Self {
            search: search.into(),
            adapter_ids: Vec::new(),
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
}

impl ProbeRunStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, ProbeRunError> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(ProbeRunError::InvalidInput);
        }
        fs::create_dir_all(&root).map_err(|_| ProbeRunError::Storage)?;
        let mut store = Self { root };
        store.recover_interrupted()?;
        Ok(store)
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
                matches_adapter && matches_search
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
        let ignored = document
            .ignored_sources
            .into_iter()
            .collect::<BTreeSet<_>>();
        Ok(dictionary
            .entries
            .iter()
            .filter(|entry| !ignored.contains(&entry.source))
            .map(|entry| PreviewEntry {
                source: entry.source.clone(),
                translation: entry.translation.clone(),
            })
            .collect())
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
        if let Ok(observations) = self.read_observations(document.summary.id()) {
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
                    });
                row.adapter_ids.push(entry.adapter_id().into());
                row.count = row.count.saturating_add(entry.count());
                row.first_seen_ms = row.first_seen_ms.min(entry.first_seen_ms());
                row.last_seen_ms = row.last_seen_ms.max(entry.last_seen_ms());
            }
        }
        for row in aggregate.values_mut() {
            row.adapter_ids.sort();
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
                });
        }
        Ok(aggregate.into_values().collect())
    }

    fn recover_interrupted(&mut self) -> Result<(), ProbeRunError> {
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
                ProbeRunStatus::Running | ProbeRunStatus::Paused
            ) {
                document.summary.status = ProbeRunStatus::Interrupted;
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
        if observations.revision() <= document.catalog_revision {
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
            .map_err(|_| ProbeRunError::Observation)
    }

    fn read_document(&self, run_id: &str) -> Result<ProbeRunDocument, ProbeRunError> {
        if !safe_identifier(run_id) {
            return Err(ProbeRunError::InvalidInput);
        }
        self.document_paths(run_id)
            .iter()
            .filter_map(|path| fs::read_to_string(path).ok())
            .filter_map(|source| serde_json::from_str::<ProbeRunDocument>(&source).ok())
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
mod tests {
    use super::*;
    use crate::FileCaptureSink;
    use tempfile::tempdir;

    fn run_store() -> (tempfile::TempDir, ProbeRunStore) {
        let root = tempdir().expect("probe root");
        let store = ProbeRunStore::open(root.path()).expect("open store");
        (root, store)
    }

    fn create_run(store: &mut ProbeRunStore) -> ProbeRunSummary {
        store
            .create(
                ProbeRunCreate::new(
                    "probe-one",
                    "UI probe",
                    "software-one",
                    "dictionary-one",
                    ["windows.gdi.text-out"],
                    true,
                )
                .expect("probe create"),
            )
            .expect("create run")
    }

    #[test]
    fn run_joins_observations_with_one_dictionary_without_persisting_translation_content() {
        let (root, mut store) = run_store();
        let summary = create_run(&mut store);
        let sink = FileCaptureSink::start(
            store
                .capture_configuration(summary.id(), 10_000)
                .expect("capture config"),
        )
        .expect("capture sink");
        for index in 0..5_000 {
            sink.observe("windows.gdi.text-out", format!("Source {index:04}"));
        }
        sink.finish().expect("finish capture");
        let dictionary = ProbeDictionarySnapshot::new(
            7,
            [
                ProbeDictionaryEntry::new("Source 4999", "译文"),
                ProbeDictionaryEntry::new("Imported", "已导入"),
            ],
        )
        .expect("dictionary snapshot");

        let page = store
            .query_entries(
                summary.id(),
                &ProbeQuery::new("", 1, 100).expect("query"),
                &dictionary,
            )
            .expect("joined page");
        assert_eq!(page.total, 5_001);
        assert_eq!(page.rows.len(), 100);
        assert_eq!(page.dictionary_revision, 7);
        let imported = store
            .query_entries(
                summary.id(),
                &ProbeQuery::new("Imported", 1, 20).expect("query"),
                &dictionary,
            )
            .expect("imported row");
        assert_eq!(imported.rows[0].state, ProbeEntryState::Unobserved);

        let document_text = fs::read_to_string(
            store
                .document_paths(summary.id())
                .into_iter()
                .find(|path| path.exists())
                .expect("run document"),
        )
        .expect("read run document");
        assert!(!document_text.contains("译文"));
        assert!(!document_text.contains("已导入"));
        assert!(root.path().exists());
    }

    #[test]
    fn run_recovers_interrupted_state_and_ignore_keeps_dictionary_unchanged() {
        let (root, mut store) = run_store();
        let summary = create_run(&mut store);
        let paused_summary = store
            .create(
                ProbeRunCreate::new(
                    "probe-paused",
                    "Paused probe",
                    "software-two",
                    "dictionary-two",
                    ["windows.gdi.text-out"],
                    false,
                )
                .expect("paused probe create"),
            )
            .expect("create paused run");
        let sink = FileCaptureSink::start(
            store
                .capture_configuration(summary.id(), 10)
                .expect("capture config"),
        )
        .expect("capture sink");
        sink.observe("windows.gdi.text-out", "Open");
        sink.finish().expect("finish capture");
        store
            .set_status(summary.id(), ProbeRunStatus::Running)
            .expect("mark running");
        store
            .set_status(paused_summary.id(), ProbeRunStatus::Paused)
            .expect("mark paused");
        store
            .set_ignored(summary.id(), &[Box::<str>::from("Open")], true)
            .expect("ignore observed source");
        let dictionary = ProbeDictionarySnapshot::new(
            3,
            [
                ProbeDictionaryEntry::new("Open", "打开"),
                ProbeDictionaryEntry::new("Save", "保存"),
            ],
        )
        .expect("dictionary snapshot");
        let preview = store
            .preview_entries(summary.id(), &dictionary)
            .expect("preview");
        assert_eq!(preview.len(), 1);
        assert_eq!(preview[0].source(), "Save");
        drop(store);

        let mut reopened = ProbeRunStore::open(root.path()).expect("reopen store");
        let recovered = reopened.summary(summary.id()).expect("summary");
        assert_eq!(recovered.status(), ProbeRunStatus::Interrupted);
        let recovered_paused = reopened
            .summary(paused_summary.id())
            .expect("paused summary");
        assert_eq!(recovered_paused.status(), ProbeRunStatus::Interrupted);
        let page = reopened
            .query_entries(
                summary.id(),
                &ProbeQuery::new("Open", 1, 20).expect("query"),
                &dictionary,
            )
            .expect("joined page");
        assert_eq!(page.rows[0].state, ProbeEntryState::Ignored);
        assert_eq!(page.rows[0].translation.as_ref(), "打开");
    }

    #[test]
    fn query_filters_joined_rows_by_run_adapter_without_changing_full_exports() {
        let (_root, mut store) = run_store();
        let summary = store
            .create(
                ProbeRunCreate::new(
                    "probe-filter",
                    "Filter probe",
                    "software-one",
                    "dictionary-one",
                    ["synthetic.adapter-one", "synthetic.adapter-two"],
                    false,
                )
                .expect("probe create"),
            )
            .expect("create run");
        let sink = FileCaptureSink::start(
            store
                .capture_configuration(summary.id(), 100)
                .expect("capture config"),
        )
        .expect("capture sink");
        sink.observe("synthetic.adapter-one", "Open");
        sink.observe("synthetic.adapter-one", "Open");
        sink.observe("synthetic.adapter-two", "Open");
        sink.observe("synthetic.adapter-two", "Save");
        sink.finish().expect("finish capture");
        let dictionary = ProbeDictionarySnapshot::new(
            2,
            [
                ProbeDictionaryEntry::new("Open", "打开"),
                ProbeDictionaryEntry::new("Imported", "已导入"),
            ],
        )
        .expect("dictionary snapshot");
        let dictionary_before = dictionary.clone();
        let observations_before =
            CaptureCatalog::read_current(&store.observation_path(summary.id()))
                .expect("observation index before filtering");

        let gdi_page = store
            .query_entries(
                summary.id(),
                &ProbeQuery::new("", 1, 20)
                    .expect("query")
                    .with_adapter_ids(["synthetic.adapter-one"])
                    .expect("adapter filter"),
                &dictionary,
            )
            .expect("filtered page");
        assert_eq!(gdi_page.total, 1);
        assert_eq!(gdi_page.rows[0].source.as_ref(), "Open");
        assert_eq!(gdi_page.rows[0].count, 3);

        let gdiplus_page = store
            .query_entries(
                summary.id(),
                &ProbeQuery::new("", 1, 20)
                    .expect("query")
                    .with_adapter_ids(["synthetic.adapter-two"])
                    .expect("adapter filter"),
                &dictionary,
            )
            .expect("filtered page");
        assert_eq!(gdiplus_page.total, 2);
        assert!(gdiplus_page
            .rows
            .iter()
            .all(|row| row.source.as_ref() != "Imported"));

        let unknown_filter = ProbeQuery::new("", 1, 20)
            .expect("query")
            .with_adapter_ids(["synthetic.adapter-unknown"])
            .expect("well-formed unknown adapter");
        assert_eq!(
            store.query_entries(summary.id(), &unknown_filter, &dictionary),
            Err(ProbeRunError::InvalidInput)
        );

        let exported = String::from_utf8(
            store
                .export(summary.id(), ProbeExportFormat::EntriesCsv, &dictionary)
                .expect("full joined export"),
        )
        .expect("utf-8 export");
        assert!(exported.contains("Imported"));
        assert!(exported.contains("Save"));
        assert_eq!(dictionary, dictionary_before);
        assert_eq!(
            CaptureCatalog::read_current(&store.observation_path(summary.id()))
                .expect("observation index after filtering"),
            observations_before
        );
    }
}
