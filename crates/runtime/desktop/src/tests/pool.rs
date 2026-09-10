use super::*;

#[test]
fn point_acquisition_uses_a_short_lived_runtime_without_creating_a_pool_session() {
    let root = tempdir().expect("point acquisition Runtime data");
    let executable = root.path().join("PointHost.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    let spec = backend
        .runtime_spec(&software_id)
        .expect("point acquisition Runtime spec");
    let discoveries = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(AcquisitionRuntimeFactory {
        discoveries: Arc::clone(&discoveries),
        calls: Arc::clone(&calls),
    }));

    let result = pool
        .acquire_point(
            software_id.as_str(),
            &spec,
            1,
            "test.acquire",
            DesktopPoint::new(20, 30),
            &DesktopAcquisitionCancellation::new(),
        )
        .expect("point acquisition");

    assert_eq!(result.blocks()[0].source(), "Open");
    assert!(pool.status(&software_id).is_none());
    assert_eq!(discoveries.load(Ordering::SeqCst), 1);
    assert_eq!(
        calls.lock().expect("acquisition calls").as_slice(),
        &[(1, 1, Box::<str>::from("test.acquire"))]
    );
}

#[test]
fn primary_point_acquisition_selects_the_first_opaque_target_inside_the_runtime_boundary() {
    let root = tempdir().expect("primary point Runtime data");
    let executable = root.path().join("PrimaryPointHost.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    let spec = backend
        .runtime_spec(&software_id)
        .expect("point acquisition Runtime spec");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(AcquisitionRuntimeFactory {
        discoveries: Arc::new(AtomicUsize::new(0)),
        calls: Arc::clone(&calls),
    }));

    pool.acquire_primary_point(
        software_id,
        &spec,
        "test.acquire",
        DesktopPoint::new(20, 30),
        &DesktopAcquisitionCancellation::new(),
    )
    .expect("primary point acquisition");

    assert_eq!(calls.lock().expect("acquisition calls")[0].1, 1);
}

#[test]
fn primary_region_acquisition_uses_the_same_short_lived_runtime_boundary() {
    let root = tempdir().expect("primary region Runtime data");
    let executable = root.path().join("PrimaryRegionHost.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    let spec = backend
        .runtime_spec(&software_id)
        .expect("region acquisition Runtime spec");
    let discoveries = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(AcquisitionRuntimeFactory {
        discoveries: Arc::clone(&discoveries),
        calls: Arc::clone(&calls),
    }));

    let result = pool
        .acquire_primary_region(
            software_id.as_str(),
            &spec,
            "test.acquire",
            DesktopRect::new(10, 20, 650, 260).expect("OCR region"),
            &DesktopAcquisitionCancellation::new(),
        )
        .expect("primary region acquisition");

    assert_eq!(result.blocks()[0].provenance(), Provenance::Visual);
    assert_eq!(discoveries.load(Ordering::SeqCst), 1);
    assert_eq!(calls.lock().expect("acquisition calls")[0].1, 1);
    assert!(pool.status(&software_id).is_none());
}

#[test]
fn point_acquisition_does_not_reuse_or_mutate_an_active_workflow_runtime() {
    let root = tempdir().expect("workflow acquisition Runtime data");
    let executable = root.path().join("WorkflowPointHost.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.acquire", "取词词典", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
        )
        .expect("acquisition dictionary");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.acquire", "取词工作流").with_targets([
                WorkflowTargetCreate::new(
                    software_id.as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.acquire"],
                ),
            ]),
        )
        .expect("acquisition workflow");
    let intent = backend
        .effective_workflow_intent("workflow.acquire")
        .expect("workflow intent");
    let spec = backend
        .runtime_spec(&software_id)
        .expect("point acquisition Runtime spec");
    let discoveries = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(AcquisitionRuntimeFactory {
        discoveries: Arc::clone(&discoveries),
        calls: Arc::clone(&calls),
    }));
    pool.reconcile_workflow(&intent).expect("active workflow");

    pool.acquire_point(
        software_id.as_str(),
        &spec,
        1,
        "test.acquire",
        DesktopPoint::new(20, 30),
        &DesktopAcquisitionCancellation::new(),
    )
    .expect("point acquisition beside workflow");

    let status = pool.status(&software_id).expect("workflow Runtime remains");
    assert!(status.is_feature_active(Feature::TextReplace));
    assert_eq!(discoveries.load(Ordering::SeqCst), 2);
    assert_eq!(calls.lock().expect("acquisition calls")[0].0, 2);
}

