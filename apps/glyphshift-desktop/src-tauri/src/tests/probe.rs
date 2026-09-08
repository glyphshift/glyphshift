use super::*;

#[test]
fn probe_preview_publish_failure_does_not_masquerade_as_a_stop_failure() {
    let error = serde_json::to_value(capture_preview_publish_error(
        DesktopRuntimeError::SessionRejected,
    ))
    .expect("serialize probe preview publish failure");

    assert_eq!(error["code"], "capture.preview_publish_failed");
}

#[test]
fn incompatible_probe_plan_is_rejected_before_its_draft_dictionary_is_created() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    let error = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-incompatible".into(),
            name: "Incompatible probe".into(),
            software_id,
            adapter_ids: vec!["test.incompatible".into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::New {
                id: "dictionary.incompatible-draft".into(),
                name: "Incompatible draft".into(),
                description: "".into(),
                source_locale: "en-US".into(),
                target_locale: "zh-CN".into(),
            },
        })
        .expect_err("incompatible plan must fail before persistence");

    assert_eq!(
        serde_json::to_value(error).expect("serialize incompatibility")["code"],
        "capture.unknown_adapter"
    );
    assert!(application
        .backend
        .dictionary("dictionary.incompatible-draft")
        .is_err());
}

#[test]
fn probe_reports_an_incompatible_runtime_bundle_before_adapter_availability() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    application.runtimes = None;
    application.runtime_bundle_error = Some(DesktopRuntimeError::AdapterAbiMismatch);

    let error = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-incompatible-runtime-bundle".into(),
            name: "Incompatible Runtime Bundle".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect_err("an ABI-mismatched Runtime Bundle must stop probe creation");

    assert_eq!(
        serde_json::to_value(error).expect("serialize Runtime Bundle error")["code"],
        "runtime.bundle_incompatible"
    );
}

#[test]
fn probe_translation_edit_publishes_the_next_live_preview_generation() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    application.adapters = vec![AdapterView {
        id: TEST_ADAPTER_ID.into(),
        name: "Replacement adapter".into(),
        version: "1.0.0".into(),
        summary: "Synthetic replacement adapter".into(),
        platforms: vec!["windows".into()],
        technologies: vec!["Synthetic".into()],
        features: vec!["textObserve".into(), "textReplace".into()],
        technical_target: "SyntheticReplace".into(),
        documentation_url: None,
        configuration: "none".into(),
        process_resident_after_deactivate: false,
    }];

    let created = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-live-preview".into(),
            name: "Live preview probe".into(),
            software_id: software_id.clone(),
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: true,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create live preview probe");
    assert_eq!(created.summary.preview_generation(), 1);

    let edited = application
        .edit_probe_translation(ProbeTranslationEditRequest {
            run_id: created.summary.id().into(),
            source: "Open".into(),
            translation: "立即打开".into(),
        })
        .expect("edit and publish live preview");
    assert_eq!(edited.summary.preview_generation(), 2);

    let refreshed = application
        .refresh_probe_text(created.summary.id())
        .expect("manual refresh");
    assert_eq!(refreshed.summary.preview_generation(), 3);
    assert_eq!(refreshed.summary.status(), ProbeRunStatus::Running);
    application
        .set_probe_run_paused(created.summary.id(), true)
        .expect("pause capture");
    let refreshed = application
        .refresh_probe_text(created.summary.id())
        .expect("refresh paused capture");
    assert_eq!(refreshed.summary.preview_generation(), 4);
    assert_eq!(refreshed.summary.status(), ProbeRunStatus::Paused);

    let calls = calls.lock().expect("runtime call log");
    assert_eq!(calls.capture_publications.len(), 4);
    assert!(calls
        .capture_publications
        .iter()
        .all(|(published_software, _)| published_software.as_ref() == software_id.as_ref()));
    assert_eq!(calls.capture_publications[0].1.generation().value(), 1);
    assert_eq!(calls.capture_publications[1].1.generation().value(), 2);
    let mut published_entries = Vec::new();
    calls.capture_publications[3]
        .1
        .snapshot()
        .visit_entries_with_adapters(|location, source, translation, adapters| {
            published_entries.push((
                location.to_owned(),
                source.to_owned(),
                translation.to_owned(),
                adapters
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>(),
            ));
        });
    assert_eq!(
        published_entries,
        vec![(
            "internal-default".to_owned(),
            "Open".to_owned(),
            "立即打开".to_owned(),
            vec![TEST_ADAPTER_ID.to_owned()],
        )]
    );
    assert_eq!(
        calls.captures_started.len(),
        1,
        "refresh keeps the same connection"
    );
    assert!(calls.captures_stopped.is_empty());
    drop(calls);
    application
        .disconnect_probe_run(created.summary.id())
        .expect("disconnect");
    let error = application
        .refresh_probe_text(created.summary.id())
        .expect_err("disconnected refresh must fail");
    assert_eq!(
        serde_json::to_value(error).unwrap()["code"],
        "capture.not_active"
    );
}

