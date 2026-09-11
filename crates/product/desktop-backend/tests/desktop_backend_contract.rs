use glyphshift_adapter_registry::{AdapterRequirement, AdapterVersion, AdapterVersionRequirement};
use glyphshift_desktop_backend::{
    BackendError, DesktopBackend, DesktopEnvironment, DictionaryCreate, DictionaryEdit,
    DictionaryEntryCreate, FontCoverage, WorkflowCreate, WorkflowFontPolicy, WorkflowTargetCreate,
};
use glyphshift_dictionary_distribution::{
    ArtifactPresentation, ArtifactStatement, CatalogRelease, DictionaryArtifactDescriptor,
    DictionaryDistribution, DictionaryReleaseKey, DictionaryReplacementPolicy,
    FileDictionaryInstallStore, FixedInstallationClock, InMemoryDictionaryCatalog,
    InMemoryTrustVerifier, InstallRequest, PublisherIdentity, Sha256Digest, SignatureEnvelope,
};
use glyphshift_domain::{AdapterId, Feature};
use sha2::{Digest, Sha256};
use std::fs;
use tempfile::tempdir;

fn environment() -> DesktopEnvironment {
    DesktopEnvironment::new(
        [AdapterRequirement::new(
            AdapterId::new("adapter-gdi"),
            AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
            [
                Feature::TextObserve,
                Feature::TextReplace,
                Feature::FontSubstitute,
            ],
        )],
        ["Available Sans"],
    )
}

