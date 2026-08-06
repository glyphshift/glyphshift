use super::*;

#[test]
#[ignore = "requires the local Windows Runtime bundle with its synthetic target"]
fn desktop_acquisition_session_uses_a_verified_worker_and_ephemeral_target_grant() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let target_executable = runtime_root.join("test-target.exe");
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/local-test/evidence/desktop-acquisition-session");
    std::fs::create_dir_all(&local_test).expect("acquisition evidence root");
    let data = tempfile::Builder::new()
        .prefix("contract-")
        .tempdir_in(local_test)
        .expect("isolated acquisition data");
    let mut target =
        TargetProcess::spawn_with_args(&target_executable, ["--uia-standard-controls"]);
    assert_eq!(target.read_response(), "uia-ready");
    let geometry = target.render_command("geometry");
    let coordinates = geometry
        .strip_prefix("uia-geometry ")
        .expect("UIA geometry response")
        .split_whitespace()
        .map(|value| value.parse::<i32>().expect("geometry coordinate"))
        .collect::<Vec<_>>();
    assert_eq!(coordinates.len(), 8);

    let mut backend = open_backend(data.path(), &runtime_root);
    let snapshot = backend
        .add_software(ExecutableSelection::new(&target_executable))
        .expect("register synthetic UIA target");
    let application_id = snapshot.software()[0].id().to_owned();
    let spec = backend
        .runtime_spec(&application_id)
        .expect("generic Runtime spec");
    let mut bundle = RuntimeBundle::open(&runtime_root).expect("verified Runtime bundle");
    let mut runtime = bundle
        .discover(application_id, &spec)
        .expect("discover UIA target");
    let target_id = runtime.targets().next().expect("discovered target").id();

    let result = runtime
        .acquire_point(
            target_id,
            "windows.uia.acquire",
            DesktopPoint::new(coordinates[0], coordinates[1]),
            &DesktopAcquisitionCancellation::new(),
        )
        .expect("Desktop UIA point acquisition");
    assert_eq!(result.blocks()[0].source(), "Fixture label");
    assert!(result.blocks()[0]
        .anchors()
        .iter()
        .any(|anchor| anchor.contains(DesktopPoint::new(coordinates[0], coordinates[1]))));
    target.stop();
}
