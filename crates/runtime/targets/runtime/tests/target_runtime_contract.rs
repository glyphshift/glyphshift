#![cfg(windows)]

use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_capture::{CaptureCatalog, CaptureConfiguration, CaptureSessionId};
use glyphshift_domain::{
    Feature, FontDecision, Generation, RenderDecision, RouteProgram, TextDecision,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_target_runtime::{
    activate_deployment, control_diagnostics, deactivate_runtime, glyphshift_runtime_activate_v1,
    glyphshift_runtime_diagnostics_query_v1, query_activation, query_diagnostics,
    update_publication,
};
use glyphshift_target_runtime_contract::{
    NativeAdapterDeployment, RuntimeCommandV1, RuntimeDiagnosticsControl,
    RuntimeDiagnosticsQueryV1, RuntimeTextOutcome, RuntimeTraceBatch, RuntimeTraceStatus,
    TargetRuntimeDeployment, MAX_RUNTIME_TRACE_BYTES, STATUS_TARGET_RUNTIME_OK,
    STATUS_TARGET_RUNTIME_UPDATE_REJECTED,
};
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use glyphshift_windows_host::{render_gdiplus, render_gdiplus_text, render_raw_gdi_unicode};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{
    GetUpdateRect, RedrawWindow, ValidateRect, RDW_NOCHILDREN, RDW_NOFRAME, RDW_VALIDATE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, WS_OVERLAPPED, WS_VISIBLE,
};

struct RedrawContractWindow(HWND);

impl RedrawContractWindow {
    fn new() -> Self {
        let class_name = "STATIC\0".encode_utf16().collect::<Vec<_>>();
        let title = "Glyphshift lifecycle redraw contract\0"
            .encode_utf16()
            .collect::<Vec<_>>();
        let window = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPED | WS_VISIBLE,
                -10_000,
                -10_000,
                240,
                120,
                null_mut(),
                null_mut(),
                null_mut(),
                null(),
            )
        };
        assert!(!window.is_null(), "create lifecycle redraw window");
        Self(window)
    }

    fn validate(&self) {
        unsafe {
            ValidateRect(self.0, null());
            RedrawWindow(
                self.0,
                null(),
                null_mut(),
                RDW_VALIDATE | RDW_NOCHILDREN | RDW_NOFRAME,
            );
        }
        assert_eq!(unsafe { GetUpdateRect(self.0, null_mut(), 0) }, 0);
    }

    fn assert_redraw_requested(&self, operation: &str) {
        assert_ne!(
            unsafe { GetUpdateRect(self.0, null_mut(), 0) },
            0,
            "{operation} must request a host repaint"
        );
    }
}

impl Drop for RedrawContractWindow {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}

fn native_package() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join("glyphshift_adapter_gdi_native.dll")
}

fn gdiplus_native_package() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join("glyphshift_adapter_gdiplus_native.dll")
}

fn qt_painter_native_package() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join("glyphshift_adapter_qt_painter_native.dll")
}

fn refresh_native_package() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join("glyphshift_test_native_adapter.dll")
}

fn pass_decision() -> RenderDecision {
    RenderDecision {
        text: TextDecision::Keep,
        font: FontDecision::Keep,
        generation: Generation::new(1),
    }
}

fn publication(generation: u64, translation: &str) -> RuntimePublication {
    RuntimePublication::new(
        RouteProgram::direct("menu"),
        TranslationSnapshot::empty(Generation::new(generation)).with_entry(
            "menu",
            "Open",
            translation,
        ),
        FontPolicy::empty(),
    )
}

fn activate_status(deployment: &TargetRuntimeDeployment) -> u32 {
    let json = deployment.encode_json().expect("target Runtime deployment");
    let command = RuntimeCommandV1 {
        struct_size: std::mem::size_of::<RuntimeCommandV1>() as u32,
        json: json.as_ptr(),
        json_len: json.len() as u32,
    };
    unsafe { glyphshift_runtime_activate_v1(&command) }
}

#[test]
fn active_runtime_accepts_an_equivalent_deployment_as_a_new_owner() {
    let initial = TargetRuntimeDeployment::new(publication(1, "First owner"), []);
    assert_eq!(activate_status(&initial), STATUS_TARGET_RUNTIME_OK);

    assert_eq!(activate_status(&initial), STATUS_TARGET_RUNTIME_OK);

    assert_eq!(
        activate_status(&TargetRuntimeDeployment::new(
            publication(1, "Conflicting owner"),
            [],
        )),
        STATUS_TARGET_RUNTIME_UPDATE_REJECTED
    );

    assert_eq!(
        activate_status(&TargetRuntimeDeployment::new(
            publication(2, "Replacement owner"),
            [],
        )),
        STATUS_TARGET_RUNTIME_OK
    );

    deactivate_runtime().expect("replacement owner stops the target Runtime");
}

