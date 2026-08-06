use super::*;

#[test]
#[ignore = "requires the local Windows Runtime bundle built by scripts/dev-app.ps1"]
fn desktop_runtime_pool_keeps_two_software_active_and_isolates_stop() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let source_target = runtime_root.join("test-target.exe");
    let targets = tempdir().expect("isolated target executables");
    let alpha_executable = targets.path().join("AlphaCanvas.exe");
    let beta_executable = targets.path().join("BetaCanvas.exe");
    std::fs::copy(&source_target, &alpha_executable).expect("alpha target executable");
    std::fs::copy(&source_target, &beta_executable).expect("beta target executable");
    let mut alpha_target = TargetProcess::spawn(&alpha_executable);
    let mut beta_target = TargetProcess::spawn(&beta_executable);
    let alpha_baseline = alpha_target.render();
    let beta_baseline = beta_target.render();

    let data = tempdir().expect("isolated desktop data");
    let mut backend = open_backend(data.path(), &runtime_root);
    let alpha_added = backend
        .add_software(ExecutableSelection::new(&alpha_executable))
        .expect("register alpha software");
    let alpha_id = alpha_added
        .selected_software_id()
        .expect("selected alpha software")
        .to_owned();
    let (_, alpha_spec) = create_text_workflow(
        &mut backend,
        &alpha_id,
        "dictionary.alpha",
        "workflow.alpha",
        "Open",
        "Alpha translated label",
    );
    let beta_added = backend
        .add_software(ExecutableSelection::new(&beta_executable))
        .expect("register beta software");
    let beta_id = beta_added
        .selected_software_id()
        .expect("selected beta software")
        .to_owned();
    let (_, beta_spec) = create_text_workflow(
        &mut backend,
        &beta_id,
        "dictionary.beta",
        "workflow.beta",
        "Open",
        "Beta translated label",
    );

    let mut pool = DesktopRuntimePool::new(
        RuntimeBundle::open(&runtime_root).expect("verified Runtime bundle"),
    );
    let alpha = pool
        .set_features(alpha_id.clone(), &alpha_spec, None, [Feature::TextReplace])
        .expect("activate alpha Runtime");
    let beta = pool
        .set_features(beta_id.clone(), &beta_spec, None, [Feature::TextReplace])
        .expect("activate beta Runtime");
    assert!(alpha.is_active());
    assert!(beta.is_active());
    assert_ne!(alpha_target.render(), alpha_baseline);
    assert_ne!(beta_target.render(), beta_baseline);

    pool.set_features(alpha_id.clone(), &alpha_spec, None, std::iter::empty())
        .expect("stop alpha Runtime");
    assert_eq!(alpha_target.render(), alpha_baseline);
    assert_ne!(beta_target.render(), beta_baseline);
    assert!(!pool
        .status(&alpha_id)
        .is_some_and(|status| status.is_active()));
    assert!(pool
        .status(&beta_id)
        .is_some_and(|status| status.is_active()));

    pool.set_features(beta_id, &beta_spec, None, std::iter::empty())
        .expect("stop beta Runtime");
    assert_eq!(beta_target.render(), beta_baseline);
    alpha_target.stop();
    beta_target.stop();
}
