use super::*;
use crate::quick_probe::{
    ProbeCreationRequest, ProbeDictionarySourceRequest, ProbeTargetSourceRequest,
    QuickProbeAssetDisposition, QuickProbeSessionStore,
};

fn quick_probe_request(executable: &Path) -> ProbeCreationRequest {
    ProbeCreationRequest {
        target: ProbeTargetSourceRequest::ActiveProcess {
            executable_path: executable.to_string_lossy().into_owned(),
        },
        dictionary: ProbeDictionarySourceRequest::Temporary {
            source_locale: "en-US".into(),
            target_locale: "zh-CN".into(),
        },
        name: None,
        adapter_ids: Vec::new(),
        live_preview_enabled: false,
    }
}

#[test]
fn bulk_delete_cleans_a_temporary_probe_and_preserves_reused_software() {
    let (mut application, _calls, software_id, _root) = workflow_application();
    let executable = software_executable(&application, &software_id);
    let started = application
        .create_probe_from_sources_for_test(quick_probe_request(&executable))
        .expect("temporary probe");
    let run_id: Box<str> = started.summary.id().into();
    let dictionary_id: Box<str> = started.summary.dictionary_id().into();
    application
        .delete_probe_runs(&[run_id.clone()])
        .expect("bulk delete active temporary probe");
    assert!(!application.quick_probe_sessions.contains(&run_id));
    assert!(application.probe_runs.summary(&run_id).is_err());
    assert!(application.backend.dictionary(&dictionary_id).is_err());
    assert!(application
        .backend
        .snapshot()
        .software()
        .iter()
        .any(|software| software.id() == software_id.as_ref()));
}

#[test]
fn bulk_delete_mixes_regular_and_temporary_probes_without_deleting_reused_assets() {
    let (mut application, _calls, software_id, _root) = workflow_application();
    let existing_dictionary = first_dictionary_id(&application);
    let regular = application
        .create_probe_from_sources_for_test(library_probe_request(
            software_id.clone(),
            existing_dictionary.clone(),
        ))
        .expect("regular probe");
    application
        .disconnect_probe_run(regular.summary.id())
        .expect("release regular probe");
    let temporary = application
        .create_probe_from_sources_for_test(quick_probe_request(&software_executable(
            &application,
            &software_id,
        )))
        .expect("temporary probe");
    application
        .delete_probe_runs(&[regular.summary.id().into(), temporary.summary.id().into()])
        .expect("mixed bulk delete");
    assert!(application
        .probe_runs
        .summary(regular.summary.id())
        .is_err());
    assert!(application
        .probe_runs
        .summary(temporary.summary.id())
        .is_err());
    assert!(application.backend.dictionary(&existing_dictionary).is_ok());
    assert!(application
        .backend
        .dictionary(temporary.summary.dictionary_id())
        .is_err());
}

#[test]
fn bulk_delete_checks_all_targets_before_removing_anything() {
    let (mut application, _calls, software_id, _root) = workflow_application();
    let temporary = application
        .create_probe_from_sources_for_test(quick_probe_request(&software_executable(
            &application,
            &software_id,
        )))
        .expect("temporary probe");
    assert!(application
        .delete_probe_runs(&[temporary.summary.id().into(), "unknown-probe".into()])
        .is_err());
    assert!(application
        .probe_runs
        .summary(temporary.summary.id())
        .is_ok());
    assert!(application
        .backend
        .dictionary(temporary.summary.dictionary_id())
        .is_ok());
    application.ai_locked_dictionary_id = Some(temporary.summary.dictionary_id().into());
    assert!(application
        .delete_probe_runs(&[temporary.summary.id().into()])
        .is_err());
    assert!(application
        .probe_runs
        .summary(temporary.summary.id())
        .is_ok());
}

#[test]
fn temporary_probe_persists_the_explicit_language_direction_and_rejects_auto() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    let executable = software_executable(&application, &software_id);
    let mut invalid = quick_probe_request(&executable);
    invalid.dictionary = ProbeDictionarySourceRequest::Temporary {
        source_locale: "auto".into(),
        target_locale: "zh-CN".into(),
    };
    let error = application
        .create_probe_from_sources_for_test(invalid)
        .err()
        .expect("reject an unsupported automatic source locale");
    assert_eq!(
        serde_json::to_value(error).expect("serialize invalid locale")["code"],
        "quick_probe.invalid_locale"
    );
    let mut invalid_target = quick_probe_request(&executable);
    invalid_target.dictionary = ProbeDictionarySourceRequest::Temporary {
        source_locale: "en-US".into(),
        target_locale: "".into(),
    };
    let error = application
        .create_probe_from_sources_for_test(invalid_target)
        .err()
        .expect("reject an empty target locale");
    assert_eq!(
        serde_json::to_value(error).expect("serialize invalid target locale")["code"],
        "quick_probe.invalid_locale"
    );

    let mut request = quick_probe_request(&executable);
    request.dictionary = ProbeDictionarySourceRequest::Temporary {
        source_locale: "ja-JP".into(),
        target_locale: "ko-KR".into(),
    };
    let started = application
        .create_probe_from_sources_for_test(request)
        .expect("create temporary probe with explicit language direction");
    let dictionary = application
        .backend
        .dictionary(started.summary.dictionary_id())
        .expect("temporary dictionary");
    assert_eq!(dictionary.metadata().source_locale(), "ja-JP");
    assert_eq!(dictionary.metadata().target_locale(), "ko-KR");
}