fn scoped_publication(generation: u64, translation: &str, adapter_id: &str) -> RuntimePublication {
    scoped_source_publication(generation, "Open", translation, adapter_id)
}

fn scoped_source_publication(
    generation: u64,
    source: &str,
    translation: &str,
    adapter_id: &str,
) -> RuntimePublication {
    RuntimePublication::new(
        RouteProgram::direct("menu"),
        TranslationSnapshot::empty(Generation::new(generation)).with_entry_for_adapters(
            "menu",
            source,
            translation,
            [adapter_id],
        ),
        FontPolicy::empty(),
    )
}

#[test]
#[ignore = "requires the native GDI Adapter DLL built before the capture contract"]
fn trh_002_capture_observes_real_adapter_text_and_writes_provenance_catalog() {
    let native_package = native_package();
    let native_hash = artifact_hash(&native_package);
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../local-test/target-runtime-contract");
    std::fs::create_dir_all(&local_test).expect("local test directory");
    let output_path = local_test.join("capture-native-contract.json");
    let _ = std::fs::remove_file(&output_path);
    let capture = CaptureConfiguration::new(
        CaptureSessionId::new("capture-native-contract").expect("capture session id"),
        output_path.clone(),
        100,
    )
    .expect("capture configuration");
    let deployment = TargetRuntimeDeployment::new(
        RuntimePublication::new(
            RouteProgram::direct("menu"),
            TranslationSnapshot::empty(Generation::new(1)),
            FontPolicy::empty(),
        ),
        [NativeAdapterDeployment::new(
            native_package,
            binding(native_hash, [Feature::TextObserve]),
        )
        .expect("GDI capture deployment")],
    )
    .with_capture(capture);

    activate_deployment(deployment).expect("activate capture deployment");
    render_raw_gdi_unicode("Captured label").expect("render observed text");
    deactivate_runtime().expect("finish capture deployment");

    let catalog = CaptureCatalog::read_current(&output_path).expect("capture catalog");
    assert!(catalog.entries().iter().any(|entry| {
        entry.source() == "Captured label"
            && entry.adapter_id() == glyphshift_adapter_gdi::ADAPTER_ID
            && entry.count() >= 1
    }));
}

fn artifact_hash(path: &Path) -> ArtifactHash {
    let mut file = File::open(path).expect("native package file");
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest).expect("native package digest");
    ArtifactHash::sha256(digest.finalize().into())
}

fn binding(hash: ArtifactHash, features: impl IntoIterator<Item = Feature>) -> AdapterBinding {
    let descriptor = glyphshift_adapter_gdi::descriptor();
    AdapterBinding {
        descriptor: descriptor.clone(),
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: hash,
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/gdi"),
        },
        features: features.into_iter().collect(),
    }
}

fn gdiplus_binding(
    hash: ArtifactHash,
    features: impl IntoIterator<Item = Feature>,
) -> AdapterBinding {
    let descriptor = glyphshift_adapter_gdiplus::descriptor();
    AdapterBinding {
        descriptor: descriptor.clone(),
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: hash,
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/gdiplus"),
        },
        features: features.into_iter().collect(),
    }
}

fn qt_painter_binding(
    hash: ArtifactHash,
    features: impl IntoIterator<Item = Feature>,
) -> AdapterBinding {
    let descriptor = glyphshift_adapter_qt_painter::descriptor();
    AdapterBinding {
        descriptor: descriptor.clone(),
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: hash,
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/qt-painter"),
        },
        features: features.into_iter().collect(),
    }
}

fn refresh_binding(hash: ArtifactHash) -> AdapterBinding {
    let descriptor = glyphshift_test_native_adapter::descriptor();
    AdapterBinding {
        descriptor: descriptor.clone(),
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: hash,
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/synthetic-refresh"),
        },
        features: vec![Feature::TextReplace],
    }
}

#[test]
#[ignore = "requires the synthetic Native Adapter DLL built before the target Runtime contract"]
fn trh_004_requests_adapter_refresh_after_each_lifecycle_change() {
    let package = refresh_native_package();
    let hash = artifact_hash(&package);
    let counter_library =
        unsafe { libloading::Library::new(&package) }.expect("load synthetic refresh Adapter");
    let refresh_count = unsafe {
        counter_library
            .get::<unsafe extern "C" fn() -> u32>(b"glyphshift_test_refresh_count_v1\0")
            .expect("resolve synthetic refresh counter")
    };
    let baseline = unsafe { refresh_count() };
    let deployment = TargetRuntimeDeployment::new(
        scoped_publication(
            1,
            "First translation",
            glyphshift_test_native_adapter::ADAPTER_ID,
        ),
        [NativeAdapterDeployment::new(package, refresh_binding(hash))
            .expect("synthetic refresh deployment")],
    );

    activate_deployment(deployment).expect("activate synthetic refresh Adapter");
    assert_eq!(unsafe { refresh_count() }, baseline + 1);

    update_publication(scoped_publication(
        2,
        "Second translation",
        glyphshift_test_native_adapter::ADAPTER_ID,
    ))
    .expect("publish next synthetic refresh generation");
    assert_eq!(unsafe { refresh_count() }, baseline + 2);

    deactivate_runtime().expect("deactivate synthetic refresh Adapter");
    assert_eq!(unsafe { refresh_count() }, baseline + 3);
}

