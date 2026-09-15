use super::*;

#[test]
fn distinguishes_a_missing_path_match_from_a_rejected_runtime() {
    let missing = serde_json::to_value(runtime_command_error(
        DesktopRuntimeError::UnknownTarget,
        true,
    ))
    .expect("serialize missing target error");
    let rejected = serde_json::to_value(runtime_command_error(
        DesktopRuntimeError::SessionRejected,
        true,
    ))
    .expect("serialize rejected session error");

    assert_eq!(missing["code"], "runtime.target_not_found");
    assert_eq!(rejected["code"], "runtime.session_rejected");
}

#[test]
fn workflow_activation_error_explains_an_empty_dictionary() {
    let error = serde_json::to_value(workflow_activation_command_error(
        BackendError::WorkflowRejected(ResolveError::NoEffectiveRules {
            software_id: "software.empty".into(),
        }),
    ))
    .expect("serialize workflow activation error");

    assert_eq!(error["code"], "workflow.no_effective_rules");
    assert_eq!(error["args"]["softwareId"], "software.empty");
}

#[test]
fn font_only_workflow_does_not_misclassify_a_product_adapter_as_unavailable() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    application
        .backend
        .replace_font_families(["Synthetic Sans"]);
    application
        .create_workflow(
            WorkflowCreate::new("workflow.font-only", "Font only").with_targets([
                WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], Vec::<Box<str>>::new())
                    .with_font_policy(glyphshift_desktop_backend::WorkflowFontPolicy::new(
                        ["Synthetic Sans"],
                        glyphshift_desktop_backend::FontCoverage::AllObservations,
                    )),
            ]),
        )
        .expect("create a font-only workflow");

    application
        .enable_workflow("workflow.font-only", false)
        .expect(
            "a current product adapter must remain valid when only font substitution is requested",
        );
}

#[test]
fn workflow_with_an_adapter_missing_from_the_current_environment_stays_rejected() {
    let (application, _calls, _software_id, data_root) = workflow_application();
    drop(application);
    let backend = DesktopBackend::open_with_environment(
        data_root.path(),
        DesktopEnvironment::new(Vec::<AdapterRequirement>::new(), Vec::<Box<str>>::new()),
    )
    .expect("reopen product data without the former adapter");
    let calls = Arc::new(StdMutex::new(WorkflowRuntimeCalls::default()));
    let runtimes: Box<dyn WorkflowRuntimeService> = Box::new(RecordingWorkflowRuntime {
        calls,
        start_capture_error: None,
        capture_capability: ProbeRuntimeCapability::DirectReplace,
    });
    let mut reopened = test_desktop_application(data_root.path(), backend, runtimes);

    let error = reopened
        .enable_workflow("workflow.product", false)
        .expect_err("an adapter absent from the current environment must remain invalid");
    let json = serde_json::to_value(error).expect("serialize unknown adapter error");
    assert_eq!(json["code"], "workflow.unknown_adapter");
    assert_eq!(json["args"]["adapterId"], TEST_ADAPTER_ID);
}

#[test]
fn runtime_protocol_rejection_does_not_collapse_into_a_generic_activation_error() {
    let error = serde_json::to_value(runtime_command_error(
        DesktopRuntimeError::ProtocolRejected,
        true,
    ))
    .expect("serialize Runtime protocol rejection");

    assert_eq!(error["code"], "runtime.component_incompatible");
}

#[test]
fn runtime_module_rejection_reaches_an_actionable_command_error() {
    let error = serde_json::to_value(runtime_command_error(
        DesktopRuntimeError::ActivationRejected(HostOperationFailure::RuntimeModuleUnavailable),
        true,
    ))
    .expect("serialize Runtime module rejection");

    assert_eq!(error["code"], "runtime.component_load_failed");
}

#[test]
fn changed_resident_adapter_set_requires_a_target_restart() {
    let error = serde_json::to_value(runtime_command_error(
        DesktopRuntimeError::ActivationRejected(HostOperationFailure::TargetRuntimeRestartRequired),
        true,
    ))
    .expect("serialize target Runtime restart requirement");

    assert_eq!(error["code"], "runtime.target_restart_required");
}

#[test]
fn isolated_worker_permission_and_timeout_reach_actionable_command_errors() {
    let denied = serde_json::to_value(runtime_command_error_with_privilege(
        DesktopRuntimeError::ActivationRejected(
            HostOperationFailure::IsolatedWorkerPermissionDenied,
        ),
        true,
        Some(true),
    ))
    .expect("serialize isolated Worker permission rejection");
    let timeout = serde_json::to_value(runtime_command_error(
        DesktopRuntimeError::ActivationRejected(HostOperationFailure::IsolatedWorkerTimeout),
        true,
    ))
    .expect("serialize isolated Worker timeout");

    assert_eq!(denied["code"], "runtime.target_access_failed");
    assert_eq!(denied["args"]["operation"], "observer");
    assert_eq!(denied["args"]["controllerElevated"], true);
    assert_eq!(timeout["code"], "runtime.activation_timed_out");
}

#[test]
fn target_access_rejections_preserve_the_failed_operation_and_controller_privilege() {
    let process = serde_json::to_value(runtime_command_error_with_privilege(
        DesktopRuntimeError::ActivationRejected(HostOperationFailure::TargetProcessUnavailable),
        true,
        Some(true),
    ))
    .expect("serialize target process rejection");
    let memory = serde_json::to_value(runtime_command_error_with_privilege(
        DesktopRuntimeError::ActivationRejected(HostOperationFailure::RemoteMemoryUnavailable),
        true,
        Some(true),
    ))
    .expect("serialize remote memory rejection");
    let thread = serde_json::to_value(runtime_command_error_with_privilege(
        DesktopRuntimeError::ActivationRejected(HostOperationFailure::RemoteThreadUnavailable),
        true,
        Some(false),
    ))
    .expect("serialize remote thread rejection");

    assert_eq!(process["args"]["operation"], "targetProcess");
    assert_eq!(process["args"]["controllerElevated"], true);
    assert_eq!(memory["args"]["operation"], "remoteMemory");
    assert_eq!(memory["args"]["controllerElevated"], true);
    assert_eq!(thread["args"]["operation"], "remoteThread");
    assert_eq!(thread["args"]["controllerElevated"], false);
}

