use glyphshift_domain::{Feature, Generation, RouteProgram};
use glyphshift_workflow::{
    resolve, AdapterInput, AdapterPlan, CompositionDiagnostic, CompositionEnvironment, Dictionary,
    DictionaryEntry, FontProfile, FontProfileBinding, ResolveError, SoftwareInput, Workflow,
    WorkflowTarget,
};

#[test]
fn wf_001_composes_text_fonts_and_parallel_adapters_as_independent_assets() {
    let workflow = Workflow::new(
        "workflow-main",
        [WorkflowTarget::new(
            "software-editor",
            AdapterPlan::parallel(["adapter-gdi", "adapter-gdiplus"]),
            ["dictionary-zh-cn"],
        )
        .with_font_bindings([FontProfileBinding::all("font-profile-cjk")])],
    );
    let software = [SoftwareInput::new(
        "software-editor",
        "zh-CN",
        Generation::new(7),
        RouteProgram::direct("menu"),
        ["menu", "dialog"],
    )];
    let dictionaries = [Dictionary::new(
        "dictionary-zh-cn",
        "zh-CN",
        [DictionaryEntry::new("File", "文件")],
    )];
    let font_profiles = [FontProfile::new(
        "font-profile-cjk",
        ["Unavailable Sans", "Available Sans"],
    )];
    let environment = CompositionEnvironment::new(
        [
            AdapterInput::new(
                "adapter-gdi",
                [Feature::TextReplace, Feature::FontSubstitute],
            ),
            AdapterInput::new(
                "adapter-gdiplus",
                [Feature::TextReplace, Feature::FontSubstitute],
            ),
        ],
        ["Available Sans"],
    );
    let compiled = resolve(
        &workflow,
        &software,
        &dictionaries,
        &font_profiles,
        &environment,
    )
    .expect("independent assets should compose into one complete target intent");
    let target = &compiled.targets()[0];

    assert_eq!(
        target
            .snapshot()
            .lookup_for_adapter("dialog", "adapter-gdi", "File")
            .as_deref(),
        Some("文件"),
        "a pure dictionary applies to every internal route until Region Binding exists",
    );

    assert_eq!(
        (
            target.adapter_ids(),
            target.requested_features(),
            target
                .snapshot()
                .lookup_for_adapter("menu", "adapter-gdi", "File")
                .as_deref(),
            target
                .font_policy()
                .lookup_entry_for_adapter("menu", "adapter-gdiplus", "Edit"),
        ),
        (
            &[
                Box::<str>::from("adapter-gdi"),
                Box::<str>::from("adapter-gdiplus")
            ][..],
            &[Feature::TextReplace, Feature::FontSubstitute][..],
            Some("文件"),
            Some(glyphshift_translation::FontRule::Substitute(
                "Available Sans".into(),
            )),
        )
    );
}

#[test]
fn wf_004_allows_a_font_only_target_without_a_dictionary() {
    let compiled = resolve(
        &Workflow::new(
            "workflow-font-only",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-font"]),
                [] as [&str; 0],
            )
            .with_font_bindings([FontProfileBinding::locations(
                "font-profile-cjk",
                ["dialog"],
            )])],
        ),
        &[SoftwareInput::new(
            "software-editor",
            "zh-CN",
            Generation::new(10),
            RouteProgram::direct("dialog"),
            ["dialog"],
        )],
        &[],
        &[FontProfile::new("font-profile-cjk", ["Available Sans"])],
        &CompositionEnvironment::new(
            [AdapterInput::new("adapter-font", [Feature::FontSubstitute])],
            ["Available Sans"],
        ),
    )
    .expect("font bindings are independent from dictionaries");

    assert_eq!(
        compiled.targets()[0].requested_features(),
        &[Feature::FontSubstitute]
    );
}

