use super::*;
use crate::quick_probe::{
    ProbeCreationRequest, ProbeDictionarySourceRequest, ProbeTargetSourceRequest,
    QuickProbeSessionStore,
};

fn quick_probe_request(executable: &Path) -> ProbeCreationRequest {
    ProbeCreationRequest {
            excluded_dictionary_ids: Vec::new(),
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

fn library_probe_request(
    software_id: impl Into<Box<str>>,
    dictionary_id: impl Into<Box<str>>,
) -> ProbeCreationRequest {
    ProbeCreationRequest {
            excluded_dictionary_ids: Vec::new(),
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
fn new_dictionary_probe_is_regular_and_stop_delete_keep_assets() {
    let (mut application, _calls, software_id, root) = workflow_application();
    let request = quick_probe_request(&software_executable(&application, &software_id));
    let started = application.create_probe_from_sources_for_test(request).unwrap();
    assert!(!started.quick_probe);
    assert!(started.summary.live_preview_enabled());
    assert!(QuickProbeSessionStore::open(root.path()).unwrap().is_empty());
    assert!(application.delete_probe_runs(&[started.summary.id().into()]).is_err());
    application.disconnect_probe_run(started.summary.id()).unwrap();
    application.delete_probe_runs(&[started.summary.id().into()]).unwrap();
    assert!(application.backend.dictionary(started.summary.dictionary_id()).is_ok());
    assert!(application.backend.snapshot().software().iter().any(|s| s.id() == software_id.as_ref()));
}

#[test]
fn active_source_creates_durable_software_and_dictionary_without_promotion() {
    let (mut application, calls, _, root) = workflow_application();
    let exe = write_synthetic_executable(root.path(), "SyntheticDurableTarget.exe");
    let started = application.create_probe_from_sources_for_test(quick_probe_request(&exe)).unwrap();
    assert!(!started.quick_probe);
    application.disconnect_probe_run(started.summary.id()).unwrap();
    application.delete_probe_runs(&[started.summary.id().into()]).unwrap();
    assert!(application.backend.dictionary(started.summary.dictionary_id()).is_ok());
    assert!(application.backend.snapshot().software().iter().any(|s| s.id() == started.summary.software_id()));
    assert!(calls.lock().unwrap().software_removed.is_empty());
}

#[test]
fn existing_dictionary_can_be_selected_without_creating_another() {
    let (mut application, _, software_id, _root) = workflow_application();
    let id = first_dictionary_id(&application);
    let count = application.backend.snapshot().dictionaries().len();
    let started = application.create_probe_from_sources_for_test(library_probe_request(software_id, id.clone())).unwrap();
    assert_eq!(started.summary.dictionary_id(), id.as_ref());
    assert_eq!(application.backend.snapshot().dictionaries().len(), count);
    assert!(!started.quick_probe);
}

#[test]
fn successful_legacy_task_is_promoted_without_losing_its_dictionary() {
    let (mut application, _, software_id, root) = workflow_application();
    let exe = software_executable(&application, &software_id);
    let started = application.create_probe_from_sources_for_test(quick_probe_request(&exe)).unwrap();
    application.disconnect_probe_run(started.summary.id()).unwrap();
    let dictionary = application.backend.dictionary(started.summary.dictionary_id()).unwrap().clone();
    application.backend.update_dictionary(DictionaryEdit::from_dictionary(&dictionary).with_name("Synthetic 临时词典")).unwrap();
    let ledger = serde_json::json!({"schema":"glyphshift.quick-probe-sessions/1", "sessions":[{
        "runId":started.summary.id(), "softwareId":software_id, "dictionaryId":dictionary.id(),
        "executablePath":exe.to_string_lossy(), "ownsSoftware":false, "ownsDictionary":true, "phase":"active"
    }]});
    std::fs::write(root.path().join("quick-probe-sessions.json"), serde_json::to_vec(&ledger).unwrap()).unwrap();
    application.quick_probe_sessions = QuickProbeSessionStore::open(root.path()).unwrap();
    application.recover_quick_probe_sessions().unwrap();
    assert!(!application.probe_run_summary(started.summary.id()).unwrap().quick_probe);
    let migrated = application.backend.dictionary(dictionary.id()).unwrap();
    assert_eq!(migrated.metadata().name(), "Synthetic 字典");
    assert_eq!(migrated.entries(), dictionary.entries());
    assert!(application.quick_probe_sessions.is_empty());
}

#[test]
fn dictionary_edit_still_publishes_after_probe_creation_transaction_ends() {
    let (mut application, calls, software_id, _root) = workflow_application();
    let request = quick_probe_request(&software_executable(&application, &software_id));
    let started = application.create_probe_from_sources_for_test(request).unwrap();
    let edited = application.edit_probe_translation(ProbeTranslationEditRequest {
        run_id: started.summary.id().into(), source: "Assets".into(), translation: "资产".into(),
        translation_context: None,
    }).unwrap();
    assert_eq!(edited.summary.preview_generation(), 2);
    assert_eq!(calls.lock().unwrap().capture_publications.len(), 2);
}

#[test]
fn quick_probe_x86_preflight_and_adapter_selection_use_native_bundle_support() {
    let (mut application, _calls, _software_id, data_root) = workflow_application();
    let executable = write_synthetic_executable(data_root.path(), "SyntheticX86Target.exe");
    let mut image = fs::read(&executable).expect("read fixture");
    image[0x84..0x86].copy_from_slice(&0x014c_u16.to_le_bytes());
    fs::write(&executable, image).expect("write x86 fixture");

    assert_eq!(
        application.software_preflight(&executable).unwrap().state,
        SoftwarePreflightState::UnsupportedArchitecture,
    );
    application
        .adapter_target_support
        .get_mut(TEST_ADAPTER_ID)
        .unwrap()
        .architectures
        .push("x86".into());
    assert_eq!(
        application.software_preflight(&executable).unwrap().state,
        SoftwarePreflightState::NotRunning,
    );
    let started = application
        .create_probe_from_sources_for_test(quick_probe_request(&executable))
        .expect("x86 quick probe passes preflight and selects its native adapter");
    assert!(!started.quick_probe);
    application
        .disconnect_probe_run(started.summary.id())
        .expect("clean probe");
}
