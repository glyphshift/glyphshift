use super::*;

#[test]
fn quick_capture_arms_then_captures_and_can_be_cancelled() {
    let mut capture = SoftwareQuickCaptureState {
        shortcut_available: true,
        ..Default::default()
    };

    assert_eq!(capture.press(), SoftwareQuickCaptureTransition::Armed);
    assert!(capture.armed);
    assert_eq!(capture.press(), SoftwareQuickCaptureTransition::Capture);
    assert!(!capture.armed);

    capture.arm().expect("arm capture from the software page");
    assert!(capture.armed);
    capture.cancel();
    assert!(!capture.armed);
}

#[test]
fn software_preflight_keeps_each_deterministic_failure_distinct() {
    assert_eq!(
        software_preflight_state(true, false, true, true, true),
        SoftwarePreflightState::SelfTarget
    );
    assert_eq!(
        software_preflight_state(false, true, true, true, true),
        SoftwarePreflightState::AlreadyAdded
    );
    assert_eq!(
        software_preflight_state(false, false, false, true, true),
        SoftwarePreflightState::UnsupportedArchitecture
    );
    assert_eq!(
        software_preflight_state(false, false, true, false, true),
        SoftwarePreflightState::RuntimeUnavailable
    );
    assert_eq!(
        software_preflight_state(false, false, true, true, false),
        SoftwarePreflightState::NotRunning
    );
    assert_eq!(
        software_preflight_state(false, false, true, true, true),
        SoftwarePreflightState::Ready
    );
}

#[test]
fn software_preflight_accepts_an_architecture_with_an_isolated_observer() {
    let (mut application, _calls, _software_id, _data_root) = workflow_application();

    application.adapter_target_support.insert(
        "test.native".into(),
        AdapterTargetSupport {
            placement: Placement::TargetProcess,
            platforms: vec!["windows".into()],
            architectures: vec!["x86".into(), "x86_64".into()],
            features: vec![Feature::TextObserve],
        },
    );

    assert!(application.supports_probe_architecture("x86_64"));
    assert!(!application.supports_probe_architecture("x86"));

    application.adapter_target_support.insert(
        "test.isolated".into(),
        AdapterTargetSupport {
            placement: Placement::IsolatedWorker,
            platforms: vec!["windows".into()],
            architectures: vec!["x86".into(), "x86_64".into()],
            features: vec![Feature::TextObserve],
        },
    );

    assert!(application.supports_probe_architecture("x86"));
    assert!(!application.supports_probe_architecture("aarch64"));
}

#[test]
fn software_mutations_return_the_same_product_snapshot_shape() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    let executable_path = application.backend.snapshot().software()[0]
        .executable_path()
        .expect("bound executable path")
        .to_owned();

    let snapshot = application
        .update_software(
            software_id.to_string(),
            "合成软件 2".to_owned(),
            "默认合成场景".to_owned(),
            executable_path,
        )
        .expect("update software through product command");

    assert_eq!(snapshot.configuration.software()[0].name(), "合成软件 2");
    assert_eq!(
        snapshot.configuration.software()[0].description(),
        "默认合成场景"
    );
    assert!(snapshot
        .workflow_runtime_status
        .contains_key("workflow.product"));
}

#[test]
fn software_delete_reports_references_before_touching_the_runtime() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe.software-reference".into(),
            name: "Software reference".into(),
            software_id: software_id.clone(),
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create a probe that references the software");

    let error = application
        .remove_software(&software_id)
        .expect_err("referenced software must be preserved");
    let serialized = serde_json::to_value(error).expect("serialize software reference error");

    assert_eq!(serialized["code"], "software.referenced");
    assert_eq!(serialized["args"]["workflowCount"], 1);
    assert_eq!(serialized["args"]["probeCount"], 1);
    assert!(calls
        .lock()
        .expect("runtime call log")
        .software_removed
        .is_empty());
}

#[test]
fn software_delete_removes_an_unreferenced_record_after_runtime_cleanup() {
    let (mut application, calls, _software_id, data_root) = workflow_application();
    let executable = data_root.path().join("DisposableHost.exe");
    fs::write(&executable, b"synthetic disposable executable")
        .expect("write disposable executable");
    let added = application
        .backend
        .add_software(ExecutableSelection::new(&executable))
        .expect("add disposable software");
    let disposable_id: Box<str> = added
        .software()
        .iter()
        .find(|software| software.executable_name() == "DisposableHost.exe")
        .expect("find disposable software")
        .id()
        .into();

    let snapshot = application
        .remove_software(&disposable_id)
        .expect("remove unreferenced software");

    assert!(snapshot
        .configuration
        .software()
        .iter()
        .all(|software| software.id() != disposable_id.as_ref()));
    assert_eq!(
        calls.lock().expect("runtime call log").software_removed,
        vec![disposable_id]
    );
}
