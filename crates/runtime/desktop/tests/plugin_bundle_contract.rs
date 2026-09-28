#![cfg(windows)]
use glyphshift_adapter_native_host::NativeAdapterMetadata;
use glyphshift_desktop_runtime::RuntimeBundle;
use glyphshift_plugin_package::{
    Adapter, FileRole, Manifest, PACKAGE_SCHEMA, Package, PackageError, PackageFile, PluginStore,
    Variant, build_package, sha256,
};
use std::fs;
use std::path::{Path, PathBuf};

fn synthetic_core() -> Vec<u8> {
    let mut bytes = vec![0; 90];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
    bytes[64..68].copy_from_slice(b"PE\0\0");
    bytes[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes[88..90].copy_from_slice(&0x20bu16.to_le_bytes());
    bytes
}

fn raylib_artifact() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("glyphshift_adapter_raylib_native.dll")
}

fn plugin(source: &Path, version: u16, metadata: &NativeAdapterMetadata) -> Package {
    fs::create_dir_all(source).unwrap();
    let bytes = fs::read(raylib_artifact())
        .expect("build the repository Raylib native package before integration tests");
    fs::write(source.join("adapter.dll"), &bytes).unwrap();
    fs::write(source.join("license.txt"), b"Synthetic package fixture").unwrap();
    let manifest = Manifest {
        schema: PACKAGE_SCHEMA.into(),
        package_id: "glyphshift-adapter-raylib".into(),
        version: [1, 0, version],
        runtime_bundle_schema: "glyphshift.runtime-bundle/4".into(),
        license_file: "license.txt".into(),
        files: vec![
            PackageFile {
                path: "adapter.dll".into(),
                sha256: sha256(&bytes),
                size: bytes.len() as u64,
                role: FileRole::NativeAdapter,
            },
            PackageFile {
                path: "license.txt".into(),
                sha256: sha256(b"Synthetic package fixture"),
                size: 25,
                role: FileRole::License,
            },
        ],
        variants: vec![Variant {
            platform: "windows".into(),
            architecture: "x86_64".into(),
            adapters: vec![Adapter {
                file: "adapter.dll".into(),
                native_metadata: metadata.clone(),
                name: "External Raylib".into(),
                summary: "Synthetic installation fixture".into(),
                technology: "raylib".into(),
                process_resident_after_deactivate: true,
            }],
        }],
    };
    Package::from_bytes(&build_package(&manifest, source).unwrap()).unwrap()
}

#[test]
fn selected_plugin_replaces_builtin_and_real_descriptor_rejection_preserves_selection() {
    let evidence = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../local-test/evidence/plugin-package/runtime-contract");
    fs::create_dir_all(&evidence).unwrap();
    let root = tempfile::tempdir_in(evidence).unwrap();
    let bundle = root.path().join("bundle");
    fs::create_dir_all(&bundle).unwrap();
    let bytes = fs::read(raylib_artifact()).unwrap();
    // This DLL is the repository-built synthetic contract input, not a downloaded package.
    let metadata = unsafe { NativeAdapterMetadata::inspect(&raylib_artifact()) }.unwrap();
    let core = synthetic_core();
    fs::write(bundle.join("controller.exe"), &core).unwrap();
    fs::write(bundle.join("runtime.dll"), &core).unwrap();
    fs::write(bundle.join("builtin.dll"), &bytes).unwrap();
    let manifest = serde_json::json!({
        "schema": "glyphshift.runtime-bundle/4", "architecture": "x86_64", "authority": "app.glyphshift.runtime.first-party",
        "controller": {"artifact":"synthetic-controller", "file":"controller.exe", "sha256":sha256(&core), "protocol":[1,0]},
        "runtime": {"file":"runtime.dll", "sha256":sha256(&core)},
        "adapters": [{"file":"builtin.dll", "sha256":sha256(&bytes), "name":"Builtin Raylib", "native_metadata":metadata}],
        "acquisition_workers":[], "additional_architectures":[]
    });
    fs::write(
        bundle.join("runtime-bundle.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let store_root = root.path().join("plugins");
    let builtin = RuntimeBundle::open_with_plugin_store(&bundle, &store_root).unwrap();
    assert_eq!(builtin.adapter_options().len(), 1);
    assert_eq!(builtin.adapter_options()[0].name(), "Builtin Raylib");
    assert!(
        !store_root.exists(),
        "normal startup should not create an empty plugin store"
    );
    let store = PluginStore::new(&store_root);
    let first = plugin(&root.path().join("first"), 0, &metadata);
    store.install(&first, first.sha256()).unwrap();
    let preflight = |packages: &[_]| {
        RuntimeBundle::open_with_packages(&bundle, packages)
            .map(|_| ())
            .map_err(|_| PackageError::Incompatible)
    };
    store.select(first.sha256(), preflight).unwrap();
    let selected = RuntimeBundle::open_with_plugin_store(&bundle, &store_root).unwrap();
    assert_eq!(
        selected.adapter_options().len(),
        1,
        "built-in and plugin must not both appear"
    );
    assert_eq!(selected.adapter_options()[0].name(), "External Raylib");
    assert_eq!(fs::read(bundle.join("runtime.dll")).unwrap(), core);
    let mut wrong = metadata.clone();
    wrong.version[2] += 1;
    let second = plugin(&root.path().join("second"), 1, &wrong);
    store.install(&second, second.sha256()).unwrap();
    assert_eq!(
        store.select(second.sha256(), preflight),
        Err(PackageError::Incompatible)
    );
    assert_eq!(store.selected().unwrap()[0].sha256(), first.sha256());
    store
        .deselect(&first.manifest().package_id, preflight)
        .unwrap();
    assert_eq!(
        RuntimeBundle::open_with_plugin_store(&bundle, &store_root)
            .unwrap()
            .adapter_options()[0]
            .name(),
        "Builtin Raylib"
    );
    assert_eq!(
        selected.adapter_options()[0].name(),
        "External Raylib",
        "existing snapshots are not rewritten"
    );
}
