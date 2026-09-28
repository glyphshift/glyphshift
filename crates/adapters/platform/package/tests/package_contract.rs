use glyphshift_adapter_native_host::NativeAdapterMetadata;
use glyphshift_plugin_package::*;
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use tempfile::TempDir;
use zip::{ZipWriter, write::SimpleFileOptions};

fn root() -> TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../local-test/evidence/plugin-package/contracts");
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

fn package(version: u16) -> Package {
    let source = root();
    let manifest = fixture(source.path(), version);
    Package::from_bytes(&build_package(&manifest, source.path()).unwrap()).unwrap()
}

fn raw_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn deterministic_package_preserves_two_architectures_without_shared_runtime() {
    let source = root();
    let manifest = fixture(source.path(), 0);
    let first = build_package(&manifest, source.path()).unwrap();
    assert_eq!(first, build_package(&manifest, source.path()).unwrap());
    let parsed = Package::from_bytes(&first).unwrap();
    assert_eq!(parsed.manifest().variants.len(), 2);
    assert_eq!(parsed.manifest().files.len(), 3);
    assert_eq!(parsed.sha256(), sha256(&first));
}

#[test]
fn install_requires_exact_external_approval_and_does_not_select() {
    let target = root();
    let store = PluginStore::new(target.path());
    let package = package(0);
    assert_eq!(
        store.install(&package, &"0".repeat(64)).unwrap_err(),
        PackageError::ApprovalRequired
    );
    assert!(store.list().unwrap().is_empty());
    store.install(&package, package.sha256()).unwrap();
    assert!(store.selected().unwrap().is_empty());
    assert_eq!(store.list().unwrap().len(), 1);
}

#[test]
fn failed_update_keeps_old_selection_and_rollback_reuses_immutable_files() {
    let target = root();
    let store = PluginStore::new(target.path());
    let first = package(0);
    let second = package(1);
    store.install(&first, first.sha256()).unwrap();
    store.select(first.sha256(), |_| Ok(())).unwrap();
    let original = store.selected().unwrap().remove(0);
    let old_path = original.file("windows-x86/adapter.dll").unwrap().to_owned();
    store.install(&second, second.sha256()).unwrap();
    assert_eq!(
        store.select(second.sha256(), |_| Err(PackageError::Incompatible)),
        Err(PackageError::Incompatible)
    );
    assert_eq!(store.selected().unwrap()[0].sha256(), first.sha256());
    store.select(second.sha256(), |_| Ok(())).unwrap();
    assert_ne!(
        store.selected().unwrap()[0]
            .file("windows-x86/adapter.dll")
            .unwrap(),
        old_path
    );
    original.verify().unwrap();
    store.select(first.sha256(), |_| Ok(())).unwrap();
    assert_eq!(
        store.selected().unwrap()[0]
            .file("windows-x86/adapter.dll")
            .unwrap(),
        old_path
    );
    store
        .deselect(&first.manifest().package_id, |_| Ok(()))
        .unwrap();
    assert!(store.selected().unwrap().is_empty());
    assert!(old_path.exists());
}

#[test]
fn two_packages_cannot_own_the_same_adapter_even_on_different_architectures() {
    let target = root();
    let store = PluginStore::new(target.path());
    let first = package(0);
    let source = root();
    let mut manifest = fixture(source.path(), 0);
    manifest.package_id = "glyphshift-adapter-other".into();
    let second = Package::from_bytes(&build_package(&manifest, source.path()).unwrap()).unwrap();
    store.install(&first, first.sha256()).unwrap();
    store.install(&second, second.sha256()).unwrap();
    store.select(first.sha256(), |_| Ok(())).unwrap();
    assert_eq!(
        store.select(second.sha256(), |_| panic!(
            "conflict must reject before code inspection"
        )),
        Err(PackageError::Conflict)
    );
    assert_eq!(store.selected().unwrap().len(), 1);
}

#[test]
fn a_published_version_cannot_be_replaced_with_different_bytes() {
    let target = root();
    let store = PluginStore::new(target.path());
    let first = package(0);
    store.install(&first, first.sha256()).unwrap();
    let source = root();
    let mut manifest = fixture(source.path(), 0);
    manifest.variants[0].adapters[0].summary = "Changed publication".into();
    let changed = Package::from_bytes(&build_package(&manifest, source.path()).unwrap()).unwrap();
    assert_eq!(
        store.install(&changed, changed.sha256()).unwrap_err(),
        PackageError::Conflict
    );
    assert_eq!(store.list().unwrap().len(), 1);
}

#[test]
fn mutation_after_install_is_rejected_before_preflight() {
    let target = root();
    let store = PluginStore::new(target.path());
    let package = package(0);
    store.install(&package, package.sha256()).unwrap();
    store.select(package.sha256(), |_| Ok(())).unwrap();
    let installed = store.selected().unwrap().remove(0);
    fs::write(
        installed.file("windows-x86/adapter.dll").unwrap(),
        b"modified",
    )
    .unwrap();
    assert_eq!(store.selected().err(), Some(PackageError::HashMismatch));
    assert_eq!(
        store.select(package.sha256(), |_| panic!(
            "changed bytes must not execute"
        )),
        Err(PackageError::HashMismatch)
    );
}