#[test]
fn point_acquisition_does_not_reuse_or_mutate_an_active_capture_runtime() {
    let root = tempdir().expect("capture acquisition Runtime data");
    let executable = root.path().join("CapturePointHost.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    let spec = backend
        .capture_runtime_spec(&software_id, &[Box::<str>::from(TEST_ADAPTER_ID)])
        .expect("capture Runtime spec");
    let capture = CaptureConfiguration::new(
        glyphshift_capture::CaptureSessionId::new("capture-acquisition").expect("session id"),
        root.path().join("capture.json"),
        100,
    )
    .expect("capture configuration");
    let discoveries = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(AcquisitionRuntimeFactory {
        discoveries: Arc::clone(&discoveries),
        calls: Arc::clone(&calls),
    }));
    pool.start_capture(software_id.as_str(), &spec, None, capture)
        .expect("active capture");

    pool.acquire_point(
        software_id.as_str(),
        &spec,
        1,
        "test.acquire",
        DesktopPoint::new(20, 30),
        &DesktopAcquisitionCancellation::new(),
    )
    .expect("point acquisition beside capture");

    let status = pool.status(&software_id).expect("capture Runtime remains");
    assert!(status.is_feature_active(Feature::TextObserve));
    assert_eq!(discoveries.load(Ordering::SeqCst), 2);
    assert_eq!(calls.lock().expect("acquisition calls")[0].0, 2);
}

#[test]
fn cancelled_point_acquisition_does_not_discover_a_runtime() {
    let root = tempdir().expect("cancelled acquisition Runtime data");
    let executable = root.path().join("CancelledPointHost.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    let spec = backend
        .runtime_spec(&software_id)
        .expect("point acquisition Runtime spec");
    let discoveries = Arc::new(AtomicUsize::new(0));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(AcquisitionRuntimeFactory {
        discoveries: Arc::clone(&discoveries),
        calls: Arc::new(Mutex::new(Vec::new())),
    }));
    let cancellation = DesktopAcquisitionCancellation::new();
    cancellation.cancel();

    assert_eq!(
        pool.acquire_point(
            software_id,
            &spec,
            1,
            "test.acquire",
            DesktopPoint::new(20, 30),
            &cancellation,
        ),
        Err(DesktopAcquisitionError::Cancelled)
    );
    assert_eq!(discoveries.load(Ordering::SeqCst), 0);
}

#[test]
fn single_workflows_stop_only_their_owned_software() {
    let root = tempdir().expect("workflow Runtime data");
    let mut backend = open_test_backend(root.path().join("data"));
    let mut software_ids = Vec::new();
    for executable_name in ["Alpha.exe", "Beta.exe", "Gamma.exe"] {
        let executable = root.path().join(executable_name);
        fs::write(&executable, b"synthetic executable").expect("selected executable");
        let snapshot = backend
            .add_software(ExecutableSelection::new(executable))
            .expect("registered executable");
        software_ids.push(
            snapshot
                .selected_software_id()
                .expect("selected software")
                .to_owned(),
        );
    }
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.shared", "共享词典", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
        )
        .expect("shared dictionary");
    let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
    for (index, software) in software_ids.iter().enumerate() {
        let id = format!("workflow.single-{index}");
        create_single_workflow(&mut backend, &id, software, "dictionary.shared");
        assert!(pool
            .reconcile_workflow(&backend.effective_workflow_intent(&id).unwrap())
            .unwrap()
            .errors()
            .is_empty());
    }
    pool.stop_workflow("workflow.single-0").unwrap();
    assert!(pool.status(&software_ids[0]).is_none());
    for software in &software_ids[1..] {
        assert!(pool
            .status(software)
            .unwrap()
            .is_feature_active(Feature::TextReplace));
    }
}

