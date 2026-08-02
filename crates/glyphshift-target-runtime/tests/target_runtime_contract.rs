#![cfg(windows)]

use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_domain::{
    Feature, FontDecision, Generation, RenderDecision, RouteProgram, TextDecision,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_target_runtime::{activate_deployment, deactivate_runtime, update_publication};
use glyphshift_target_runtime_contract::{NativeAdapterDeployment, TargetRuntimeDeployment};
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
        .join("../../target/local-test/target-runtime-contract");
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
    redraw_window.assert_redraw_requested("activation");
    let excluded = render_raw_gdi_unicode("Open").expect("adapter-scoped pass-through");
    assert_eq!(excluded.signature(), baseline.signature());

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
