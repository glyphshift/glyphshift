use super::*;

impl DesktopApplication {
    pub(crate) fn probe_run_summary(&mut self, run_id: &str) -> Result<ProbeRunView, CommandError> {
        let summary = self.probe_runs.summary(run_id).map_err(probe_run_error)?;
        self.probe_run_view(summary)
    }

    pub(crate) fn probe_run_entries(
        &mut self,
        request: ProbeRunQueryRequest,
    ) -> Result<ProbeEntryPage, CommandError> {
        self.probe_run_entries_visible(request, |_| true)
    }

    fn probe_run_entries_visible(
        &mut self,
        request: ProbeRunQueryRequest,
        visible: impl FnMut(&str) -> bool,
    ) -> Result<ProbeEntryPage, CommandError> {
        self.probe_run_entries_resolved(request, visible, |_| None)
    }

    pub(super) fn probe_run_entries_resolved(
        &mut self,
        request: ProbeRunQueryRequest,
        visible: impl FnMut(&str) -> bool,
        mut filtered: impl FnMut(&str) -> Option<glyphshift_ai_translation::SkipReason>,
    ) -> Result<ProbeEntryPage, CommandError> {
        let query = ProbeQuery::new(request.search, request.page, request.page_size)
            .and_then(|query| query.with_adapter_ids(request.adapter_ids))
            .map(|query| query.with_translation_filter(request.translation_filter))
            .map_err(probe_run_error)?;
        let summary = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        let dictionary = self.probe_entries_snapshot(&summary)?;
        let resolver = crate::entry_resolution::EntryResolver::new(&self.backend, &summary);
        // Display observed membership across attached dictionaries; collection exclusion is unchanged.
        let display_dictionary = dictionary
            .as_ref()
            .clone()
            .with_excluded_sources(BTreeSet::new());
        self.probe_runs
            .query_entries_grouped(
                &request.run_id,
                &query,
                &display_dictionary,
                visible,
                |row| {
                    let skipped = filtered(row.source());
                    let mut row = row.clone();
                    resolver.project(&mut row, skipped);
                    std::borrow::Cow::Owned(row)
                },
                |row| {
                    if request.merge_rules.unwrap_or(true)
                        && row.state() != glyphshift_capture::ProbeEntryState::Ignored
                    {
                        resolver.rule_group_key(row.source())
                    } else {
                        None
                    }
                },
            )
            .map_err(probe_run_error)
    }

    pub(crate) fn edit_probe_translation(
        &mut self,
        mut request: ProbeTranslationEditRequest,
    ) -> Result<ProbeRunView, CommandError> {
        let summary = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        let rule_source = crate::entry_resolution::EntryResolver::new(&self.backend, &summary)
            .editable_rule_source(&request.source)
            .map_err(|_| CommandError::new("capture.source_owned_by_dictionary"))?;
        let rule_matched = rule_source.is_some();
        if let Some(source) = rule_source {
            request.source = source;
        }
        self.ensure_ai_dictionary_writable(summary.dictionary_id())?;
        let dictionary = self
            .backend
            .dictionary(summary.dictionary_id())
            .cloned()
            .map_err(|_| CommandError::new("dictionary.not_found"))?;
        let existing = dictionary
            .entries()
            .iter()
            .find(|entry| entry.source() == request.source.as_ref());
        if existing.is_none()
            && !rule_matched
            && !self
                .probe_runs
                .excluded_sources_for(
                    &request.run_id,
                    self.probe_entries_snapshot(&summary)?.as_ref(),
                    &[request.source.clone()],
                )
                .map_err(probe_run_error)?
                .is_empty()
        {
            return Err(CommandError::new("capture.source_owned_by_dictionary"));
        }
        let translation = request.translation.trim();
        let changed = if translation.is_empty() {
            let snapshot = self.probe_dictionary_snapshot(dictionary.id())?;
            let mut sources = self
                .probe_runs
                .dictionary_sources_for_rows(&request.run_id, &[request.source.clone()], &snapshot)
                .map_err(probe_run_error)?;
            if existing.is_some() && !sources.contains(&request.source) {
                sources.push(request.source.clone());
            }
            if !sources.is_empty() {
                self.backend
                    .delete_dictionary_entries(dictionary.id(), sources, dictionary.revision())
                    .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
                true
            } else {
                false
            }
        } else if existing.is_some_and(|entry| entry.translation() == translation) {
            false
        } else {
            self.backend
                .upsert_dictionary_entry(
                    dictionary.id(),
                    DictionaryEntryCreate::new(request.source.clone(), translation),
                    existing.map(|entry| entry.source()),
                    dictionary.revision(),
                )
                .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
            true
        };
        if changed {
            self.reconcile_enabled_workflows()?;
        }
        self.publish_probe_preview_if_active(&request.run_id)?;
        self.probe_run_summary(&request.run_id)
    }