#[test]
fn workflow_enable_and_disable_share_one_product_command_path() {
    let (mut application, calls, software_id, _data_root) = workflow_application();

    let enabled = application
        .enable_workflow("workflow.product", false)
        .expect("enable product workflow");

    assert_eq!(enabled.definition.id(), "workflow.product");
    assert!(enabled.activation.enabled);
    assert_eq!(enabled.runtime.workflow_id.as_ref(), "workflow.product");
    assert_eq!(enabled.runtime.targets[0].software_id, software_id);
    assert!(enabled.runtime.targets[0].active);
    assert_eq!(
        application.backend.enabled_workflow_ids(),
        &[Box::<str>::from("workflow.product")]
    );
    assert_eq!(
        calls.lock().expect("runtime call log").enabled,
        vec![(
            Box::<str>::from("workflow.product"),
            false,
            vec![software_id.clone()]
        )]
    );

    let disabled = application
        .disable_workflow("workflow.product")
        .expect("disable product workflow");

    assert!(!disabled.activation.enabled);
    assert!(!disabled.runtime.targets[0].active);
    assert!(application.backend.enabled_workflow_ids().is_empty());
    assert_eq!(
        calls.lock().expect("runtime call log").disabled,
        vec![Box::<str>::from("workflow.product")]
    );
}

#[test]
fn workflow_diagnostics_exposes_public_names_and_bounded_trace_facts() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    application.adapters.push(AdapterView {
        id: TEST_ADAPTER_ID.into(),
        name: "合成适配器".into(),
        version: "1.0.0".into(),
        summary: "合成诊断适配器".into(),
        platforms: vec!["windows".into()],
        technologies: vec!["GDI".into()],
        features: vec!["text-replace".into()],
        technical_target: "Synthetic".into(),
        documentation_url: None,
        configuration: "none".into(),
        process_resident_after_deactivate: false,
    });
    application
        .enable_workflow("workflow.product", false)
        .expect("enable workflow");

    application
        .control_workflow_diagnostics("workflow.product", true)
        .expect("enable workflow diagnostics");
    let diagnostics = application
        .workflow_diagnostics("workflow.product")
        .expect("query workflow diagnostics");

    assert_eq!(diagnostics.workflow_id.as_ref(), "workflow.product");
    assert_eq!(diagnostics.records.len(), 1);
    assert_eq!(
        diagnostics.records[0].software_id.as_ref(),
        software_id.as_ref()
    );
    assert_eq!(diagnostics.records[0].adapter_name.as_ref(), "合成适配器");
    assert_eq!(diagnostics.records[0].source_text.as_ref(), "Open");
    assert_eq!(diagnostics.records[0].status.as_ref(), "matched");
    assert_eq!(diagnostics.dropped, 3);
}

#[test]
fn workflow_enable_keeps_desired_activation_when_runtime_is_unavailable() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    application.runtimes = None;

    let result = application
        .enable_workflow("workflow.product", false)
        .expect("persist desired workflow activation");

    assert!(result.activation.enabled);
    assert_eq!(
        application.backend.enabled_workflow_ids(),
        &[Box::<str>::from("workflow.product")]
    );
    assert_eq!(result.runtime.targets[0].software_id, software_id);
    assert!(result.runtime.targets[0].translation_requested);
    assert!(!result.runtime.targets[0].active);
    assert!(result.runtime.errors.contains_key(software_id.as_ref()));
}

#[test]
fn workflow_enable_can_explicitly_replace_a_conflicting_activation() {
    let (mut application, calls, _software_id, _data_root) = workflow_application();
    application
        .backend
        .copy_workflow("workflow.product", "workflow.replacement", "替换工作流")
        .expect("copy conflicting workflow");
    application
        .enable_workflow("workflow.product", false)
        .expect("enable original workflow");

    let replacement = application
        .enable_workflow("workflow.replacement", true)
        .expect("explicitly replace workflow");

    assert_eq!(replacement.definition.id(), "workflow.replacement");
    assert_eq!(
        application.backend.enabled_workflow_ids(),
        &[Box::<str>::from("workflow.replacement")]
    );
    assert_eq!(
        calls
            .lock()
            .expect("runtime call log")
            .enabled
            .last()
            .map(|call| (call.0.as_ref(), call.1)),
        Some(("workflow.replacement", true))
    );
}

#[test]
fn workflow_runtime_serialization_does_not_expose_process_or_adapter_identity() {
    let (mut application, _calls, _software_id, _data_root) = workflow_application();
    let result = application
        .enable_workflow("workflow.product", false)
        .expect("enable workflow");

    let json = serde_json::to_value(result).expect("serialize workflow command result");
    let runtime_target = &json["runtime"]["targets"][0];

    assert_eq!(
        runtime_target
            .as_object()
            .expect("runtime target object")
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            "active",
            "appliedGeneration",
            "discovered",
            "fontActive",
            "fontRequested",
            "softwareId",
            "translationActive",
            "translationRequested",
        ]
    );
}

#[test]
fn desktop_snapshot_returns_product_configuration_activation_and_runtime_state() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    application
        .enable_workflow("workflow.product", false)
        .expect("enable workflow");

    let json = serde_json::to_value(application.snapshot()).expect("serialize product snapshot");

    assert_eq!(json["workflows"][0]["id"], "workflow.product");
    assert_eq!(
        json["dictionaries"][0]["metadata"]["id"],
        "dictionary.product"
    );
    assert_eq!(
        json["dictionaries"][0]["installation"],
        serde_json::json!({
            "state": "unmanaged",
            "installedRelease": null,
            "verifiedPublisher": null,
            "updateRelease": null,
        })
    );
    assert!(json["dictionaries"][0].get("digest").is_none());
    assert!(json["dictionaries"][0].get("signature").is_none());
    assert_eq!(
        json["activations"][0],
        serde_json::json!({
            "workflowId": "workflow.product",
            "revision": 1,
        })
    );
    assert_eq!(
        json["workflowRuntimeStatus"]["workflow.product"]["targets"][0]["softwareId"],
        software_id.as_ref()
    );
    assert_eq!(
        json["workflowRuntimeStatus"]["workflow.product"]["targets"][0]["active"],
        true
    );
}

#[test]
fn desktop_snapshot_exposes_adapter_documentation_as_presentation_metadata() {
    let (mut application, _calls, _software_id, _data_root) = workflow_application();
    application.adapters[0].documentation_url = Some("https://example.invalid/adapter".into());

    let json = serde_json::to_value(application.snapshot()).expect("serialize product snapshot");

    assert_eq!(
        json["adapters"][0]["documentationUrl"],
        "https://example.invalid/adapter"
    );
}