#[test]
fn workflow_shortcut_survives_restart_and_is_not_copied() {
    let root = tempdir().expect("product data");
    let executable = root.path().join("SyntheticShortcutEditor.exe");
    fs::write(&executable, b"synthetic executable").expect("synthetic executable");
    let mut backend =
        DesktopBackend::open_with_environment(root.path(), environment()).expect("backend");
    let software_id = backend
        .add_software(glyphshift_desktop_backend::ExecutableSelection::new(
            &executable,
        ))
        .expect("software")
        .selected_software_id()
        .expect("selected")
        .to_owned();
    let create = WorkflowCreate::new("shortcut-workflow", "Shortcut workflow")
        .with_global_shortcut("Ctrl+Alt+KeyT")
        .with_targets([WorkflowTargetCreate::new(
            software_id,
            ["adapter-gdi"],
            [] as [&str; 0],
        )]);
    backend.create_workflow(create).expect("workflow");
    let mut reopened =
        DesktopBackend::open_with_environment(root.path(), environment()).expect("restart");
    assert_eq!(
        reopened
            .workflow("shortcut-workflow")
            .expect("workflow")
            .global_shortcut(),
        "Ctrl+Alt+KeyT"
    );
    assert_eq!(
        reopened.snapshot().workflows()[0].global_shortcut(),
        "Ctrl+Alt+KeyT"
    );
    let copy = reopened
        .copy_workflow("shortcut-workflow", "shortcut-copy", "Copy")
        .expect("copy");
    assert_eq!(copy.global_shortcut(), "");
    let path = root.path().join("workflows-v4/shortcut-workflow.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    value["globalShortcut"] = serde_json::json!({"invalid":true});
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let recovered = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("invalid optional field recovery");
    assert_eq!(
        recovered
            .workflow("shortcut-workflow")
            .expect("workflow retained")
            .global_shortcut(),
        ""
    );
}

#[test]
fn desktop_summary_derives_verified_then_modified_without_polluting_dictionary_content() {
    const ARTIFACT_URL: &str = "https://catalog.example/dictionary.json";
    let root = tempdir().expect("isolated product data");
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("open empty product data");
    let payload = glyphshift_dictionary_package::DictionaryPackage::create(
        glyphshift_dictionary_package::DictionaryCreate::new(
            "dictionary.catalog",
            "Catalog Dictionary",
            "en-US",
            "zh-CN",
        )
        .with_release_version("1.2.0")
        .with_entries([glyphshift_dictionary_package::DictionaryEntryCreate::new(
            "Open", "打开",
        )]),
    )
    .expect("dictionary package")
    .encode_json()
    .expect("encode dictionary")
    .into_bytes();
    let digest: [u8; 32] = Sha256::digest(&payload).into();
    let publisher = PublisherIdentity::new("publisher.example").expect("publisher");
    let signature =
        SignatureEnvelope::new("fixture", "test-key", "signed-statement").expect("signature");
    let release = CatalogRelease::new(
        DictionaryReleaseKey::new("glyphshift.official", "dictionary.catalog", "1.2.0")
            .expect("release key"),
        "en-US",
        "zh-CN",
        "en-US",
        vec![
            ArtifactPresentation::new("en-US", "Catalog Dictionary", "Test release")
                .expect("presentation"),
        ],
        DictionaryArtifactDescriptor::new(
            payload.len() as u64,
            Sha256Digest::new(digest),
            [ARTIFACT_URL],
            publisher.clone(),
            signature.clone(),
        )
        .expect("artifact descriptor"),
    )
    .expect("catalog release");
    let mut distribution = DictionaryDistribution::new(
        Box::new(
            InMemoryDictionaryCatalog::new()
                .with_release(release.clone())
                .with_artifact(ARTIFACT_URL, payload),
        ),
        Box::new(InMemoryTrustVerifier::new().with_trusted_artifact(
            ArtifactStatement::for_release(&release),
            signature,
            publisher,
        )),
        Box::new(FileDictionaryInstallStore::open(root.path()).expect("file install store")),
        Box::new(FixedInstallationClock::new(1_700_000_000_000)),
    );
    distribution
        .install(&InstallRequest::new(
            release.key().clone(),
            DictionaryReplacementPolicy::RejectExisting,
        ))
        .expect("install release");

    backend
        .reload_dictionaries()
        .expect("reload active dictionaries");
    let snapshot = backend.snapshot();
    let installed = &snapshot.dictionaries()[0];
    assert_eq!(installed.installation().state(), "verified");
    assert_eq!(installed.installation().installed_release(), Some("1.2.0"));
    assert_eq!(
        installed.installation().verified_publisher(),
        Some("publisher.example")
    );

    backend
        .update_dictionary(
            DictionaryEdit::new(
                "dictionary.catalog",
                "Catalog Dictionary",
                "en-US",
                "zh-CN",
                1,
            )
            .with_release_version("1.2.0")
            .with_entries([DictionaryEntryCreate::new("Open", "开启")]),
        )
        .expect("edit installed dictionary");
    assert_eq!(
        backend.snapshot().dictionaries()[0].installation().state(),
        "modified"
    );
    let dictionary_json =
        fs::read_to_string(root.path().join("dictionaries/dictionary.catalog.json"))
            .expect("active dictionary");
    assert!(!dictionary_json.contains("installation"));
    assert!(!dictionary_json.contains("publisher"));
    assert!(!dictionary_json.contains("digest"));
}

#[test]
fn desktop_imports_and_exports_one_portable_dictionary_v3_file() {
    let root = tempdir().expect("temporary product data");
    let exchange = tempdir().expect("temporary exchange data");
    let input = exchange.path().join("portable-dictionary.json");
    let output = exchange.path().join("published-dictionary.json");
    let package = glyphshift_dictionary_package::DictionaryPackage::create(
        glyphshift_dictionary_package::DictionaryCreate::new(
            "dictionary.portable",
            "Portable Dictionary",
            "en-US",
            "zh-CN",
        )
        .with_release_version("1.0.0")
        .with_font_families(["Dictionary Sans", "Fallback Sans"]).with_font_scale_percent(Some(125))
        .with_entries([
            glyphshift_dictionary_package::DictionaryEntryCreate::new("Open", "打开"),
            glyphshift_dictionary_package::DictionaryEntryCreate::new(
                "Quoted, \"text\"",
                "第一行\n第二行",
            ),
        ]),
    )
    .expect("portable dictionary package");
    fs::write(
        &input,
        package.encode_json().expect("encode portable dictionary"),
    )
    .expect("write import fixture");

    let mut backend = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("open empty product data");
    let imported = backend
        .import_dictionary_file(&input)
        .expect("import portable dictionary");
    assert_eq!(imported.id(), "dictionary.portable");
    assert_eq!(
        backend.snapshot().dictionaries()[0].installation().state(),
        "unmanaged"
    );

    backend
        .export_dictionary_file("dictionary.portable", &output)
        .expect("export portable dictionary");
    let exported = fs::read_to_string(output).expect("read exported dictionary");
    let reopened = glyphshift_dictionary_package::DictionaryPackage::decode_json(
        &exported,
        Some("dictionary.portable"),
    )
    .expect("export remains a valid dictionary package");
    assert_eq!(reopened.revision(), package.revision());
    assert_eq!(reopened.view().metadata().font_families(), package.view().metadata().font_families());
    assert_eq!(imported.metadata().font_families(), package.view().metadata().font_families());
    assert_eq!(reopened.view().metadata().font_scale_percent(), Some(125));
    assert_eq!(reopened.view().entries()[0].translation(), Some("打开"));
    let csv_path = exchange.path().join("export.CSV");
    backend
        .export_dictionary_file("dictionary.portable", &csv_path)
        .expect("CSV export");
    assert_eq!(
        fs::read_to_string(csv_path).expect("CSV text"),
        "\u{feff}source,translation\r\n\"Open\",\"打开\"\r\n\"Quoted, \"\"text\"\"\",\"第一行\n第二行\"\r\n"
    );
}

#[test]
fn desktop_rejects_invalid_or_duplicate_dictionary_imports_without_overwriting() {
    let root = tempdir().expect("temporary product data");
    let exchange = tempdir().expect("temporary exchange data");
    let valid = exchange.path().join("valid.json");
    let invalid = exchange.path().join("invalid.json");
    let package = glyphshift_dictionary_package::DictionaryPackage::create(
        glyphshift_dictionary_package::DictionaryCreate::new(
            "dictionary.duplicate",
            "Original",
            "en-US",
            "zh-CN",
        ),
    )
    .expect("dictionary package");
    fs::write(&valid, package.encode_json().expect("encode dictionary"))
        .expect("write valid fixture");
    fs::write(&invalid, "{not-json").expect("write invalid fixture");

    let mut backend = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("open empty product data");
    backend
        .import_dictionary_file(&valid)
        .expect("first import succeeds");
    assert!(matches!(
        backend.import_dictionary_file(&valid),
        Err(BackendError::DuplicateDictionary(id)) if id.as_ref() == "dictionary.duplicate"
    ));
    assert!(matches!(
        backend.import_dictionary_file(&invalid),
        Err(BackendError::InvalidArtifact("dictionary-import-json"))
    ));
    assert_eq!(
        backend
            .dictionary("dictionary.duplicate")
            .unwrap()
            .metadata()
            .name(),
        "Original"
    );
}

#[test]
fn capture_spec_uses_only_explicit_observable_adapters_and_an_empty_publication() {
    let root = tempdir().expect("capture product data");
    let executable = root.path().join("SyntheticCaptureHost.exe");
    fs::write(&executable, b"synthetic executable").expect("synthetic executable fixture");
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("open capture product data");
    let software_id = backend
        .add_software(glyphshift_desktop_backend::ExecutableSelection::new(
            executable,
        ))
        .expect("register capture target")
        .selected_software_id()
        .expect("selected software")
        .to_owned();

    let spec = backend
        .capture_runtime_spec(&software_id, &[Box::<str>::from("adapter-gdi")])
        .expect("compile capture spec");

    assert_eq!(spec.requirements().len(), 1);
    assert_eq!(
        spec.requirements()[0].features().collect::<Vec<_>>(),
        vec![Feature::TextObserve, Feature::TextReplace]
    );
    let mut translations = 0;
    spec.publication()
        .snapshot()
        .visit_entries(|_, _, _| translations += 1);
    let mut fonts = 0;
    spec.publication()
        .font_policy()
        .visit_locations(|_, _| fonts += 1);
    assert_eq!((translations, fonts), (0, 0));
}

#[test]
fn software_extension_process_family_reaches_the_runtime_spec_without_entering_workflow_data() {
    let root = tempdir().expect("process family product data");
    let executable = root.path().join("SyntheticFamilyHost.exe");
    fs::write(&executable, b"synthetic executable").expect("synthetic executable fixture");
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("open process family product data");
    let software_id = backend
        .add_software(glyphshift_desktop_backend::ExecutableSelection::new(
            executable,
        ))
        .expect("register process family root")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    drop(backend);

    let extension_path = root
        .path()
        .join("extensions")
        .join(format!("{software_id}.json"));
    let mut artifact = serde_json::from_str::<serde_json::Value>(
        &fs::read_to_string(&extension_path).expect("software extension"),
    )
    .expect("extension json");
    artifact["descendant_executables"] =
        serde_json::json!(["SyntheticRenderer.exe", "SyntheticWorker.exe"]);
    fs::write(
        &extension_path,
        serde_json::to_string(&artifact).expect("encode extension json"),
    )
    .expect("write process family extension");

    let reopened = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("reopen process family product data");
    let spec = reopened
        .runtime_spec(&software_id)
        .expect("compile process family runtime spec");

    assert_eq!(
        spec.descendant_executable_names(),
        &[
            Box::<str>::from("SyntheticRenderer.exe"),
            Box::<str>::from("SyntheticWorker.exe"),
        ]
    );
    assert!(reopened.snapshot().workflows().is_empty());
}

#[test]
fn desktop_workflow_v4_persists_adapter_plan_and_inline_font_policy_outside_the_dictionary() {
    let root = tempdir().expect("isolated product data");
    let executable = root.path().join("SyntheticEditor.exe");
    fs::write(&executable, b"synthetic executable").expect("synthetic executable fixture");
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("open empty product data");
    let software_id = backend
        .add_software(glyphshift_desktop_backend::ExecutableSelection::new(
            &executable,
        ))
        .expect("add synthetic software")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary-ui", "UI", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
        )
        .expect("create pure dictionary");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow-ui", "UI Workflow").with_targets([
                WorkflowTargetCreate::new(software_id.clone(), ["adapter-gdi"], ["dictionary-ui"])
                    .with_font_policy(serde_json::from_value(serde_json::json!({
                        "families": ["Available Sans"], "coverage": "dictionary_matches",
                        "dictionaryOverrides": {"dictionary-ui": {"families": ["Available Sans"], "scalePercent": 100}}
                    })).unwrap()),
            ]),
        )
        .expect("create composed workflow");

    let dictionary_json = fs::read_to_string(root.path().join("dictionaries/dictionary-ui.json"))
        .expect("dictionary artifact");
    let workflow_json = fs::read_to_string(root.path().join("workflows-v4/workflow-ui.json"))
        .expect("workflow artifact");
    assert!(dictionary_json.contains("glyphshift.dictionary/3"));
    assert!(!dictionary_json.contains("adapterPlan"));
    assert!(!dictionary_json.contains("fontProfile"));
    assert!(workflow_json.contains("glyphshift.workflow/4"));
    assert!(workflow_json.contains("adapterPlan"));
    assert!(workflow_json.contains("fontPolicy"));
    assert!(workflow_json.contains("dictionary_matches"));
    assert!(!workflow_json.contains("fontBindings"));
    assert!(!workflow_json.contains("location"));

    let mut reopened = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("reopen composed product data");
    let workflow = reopened.workflow("workflow-ui").expect("workflow detail");
    assert_eq!(
        (
            workflow.targets()[0].adapter_plan().adapter_ids(),
            workflow.targets()[0]
                .font_policy()
                .expect("inline font policy")
                .families(),
            workflow.targets()[0]
                .font_policy()
                .expect("inline font policy")
                .coverage(),
        ),
        (
            &[Box::<str>::from("adapter-gdi")][..],
            &[Box::<str>::from("Available Sans")][..],
            FontCoverage::DictionaryMatches,
        )
    );
    assert_eq!(
        reopened
            .effective_workflow_intent("workflow-ui")
            .expect("compiled intent")
            .targets()[0]
            .requested_features(),
        &[Feature::TextReplace, Feature::FontSubstitute]
    );
    let policy = serde_json::to_value(workflow.targets()[0].font_policy()).unwrap();
    assert_eq!(policy["dictionaryOverrides"]["dictionary-ui"]["scalePercent"], 100);
    let copied = reopened.copy_workflow("workflow-ui", "copy-ui", "Copy").unwrap();
    assert_eq!(serde_json::to_value(copied.targets()[0].font_policy()).unwrap(), policy);
    assert!(!dictionary_json.contains("dictionaryOverrides"));
    for overrides in [
        serde_json::json!({"dictionary-ui": {"families": [], "scalePercent": 201}}),
        serde_json::json!({"not-selected": {"families": ["Available Sans"]}}),
    ] {
        let policy = serde_json::from_value(serde_json::json!({"families": [], "coverage": "dictionary_matches", "dictionaryOverrides": overrides})).unwrap();
        assert!(reopened.create_workflow(WorkflowCreate::new("invalid-overrides", "Invalid").with_targets([
            WorkflowTargetCreate::new(software_id.clone(), ["adapter-gdi"], ["dictionary-ui"]).with_font_policy(policy)
        ])).is_err());
    }

}

