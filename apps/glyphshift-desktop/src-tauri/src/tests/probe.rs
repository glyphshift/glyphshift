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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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
            excluded_dictionary_ids: Vec::new(),
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

#[test]
fn probe_import_modes_preserve_metadata_pending_entries_and_reject_partial_bad_input() {
    use crate::probe_transfer::{ImportMode, ProbeImportRequest};
    let (mut app, _, software_id, root) = workflow_application();
    let run = app.create_probe_run(ProbeRunCreateRequest {
        excluded_dictionary_ids: Vec::new(), id: "probe-import".into(), name: "Import".into(), software_id,
        adapter_ids: vec![TEST_ADAPTER_ID.into()], live_preview_enabled: false,
        dictionary: ProbeDictionaryBindingRequest::Existing { dictionary_id: "dictionary.product".into() },
    }).unwrap();
    let original = app.backend.dictionary(run.summary.dictionary_id()).unwrap().clone();
    let input = root.path().join("entries.csv");
    std::fs::write(&input, "source,translation\r\nNew,First\r\nPending,\r\n").unwrap();
    let request = |mode| ProbeImportRequest { run_id: "probe-import".into(), input_path: input.clone(), format: "csv".into(), mode };
    app.import_probe_entries(request(ImportMode::Overwrite)).unwrap();
    assert!(app.backend.dictionary(original.id()).unwrap().entries().iter().any(|e| e.source() == "Pending" && e.translation().is_empty()));
    std::fs::write(&input, "source,translation\nNew,Second").unwrap();
    app.import_probe_entries(request(ImportMode::KeepExisting)).unwrap();
    assert_eq!(app.backend.dictionary(original.id()).unwrap().entries().iter().find(|e| e.source() == "New").unwrap().translation(), "First");
    app.import_probe_entries(request(ImportMode::Overwrite)).unwrap();
    assert_eq!(app.backend.dictionary(original.id()).unwrap().entries().iter().find(|e| e.source() == "New").unwrap().translation(), "Second");
    let before = app.backend.dictionary(original.id()).unwrap().clone();
    std::fs::write(&input, "source,translation\nNew,Third\nNew,Duplicate").unwrap();
    assert!(app.import_probe_entries(request(ImportMode::Replace)).is_err());
    assert_eq!(app.backend.dictionary(original.id()).unwrap(), &before);
    std::fs::write(&input, "source,translation\nOnly,Replacement").unwrap();
    app.import_probe_entries(request(ImportMode::Replace)).unwrap();
    let dictionary = app.backend.dictionary(original.id()).unwrap();
    assert_eq!(dictionary.metadata(), original.metadata());
    assert_eq!(dictionary.entries().len(), 1);
    assert_eq!(dictionary.entries()[0].source(), "Only");
}


#[test]
fn invalid_exclusion_is_rejected_before_creating_a_dictionary() {
    let (mut app, _, software_id, _root) = workflow_application();
    for excluded in ["dictionary.missing", "dictionary.new"] {
        assert!(app.create_probe_run(ProbeRunCreateRequest {
            excluded_dictionary_ids: vec![excluded.into()], id: "probe-excluded".into(), name: "Excluded".into(), software_id: software_id.clone(),
            adapter_ids: vec![TEST_ADAPTER_ID.into()], live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::New { id: "dictionary.new".into(), name: "New".into(), description: "".into(), source_locale: "en-US".into(), target_locale: "zh-CN".into() },
        }).is_err());
        assert!(app.backend.dictionary("dictionary.new").is_err());
    }
}