#[test]
fn capture_owns_one_software_and_cannot_overlap_a_translation_workflow() {
    let root = tempdir().expect("capture Runtime data");
    let executable = root.path().join("CaptureHost.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.capture", "捕获冲突词典", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
        )
        .expect("capture conflict dictionary");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.capture", "捕获冲突工作流").with_targets([
                WorkflowTargetCreate::new(
                    software_id.clone(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.capture"],
                ),
            ]),
        )
        .expect("capture conflict workflow");
    let spec = backend
        .capture_runtime_spec(&software_id, &[Box::<str>::from(TEST_ADAPTER_ID)])
        .expect("capture spec");
    let intent = backend
        .effective_workflow_intent("workflow.capture")
        .expect("workflow intent");
    let configuration = CaptureConfiguration::new(
        glyphshift_capture::CaptureSessionId::new("capture-test").expect("session id"),
        root.path().join("capture.json"),
        100,
    )
    .expect("capture configuration");
    let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));

    let active = pool
        .start_capture(software_id.as_str(), &spec, None, configuration)
        .expect("start capture");
    assert!(active.is_feature_active(Feature::TextObserve));
    pool.control_runtime_diagnostics(software_id.as_str(), true)
        .expect("enable desktop runtime diagnostics");
    let diagnostics = pool
        .query_runtime_diagnostics(software_id.as_str())
        .expect("query desktop runtime diagnostics");
    assert_eq!(diagnostics.records().len(), 1);
    assert_eq!(diagnostics.records()[0].source_text(), "Open");
    assert_eq!(diagnostics.dropped(), 3);
    assert_eq!(
        pool.reconcile_workflow(&intent),
        Err(DesktopRuntimeError::TargetInUse(
            TargetExecutionOwner::Capture
        ))
    );

    pool.stop_capture(software_id.as_str())
        .expect("stop capture");
    assert!(pool
        .reconcile_workflow(&intent)
        .expect("start workflow after capture")
        .errors()
        .is_empty());
    let workflow_conflict_configuration = CaptureConfiguration::new(
        glyphshift_capture::CaptureSessionId::new("capture-workflow-conflict").expect("session id"),
        root.path().join("capture-workflow-conflict.json"),
        100,
    )
    .expect("workflow conflict capture configuration");
    assert_eq!(
        pool.start_capture(
            software_id.as_str(),
            &spec,
            None,
            workflow_conflict_configuration,
        ),
        Err(DesktopRuntimeError::TargetInUse(
            TargetExecutionOwner::Workflow
        ))
    );
}

#[test]
fn process_family_capture_selects_every_discovered_target_when_not_narrowed() {
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../local-test/evidence/desktop-runtime-family-capture");
    fs::create_dir_all(&local_test).expect("family capture local-test root");
    let root = tempfile::Builder::new()
        .prefix("contract-")
        .tempdir_in(local_test)
        .expect("family capture product data");
    let executable = root.path().join("SyntheticFamilyHost.exe");
    fs::write(&executable, b"synthetic executable").expect("family capture executable");
    let mut backend = open_test_backend(root.path());
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("register family capture software")
        .selected_software_id()
        .expect("selected family capture software")
        .to_owned();
    let spec = backend
        .capture_runtime_spec(&software_id, &[Box::<str>::from(TEST_ADAPTER_ID)])
        .expect("family capture Runtime spec");
    let captured_target_ids = Arc::new(Mutex::new(Vec::new()));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(FamilyCaptureFactory {
        captured_target_ids: captured_target_ids.clone(),
    }));
    let capture = CaptureConfiguration::new(
        glyphshift_capture::CaptureSessionId::new("family-capture")
            .expect("family capture session id"),
        root.path().join("capture.json"),
        100,
    )
    .expect("family capture configuration");

    pool.start_capture(software_id.clone(), &spec, None, capture)
        .expect("start every family target");
    assert_eq!(
        *captured_target_ids.lock().expect("captured family targets"),
        vec![1, 2, 3]
    );
    pool.stop_capture(software_id).expect("stop family capture");
}