#[test]
fn wf_005_composes_required_features_across_multiple_adapters() {
    let compiled = resolve(
        &Workflow::new(
            "workflow-split-capabilities",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-text", "adapter-font"]),
                ["dictionary-zh-cn"],
            )
            .with_font_bindings([FontProfileBinding::all("font-profile-cjk")])],
        ),
        &[SoftwareInput::new(
            "software-editor",
            "zh-CN",
            Generation::new(11),
            RouteProgram::direct("menu"),
            ["menu"],
        )],
        &[Dictionary::new(
            "dictionary-zh-cn",
            "zh-CN",
            [DictionaryEntry::new("File", "文件")],
        )],
        &[FontProfile::new("font-profile-cjk", ["Available Sans"])],
        &CompositionEnvironment::new(
            [
                AdapterInput::new("adapter-text", [Feature::TextReplace]),
                AdapterInput::new("adapter-font", [Feature::FontSubstitute]),
            ],
            ["Available Sans"],
        ),
    )
    .expect("parallel adapters may contribute different capabilities");

    assert_eq!(
        compiled.targets()[0].requested_features(),
        &[Feature::TextReplace, Feature::FontSubstitute]
    );
}

#[test]
fn wf_006_reports_dictionary_precedence_without_exposing_adapter_identity() {
    let compiled = resolve(
        &Workflow::new(
            "workflow-precedence",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-text"]),
                ["dictionary-first", "dictionary-second"],
            )],
        ),
        &[SoftwareInput::new(
            "software-editor",
            "zh-CN",
            Generation::new(12),
            RouteProgram::direct("menu"),
            ["menu"],
        )],
        &[
            Dictionary::new(
                "dictionary-first",
                "zh-CN",
                [DictionaryEntry::new("File", "文件")],
            ),
            Dictionary::new(
                "dictionary-second",
                "zh-CN",
                [DictionaryEntry::new("File", "档案")],
            ),
        ],
        &[],
        &CompositionEnvironment::new(
            [AdapterInput::new("adapter-text", [Feature::TextReplace])],
            [] as [&str; 0],
        ),
    )
    .expect("the first dictionary owns a duplicate semantic rule");

    assert_eq!(
        compiled.diagnostics(),
        &[CompositionDiagnostic::EntryConflict {
            software_id: "software-editor".into(),
            source: "File".into(),
            winning_dictionary_id: "dictionary-first".into(),
            shadowed_dictionary_id: "dictionary-second".into(),
        }]
    );
}

#[test]
fn wf_007_rejects_a_dictionary_target_locale_mismatch() {
    let result = resolve(
        &Workflow::new(
            "workflow-locale",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-text"]),
                ["dictionary-ja-jp"],
            )],
        ),
        &[SoftwareInput::new(
            "software-editor",
            "zh-CN",
            Generation::new(13),
            RouteProgram::direct("menu"),
            ["menu"],
        )],
        &[Dictionary::new(
            "dictionary-ja-jp",
            "ja-JP",
            [DictionaryEntry::new("File", "ファイル")],
        )],
        &[],
        &CompositionEnvironment::new(
            [AdapterInput::new("adapter-text", [Feature::TextReplace])],
            [] as [&str; 0],
        ),
    );

    assert_eq!(
        result,
        Err(ResolveError::LocaleMismatch {
            software_id: "software-editor".into(),
            dictionary_id: "dictionary-ja-jp".into(),
        })
    );
}

#[test]
fn wf_002_rejects_duplicate_adapter_bindings_before_compilation() {
    let workflow = Workflow::new(
        "workflow-duplicate-adapter",
        [WorkflowTarget::new(
            "software-editor",
            AdapterPlan::parallel(["adapter-gdi", "adapter-gdi"]),
            ["dictionary-zh-cn"],
        )],
    );
    let result = resolve(
        &workflow,
        &[SoftwareInput::new(
            "software-editor",
            "zh-CN",
            Generation::new(8),
            RouteProgram::direct("menu"),
            ["menu"],
        )],
        &[Dictionary::new(
            "dictionary-zh-cn",
            "zh-CN",
            [DictionaryEntry::new("File", "文件")],
        )],
        &[],
        &CompositionEnvironment::new(
            [AdapterInput::new("adapter-gdi", [Feature::TextReplace])],
            [] as [&str; 0],
        ),
    );

    assert_eq!(
        result,
        Err(ResolveError::DuplicateAdapter {
            software_id: "software-editor".into(),
            adapter_id: "adapter-gdi".into(),
        })
    );
}