#[test]
fn workflow_create_returns_the_updated_product_snapshot() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();

    let snapshot = application
        .create_workflow(
            WorkflowCreate::new("workflow.secondary", "备用工作流").with_targets([
                WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.product"]),
            ]),
        )
        .expect("create workflow through product command");

    assert_eq!(
        snapshot
            .configuration
            .workflows()
            .iter()
            .map(|workflow| workflow.id())
            .collect::<Vec<_>>(),
        vec!["workflow.product", "workflow.secondary"]
    );
    assert_eq!(
        application
            .workflow_detail("workflow.secondary")
            .expect("load created workflow")
            .name(),
        "备用工作流"
    );
}

#[test]
fn updating_an_enabled_workflow_reconciles_its_new_generation() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    let enabled = application
        .enable_workflow("workflow.product", false)
        .expect("enable workflow");
    let previous_generation = enabled.runtime.targets[0]
        .applied_generation
        .expect("initial applied generation");

    let snapshot = application
        .update_workflow(
            WorkflowEdit::new("workflow.product", "已更新工作流", 1).with_targets([
                WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.product"]),
            ]),
        )
        .expect("update enabled workflow");

    let summary = &snapshot.configuration.workflows()[0];
    assert_eq!((summary.name(), summary.revision()), ("已更新工作流", 2));
    assert!(
        snapshot.workflow_runtime_status["workflow.product"].targets[0]
            .applied_generation
            .is_some_and(|generation| generation > previous_generation)
    );
}

#[test]
fn workflow_copy_and_batch_delete_return_the_updated_product_snapshot() {
    let (mut application, _calls, _software_id, _data_root) = workflow_application();

    let copied = application
        .copy_workflow(
            "workflow.product",
            "workflow.copy".into(),
            "工作流副本".into(),
        )
        .expect("copy workflow");
    assert_eq!(
        copied
            .configuration
            .workflows()
            .iter()
            .map(|workflow| workflow.id())
            .collect::<Vec<_>>(),
        vec!["workflow.copy", "workflow.product"]
    );

    let deleted = application
        .delete_workflows(&[Box::<str>::from("workflow.copy")])
        .expect("delete copied workflow");
    assert_eq!(
        deleted
            .configuration
            .workflows()
            .iter()
            .map(|workflow| workflow.id())
            .collect::<Vec<_>>(),
        vec!["workflow.product"]
    );
}

#[test]
fn persisted_activations_are_restored_and_refreshed_as_workflow_runtime_state() {
    let (mut application, _calls, _software_id, data_root) = workflow_application();
    application
        .enable_workflow("workflow.product", false)
        .expect("persist enabled workflow");
    drop(application);
    let backend = DesktopBackend::open_with_environment(
        data_root.path(),
        DesktopEnvironment::new(
            [AdapterRequirement::new(
                glyphshift_domain::AdapterId::new(TEST_ADAPTER_ID),
                AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
                [
                    Feature::TextObserve,
                    Feature::TextReplace,
                    Feature::FontSubstitute,
                ],
            )],
            Vec::<Box<str>>::new(),
        ),
    )
    .expect("reopen product backend");
    let calls = Arc::new(StdMutex::new(WorkflowRuntimeCalls::default()));
    let runtimes: Box<dyn WorkflowRuntimeService> = Box::new(RecordingWorkflowRuntime {
        calls: Arc::clone(&calls),
        start_capture_error: None,
        capture_capability: ProbeRuntimeCapability::DirectReplace,
    });
    let mut reopened = DesktopApplication {
        backend,
        dictionary_distribution: offline_dictionary_distribution(data_root.path())
            .expect("offline dictionary distribution"),
        runtimes: Some(runtimes),
        runtime_bundle_error: None,
        workflow_runtime_status: BTreeMap::new(),
        adapters: Vec::new(),
        adapter_target_support: BTreeMap::new(),
        font_families: Vec::new(),
        font_cache_root: data_root.path().to_path_buf(),
        probe_snapshot_cache: Default::default(),
        exiting: false,
        exit_ready: false,
        collection_filter_policy: Default::default(),
        collection_filter: Default::default(),
        collection_versions: Default::default(),
        pending_collection_runs: Default::default(),
        workflow_compatibility_checks: Default::default(),
        probe_runs: ProbeRunStore::open(data_root.path().join("probe-runs"))
            .expect("probe run store"),
        quick_probe_sessions: QuickProbeSessionStore::open(data_root.path())
            .expect("quick probe session store"),
        active_probe_run_id: None,
        active_probe_capability: None,
        ai_locked_dictionary_id: None,
    };

    reopened
        .restore_enabled_workflows()
        .expect("restore enabled workflow runtime");
    let restored = reopened.snapshot();
    assert!(restored.workflow_runtime_status["workflow.product"].targets[0].active);

    let refreshed = reopened
        .refresh_workflows()
        .expect("refresh enabled workflows");
    assert!(refreshed.workflow_runtime_status["workflow.product"].targets[0].active);
    assert_eq!(
        calls.lock().expect("runtime call log").refreshed,
        vec![Box::<str>::from("workflow.product")]
    );
}