#[test]
fn excluded_dictionary_references_and_revisions_are_preserved() {
    let (mut app, _, software_id, _root) = workflow_application();
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.excluded", "Excluded", "en-US", "zh-CN")
        .with_entries([DictionaryEntryCreate::new("Done", "完成")])).unwrap();
    let run = app.create_probe_run(ProbeRunCreateRequest {
        excluded_dictionary_ids: vec!["dictionary.excluded".into()], id: "probe-exclusion-reference".into(), name: "Exclusion".into(), software_id,
        adapter_ids: vec![TEST_ADAPTER_ID.into()], live_preview_enabled: false,
        dictionary: ProbeDictionaryBindingRequest::Existing { dictionary_id: "dictionary.product".into() },
    }).unwrap();
    assert_eq!(run.exclusion_revisions, vec![1]);
    let error = app.delete_dictionaries(&["dictionary.excluded".into()]).unwrap_err();
    assert_eq!(serde_json::to_value(error).unwrap()["code"], "dictionary.referenced");
    let dictionary = app.backend.dictionary("dictionary.excluded").unwrap().clone();
    app.backend.update_dictionary(DictionaryEdit::from_dictionary(&dictionary).with_entries([DictionaryEntryCreate::new("Done", "更新")])).unwrap();
    let refreshed = app.probe_run_summary(run.summary.id()).unwrap();
    assert_eq!(refreshed.dictionary_revision, run.dictionary_revision);
    assert_eq!(refreshed.exclusion_revisions, vec![2]);
}

#[test]
fn attached_dictionary_sources_block_collection_and_recheck_ai_writeback() {
    let (mut app, _, software_id, root) = workflow_application();
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.attached", "Attached", "en-US", "zh-CN")
        .with_entries([DictionaryEntryCreate::new("Already owned", "")])).unwrap();
    let run = app.create_probe_run(ProbeRunCreateRequest {
        excluded_dictionary_ids: vec!["dictionary.attached".into()], id: "probe-ownership".into(), name: "Ownership".into(), software_id,
        adapter_ids: vec![TEST_ADAPTER_ID.into()], live_preview_enabled: false,
        dictionary: ProbeDictionaryBindingRequest::Existing { dictionary_id: "dictionary.product".into() },
    }).unwrap();
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(run.summary.id(), DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    for source in ["Already owned", "Claimed later", "Still new"] { sink.observe(TEST_ADAPTER_ID, source); }
    sink.finish().unwrap();
    let mut ai = glyphshift_ai_translation::AiTranslation::new();
    let plan = ai.plan_translation(app.probe_ai_plan_request(run.summary.id()).unwrap()).unwrap();
    assert!(!plan.candidates().iter().any(|entry| entry.source() == "Already owned"));
    assert!(plan.candidates().iter().any(|entry| entry.source() == "Claimed later"));
    assert!(app.edit_probe_translation(ProbeTranslationEditRequest {
        run_id: run.summary.id().into(), source: "Already owned".into(), translation: "Must not append".into(),
    }).is_err());
    let input = root.path().join("owned.csv");
    std::fs::write(&input, "source,translation\nFresh,Allowed\nAlready owned,Blocked").unwrap();
    let before_import = app.backend.dictionary("dictionary.product").unwrap().clone();
    for mode in [crate::probe_transfer::ImportMode::KeepExisting, crate::probe_transfer::ImportMode::Overwrite, crate::probe_transfer::ImportMode::Replace] {
        assert!(app.import_probe_entries(crate::probe_transfer::ProbeImportRequest {
            run_id: run.summary.id().into(), input_path: input.clone(), format: "csv".into(), mode,
        }).is_err());
        assert_eq!(app.backend.dictionary("dictionary.product").unwrap(), &before_import);
    }
    let attached = app.backend.dictionary("dictionary.attached").unwrap().clone();
    app.backend.update_dictionary(DictionaryEdit::from_dictionary(&attached).with_entries([
        DictionaryEntryCreate::new("Already owned", ""), DictionaryEntryCreate::new("Claimed later", ""),
    ])).unwrap();
    let revision = app.backend.dictionary("dictionary.product").unwrap().revision();
    let applied = app.apply_probe_ai_results(ai::ProbeAiApplyRequest {
        run_id: run.summary.id().into(), snapshot_revision: revision,
        results: vec![
            ai::ProbeAiTranslationResult { item_id: "claimed".into(), source: "Claimed later".into(), translation: "Owned elsewhere".into() },
            ai::ProbeAiTranslationResult { item_id: "new".into(), source: "Still new".into(), translation: "New translation".into() },
        ],
    }).unwrap();
    assert_eq!(applied.applied_count, 1);
    assert_eq!(applied.skipped_count, 1);
    let destination = app.backend.dictionary("dictionary.product").unwrap();
    assert!(!destination.entries().iter().any(|entry| ["Already owned", "Claimed later"].contains(&entry.source())));
    assert!(destination.entries().iter().any(|entry| entry.source() == "Still new"));
    assert!(app.backend.dictionary("dictionary.attached").unwrap().entries().iter().all(|entry| entry.translation().is_empty()));
}

