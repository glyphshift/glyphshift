use glyphshift_capture::{ProbeEntryResolution, ProbeEntryRow, ProbeEntryState, ProbeRunSummary};
use glyphshift_desktop_backend::DesktopBackend;
use glyphshift_translation::RegexTranslationRules;
use std::{collections::BTreeMap, sync::Arc};

/// A read projection across the workflow's dictionaries. It never changes dictionary ownership.
pub(super) struct EntryResolver {
    entries: BTreeMap<String, Vec<(Box<str>, Box<str>)>>,
    writer: Box<str>,
    rules: Vec<(Box<str>, RegexTranslationRules)>,
}

impl EntryResolver {
    pub(super) fn new(backend: &DesktopBackend, summary: &ProbeRunSummary) -> Self {
        let ids = summary
            .workflow_id()
            .and_then(|id| backend.workflow(id).ok())
            .and_then(|workflow| {
                workflow
                    .targets()
                    .iter()
                    .find(|target| target.software_id() == summary.software_id())
                    .map(|target| target.dictionary_ids().to_vec())
            })
            .unwrap_or_else(|| {
                std::iter::once(summary.dictionary_id().into())
                    .chain(summary.excluded_dictionary_ids().iter().cloned())
                    .collect()
            });
        let mut entries: BTreeMap<String, Vec<(Box<str>, Box<str>)>> = BTreeMap::new();
        let mut rules = Vec::new();
        for id in ids {
            if let Ok(dictionary) = backend.dictionary(&id) {
                rules.push((id.clone(), RegexTranslationRules::compile(dictionary.metadata().text_rules().to_vec()).expect("validated rules")));
                for entry in dictionary.entries() {
                    entries
                        .entry(entry.source().to_owned())
                        .or_default()
                        .push((id.clone(), entry.translation().into()));
                }
            }
        }
        Self {
            entries,
            writer: summary.dictionary_id().into(),
            rules,
        }
    }

    pub(super) fn collection_sources(&self, source: &str) -> Vec<Box<str>> {
        for (id, rules) in &self.rules {
            if let Some(sources) = rules.collection_sources(source) {
                return if id == &self.writer { sources.into_iter().map(Into::into).collect() } else { Vec::new() };
            }
        }
        vec![source.into()]
    }

    pub(super) fn rule_group_key(&self, source: &str) -> Option<Vec<Box<str>>> {
        for (id, rules) in &self.rules {
            if let Some(index) = rules.matching_rule_index(source) {
                let sources = rules.collection_sources(source)?;
                if sources.is_empty() { return None; }
                return Some(std::iter::once(id.clone()).chain(std::iter::once(index.to_string().into_boxed_str()))
                    .chain(sources.into_iter().map(Into::into)).collect());
            }
        }
        None
    }

    pub(super) fn editable_rule_source(&self, source: &str) -> Result<Option<Box<str>>, ()> {
        for (id, rules) in &self.rules {
            if let Some(sources) = rules.collection_sources(source) {
                return if id == &self.writer && sources.len() == 1 {
                    Ok(Some(sources[0].as_str().into()))
                } else { Err(()) };
            }
        }
        Ok(None)
    }

    pub(super) fn project(
        &self,
        row: &mut ProbeEntryRow,
        skip_reason: Option<glyphshift_ai_translation::SkipReason>,
    ) {
        let filtered = skip_reason.is_some();
        let exact = self.entries.get(row.source());
        let mut dictionary_ids = exact
            .into_iter()
            .flatten()
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        let mut editable = dictionary_ids.is_empty() || dictionary_ids.contains(&self.writer);
        let mut kind = if filtered {
            "filtered"
        } else if exact.is_some() {
            "dictionary_pending"
        } else {
            "pending"
        };
        let mut rule_index = None;
        let mut edit_source = None;
        let mut edit_translation = None;
        let mut translation: Option<Arc<str>> = None;
        if row.state() == ProbeEntryState::Ignored {
            kind = "ignored";
        } else if let Some((id, rules, index)) = self.rules.iter().find_map(|(id, rules)| rules.matching_rule_index(row.source()).map(|index| (id, rules, index))) {
            rule_index = Some(index);
            edit_source = self.editable_rule_source(row.source()).ok().flatten();
            editable = edit_source.is_some();
            edit_translation = edit_source.as_ref().map(|source| self.entries.get(source.as_ref())
                .and_then(|entries| entries.iter().find(|(owner, _)| owner == id))
                .map(|(_, text)| text.clone()).unwrap_or_default());
            dictionary_ids = vec![id.clone()];
            translation = rules.replace(row.source(), |source| self.entries.get(source)?
                .iter().find(|(owner, text)| owner == id && !text.trim().is_empty())
                .map(|(_, text)| Arc::from(text.as_ref())));
            kind = match translation.as_deref() { Some("") => "rule_skipped", Some(_) => "rule_translated", None => "rule_pending" };
            // A matched rule owns this display row, including missing translations.
            if translation.is_none() { translation = Some(Arc::from("")); }
        } else if row.has_translation_conflict() {
            kind = "conflict";
        } else if let Some((_, text)) = exact.into_iter().flatten().find(|(_, text)| !text.trim().is_empty()) {
            kind = "dictionary"; translation = Some(Arc::from(text.as_ref()));
        } else if !row.translation().trim().is_empty() {
            kind = "dictionary";
        }

        row.set_resolution(
            ProbeEntryResolution {
                kind: kind.into(),
                skip_reason: skip_reason
                    .and_then(|reason| serde_json::to_value(reason).ok())
                    .and_then(|value| value.as_str().map(Into::into)),
                dictionary_ids,
                rule_index,
                editable,
                edit_source,
                edit_translation,
            },
            translation.as_deref(),
        );
    }
}
