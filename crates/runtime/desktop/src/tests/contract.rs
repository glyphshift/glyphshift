use super::*;

#[test]
fn desktop_contract_exposes_instances_without_controller_tokens_or_paths() {
    let root = tempdir().expect("desktop data");
    let executable = root.path().join("MotionCanvas.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let snapshot = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable");
    let application_id = snapshot.software()[0].id();
    let spec = backend
        .runtime_spec(application_id)
        .expect("generic runtime spec");

    let runtime_library = root.path().join("runtime.dll");
    fs::write(&runtime_library, b"synthetic runtime").expect("runtime artifact");
    let artifacts = TargetArtifactCatalog::new(RuntimeArtifact::new(&runtime_library, [0; 32]), [])
        .expect("artifact catalog");
    let mut ledger = NonceLedger::new();
    let runtime = DesktopRuntime::connect(
        InventoryController,
        application_id.into(),
        Vec::new(),
        spec.publication().clone(),
        AdapterRegistry::new(AdapterTrustPolicy::new([], [])),
        artifacts,
        WorkerArtifactCatalog::default(),
        BTreeSet::new(),
        ProtocolVersion::new(1, 0),
        ControllerNonce::new([7; 32]),
        &mut ledger,
    )
    .expect("desktop runtime discovery");

    assert_eq!(runtime.application_id(), application_id);
    assert_eq!(
        runtime
            .targets()
            .map(|target| (target.id(), target.display_name()))
            .collect::<Vec<_>>(),
        vec![(1, "MotionCanvas — 主窗口")]
    );
    assert!(!runtime.is_active());
    assert!(!runtime.supports(Feature::TextReplace));
}