#[test]
fn desktop_persists_pending_dictionary_entries_but_publishes_only_completed_entries() {
    let root = tempdir().expect("pending dictionary product data");
    let executable = root.path().join("SyntheticPendingHost.exe");
    fs::write(&executable, b"synthetic executable").expect("synthetic executable fixture");
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("open empty product data");
    let software_id = backend
        .add_software(glyphshift_desktop_backend::ExecutableSelection::new(
            executable,
        ))
        .expect("add synthetic software")
        .selected_software_id()
        .expect("selected software")
        .to_owned();

    let dictionary = backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.pending", "Pending", "en-US", "zh-CN").with_entries(
                [
                    DictionaryEntryCreate::new("Open", "打开"),
                    DictionaryEntryCreate::new("Save", ""),
                ],
            ),
        )
        .expect("persist completed and pending entries");
    assert_eq!(dictionary.entries().len(), 2);
    assert_eq!(dictionary.entries()[1].translation(), "");

    backend
        .create_workflow(
            WorkflowCreate::new("workflow.pending", "Pending Workflow").with_targets([
                WorkflowTargetCreate::new(
                    software_id.clone(),
                    ["adapter-gdi"],
                    ["dictionary.pending"],
                ),
            ]),
        )
        .expect("create workflow");
    let runtime = backend
        .workflow_runtime_spec("workflow.pending", &software_id)
        .expect("compile runtime spec");
    let mut published = Vec::new();
    runtime
        .publication()
        .snapshot()
        .visit_entries(|_, source, translation| {
            published.push((source.to_owned(), translation.to_owned()));
        });

    assert_eq!(published, vec![("Open".to_owned(), "打开".to_owned())]);
    let encoded = String::from_utf8(
        backend
            .dictionary_json("dictionary.pending")
            .expect("read pending dictionary"),
    )
    .expect("dictionary utf8");
    assert!(encoded.contains("glyphshift.dictionary/3"));
    assert!(encoded.contains(r#"{"source":"Save"}"#));
}

#[test]
fn desktop_keeps_dictionary_text_and_workflow_font_policy_independent_across_restart() {
    let root = tempdir().expect("temporary product data");
    let executable = root.path().join("SyntheticCreativeHost.exe");
    fs::write(&executable, b"synthetic executable").expect("synthetic executable fixture");
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("open empty product data");
    let software_id = backend
        .add_software(glyphshift_desktop_backend::ExecutableSelection::new(
            executable,
        ))
        .expect("add synthetic software")
        .selected_software_id()
        .expect("selected software")
        .to_owned();

    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.interface-zh-cn", "界面汉化", "en", "zh-CN")
                .with_description("用于创作软件界面汉化")
                .with_release_version("0.1.0")
                .with_tags(["menu", "ui"])
                .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
        )
        .expect("create a pure dictionary");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.cjk-ui", "简体中文界面").with_targets([
                WorkflowTargetCreate::new(
                    software_id,
                    ["adapter-gdi"],
                    ["dictionary.interface-zh-cn"],
                )
                .with_font_policy(WorkflowFontPolicy::new(
                    ["Unavailable Sans", "Available Sans"],
                    FontCoverage::AllObservations,
                )),
            ]),
        )
        .expect("create workflow with inline font policy");
    drop(backend);

    let reopened = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("reopen current product data");
    let dictionary = reopened
        .dictionary("dictionary.interface-zh-cn")
        .expect("read dictionary by id");
    let workflow = reopened
        .workflow("workflow.cjk-ui")
        .expect("read workflow by id");
    let font_policy = workflow.targets()[0]
        .font_policy()
        .expect("read inline font policy");

    assert_eq!(
        (
            dictionary.metadata().name(),
            dictionary.metadata().source_locale(),
            dictionary.metadata().target_locale(),
            dictionary.metadata().release_version(),
            dictionary.metadata().tags(),
            dictionary.entries()[0].translation(),
            workflow.name(),
            font_policy.families(),
            font_policy.coverage(),
        ),
        (
            "界面汉化",
            "en",
            "zh-CN",
            "0.1.0",
            &[Box::<str>::from("menu"), Box::<str>::from("ui")][..],
            "打开",
            "简体中文界面",
            &[
                Box::<str>::from("Unavailable Sans"),
                Box::<str>::from("Available Sans"),
            ][..],
            FontCoverage::AllObservations,
        )
    );
}