#[test]
fn failed_capture_start_is_discarded_so_connect_and_continue_can_retry() {
    let root = tempdir().expect("capture retry Runtime data");
    let executable = root.path().join("RetryCaptureHost.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    let spec = backend
        .capture_runtime_spec(&software_id, &[Box::<str>::from(TEST_ADAPTER_ID)])
        .expect("capture spec");
    let configuration = CaptureConfiguration::new(
        glyphshift_capture::CaptureSessionId::new("capture-retry").expect("session id"),
        root.path().join("capture.json"),
        100,
    )
    .expect("capture configuration");
    let discoveries = Arc::new(AtomicUsize::new(0));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(RetryRuntimeFactory {
        discoveries: Arc::clone(&discoveries),
    }));

    assert_eq!(
        pool.start_capture(software_id.as_str(), &spec, None, configuration.clone()),
        Err(DesktopRuntimeError::ProtocolRejected)
    );
    let connected = pool
        .start_capture(software_id.as_str(), &spec, None, configuration)
        .expect("retry should rediscover a clean Runtime");

    assert!(connected.is_feature_active(Feature::TextObserve));
    assert_eq!(discoveries.load(Ordering::SeqCst), 2);
}

#[test]
fn abandoned_capture_is_discarded_so_resume_can_discover_a_fresh_runtime() {
    let root = tempdir().expect("abandoned capture Runtime data");
    let executable = root.path().join("AbandonedCaptureHost.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    let spec = backend
        .capture_runtime_spec(&software_id, &[Box::<str>::from(TEST_ADAPTER_ID)])
        .expect("capture spec");
    let first_configuration = CaptureConfiguration::new(
        glyphshift_capture::CaptureSessionId::new("capture-before-abandon")
            .expect("first session id"),
        root.path().join("capture-before-abandon.json"),
        100,
    )
    .expect("first capture configuration");
    let second_configuration = CaptureConfiguration::new(
        glyphshift_capture::CaptureSessionId::new("capture-after-abandon")
            .expect("second session id"),
        root.path().join("capture-after-abandon.json"),
        100,
    )
    .expect("second capture configuration");
    let discoveries = Arc::new(AtomicUsize::new(0));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(AcquisitionRuntimeFactory {
        discoveries: Arc::clone(&discoveries),
        calls: Arc::new(Mutex::new(Vec::new())),
    }));

    pool.start_capture(software_id.as_str(), &spec, None, first_configuration)
        .expect("start capture before target loss");
    pool.abandon_capture(software_id.as_str());

    assert!(pool.status(&software_id).is_none());
    let resumed = pool
        .start_capture(software_id.as_str(), &spec, None, second_configuration)
        .expect("resume should discover a fresh Runtime");
    assert!(resumed.is_feature_active(Feature::TextObserve));
    assert_eq!(discoveries.load(Ordering::SeqCst), 2);
}