    pub(crate) fn sync_probe_dictionary_entries(
        &mut self,
        request: ProbeDictionarySyncRequest,
    ) -> Result<ProbeRunView, CommandError> {
        if request.entries.is_empty() {
            return Err(CommandError::new("capture.invalid_configuration"));
        }
        let summary = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        self.ensure_ai_dictionary_writable(summary.dictionary_id())?;
        let dictionary = self
            .backend
            .dictionary(summary.dictionary_id())
            .cloned()
            .map_err(|_| CommandError::new("dictionary.not_found"))?;
        let requested_sources = request
            .entries
            .iter()
            .map(|entry| Box::<str>::from(entry.source.trim()))
            .collect::<Vec<_>>();
        let excluded = self
            .probe_runs
            .excluded_sources_for(
                &request.run_id,
                self.probe_entries_snapshot(&summary)?.as_ref(),
                &requested_sources,
            )
            .map_err(probe_run_error)?;
        if excluded.iter().any(|source| {
            !dictionary
                .entries()
                .iter()
                .any(|entry| entry.source() == source.as_ref())
        }) {
            return Err(CommandError::new("capture.source_owned_by_dictionary"));
        }
        let mut sources = BTreeSet::new();
        let mut entries = Vec::with_capacity(request.entries.len());
        for entry in request.entries {
            let source = entry.source.trim();
            let translation = entry.translation.trim();
            if source.is_empty()
                || translation.is_empty()
                || !sources.insert(Box::<str>::from(source))
            {
                return Err(CommandError::new("capture.invalid_configuration"));
            }
            entries.push(DictionaryEntryCreate::new(source, translation));
        }
        let updated_dictionary = self
            .backend
            .upsert_dictionary_entries(dictionary.id(), entries, dictionary.revision())
            .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
        if updated_dictionary.revision() != dictionary.revision() {
            self.reconcile_enabled_workflows()?;
            self.publish_probe_preview_if_active(&request.run_id)?;
        }
        self.probe_run_summary(&request.run_id)
    }

    pub(crate) fn bulk_probe_entries(
        &mut self,
        request: ProbeBulkRequest,
    ) -> Result<ProbeRunView, CommandError> {
        match request.action.as_ref() {
            "ignore" => {
                self.probe_runs
                    .set_ignored(&request.run_id, &request.sources, true)
                    .map_err(probe_run_error)?;
            }
            "restore" => {
                self.probe_runs
                    .set_ignored(&request.run_id, &request.sources, false)
                    .map_err(probe_run_error)?;
            }
            "clear_translations" => {
                let summary = self
                    .probe_runs
                    .summary(&request.run_id)
                    .map_err(probe_run_error)?;
                self.ensure_ai_dictionary_writable(summary.dictionary_id())?;
                let dictionary = self
                    .backend
                    .dictionary(summary.dictionary_id())
                    .cloned()
                    .map_err(|_| CommandError::new("dictionary.not_found"))?;
                let existing = dictionary
                    .entries()
                    .iter()
                    .map(|entry| entry.source())
                    .collect::<BTreeSet<_>>();
                let resolver = crate::entry_resolution::EntryResolver::new(&self.backend, &summary);
                let mut raw_sources = Vec::new();
                let mut rule_sources = Vec::new();
                for source in &request.sources {
                    match resolver.editable_rule_source(source) {
                        Ok(Some(key)) => {
                            if existing.contains(key.as_ref()) {
                                rule_sources.push(key);
                            }
                        }
                        Ok(None) => raw_sources.push(source.clone()),
                        Err(()) => {
                            return Err(CommandError::new("capture.source_owned_by_dictionary"))
                        }
                    }
                }
                let snapshot = self.probe_dictionary_snapshot(dictionary.id())?;
                let mut sources = self
                    .probe_runs
                    .dictionary_sources_for_rows(&request.run_id, &raw_sources, &snapshot)
                    .map_err(probe_run_error)?;
                sources.extend(
                    raw_sources
                        .iter()
                        .filter(|source| existing.contains(source.as_ref()))
                        .cloned()
                        .collect::<Vec<_>>(),
                );
                sources.extend(rule_sources);
                sources.sort();
                sources.dedup();
                if !sources.is_empty() {
                    self.backend
                        .delete_dictionary_entries(dictionary.id(), sources, dictionary.revision())
                        .map_err(|_| CommandError::new("dictionary.invalid_update"))?;
                    self.reconcile_enabled_workflows()?;
                }
            }
            _ => return Err(CommandError::new("capture.invalid_configuration")),
        }
        self.publish_probe_preview_if_active(&request.run_id)?;
        self.probe_run_summary(&request.run_id)
    }

    pub(crate) fn export_probe_run(
        &mut self,
        request: ProbeExportRequest,
    ) -> Result<(), CommandError> {
        if !request.output_path.is_absolute() {
            return Err(CommandError::new("capture.export_failed"));
        }
        let summary = self
            .probe_runs
            .summary(&request.run_id)
            .map_err(probe_run_error)?;
        let content = if request.format == ProbeExportFormat::DictionaryJson {
            self.backend
                .dictionary_json(summary.dictionary_id())
                .map_err(|_| CommandError::new("capture.export_failed"))?
        } else {
            let dictionary = self.probe_entries_snapshot(&summary)?;
            self.probe_runs
                .export(&request.run_id, request.format, &dictionary)
                .map_err(probe_run_error)?
        };
        std::fs::write(request.output_path, content)
            .map_err(|_| CommandError::new("capture.export_failed"))
    }
}
