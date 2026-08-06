use super::*;

#[test]
fn rejects_a_manifest_that_self_authorizes_an_unknown_bundle_authority() {
    let root = tempdir().expect("runtime bundle root");
    fs::write(
        root.path().join("runtime-bundle.json"),
        r#"{
          "schema":"glyphshift.runtime-bundle/2",
          "authority":"example.untrusted",
          "controller":{"artifact":"windows","file":"controller.exe","sha256":"0000000000000000000000000000000000000000000000000000000000000000","protocol":[1,0]},
          "runtime":{"file":"runtime.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"},
          "adapters":[{"file":"adapter.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}]
        }"#,
    )
    .expect("bundle manifest");

    assert_eq!(
        RuntimeBundle::open(root.path()).err(),
        Some(DesktopRuntimeError::InvalidManifest)
    );
}

#[test]
fn rejects_parent_paths_before_loading_native_code() {
    let root = tempdir().expect("runtime bundle root");
    fs::write(
        root.path().join("runtime-bundle.json"),
        r#"{
          "schema":"glyphshift.runtime-bundle/2",
          "authority":"app.glyphshift.runtime.first-party",
          "controller":{"artifact":"windows","file":"../controller.exe","sha256":"0000000000000000000000000000000000000000000000000000000000000000","protocol":[1,0]},
          "runtime":{"file":"runtime.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"},
          "adapters":[{"file":"adapter.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}]
        }"#,
    )
    .expect("bundle manifest");

    assert_eq!(
        RuntimeBundle::open(root.path()).err(),
        Some(DesktopRuntimeError::InvalidArtifactPath)
    );
}

#[test]
fn rejects_runtime_bytes_that_do_not_match_the_manifest_before_loading_adapters() {
    let root = tempdir().expect("runtime bundle root");
    let controller_bytes = b"synthetic controller";
    fs::write(root.path().join("controller.exe"), controller_bytes).expect("controller artifact");
    fs::write(root.path().join("runtime.dll"), b"changed runtime").expect("runtime artifact");
    fs::write(root.path().join("adapter.dll"), b"unreached adapter").expect("adapter artifact");
    let controller_hash = Sha256::digest(controller_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    fs::write(
        root.path().join("runtime-bundle.json"),
        format!(
            r#"{{
              "schema":"glyphshift.runtime-bundle/2",
              "authority":"app.glyphshift.runtime.first-party",
              "controller":{{"artifact":"windows","file":"controller.exe","sha256":"{controller_hash}","protocol":[1,0]}},
              "runtime":{{"file":"runtime.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}},
              "adapters":[{{"file":"adapter.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}]
            }}"#
        ),
    )
    .expect("bundle manifest");

    assert_eq!(
        RuntimeBundle::open(root.path()).err(),
        Some(DesktopRuntimeError::ArtifactHashMismatch)
    );
}

#[test]
fn parses_only_full_sha256_values() {
    assert_eq!(
        parse_hash("not-a-hash"),
        Err(DesktopRuntimeError::InvalidArtifactHash)
    );
    assert_eq!(parse_hash(&"f".repeat(64)), Ok([0xff; 32]));
}

#[test]
fn adapter_documentation_accepts_only_absolute_https_urls() {
    assert_eq!(parse_documentation_url(None), Ok(None));
    assert_eq!(
        parse_documentation_url(Some("https://example.invalid/reference")),
        Ok(Some("https://example.invalid/reference".into()))
    );
    for invalid in [
        "",
        " https://example.invalid/reference",
        "http://example.invalid/reference",
        "https://user@example.invalid/reference",
        "reference/index.html",
    ] {
        assert_eq!(
            parse_documentation_url(Some(invalid)),
            Err(DesktopRuntimeError::InvalidManifest)
        );
    }
}
