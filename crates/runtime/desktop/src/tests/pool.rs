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
fn workflow_reconcile_runs_two_targets_and_stops_only_its_owned_software() {
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
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.group", "双目标工作流").with_targets([
                WorkflowTargetCreate::new(
                    software_ids[0].as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.shared"],
                ),
                WorkflowTargetCreate::new(
                    software_ids[1].as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.shared"],
                ),
            ]),
        )
        .expect("group workflow");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.other", "独立工作流").with_targets([
                WorkflowTargetCreate::new(
                    software_ids[2].as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.shared"],
                ),
            ]),
        )
        .expect("other workflow");
    let group = backend
        .effective_workflow_intent("workflow.group")
        .expect("compiled group intent");
    let other = backend
        .effective_workflow_intent("workflow.other")
        .expect("compiled other intent");
    let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));

    pool.reconcile_workflow(&other)
        .expect("reconcile unrelated workflow");
    let activated = pool
        .reconcile_workflow(&group)
        .expect("reconcile two targets");
    assert!(activated.errors().is_empty());
    for software_id in &software_ids[..2] {
        let status = pool.status(software_id).expect("owned Runtime status");
        assert!(status.is_feature_requested(Feature::TextReplace));
        assert!(status.is_feature_active(Feature::TextReplace));
    }

    let stopped = pool
        .stop_workflow("workflow.group")
        .expect("stop only the group workflow");
    assert!(stopped.errors().is_empty());
    assert!(software_ids[..2]
        .iter()
        .all(|software_id| pool.status(software_id).is_none()));
    assert!(pool
        .status(&software_ids[2])
        .is_some_and(|status| status.is_feature_active(Feature::TextReplace)));
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
fn workflow_reconcile_publishes_a_shared_dictionary_generation_to_every_target() {
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
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.hot", "热更新工作流").with_targets([
                WorkflowTargetCreate::new(
                    software_ids[0].as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.hot"],
                ),
                WorkflowTargetCreate::new(
                    software_ids[1].as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.hot"],
                ),
            ]),
        )
        .expect("shared workflow");
    let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
    let initial = backend
        .effective_workflow_intent("workflow.hot")
        .expect("initial intent");
    pool.reconcile_workflow(&initial)
        .expect("initial reconcile");

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
        .expect("update shared dictionary");
    let next = backend
        .effective_workflow_intent("workflow.hot")
        .expect("next intent");
    let updated = pool
        .reconcile_workflow(&next)
        .expect("publish next generation");

    assert_eq!(updated.statuses().len(), 2);
    assert!(updated.statuses().iter().all(|status| {
        status.applied_generation() == Some(glyphshift_domain::Generation::new(3))
    }));
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
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.old", "旧工作流").with_targets([
                WorkflowTargetCreate::new(
                    software_ids[0].as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.replace"],
                ),
                WorkflowTargetCreate::new(
                    software_ids[1].as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.replace"],
                ),
            ]),
        )
        .expect("old workflow");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.new", "新工作流").with_targets([
                WorkflowTargetCreate::new(
                    software_ids[0].as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.replace"],
                ),
            ]),
        )
        .expect("new workflow");
    let old = backend
        .effective_workflow_intent("workflow.old")
        .expect("old intent");
    let new = backend
        .effective_workflow_intent("workflow.new")
        .expect("new intent");
    let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
    pool.reconcile_workflow(&old).expect("start old workflow");

    let replaced = pool
        .replace_workflow(&new)
        .expect("replace the complete old intent");

    assert!(replaced.errors().is_empty());
    assert!(pool
        .status(&software_ids[0])
        .is_some_and(|status| status.is_feature_active(Feature::TextReplace)));
    assert!(pool.status(&software_ids[1]).is_none());
    assert!(pool.stop_workflow("workflow.old").is_err());
}

#[test]
fn workflow_refresh_reapplies_requested_features_after_target_restart() {
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
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.stop", "停止工作流").with_targets([
                WorkflowTargetCreate::new(
                    software_ids[0].as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.stop"],
                ),
                WorkflowTargetCreate::new(
                    software_ids[1].as_str(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.stop"],
                ),
            ]),
        )
        .expect("stop workflow");
    let intent = backend
        .effective_workflow_intent("workflow.stop")
        .expect("stop intent");
    let mut pool = DesktopRuntimePool::with_factory(Box::new(InMemoryRuntimeFactory));
    pool.reconcile_workflow(&intent).expect("initial reconcile");

    let stopped = pool
        .stop_workflow("workflow.stop")
        .expect("bounded stop report");

    assert_eq!(
        stopped.errors().get(software_ids[0].as_str()),
        Some(&DesktopRuntimeError::SessionRejected)
    );
    assert!(pool
        .status(&software_ids[0])
        .is_some_and(|status| status.is_feature_active(Feature::TextReplace)));
    assert!(pool.status(&software_ids[1]).is_none());
}
