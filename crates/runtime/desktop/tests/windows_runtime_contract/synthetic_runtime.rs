use super::*;

#[test]
#[ignore = "requires the local Windows Runtime bundle built by scripts/dev-app.ps1"]
fn desktop_runtime_changes_pixels_updates_and_restores_pass_through() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let target_executable = runtime_root.join("test-target.exe");
    let mut target = TargetProcess::spawn(&target_executable);
    let baseline = target.render();

    let data = tempdir().expect("isolated desktop data");
    let mut backend = open_backend(data.path(), &runtime_root);
    let snapshot = backend
        .add_software(ExecutableSelection::new(&target_executable))
        .expect("register target executable");
    let application_id = snapshot.software()[0].id().to_owned();
    let (dictionary, spec) = create_text_workflow(
        &mut backend,
        &application_id,
        "dictionary.runtime-update",
        "workflow.runtime-update",
        "Open",
        "First translated label",
    );

    let mut bundle = RuntimeBundle::open(&runtime_root).expect("verified Runtime bundle");
    let mut runtime = bundle
        .discover(application_id.clone(), &spec)
        .expect("target discovery");
    let target_id = runtime
        .targets()
        .next()
        .expect("isolated target instance")
        .id();
    runtime
        .start(target_id, [Feature::TextReplace])
        .expect("target Runtime activation");
    assert!(runtime.is_feature_active(Feature::TextReplace));
    assert!(!runtime.is_feature_active(Feature::FontSubstitute));
    let translated = target.render();
    assert_ne!(translated, baseline, "desktop Runtime should replace text");

    backend
        .update_dictionary(
            DictionaryEdit::new(
                dictionary.id(),
                dictionary.metadata().name(),
                dictionary.metadata().source_locale(),
                dictionary.metadata().target_locale(),
                dictionary.revision(),
            )
            .with_entries([DictionaryEntryCreate::new(
                "Open",
                "Second translated label",
            )]),
        )
        .expect("second translation");
    let second_spec = backend
        .workflow_runtime_spec("workflow.runtime-update", &application_id)
        .expect("updated Runtime spec");
    runtime
        .publish(second_spec.publication().clone())
        .expect("Runtime publication update");
    let updated = target.render();
    assert_ne!(
        updated, translated,
        "target should apply the next generation"
    );

    runtime.stop().expect("Runtime pass-through");
    assert!(!runtime.is_feature_active(Feature::TextReplace));
    assert_eq!(
        target.render(),
        baseline,
        "stop should restore pass-through"
    );

    let spec = backend
        .workflow_runtime_spec("workflow.runtime-update", &application_id)
        .expect("restart Runtime spec");
    let mut restarted = bundle
        .discover(application_id, &spec)
        .expect("restart target discovery");
    let restarted_target_id = restarted
        .targets()
        .next()
        .expect("restart target instance")
        .id();
    restarted
        .start(restarted_target_id, [Feature::TextReplace])
        .expect("target Runtime reactivation after pass-through");
    assert_ne!(
        target.render(),
        baseline,
        "restarting should reactivate visible replacement"
    );
    restarted.stop().expect("second Runtime pass-through");
    assert_eq!(
        target.render(),
        baseline,
        "second stop should restore pass-through"
    );
    target.stop();
}

#[test]
#[ignore = "requires the local Windows Runtime bundle built by scripts/build-runtime-bundle.ps1"]
fn desktop_runtime_updates_gdiplus_translation_without_restarting_the_target() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let target_executable = runtime_root.join("test-target.exe");
    let mut target = TargetProcess::spawn(&target_executable);
    let baseline = target.render_command("render-gdiplus");

    let data = tempdir().expect("isolated desktop data");
    let mut backend = open_backend(data.path(), &runtime_root);
    let snapshot = backend
        .add_software(ExecutableSelection::new(&target_executable))
        .expect("register target executable");
    let application_id = snapshot.software()[0].id().to_owned();
    let (dictionary, spec) = create_text_workflow_with_adapter(
        &mut backend,
        &application_id,
        TEST_GDIPLUS_ADAPTER_ID,
        "dictionary.gdiplus-update",
        "workflow.gdiplus-update",
        "Open",
        "First GDI+ label",
    );
    let initial_generation = spec.publication().generation().value();

    let mut bundle = RuntimeBundle::open(&runtime_root).expect("verified Runtime bundle");
    let mut runtime = bundle
        .discover(application_id.clone(), &spec)
        .expect("target discovery");
    let target_id = runtime.targets().next().expect("isolated target").id();
    runtime
        .start(target_id, [Feature::TextReplace])
        .expect("GDI+ Runtime activation");
    runtime
        .control_runtime_diagnostics(true)
        .expect("enable GDI+ diagnostics");

    let translated = target.render_command("render-gdiplus");
    assert_ne!(translated, baseline, "GDI+ should replace source text");
    let initial_diagnostics = runtime
        .query_runtime_diagnostics()
        .expect("query initial GDI+ diagnostics");
    assert!(initial_diagnostics.records().iter().any(|record| {
        record.adapter_id() == TEST_GDIPLUS_ADAPTER_ID
            && record.source_text() == "Open"
            && record.generation() == initial_generation
            && record.status() == RuntimeTraceStatus::Matched
            && record.text() == RuntimeTextOutcome::Replaced
    }));

    backend
        .update_dictionary(
            DictionaryEdit::new(
                dictionary.id(),
                dictionary.metadata().name(),
                dictionary.metadata().source_locale(),
                dictionary.metadata().target_locale(),
                dictionary.revision(),
            )
            .with_entries([DictionaryEntryCreate::new("Open", "Second GDI+ label")]),
        )
        .expect("update GDI+ dictionary");
    let updated_spec = backend
        .workflow_runtime_spec("workflow.gdiplus-update", &application_id)
        .expect("compile updated GDI+ Runtime spec");
    let updated_generation = updated_spec.publication().generation().value();
    assert!(updated_generation > initial_generation);
    runtime
        .publish(updated_spec.publication().clone())
        .expect("publish updated GDI+ dictionary");

    let updated = target.render_command("render-gdiplus");
    assert_ne!(updated, translated, "GDI+ should apply the next generation");
    let updated_diagnostics = runtime
        .query_runtime_diagnostics()
        .expect("query updated GDI+ diagnostics");
    assert!(updated_diagnostics.records().iter().any(|record| {
        record.adapter_id() == TEST_GDIPLUS_ADAPTER_ID
            && record.source_text() == "Open"
            && record.generation() == updated_generation
            && record.status() == RuntimeTraceStatus::Matched
            && record.text() == RuntimeTextOutcome::Replaced
    }));

    runtime
        .control_runtime_diagnostics(false)
        .expect("disable GDI+ diagnostics");
    runtime.stop().expect("GDI+ Runtime pass-through");
    assert_eq!(target.render_command("render-gdiplus"), baseline);
    target.stop();
}

