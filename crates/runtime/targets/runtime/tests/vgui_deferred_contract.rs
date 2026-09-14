#![cfg(all(windows, target_arch = "x86"))]
use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_adapter_sdk::AdapterDescriptor;
use glyphshift_capture::{CaptureProducerConfiguration, CaptureProducerId};
use glyphshift_domain::{Feature, Generation, RouteProgram};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_target_runtime::{
    activate_deployment, deactivate_runtime, query_observations, update_publication,
};
use glyphshift_target_runtime_contract::{NativeAdapterDeployment, TargetRuntimeDeployment};
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn adapter(profile: &Path, file: &str, descriptor: AdapterDescriptor) -> NativeAdapterDeployment {
    let library = profile.join(file);
    let binding = AdapterBinding {
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: ArtifactHash::sha256(
            Sha256::digest(std::fs::read(&library).unwrap()).into(),
        ),
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new(file),
        },
        descriptor,
        features: vec![Feature::TextObserve, Feature::TextReplace],
    };
    NativeAdapterDeployment::new(library, binding).unwrap()
}

fn publication(generation: u64, translation: &str) -> RuntimePublication {
    RuntimePublication::new(
        RouteProgram::direct("text"),
        TranslationSnapshot::empty(Generation::new(generation)).with_entry(
            "text",
            "Open",
            translation,
        ),
        FontPolicy::empty(),
    )
}

#[test]
#[ignore = "requires the independent VGUI surface and two differently laid out image fixtures"]
fn deferred_vgui_runs_replace_whole_text_and_replay_unknown_operations() {
    let root =
        PathBuf::from(std::env::var_os("GLYPHSHIFT_VGUI_FIXTURE_ROOT").expect("fixture root"));
    let profile = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    unsafe {
        let surface = Box::leak(Box::new(
            libloading::Library::new(root.join("vguimatsurface.dll")).unwrap(),
        ));
        let a = Box::leak(Box::new(
            libloading::Library::new(root.join("caption-a.dll")).unwrap(),
        ));
        let b = Box::leak(Box::new(
            libloading::Library::new(root.join("caption-b.dll")).unwrap(),
        ));
        let reset = *surface
            .get::<unsafe extern "C" fn()>(b"fixture_reset\0")
            .unwrap();
        let output = *surface
            .get::<unsafe extern "C" fn() -> *const u16>(b"fixture_output\0")
            .unwrap();
        let trace = *surface
            .get::<unsafe extern "C" fn() -> u64>(b"fixture_trace\0")
            .unwrap();
        let first = *a
            .get::<unsafe extern "C" fn(*const u16, i32)>(b"fixture_paint\0")
            .unwrap();
        let second = *b
            .get::<unsafe extern "C" fn(*const u16, i32)>(b"fixture_paint\0")
            .unwrap();
        let render = |call: unsafe extern "C" fn(*const u16, i32), mode| {
            reset();
            let input = "Open".encode_utf16().chain(Some(0)).collect::<Vec<_>>();
            call(input.as_ptr(), mode);
            let ptr = output();
            let len = (0..100).find(|i| *ptr.add(*i) == 0).unwrap();
            String::from_utf16(std::slice::from_raw_parts(ptr, len)).unwrap()
        };
        assert_eq!(render(first, 0), "Open");
        assert_eq!(render(second, 1), "Op|en");
        let expected = (0..14)
            .map(|mode| {
                render(first, mode);
                trace()
            })
            .collect::<Vec<_>>();
        let descriptor = glyphshift_adapter_native_host::LoadedNativeAdapter::inspect(
            &profile.join("glyphshift_adapter_vgui_runs_native.dll"),
        )
        .unwrap();
        let deployment = TargetRuntimeDeployment::new(
            publication(1, "打开"),
            [
                adapter(
                    &profile,
                    "glyphshift_adapter_vgui_runs_native.dll",
                    descriptor,
                ),
                adapter(
                    &profile,
                    "glyphshift_adapter_gdi_native.dll",
                    glyphshift_adapter_gdi::descriptor(),
                ),
            ],
        )
        .with_observation_producer(
            CaptureProducerConfiguration::new(
                CaptureProducerId::new("vgui-deferred-test").unwrap(),
                1,
            )
            .unwrap(),
        );
        activate_deployment(deployment).unwrap();
        assert!(
            glyphshift_target_runtime::query_activation()
                .unwrap()
                .active_adapter_ids()
                .any(|id| id == "windows.vgui.text-run"),
            "VGUI adapter must activate before checking rendered text"
        );
        assert_eq!(render(first, 0), "打开");
        assert_eq!(render(second, 0), "打开");
        let records = query_observations().unwrap();
        assert_eq!(records.records().len(), 2);
        assert!(records
            .records()
            .iter()
            .all(|record| record.source() == "Open"
                && record.adapter_id() == "windows.vgui.text-run"));
        assert_eq!(
            render(first, 4),
            "打开",
            "packed color must establish the same drawing state as RGBA"
        );
        assert_eq!(
            render(first, 5),
            "打开",
            "complete PrintText owns its draw boundary without an enclosing image"
        );
        assert_eq!(
            render(first, 6),
            "|Open",
            "opaque calls invalidate direct drawing state"
        );
        assert_eq!(
            render(first, 7),
            "打开",
            "inherited verified Paint keeps its drawing boundary"
        );
        assert_eq!(
            render(first, 8),
            "打开",
            "a public panel context bounds custom glyph drawing"
        );
        assert_eq!(
            render(first, 9),
            "Open",
            "a mismatched panel context must replay original drawing"
        );
        assert_eq!(
            render(first, 10),
            "打开",
            "nested public contexts keep independent ownership"
        );
        assert_eq!(
            render(first, 11),
            "Open",
            "excessive context nesting preserves original drawing"
        );
        assert_eq!(
            render(first, 8),
            "打开",
            "balanced overflow must not poison subsequent contexts"
        );
        for mode in [1, 2, 3] {
            assert_eq!(
                render(first, mode),
                if mode == 1 { "Op|en" } else { "Open" }
            );
            assert_eq!(
                trace(),
                expected[mode as usize],
                "positions, style and non-text ordering stay identical on fallback"
            );
        }
        update_publication(publication(2, "第二代")).unwrap();
        assert_eq!(
            render(first, 12),
            "Label第二代",
            "a complete draw before an empty deferred run must not invalidate it"
        );
        for mode in [0, 4, 5, 7, 8, 10, 13] {
            assert_eq!(render(second, mode), "第二代");
        }
        update_publication(publication(3, "This translation exceeds the proven width")).unwrap();
        for mode in [0, 4, 5, 7, 8, 10, 13] {
            assert_eq!(render(first, mode), "Open");
            assert_eq!(trace(), expected[mode as usize]);
        }
        deactivate_runtime().unwrap();
        for mode in [0, 4, 5, 7, 8, 10, 13] {
            assert_eq!(render(first, mode), "Open");
            assert_eq!(render(second, mode), "Open");
            assert_eq!(trace(), expected[mode as usize]);
        }
    }
}