#[test]
#[ignore = "requires Native Adapter DLLs built before the target Runtime contract"]
fn trh_003_keeps_a_compatible_adapter_active_when_a_peer_is_unavailable() {
    let gdi_package = native_package();
    let gdi_hash = artifact_hash(&gdi_package);
    let qt_package = qt_painter_native_package();
    let qt_hash = artifact_hash(&qt_package);
    let baseline = render_raw_gdi_unicode("Open").expect("baseline render");
    let deployment = TargetRuntimeDeployment::new(
        scoped_publication(
            1,
            "Translated by compatible adapter",
            glyphshift_adapter_gdi::ADAPTER_ID,
        ),
        [
            NativeAdapterDeployment::new(gdi_package, binding(gdi_hash, [Feature::TextReplace]))
                .expect("compatible GDI deployment"),
            NativeAdapterDeployment::new(
                qt_package,
                qt_painter_binding(qt_hash, [Feature::TextReplace]),
            )
            .expect("unavailable Qt deployment"),
        ],
    );

    activate_deployment(deployment)
        .expect("one unavailable candidate must not disable a compatible adapter");
    assert_eq!(
        query_activation()
            .expect("activation report")
            .active_adapter_ids()
            .collect::<Vec<_>>(),
        vec![glyphshift_adapter_gdi::ADAPTER_ID]
    );
    let translated = render_raw_gdi_unicode("Open").expect("translated render");
    assert_ne!(translated.signature(), baseline.signature());

    deactivate_runtime().expect("deactivate compatible adapter");
    let restored = render_raw_gdi_unicode("Open").expect("restored render");
    assert_eq!(restored.signature(), baseline.signature());
}