#[test]
fn desktop_migrates_dictionary_v2_without_losing_completed_entries() {
    let root = tempdir().expect("temporary product data");
    drop(
        DesktopBackend::open_with_environment(root.path(), environment())
            .expect("initialize current product data"),
    );
    let dictionary_path = root.path().join("dictionaries/dictionary.legacy.json");
    fs::write(
        &dictionary_path,
        r#"{
  "schema": "glyphshift.dictionary/2",
  "revision": 7,
  "metadata": {
    "id": "dictionary.legacy",
    "releaseVersion": "1.2.3",
    "name": "Legacy",
    "description": "Legacy product data",
    "sourceLocale": "en-US",
    "targetLocale": "zh-CN",
    "authors": ["Fixture"],
    "license": "MIT",
    "homepage": null,
    "tags": ["ui"]
  },
  "entries": [{"source": "Open", "translation": "打开"}]
}"#,
    )
    .expect("write synthetic Dictionary /2 fixture");

    let reopened = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("migrate valid Dictionary /2 product data");
    let dictionary = reopened
        .dictionary("dictionary.legacy")
        .expect("load migrated dictionary");
    assert_eq!(dictionary.revision(), 7);
    assert_eq!(dictionary.entries()[0].translation(), "打开");
    let persisted = fs::read_to_string(dictionary_path).expect("read migrated dictionary");
    assert!(persisted.contains("glyphshift.dictionary/3"));
    assert!(!persisted.contains("glyphshift.dictionary/2"));
}