fn library_probe_request(
    software_id: impl Into<Box<str>>,
    dictionary_id: impl Into<Box<str>>,
) -> ProbeCreationRequest {
    ProbeCreationRequest {
        target: ProbeTargetSourceRequest::Library {
            software_id: software_id.into(),
        },
        dictionary: ProbeDictionarySourceRequest::Library {
            dictionary_id: dictionary_id.into(),
        },
        name: Some("Library probe".into()),
        adapter_ids: Vec::new(),
        live_preview_enabled: false,
    }
}

fn first_dictionary_id(application: &DesktopApplication) -> Box<str> {
    application.backend.snapshot().dictionaries()[0]
        .metadata()
        .id()
        .into()
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
        .create_probe_from_sources_for_test(quick_probe_request(&executable))
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
fn automatic_quick_probe_publishes_dictionary_edits_when_writeback_is_available() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    let executable = software_executable(&application, &software_id);

    let started = application
        .create_probe_from_sources_for_test(quick_probe_request(&executable))
        .expect("start automatic quick probe");

    assert!(started.summary.live_preview_enabled());
    assert_eq!(started.summary.preview_generation(), 1);

    let edited = application
        .edit_probe_translation(ProbeTranslationEditRequest {
            run_id: started.summary.id().into(),
            source: "Assets".into(),
            translation: "资产".into(),
        })
        .expect("publish quick-probe dictionary edit");

    assert_eq!(edited.summary.preview_generation(), 2);
    let calls = calls.lock().expect("runtime call log");
    assert_eq!(calls.capture_publications.len(), 2);
    assert_eq!(calls.capture_publications[1].1.generation().value(), 2);
}

#[test]
fn automatic_quick_probe_remains_collection_only_without_active_writeback() {
    let (mut application, calls, software_id, _data_root) = workflow_application();
    let executable = software_executable(&application, &software_id);
    application.runtimes = Some(Box::new(RecordingWorkflowRuntime {
        calls: Arc::clone(&calls),
        start_capture_error: None,
        capture_capability: ProbeRuntimeCapability::CollectionOnly,
    }));

    let started = application
        .create_probe_from_sources_for_test(quick_probe_request(&executable))
        .expect("start collection-only quick probe");

    assert!(!started.summary.live_preview_enabled());
    assert_eq!(started.summary.preview_generation(), 0);
    assert!(calls
        .lock()
        .expect("runtime call log")
        .capture_publications
        .is_empty());
}

#[test]
fn probe_creation_reuses_both_library_sources_without_temporary_ownership() {
    let (mut application, _calls, software_id, data_root) = workflow_application();
    let dictionary_id = first_dictionary_id(&application);

    let started = application
        .create_probe_from_sources_for_test(library_probe_request(
            software_id.clone(),
            dictionary_id.clone(),
        ))
        .expect("start a probe from library assets");

    assert_eq!(started.summary.software_id(), software_id.as_ref());
    assert_eq!(started.summary.dictionary_id(), dictionary_id.as_ref());
    assert!(!started.quick_probe);
    assert!(QuickProbeSessionStore::open(data_root.path())
        .expect("reopen library probe ledger")
        .is_empty());
}

#[test]
fn probe_creation_can_pair_an_active_process_with_a_library_dictionary() {
    let (mut application, _calls, _software_id, data_root) = workflow_application();
    let dictionary_id = first_dictionary_id(&application);
    let executable = write_synthetic_executable(data_root.path(), "SyntheticActiveTarget.exe");
    let mut request = quick_probe_request(&executable);
    request.dictionary = ProbeDictionarySourceRequest::Library {
        dictionary_id: dictionary_id.clone(),
    };

    let started = application
        .create_probe_from_sources_for_test(request)
        .expect("start an active-process probe with a library dictionary");

    assert_eq!(started.summary.dictionary_id(), dictionary_id.as_ref());
    assert!(started.quick_probe);
    let cleaned = application
        .cleanup_quick_probe(started.summary.id())
        .expect("clean active-process probe");
    assert_eq!(cleaned.software, QuickProbeAssetDisposition::Removed);
    assert_eq!(cleaned.dictionary, QuickProbeAssetDisposition::Reused);
    assert!(application.backend.dictionary(&dictionary_id).is_ok());
}

#[test]
fn probe_creation_can_pair_library_software_with_a_temporary_dictionary() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    let mut request = quick_probe_request(&software_executable(&application, &software_id));
    request.target = ProbeTargetSourceRequest::Library {
        software_id: software_id.clone(),
    };

    let started = application
        .create_probe_from_sources_for_test(request)
        .expect("start a library-software probe with a temporary dictionary");

    assert_eq!(started.summary.software_id(), software_id.as_ref());
    assert!(started.quick_probe);
    let cleaned = application
        .cleanup_quick_probe(started.summary.id())
        .expect("clean temporary dictionary probe");
    assert_eq!(cleaned.software, QuickProbeAssetDisposition::Reused);
    assert_eq!(cleaned.dictionary, QuickProbeAssetDisposition::Removed);
}

#[test]
fn quick_probe_can_retain_a_new_software_dictionary_and_probe() {
    let (mut application, _calls, _software_id, data_root) = workflow_application();
    let executable = write_synthetic_executable(data_root.path(), "SyntheticQuickTarget.exe");

    let started = application
        .create_probe_from_sources_for_test(quick_probe_request(&executable))
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
        .create_probe_from_sources_for_test(quick_probe_request(&executable))
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
        .create_probe_from_sources_for_test(quick_probe_request(&executable))
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
        .create_probe_from_sources_for_test(quick_probe_request(&executable))
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
        .create_probe_from_sources_for_test(quick_probe_request(&executable))
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