#[test]
fn undeclared_neighbor_libraries_and_changed_manifest_are_rejected() {
    let target = root();
    let store = PluginStore::new(target.path());
    let package = package(0);
    store.install(&package, package.sha256()).unwrap();
    store.select(package.sha256(), |_| Ok(())).unwrap();
    let installed = store.selected().unwrap().remove(0);
    let parent = installed
        .file("windows-x86/adapter.dll")
        .unwrap()
        .parent()
        .unwrap();
    fs::write(parent.join("extra.dll"), image("x86")).unwrap();
    assert_eq!(installed.verify(), Err(PackageError::InvalidArchive));
    fs::remove_file(parent.join("extra.dll")).unwrap();
    let manifest = parent.parent().unwrap().join("manifest.json");
    fs::write(manifest, b"{}").unwrap();
    assert_eq!(store.selected().err(), Some(PackageError::HashMismatch));
}

#[test]
fn rejects_archive_traversal_windows_aliases_and_case_aliases() {
    for name in [
        "../escape.dll",
        "a/../../escape.dll",
        "c:/escape.dll",
        "a\\escape.dll",
        "con.dll",
        "a/aux.txt",
        "lpt1.dll",
        "a/file.",
        "UPPER.dll",
    ] {
        assert_eq!(
            Package::from_bytes(&raw_zip(&[(name, b"payload")])).err(),
            Some(PackageError::UnsafePath),
            "{name}"
        );
    }
}

#[test]
fn refuses_links_unknown_files_and_oversized_manifests() {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .add_symlink("linked.dll", "../outside", SimpleFileOptions::default())
        .unwrap();
    assert_eq!(
        Package::from_bytes(&writer.finish().unwrap().into_inner()).err(),
        Some(PackageError::UnsafePath)
    );
    assert_eq!(
        Package::from_bytes(&raw_zip(&[("manifest.json", &vec![b' '; 1024 * 1024 + 1])])).err(),
        Some(PackageError::SizeLimit)
    );
    let source = root();
    let manifest = fixture(source.path(), 0);
    fs::write(source.path().join("unlisted.dll"), b"extra").unwrap();
    assert_eq!(
        build_package(&manifest, source.path()).err(),
        Some(PackageError::InvalidArchive)
    );
}

#[test]
fn rejects_wrong_abi_architecture_runtime_contract_and_declared_hash() {
    let source = root();
    let manifest = fixture(source.path(), 0);
    let mut invalid = manifest.clone();
    invalid.variants[0].adapters[0].native_metadata.abi = [9, 0];
    assert_eq!(invalid.validate(), Err(PackageError::Incompatible));
    invalid = manifest.clone();
    invalid.runtime_bundle_schema = "glyphshift.runtime-bundle/99".into();
    assert_eq!(invalid.validate(), Err(PackageError::Incompatible));
    invalid = manifest.clone();
    invalid.files[0].sha256 = "0".repeat(64);
    assert_eq!(
        build_package(&invalid, source.path()).err(),
        Some(PackageError::HashMismatch)
    );
    let wrong = image("x86_64");
    fs::write(source.path().join(&manifest.files[0].path), &wrong).unwrap();
    invalid = manifest;
    invalid.files[0].sha256 = sha256(&wrong);
    invalid.files[0].size = wrong.len() as u64;
    assert_eq!(
        build_package(&invalid, source.path()).err(),
        Some(PackageError::Incompatible)
    );
}

#[test]
fn writer_lock_prevents_lost_selections_and_corrupt_state_is_not_reset() {
    let target = root();
    let store = PluginStore::new(target.path());
    let package = package(0);
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(target.path().join("store.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert_eq!(
        store.install(&package, package.sha256()).unwrap_err(),
        PackageError::Busy
    );
    drop(lock);
    fs::write(target.path().join("state.json"), b"broken").unwrap();
    assert_eq!(
        store.install(&package, package.sha256()).unwrap_err(),
        PackageError::InvalidManifest
    );
    assert_eq!(
        fs::read(target.path().join("state.json")).unwrap(),
        b"broken"
    );
}

#[test]
fn one_package_can_offer_multiple_adapter_ids_and_support_files() {
    let source = root();
    let mut manifest = fixture(source.path(), 0);
    for variant in &mut manifest.variants {
        let mut second = variant.adapters[0].clone();
        second.native_metadata.adapter_id = "synthetic.second".into();
        second.file = format!("windows-{}/second.dll", variant.architecture);
        let bytes = image(&variant.architecture);
        fs::write(source.path().join(&second.file), &bytes).unwrap();
        manifest.files.push(PackageFile {
            path: second.file.clone(),
            sha256: sha256(&bytes),
            size: bytes.len() as u64,
            role: FileRole::NativeAdapter,
        });
        variant.adapters.push(second);
    }
    fs::write(source.path().join("support.txt"), b"support").unwrap();
    manifest.files.push(PackageFile {
        path: "support.txt".into(),
        sha256: sha256(b"support"),
        size: 7,
        role: FileRole::Support,
    });
    let parsed = Package::from_bytes(&build_package(&manifest, source.path()).unwrap()).unwrap();
    assert_eq!(parsed.manifest().variants[0].adapters.len(), 2);
}