#[test]
fn offline_probe_start_is_discarded_so_reopened_target_can_reconnect() {
    let root = tempdir().expect("offline capture Runtime data");
    let executable = root.path().join("ReopenedCaptureHost.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    let spec = backend
        .capture_runtime_spec(&software_id, &[Box::<str>::from(TEST_ADAPTER_ID)])
        .expect("capture spec");
    let configuration = CaptureConfiguration::new(
        glyphshift_capture::CaptureSessionId::new("capture-after-reopen").expect("session id"),
        root.path().join("capture.json"),
        100,
    )
    .expect("capture configuration");
    let discoveries = Arc::new(AtomicUsize::new(0));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(OfflineThenRunningRuntimeFactory {
        discoveries: Arc::clone(&discoveries),
    }));

    assert_eq!(
        pool.start_capture(software_id.as_str(), &spec, None, configuration.clone()),
        Err(DesktopRuntimeError::UnknownTarget)
    );
    let connected = pool
        .start_capture(software_id.as_str(), &spec, None, configuration)
        .expect("reopened target should trigger fresh discovery");

    assert!(connected.is_feature_active(Feature::TextObserve));
    assert_eq!(discoveries.load(Ordering::SeqCst), 2);
}

#[test]
fn single_workflows_publish_shared_dictionary_updates_to_each_software() {
    let root = tempdir().expect("workflow Runtime data");
    let mut backend = open_test_backend(root.path().join("data"));
    let mut software_ids = Vec::new();
    for executable_name in ["First.exe", "Second.exe"] {
        let executable = root.path().join(executable_name);
        fs::write(&executable, b"synthetic executable").expect("selected executable");
        let snapshot = backend
            .add_software(ExecutableSelection::new(executable))
            .expect("registered executable");
        software_ids.push(
            snapshot
                .selected_software_id()
                .expect("selected software")
                .to_owned(),
        );
    }
    let dictionary = backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.hot", "共享热更新词典", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "第一次")]),
        )
        .expect("shared dictionary");
    let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
    for (index, software) in software_ids.iter().enumerate() {
        let id = format!("workflow.hot-{index}");
        create_single_workflow(&mut backend, &id, software, "dictionary.hot");
        pool.reconcile_workflow(&backend.effective_workflow_intent(&id).unwrap())
            .unwrap();
    }
    backend
        .update_dictionary(
            DictionaryEdit::new(
                dictionary.id(),
                dictionary.metadata().name(),
                dictionary.metadata().source_locale(),
                dictionary.metadata().target_locale(),
                dictionary.revision(),
            )
            .with_entries([DictionaryEntryCreate::new("Open", "第二次")]),
        )
        .unwrap();
    for index in 0..software_ids.len() {
        let updated = pool
            .reconcile_workflow(
                &backend
                    .effective_workflow_intent(&format!("workflow.hot-{index}"))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(updated.statuses().len(), 1);
        assert_eq!(
            updated.statuses()[0].applied_generation(),
            Some(glyphshift_domain::Generation::new(3))
        );
    }
}

#[test]
fn explicit_workflow_replacement_removes_the_entire_old_intent() {
    let root = tempdir().expect("workflow Runtime data");
    let mut backend = open_test_backend(root.path().join("data"));
    let mut software_ids = Vec::new();
    for executable_name in ["OldPrimary.exe", "OldSecondary.exe"] {
        let executable = root.path().join(executable_name);
        fs::write(&executable, b"synthetic executable").expect("selected executable");
        let snapshot = backend
            .add_software(ExecutableSelection::new(executable))
            .expect("registered executable");
        software_ids.push(
            snapshot
                .selected_software_id()
                .expect("selected software")
                .to_owned(),
        );
    }
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.replace", "替换词典", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
        )
        .expect("replacement dictionary");
    create_single_workflow(
        &mut backend,
        "workflow.old",
        &software_ids[0],
        "dictionary.replace",
    );
    create_single_workflow(
        &mut backend,
        "workflow.other",
        &software_ids[1],
        "dictionary.replace",
    );
    create_single_workflow(
        &mut backend,
        "workflow.new",
        &software_ids[0],
        "dictionary.replace",
    );
    let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
    for id in ["workflow.old", "workflow.other"] {
        pool.reconcile_workflow(&backend.effective_workflow_intent(id).unwrap())
            .unwrap();
    }
    let replaced = pool
        .replace_workflow(&backend.effective_workflow_intent("workflow.new").unwrap())
        .unwrap();
    assert!(replaced.errors().is_empty());
    for software in &software_ids {
        assert!(pool
            .status(software)
            .unwrap()
            .is_feature_active(Feature::TextReplace));
    }
    assert!(pool.stop_workflow("workflow.old").unwrap().errors().is_empty());
    pool.stop_workflow("workflow.new").unwrap();
    assert!(pool.status(&software_ids[0]).is_none());
    assert!(pool.status(&software_ids[1]).is_some());
}

