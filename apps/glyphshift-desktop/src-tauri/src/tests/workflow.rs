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
        collection_versions: Default::default(),
        pending_collection_runs: Default::default(),
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
    runtime.errors.insert("synthetic".into(), CommandError::new("runtime.target_restart_required"));
    app.workflow_runtime_status.insert("workflow.lifecycle".into(), runtime);
    let before = calls.lock().unwrap().refreshed.len();
    app.refresh_workflows_with_retry(false).unwrap();
    assert_eq!(calls.lock().unwrap().refreshed.len(), before);
    app.refresh_workflows_with_retry(true).unwrap();
    assert_eq!(calls.lock().unwrap().refreshed.len(), before + 1);
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
    runtime.errors.insert("synthetic".into(), CommandError::new("runtime.target_restart_required"));
    assert!(!crate::workflow_lifecycle::automatic_retry_allowed(&runtime, 200));
}