#[test]
fn collection_resolution_preserves_counts_and_filters_before_pagination_without_writes() {
    let (mut app, _, software_id, _root) = workflow_application();
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.attached", "Attached", "en-US", "zh-CN")
        .with_entries([DictionaryEntryCreate::new("Total", "总计"), DictionaryEntryCreate::new("Owned", "附加词条")])).unwrap();
    let dictionary = app.backend.dictionary("dictionary.attached").unwrap().clone();
    app.backend.update_dictionary(DictionaryEdit::from_dictionary(&dictionary).with_text_rules(vec![
        glyphshift_translation::RegexTranslationRule { enabled: true, pattern: r"^(.+?)(:[ \t]*[0-9]+)$".into(), replacement: "{{TR}}$2".into() },
        glyphshift_translation::RegexTranslationRule { enabled: true, pattern: "^Erase$".into(), replacement: "".into() },
    ])).unwrap();
    let run = app.create_probe_run(ProbeRunCreateRequest {
        excluded_dictionary_ids: vec!["dictionary.attached".into()], id: "probe-resolution".into(), name: "Resolution".into(), software_id,
        adapter_ids: vec![TEST_ADAPTER_ID.into()], live_preview_enabled: false,
        dictionary: ProbeDictionaryBindingRequest::Existing { dictionary_id: "dictionary.product".into() },
    }).unwrap();
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(run.summary.id(), DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    for source in ["Total: 33", "Selected:0", "Owned", "Erase"] { sink.observe(TEST_ADAPTER_ID, source); }
    sink.finish().unwrap();
    let before = app.backend.dictionary("dictionary.product").unwrap().clone();
    let request = |search: &str, filter| crate::probe::ProbeRunQueryRequest { run_id: run.summary.id().into(), search: search.into(), adapter_ids: vec![], translation_filter: filter, merge_rules: None, page: 1, page_size: 50 };
    let page = app.probe_run_entries(request("", ProbeTranslationFilter::RuleMatched)).unwrap();
    assert_eq!(page.total(), 3);
    let mut paged = request("", ProbeTranslationFilter::RuleMatched);
    paged.page_size = 1;
    let first = app.probe_run_entries(paged).unwrap();
    assert_eq!(first.total(), 3);
    assert_eq!(first.rows().len(), 1);
    let json = serde_json::to_value(&page).unwrap();
    let rows = json["rows"].as_array().unwrap();
    let total = rows.iter().find(|row| row["source"] == "Total: 33").unwrap();
    assert_eq!(total["translation"], "总计: 33");
    assert_eq!(total["resolution"]["kind"], "rule_translated");
    assert_eq!(total["resolution"]["dictionaryIds"][0], "dictionary.attached");
    assert_eq!(rows.iter().find(|row| row["source"] == "Selected:0").unwrap()["resolution"]["kind"], "rule_pending");
    assert_eq!(app.probe_run_entries(request("总计", ProbeTranslationFilter::Translated)).unwrap().total(), 1);
    assert_eq!(app.probe_run_entries(request("", ProbeTranslationFilter::Skipped)).unwrap().total(), 1);
    let owned = app.probe_run_entries(request("Owned", ProbeTranslationFilter::OtherDictionary)).unwrap();
    assert_eq!(owned.total(), 1);
    assert_eq!(owned.rows()[0].translation(), "附加词条");
    assert_eq!(serde_json::to_value(owned).unwrap()["rows"][0]["resolution"]["editable"], false);
    assert_eq!(app.backend.dictionary("dictionary.product").unwrap(), &before);
    assert!(app.edit_probe_translation(ProbeTranslationEditRequest { run_id: run.summary.id().into(), source: "Total: 33".into(), translation: "直接翻译".into() }).is_err());
    let exact = app.probe_run_entries(request("Total: 33", ProbeTranslationFilter::Translated)).unwrap();
    assert_eq!(exact.rows()[0].translation(), "总计: 33");
    assert_eq!(serde_json::to_value(exact).unwrap()["rows"][0]["resolution"]["kind"], "rule_translated");
}
