use glyphshift_adapter_registry::{AdapterRequirement, AdapterVersion, AdapterVersionRequirement};
use glyphshift_desktop_backend::{
    BackendError, DesktopBackend, DesktopEnvironment, DictionaryCreate, DictionaryRuleCreate,
    FontProfileBinding, FontProfileCreate, WorkflowCreate, WorkflowTargetCreate,
};
use glyphshift_domain::{AdapterId, Feature};
use std::fs;
use tempfile::tempdir;

fn environment() -> DesktopEnvironment {
    DesktopEnvironment::new(
        [AdapterRequirement::new(
            AdapterId::new("adapter-gdi"),
            AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
            [Feature::TextReplace, Feature::FontSubstitute],
        )],
        ["Available Sans"],
    )
}

#[test]
fn desktop_workflow_v2_persists_adapter_plan_and_font_binding_outside_the_dictionary() {
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
                .with_entries([DictionaryRuleCreate::replace("main-ui", "Open", "打开")]),
        )
        .expect("create pure dictionary");
    backend
        .create_font_profile(FontProfileCreate::new(
            "font-profile-ui",
            "UI Fonts",
            ["Available Sans"],
        ))
        .expect("create font profile");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow-ui", "UI Workflow").with_targets([
                WorkflowTargetCreate::new(software_id.clone(), ["adapter-gdi"], ["dictionary-ui"])
                    .with_font_bindings([FontProfileBinding::locations(
                        "font-profile-ui",
                        ["main-ui"],
                    )]),
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
    assert!(workflow_json.contains("glyphshift.workflow/2"));
    assert!(workflow_json.contains("adapterPlan"));
    assert!(workflow_json.contains("fontBindings"));

    let reopened = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("reopen composed product data");
    let workflow = reopened.workflow("workflow-ui").expect("workflow detail");
    assert_eq!(
        (
            workflow.targets()[0].adapter_plan().adapter_ids(),
            workflow.targets()[0].font_bindings()[0].font_profile_id(),
        ),
        (&[Box::<str>::from("adapter-gdi")][..], "font-profile-ui")
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
fn desktop_assets_keep_dictionary_text_and_font_profiles_independent_across_restart() {
    let root = tempdir().expect("temporary product data");
    let mut backend = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("open empty product data");

    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.interface-zh-cn", "界面汉化", "en", "zh-CN")
                .with_description("用于创作软件界面汉化")
                .with_release_version("0.1.0")
                .with_tags(["menu", "ui"])
                .with_entries([DictionaryRuleCreate::replace("main-ui", "Open", "打开")]),
        )
        .expect("create a pure dictionary");
    backend
        .create_font_profile(
            FontProfileCreate::new(
                "font-profile.cjk-ui",
                "简体中文界面",
                ["Unavailable Sans", "Available Sans"],
            )
            .with_description("跨平台中文字体候选"),
        )
        .expect("create an independent font profile");
    drop(backend);

    let reopened = DesktopBackend::open_with_environment(root.path(), environment())
        .expect("reopen current product data");
    let dictionary = reopened
        .dictionary("dictionary.interface-zh-cn")
        .expect("read dictionary by id");
    let font_profile = reopened
        .font_profile("font-profile.cjk-ui")
        .expect("read font profile by id");

    assert_eq!(
        (
            dictionary.metadata().name(),
            dictionary.metadata().source_locale(),
            dictionary.metadata().target_locale(),
            dictionary.metadata().release_version(),
            dictionary.metadata().tags(),
            dictionary.entries()[0].translation(),
            font_profile.metadata().name(),
            font_profile.families(),
            font_profile.resolved_family(),
        ),
        (
            "界面汉化",
            "en",
            "zh-CN",
            "0.1.0",
            &[Box::<str>::from("menu"), Box::<str>::from("ui")][..],
            Some("打开"),
            "简体中文界面",
            &[
                Box::<str>::from("Unavailable Sans"),
                Box::<str>::from("Available Sans"),
            ][..],
            Some("Available Sans"),
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