#[test]
#[ignore = "requires the local Windows Runtime bundle built by scripts/dev-app.ps1"]
fn desktop_runtime_protects_gdi_symbol_fonts_and_documents_the_gdiplus_risk() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let target_executable = runtime_root.join("test-target.exe");
    let mut target = TargetProcess::spawn(&target_executable);
    let baseline_text = target.render();
    let baseline_gdi_glyphs = target.render_command("render-gdi-glyph-indices");
    let baseline_gdi_symbol = target.render_command("render-gdi-symbol");
    let baseline_gdiplus_symbol = target.render_command("render-gdiplus-symbol");

    let data = tempdir().expect("isolated desktop data");
    let mut backend = open_backend(data.path(), &runtime_root);
    let snapshot = backend
        .add_software(ExecutableSelection::new(&target_executable))
        .expect("register target executable");
    let application_id = snapshot.software()[0].id().to_owned();
    let spec = create_all_observations_font_workflow(&mut backend, &application_id, "Arial");

    let mut bundle = RuntimeBundle::open(&runtime_root).expect("verified Runtime bundle");
    let mut runtime = bundle
        .discover(application_id, &spec)
        .expect("target discovery");
    let target_id = runtime
        .targets()
        .next()
        .expect("isolated target instance")
        .id();
    runtime
        .start(target_id, [Feature::FontSubstitute])
        .expect("font Runtime activation");

    let substituted_text = target.render();
    assert_ne!(
        substituted_text, baseline_text,
        "ordinary GDI text should use the configured substitute font"
    );
    assert_eq!(
        target.render_command("render-gdi-glyph-indices"),
        substituted_text,
        "font-only substitution must decode glyph indices before changing the font"
    );
    assert_eq!(
        target.render_command("render-gdi-symbol"),
        baseline_gdi_symbol,
        "GDI SYMBOL_CHARSET text must retain its original font"
    );
    assert_ne!(
        target.render_command("render-gdiplus-symbol"),
        baseline_gdiplus_symbol,
        "GDI+ still substitutes a symbol font because it exposes no reliable font category"
    );

    runtime.stop().expect("Runtime pass-through");
    assert_eq!(target.render(), baseline_text);
    assert_eq!(
        target.render_command("render-gdi-glyph-indices"),
        baseline_gdi_glyphs
    );
    assert_eq!(
        target.render_command("render-gdi-symbol"),
        baseline_gdi_symbol
    );
    assert_eq!(
        target.render_command("render-gdiplus-symbol"),
        baseline_gdiplus_symbol
    );
    target.stop();
}

#[test]
#[ignore = "requires the local Windows Runtime bundle built by scripts/dev-app.ps1"]
fn desktop_runtime_refresh_reconnects_requested_features_after_target_restart() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let target_executable = runtime_root.join("test-target.exe");
    let mut target = TargetProcess::spawn(&target_executable);

    let data = tempdir().expect("isolated desktop data");
    let mut backend = open_backend(data.path(), &runtime_root);
    let snapshot = backend
        .add_software(ExecutableSelection::new(&target_executable))
        .expect("register target executable");
    let application_id = snapshot.software()[0].id().to_owned();
    let (_, spec) = create_text_workflow(
        &mut backend,
        &application_id,
        "dictionary.runtime-reconnect",
        "workflow.runtime-reconnect",
        "Open",
        "Reconnected label",
    );
    let bundle = RuntimeBundle::open(&runtime_root).expect("verified Runtime bundle");
    let mut pool = DesktopRuntimePool::new(bundle);
    let active = pool
        .set_features(application_id.clone(), &spec, None, [Feature::TextReplace])
        .expect("initial activation");
    assert!(active.is_feature_requested(Feature::TextReplace));
    assert!(active.is_feature_active(Feature::TextReplace));

    target.stop();
    let mut restarted_target = TargetProcess::spawn(&target_executable);
    let restarted_baseline = restarted_target.render();
    let refreshed = pool
        .refresh(application_id.clone(), &spec)
        .expect("refresh after target restart");
    assert!(refreshed.is_feature_requested(Feature::TextReplace));
    assert!(refreshed.is_feature_active(Feature::TextReplace));
    assert_ne!(
        restarted_target.render(),
        restarted_baseline,
        "refresh must reapply the previously requested translation feature"
    );

    pool.set_features(application_id, &spec, None, [])
        .expect("stop refreshed Runtime");
    restarted_target.stop();
}