#[test]
fn workflow_status_refresh_preserves_requested_features() {
    let root = tempdir().expect("workflow Runtime data");
    let executable = root.path().join("Restarted.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let snapshot = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable");
    let software_id = snapshot
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.restart", "重启词典", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "重连")]),
        )
        .expect("restart dictionary");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.restart", "重启工作流").with_targets([
                WorkflowTargetCreate::new(
                    software_id.as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.restart"],
                ),
            ]),
        )
        .expect("restart workflow");
    let intent = backend
        .effective_workflow_intent("workflow.restart")
        .expect("restart intent");
    let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
    pool.reconcile_workflow(&intent).expect("initial reconcile");

    let refreshed = pool
        .refresh_workflow(&intent)
        .expect("refresh restarted target");

    assert!(refreshed.errors().is_empty());
    let status = pool.status(&software_id).expect("refreshed status");
    assert!(status.is_feature_requested(Feature::TextReplace));
    assert!(status.is_feature_active(Feature::TextReplace));
    assert_eq!(status.applied_generation(), Some(Generation::new(2)));
}

#[test]
fn workflow_stop_failure_keeps_one_last_applied_target_without_rolling_back_others() {
    let root = tempdir().expect("workflow Runtime data");
    let mut backend = open_test_backend(root.path().join("data"));
    let mut software_ids = Vec::new();
    for executable_name in ["StopFailure.exe", "StopSuccess.exe"] {
        let executable = root.path().join(executable_name);
        fs::write(&executable, b"synthetic executable").expect("selected executable");
        let snapshot = backend
            .add_software(ExecutableSelection::new(executable))
            .expect("registered executable");
        software_ids.push(
            snapshot
                .selected_software_id()
                .expect("selected software")
                .to_owned(),
        );
    }
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.stop", "停止词典", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "停止")]),
        )
        .expect("stop dictionary");
    let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
    for (index, software) in software_ids.iter().enumerate() {
        let id = format!("workflow.stop-{index}");
        create_single_workflow(&mut backend, &id, software, "dictionary.stop");
        pool.reconcile_workflow(&backend.effective_workflow_intent(&id).unwrap())
            .unwrap();
    }
    let stopped = pool.stop_workflow("workflow.stop-0").unwrap();
    assert_eq!(
        stopped.errors().get(software_ids[0].as_str()),
        Some(&DesktopRuntimeError::SessionRejected)
    );
    assert!(pool.workflow_owns_target("workflow.stop-0", &software_ids[0]));
    assert!(!pool.workflow_owns_target("workflow.stop-1", &software_ids[0]));
    let retried = pool.stop_workflow("workflow.stop-0").unwrap();
    assert_eq!(retried.errors().get(software_ids[0].as_str()), Some(&DesktopRuntimeError::SessionRejected));
    assert!(pool
        .status(&software_ids[1])
        .unwrap()
        .is_feature_active(Feature::TextReplace));
    assert!(pool
        .stop_workflow("workflow.stop-1")
        .unwrap()
        .errors()
        .is_empty());
    assert!(pool.status(&software_ids[1]).is_none());
    assert!(pool
        .status(&software_ids[0])
        .unwrap()
        .is_feature_active(Feature::TextReplace));
}

