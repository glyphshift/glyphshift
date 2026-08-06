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
fn isolated_worker_permission_and_timeout_reach_actionable_command_errors() {
    let denied = serde_json::to_value(runtime_command_error(
        DesktopRuntimeError::ActivationRejected(
            HostOperationFailure::IsolatedWorkerPermissionDenied,
        ),
        true,
    ))
    .expect("serialize isolated Worker permission rejection");
    let timeout = serde_json::to_value(runtime_command_error(
        DesktopRuntimeError::ActivationRejected(HostOperationFailure::IsolatedWorkerTimeout),
        true,
    ))
    .expect("serialize isolated Worker timeout");

    assert_eq!(denied["code"], "runtime.target_access_failed");
    assert_eq!(timeout["code"], "runtime.activation_timed_out");
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
    application.adapters.push(AdapterView {
        id: TEST_ADAPTER_ID.into(),
        name: "Synthetic adapter".into(),
        version: "1.0.0".into(),
        summary: "Synthetic adapter presentation".into(),
        platforms: vec!["windows".into()],
        technologies: vec!["Synthetic".into()],
        features: vec!["textObserve".into()],
        technical_target: "SyntheticTarget".into(),
        documentation_url: Some("https://example.invalid/adapter".into()),
        configuration: "none".into(),
    });

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
        workflow_runtime_status: BTreeMap::new(),
        adapters: Vec::new(),
        font_families: Vec::new(),
        font_cache_root: data_root.path().to_path_buf(),
        probe_runs: ProbeRunStore::open(data_root.path().join("probe-runs"))
            .expect("probe run store"),
        active_probe_run_id: None,
        active_probe_capability: None,
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
