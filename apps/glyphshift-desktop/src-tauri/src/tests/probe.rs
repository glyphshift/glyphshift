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
fn probe_preview_uses_replacement_adapters_without_rejecting_collection_adapters() {
    let (mut application, _calls, _software_id, _data_root) = workflow_application();
    application.adapters = vec![
        AdapterView {
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
        },
        AdapterView {
            id: "test.observe-only".into(),
            name: "Collection adapter".into(),
            version: "1.0.0".into(),
            summary: "Synthetic collection adapter".into(),
            platforms: vec!["windows".into()],
            technologies: vec!["Synthetic".into()],
            features: vec!["textObserve".into()],
            technical_target: "SyntheticObserve".into(),
            documentation_url: None,
            configuration: "none".into(),
        },
    ];
    let mixed = vec![TEST_ADAPTER_ID.into(), "test.observe-only".into()];
    let collection_only = vec![Box::<str>::from("test.observe-only")];

    assert!(application.adapters_support_preview(&mixed));
    assert_eq!(
        application.preview_adapter_ids(&mixed),
        vec![Box::<str>::from(TEST_ADAPTER_ID)]
    );
    assert!(!application.adapters_support_preview(&collection_only));
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

    let calls = calls.lock().expect("runtime call log");
    assert_eq!(calls.capture_publications.len(), 2);
    assert!(calls
        .capture_publications
        .iter()
        .all(|(published_software, _)| published_software.as_ref() == software_id.as_ref()));
    assert_eq!(calls.capture_publications[0].1.generation().value(), 1);
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
    let (mut application, _calls, software_id, _data_root) = workflow_application();
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
fn probe_settings_and_clear_all_preserve_the_run_but_clear_its_bound_dictionary() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    let run = application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe-settings".into(),
            name: "Probe settings".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create settings probe");

    let renamed = application
        .update_probe_run(ProbeRunUpdateRequest {
            run_id: run.summary.id().into(),
            name: "Renamed probe".into(),
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
        })
        .expect("rename connected probe");
    assert_eq!(renamed.summary.name(), "Renamed probe");
    assert_eq!(
        application.clear_probe_run_entries(run.summary.id()),
        Err(CommandError::new("capture.invalid_state"))
    );

    application
        .disconnect_probe_run(run.summary.id())
        .expect("release probe");
    let cleared = application
        .clear_probe_run_entries(run.summary.id())
        .expect("clear probe entries");
    assert_eq!(cleared.summary.id(), run.summary.id());
    assert_eq!(cleared.dictionary_entry_count, 0);
    assert!(application
        .backend
        .dictionary("dictionary.product")
        .expect("cleared dictionary")
        .entries()
        .is_empty());
}
