//! Standalone SDK tools; no desktop, controller, or target Runtime dependency.
use glyphshift_adapter_native_host::{NativeAdapterMetadata, inspect_pe_architecture};
use glyphshift_plugin_package::{Manifest, Package, PackageError, build_package, sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

fn main() {
    if let Err(error) = run() {
        eprintln!("Adapter SDK tool: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let args = args.iter().map(String::as_str).collect::<Vec<_>>();
    let result = match args.as_slice() {
        ["metadata", dll, "--trusted-sha256", trusted] => metadata(Path::new(dll), trusted),
        ["pack", manifest, source, output] => (|| {
            let manifest: Manifest = serde_json::from_slice(&bounded(Path::new(manifest), 1024 * 1024)?)?;
            let bytes = build_package(&manifest, Path::new(source))?;
            let path = Path::new(output);
            let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            file.write_all(&bytes)?;
            file.as_file().sync_all()?;
            file.persist_noclobber(path).map_err(|_| PackageError::Conflict)?;
            println!("{}", serde_json::json!({"sha256":sha256(&bytes), "bytes":bytes.len()}));
            Ok(())
        })(),
        ["verify", package] => Package::open(package).map(|p| {
            println!("{}",serde_json::json!({"sha256":p.sha256(), "manifest":p.manifest()}));
        }),
        _ => return Err("usage: glyphshift-adapter-tool metadata <built-dll> --trusted-sha256 <approved-digest> | pack <manifest.json> <source-directory> <output.gsp> | verify <package.gsp>".into()),
    };
    result.map_err(|e| format!("{e:?}"))
}

fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>, PackageError> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(PackageError::SizeLimit);
    }
    Ok(bytes)
}

fn metadata(path: &Path, trusted: &str) -> Result<(), PackageError> {
    let bytes = bounded(path, 64 * 1024 * 1024)?;
    if sha256(&bytes) != trusted {
        return Err(PackageError::ApprovalRequired);
    }
    if inspect_pe_architecture(path)? != std::env::consts::ARCH {
        return Err(PackageError::Incompatible);
    }
    // Explicit digest approval is required: loading a native descriptor executes DllMain.
    let descriptor =
        unsafe { NativeAdapterMetadata::inspect(path) }.map_err(|_| PackageError::Incompatible)?;
    println!("{}", serde_json::to_string(&descriptor)?);
    Ok(())
}