#[test]
fn desktop_skips_one_invalid_dictionary_and_reports_it_without_failing_startup() {
    let root = tempdir().expect("temporary product data");
    drop(
        DesktopBackend::open_with_environment(root.path(), environment())
            .expect("initialize current product data"),
    );
    fs::write(
        root.path().join("dictionaries/legacy.json"),
        r#"{
  "schema": "glyphshift.dictionary/1",
  "id": "legacy",
  "name": "Legacy",
  "locale": "zh-CN",
  "hookType": "gdi",
  "defaultFont": "Legacy Sans",
  "entries": []
}"#,
    )
    .expect("write synthetic obsolete dictionary fixture");

    let reopened = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("skip one unsupported dictionary without failing startup");
    assert!(reopened.snapshot().dictionaries().is_empty());
    let snapshot = serde_json::to_value(reopened.snapshot()).expect("serialize startup warnings");
    assert_eq!(
        snapshot["artifactWarnings"][0]["artifactKind"],
        "dictionary"
    );
    assert_eq!(snapshot["artifactWarnings"][0]["artifactId"], "legacy");
}

#[test]
fn desktop_keeps_valid_local_software_when_another_record_is_invalid() {
    let root = tempdir().expect("temporary product data");
    let executable = root.path().join("SyntheticEditor.exe");
    fs::write(&executable, b"synthetic executable").expect("synthetic executable fixture");
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("open empty product data");
    backend
        .add_software(glyphshift_desktop_backend::ExecutableSelection::new(
            &executable,
        ))
        .expect("add valid software");
    drop(backend);

    let state_path = root.path().join("desktop-state.json");
    let mut state: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&state_path).expect("read desktop state"))
            .expect("parse desktop state");
    state["unknownFutureField"] = serde_json::json!(true);
    let valid_id = state["selectedSoftwareId"]
        .as_str()
        .expect("selected valid software")
        .to_owned();
    state["software"][&valid_id]["displayName"] = serde_json::json!(42);
    state["software"][&valid_id]["description"] = serde_json::json!(42);
    state["software"][&valid_id]["unknownRecordField"] = serde_json::json!(true);
    state["software"]["local.invalid"] = serde_json::json!({
        "displayName": 42,
        "description": "invalid record",
        "executablePath": "relative.exe"
    });
    fs::write(
        &state_path,
        serde_json::to_vec(&state).expect("encode partial desktop state"),
    )
    .expect("write partial desktop state");

    let reopened = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("keep valid local software");
    assert_eq!(reopened.snapshot().software().len(), 1);
    assert_eq!(reopened.snapshot().software()[0].name(), valid_id);
}

