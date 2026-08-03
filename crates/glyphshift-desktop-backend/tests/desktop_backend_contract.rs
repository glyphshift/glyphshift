use glyphshift_adapter_registry::{AdapterRequirement, AdapterVersion, AdapterVersionRequirement};
use glyphshift_desktop_backend::{
    BackendError, DesktopBackend, DesktopEnvironment, DictionaryCreate, DictionaryEntryCreate,
    FontCoverage, WorkflowCreate, WorkflowFontPolicy, WorkflowTargetCreate,
};
use glyphshift_domain::{AdapterId, Feature};
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
fn desktop_workflow_v3_persists_adapter_plan_and_inline_font_policy_outside_the_dictionary() {
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
                    .with_font_policy(WorkflowFontPolicy::new(
                        ["Available Sans"],
                        FontCoverage::DictionaryMatches,
                    )),
            ]),
        )
        .expect("create composed workflow");

    let dictionary_json = fs::read_to_string(root.path().join("dictionaries/dictionary-ui.json"))
        .expect("dictionary artifact");
    let workflow_json = fs::read_to_string(root.path().join("workflows/workflow-ui.json"))
        .expect("workflow artifact");
    assert!(dictionary_json.contains("glyphshift.dictionary/2"));
    assert!(!dictionary_json.contains("adapterPlan"));
    assert!(!dictionary_json.contains("fontProfile"));
    assert!(workflow_json.contains("glyphshift.workflow/3"));
    assert!(workflow_json.contains("adapterPlan"));
    assert!(workflow_json.contains("fontPolicy"));
    assert!(workflow_json.contains("dictionary_matches"));
    assert!(!workflow_json.contains("fontBindings"));
    assert!(!workflow_json.contains("location"));

    let reopened = DesktopBackend::open_with_environment(root.path(), environment())
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
fn desktop_rejects_the_unreleased_dictionary_v1_shape_instead_of_migrating_it() {
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

    let reopened = DesktopBackend::open_with_environment(root.path(), environment());
    assert!(matches!(
        reopened,
        Err(BackendError::InvalidArtifact("dictionary-json"))
    ));
}