#[test]
fn workflow_collection_uses_the_writer_and_other_dictionary_sources() {
    let (mut app, calls, software_id, _root) = workflow_application();
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.writer", "Writer", "en-US", "zh-CN")).unwrap();
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.pending", "Pending", "en-US", "zh-CN")
        .with_entries([DictionaryEntryCreate::new("Owned elsewhere", "")])).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.collect", "Collect").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.writer", "dictionary.pending"])
            .with_write_dictionary("dictionary.writer"),
    ])).unwrap();
    let ready = app.workflow_collection_view("workflow.collect").unwrap();
    assert_eq!(ready.summary.status(), ProbeRunStatus::Ready);
    app.resume_probe_run(ready.summary.id()).unwrap();
    let run = app.probe_runs.summary("collection-workflow.collect-0").unwrap();
    assert_eq!(run.dictionary_id(), "dictionary.writer");
    assert_eq!(run.excluded_dictionary_ids(), &[Box::<str>::from("dictionary.pending")]);
    assert_eq!(run.status(), ProbeRunStatus::Running);
    assert!(calls.lock().unwrap().captures_started.is_empty());
    assert_eq!(run.workflow_id(), Some("workflow.collect"));
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(run.id(), DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    sink.observe(TEST_ADAPTER_ID, "Owned elsewhere");
    sink.observe(TEST_ADAPTER_ID, "New source");
    sink.finish().unwrap();
    app.refresh_workflows().unwrap();
    app.probe_run_summary(run.id()).unwrap();
    let paused = app.set_probe_run_paused(run.id(), true).unwrap();
    assert_eq!(paused.summary.status(), ProbeRunStatus::Paused);
    assert!(app.backend.enabled_workflow_ids().iter().any(|id| id.as_ref() == "workflow.collect"));
    assert_eq!(app.set_probe_run_paused(run.id(), false).unwrap().summary.status(), ProbeRunStatus::Running);
    app.refresh_probe_text(run.id()).unwrap();
    assert_eq!(calls.lock().unwrap().capture_publications.len(), 1);
    let writer = app.backend.dictionary("dictionary.writer").unwrap();
    assert_eq!(writer.entries().len(), 1);
    assert_eq!(writer.entries()[0].source(), "New source");
    assert!(writer.entries()[0].translation().is_empty());
    app.edit_probe_translation(ProbeTranslationEditRequest {
        run_id: run.id().into(), source: "New source".into(), translation: "Translated source".into(),
    }).unwrap();
    assert_eq!(app.backend.dictionary("dictionary.writer").unwrap().entries()[0].translation(), "Translated source");
    assert!(!calls.lock().unwrap().refreshed.is_empty());
    let current = app.backend.workflow("workflow.collect").unwrap();
    assert!(app.backend.update_workflow(WorkflowEdit::new(current.id(), current.name(), current.revision()).with_targets([
        WorkflowTargetCreate::new(run.software_id(), [TEST_ADAPTER_ID], ["dictionary.writer", "dictionary.pending"])
            .with_write_dictionary("dictionary.pending"),
    ])).is_err());
    assert_eq!(app.backend.workflow("workflow.collect").unwrap().targets()[0].write_dictionary_id(), Some("dictionary.writer"));
    app.disconnect_probe_run(run.id()).unwrap();
    assert_eq!(app.probe_runs.summary(run.id()).unwrap().status(), ProbeRunStatus::Ready);
    assert!(app.refresh_probe_text(run.id()).is_err());
    assert_eq!(app.backend.dictionary("dictionary.writer").unwrap().entries().len(), 1);
    let current = app.backend.workflow("workflow.collect").unwrap();
    app.backend.update_workflow(WorkflowEdit::new(current.id(), current.name(), current.revision()).with_targets([
        WorkflowTargetCreate::new(run.software_id(), [TEST_ADAPTER_ID], ["dictionary.pending"]).with_write_dictionary("dictionary.pending"),
    ])).unwrap();
    let switched = app.workflow_collection_view("workflow.collect").unwrap();
    assert_ne!(switched.summary.id(), run.id());
    assert_eq!(app.backend.dictionary("dictionary.pending").unwrap().entries().len(), 1);
    assert_eq!(app.probe_runs.summary(run.id()).unwrap().dictionary_id(), "dictionary.writer");
    app.delete_workflows(&["workflow.collect".into()]).unwrap();
    assert!(!app.probe_runs.list().unwrap().iter().any(|record| record.workflow_id() == Some("workflow.collect")));
    assert_eq!(app.backend.dictionary("dictionary.writer").unwrap().entries().len(), 1);
}

#[test]
fn workflow_collection_start_reports_runtime_failure() {
    let (mut app, _calls, software_id, _root) = workflow_application();
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.writer", "Writer", "en-US", "zh-CN")).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.collect", "Collect").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.writer"])
            .with_write_dictionary("dictionary.writer"),
    ])).unwrap();
    let ready = app.workflow_collection_view("workflow.collect").unwrap();
    app.runtimes = None;
    let result = serde_json::to_value(app.resume_probe_run(ready.summary.id()).unwrap()).unwrap();
    assert_eq!(result["workflowRuntime"]["lifecycle"]["phase"], "failed");
    assert_eq!(result["workflowRuntime"]["lifecycle"]["enabled"], true);
}

#[test]
fn missing_software_binding_has_a_specific_repair_message() {
    let error = serde_json::to_value(workflow_activation_command_error(
        BackendError::SoftwareBindingMissing("software.synthetic".into()),
    )).unwrap();
    assert_eq!(error["code"], "software.binding_missing");
    assert_eq!(error["args"]["softwareId"], "software.synthetic");
}

#[test]
fn collection_summaries_are_read_only_and_locked_final_batches_retry() {
    let (mut app, _calls, software_id, _root) = workflow_application();
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.writer", "Writer", "en-US", "zh-CN")).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.batch", "Batch").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.writer"])
            .with_write_dictionary("dictionary.writer"),
    ])).unwrap();
    let ready = app.workflow_collection_view("workflow.batch").unwrap();
    let id = ready.summary.id();
    app.resume_probe_run(id).unwrap();
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(id, DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    for index in 0..500 { sink.observe(TEST_ADAPTER_ID, format!("New source {index}")); }
    sink.finish().unwrap();
    for _ in 0..5 { app.probe_run_summary(id).unwrap(); }
    assert!(app.backend.dictionary("dictionary.writer").unwrap().entries().is_empty());
    app.ai_locked_dictionary_id = Some("dictionary.writer".into());
    app.disable_workflow("workflow.batch").unwrap();
    assert!(app.pending_collection_runs.contains(id));
    assert!(app.backend.dictionary("dictionary.writer").unwrap().entries().is_empty());
    app.ai_locked_dictionary_id = None;
    app.collect_workflow_sources(id).unwrap();
    assert!(!app.pending_collection_runs.contains(id));
    let dictionary = app.backend.dictionary("dictionary.writer").unwrap();
    assert_eq!(dictionary.entries().len(), 500);
    let revision = dictionary.revision();
    let summary = app.probe_runs.summary(id).unwrap();
    let snapshot = app.probe_entries_snapshot(&summary).unwrap();
    for _ in 0..20 { app.collect_workflow_sources(id).unwrap(); }
    assert_eq!(app.backend.dictionary("dictionary.writer").unwrap().revision(), revision);
    assert!(Arc::ptr_eq(&snapshot, &app.probe_entries_snapshot(&summary).unwrap()));
    let plan = app.probe_ai_plan_request(id).unwrap();
    let _ = plan;
}

