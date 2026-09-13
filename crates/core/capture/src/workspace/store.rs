use super::super::{
    safe_identifier, unix_time_millis, CaptureCatalog, CaptureConfiguration, CaptureSessionId,
};
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

impl ProbeRunStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, ProbeRunError> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(ProbeRunError::InvalidInput);
        }
        fs::create_dir_all(&root).map_err(|_| ProbeRunError::Storage)?;
        let mut store = Self {
            root,
            source_policies: BTreeMap::new(),
            cache: RefCell::new(ReadCache::default()),
        };
        store.recover_disconnected()?;
        Ok(store)
    }

    pub fn set_source_policy(
        &mut self,
        adapter_id: impl Into<Box<str>>,
        policy: glyphshift_domain::SourceTextPolicy,
    ) {
        self.source_policies.insert(adapter_id.into(), policy);
        *self.cache.get_mut() = ReadCache::default();
    }

    fn run_policy(&self, document: &ProbeRunDocument) -> glyphshift_domain::SourceTextPolicy {
        let policies = document
            .summary
            .adapter_ids
            .iter()
            .map(|id| self.source_policies.get(id).copied().unwrap_or_default())
            .collect::<BTreeSet<_>>();
        if policies.len() == 1 {
            *policies.first().unwrap()
        } else {
            glyphshift_domain::SourceTextPolicy::Exact
        }
    }

    pub(super) fn source_keys(
        &self,
        document: &ProbeRunDocument,
        observations: Option<&CaptureCatalog>,
    ) -> ProbeSourceKeys {
        use glyphshift_domain::SourceTextPolicy;
        let mut keys = ProbeSourceKeys {
            common: self.run_policy(document),
            normalized: BTreeMap::new(),
            exact: BTreeSet::new(),
        };
        if let Some(observations) = observations {
            for entry in observations.entries() {
                let policy = self
                    .source_policies
                    .get(entry.adapter_id())
                    .copied()
                    .unwrap_or_default();
                if policy == SourceTextPolicy::Exact {
                    keys.exact.insert(entry.source().to_owned());
                } else {
                    keys.normalized
                        .entry(policy)
                        .or_default()
                        .insert(entry.source().to_owned());
                }
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
                workflow_id: create.workflow_id,
                name: create.name,
                software_id: create.software_id,
                dictionary_id: create.dictionary_id,
                excluded_dictionary_ids: create.excluded_dictionary_ids,
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
        let configuration_changed = document.summary.excluded_dictionary_ids
            != update.excluded_dictionary_ids
            || document.summary.adapter_ids != update.adapter_ids
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
        if document.summary.excluded_dictionary_ids == update.excluded_dictionary_ids
            && document.summary.name == update.name
            && document.summary.dictionary_id == update.dictionary_id
            && document.summary.adapter_ids == update.adapter_ids
            && document.summary.live_preview_enabled == update.live_preview_enabled
        {
            return Ok(document.summary);
        }
        document.summary.name = update.name;
        document.summary.dictionary_id = update.dictionary_id;
        document.summary.excluded_dictionary_ids = update.excluded_dictionary_ids;
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

    pub fn attach_workflow(
        &mut self,
        run_id: &str,
        workflow_id: &str,
    ) -> Result<(), ProbeRunError> {
        if !safe_identifier(workflow_id) {
            return Err(ProbeRunError::InvalidInput);
        }
        let mut document = self.read_document(run_id)?;
        if document.summary.workflow_id.as_deref() == Some(workflow_id) {
            return Ok(());
        }
        if document.summary.workflow_id.is_some()
            || matches!(
                document.summary.status,
                ProbeRunStatus::Running | ProbeRunStatus::Paused
            )
        {
            return Err(ProbeRunError::InvalidState);
        }
        document.summary.workflow_id = Some(workflow_id.into());
        self.touch(&mut document);
        self.write_document(&document)
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

    /// Uses the same source normalization as the collection table when checking writes.

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

    pub(super) fn synchronized_document(
        &self,
        run_id: &str,
    ) -> Result<ProbeRunDocument, ProbeRunError> {
        let mut document = self.read_document(run_id)?;
        let Ok(observations) = self.read_observations(run_id) else {
            return Ok(document);
        };
        if self.cache.borrow().synchronized.as_ref() == Some(&document) {
            return Ok(document);
        }
        let keys = self.source_keys(&document, Some(&observations));
        document.ignored_sources = document
            .ignored_sources
            .iter()
            .map(|source| keys.key(source).into())
            .collect::<BTreeSet<Box<str>>>()
            .into_iter()
            .collect();
        let observed_count = observations
            .entries()
            .iter()
            .map(|entry| entry.source())
            .collect::<BTreeSet<_>>()
            .len();
        if observations.revision() <= document.catalog_revision
            && observed_count == document.summary.observed_count
            && document.ignored_sources.len() == document.summary.ignored_count
        {
            self.cache.borrow_mut().synchronized = Some(document.clone());
            return Ok(document);
        }
        document.catalog_revision = observations.revision();
        document.summary.observation_revision =
            document.summary.observation_revision.saturating_add(1);
        document.summary.observed_count = observed_count;
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
        self.cache.borrow_mut().synchronized = Some(document.clone());
        Ok(document)
    }

    pub(super) fn touch(&self, document: &mut ProbeRunDocument) {
        document.storage_revision = document.storage_revision.saturating_add(1);
        document.summary.updated_at_ms = unix_time_millis();
    }

    fn run_directory(&self, run_id: &str) -> PathBuf {
        self.root.join(run_id)
    }

    pub(super) fn observation_path(&self, run_id: &str) -> PathBuf {
        self.run_directory(run_id).join("observations.json")
    }

    pub(super) fn document_paths(&self, run_id: &str) -> [PathBuf; 2] {
        let directory = self.run_directory(run_id);
        [directory.join("run.a.json"), directory.join("run.b.json")]
    }

    pub(super) fn read_observations(
        &self,
        run_id: &str,
    ) -> Result<Arc<CaptureCatalog>, ProbeRunError> {
        if !safe_identifier(run_id) {
            return Err(ProbeRunError::InvalidInput);
        }
        let path = self.observation_path(run_id);
        // Compare actual bytes, not mtime/size: rapid equal-length rewrites must be visible.
        let sources = [path.with_extension("a.json"), path.with_extension("b.json")]
            .map(|path| fs::read_to_string(path).ok());
        let mut cache = self.cache.borrow_mut();
        if cache.run_id != run_id {
            *cache = ReadCache {
                run_id: run_id.to_owned(),
                ..ReadCache::default()
            };
        }
        if cache.sources != sources {
            cache.catalog = sources
                .iter()
                .flatten()
                .filter_map(|source| CaptureCatalog::decode_json(source).ok())
                .max_by_key(CaptureCatalog::revision)
                .map(|catalog| {
                    Arc::new(catalog.map_sources(|adapter, source| {
                        self.source_policies
                            .get(adapter)
                            .copied()
                            .unwrap_or_default()
                            .key(source)
                    }))
                });
            cache.sources = sources;
            cache.synchronized = None;
            cache.rows = None;
            cache.decodes += 1;
        }
        cache.catalog.clone().ok_or(ProbeRunError::Observation)
    }

    pub(super) fn read_document(&self, run_id: &str) -> Result<ProbeRunDocument, ProbeRunError> {
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

    pub(super) fn write_document(&self, document: &ProbeRunDocument) -> Result<(), ProbeRunError> {
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
            workflow_id: summary
                .get("workflowId")
                .and_then(serde_json::Value::as_str)
                .filter(|id| safe_identifier(id))
                .map(Into::into),
            excluded_dictionary_ids: summary
                .get("excludedDictionaryIds")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|id| id.as_str())
                .filter(|id| safe_identifier(id) && *id != dictionary_id)
                .map(Into::into)
                .collect(),
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
