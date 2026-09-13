use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

impl ProbeRunStore {
    pub fn excluded_sources_for(
        &self,
        run_id: &str,
        dictionary: &ProbeDictionarySnapshot,
        sources: &[Box<str>],
    ) -> Result<BTreeSet<Box<str>>, ProbeRunError> {
        let document = self.read_document(run_id)?;
        let observations = self.read_observations(run_id).ok();
        let keys = self.source_keys(&document, observations.as_deref());
        let excluded = dictionary
            .excluded_sources
            .iter()
            .map(|source| keys.key(source))
            .collect::<BTreeSet<_>>();
        Ok(sources
            .iter()
            .filter(|source| excluded.contains(&keys.key(source)))
            .cloned()
            .collect())
    }

    pub fn uncollected_sources(
        &mut self,
        run_id: &str,
        dictionary: &ProbeDictionarySnapshot,
    ) -> Result<Vec<Box<str>>, ProbeRunError> {
        self.uncollected_sources_mapped(run_id, dictionary, |source| vec![source.into()])
    }

    pub fn uncollected_sources_mapped(
        &mut self,
        run_id: &str,
        dictionary: &ProbeDictionarySnapshot,
        mut map: impl FnMut(&str) -> Vec<Box<str>>,
    ) -> Result<Vec<Box<str>>, ProbeRunError> {
        let document = self.synchronized_document(run_id)?;
        let observations = self.read_observations(run_id).ok();
        let keys = self.source_keys(&document, observations.as_deref());
        let existing = dictionary
            .entries
            .iter()
            .map(|entry| keys.key(&entry.source))
            .collect::<BTreeSet<_>>();
        let excluded = dictionary
            .excluded_sources
            .iter()
            .map(|source| keys.key(source))
            .collect::<BTreeSet<_>>();
        let ignored = document
            .ignored_sources
            .iter()
            .map(|source| keys.key(source))
            .collect::<BTreeSet<_>>();
        // Collection needs source membership only; don't build display rows, translations or adapter lists.
        Ok(observations
            .iter()
            .flat_map(|catalog| catalog.entries())
            .map(|entry| keys.key(entry.source()))
            .filter(|source| !ignored.contains(source))
            .flat_map(|source| map(&source))
            .map(|source| keys.key(&source))
            .filter(|source| {
                !existing.contains(source)
                    && !excluded.contains(source)
                    && !ignored.contains(source)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn query_entries(
        &mut self,
        run_id: &str,
        query: &ProbeQuery,
        dictionary: &ProbeDictionarySnapshot,
    ) -> Result<ProbeEntryPage, ProbeRunError> {
        self.query_entries_visible(run_id, query, dictionary, |_| true)
    }

    pub fn query_entries_visible(
        &mut self,
        run_id: &str,
        query: &ProbeQuery,
        dictionary: &ProbeDictionarySnapshot,
        visible: impl FnMut(&str) -> bool,
    ) -> Result<ProbeEntryPage, ProbeRunError> {
        self.query_entries_projected(run_id, query, dictionary, visible, |row| {
            std::borrow::Cow::Borrowed(row)
        })
    }

    pub fn query_entries_projected(
        &mut self,
        run_id: &str,
        query: &ProbeQuery,
        dictionary: &ProbeDictionarySnapshot,
        visible: impl FnMut(&str) -> bool,
        project: impl for<'a> FnMut(&'a ProbeEntryRow) -> std::borrow::Cow<'a, ProbeEntryRow>,
    ) -> Result<ProbeEntryPage, ProbeRunError> {
        self.query_entries_grouped(run_id, query, dictionary, visible, project, |_| None)
    }

    pub fn query_entries_grouped(
        &mut self,
        run_id: &str,
        query: &ProbeQuery,
        dictionary: &ProbeDictionarySnapshot,
        mut visible: impl FnMut(&str) -> bool,
        mut project: impl for<'a> FnMut(&'a ProbeEntryRow) -> std::borrow::Cow<'a, ProbeEntryRow>,
        mut group: impl FnMut(&ProbeEntryRow) -> Option<Vec<Box<str>>>,
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
        let rows = self.cached_rows(&document, dictionary)?;
        let search_texts = self
            .cache
            .borrow()
            .rows
            .as_ref()
            .expect("rows populated by cached_rows")
            .search_texts
            .clone();
        let matching = rows
            .iter()
            .zip(search_texts.iter())
            .map(|(row, search)| (project(row), search))
            .filter(|(row, search)| {
                let matches_adapter = adapter_filter.is_empty()
                    || row
                        .adapter_ids
                        .iter()
                        .any(|adapter| adapter_filter.contains(adapter));
                let matches_search = needle.is_empty()
                    || search.iter().any(|text| text.contains(&needle))
                    || row.translation.to_lowercase().contains(&needle);
                let matches_translation = match query.translation_filter {
                    ProbeTranslationFilter::All => true,
                    ProbeTranslationFilter::Untranslated => row.translation.trim().is_empty(),
                    ProbeTranslationFilter::Translated => !row.translation.trim().is_empty(),
                    ProbeTranslationFilter::Skipped => {
                        row.state == ProbeEntryState::Ignored
                            || row.resolution.as_ref().is_some_and(|value| {
                                matches!(
                                    value.kind.as_ref(),
                                    "filtered" | "ignored" | "rule_skipped"
                                )
                            })
                    }
                    ProbeTranslationFilter::RuleMatched => row
                        .resolution
                        .as_ref()
                        .is_some_and(|value| value.rule_index.is_some()),
                    ProbeTranslationFilter::OtherDictionary => {
                        row.resolution.as_ref().is_some_and(|value| {
                            value
                                .dictionary_ids
                                .iter()
                                .any(|id| id != &document.summary.dictionary_id)
                        })
                    }
                };
                matches_adapter && matches_search && matches_translation && visible(&row.source)
            })
            .map(|(row, _)| row)
            .collect::<Vec<_>>();
        // Rows arrive newest first. Keep that representative and aggregate matching observations.
        let mut indexes = BTreeMap::<Vec<Box<str>>, usize>::new();
        let mut merged = Vec::<std::borrow::Cow<'_, ProbeEntryRow>>::new();
        for row in matching {
            if let Some(key) = group(&row) {
                if let Some(&index) = indexes.get(&key) {
                    let previous = merged[index].to_mut();
                    previous.count = previous.count.saturating_add(row.count);
                    previous.first_seen_ms = previous.first_seen_ms.min(row.first_seen_ms);
                    previous.adapter_ids.extend(row.adapter_ids.iter().cloned());
                    previous.adapter_ids.sort();
                    previous.adapter_ids.dedup();
                    continue;
                }
                indexes.insert(key, merged.len());
            }
            merged.push(row);
        }
        let total = merged.len();
        let rows = merged
            .into_iter()
            .skip(query.page.saturating_sub(1).saturating_mul(query.page_size))
            .take(query.page_size)
            .map(std::borrow::Cow::into_owned)
            .collect();
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
        let keys = self.source_keys(&document, observations.as_deref());
        let ignored = document
            .ignored_sources
            .into_iter()
            .collect::<BTreeSet<_>>();
        Ok(dictionary
            .entries
            .iter()
            .filter(|entry| {
                !entry.translation.trim().is_empty()
                    && !ignored.contains(&Box::<str>::from(keys.key(&entry.source)))
            })
            .map(|entry| PreviewEntry {
                source: entry.source.clone(),
                translation: entry.translation.clone(),
            })
            .collect())
    }

    /// Expands an explicit clear action on a projected row to its saved keys.
    /// Merely reading or selecting a translation never deletes these variants.
    pub fn dictionary_sources_for_rows(
        &self,
        run_id: &str,
        sources: &[Box<str>],
        dictionary: &ProbeDictionarySnapshot,
    ) -> Result<Vec<Box<str>>, ProbeRunError> {
        let document = self.read_document(run_id)?;
        let observations = self.read_observations(run_id).ok();
        let keys = self.source_keys(&document, observations.as_deref());
        let selected = sources
            .iter()
            .map(AsRef::as_ref)
            .collect::<BTreeSet<&str>>();
        Ok(dictionary
            .entries
            .iter()
            .filter(|entry| selected.contains(keys.key(&entry.source).as_str()))
            .map(|entry| entry.source.clone())
            .collect())
    }

    /// One immutable view for bulk consumers such as AI planning; no repeated page reads.
    pub fn entries_snapshot(
        &mut self,
        run_id: &str,
        dictionary: &ProbeDictionarySnapshot,
    ) -> Result<Arc<Vec<ProbeEntryRow>>, ProbeRunError> {
        let document = self.synchronized_document(run_id)?;
        self.cached_rows(&document, dictionary)
    }

    fn cached_rows(
        &self,
        document: &ProbeRunDocument,
        dictionary: &ProbeDictionarySnapshot,
    ) -> Result<Arc<Vec<ProbeEntryRow>>, ProbeRunError> {
        let cached = self
            .cache
            .borrow()
            .rows
            .as_ref()
            .filter(|cached| cached.document == *document && cached.dictionary == *dictionary)
            .map(|cached| cached.rows.clone());
        let rows = if let Some(rows) = cached {
            rows
        } else {
            let mut rows = self.combined_rows(document, dictionary)?;
            rows.sort_by(|left, right| {
                right
                    .last_seen_ms
                    .cmp(&left.last_seen_ms)
                    .then_with(|| left.source.cmp(&right.source))
            });
            let rows = Arc::new(rows);
            let mut cache = self.cache.borrow_mut();
            cache.row_builds += 1;
            let search_texts = Arc::new(
                rows.iter()
                    .map(|row| {
                        std::iter::once(row.source.to_lowercase())
                            .chain(std::iter::once(row.translation.to_lowercase()))
                            .chain(row.adapter_ids.iter().map(|adapter| adapter.to_lowercase()))
                            .collect()
                    })
                    .collect(),
            );
            cache.rows = Some(RowCache {
                document: document.clone(),
                dictionary: dictionary.clone(),
                rows: rows.clone(),
                search_texts,
            });
            rows
        };
        Ok(rows)
    }

    pub(super) fn combined_rows(
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
                        resolution: None,
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
                if !translation.trim().is_empty() {
                    row.state = ProbeEntryState::Translated;
                }
            }
            if ignored.contains(&row.source) {
                row.state = ProbeEntryState::Ignored;
            }
        }
        for entry in dictionary
            .entries
            .iter()
            .filter(|_| document.summary.workflow_id.is_none())
        {
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
                    resolution: None,
                });
        }
        let keys = self.source_keys(document, observations.as_deref());
        let excluded = dictionary
            .excluded_sources
            .iter()
            .map(|source| keys.key(source))
            .collect::<BTreeSet<_>>();
        aggregate.retain(|source, _| !excluded.contains(&keys.key(source)));
        if keys.common == glyphshift_domain::SourceTextPolicy::Exact && keys.normalized.is_empty() {
            return Ok(aggregate.into_values().collect());
        }
        let mut grouped = BTreeMap::<Box<str>, ProbeEntryRow>::new();
        for mut row in aggregate.into_values() {
            row.source = keys.key(&row.source).into();
            let key = row.source.clone();
            grouped
                .entry(key)
                .and_modify(|previous| {
                    if row.count != 0 {
                        previous.first_seen_ms = if previous.count == 0 {
                            row.first_seen_ms
                        } else {
                            previous.first_seen_ms.min(row.first_seen_ms)
                        };
                        previous.last_seen_ms = previous.last_seen_ms.max(row.last_seen_ms);
                        previous.count = previous.count.saturating_add(row.count);
                        previous.adapter_ids.extend(row.adapter_ids.iter().cloned());
                        previous.adapter_ids.sort();
                        previous.adapter_ids.dedup();
                    }
                })
                .or_insert(row);
        }
        let mut dictionary_groups = BTreeMap::<String, Vec<&ProbeDictionaryEntry>>::new();
        for entry in dictionary.entries.iter() {
            dictionary_groups
                .entry(keys.key(&entry.source))
                .or_default()
                .push(entry);
        }
        let ignored = ignored
            .iter()
            .map(|source| keys.key(source))
            .collect::<BTreeSet<_>>();
        for row in grouped.values_mut() {
            let candidates = dictionary_groups
                .get(row.source.as_ref())
                .cloned()
                .unwrap_or_default();
            let explicit = candidates.iter().find(|entry| entry.source == row.source);
            let translations = candidates
                .iter()
                .map(|entry| entry.translation.as_ref())
                .collect::<BTreeSet<_>>();
            row.translation = if let Some(entry) = explicit {
                entry.translation.clone()
            } else if translations.len() == 1 {
                (*translations.first().unwrap()).into()
            } else {
                "".into()
            };
            row.translation_variants = if explicit.is_none() && translations.len() > 1 {
                candidates
                    .into_iter()
                    .map(|entry| (entry.translation.as_ref(), entry))
                    .collect::<BTreeMap<_, _>>()
                    .into_values()
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
            row.state = if ignored.contains(row.source.as_ref()) {
                ProbeEntryState::Ignored
            } else if row.count == 0 {
                ProbeEntryState::Unobserved
            } else if row.translation.is_empty() {
                ProbeEntryState::Pending
            } else {
                ProbeEntryState::Translated
            };
        }
        Ok(grouped.into_values().collect())
    }
}