#[test]
fn workflow_lifecycle_preserves_collection_choice_and_reports_stop_failure() {
    let (mut app, calls, software_id, _root) = workflow_application();
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.lifecycle", "Writer", "en-US", "zh-CN")).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.lifecycle", "Lifecycle").with_targets([
        WorkflowTargetCreate::new(software_id.clone(), [TEST_ADAPTER_ID], ["dictionary.lifecycle"])
            .with_write_dictionary("dictionary.lifecycle"),
    ])).unwrap();
    let run = app.workflow_collection_view("workflow.lifecycle").unwrap();
    app.enable_workflow("workflow.lifecycle", false).unwrap();
    app.set_probe_run_paused(run.summary.id(), true).unwrap();
    app.disable_workflow("workflow.lifecycle").unwrap();
    let started = app.enable_workflow("workflow.lifecycle", false).unwrap();
    assert_eq!(started.runtime.lifecycle.as_ref().unwrap().phase, "running");
    assert!(!started.runtime.lifecycle.as_ref().unwrap().collect_new_sources);
    assert_eq!(app.probe_runs.summary(run.summary.id()).unwrap().status(), ProbeRunStatus::Paused);
    assert!(calls.lock().unwrap().capture_controls.last().unwrap().1);
    let mut runtime = started.runtime;
    runtime.errors.insert(software_id.into(), CommandError::new("runtime.stop_unconfirmed"));
    app.backend.disable_workflow("workflow.lifecycle").unwrap();
    assert_eq!(app.project_workflow_runtime(runtime.clone()).lifecycle.unwrap().phase, "stop_failed");
    runtime.errors.clear();
    for target in &mut runtime.targets { target.active = false; }
    assert_eq!(app.project_workflow_runtime(runtime.clone()).lifecycle.unwrap().phase, "stopped");
    app.backend.enable_workflow("workflow.lifecycle").unwrap();
    runtime.errors.insert("synthetic".into(), CommandError::new("runtime.target_not_found"));
    assert_eq!(app.project_workflow_runtime(runtime.clone()).lifecycle.unwrap().phase, "waiting");
    runtime.errors.insert("synthetic".into(), CommandError::new("runtime.component_load_failed"));
    runtime.retry_attempt = 0;
    runtime.retry_after_ms = 0;
    app.workflow_runtime_status.insert("workflow.lifecycle".into(), runtime);
    let before = calls.lock().unwrap().refreshed.len();
    app.refresh_workflows_with_retry(false).unwrap();
    assert_eq!(calls.lock().unwrap().refreshed.len(), before + 1);
    let mut recovered = app.workflow_runtime_status["workflow.lifecycle"].clone();
    assert!(recovered.targets.iter().any(|target| target.active));
    assert!(recovered.errors.is_empty());

    recovered.targets.iter_mut().for_each(|target| target.active = false);
    recovered.errors.insert("synthetic".into(), CommandError::new("runtime.target_restart_required"));
    app.workflow_runtime_status.insert("workflow.lifecycle".into(), recovered);
    app.refresh_workflows_with_retry(false).unwrap();
    assert_eq!(calls.lock().unwrap().refreshed.len(), before + 1);
    app.refresh_workflows_with_retry(true).unwrap();
    assert_eq!(calls.lock().unwrap().refreshed.len(), before + 2);
}

#[test]
fn workflow_reports_no_compatibility_signal_after_five_seconds_and_clears_it_after_observation() {
    let (mut app, _calls, software_id, _root) = workflow_application();
    app.backend.create_dictionary(DictionaryCreate::new(
        "dictionary.compatibility",
        "Compatibility",
        "en-US",
        "zh-CN",
    )).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.compatibility", "Compatibility").with_targets([
        WorkflowTargetCreate::new(
            software_id.clone(),
            [TEST_ADAPTER_ID],
            ["dictionary.compatibility"],
        ).with_write_dictionary("dictionary.compatibility"),
    ])).unwrap();

    let run = app.workflow_collection_view("workflow.compatibility").unwrap();
    let started = app.enable_workflow("workflow.compatibility", false).unwrap();
    assert_eq!(started.runtime.lifecycle.as_ref().unwrap().phase, "running");

    let summary = app.probe_runs.summary(run.summary.id()).unwrap();
    app.workflow_compatibility_checks.insert(run.summary.id().to_owned(), WorkflowCompatibilityCheck {
        started_at_ms: glyphshift_capture::unix_time_millis().saturating_sub(5_000),
        baseline_observation_revision: summary.observation_revision(),
        matched: false,
    });
    app.update_workflow_collection_status("workflow.compatibility", true).unwrap();

    let runtime = app.workflow_runtime_status["workflow.compatibility"].clone();
    assert_eq!(app.project_workflow_runtime(runtime.clone()).lifecycle.unwrap().phase, "running");
    assert_eq!(
        runtime.warnings.get(software_id.as_ref()).map(CommandError::code),
        Some("runtime.no_compatibility_signal"),
    );

    let sink = glyphshift_capture::FileCaptureSink::start(
        app.probe_runs.capture_configuration(run.summary.id(), DEFAULT_MAX_ENTRIES).unwrap(),
    ).unwrap();
    sink.observe(TEST_ADAPTER_ID, "Open menu");
    sink.finish().unwrap();
    app.collect_workflow_sources(run.summary.id()).unwrap();
    app.update_workflow_collection_status("workflow.compatibility", true).unwrap();

    assert!(app.workflow_runtime_status["workflow.compatibility"].warnings.is_empty());
    assert!(app.workflow_compatibility_checks[run.summary.id()].matched);
}

#[test]
fn workflow_legacy_pause_migrates_and_survives_backend_reopen() {
    let (mut app, _calls, software_id, root) = workflow_application();
    app.backend.create_workflow(WorkflowCreate::new("workflow.legacy", "Legacy").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.product"])
            .with_write_dictionary("dictionary.product"),
    ])).unwrap();
    let run = app.workflow_collection_view("workflow.legacy").unwrap();
    app.probe_runs.set_status(run.summary.id(), ProbeRunStatus::Paused).unwrap();
    let path = root.path().join("workflows-v4").join("workflow.legacy.json");
    let mut artifact: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    artifact["targets"][0].as_object_mut().unwrap().remove("collectNewSources");
    std::fs::write(&path, serde_json::to_vec(&artifact).unwrap()).unwrap();
    app.backend = DesktopBackend::open_with_environment(root.path(), test_desktop_environment()).unwrap();
    app.prepare_workflow_collection("workflow.legacy").unwrap();
    assert!(!app.backend.workflow("workflow.legacy").unwrap().targets()[0].collection_enabled());
    app.backend = DesktopBackend::open_with_environment(root.path(), test_desktop_environment()).unwrap();
    assert_eq!(app.backend.workflow("workflow.legacy").unwrap().targets()[0].collection_preference(), Some(false));
}