#[test]
fn wf_003_rejects_an_empty_font_location_scope() {
    let workflow = Workflow::new(
        "workflow-empty-font-scope",
        [WorkflowTarget::new(
            "software-editor",
            AdapterPlan::parallel(["adapter-gdi"]),
            ["dictionary-zh-cn"],
        )
        .with_font_bindings([FontProfileBinding::locations(
            "font-profile-cjk",
            [] as [&str; 0],
        )])],
    );
    let result = resolve(
        &workflow,
        &[SoftwareInput::new(
            "software-editor",
            "zh-CN",
            Generation::new(9),
            RouteProgram::direct("menu"),
            ["menu"],
        )],
        &[Dictionary::new(
            "dictionary-zh-cn",
            "zh-CN",
            [DictionaryEntry::new("File", "文件")],
        )],
        &[FontProfile::new("font-profile-cjk", ["Available Sans"])],
        &CompositionEnvironment::new(
            [AdapterInput::new(
                "adapter-gdi",
                [Feature::TextReplace, Feature::FontSubstitute],
            )],
            ["Available Sans"],
        ),
    );

    assert_eq!(
        result,
        Err(ResolveError::EmptyFontScope {
            software_id: "software-editor".into(),
            font_profile_id: "font-profile-cjk".into(),
        })
    );
}

#[test]
fn wf_008_rejects_overlapping_font_profile_bindings() {
    let result = resolve(
        &Workflow::new(
            "workflow-overlapping-font-scopes",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-font"]),
                [] as [&str; 0],
            )
            .with_font_bindings([
                FontProfileBinding::locations("font-profile-primary", ["dialog"]),
                FontProfileBinding::locations("font-profile-secondary", ["dialog"]),
            ])],
        ),
        &[SoftwareInput::new(
            "software-editor",
            "zh-CN",
            Generation::new(14),
            RouteProgram::direct("dialog"),
            ["dialog"],
        )],
        &[],
        &[
            FontProfile::new("font-profile-primary", ["Available Sans"]),
            FontProfile::new("font-profile-secondary", ["Available Sans"]),
        ],
        &CompositionEnvironment::new(
            [AdapterInput::new("adapter-font", [Feature::FontSubstitute])],
            ["Available Sans"],
        ),
    );

    assert_eq!(
        result,
        Err(ResolveError::FontScopeConflict {
            software_id: "software-editor".into(),
            location: "dialog".into(),
        })
    );
}

#[test]
fn wf_009_rejects_a_plan_without_the_required_adapter_capability() {
    let result = resolve(
        &Workflow::new(
            "workflow-missing-font-capability",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-text-only"]),
                [] as [&str; 0],
            )
            .with_font_bindings([FontProfileBinding::all("font-profile-cjk")])],
        ),
        &[SoftwareInput::new(
            "software-editor",
            "zh-CN",
            Generation::new(15),
            RouteProgram::direct("dialog"),
            ["dialog"],
        )],
        &[],
        &[FontProfile::new("font-profile-cjk", ["Available Sans"])],
        &CompositionEnvironment::new(
            [AdapterInput::new(
                "adapter-text-only",
                [Feature::TextReplace],
            )],
            ["Available Sans"],
        ),
    );

    assert_eq!(
        result,
        Err(ResolveError::FeatureUnavailable {
            software_id: "software-editor".into(),
            feature: Feature::FontSubstitute,
        })
    );
}
