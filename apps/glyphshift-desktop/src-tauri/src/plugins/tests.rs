use super::*;
use glyphshift_adapter_native_host::NativeAdapterMetadata;
use glyphshift_plugin_package::*;
use std::fs;
use tempfile::TempDir;

fn root() -> TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../local-test/evidence/plugin-management/contracts");
    fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

fn image(architecture: &str) -> Vec<u8> {
    let (machine, magic): (u16, u16) = if architecture == "x86" {
        (0x14c, 0x10b)
    } else {
        (0x8664, 0x20b)
    };
    let mut bytes = vec![0; 90];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
    bytes[64..68].copy_from_slice(b"PE\0\0");
    bytes[68..70].copy_from_slice(&machine.to_le_bytes());
    bytes[88..90].copy_from_slice(&magic.to_le_bytes());
    bytes
}

fn fixture(root: &Path, version: u16) -> Manifest {
    let mut files = Vec::new();
    let mut variants = Vec::new();
    for architecture in ["x86", "x86_64"] {
        let directory = format!("windows-{architecture}");
        fs::create_dir_all(root.join(&directory)).unwrap();
        let name = format!("{directory}/adapter.dll");
        let mut bytes = image(architecture);
        bytes.extend(version.to_le_bytes());
        fs::write(root.join(&name), &bytes).unwrap();
        files.push(PackageFile {
            path: name.clone(),
            sha256: sha256(&bytes),
            size: bytes.len() as u64,
            role: FileRole::NativeAdapter,
        });
        variants.push(Variant {
            platform: "windows".into(),
            architecture: architecture.into(),
            adapters: vec![Adapter {
                file: name,
                native_metadata: NativeAdapterMetadata {
                    adapter_id: "synthetic.inline".into(),
                    version: [1, 0, version],
                    abi: [1, 0],
                    apply_model: 1,
                    placement: 1,
                    feature_bits: 3,
                    platform_bits: 1,
                    architecture_bits: 3,
                    source_policy: 0,
                },
                name: "Synthetic inline".into(),
                summary: "Synthetic fixture, not executable code".into(),
                technology: "Synthetic".into(),
                process_resident_after_deactivate: true,
            }],
        });
    }
    fs::write(root.join("license.txt"), b"Synthetic fixture license").unwrap();
    files.push(PackageFile {
        path: "license.txt".into(),
        sha256: sha256(b"Synthetic fixture license"),
        size: 25,
        role: FileRole::License,
    });
    Manifest {
        schema: PACKAGE_SCHEMA.into(),
        package_id: "glyphshift-adapter-synthetic".into(),
        version: [1, 0, version],
        runtime_bundle_schema: "glyphshift.runtime-bundle/4".into(),
        license_file: "license.txt".into(),
        files,
        variants,
    }
}

fn setup() -> (TempDir, DesktopPlugins, PathBuf) {
    let root = root();
    let source = root.path().join("source");
    let manifest = fixture(&source, 0);
    let path = root.path().join("synthetic.gsp");
    fs::write(&path, build_package(&manifest, &source).unwrap()).unwrap();
    let manager = DesktopPlugins::new(
        root.path(),
        root.path().join("runtime"),
        BTreeSet::new(),
        true,
    );
    (root, manager, path)
}

#[test]
fn preview_is_static_and_install_uses_the_exact_approved_bytes() {
    let (_root, mut manager, path) = setup();
    let preview = manager.prepare(&path).unwrap();
    assert_eq!(preview.architectures.len(), 2);
    assert!(manager.store.list().unwrap().is_empty());
    // Replace the source after confirmation: only the cached, approved bytes install.
    fs::write(&path, b"changed after preview").unwrap();
    let snapshot = manager.install(&preview.sha256).unwrap();
    assert_eq!(snapshot.packages.len(), 1);
    assert_eq!(snapshot.packages[0].sha256, preview.sha256);
    assert!(!snapshot.packages[0].selected);
    assert!(!snapshot.packages[0].loaded);
    assert!(!snapshot.restart_required);
    assert_eq!(
        manager.install(&preview.sha256).err().unwrap().code(),
        "plugin.approval"
    );
}

#[test]
fn wrong_or_expired_approval_does_not_install() {
    let (_root, mut manager, path) = setup();
    manager.prepare(&path).unwrap();
    assert_eq!(
        manager.install(&"0".repeat(64)).err().unwrap().code(),
        "plugin.approval"
    );
    let preview = manager.prepare(&path).unwrap();
    manager.prepared.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(301);
    assert_eq!(
        manager.install(&preview.sha256).err().unwrap().code(),
        "plugin.approval"
    );
    assert!(manager.store.list().unwrap().is_empty());
}

#[test]
fn selection_is_separate_from_loaded_state_and_failed_preflight_preserves_it() {
    let (_root, mut manager, path) = setup();
    let preview = manager.prepare(&path).unwrap();
    manager.install(&preview.sha256).unwrap();
    // No runtime exists: enabling must fail without selecting the package.
    assert_eq!(
        manager.select(&preview.sha256, true).err().unwrap().code(),
        "plugin.incompatible"
    );
    assert!(!manager.snapshot().unwrap().packages[0].selected);
    // Simulate a store update by another tool. This session never marks it loaded.
    manager.store.select(&preview.sha256, |_| Ok(())).unwrap();
    let snapshot = manager.snapshot().unwrap();
    assert!(snapshot.restart_required);
    assert!(snapshot.packages[0].selected);
    assert!(!snapshot.packages[0].loaded);
    manager.loaded.insert(preview.sha256.clone());
    assert!(!manager.snapshot().unwrap().restart_required);
    manager
        .store
        .deselect_release(&preview.sha256, |_| Ok(()))
        .unwrap();
    let snapshot = manager.snapshot().unwrap();
    assert!(snapshot.restart_required);
    assert!(!snapshot.packages[0].selected);
    assert!(snapshot.packages[0].loaded);
}

#[test]
fn corrupt_installed_release_remains_visible_with_an_error() {
    let (_root, mut manager, path) = setup();
    let preview = manager.prepare(&path).unwrap();
    manager.install(&preview.sha256).unwrap();
    let installed = manager.store.get(&preview.sha256).unwrap();
    fs::write(
        installed.file("windows-x86/adapter.dll").unwrap(),
        b"corrupt",
    )
    .unwrap();
    let snapshot = manager.snapshot().unwrap();
    assert_eq!(snapshot.packages.len(), 1);
    assert!(snapshot.packages[0].problem.is_some());
    assert_eq!(snapshot.packages[0].sha256, preview.sha256);
}