#[test]
fn workflow_transient_retry_is_bounded_and_hard_failure_requires_manual_retry() {
    let (mut app, _calls, _software_id, _root) = workflow_application();
    let mut runtime = app.enable_workflow("workflow.product", false).unwrap().runtime;
    runtime.targets.iter_mut().for_each(|target| target.active = false);
    runtime.errors.insert("synthetic".into(), CommandError::new("runtime.activation_timed_out"));
    runtime.retry_attempt = 2;
    runtime.retry_after_ms = 100;
    assert!(!crate::workflow_lifecycle::automatic_retry_allowed(&runtime, 99));
    assert!(crate::workflow_lifecycle::automatic_retry_allowed(&runtime, 100));
    runtime.retry_attempt = 3;
    assert!(!crate::workflow_lifecycle::automatic_retry_allowed(&runtime, 200));
    runtime.retry_attempt = 0;
    runtime.errors.clear();
    runtime.errors.insert("synthetic".into(), CommandError::new("runtime.component_load_failed"));
    runtime.retry_after_ms = 300;
    assert!(!crate::workflow_lifecycle::automatic_retry_allowed(&runtime, 299));
    assert!(crate::workflow_lifecycle::automatic_retry_allowed(&runtime, 300));
    runtime.retry_attempt = 3;
    assert!(!crate::workflow_lifecycle::automatic_retry_allowed(&runtime, 400));
    runtime.retry_attempt = 0;
    runtime.errors.clear();
    runtime.errors.insert("synthetic".into(), CommandError::new("runtime.target_restart_required"));
    assert!(!crate::workflow_lifecycle::automatic_retry_allowed(&runtime, 200));
}

#[test]
fn collection_filters_before_dictionary_write_and_rechecks_changed_policy() {
    let (mut app, _, software_id, _root) = workflow_application();
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.filtered", "Filtered", "en-US", "zh-CN")
        .with_entries([DictionaryEntryCreate::new("42", "既有数字")])).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.filtered", "Filtered").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.filtered"])
            .with_write_dictionary("dictionary.filtered"),
    ])).unwrap();
    let run = app.workflow_collection_view("workflow.filtered").unwrap();
    app.set_collection_filter_policy(glyphshift_ai_translation::FilterPolicy::default().with_excluded_patterns(["^Debug"]));
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(run.summary.id(), DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    for source in ["1234", "0.1818", "100 px", "https://example.invalid", "Ctrl+S", "Debug text", "Selected:0", "Open menu"] { sink.observe(TEST_ADAPTER_ID, source); }
    sink.finish().unwrap();
    app.collect_workflow_sources(run.summary.id()).unwrap();
    let dictionary = app.backend.dictionary("dictionary.filtered").unwrap();
    assert_eq!(dictionary.entries().iter().map(|entry| entry.source()).collect::<Vec<_>>(), vec!["42", "Open menu", "Selected:0"]);
    assert_eq!(dictionary.entries()[0].translation(), "既有数字");
    let revision = dictionary.revision();
    app.collect_workflow_sources(run.summary.id()).unwrap();
    assert_eq!(app.backend.dictionary("dictionary.filtered").unwrap().revision(), revision);
    let mut policy = serde_json::to_value(glyphshift_ai_translation::FilterPolicy::default()).unwrap();
    policy["skipPureNumbersOrSymbols"] = false.into();
    app.set_collection_filter_policy(serde_json::from_value(policy).unwrap());
    // Policy changes recheck persisted observations even when no new observation arrives.
    app.collect_workflow_sources(run.summary.id()).unwrap();
    let sources = app.backend.dictionary("dictionary.filtered").unwrap().entries().iter().map(|entry| entry.source()).collect::<Vec<_>>();
    assert!(sources.contains(&"1234"));
    assert!(sources.contains(&"Debug text"));
    assert!(!sources.contains(&"https://example.invalid"));
    assert!(!sources.contains(&"100 px"));
    app.set_collection_filter_policy(glyphshift_ai_translation::FilterPolicy::default());
    app.collect_workflow_sources(run.summary.id()).unwrap();
    assert!(app.backend.dictionary("dictionary.filtered").unwrap().entries().iter().any(|entry| entry.source() == "1234"));
}

#[test]
fn exit_stops_runtime_without_erasing_saved_intent_or_restarting_on_poll() {
    let (mut app, calls, _, _root) = workflow_application();
    app.enable_workflow("workflow.product", false).unwrap();
    let desired = app.backend.enabled_workflow_ids().to_vec();
    let activations = calls.lock().unwrap().enabled.len();
    app.prepare_exit().unwrap();
    assert!(calls.lock().unwrap().disabled.iter().any(|id| id.as_ref() == "workflow.product"));
    assert_eq!(app.backend.enabled_workflow_ids(), desired);
    app.refresh_workflows().unwrap();
    app.reconcile_enabled_workflows().unwrap();
    assert_eq!(calls.lock().unwrap().enabled.len(), activations);
    app.prepare_exit().unwrap();
}

#[test]
fn exit_failure_still_stops_other_workflows_and_allows_retry() {
    let (mut app, calls, software_id, _root) = workflow_application();
    app.enable_workflow("workflow.product", false).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.second", "Second").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.product"]),
    ])).unwrap();
    let intent = app.backend.effective_workflow_intent("workflow.second").unwrap();
    app.workflow_runtime_status.insert("workflow.second".into(), idle_workflow_runtime_view(&intent, false));
    calls.lock().unwrap().stop_failure_ids.insert("workflow.product".into());
    assert_eq!(serde_json::to_value(app.prepare_exit().unwrap_err()).unwrap()["code"], "runtime.exit_stop_failed");
    assert!(calls.lock().unwrap().disabled.iter().any(|id| id.as_ref() == "workflow.second"));
    assert!(app.exiting);
    calls.lock().unwrap().stop_failure_ids.clear();
    app.prepare_exit().unwrap();
}