#[test]
fn probe_creation_waits_for_an_offline_target_instead_of_reporting_creation_failure() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    application.runtimes = Some(Box::new(RecordingWorkflowRuntime {
        calls,
        start_capture_error: Some(DesktopRuntimeError::UnknownTarget),
        capture_capability: ProbeRuntimeCapability::DirectReplace,
    }));

    let created = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-waiting-target".into(),
            name: "Waiting target probe".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("persist probe while target is offline");

    assert_eq!(created.summary.status(), ProbeRunStatus::Ready);
    assert!(application.active_probe_run_id.is_none());
    assert_eq!(application.probe_run_list().expect("list probes").len(), 1);

    let resume_error = application
        .resume_probe_run(created.summary.id())
        .expect_err("explicit reconnect should still explain the offline target");
    let error_json = serde_json::to_value(resume_error).expect("serialize reconnect error");
    assert_eq!(error_json["code"], "runtime.target_not_found");
}

#[test]
fn probe_reconnect_explains_when_a_workflow_owns_the_target() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    application.runtimes = Some(Box::new(RecordingWorkflowRuntime {
        calls: Arc::clone(&calls),
        start_capture_error: Some(DesktopRuntimeError::UnknownTarget),
        capture_capability: ProbeRuntimeCapability::DirectReplace,
    }));
    let created = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-workflow-conflict".into(),
            name: "Workflow conflict probe".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("persist disconnected probe");
    application.runtimes = Some(Box::new(RecordingWorkflowRuntime {
        calls,
        start_capture_error: Some(DesktopRuntimeError::TargetInUse(
            TargetExecutionOwner::Workflow,
        )),
        capture_capability: ProbeRuntimeCapability::DirectReplace,
    }));

    let error = application
        .resume_probe_run(created.summary.id())
        .expect_err("workflow ownership must reject probe reconnect");
    let error_json = serde_json::to_value(error).expect("serialize ownership conflict");
    assert_eq!(error_json["code"], "capture.target_in_use_by_workflow");
}

#[test]
fn probe_view_reports_actual_collection_only_capability_without_persisting_it() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    application.runtimes = Some(Box::new(RecordingWorkflowRuntime {
        calls,
        start_capture_error: None,
        capture_capability: ProbeRuntimeCapability::CollectionOnly,
    }));

    let created = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-collection-only".into(),
            name: "Collection-only probe".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create collection-only probe");

    assert_eq!(
        created.runtime_capability,
        Some(ProbeRuntimeCapability::CollectionOnly)
    );
    let disconnected = application
        .disconnect_probe_run(created.summary.id())
        .expect("disconnect collection-only probe");
    assert_eq!(disconnected.runtime_capability, None);
}

