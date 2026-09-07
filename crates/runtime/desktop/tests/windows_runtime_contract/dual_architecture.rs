use super::*;

#[test]
#[ignore = "requires the dual-architecture Runtime bundle built with -IncludeTestTarget"]
fn one_x64_session_translates_x86_and_x64_with_one_adapter_identity() {
    let root = std::path::PathBuf::from(
        std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT").expect("explicit local Runtime bundle"),
    );
    let x64 = root.join("test-target.exe");
    let x86 = root.join("test-target-x86.exe");
    let mut target64 = TargetProcess::spawn(&x64);
    assert_eq!(
        target64.render_command(&format!("start-render-child {}", x86.display())),
        "child-started"
    );
    let baseline64 = target64.render();
    let baseline32 = target64.render_command("render-child render");
    let data = tempfile::Builder::new()
        .prefix("dual-gdi-")
        .tempdir_in(root.parent().unwrap())
        .unwrap();
    let mut backend = open_backend(data.path(), &root);
    let registered = backend
        .add_software(ExecutableSelection::new(&x64))
        .unwrap();
    let id = registered.software()[0].id().to_owned();
    let (dictionary, translation_spec) = create_text_workflow(
        &mut backend,
        &id,
        "dictionary.dual",
        "workflow.dual",
        "Open",
        "第一代中文",
    );
    let spec = backend
        .capture_runtime_spec(&id, &[TEST_ADAPTER_ID.into()])
        .unwrap()
        .with_executable_paths([
            x64.to_string_lossy().to_string(),
            x86.to_string_lossy().to_string(),
        ]);
    let mut bundle = RuntimeBundle::open(&root).unwrap();
    assert_eq!(
        bundle
            .adapter_options()
            .iter()
            .filter(|option| option.id() == TEST_ADAPTER_ID)
            .count(),
        1
    );
    let option = bundle
        .adapter_options()
        .iter()
        .find(|option| option.id() == TEST_ADAPTER_ID)
        .unwrap();
    assert!(option.architectures().iter().any(|a| a.as_ref() == "x86"));
    assert!(option
        .architectures()
        .iter()
        .any(|a| a.as_ref() == "x86_64"));
    let mut runtime = bundle.discover(id.clone(), &spec).unwrap();
    let targets = runtime
        .targets()
        .map(|target| target.id())
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2);
    let capture_path = data.path().join("capture.json");
    let capture = CaptureConfiguration::new(
        CaptureSessionId::new("dual-gdi").unwrap(),
        &capture_path,
        1000,
    )
    .unwrap();
    runtime
        .start_capture(
            targets,
            [Feature::TextObserve, Feature::TextReplace],
            capture,
        )
        .unwrap();
    assert_eq!(runtime.active_target_count(), 2);
    runtime
        .publish(translation_spec.publication().clone())
        .unwrap();
    let first64 = target64.render();
    let first32 = target64.render_command("render-child render");
    assert_ne!(first64, baseline64);
    assert_ne!(first32, baseline32);
    backend
        .update_dictionary(
            DictionaryEdit::new(
                dictionary.id(),
                dictionary.metadata().name(),
                dictionary.metadata().source_locale(),
                dictionary.metadata().target_locale(),
                dictionary.revision(),
            )
            .with_entries([DictionaryEntryCreate::new("Open", "第二代文字")]),
        )
        .unwrap();
    let updated = backend.workflow_runtime_spec("workflow.dual", &id).unwrap();
    runtime.publish(updated.publication().clone()).unwrap();
    assert_ne!(target64.render(), first64);
    assert_ne!(target64.render_command("render-child render"), first32);
    target64.render_command("render-child render-text Only x86");
    target64.render_command("render-text Only x64");
    std::thread::sleep(Duration::from_millis(350));
    runtime.stop().unwrap();
    assert_eq!(target64.render(), baseline64);
    assert_eq!(target64.render_command("render-child render"), baseline32);
    let catalog = CaptureCatalog::read_current(&capture_path).unwrap();
    assert!(catalog
        .entries()
        .iter()
        .any(|entry| entry.source() == "Only x86"));
    assert!(catalog
        .entries()
        .iter()
        .any(|entry| entry.source() == "Only x64"));
    // The controlling processes can be replaced without changing either target process.
    drop(runtime);
    let mut reconnected = bundle.discover(id, &spec).unwrap();
    let ids = reconnected
        .targets()
        .map(|target| target.id())
        .collect::<Vec<_>>();
    let capture = CaptureConfiguration::new(
        CaptureSessionId::new("dual-reconnect").unwrap(),
        data.path().join("reconnect.json"),
        1000,
    )
    .unwrap();
    reconnected
        .start_capture(ids, [Feature::TextObserve, Feature::TextReplace], capture)
        .unwrap();
    reconnected
        .publish(translation_spec.publication().clone())
        .unwrap();
    assert_ne!(target64.render(), baseline64);
    assert_ne!(target64.render_command("render-child render"), baseline32);
    reconnected.stop().unwrap();
    assert_eq!(target64.render(), baseline64);
    assert_eq!(target64.render_command("render-child render"), baseline32);
    target64.stop();
}

