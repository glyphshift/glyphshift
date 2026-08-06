use super::*;
use glyphshift_domain::Placement;

#[test]
#[ignore = "requires the local Windows Runtime bundle built by scripts/build-runtime-bundle.ps1"]
fn runtime_bundle_exposes_observe_only_adapters_without_promoting_them_to_translation() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let bundle = RuntimeBundle::open(runtime_root).expect("verified Runtime bundle");

    let observer = bundle
        .adapter_options()
        .iter()
        .find(|adapter| adapter.id() == TEST_CONSOLE_OBSERVER_ID)
        .expect("Console observer must be visible to the desktop Probe catalog");
    assert_eq!(observer.features(), [Feature::TextObserve]);
    assert_eq!(observer.placement(), Placement::TargetProcess);
    assert!(!bundle
        .translation_adapter_ids()
        .iter()
        .any(|adapter_id| adapter_id.as_ref() == TEST_CONSOLE_OBSERVER_ID));

    let uia = bundle
        .adapter_options()
        .iter()
        .find(|adapter| adapter.id() == TEST_UIA_OBSERVER_ID)
        .expect("UIA observer must be visible to the desktop Probe catalog");
    assert_eq!(uia.features(), [Feature::TextObserve]);
    assert_eq!(uia.placement(), Placement::IsolatedWorker);
    assert!(uia
        .architectures()
        .iter()
        .any(|architecture| architecture.as_ref() == "x86"));
    assert!(!bundle
        .translation_adapter_ids()
        .iter()
        .any(|adapter_id| adapter_id.as_ref() == TEST_UIA_OBSERVER_ID));
    assert_eq!(
        bundle.acquisition_worker_ids(),
        [Box::<str>::from("windows.uia.acquire")]
    );
    bundle
        .acquisition_worker_host("windows.uia.acquire")
        .expect("verified UIA acquisition worker host");
}

#[test]
#[ignore = "requires the local Windows Runtime bundle built by scripts/build-runtime-bundle.ps1"]
fn runtime_bundle_rejects_a_missing_isolated_worker_artifact() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/local-test/evidence/runtime-bundle-missing-worker");
    std::fs::create_dir_all(&local_test).expect("missing-worker evidence root");
    let copy = tempfile::Builder::new()
        .prefix("contract-")
        .tempdir_in(local_test)
        .expect("isolated bundle copy");
    for entry in std::fs::read_dir(&runtime_root).expect("Runtime bundle files") {
        let entry = entry.expect("Runtime bundle entry");
        if entry
            .file_type()
            .expect("Runtime bundle entry type")
            .is_file()
        {
            std::fs::copy(entry.path(), copy.path().join(entry.file_name()))
                .expect("copy Runtime bundle artifact");
        }
    }
    let manifest = serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(copy.path().join("runtime-bundle.json"))
            .expect("Runtime bundle manifest"),
    )
    .expect("Runtime bundle manifest json");
    let worker_file = manifest["isolated_workers"][0]["file"]
        .as_str()
        .expect("isolated worker artifact name");
    std::fs::remove_file(copy.path().join(worker_file)).expect("remove isolated worker copy");

    assert_eq!(
        RuntimeBundle::open(copy.path()).err(),
        Some(DesktopRuntimeError::BundleUnavailable)
    );
}
