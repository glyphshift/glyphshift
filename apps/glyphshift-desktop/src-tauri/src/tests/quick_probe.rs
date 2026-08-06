use super::*;
use crate::quick_probe::{
    QuickProbeAssetDisposition, QuickProbeSessionStore, QuickProbeStartRequest,
};

fn quick_probe_request(executable: &Path) -> QuickProbeStartRequest {
    QuickProbeStartRequest {
        executable_path: executable.to_string_lossy().into_owned(),
        target_locale: "zh-CN".into(),
    }
}

fn software_executable(application: &DesktopApplication, software_id: &str) -> PathBuf {
    application
        .backend
        .snapshot()
        .software()
        .iter()
        .find(|software| software.id() == software_id)
        .and_then(|software| software.executable_path())
        .map(PathBuf::from)
        .expect("synthetic software executable")
}

#[test]
fn quick_probe_reuses_software_and_cleans_only_its_owned_assets() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    let executable = software_executable(&application, &software_id);

    let started = application
        .start_quick_probe_for_test(quick_probe_request(&executable))
        .expect("start quick probe");

    assert_eq!(started.summary.status(), ProbeRunStatus::Running);
    assert_eq!(started.summary.software_id(), software_id.as_ref());
    assert!(started.quick_probe);
    assert!(application
        .backend
        .dictionary(started.summary.dictionary_id())
        .is_ok());
    assert!(QuickProbeSessionStore::open(_data_root.path())
        .expect("reopen quick probe ledger")
        .contains(started.summary.id()));

    let cleaned = application
        .cleanup_quick_probe(started.summary.id())
        .expect("clean quick probe");

    assert_eq!(cleaned.software, QuickProbeAssetDisposition::Reused);
    assert_eq!(cleaned.dictionary, QuickProbeAssetDisposition::Removed);
    assert!(application.probe_run_summary(started.summary.id()).is_err());
    assert!(application
        .backend
        .dictionary(started.summary.dictionary_id())
        .is_err());
    assert!(application
        .backend
        .snapshot()
        .software()
        .iter()
        .any(|software| software.id() == software_id.as_ref()));
    let calls = calls.lock().expect("runtime call log");
    assert_eq!(calls.captures_started, vec![software_id.clone()]);
    assert_eq!(calls.captures_stopped, vec![software_id]);
}

#[test]
fn quick_probe_can_retain_a_new_software_dictionary_and_probe() {
    let (mut application, _calls, _software_id, data_root) = workflow_application();
    let executable = write_synthetic_executable(data_root.path(), "SyntheticQuickTarget.exe");

    let started = application
        .start_quick_probe_for_test(quick_probe_request(&executable))
        .expect("start quick probe for a new software");
    let retained = application
        .retain_quick_probe(started.summary.id())
        .expect("retain quick probe assets");

    assert!(!retained.quick_probe);
    assert!(application
        .backend
        .dictionary(retained.summary.dictionary_id())
        .is_ok());
    assert!(application
        .backend
        .snapshot()
        .software()
        .iter()
        .any(|software| software.id() == retained.summary.software_id()));
    assert!(!QuickProbeSessionStore::open(data_root.path())
        .expect("reopen retained ledger")
        .contains(retained.summary.id()));
}

#[test]
fn quick_probe_removes_new_assets_when_the_user_ends_the_session() {
    let (mut application, calls, _software_id, data_root) = workflow_application();
    let executable = write_synthetic_executable(data_root.path(), "SyntheticDisposableTarget.exe");
    let initial_software_count = application.backend.snapshot().software().len();

    let started = application
        .start_quick_probe_for_test(quick_probe_request(&executable))
        .expect("start disposable quick probe");
    let created_software_id = Box::<str>::from(started.summary.software_id());
    let cleaned = application
        .cleanup_quick_probe(started.summary.id())
        .expect("clean disposable quick probe");

    assert_eq!(cleaned.software, QuickProbeAssetDisposition::Removed);
    assert_eq!(cleaned.dictionary, QuickProbeAssetDisposition::Removed);
    assert_eq!(
        application.backend.snapshot().software().len(),
        initial_software_count
    );
    assert!(calls
        .lock()
        .expect("runtime call log")
        .software_removed
        .contains(&created_software_id));
}