#[test]
fn workflow_collection_shares_translation_session_and_can_be_disabled() {
    let root = tempdir().unwrap();
    let executable = root.path().join("SyntheticCollectionHost.exe");
    fs::write(&executable, b"synthetic").unwrap();
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .unwrap()
        .selected_software_id()
        .unwrap()
        .to_owned();
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.collection", "Collection", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "Translation")]),
        )
        .unwrap();
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.collection", "Collection").with_targets([
                WorkflowTargetCreate::new(
                    software_id.clone(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.collection"],
                ),
            ]),
        )
        .unwrap();
    let intent = backend
        .effective_workflow_intent("workflow.collection")
        .unwrap();
    let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
    let capture = CaptureConfiguration::new(
        glyphshift_capture::CaptureSessionId::new("collection").unwrap(),
        root.path().join("entries.json"),
        100,
    )
    .unwrap();
    pool.configure_workflow_collection(
        "workflow.collection",
        [(software_id.clone().into(), capture)].into(),
    );
    assert!(pool
        .reconcile_workflow(&intent)
        .unwrap()
        .errors()
        .is_empty());
    let status = pool.status(&software_id).unwrap();
    assert!(status.is_feature_active(Feature::TextObserve));
    assert!(status.is_feature_active(Feature::TextReplace));
    assert!(pool
        .reconcile_workflow(&intent)
        .unwrap()
        .errors()
        .is_empty());
    pool.configure_workflow_collection("workflow.collection", BTreeMap::new());
    assert!(pool
        .reconcile_workflow(&intent)
        .unwrap()
        .errors()
        .is_empty());
    assert!(pool
        .status(&software_id)
        .unwrap()
        .is_feature_active(Feature::TextReplace));
    assert!(!pool
        .status(&software_id)
        .unwrap()
        .is_feature_active(Feature::TextObserve));
    assert!(pool
        .stop_workflow("workflow.collection")
        .unwrap()
        .errors()
        .is_empty());
    assert!(!pool
        .status(&software_id)
        .is_some_and(|status| status.is_active()));
}

fn create_single_workflow(
    backend: &mut DesktopBackend,
    id: &str,
    software: &str,
    dictionary: &str,
) {
    backend
        .create_workflow(
            WorkflowCreate::new(id, id).with_targets([WorkflowTargetCreate::new(
                software,
                [TEST_ADAPTER_ID],
                [dictionary],
            )]),
        )
        .unwrap();
}

#[test]
fn failed_workflow_collection_start_is_discarded_before_retry() {
    let root = tempdir().expect("capture retry Runtime data");
    let executable = root.path().join("RetryCaptureHost.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let software_id = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    backend.create_dictionary(DictionaryCreate::new("dictionary.retry", "Retry", "en-US", "zh-CN")
        .with_entries([DictionaryEntryCreate::new("Open", "Translated")])).unwrap();
    create_single_workflow(&mut backend, "workflow.retry", &software_id, "dictionary.retry");
    let intent = backend.effective_workflow_intent("workflow.retry").unwrap();
    let configuration = CaptureConfiguration::new(
        glyphshift_capture::CaptureSessionId::new("capture-retry").expect("session id"),
        root.path().join("capture.json"),
        100,
    )
    .expect("capture configuration");
    let discoveries = Arc::new(AtomicUsize::new(0));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(RetryRuntimeFactory {
        discoveries: Arc::clone(&discoveries),
    }));

    pool.configure_workflow_collection("workflow.retry", BTreeMap::from([(software_id.clone().into(), configuration)]));
    let failed = pool.reconcile_workflow(&intent).unwrap();
    assert_eq!(failed.errors().get(software_id.as_str()), Some(&DesktopRuntimeError::ProtocolRejected));
    let connected = pool.reconcile_workflow(&intent).unwrap();
    assert!(connected.errors().is_empty(), "retry must rediscover after failed collection activation");
    assert!(pool.status(&software_id).unwrap().is_feature_active(Feature::TextObserve));
    assert_eq!(discoveries.load(Ordering::SeqCst), 2);
}

