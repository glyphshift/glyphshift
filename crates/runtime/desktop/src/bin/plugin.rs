//! Local developer entry point. No network downloads or automatic publisher approval.
use glyphshift_desktop_runtime::RuntimeBundle;
use glyphshift_plugin_package::{
    Adapter, FileRole, Manifest, PACKAGE_SCHEMA, Package, PackageError, PackageFile, PluginStore,
    Variant, build_package, sha256,
};
use std::fs;
use std::path::Path;

fn main() {
    if let Err(error) = run() {
        eprintln!("Plugin operation failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let strings = args.iter().map(String::as_str).collect::<Vec<_>>();
    let result = match strings.as_slice() {
        ["inspect", path] => Package::open(path).map(|package| {
            println!("{}", serde_json::json!({"sha256": package.sha256(), "manifest": package.manifest(), "trust": "not-approved"}));
        }),
        ["pack", manifest, source, output] => (|| {
            let manifest: Manifest = serde_json::from_slice(&fs::read(manifest)?)?;
            write_new(output, &build_package(&manifest, Path::new(source))?)
        })(),
        ["export-bundle", bundle, adapter, package_id, version, license, output] => export_bundle(bundle, adapter, package_id, version, license, output),
        ["install", package, store, "--approve-sha256", approved] => (|| {
            let package = Package::open(package)?;
            let selection = PluginStore::new(*store).install(&package, approved)?;
            println!("{}", serde_json::to_string(&selection)?);
            Ok(())
        })(),
        ["list", store] => (|| {
            println!("{}", serde_json::to_string_pretty(&PluginStore::new(*store).list()?)?);
            Ok(())
        })(),
        ["select", store, digest, bundle] => PluginStore::new(*store).select(digest, |packages| {
            RuntimeBundle::open_with_packages(bundle, packages).map(|_| ()).map_err(|error| {
                eprintln!("Runtime preflight rejected selection: {error:?}");
                PackageError::Incompatible
            })
        }),
        ["deselect", store, package_id, bundle] => PluginStore::new(*store).deselect(package_id, |packages| {
            RuntimeBundle::open_with_packages(bundle, packages).map(|_| ()).map_err(|_| PackageError::Incompatible)
        }),
        ["verify", store, bundle] => RuntimeBundle::open_with_plugin_store(bundle, *store).map(|bundle| {
            println!("Runtime verified with {} adapter options", bundle.adapter_options().len());
        }).map_err(|_| PackageError::Incompatible),
        _ => return Err("usage:\n  glyphshift-plugin inspect <package.gsp>\n  glyphshift-plugin pack <manifest.json> <source-directory> <output.gsp>\n  glyphshift-plugin export-bundle <bundle-root> <adapter-id> <package-id> <major.minor.patch> <license-file> <output.gsp>\n  glyphshift-plugin install <package.gsp> <store> --approve-sha256 <trusted-sha256>\n  glyphshift-plugin list <store>\n  glyphshift-plugin select <store> <installed-sha256> <bundle-root>\n  glyphshift-plugin deselect <store> <package-id> <bundle-root>\n  glyphshift-plugin verify <store> <bundle-root>\nSelection changes apply on the next App start; loaded targets keep their current files. Local digest approval is not publisher signature verification.".into()),
    };
    result.map_err(|e| format!("{e:?}"))
}

fn write_new(path: &str, bytes: &[u8]) -> Result<(), PackageError> {
    use std::io::Write;
    let path = Path::new(path);
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(path)
        .map_err(|_| PackageError::Conflict)?;
    println!("sha256={}", sha256(bytes));
    Ok(())
}

/// Repackages existing, hash-checked build outputs. Does not load a DLL or copy
/// Controller/Runtime. Each exported variant still uses the original Adapter ID.
fn export_bundle(
    bundle: &str,
    adapter_id: &str,
    package_id: &str,
    version: &str,
    license: &str,
    output: &str,
) -> Result<(), PackageError> {
    let version: Vec<u16> = version
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| PackageError::InvalidManifest)?;
    let version: [u16; 3] = version
        .try_into()
        .map_err(|_| PackageError::InvalidManifest)?;
    let root = Path::new(bundle).canonicalize()?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("runtime-bundle.json"))?)?;
    if manifest["schema"] != "glyphshift.runtime-bundle/4" {
        return Err(PackageError::Incompatible);
    }
    let output_parent = Path::new(output)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stage = tempfile::tempdir_in(output_parent)?;
    let license_bytes = fs::read(license)?;
    fs::write(stage.path().join("license.txt"), &license_bytes)?;
    let mut files = vec![PackageFile {
        path: "license.txt".into(),
        sha256: sha256(&license_bytes),
        size: license_bytes.len() as u64,
        role: FileRole::License,
    }];
    let mut variants = Vec::new();
    let mut groups = vec![&manifest];
    if let Some(additional) = manifest["additional_architectures"].as_array() {
        groups.extend(additional);
    }
    for group in groups {
        let architecture = group["architecture"]
            .as_str()
            .ok_or(PackageError::InvalidManifest)?;
        if !matches!(architecture, "x86" | "x86_64") {
            return Err(PackageError::Incompatible);
        }
        let adapters = group["adapters"]
            .as_array()
            .ok_or(PackageError::InvalidManifest)?;
        let mut entries = Vec::new();
        for adapter in adapters
            .iter()
            .filter(|a| a["native_metadata"]["adapter_id"] == adapter_id)
        {
            let source_name = adapter["file"]
                .as_str()
                .ok_or(PackageError::InvalidManifest)?;
            if source_name.contains(['/', '\\', ':']) || !source_name.ends_with(".dll") {
                return Err(PackageError::UnsafePath);
            }
            let path = root.join(source_name).canonicalize()?;
            if path.parent() != Some(root.as_path()) {
                return Err(PackageError::UnsafePath);
            }
            let bytes = fs::read(path)?;
            let digest = sha256(&bytes);
            if adapter["sha256"].as_str() != Some(&digest) {
                return Err(PackageError::HashMismatch);
            }
            let name = format!("windows-{architecture}/adapter.dll");
            fs::create_dir_all(stage.path().join(format!("windows-{architecture}")))?;
            fs::write(stage.path().join(&name), &bytes)?;
            files.push(PackageFile {
                path: name.clone(),
                sha256: digest,
                size: bytes.len() as u64,
                role: FileRole::NativeAdapter,
            });
            entries.push(Adapter {
                file: name,
                native_metadata: serde_json::from_value(adapter["native_metadata"].clone())?,
                name: adapter["name"].as_str().unwrap_or(adapter_id).into(),
                summary: adapter["summary"].as_str().unwrap_or("").into(),
                technology: adapter["technology"].as_str().unwrap_or("").into(),
                process_resident_after_deactivate: adapter["process_resident_after_deactivate"]
                    .as_bool()
                    .unwrap_or(false),
            });
        }
        if !entries.is_empty() {
            variants.push(Variant {
                platform: "windows".into(),
                architecture: architecture.into(),
                adapters: entries,
            });
        }
    }
    let package = Manifest {
        schema: PACKAGE_SCHEMA.into(),
        package_id: package_id.into(),
        version,
        runtime_bundle_schema: "glyphshift.runtime-bundle/4".into(),
        license_file: "license.txt".into(),
        variants,
        files,
    };
    write_new(output, &build_package(&package, stage.path())?)
}