#[test]
#[ignore = "requires the dual-architecture Runtime bundle built with -IncludeTestTarget"]
fn x86_native_drawing_adapters_use_their_native_calling_conventions() {
    let root = std::path::PathBuf::from(std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT").unwrap());
    let executable = root.join("test-target-x86.exe");
    for (adapter, command) in [
        ("windows.gdi.text-out", "render-text-out"),
        ("windows.user32.draw-text", "render-draw-text"),
        ("windows.gdiplus.draw-string", "render-gdiplus"),
        ("windows.directwrite.text-layout", "render-directwrite"),
    ] {
        let mut target = TargetProcess::spawn(&executable);
        let baseline = target.render_command(command);
        let data = tempfile::Builder::new()
            .prefix("x86-gdi-")
            .tempdir_in(root.parent().unwrap())
            .unwrap();
        let mut backend = open_backend(data.path(), &root);
        let snapshot = backend
            .add_software(ExecutableSelection::new(&executable))
            .unwrap();
        let id = snapshot.software()[0].id().to_owned();
        let (_, spec) = create_text_workflow_with_adapter(
            &mut backend,
            &id,
            adapter,
            "dictionary.x86",
            "workflow.x86",
            "Open",
            "中文替换",
        );
        let mut bundle = RuntimeBundle::open(&root).unwrap();
        let mut runtime = bundle.discover(id, &spec).unwrap();
        let target_id = runtime.targets().next().unwrap().id();
        runtime.start(target_id, [Feature::TextReplace]).unwrap();
        assert_ne!(target.render_command(command), baseline);
        runtime.stop().unwrap();
        assert_eq!(target.render_command(command), baseline);
        target.stop();
    }
}

#[test]
#[ignore = "requires the dual-architecture Runtime bundle built with -IncludeTestTarget"]
fn x86_gdi_font_substitution_and_restore_use_the_same_public_pipeline() {
    let root = std::path::PathBuf::from(std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT").unwrap());
    let executable = root.join("test-target-x86.exe");
    let mut target = TargetProcess::spawn(&executable);
    let baseline = target.render();
    let data = tempfile::Builder::new()
        .prefix("x86-font-")
        .tempdir_in(root.parent().unwrap())
        .unwrap();
    let mut backend = open_backend(data.path(), &root);
    let snapshot = backend
        .add_software(ExecutableSelection::new(&executable))
        .unwrap();
    let id = snapshot.software()[0].id().to_owned();
    let spec = create_all_observations_font_workflow(&mut backend, &id, "Courier New");
    let mut bundle = RuntimeBundle::open(&root).unwrap();
    let mut runtime = bundle.discover(id, &spec).unwrap();
    let target_id = runtime.targets().next().unwrap().id();
    runtime.start(target_id, [Feature::FontSubstitute]).unwrap();
    assert_ne!(target.render(), baseline);
    runtime.stop().unwrap();
    assert_eq!(target.render(), baseline);
    target.stop();
}