#[test]
fn dictionary_rules_collect_one_label_and_ai_ignores_dynamic_counts() {
    let (mut app, _, software_id, _root) = workflow_application();
    let rule = glyphshift_translation::RegexTranslationRule { pattern: r"^(.+?)(:[0-9]+)$".into(), replacement: "{{TR}}$2".into(), enabled: true };
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.counter", "Counter", "en-US", "zh-CN")
        .with_text_rules(vec![rule.clone()])
        .with_entries([DictionaryEntryCreate::new("Total:18", "旧译文")])).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.counter", "Counter").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.counter"]).with_write_dictionary("dictionary.counter"),
    ])).unwrap();
    let run = app.workflow_collection_view("workflow.counter").unwrap();
    let id = run.summary.id();
    app.resume_probe_run(id).unwrap();
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(id, DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    for n in 0..101 { sink.observe(TEST_ADAPTER_ID, format!("Total:{n}")); }
    sink.finish().unwrap();
    app.collect_workflow_sources(id).unwrap();
    let dictionary = app.backend.dictionary("dictionary.counter").unwrap().clone();
    assert_eq!(dictionary.entries().len(), 2);
    assert!(dictionary.entries().iter().any(|entry| entry.source() == "Total" && entry.translation().is_empty()));
    let request = app.probe_ai_plan_request(id).unwrap();
    let plan = glyphshift_ai_translation::AiTranslation::new().plan_translation(request).unwrap();
    assert_eq!(plan.candidates().len(), 1);
    assert_eq!(plan.candidates()[0].source(), "Total");
    let applied = app.apply_probe_ai_results(ai::ProbeAiApplyRequest {
        run_id: id.into(), snapshot_revision: dictionary.revision(), results: vec![
            ai::ProbeAiTranslationResult { item_id: plan.candidates()[0].item_id().into(), source: "Total".into(), translation: "总计".into() },
        ],
    }).unwrap();
    assert_eq!(applied.applied_count, 1);
    let dictionary = app.backend.dictionary("dictionary.counter").unwrap().clone();
    let mut disabled = rule; disabled.enabled = false;
    app.backend.update_dictionary(DictionaryEdit::from_dictionary(&dictionary).with_text_rules(vec![disabled])).unwrap();
    app.collect_workflow_sources(id).unwrap();
    assert_eq!(app.backend.dictionary("dictionary.counter").unwrap().entries().len(), 102);
}

#[test]
fn dictionary_rule_manual_edits_write_fixed_key_and_refresh_all_counts() {
    let (mut app, _, software_id, _root) = workflow_application();
    let rule = glyphshift_translation::RegexTranslationRule { pattern: r"^(.+?)(:[0-9]+)$".into(), replacement: "{{TR}}$2".into(), enabled: true };
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.counter", "Counter", "en-US", "zh-CN").with_text_rules(vec![rule])).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.counter", "Counter").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.counter"]).with_write_dictionary("dictionary.counter"),
    ])).unwrap();
    let run = app.workflow_collection_view("workflow.counter").unwrap();
    let id = run.summary.id();
    app.resume_probe_run(id).unwrap();
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(id, DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    for n in 0..101 { sink.observe(TEST_ADAPTER_ID, format!("Total:{n}")); }
    sink.finish().unwrap();
    app.collect_workflow_sources(id).unwrap();
    let rows = |app: &mut DesktopApplication| {
        let request = serde_json::from_value(serde_json::json!({"runId":id,"search":"Total:18","adapterIds":[],"translationFilter":"all","page":1,"pageSize":50})).unwrap();
        serde_json::to_value(app.probe_run_entries(request).unwrap()).unwrap()
    };
    let pending = rows(&mut app);
    assert_eq!(pending["rows"][0]["resolution"]["editable"], true);
    assert_eq!(pending["rows"][0]["resolution"]["editSource"], "Total");
    for text in ["总计", "合计"] {
        app.edit_probe_translation(super::super::probe::ProbeTranslationEditRequest { run_id:id.into(), source:"Total:18".into(), translation:text.into() }).unwrap();
        let dictionary = app.backend.dictionary("dictionary.counter").unwrap();
        assert_eq!(dictionary.entries().len(), 1);
        assert_eq!(dictionary.entries()[0].source(), "Total");
        assert_eq!(dictionary.entries()[0].translation(), text);
        let page = rows(&mut app);
        assert_eq!(page["rows"][0]["translation"], format!("{text}:18"));
        assert_eq!(page["rows"][0]["resolution"]["editTranslation"], text);
    }
    app.edit_probe_translation(super::super::probe::ProbeTranslationEditRequest { run_id:id.into(), source:"Total:0".into(), translation:"".into() }).unwrap();
    assert!(app.backend.dictionary("dictionary.counter").unwrap().entries().is_empty());
    assert_eq!(rows(&mut app)["rows"][0]["resolution"]["kind"], "rule_pending");
    app.edit_probe_translation(super::super::probe::ProbeTranslationEditRequest { run_id:id.into(), source:"Total:100".into(), translation:"总数".into() }).unwrap();
    app.bulk_probe_entries(super::super::probe::ProbeBulkRequest { run_id:id.into(), sources:vec!["Total:0".into(),"Total:18".into()], action:"clear_translations".into() }).unwrap();
    assert!(app.backend.dictionary("dictionary.counter").unwrap().entries().is_empty());
    app.edit_probe_translation(super::super::probe::ProbeTranslationEditRequest { run_id:id.into(), source:"Total:18".into(), translation:"总数".into() }).unwrap();
    let dictionary = app.backend.dictionary("dictionary.counter").unwrap().clone();
    app.update_dictionary_with_capture_clear(DictionaryEdit::from_dictionary(&dictionary).with_entries([]), true).unwrap();
    app.collect_workflow_sources(id).unwrap();
    assert!(app.backend.dictionary("dictionary.counter").unwrap().entries().is_empty());
    assert_eq!(rows(&mut app)["total"], 0);

}