#[test]
fn probe_runs_pause_release_and_reuse_one_dictionary_without_copying_entries() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    let first = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-first".into(),
            name: "First probe".into(),
            software_id: software_id.clone(),
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create first probe");
    assert_eq!(first.summary.status(), ProbeRunStatus::Running);
    assert_eq!(first.summary.dictionary_id(), "dictionary.product");
    let error = application
        .refresh_probe_text(first.summary.id())
        .expect_err("read-only capture must not publish translations");
    assert_eq!(
        serde_json::to_value(error).unwrap()["code"],
        "capture.preview_unavailable"
    );
    assert!(calls.lock().unwrap().capture_publications.is_empty());

    let edited = application
        .edit_probe_translation(ProbeTranslationEditRequest {
            run_id: first.summary.id().into(),
            source: "Close".into(),
            translation: "关闭".into(),
        })
        .expect("edit the bound dictionary directly");
    assert_eq!(edited.dictionary_entry_count, 2);
    assert_eq!(edited.dictionary_revision, 2);
    assert!(application
        .backend
        .dictionary("dictionary.product")
        .expect("edited shared dictionary")
        .entries()
        .iter()
        .any(|entry| entry.source() == "Close" && entry.translation() == "关闭"));

    let paused = application
        .set_probe_run_paused(first.summary.id(), true)
        .expect("pause without ending run");
    assert_eq!(paused.summary.status(), ProbeRunStatus::Paused);
    assert_eq!(
        application.active_probe_run_id.as_deref(),
        Some(first.summary.id())
    );
    {
        let calls = calls.lock().expect("runtime call log");
        assert_eq!(
            calls.capture_controls.as_slice(),
            &[(software_id.clone(), true)]
        );
        assert!(calls.captures_abandoned.is_empty());
    }
    let released = application
        .disconnect_probe_run(first.summary.id())
        .expect("release runtime");
    assert_eq!(released.summary.status(), ProbeRunStatus::Ready);

    application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-second".into(),
            name: "Second probe".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create second probe");
    assert_eq!(
        application.probe_run_list().expect("list probe runs").len(),
        2
    );
    assert_eq!(
        application
            .backend
            .dictionary("dictionary.product")
            .expect("shared dictionary")
            .entries()
            .len(),
        2
    );
}

#[test]
fn probe_resume_reconnects_after_the_old_target_stops_confirming_runtime_control() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    let run = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-target-exited-before-pause".into(),
            name: "Target exited before pause".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create running probe");

    calls
        .lock()
        .expect("runtime call log")
        .capture_control_error = Some(DesktopRuntimeError::SessionRejected);

    let paused = application
        .set_probe_run_paused(run.summary.id(), true)
        .expect("local pause must succeed after the target exits");

    assert_eq!(paused.summary.status(), ProbeRunStatus::Paused);
    assert_eq!(
        application.active_probe_run_id.as_deref(),
        Some(run.summary.id())
    );
    assert_eq!(
        application.active_probe_capability,
        Some(ProbeRuntimeCapability::DirectReplace)
    );
    {
        let calls = calls.lock().expect("runtime call log");
        assert_eq!(calls.capture_controls.len(), 1);
        assert!(calls.capture_controls[0].1);
        assert!(calls.captures_abandoned.is_empty());
    }

    let resumed = application
        .set_probe_run_paused(run.summary.id(), false)
        .expect("resume must create a fresh runtime session");
    assert_eq!(resumed.summary.status(), ProbeRunStatus::Running);
    let calls = calls.lock().expect("runtime call log");
    assert_eq!(calls.captures_started.len(), 2);
    assert_eq!(calls.captures_abandoned.len(), 1);
}

#[test]
fn probe_resume_reuses_a_live_runtime_after_pause_confirmation_fails() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    let run = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-live-after-pause-rejection".into(),
            name: "Live target after pause rejection".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create running probe");
    calls
        .lock()
        .expect("runtime call log")
        .capture_control_error = Some(DesktopRuntimeError::SessionRejected);

    application
        .set_probe_run_paused(run.summary.id(), true)
        .expect("local pause should not require Runtime confirmation");
    {
        let mut calls = calls.lock().expect("runtime call log");
        calls.capture_control_error = None;
        calls.capture_start_error = Some(DesktopRuntimeError::ProtocolRejected);
    }

    let resumed = application
        .set_probe_run_paused(run.summary.id(), false)
        .expect("resume should reuse the still-live Runtime instead of reconnecting");

    assert_eq!(resumed.summary.status(), ProbeRunStatus::Running);
    let calls = calls.lock().expect("runtime call log");
    assert_eq!(calls.captures_started.len(), 1);
    assert!(calls.captures_abandoned.is_empty());
    assert_eq!(
        calls
            .capture_controls
            .iter()
            .map(|(_, paused)| *paused)
            .collect::<Vec<_>>(),
        vec![true, false]
    );
}