#[test]
#[ignore = "requires Native Adapter DLLs built before the target Runtime contract"]
fn trh_001_runs_a_real_native_adapter_from_publication_through_update_and_stop() {
    let redraw_window = RedrawContractWindow::new();
    let baseline = render_raw_gdi_unicode("Open").expect("baseline render");
    let gdiplus_baseline = render_gdiplus(pass_decision()).expect("baseline GDI+ render");
    let ae_padded_baseline = render_gdiplus_text("Project       .", pass_decision())
        .expect("baseline AE-padded GDI+ render");
    let native_package = native_package();
    let native_hash = artifact_hash(&native_package);
    let gdiplus_package = gdiplus_native_package();
    let gdiplus_hash = artifact_hash(&gdiplus_package);
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../local-test/target-runtime-contract");
    std::fs::create_dir_all(&local_test).expect("local test directory");
    let changed_package = local_test.join("changed-adapter.dll");
    std::fs::copy(&native_package, &changed_package).expect("adapter test copy");
    std::fs::OpenOptions::new()
        .append(true)
        .open(&changed_package)
        .expect("changed adapter copy")
        .write_all(b"changed")
        .expect("change adapter copy");
    let changed_deployment = TargetRuntimeDeployment::new(
        publication(1, "Rejected"),
        [NativeAdapterDeployment::new(
            changed_package,
            binding(native_hash, [Feature::TextReplace]),
        )
        .expect("changed target-process binding")],
    );
    assert_eq!(
        activate_deployment(changed_deployment),
        Err(glyphshift_target_runtime::TargetRuntimeError::AdapterArtifactChanged)
    );

    let deployment = TargetRuntimeDeployment::new(
        scoped_publication(1, "Excluded label", "example.synthetic.other"),
        [
            NativeAdapterDeployment::new(
                native_package.clone(),
                binding(native_hash, [Feature::TextReplace]),
            )
            .expect("GDI target-process binding"),
            NativeAdapterDeployment::new(
                gdiplus_package.clone(),
                gdiplus_binding(gdiplus_hash, [Feature::TextReplace]),
            )
            .expect("GDI+ target-process binding"),
        ],
    );

    redraw_window.validate();
    activate_deployment(deployment).expect("target Runtime activation");
    control_diagnostics(RuntimeDiagnosticsControl::new(true))
        .expect("enable bounded runtime diagnostics");
    redraw_window.assert_redraw_requested("activation");
    let excluded = render_raw_gdi_unicode("Open").expect("adapter-scoped pass-through");
    assert_eq!(excluded.signature(), baseline.signature());
    let excluded_trace = query_diagnostics().expect("query excluded trace");
    assert_eq!(excluded_trace.records().len(), 1);
    assert_eq!(excluded_trace.records()[0].source_text(), "Open");
    assert_eq!(
        excluded_trace.records()[0].status(),
        RuntimeTraceStatus::NoMatch
    );
    assert_eq!(
        excluded_trace.records()[0].text(),
        RuntimeTextOutcome::Unmatched
    );
    assert!(query_diagnostics()
        .expect("trace query drains the bounded window")
        .records()
        .is_empty());

    redraw_window.validate();
    update_publication(scoped_publication(
        2,
        "First translated label",
        glyphshift_adapter_gdi::ADAPTER_ID,
    ))
    .expect("selected adapter publication update");
    redraw_window.assert_redraw_requested("publication update");
    let first = render_raw_gdi_unicode("Open").expect("first translated render");
    assert_ne!(first.signature(), baseline.signature());
    let translated_trace = query_diagnostics().expect("query translated trace");
    assert_eq!(translated_trace.records().len(), 1);
    assert_eq!(
        translated_trace.records()[0].status(),
        RuntimeTraceStatus::Matched
    );
    assert_eq!(
        translated_trace.records()[0].text(),
        RuntimeTextOutcome::Replaced
    );
    assert_eq!(translated_trace.records()[0].generation(), 2);
    render_raw_gdi_unicode("Open").expect("render for C ABI diagnostics query");
    let mut trace_json = vec![0_u8; MAX_RUNTIME_TRACE_BYTES];
    let mut query = RuntimeDiagnosticsQueryV1 {
        struct_size: std::mem::size_of::<RuntimeDiagnosticsQueryV1>() as u32,
        output: trace_json.as_mut_ptr(),
        output_capacity: trace_json.len() as u32,
        output_len: 0,
    };
    assert_eq!(
        unsafe { glyphshift_runtime_diagnostics_query_v1(&mut query) },
        STATUS_TARGET_RUNTIME_OK
    );
    let queried = RuntimeTraceBatch::decode_json(
        std::str::from_utf8(&trace_json[..query.output_len as usize])
            .expect("C ABI trace JSON is UTF-8"),
    )
    .expect("C ABI trace batch");
    assert_eq!(queried.records().len(), 1);
    assert_eq!(queried.records()[0].source_text(), "Open");
    assert_eq!(
        render_gdiplus(pass_decision())
            .expect("GDI+ must remain outside the GDI-only entry")
            .signature(),
        gdiplus_baseline.signature()
    );

    update_publication(scoped_publication(
        3,
        "项目翻译",
        glyphshift_adapter_gdiplus::ADAPTER_ID,
    ))
    .expect("runtime publication update");
    assert_eq!(
        render_raw_gdi_unicode("Open")
            .expect("GDI must remain outside the GDI+-only entry")
            .signature(),
        baseline.signature()
    );
    let gdiplus_translated =
        render_gdiplus(pass_decision()).expect("GDI+ Chinese translated render");
    assert_ne!(gdiplus_translated.signature(), gdiplus_baseline.signature());

    update_publication(scoped_source_publication(
        4,
        "Project",
        "项目",
        glyphshift_adapter_gdiplus::ADAPTER_ID,
    ))
    .expect("AE-padded runtime publication update");
    let ae_padded_translated = render_gdiplus_text("Project       .", pass_decision())
        .expect("AE-padded GDI+ translated render");
    assert_ne!(
        ae_padded_translated.signature(),
        ae_padded_baseline.signature(),
        "AE layout padding must not prevent the semantic label from matching"
    );

    redraw_window.validate();
    deactivate_runtime().expect("target Runtime pass-through");
    redraw_window.assert_redraw_requested("deactivation");
    let restored = render_raw_gdi_unicode("Open").expect("restored render");
    assert_eq!(restored.signature(), baseline.signature());
    assert_eq!(
        render_gdiplus(pass_decision())
            .expect("restored GDI+ render")
            .signature(),
        gdiplus_baseline.signature()
    );

    let changed_features = TargetRuntimeDeployment::new(
        publication(5, "Font-only publication"),
        [
            NativeAdapterDeployment::new(
                native_package,
                binding(native_hash, [Feature::FontSubstitute]),
            )
            .expect("same GDI adapter with a changed requested Feature set"),
            NativeAdapterDeployment::new(
                gdiplus_package,
                gdiplus_binding(gdiplus_hash, [Feature::FontSubstitute]),
            )
            .expect("same GDI+ adapter with a changed requested Feature set"),
        ],
    );
    activate_deployment(changed_features)
        .expect("an inactive Runtime must accept a new Feature set for the same adapter");
    deactivate_runtime().expect("changed Feature set pass-through");
}
