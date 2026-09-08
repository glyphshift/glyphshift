#![cfg(all(windows, target_arch = "x86"))]
use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_adapter_sdk::AdapterDescriptor;

use glyphshift_domain::{Feature, Generation, RouteProgram};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_target_runtime::{activate_deployment, deactivate_runtime};
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
#[ignore = "requires independent synthetic VGUI ABI fixtures"]
fn a_wrong_method_contract_is_rejected_before_native_calls_or_table_changes() {
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
        let _image = Box::leak(Box::new(
            libloading::Library::new(root.join("caption-a.dll")).unwrap(),
        ));
        let native = Box::leak(Box::new(
            libloading::Library::new(profile.join("glyphshift_adapter_vgui_runs_native.dll"))
                .unwrap(),
        ));
        let factory = *surface
            .get::<unsafe extern "C" fn(*const u8, *mut i32) -> *mut usize>(b"CreateInterface\0")
            .unwrap();
        let object = factory(b"VGUI_Surface031\0".as_ptr(), std::ptr::null_mut());
        surface
            .get::<unsafe extern "C" fn()>(b"fixture_wrong_abi\0")
            .unwrap()();
        let original = *object;
        let descriptor = glyphshift_adapter_native_host::LoadedNativeAdapter::inspect(
            &profile.join("glyphshift_adapter_vgui_runs_native.dll"),
        )
        .unwrap();
        let deployment = TargetRuntimeDeployment::new(
            publication(1, "打开"),
            [adapter(
                &profile,
                "glyphshift_adapter_vgui_runs_native.dll",
                descriptor,
            )],
        );
        assert_eq!(
            activate_deployment(deployment.clone()),
            Err(glyphshift_target_runtime::TargetRuntimeError::AdapterActivation)
        );
        assert_eq!(
            *object, original,
            "rejection cannot publish a partially validated vtable"
        );
        let stage = *native
            .get::<*const std::sync::atomic::AtomicI32>(b"glyphshift_vgui_activation_stage_v1\0")
            .unwrap();
        assert_eq!((*stage).load(std::sync::atomic::Ordering::Acquire), -1071);
        surface
            .get::<unsafe extern "C" fn()>(b"fixture_restore_abi\0")
            .unwrap()();
        activate_deployment(deployment).unwrap();
        deactivate_runtime().unwrap();
    }
}