#[test]
fn quick_probe_start_failure_compensates_every_new_asset() {
    let (mut application, calls, _software_id, data_root) = workflow_application();
    let executable = write_synthetic_executable(data_root.path(), "SyntheticOfflineTarget.exe");
    let initial_snapshot = application.backend.snapshot();
    application.runtimes = Some(Box::new(RecordingWorkflowRuntime {
        calls,
        start_capture_error: Some(DesktopRuntimeError::UnknownTarget),
        capture_capability: ProbeRuntimeCapability::DirectReplace,
    }));

    let error = application
        .start_quick_probe_for_test(quick_probe_request(&executable))
        .expect_err("an offline quick target must fail as one operation");

    assert_eq!(
        serde_json::to_value(error).expect("serialize quick probe failure")["code"],
        "quick_probe.target_stopped"
    );
    let snapshot = application.backend.snapshot();
    assert_eq!(snapshot.software().len(), initial_snapshot.software().len());
    assert_eq!(
        snapshot.dictionaries().len(),
        initial_snapshot.dictionaries().len()
    );
    assert!(application
        .probe_run_list()
        .expect("list probes after compensation")
        .is_empty());
    assert!(QuickProbeSessionStore::open(data_root.path())
        .expect("reopen compensated ledger")
        .is_empty());
}

#[test]
fn quick_probe_cleanup_retains_owned_assets_that_a_workflow_now_references() {
    let (mut application, _calls, _software_id, data_root) = workflow_application();
    let executable = write_synthetic_executable(data_root.path(), "SyntheticPromotedTarget.exe");
    let started = application
        .start_quick_probe_for_test(quick_probe_request(&executable))
        .expect("start promotable quick probe");
    application
        .backend
        .create_workflow(
            WorkflowCreate::new("workflow.quick-reference", "Quick reference").with_targets([
                WorkflowTargetCreate::new(
                    started.summary.software_id(),
                    [TEST_ADAPTER_ID],
                    [started.summary.dictionary_id()],
                ),
            ]),
        )
        .expect("reference quick assets from a workflow");

    let cleaned = application
        .cleanup_quick_probe(started.summary.id())
        .expect("clean quick probe while preserving referenced assets");

    assert_eq!(cleaned.software, QuickProbeAssetDisposition::Retained);
    assert_eq!(cleaned.dictionary, QuickProbeAssetDisposition::Retained);
    assert!(application
        .backend
        .dictionary(started.summary.dictionary_id())
        .is_ok());
    assert!(application
        .backend
        .snapshot()
        .software()
        .iter()
        .any(|software| software.id() == started.summary.software_id()));
}

#[test]
fn quick_probe_recovery_cleans_an_orphaned_owned_dictionary() {
    let (mut application, _calls, software_id, data_root) = workflow_application();
    let executable = software_executable(&application, &software_id);
    let started = application
        .start_quick_probe_for_test(quick_probe_request(&executable))
        .expect("start quick probe before simulated interruption");
    let run_id = Box::<str>::from(started.summary.id());
    let dictionary_id = Box::<str>::from(started.summary.dictionary_id());
    application
        .disconnect_probe_run(&run_id)
        .expect("disconnect before simulating lost probe metadata");
    application
        .delete_probe_runs(std::slice::from_ref(&run_id))
        .expect("remove probe metadata while retaining ownership ledger");

    application
        .recover_quick_probe_sessions()
        .expect("recover orphaned quick probe");

    assert!(application.backend.dictionary(&dictionary_id).is_err());
    assert!(!QuickProbeSessionStore::open(data_root.path())
        .expect("reopen recovered ledger")
        .contains(&run_id));
}