#[test]
fn probe_resume_reports_an_offline_target_after_discarding_the_stale_runtime() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    let run = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-offline-after-pause-rejection".into(),
            name: "Offline target after pause rejection".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create running probe");
    calls
        .lock()
        .expect("runtime call log")
        .capture_control_error = Some(DesktopRuntimeError::SessionRejected);
    application
        .set_probe_run_paused(run.summary.id(), true)
        .expect("pause should remain local when target is lost");
    calls.lock().expect("runtime call log").capture_start_error =
        Some(DesktopRuntimeError::UnknownTarget);

    let error = application
        .set_probe_run_paused(run.summary.id(), false)
        .expect_err("an offline target cannot resume collection");

    let error_json = serde_json::to_value(error).expect("serialize reconnect error");
    assert_eq!(error_json["code"], "runtime.target_not_found");
    assert_eq!(
        application
            .probe_runs
            .summary(run.summary.id())
            .expect("paused probe status")
            .status(),
        ProbeRunStatus::Paused
    );
    assert!(application.active_probe_run_id.is_none());
    let calls = calls.lock().expect("runtime call log");
    assert_eq!(calls.captures_started.len(), 2);
    assert_eq!(calls.captures_abandoned.len(), 1);
}

#[test]
fn probe_settings_and_clear_all_preserve_the_run_but_clear_its_bound_dictionary() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    let run = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-settings".into(),
            name: "Probe settings".into(),
            software_id: software_id.clone(),
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: true,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create settings probe");

    let renamed = application
        .update_probe_run(ProbeRunUpdateRequest {
            run_id: run.summary.id().into(),
            name: "Renamed probe".into(),
            dictionary_id: "dictionary.product".into(),
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: true,
        })
        .expect("rename connected probe");
    assert_eq!(renamed.summary.name(), "Renamed probe");
    assert_eq!(
        application.clear_probe_run_entries(run.summary.id()),
        Err(CommandError::new("capture.invalid_state"))
    );

    let paused = application
        .set_probe_run_paused(run.summary.id(), true)
        .expect("pause probe before clearing");
    assert_eq!(paused.summary.status(), ProbeRunStatus::Paused);
    let cleared = application
        .clear_probe_run_entries(run.summary.id())
        .expect("clear paused probe entries");
    assert_eq!(cleared.summary.id(), run.summary.id());
    assert_eq!(cleared.summary.status(), ProbeRunStatus::Paused);
    assert_eq!(cleared.summary.preview_generation(), 1);
    assert_eq!(cleared.dictionary_entry_count, 0);
    assert!(application
        .backend
        .dictionary("dictionary.product")
        .expect("cleared dictionary")
        .entries()
        .is_empty());
    assert!(application.active_probe_run_id.is_none());
    let blocked = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-blocked-while-paused".into(),
            name: "Blocked while paused".into(),
            software_id: software_id.clone(),
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect_err("a cleared paused run still holds the probe slot");
    assert_eq!(blocked, CommandError::new("capture.already_active"));
    assert_eq!(application.probe_run_list().expect("probe list").len(), 1);

    let resumed = application
        .set_probe_run_paused(run.summary.id(), false)
        .expect("resume with a fresh capture writer");
    assert_eq!(resumed.summary.status(), ProbeRunStatus::Running);
    assert_eq!(resumed.summary.preview_generation(), 2);
    assert_eq!(
        application.active_probe_run_id.as_deref(),
        Some(run.summary.id())
    );

    let calls = calls.lock().expect("runtime call log");
    assert_eq!(calls.captures_started.len(), 2);
    assert_eq!(calls.captures_stopped.len(), 1);
    assert_eq!(calls.capture_publications.len(), 2);
    assert_eq!(calls.capture_publications[1].1.generation().value(), 2);
    let mut published_entries = Vec::new();
    calls.capture_publications[1]
        .1
        .snapshot()
        .visit_entries_with_adapters(|location, source, translation, adapters| {
            published_entries.push((
                location.to_owned(),
                source.to_owned(),
                translation.to_owned(),
                adapters.iter().cloned().collect::<Vec<_>>(),
            ));
        });
    assert!(published_entries.is_empty());
}

#[test]
fn cleared_paused_probe_can_be_released_without_stopping_runtime_twice() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    let run = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-clear-then-release".into(),
            name: "Clear then release".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create probe");
    application
        .set_probe_run_paused(run.summary.id(), true)
        .expect("pause probe");
    application
        .clear_probe_run_entries(run.summary.id())
        .expect("clear paused probe");

    let released = application
        .disconnect_probe_run(run.summary.id())
        .expect("release detached paused probe");
    assert_eq!(released.summary.status(), ProbeRunStatus::Ready);
    assert!(application.active_probe_run_id.is_none());
    let calls = calls.lock().expect("runtime call log");
    assert_eq!(calls.captures_stopped.len(), 1);
}

