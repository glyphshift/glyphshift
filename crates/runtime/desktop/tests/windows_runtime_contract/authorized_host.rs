use super::*;

#[test]
#[ignore = "requires an explicitly authorized, already-running Windows host"]
fn desktop_runtime_activates_in_an_authorized_real_host() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let host_executable = std::env::var_os("GLYPHSHIFT_REAL_HOST_EXECUTABLE")
        .map(std::path::PathBuf::from)
        .expect("authorized host executable path");
    let adapter_id = std::env::var("GLYPHSHIFT_REAL_HOST_ADAPTER_ID")
        .unwrap_or_else(|_| TEST_ADAPTER_ID.to_owned());
    let requested_feature = match std::env::var("GLYPHSHIFT_REAL_HOST_FEATURE").as_deref() {
        Ok("observe") => Feature::TextObserve,
        _ => Feature::TextReplace,
    };
    let source = std::env::var("GLYPHSHIFT_REAL_HOST_SOURCE").unwrap_or_else(|_| "File".into());
    let translation =
        std::env::var("GLYPHSHIFT_REAL_HOST_TRANSLATION").unwrap_or_else(|_| "文件".into());
    let updated_translation = std::env::var("GLYPHSHIFT_REAL_HOST_TRANSLATION_UPDATE").ok();
    let require_replacement_hits = std::env::var("GLYPHSHIFT_REAL_HOST_REQUIRE_HITS")
        .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "yes"));

    let data = tempdir().expect("isolated desktop data");
    let mut backend = open_backend(data.path(), &runtime_root);
    let snapshot = backend
        .add_software(ExecutableSelection::new(&host_executable))
        .expect("register authorized host executable");
    let application_id = snapshot.software()[0].id().to_owned();
    let (dictionary, spec) = if requested_feature == Feature::TextObserve {
        (
            None,
            backend
                .capture_runtime_spec(&application_id, &[adapter_id.clone().into_boxed_str()])
                .expect("compiled authorized capture Runtime spec"),
        )
    } else {
        let (dictionary, spec) = create_text_workflow_with_adapter(
            &mut backend,
            &application_id,
            &adapter_id,
            "dictionary.real-host",
            "workflow.real-host",
            &source,
            &translation,
        );
        (Some(dictionary), spec)
    };

    let mut bundle = RuntimeBundle::open(&runtime_root).expect("verified Runtime bundle");
    let mut runtime = bundle
        .discover(application_id.clone(), &spec)
        .expect("authorized host discovery");
    let target_id = runtime
        .targets()
        .next()
        .expect("authorized host must already be running")
        .id();
    runtime
        .start(target_id, [requested_feature])
        .expect("authorized host Runtime activation");
    assert!(runtime.is_feature_active(requested_feature));
    if let Some(hold_ms) = std::env::var_os("GLYPHSHIFT_REAL_HOST_HOLD_MS")
        .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
    {
        println!("authorized host Runtime is active");
        runtime
            .control_runtime_diagnostics(true)
            .expect("enable authorized host diagnostics");
        let initial_generation = spec.publication().generation().value();
        let initial_hold_ms = if updated_translation.is_some() {
            hold_ms / 2
        } else {
            hold_ms
        };
        let deadline = Instant::now() + Duration::from_millis(initial_hold_ms);
        let mut hit_count = 0_usize;
        let mut initial_replacement_hits = 0_usize;
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(250));
            let batch = runtime
                .query_runtime_diagnostics()
                .expect("query authorized host diagnostics");
            for record in batch
                .records()
                .iter()
                .filter(|record| record.adapter_id() == adapter_id)
            {
                hit_count += 1;
                if record.source_text() == source
                    && record.generation() == initial_generation
                    && record.status() == RuntimeTraceStatus::Matched
                    && record.text() == RuntimeTextOutcome::Replaced
                {
                    initial_replacement_hits += 1;
                }
            }
        }

        let mut updated_replacement_hits = 0_usize;
        if let (Some(dictionary), Some(updated_translation)) =
            (dictionary.as_ref(), updated_translation.as_deref())
        {
            assert_eq!(requested_feature, Feature::TextReplace);
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
                        source.as_str(),
                        updated_translation,
                    )]),
                )
                .expect("update authorized host dictionary");
            let updated_spec = backend
                .workflow_runtime_spec("workflow.real-host", &application_id)
                .expect("compile updated authorized host Runtime spec");
            let updated_generation = updated_spec.publication().generation().value();
            assert!(updated_generation > initial_generation);
            runtime
                .publish(updated_spec.publication().clone())
                .expect("publish updated authorized host dictionary");

            let deadline = Instant::now() + Duration::from_millis(hold_ms - initial_hold_ms);
            while Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(250));
                let batch = runtime
                    .query_runtime_diagnostics()
                    .expect("query updated authorized host diagnostics");
                for record in batch
                    .records()
                    .iter()
                    .filter(|record| record.adapter_id() == adapter_id)
                {
                    hit_count += 1;
                    if record.source_text() == source
                        && record.generation() == updated_generation
                        && record.status() == RuntimeTraceStatus::Matched
                        && record.text() == RuntimeTextOutcome::Replaced
                    {
                        updated_replacement_hits += 1;
                    }
                }
            }
        }
        runtime
            .control_runtime_diagnostics(false)
            .expect("disable authorized host diagnostics");
        println!("authorized host adapter diagnostics hits: {hit_count}");
        println!("authorized host initial replacement hits: {initial_replacement_hits}");
        println!("authorized host updated replacement hits: {updated_replacement_hits}");
        if require_replacement_hits && requested_feature == Feature::TextReplace {
            assert!(
                initial_replacement_hits > 0,
                "authorized host must replace the configured source text"
            );
            if updated_translation.is_some() {
                assert!(
                    updated_replacement_hits > 0,
                    "authorized host must use the updated dictionary generation"
                );
            }
        }
    }
    runtime.stop().expect("authorized host pass-through");
}

