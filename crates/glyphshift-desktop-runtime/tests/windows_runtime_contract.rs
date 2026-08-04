#![cfg(windows)]

use glyphshift_capture::{CaptureCatalog, CaptureConfiguration, CaptureSessionId};
use glyphshift_desktop_backend::{
    DesktopBackend, DesktopEnvironment, DesktopRuntimeSpec, DictionaryCreate, DictionaryEdit,
    DictionaryEntryCreate, DictionaryView, ExecutableSelection, FontCoverage, WorkflowCreate,
    WorkflowFontPolicy, WorkflowTargetCreate,
};
use glyphshift_desktop_runtime::{DesktopRuntimeError, DesktopRuntimePool, RuntimeBundle};
use glyphshift_domain::Feature;
use glyphshift_session::{RuntimeTextOutcome, RuntimeTraceStatus};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::tempdir;

const TEST_ADAPTER_ID: &str = "windows.gdi.ext-text-out";
const TEST_GDIPLUS_ADAPTER_ID: &str = "windows.gdiplus.draw-string";
const TEST_CONSOLE_OBSERVER_ID: &str = "windows.console.write-console";
const TEST_UIA_OBSERVER_ID: &str = "windows.uia.observe";

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
    assert!(!bundle
        .translation_adapter_ids()
        .iter()
        .any(|adapter_id| adapter_id.as_ref() == TEST_UIA_OBSERVER_ID));
}

#[test]
#[ignore = "requires the local Windows Runtime bundle built by scripts/build-runtime-bundle.ps1"]
fn runtime_bundle_rejects_a_missing_isolated_worker_artifact() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/local-test/evidence/runtime-bundle-missing-worker");
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

#[test]
#[ignore = "requires the local Windows Runtime bundle with its synthetic target"]
fn desktop_bundle_captures_uia_public_text_without_password_content() {
    let runtime_root = std::env::var_os("GLYPHSHIFT_RUNTIME_ROOT")
        .map(std::path::PathBuf::from)
        .expect("local Runtime bundle root");
    let target_executable = runtime_root.join("test-target.exe");
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/local-test/evidence/desktop-uia-capture");
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
        .join("../../target/local-test/evidence/desktop-central-capture");
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

fn open_backend(data_root: &Path, runtime_root: &Path) -> DesktopBackend {
    let bundle = RuntimeBundle::open(runtime_root).expect("verified Runtime bundle");
    DesktopBackend::open_with_environment(
        data_root,
        DesktopEnvironment::new(
            bundle.adapter_requirements().iter().cloned(),
            ["Arial", "Courier New"],
        ),
    )
    .expect("desktop backend")
}

struct TargetProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl TargetProcess {
    fn spawn(executable: &Path) -> Self {
        Self::spawn_with_args(executable, std::iter::empty::<&str>())
    }

    fn spawn_with_args<'a>(
        executable: &Path,
        arguments: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        use std::os::windows::process::CommandExt;

        let mut child = Command::new(executable)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .expect("isolated target should start");
        let stdin = child.stdin.take().expect("target stdin");
        let stdout = BufReader::new(child.stdout.take().expect("target stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn render(&mut self) -> String {
        self.render_command("render")
    }

    fn render_command(&mut self, command: &str) -> String {
        writeln!(self.stdin, "{command}").expect("render command");
        self.stdin.flush().expect("flush render command");
        self.read_response()
    }

    fn read_response(&mut self) -> String {
        let mut evidence = String::new();
        self.stdout
            .read_line(&mut evidence)
            .expect("read target evidence");
        assert!(!evidence.is_empty(), "target should return pixel evidence");
        evidence.trim().to_owned()
    }

    fn stop(&mut self) {
        let _ = writeln!(self.stdin, "exit");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

fn create_all_observations_font_workflow(
    backend: &mut DesktopBackend,
    software_id: &str,
    family: &str,
) -> DesktopRuntimeSpec {
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.font-safety", "Font safety").with_targets([
                WorkflowTargetCreate::new(
                    software_id,
                    [TEST_ADAPTER_ID, TEST_GDIPLUS_ADAPTER_ID],
                    [] as [&str; 0],
                )
                .with_font_policy(WorkflowFontPolicy::new(
                    [family],
                    FontCoverage::AllObservations,
                )),
            ]),
        )
        .expect("create all-observations font workflow");
    backend
        .workflow_runtime_spec("workflow.font-safety", software_id)
        .expect("compiled font safety Runtime spec")
}

impl Drop for TargetProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn create_text_workflow(
    backend: &mut DesktopBackend,
    software_id: &str,
    dictionary_id: &str,
    workflow_id: &str,
    source: &str,
    translation: &str,
) -> (DictionaryView, DesktopRuntimeSpec) {
    create_text_workflow_with_adapter(
        backend,
        software_id,
        TEST_ADAPTER_ID,
        dictionary_id,
        workflow_id,
        source,
        translation,
    )
}

fn create_text_workflow_with_adapter(
    backend: &mut DesktopBackend,
    software_id: &str,
    adapter_id: &str,
    dictionary_id: &str,
    workflow_id: &str,
    source: &str,
    translation: &str,
) -> (DictionaryView, DesktopRuntimeSpec) {
    let dictionary = backend
        .create_dictionary(
            DictionaryCreate::new(dictionary_id, dictionary_id, "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new(source, translation)]),
        )
        .expect("create Runtime dictionary");
    backend
        .create_workflow(WorkflowCreate::new(workflow_id, workflow_id).with_targets([
            WorkflowTargetCreate::new(software_id, [adapter_id], [dictionary_id]),
        ]))
        .expect("create Runtime workflow");
    let spec = backend
        .workflow_runtime_spec(workflow_id, software_id)
        .expect("compiled workflow Runtime spec");
    (dictionary, spec)
}

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