#[test]
fn pending_dictionary_entries_are_excluded_from_probe_snapshots() {
    let (mut application, _calls, _software_id, _data_root) = workflow_application();
    let dictionary = application
        .backend
        .dictionary("dictionary.product")
        .expect("product dictionary")
        .clone();
    application
        .backend
        .upsert_dictionary_entry(
            dictionary.id(),
            DictionaryEntryCreate::new("Pending", ""),
            None,
            dictionary.revision(),
        )
        .expect("store pending entry");

    let snapshot = application
        .probe_dictionary_snapshot("dictionary.product")
        .expect("build completed-only probe snapshot");

    assert_eq!(snapshot.revision(), dictionary.revision() + 1);
}

#[test]
fn probe_ai_plan_uses_every_observed_row_and_preserves_completed_entries() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    let run = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-ai-plan".into(),
            name: "AI plan probe".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create AI plan probe");
    let capture = glyphshift_capture::FileCaptureSink::start(
        application
            .probe_runs
            .capture_configuration(run.summary.id(), DEFAULT_MAX_ENTRIES)
            .expect("capture configuration"),
    )
    .expect("start synthetic observation sink");
    capture.observe(TEST_ADAPTER_ID, "Save");
    capture.observe(TEST_ADAPTER_ID, "42");
    capture.observe(TEST_ADAPTER_ID, "Open");
    for index in 1..=51 {
        capture.observe(TEST_ADAPTER_ID, format!("Pending item {index}"));
    }
    glyphshift_capture::FileCaptureSink::finish(capture).expect("finish observations");

    let mut translation = glyphshift_ai_translation::AiTranslation::new();
    let plan = translation
        .plan_translation(
            application
                .probe_ai_plan_request(run.summary.id())
                .expect("build full probe AI request"),
        )
        .expect("plan probe translations");

    assert_eq!(plan.candidates().len(), 52);
    assert!(plan
        .candidates()
        .iter()
        .any(|item| item.source() == "Pending item 51"));
    assert!(plan.candidates().iter().any(|item| item.source() == "Save"));
    assert!(plan.skipped().iter().any(|item| item.source() == "42"
        && item.reason() == glyphshift_ai_translation::SkipReason::PureNumberOrSymbols));
    assert!(plan.skipped().iter().any(|item| item.source() == "Open"
        && item.reason() == glyphshift_ai_translation::SkipReason::AlreadyTranslated));
}

#[test]
fn probe_ai_writeback_rechecks_blank_entries_after_user_edits() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    let run = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-ai-writeback".into(),
            name: "AI writeback probe".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create AI writeback probe");
    let capture = glyphshift_capture::FileCaptureSink::start(
        application
            .probe_runs
            .capture_configuration(run.summary.id(), DEFAULT_MAX_ENTRIES)
            .expect("capture configuration"),
    )
    .expect("start synthetic observation sink");
    capture.observe(TEST_ADAPTER_ID, "Save");
    capture.observe(TEST_ADAPTER_ID, "Close");
    glyphshift_capture::FileCaptureSink::finish(capture).expect("finish observations");
    let base_revision = application
        .backend
        .dictionary("dictionary.product")
        .expect("bound dictionary")
        .revision();
    application
        .edit_probe_translation(ProbeTranslationEditRequest {
            run_id: run.summary.id().into(),
            source: "Save".into(),
            translation: "人工保存".into(),
        })
        .expect("user translation wins before AI writeback");

    let applied = application
        .apply_probe_ai_results(ai::ProbeAiApplyRequest {
            run_id: run.summary.id().into(),
            snapshot_revision: base_revision,
            results: vec![
                ai::ProbeAiTranslationResult {
                    item_id: "probe-row-save".into(),
                    source: "Save".into(),
                    translation: "AI 保存".into(),
                },
                ai::ProbeAiTranslationResult {
                    item_id: "probe-row-close".into(),
                    source: "Close".into(),
                    translation: "关闭".into(),
                },
            ],
        })
        .expect("apply AI results with compare-and-set semantics");

    assert_eq!(applied.applied_count, 1);
    assert_eq!(applied.skipped_count, 1);
    let dictionary = application
        .backend
        .dictionary("dictionary.product")
        .expect("updated dictionary");
    assert!(dictionary
        .entries()
        .iter()
        .any(|entry| entry.source() == "Save" && entry.translation() == "人工保存"));
    assert!(dictionary
        .entries()
        .iter()
        .any(|entry| entry.source() == "Close" && entry.translation() == "关闭"));
}
