use super::*;

#[test]
#[ignore = "requires the local Windows Runtime bundle with its synthetic target"]
fn desktop_bundle_captures_uia_public_text_without_password_content() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let target_executable = runtime_root.join("test-target.exe");
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/local-test/evidence/desktop-uia-capture");
    std::fs::create_dir_all(&local_test).expect("UIA capture evidence root");
    let data = tempfile::Builder::new()
        .prefix("contract-")
        .tempdir_in(local_test)
        .expect("isolated UIA capture data");
    let mut target =
        TargetProcess::spawn_with_args(&target_executable, ["--uia-standard-controls"]);
    assert_eq!(target.read_response(), "uia-ready");
    let mut backend = open_backend(data.path(), &runtime_root);
    let snapshot = backend
        .add_software(ExecutableSelection::new(&target_executable))
        .expect("register synthetic UIA target");
    let application_id = snapshot.software()[0].id().to_owned();
    let spec = backend
        .capture_runtime_spec(&application_id, &[TEST_UIA_OBSERVER_ID.into()])
        .expect("compiled UIA capture Runtime spec");
    let output = data.path().join("capture.json");
    let capture = CaptureConfiguration::new(
        CaptureSessionId::new("desktop-uia-capture").expect("capture session id"),
        &output,
        100,
    )
    .expect("UIA capture configuration");
    let bundle = RuntimeBundle::open(&runtime_root).expect("verified Runtime bundle");
    let mut pool = DesktopRuntimePool::new(bundle);

    pool.start_capture(application_id.clone(), &spec, None, capture)
        .expect("start UIA capture");
    std::thread::sleep(Duration::from_millis(1_250));
    pool.stop_capture(application_id).expect("stop UIA capture");
    let catalog = CaptureCatalog::read_current(&output).expect("UIA capture catalog");
    let sources = catalog
        .entries()
        .iter()
        .filter(|entry| entry.adapter_id() == TEST_UIA_OBSERVER_ID)
        .map(|entry| entry.source())
        .collect::<BTreeSet<_>>();
    for expected in ["Fixture label", "Fixture value", "Fixture document"] {
        assert!(sources.contains(expected), "missing {expected}");
    }
    assert!(!sources.contains("Fixture secret"));
    target.stop();
}

#[test]
#[ignore = "requires the local Windows Runtime bundle with its synthetic target"]
fn desktop_capture_owns_one_checkpoint_for_a_process_family() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let target_executable = runtime_root.join("test-target.exe");
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/local-test/evidence/desktop-central-capture");
    std::fs::create_dir_all(&local_test).expect("central capture evidence root");
    let data = tempfile::Builder::new()
        .prefix("contract-")
        .tempdir_in(local_test)
        .expect("isolated central capture data");
    let mut target = TargetProcess::spawn(&target_executable);
    assert_eq!(
        target.render_command("start-console-child"),
        "child-started"
    );
    let mut backend = open_backend(data.path(), &runtime_root);
    let snapshot = backend
        .add_software(ExecutableSelection::new(&target_executable))
        .expect("register synthetic capture target");
    let application_id = snapshot.software()[0].id().to_owned();
    drop(backend);
    let extension_path = data
        .path()
        .join("extensions")
        .join(format!("{application_id}.json"));
    let mut extension = serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(&extension_path).expect("software extension"),
    )
    .expect("software extension json");
    extension["descendant_executables"] = serde_json::json!(["test-target.exe"]);
    std::fs::write(
        &extension_path,
        serde_json::to_string(&extension).expect("encode process family extension"),
    )
    .expect("write process family extension");
    let backend = open_backend(data.path(), &runtime_root);
    let spec = backend
        .capture_runtime_spec(&application_id, &[TEST_CONSOLE_OBSERVER_ID.into()])
        .expect("compiled Console capture Runtime spec");
    let output = data.path().join("capture.json");
    let capture = CaptureConfiguration::new(
        CaptureSessionId::new("desktop-central-capture").expect("capture session id"),
        &output,
        100,
    )
    .expect("central capture configuration");
    let bundle = RuntimeBundle::open(&runtime_root).expect("verified Runtime bundle");
    let mut pool = DesktopRuntimePool::new(bundle);

    pool.start_capture(application_id.clone(), &spec, None, capture)
        .expect("start Desktop-owned capture");
    pool.control_capture(&application_id, true)
        .expect("pause every process-family producer");
    assert_eq!(
        target.render_command("write-console"),
        "parent-console-attempted"
    );
    assert_eq!(
        target.render_command("child-write-console"),
        "child-console-attempted"
    );
    pool.control_capture(&application_id, false)
        .expect("resume every process-family producer");
    assert_eq!(
        target.render_command("write-console"),
        "parent-console-attempted"
    );
    assert_eq!(
        target.render_command("child-write-console"),
        "child-console-attempted"
    );
    std::thread::sleep(Duration::from_millis(350));
    assert_eq!(target.render_command("stop-console-child"), "child-stopped");
    assert_eq!(
        target.render_command("write-console"),
        "parent-console-attempted"
    );
    pool.stop_capture(application_id)
        .expect("drain and stop Desktop-owned capture");

    let catalog = CaptureCatalog::read_current(&output).expect("Desktop-owned checkpoint");
    assert_eq!(
        catalog
            .entries()
            .iter()
            .find(|entry| {
                entry.adapter_id() == TEST_CONSOLE_OBSERVER_ID
                    && entry.source() == "ParentConsoleText"
            })
            .map(|entry| entry.count()),
        Some(2)
    );
    assert_eq!(
        catalog
            .entries()
            .iter()
            .find(|entry| {
                entry.adapter_id() == TEST_CONSOLE_OBSERVER_ID
                    && entry.source() == "ChildConsoleText"
            })
            .map(|entry| entry.count()),
        Some(1)
    );
    target.stop();
}