#[test]
fn desktop_skips_one_invalid_workflow_without_hiding_other_workflows() {
    let root = tempdir().expect("temporary product data");
    let executable = root.path().join("SyntheticEditor.exe");
    fs::write(&executable, b"synthetic executable").expect("synthetic executable fixture");
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("open empty product data");
    let software_id = backend
        .add_software(glyphshift_desktop_backend::ExecutableSelection::new(
            &executable,
        ))
        .expect("add valid software")
        .selected_software_id()
        .expect("selected software")
        .to_owned();
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.valid", "Valid Workflow").with_targets([
                WorkflowTargetCreate::new(software_id, ["adapter-gdi"], [] as [&str; 0]),
            ]),
        )
        .expect("create valid workflow");
    drop(backend);
    let valid_path = root.path().join("workflows-v4/workflow.valid.json");
    let mut valid: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&valid_path).expect("read valid workflow"))
            .expect("parse valid workflow");
    valid["name"] = serde_json::json!(42);
    valid["description"] = serde_json::json!(42);
    valid["revision"] = serde_json::json!("invalid");
    valid["unknownFutureField"] = serde_json::json!(true);
    valid["targets"]
        .as_array_mut()
        .expect("workflow targets")
        .push(serde_json::json!({ "softwareId": 42 }));
    fs::write(
        &valid_path,
        serde_json::to_vec(&valid).expect("encode partially invalid workflow"),
    )
    .expect("write partially invalid workflow");
    fs::write(
        root.path().join("workflows-v4/workflow.invalid.json"),
        r#"{"schema":"glyphshift.workflow/4","id":42,"name":"Invalid"}"#,
    )
    .expect("write invalid workflow record");

    let reopened = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("skip only invalid workflow");
    assert_eq!(reopened.snapshot().workflows().len(), 1);
    assert_eq!(reopened.snapshot().workflows()[0].id(), "workflow.valid");
    assert_eq!(reopened.snapshot().workflows()[0].name(), "workflow.valid");
    assert_eq!(reopened.snapshot().workflows()[0].revision(), 1);
}