#[test]
#[ignore = "requires an explicitly authorized, already-running Windows host"]
fn desktop_runtime_captures_an_authorized_real_host() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let host_executable = std::env::var_os("GLYPHSHIFT_REAL_HOST_EXECUTABLE")
        .map(std::path::PathBuf::from)
        .expect("authorized host executable path");
    let adapter_ids = std::env::var("GLYPHSHIFT_REAL_HOST_ADAPTER_IDS")
        .expect("authorized capture Adapter ids")
        .split(';')
        .filter(|adapter_id| !adapter_id.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(!adapter_ids.is_empty(), "at least one Adapter is required");
    let output = std::env::var_os("GLYPHSHIFT_REAL_HOST_CAPTURE_OUTPUT")
        .map(std::path::PathBuf::from)
        .expect("local capture output path");
    let hold_ms = std::env::var("GLYPHSHIFT_REAL_HOST_HOLD_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5_000);
    let evidence_root = output.parent().expect("capture output parent");
    std::fs::create_dir_all(evidence_root).expect("capture evidence root");
    let data = tempfile::Builder::new()
        .prefix("contract-")
        .tempdir_in(evidence_root)
        .expect("isolated authorized capture data");
    let mut backend = open_backend(data.path(), &runtime_root);
    let snapshot = backend
        .add_software(ExecutableSelection::new(&host_executable))
        .expect("register authorized host executable");
    let application_id = snapshot.software()[0].id().to_owned();
    let spec = backend
        .capture_runtime_spec(
            &application_id,
            &adapter_ids
                .iter()
                .cloned()
                .map(String::into_boxed_str)
                .collect::<Vec<_>>(),
        )
        .expect("compiled authorized capture Runtime spec");
    let capture = CaptureConfiguration::new(
        CaptureSessionId::new("authorized-real-host").expect("capture session id"),
        &output,
        5_000,
    )
    .expect("capture configuration");
    let bundle = RuntimeBundle::open(&runtime_root).expect("verified Runtime bundle");
    let mut pool = DesktopRuntimePool::new(bundle);

    pool.start_capture(application_id.clone(), &spec, None, capture)
        .expect("start authorized host capture");
    println!("authorized host capture is active");
    std::thread::sleep(Duration::from_millis(hold_ms));
    pool.stop_capture(application_id)
        .expect("stop authorized host capture");

    let catalog = CaptureCatalog::read_current(&output).expect("authorized host capture catalog");
    println!(
        "authorized host captured unique entries: {}",
        catalog.entries().len()
    );
}

#[test]
#[ignore = "requires an explicitly authorized, already-running Windows host"]
fn desktop_runtime_applies_and_restores_font_policy_in_an_authorized_real_host() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let host_executable = std::env::var_os("GLYPHSHIFT_REAL_HOST_EXECUTABLE")
        .map(std::path::PathBuf::from)
        .expect("authorized host executable path");

    let data = tempdir().expect("isolated desktop data");
    let mut backend = open_backend(data.path(), &runtime_root);
    let snapshot = backend
        .add_software(ExecutableSelection::new(&host_executable))
        .expect("register authorized host executable");
    let application_id = snapshot.software()[0].id().to_owned();
    let family = std::env::var("GLYPHSHIFT_REAL_HOST_FONT").unwrap_or_else(|_| "Arial".to_owned());
    let spec = create_all_observations_font_workflow(&mut backend, &application_id, &family);

    let mut bundle = RuntimeBundle::open(&runtime_root).expect("verified Runtime bundle");
    let mut runtime = bundle
        .discover(application_id, &spec)
        .expect("authorized host discovery");
    let target_id = runtime
        .targets()
        .next()
        .expect("authorized host must already be running")
        .id();
    runtime
        .start(target_id, [Feature::FontSubstitute])
        .expect("authorized host font Runtime activation");
    assert!(runtime.is_feature_active(Feature::FontSubstitute));
    if let Some(hold_ms) = std::env::var_os("GLYPHSHIFT_REAL_HOST_HOLD_MS")
        .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
    {
        println!("authorized host font Runtime is active");
        runtime
            .control_runtime_diagnostics(true)
            .expect("enable authorized host diagnostics");
        let deadline = Instant::now() + Duration::from_millis(hold_ms);
        let mut observed_adapters = BTreeSet::new();
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(250));
            let batch = runtime
                .query_runtime_diagnostics()
                .expect("query authorized host diagnostics");
            observed_adapters.extend(
                batch
                    .records()
                    .iter()
                    .map(|record| record.adapter_id().to_owned()),
            );
        }
        assert!(
            observed_adapters.contains(TEST_ADAPTER_ID),
            "authorized host should exercise the GDI menu path"
        );
        assert!(
            observed_adapters.contains(TEST_GDIPLUS_ADAPTER_ID),
            "authorized host should exercise the GDI+ panel path"
        );
        runtime
            .control_runtime_diagnostics(false)
            .expect("disable authorized host diagnostics");
        println!("authorized host diagnostics observed both GDI and GDI+ paths");
    }
    runtime.stop().expect("authorized host font pass-through");
}
