use super::*;

#[test]
fn rejects_a_manifest_that_self_authorizes_an_unknown_bundle_authority() {
    let root = tempdir().expect("runtime bundle root");
    fs::write(
        root.path().join("runtime-bundle.json"),
        r#"{
          "schema":"glyphshift.runtime-bundle/3",
          "authority":"example.untrusted",
          "controller":{"artifact":"windows","file":"controller.exe","sha256":"0000000000000000000000000000000000000000000000000000000000000000","protocol":[1,0]},
          "runtime":{"file":"runtime.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"},
          "adapters":[{"file":"adapter.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}],
          "acquisition_workers":[]
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
          "schema":"glyphshift.runtime-bundle/3",
          "authority":"app.glyphshift.runtime.first-party",
          "controller":{"artifact":"windows","file":"../controller.exe","sha256":"0000000000000000000000000000000000000000000000000000000000000000","protocol":[1,0]},
          "runtime":{"file":"runtime.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"},
          "adapters":[{"file":"adapter.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}],
          "acquisition_workers":[]
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
              "schema":"glyphshift.runtime-bundle/3",
              "authority":"app.glyphshift.runtime.first-party",
              "controller":{{"artifact":"windows","file":"controller.exe","sha256":"{controller_hash}","protocol":[1,0]}},
              "runtime":{{"file":"runtime.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}},
              "adapters":[{{"file":"adapter.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}],
              "acquisition_workers":[]
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

fn sha256(bytes: &[u8]) -> Box<str> {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
        .into()
}

fn acquisition_manifest(file: &str, hash: Box<str>, adapter_id: &str) -> AcquisitionWorkerManifest {
    AcquisitionWorkerManifest {
        file: file.into(),
        sha256: hash,
        adapter_id: adapter_id.into(),
        support_files: Vec::new(),
    }
}

#[test]
fn acquisition_worker_catalog_verifies_artifacts_and_hides_paths_behind_a_host_factory() {
    let root = tempdir().expect("bundle root");
    let bytes = b"synthetic acquisition worker";
    fs::write(root.path().join("acquisition-worker.exe"), bytes).expect("worker artifact");
    let catalog = AcquisitionWorkerCatalog::load(
        root.path(),
        &[acquisition_manifest(
            "acquisition-worker.exe",
            sha256(bytes),
            "windows.uia.acquire",
        )],
    )
    .expect("verified acquisition worker catalog");

    assert_eq!(
        catalog.adapter_ids(),
        [Box::<str>::from("windows.uia.acquire")]
    );
    catalog
        .host("windows.uia.acquire")
        .expect("verified worker host");
    assert_eq!(
        catalog.host("unknown.acquire").err(),
        Some(DesktopRuntimeError::AcquisitionWorkerUnavailable)
    );
}

#[test]
fn acquisition_worker_catalog_rejects_duplicate_missing_tampered_and_parent_artifacts() {
    let root = tempdir().expect("bundle root");
    let bytes = b"synthetic acquisition worker";
    fs::write(root.path().join("worker.exe"), bytes).expect("worker artifact");
    let valid = || acquisition_manifest("worker.exe", sha256(bytes), "windows.uia.acquire");

    assert_eq!(
        AcquisitionWorkerCatalog::load(root.path(), &[valid(), valid()]).err(),
        Some(DesktopRuntimeError::InvalidManifest)
    );
    assert_eq!(
        AcquisitionWorkerCatalog::load(
            root.path(),
            &[acquisition_manifest(
                "missing.exe",
                sha256(bytes),
                "windows.uia.acquire",
            )],
        )
        .err(),
        Some(DesktopRuntimeError::BundleUnavailable)
    );
    assert_eq!(
        AcquisitionWorkerCatalog::load(
            root.path(),
            &[acquisition_manifest(
                "worker.exe",
                "0".repeat(64).into(),
                "windows.uia.acquire",
            )],
        )
        .err(),
        Some(DesktopRuntimeError::ArtifactHashMismatch)
    );
    assert_eq!(
        AcquisitionWorkerCatalog::load(
            root.path(),
            &[acquisition_manifest(
                "../worker.exe",
                sha256(bytes),
                "windows.uia.acquire",
            )],
        )
        .err(),
        Some(DesktopRuntimeError::InvalidManifest)
    );
}

#[test]
fn acquisition_worker_catalog_verifies_every_support_file() {
    let root = tempdir().expect("bundle root");
    let worker_bytes = b"synthetic acquisition worker";
    let model_bytes = b"synthetic language model";
    fs::write(root.path().join("worker.exe"), worker_bytes).expect("worker artifact");
    fs::write(root.path().join("model.bin"), model_bytes).expect("support artifact");
    let support = AcquisitionSupportFileManifest {
        file: "model.bin".into(),
        sha256: sha256(model_bytes),
    };
    let manifest = || AcquisitionWorkerManifest {
        file: "worker.exe".into(),
        sha256: sha256(worker_bytes),
        adapter_id: "windows.ocr.acquire".into(),
        support_files: vec![support.clone()],
    };

    AcquisitionWorkerCatalog::load(root.path(), &[manifest()]).expect("support artifact verified");

    fs::write(root.path().join("model.bin"), b"tampered model").expect("tamper support");
    assert_eq!(
        AcquisitionWorkerCatalog::load(root.path(), &[manifest()]).err(),
        Some(DesktopRuntimeError::ArtifactHashMismatch)
    );

    let mut duplicate = manifest();
    duplicate
        .support_files
        .push(AcquisitionSupportFileManifest {
            file: "MODEL.bin".into(),
            ..support
        });
    assert_eq!(
        AcquisitionWorkerCatalog::load(root.path(), &[duplicate]).err(),
        Some(DesktopRuntimeError::InvalidManifest)
    );
}

#[test]
fn runtime_bundle_three_requires_acquisition_workers() {
    let result = serde_json::from_str::<BundleManifest>(
        r#"{
          "schema":"glyphshift.runtime-bundle/3",
          "authority":"app.glyphshift.runtime.first-party",
          "controller":{"artifact":"windows","file":"controller.exe","sha256":"0000000000000000000000000000000000000000000000000000000000000000","protocol":[1,0]},
          "runtime":{"file":"runtime.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"},
          "adapters":[]
        }"#,
    );

    assert!(result.is_err());
}

#[test]
fn runtime_bundle_three_requires_explicit_acquisition_support_files() {
    let result = serde_json::from_str::<BundleManifest>(
        r#"{
          "schema":"glyphshift.runtime-bundle/3",
          "authority":"app.glyphshift.runtime.first-party",
          "controller":{"artifact":"windows","file":"controller.exe","sha256":"0000000000000000000000000000000000000000000000000000000000000000","protocol":[1,0]},
          "runtime":{"file":"runtime.dll","sha256":"0000000000000000000000000000000000000000000000000000000000000000"},
          "adapters":[],
          "acquisition_workers":[{"file":"worker.exe","sha256":"0000000000000000000000000000000000000000000000000000000000000000","adapter_id":"windows.uia.acquire"}]
        }"#,
    );

    assert!(result.is_err());
}