#[test]
fn workflow_write_dictionary_is_selected_persistent_and_copied() {
    let root = tempdir().unwrap();
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
    let target = || WorkflowTargetCreate::new("software.synthetic", ["adapter-gdi"], ["dictionary.a", "dictionary.b"]);
    assert!(backend.create_workflow(WorkflowCreate::new("invalid-writer", "Invalid")
        .with_targets([target().with_write_dictionary("dictionary.other")])).is_err());
    backend.create_workflow(WorkflowCreate::new("writer", "Writer")
        .with_targets([target().with_write_dictionary("dictionary.a")])).unwrap();
    let mut reopened = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
    assert_eq!(reopened.workflow("writer").unwrap().targets()[0].write_dictionary_id(), Some("dictionary.a"));
    assert_eq!(reopened.copy_workflow("writer", "writer-copy", "Copy").unwrap().targets()[0].write_dictionary_id(), Some("dictionary.a"));
    let path = root.path().join("workflows-v4/writer.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["targets"][0]["writeDictionaryId"] = serde_json::json!("dictionary.other");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let recovered = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
    assert_eq!(recovered.workflow("writer").unwrap().targets()[0].write_dictionary_id(), None);
    assert_eq!(recovered.workflow("writer").unwrap().targets()[0].dictionary_ids().len(), 2);
}


#[test]
fn breaking_workflow_upgrade_does_not_load_legacy_tasks_or_activation() {
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("workflows")).unwrap();
    fs::write(root.path().join("workflows/legacy.json"), r#"{"id":"legacy","name":"Legacy","targets":[]}"#).unwrap();
    fs::write(root.path().join("workflow-state.json"), r#"{"enabledWorkflows":["legacy"]}"#).unwrap();
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
    backend.create_dictionary(DictionaryCreate::new("keep", "Keep", "en-US", "zh-CN")
        .with_entries([DictionaryEntryCreate::new("Keep", "Preserved")])).unwrap();
    drop(backend);
    let backend = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
    assert!(backend.snapshot().workflows().is_empty());
    assert!(backend.enabled_workflow_ids().is_empty());
    assert_eq!(backend.dictionary("keep").unwrap().entries()[0].translation(), "Preserved");
}

#[test]
fn workflow_creation_rejects_multiple_software_targets() {
    let root = tempdir().unwrap();
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
    let create = WorkflowCreate::new("multi", "Multi").with_targets([
        WorkflowTargetCreate::new("one", ["adapter-gdi"], [] as [&str; 0]),
        WorkflowTargetCreate::new("two", ["adapter-gdi"], [] as [&str; 0]),
    ]);
    assert!(backend.create_workflow(create).is_err());
    assert!(backend.snapshot().workflows().is_empty());
}

#[test]
fn workflow_uses_selected_dictionary_language_instead_of_software_default() {
    let root = tempdir().unwrap();
    let executable = root.path().join("SyntheticLanguageHost.exe");
    fs::write(&executable, b"synthetic executable").unwrap();
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
    let software = backend.add_software(glyphshift_desktop_backend::ExecutableSelection::new(&executable))
        .unwrap().selected_software_id().unwrap().to_owned();
    backend.create_dictionary(DictionaryCreate::new("to-english", "English", "zh-CN", "en-US")
        .with_entries([DictionaryEntryCreate::new("打开", "Open")])).unwrap();
    for (id, writer) in [("collect-english", true), ("translate-english", false)] {
        let target = WorkflowTargetCreate::new(software.as_str(), ["adapter-gdi"], ["to-english"]);
        let target = if writer { target.with_write_dictionary("to-english") } else { target };
        backend.create_workflow(WorkflowCreate::new(id, id).with_targets([target])).unwrap();
        let runtime = backend.workflow_runtime_spec(id, &software).expect("dictionary determines the target language");
        let mut entries = Vec::new();
        runtime.publication().snapshot().visit_entries(|_, source, translation| entries.push((source.to_owned(), translation.to_owned())));
        assert_eq!(entries, vec![("打开".to_owned(), "Open".to_owned())]);
    }
    let restored = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
    assert!(restored.workflow_runtime_spec("collect-english", &software).is_ok());
}

#[test]
fn missing_software_binding_never_becomes_name_only_runtime_authorization() {
    for damaged_state in ["{broken", r#"{"software":{}}"#] {
        let root = tempdir().unwrap();
        let executable = root.path().join("SyntheticBoundEditor.exe");
        fs::write(&executable, b"synthetic").unwrap();
        let mut backend = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
        let id = backend.add_software(glyphshift_desktop_backend::ExecutableSelection::new(&executable))
            .unwrap().selected_software_id().unwrap().to_owned();
        backend.create_dictionary(DictionaryCreate::new("binding-dictionary", "Binding", "en-US", "zh-CN")
            .with_entries([DictionaryEntryCreate::new("Save", "保存")])).unwrap();
        backend.create_workflow(WorkflowCreate::new("binding-workflow", "Binding").with_targets([
            WorkflowTargetCreate::new(&*id, ["adapter-gdi"], ["binding-dictionary"]).with_write_dictionary("binding-dictionary")
        ])).unwrap();
        drop(backend);
        fs::write(root.path().join("desktop-state.json"), damaged_state).unwrap();
        let mut reopened = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
        assert_eq!(reopened.snapshot().software().len(), 1, "retain the asset for repair");
        let expected = BackendError::SoftwareBindingMissing(id.clone().into());
        assert_eq!(reopened.runtime_spec(&id).unwrap_err(), expected);
        assert_eq!(reopened.capture_runtime_spec(&id, &["adapter-gdi".into()]).unwrap_err(), expected);
        assert_eq!(reopened.workflow_runtime_spec("binding-workflow", &id).unwrap_err(), expected);
        assert_eq!(reopened.effective_workflow_intent("binding-workflow").unwrap_err(), expected);
        assert_eq!(reopened.enable_workflow("binding-workflow").unwrap_err(), expected);
        reopened.update_software(glyphshift_desktop_backend::SoftwareEdit::new(&*id, "Rebound", &executable)).unwrap();
        assert_eq!(reopened.runtime_spec(&id).unwrap().executable_paths(), &[executable.to_string_lossy().into_owned().into_boxed_str()]);
        reopened.enable_workflow("binding-workflow").unwrap();
    }
}

#[test]
fn malformed_legacy_runtime_is_isolated_to_its_software_record() {
    for runtime in [
        serde_json::json!({"capabilities":[{"adapter":"adapter-gdi","version":[1,0,0],"features":["unknown_legacy_feature"]}],"route":{"kind":"direct","locations":["internal-default"]}}),
        serde_json::json!({"capabilities":[],"route":{"kind":"direct","locations":["internal-default"]}}),
        serde_json::json!({"capabilities":[{"adapter":"adapter-gdi","version":[1,0,0],"features":["text_replace"]}],"route":{"kind":"direct","locations":["missing-location"]}}),
    ] {
        let root = tempdir().unwrap();
        let mut backend = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
        let mut ids = Vec::new();
        for name in ["SyntheticHealthy.exe", "SyntheticLegacy.exe"] {
            let executable = root.path().join(name);
            fs::write(&executable, b"synthetic").unwrap();
            ids.push(backend.add_software(glyphshift_desktop_backend::ExecutableSelection::new(executable))
                .unwrap().selected_software_id().unwrap().to_owned());
        }
        drop(backend);
        let path = root.path().join("extensions").join(format!("{}.json", ids[1]));
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["runtime"] = runtime;
        let original = serde_json::to_vec(&value).unwrap();
        fs::write(&path, &original).unwrap();
        let reopened = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
        assert_eq!(reopened.snapshot().software().len(), 1);
        assert_eq!(reopened.snapshot().software()[0].id(), ids[0]);
        assert_eq!(fs::read(path).unwrap(), original, "preserve rejected evidence");
        let snapshot = serde_json::to_value(reopened.snapshot()).unwrap();
        assert!(snapshot["artifactWarnings"].as_array().unwrap().iter().any(|warning| warning["artifactId"] == ids[1] && warning["issue"] == "invalid_runtime"));
    }
}

#[test]
fn dictionary_rules_change_workflow_publication_with_dictionary_revision() {
    use glyphshift_translation::{RegexTranslationRule, RegexTranslationRules};
    let root = tempdir().unwrap();
    let executable = root.path().join("SyntheticRulesEditor.exe");
    fs::write(&executable, b"synthetic executable").unwrap();
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment()).unwrap();
    let software_id = backend.add_software(glyphshift_desktop_backend::ExecutableSelection::new(&executable)).unwrap().selected_software_id().unwrap().to_owned();
    backend.create_dictionary(DictionaryCreate::new("rules-dictionary", "Rules dictionary", "en", "zh-CN").with_entries([DictionaryEntryCreate::new("Total", "总计")])).unwrap();
    backend.create_workflow(WorkflowCreate::new("rules-workflow", "Rules workflow").with_targets([WorkflowTargetCreate::new(software_id.as_str(), ["adapter-gdi"], ["rules-dictionary"])] )).unwrap();
    let old = backend.workflow_runtime_spec("rules-workflow", &software_id).unwrap();
    let rules = RegexTranslationRules::compile(vec![RegexTranslationRule { pattern: r"^(.+?)(:[0-9]+)$".into(), replacement: "{{TR}}$2".into(), enabled: true }]).unwrap();
    let dictionary = backend.dictionary("rules-dictionary").unwrap().clone();
    backend.update_dictionary(glyphshift_desktop_backend::DictionaryEdit::from_dictionary(&dictionary).with_text_rules(rules.rules().to_vec())).unwrap();
    let new = backend.workflow_runtime_spec("rules-workflow", &software_id).unwrap();
    assert!(new.publication().generation() > old.publication().generation());
    assert_ne!(new.publication().identity().unwrap(), old.publication().identity().unwrap());
    assert_eq!(new.publication(), backend.workflow_runtime_spec("rules-workflow", &software_id).unwrap().publication());
    let dictionary = backend.dictionary("rules-dictionary").unwrap().clone();
    backend.update_dictionary(glyphshift_desktop_backend::DictionaryEdit::from_dictionary(&dictionary).with_text_rules(vec![])).unwrap();
    let removed = backend.workflow_runtime_spec("rules-workflow", &software_id).unwrap();
    assert!(removed.publication().generation() > new.publication().generation());
    assert!(removed.publication().snapshot().dictionary_rules().is_empty());
    assert_eq!(backend.workflow("rules-workflow").unwrap().revision(), 1);
}