#[test]
fn clearing_dictionary_discards_old_capture_and_allows_recapture() {
    let (mut app, calls, software_id, root) = workflow_application();
    app.backend.create_workflow(WorkflowCreate::new("workflow.clear", "Clear").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.product"]).with_write_dictionary("dictionary.product"),
    ])).unwrap();
    let run = app.workflow_collection_view("workflow.clear").unwrap();
    let id = run.summary.id();
    app.resume_probe_run(id).unwrap();
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(id, DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    sink.observe(TEST_ADAPTER_ID, "Captured entry");
    sink.finish().unwrap();
    app.collect_workflow_sources(id).unwrap();
    let dictionary = app.backend.dictionary("dictionary.product").unwrap().clone();
    assert!(dictionary.entries().iter().any(|entry| entry.source() == "Captured entry"));
    app.update_dictionary_with_capture_clear(DictionaryEdit::from_dictionary(&dictionary).with_entries([]), true).unwrap();
    assert!(calls.lock().unwrap().disabled.iter().any(|id| id.as_ref() == "workflow.clear"));
    assert!(app.backend.enabled_workflow_ids().iter().any(|id| id.as_ref() == "workflow.clear"));
    assert!(calls.lock().unwrap().enabled.iter().filter(|(id, _, _)| id.as_ref() == "workflow.clear").count() >= 2);
    app.workflow_collection_view("workflow.clear").unwrap();
    app.collect_workflow_sources(id).unwrap();
    assert!(app.backend.dictionary("dictionary.product").unwrap().entries().is_empty());
    let request = serde_json::from_value(serde_json::json!({"runId":id,"search":"Captured entry","page":1,"pageSize":50})).unwrap();
    assert_eq!(app.probe_run_entries(request).unwrap().total(), 0);
    app.probe_runs = ProbeRunStore::open(root.path().join("probe-runs")).unwrap();
    app.collection_versions.clear();
    app.collect_workflow_sources(id).unwrap();
    assert!(app.backend.dictionary("dictionary.product").unwrap().entries().is_empty());
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(id, DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    sink.observe(TEST_ADAPTER_ID, "Captured entry");
    sink.observe(TEST_ADAPTER_ID, "Fresh entry");
    sink.finish().unwrap();
    app.collect_workflow_sources(id).unwrap();
    let dictionary = app.backend.dictionary("dictionary.product").unwrap();
    assert_eq!(dictionary.entries().len(), 2);
    assert!(dictionary.entries().iter().any(|entry| entry.source() == "Captured entry"));
    assert!(dictionary.entries().iter().any(|entry| entry.source() == "Fresh entry"));
}

#[test]
fn clearing_dynamic_source_resets_capture_without_blocking_rule_key() {
    let (mut app, _, software_id, _root) = workflow_application();
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.dynamic", "Dynamic", "en-US", "zh-CN")
        .with_entries([DictionaryEntryCreate::new("Total:18", "总计")])
        .with_text_rules(vec![glyphshift_translation::RegexTranslationRule { pattern:r"^(.+?)(:[0-9]+)$".into(), replacement:"{{TR}}$2".into(), enabled:true }])).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.dynamic", "Dynamic").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.dynamic"]).with_write_dictionary("dictionary.dynamic"),
    ])).unwrap();
    let run = app.workflow_collection_view("workflow.dynamic").unwrap();
    let id = run.summary.id();
    app.resume_probe_run(id).unwrap();
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(id, DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    sink.observe(TEST_ADAPTER_ID, "Total:18");
    sink.finish().unwrap();
    let dictionary = app.backend.dictionary("dictionary.dynamic").unwrap().clone();
    app.update_dictionary_with_capture_clear(DictionaryEdit::from_dictionary(&dictionary).with_entries([]), true).unwrap();
    app.collect_workflow_sources(id).unwrap();
    assert!(app.backend.dictionary("dictionary.dynamic").unwrap().entries().is_empty());
}

#[test]
fn clearing_empty_dictionary_hides_uncollected_workflow_rows() {
    let (mut app, _, software_id, _root) = workflow_application();
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.empty", "Empty", "en-US", "zh-CN")).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.empty", "Empty").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.empty"]).with_write_dictionary("dictionary.empty"),
    ])).unwrap();
    let run = app.workflow_collection_view("workflow.empty").unwrap();
    let id = run.summary.id();
    app.resume_probe_run(id).unwrap();
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(id, DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    sink.observe(TEST_ADAPTER_ID, "Uncollected entry");
    sink.finish().unwrap();
    let dictionary = app.backend.dictionary("dictionary.empty").unwrap().clone();
    app.update_dictionary_with_capture_clear(DictionaryEdit::from_dictionary(&dictionary).with_entries([]), true).unwrap();
    let request = serde_json::from_value(serde_json::json!({"runId":id,"search":"","page":1,"pageSize":50})).unwrap();
    assert_eq!(app.probe_run_entries(request).unwrap().total(), 0);
    assert_eq!(serde_json::to_value(app.probe_runs.summary(id).unwrap()).unwrap()["observedCount"], 0);
    app.collect_workflow_sources(id).unwrap();
    assert!(app.backend.dictionary("dictionary.empty").unwrap().entries().is_empty());
}

#[test]
fn rule_rows_merge_by_owner_rule_and_fixed_source_before_pagination() {
    let (mut app, _, software_id, _root) = workflow_application();
    let rule = glyphshift_translation::RegexTranslationRule { pattern:r"^(.+?)(:[0-9]+)$".into(), replacement:"{{TR}}$2".into(), enabled:true };
    app.backend.create_dictionary(DictionaryCreate::new("dictionary.groups", "Groups", "en-US", "zh-CN")
        .with_entries([DictionaryEntryCreate::new("Selected", "选择"), DictionaryEntryCreate::new("Total", "总计")])
        .with_text_rules(vec![rule.clone()])).unwrap();
    app.backend.create_workflow(WorkflowCreate::new("workflow.groups", "Groups").with_targets([
        WorkflowTargetCreate::new(software_id, [TEST_ADAPTER_ID], ["dictionary.groups"]).with_write_dictionary("dictionary.groups"),
    ])).unwrap();
    let run = app.workflow_collection_view("workflow.groups").unwrap();
    let id = run.summary.id();
    app.resume_probe_run(id).unwrap();
    let sink = glyphshift_capture::FileCaptureSink::start(app.probe_runs.capture_configuration(id, DEFAULT_MAX_ENTRIES).unwrap()).unwrap();
    for n in 0..20 { sink.observe(TEST_ADAPTER_ID, format!("Selected:{n}")); }
    sink.observe(TEST_ADAPTER_ID, "Total:20");
    sink.finish().unwrap();
    let rows = |app: &mut DesktopApplication, merge: bool, search: &str, size: usize| {
        let request = serde_json::from_value(serde_json::json!({"runId":id,"search":search,"mergeRules":merge,"page":1,"pageSize":size})).unwrap();
        serde_json::to_value(app.probe_run_entries(request).unwrap()).unwrap()
    };
    assert_eq!(rows(&mut app, false, "", 50)["total"], 21);
    let merged = rows(&mut app, true, "", 1);
    assert_eq!(merged["total"], 2);
    assert_eq!(merged["rows"].as_array().unwrap().len(), 1);
    let selected = rows(&mut app, true, "选择", 50);
    assert_eq!(selected["total"], 1);
    assert_eq!(selected["rows"][0]["count"], 20);
    assert_eq!(selected["rows"][0]["resolution"]["editSource"], "Selected");
    assert_eq!(rows(&mut app, true, "Selected:18", 50)["rows"][0]["source"], "Selected:18");
    let dictionary = app.backend.dictionary("dictionary.groups").unwrap().clone();
    let mut disabled = rule; disabled.enabled = false;
    app.update_dictionary(DictionaryEdit::from_dictionary(&dictionary).with_text_rules(vec![disabled])).unwrap();
    assert_eq!(rows(&mut app, true, "", 50)["total"], 21);
}