#[test]
fn workflow_status_poll_keeps_the_active_connection() {
    let root = tempdir().expect("workflow Runtime data");
    let executable = root.path().join("Restarted.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let snapshot = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable");
    let software_id = snapshot
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.restart", "重启词典", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "重连")]),
        )
        .expect("restart dictionary");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.restart", "重启工作流").with_targets([
                WorkflowTargetCreate::new(
                    software_id.as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.restart"],
                ),
            ]),
        )
        .expect("restart workflow");
    let intent = backend
        .effective_workflow_intent("workflow.restart")
        .expect("restart intent");
    let discoveries = Arc::new(AtomicUsize::new(1));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(RetryRuntimeFactory { discoveries: Arc::clone(&discoveries) }));
    let configuration = CaptureConfiguration::new(
        glyphshift_capture::CaptureSessionId::new("poll-capture").unwrap(),
        root.path().join("observations.json"), 100,
    ).unwrap();
    pool.configure_workflow_collection("workflow.restart", BTreeMap::from([(software_id.clone().into(), configuration)]));
    pool.reconcile_workflow(&intent).expect("initial reconcile");

    let before = discoveries.load(Ordering::SeqCst);
    let refreshed = pool
        .refresh_workflow(&intent)
        .expect("refresh restarted target");

    assert!(refreshed.errors().is_empty());
    for _ in 0..3 {
        assert!(pool.refresh_workflow(&intent).unwrap().errors().is_empty());
        assert!(pool.status(&software_id).unwrap().is_feature_active(Feature::TextObserve));
        pool.control_workflow_collection("workflow.restart", &software_id, true).unwrap();
    }

    assert_eq!(discoveries.load(Ordering::SeqCst), before, "status polling must not stop and rediscover the running target");
    let status = pool.status(&software_id).expect("refreshed status");
    assert!(status.is_feature_requested(Feature::TextReplace));
    assert!(status.is_feature_active(Feature::TextReplace));
    assert_eq!(status.applied_generation(), Some(Generation::new(2)));
}

#[test]
fn workflow_refresh_discards_exited_instance_and_finds_restart_without_restarting_live_target() {
    let root = tempdir().unwrap();
    let exe = root.path().join("SyntheticLifetime.exe");
    fs::write(&exe, b"synthetic").unwrap();
    let mut backend = open_test_backend(root.path().join("data"));
    let software = backend.add_software(ExecutableSelection::new(exe)).unwrap().selected_software_id().unwrap().to_owned();
    backend.create_dictionary(DictionaryCreate::new("lifetime", "Lifetime", "en-US", "zh-CN")
        .with_entries([DictionaryEntryCreate::new("Open", "Translation")])).unwrap();
    create_single_workflow(&mut backend, "lifetime", &software, "lifetime");
    let intent = backend.effective_workflow_intent("lifetime").unwrap();
    let instance = Arc::new(AtomicUsize::new(1));
    let discoveries = Arc::new(AtomicUsize::new(0));
    let mut pool = DesktopRuntimePool::with_factory(Box::new(RestartingRuntimeFactory { instance: instance.clone(), discoveries: discoveries.clone() }));
    pool.reconcile_workflow(&intent).unwrap();
    pool.refresh_workflow(&intent).unwrap();
    assert_eq!(discoveries.load(Ordering::SeqCst), 1, "live targets must not restart");
    instance.store(0, Ordering::SeqCst);
    let stopped = pool.refresh_workflow(&intent).unwrap();
    assert_eq!(stopped.errors().get(software.as_str()), Some(&DesktopRuntimeError::UnknownTarget));
    assert!(!pool.status(&software).is_some_and(|status| status.is_active()));
    instance.store(2, Ordering::SeqCst);
    assert!(pool.refresh_workflow(&intent).unwrap().errors().is_empty());
    assert_eq!(pool.status(&software).unwrap().active_target_id(), Some(2));
}
