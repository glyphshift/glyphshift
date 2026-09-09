use glyphshift_domain::{Feature, Generation, RouteProgram};
use glyphshift_translation::FontRule;
use glyphshift_workflow::{
    resolve, AdapterInput, AdapterPlan, CompositionDiagnostic, CompositionEnvironment, Dictionary,
    DictionaryEntry, FontCoverage, ResolveError, SoftwareInput, TargetFontPolicy, Workflow,
    WorkflowTarget,
};

fn software(generation: u64) -> SoftwareInput {
    SoftwareInput::new(
        "software-editor",
        "zh-CN",
        Generation::new(generation),
        RouteProgram::direct("internal-default"),
        ["internal-default"],
    )
}

fn dictionary() -> Dictionary {
    Dictionary::new(
        "dictionary-zh-cn",
        "zh-CN",
        [DictionaryEntry::new("File", "文件")],
    )
}

#[test]
fn wf_001_dictionary_match_font_policy_keeps_untranslated_text_unchanged() {
    let compiled = resolve(
        &Workflow::new(
            "workflow-main",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-gdi", "adapter-observe"]),
                ["dictionary-zh-cn"],
            )
            .with_font_policy(TargetFontPolicy::new(
                ["Unavailable Sans", "Available Sans"],
                FontCoverage::DictionaryMatches,
            ))],
        ),
        &[software(7)],
        &[dictionary()],
        &CompositionEnvironment::new(
            [
                AdapterInput::new(
                    "adapter-gdi",
                    [Feature::TextReplace, Feature::FontSubstitute],
                ),
                AdapterInput::new("adapter-observe", [Feature::TextReplace]),
            ],
            ["Available Sans"],
        ),
    )
    .expect("inline font policy should compile");
    let target = &compiled.targets()[0];

    assert_eq!(
        target
            .font_policy()
            .lookup_entry_for_adapter("internal-default", "adapter-gdi", "File"),
        Some(FontRule::Substitute("Available Sans".into()))
    );
    assert_eq!(
        target
            .font_policy()
            .lookup_entry_for_adapter("internal-default", "adapter-gdi", "Edit"),
        None,
        "untranslated text keeps its original font by default",
    );
    assert_eq!(
        target.font_policy().lookup_entry_for_adapter(
            "internal-default",
            "adapter-observe",
            "File"
        ),
        None,
        "font rules only target adapters that declare FontSubstitute",
    );
}

#[test]
fn wf_002_all_observations_supports_a_font_only_target() {
    let compiled = resolve(
        &Workflow::new(
            "workflow-font-only",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-font"]),
                [] as [&str; 0],
            )
            .with_font_policy(TargetFontPolicy::new(
                ["Available Sans"],
                FontCoverage::AllObservations,
            ))],
        ),
        &[software(10)],
        &[],
        &CompositionEnvironment::new(
            [AdapterInput::new("adapter-font", [Feature::FontSubstitute])],
            ["Available Sans"],
        ),
    )
    .expect("all-observations coverage may run without a dictionary");
    let target = &compiled.targets()[0];

    assert_eq!(target.requested_features(), &[Feature::FontSubstitute]);
    assert_eq!(
        target.font_policy().lookup_entry_for_adapter(
            "internal-default",
            "adapter-font",
            "Anything"
        ),
        Some(FontRule::Substitute("Available Sans".into()))
    );
}

#[test]
fn wf_003_composes_text_and_font_capabilities_across_selected_adapters() {
    let compiled = resolve(
        &Workflow::new(
            "workflow-split-capabilities",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-text", "adapter-font"]),
                ["dictionary-zh-cn"],
            )
            .with_font_policy(TargetFontPolicy::new(
                ["Available Sans"],
                FontCoverage::DictionaryMatches,
            ))],
        ),
        &[software(11)],
        &[dictionary()],
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
fn wf_004_reports_dictionary_precedence_without_exposing_adapter_identity() {
    let compiled = resolve(
        &Workflow::new(
            "workflow-precedence",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-text"]),
                ["dictionary-first", "dictionary-second"],
            )],
        ),
        &[software(12)],
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
fn wf_005_rejects_a_dictionary_target_locale_mismatch() {
    let result = resolve(
        &Workflow::new(
            "workflow-locale",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-text"]),
                ["dictionary-ja-jp"],
            )],
        ),
        &[software(13)],
        &[Dictionary::new(
            "dictionary-ja-jp",
            "ja-JP",
            [DictionaryEntry::new("File", "ファイル")],
        )],
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
fn wf_006_rejects_duplicate_adapter_bindings_before_compilation() {
    let result = resolve(
        &Workflow::new(
            "workflow-duplicate-adapter",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-gdi", "adapter-gdi"]),
                ["dictionary-zh-cn"],
            )],
        ),
        &[software(8)],
        &[dictionary()],
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
fn wf_007_rejects_an_empty_font_family_chain() {
    let result = resolve(
        &Workflow::new(
            "workflow-empty-fonts",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-gdi"]),
                ["dictionary-zh-cn"],
            )
            .with_font_policy(TargetFontPolicy::new(
                [] as [&str; 0],
                FontCoverage::DictionaryMatches,
            ))],
        ),
        &[software(9)],
        &[dictionary()],
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
        Err(ResolveError::EmptyFontFamilies {
            software_id: "software-editor".into(),
        })
    );
}

#[test]
fn wf_008_dictionary_match_font_policy_without_entries_has_no_effective_rules() {
    let result = resolve(
        &Workflow::new(
            "workflow-empty-dictionary-fonts",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-gdi"]),
                ["dictionary-empty"],
            )
            .with_font_policy(TargetFontPolicy::new(
                ["Available Sans"],
                FontCoverage::DictionaryMatches,
            ))],
        ),
        &[software(14)],
        &[Dictionary::new("dictionary-empty", "zh-CN", [])],
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
        Err(ResolveError::NoEffectiveRules {
            software_id: "software-editor".into(),
        })
    );
}

#[test]
fn wf_009_rejects_a_plan_without_font_substitution_capability() {
    let result = resolve(
        &Workflow::new(
            "workflow-missing-font-capability",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["adapter-text-only"]),
                ["dictionary-zh-cn"],
            )
            .with_font_policy(TargetFontPolicy::new(
                ["Available Sans"],
                FontCoverage::DictionaryMatches,
            ))],
        ),
        &[software(15)],
        &[dictionary()],
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

#[test]
fn dictionary_fonts_follow_translation_precedence_and_workflow_mode() {
    let dictionaries = [
        Dictionary::new("first", "zh-CN", [DictionaryEntry::new("File", "First")])
            .with_font_families(["Dictionary Sans"]),
        Dictionary::new(
            "second",
            "zh-CN",
            [
                DictionaryEntry::new("File", "Shadowed"),
                DictionaryEntry::new("Edit", "Default"),
            ],
        ),
    ];
    let environment = CompositionEnvironment::new(
        [
            AdapterInput::new("font", [Feature::TextReplace, Feature::FontSubstitute]),
            AdapterInput::new("text", [Feature::TextReplace]),
        ],
        ["Dictionary Sans", "Workflow Sans"],
    );
    for prefer in [false, true] {
        let result = resolve(
            &Workflow::new(
                "workflow",
                [WorkflowTarget::new(
                    "software-editor",
                    AdapterPlan::parallel(["font", "text"]),
                    ["first", "second"],
                )
                .with_font_policy(
                    TargetFontPolicy::new(["Workflow Sans"], FontCoverage::AllObservations)
                        .with_dictionary_fonts(prefer),
                )],
            ),
            &[software(1)],
            &dictionaries,
            &environment,
        )
        .unwrap();
        let policy = result.targets()[0].font_policy();
        assert_eq!(
            policy.lookup_entry_for_adapter("internal-default", "font", "File"),
            Some(FontRule::Substitute(
                if prefer {
                    "Dictionary Sans"
                } else {
                    "Workflow Sans"
                }
                .into()
            ))
        );
        assert_eq!(
            policy.lookup_entry_for_adapter("internal-default", "font", "Edit"),
            Some(FontRule::Substitute("Workflow Sans".into()))
        );
        assert_eq!(
            policy.lookup_entry_for_adapter("internal-default", "text", "File"),
            None
        );
    }
    let result = resolve(
        &Workflow::new(
            "workflow",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["font"]),
                ["first", "second"],
            )],
        ),
        &[software(1)],
        &dictionaries,
        &environment,
    )
    .unwrap();
    assert_eq!(
        result.targets()[0].font_policy().lookup_entry_for_adapter(
            "internal-default",
            "font",
            "File"
        ),
        None
    );
}

#[test]
fn dictionary_fonts_can_be_used_without_a_workflow_default() {
    let result = resolve(
        &Workflow::new(
            "workflow",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["font"]),
                ["first"],
            )
            .with_font_policy(
                TargetFontPolicy::new(Vec::<Box<str>>::new(), FontCoverage::DictionaryMatches)
                    .with_dictionary_fonts(true),
            )],
        ),
        &[software(1)],
        &[
            Dictionary::new("first", "zh-CN", [DictionaryEntry::new("File", "First")])
                .with_font_families(["Dictionary Sans"]),
        ],
        &CompositionEnvironment::new(
            [AdapterInput::new(
                "font",
                [Feature::TextReplace, Feature::FontSubstitute],
            )],
            ["Dictionary Sans"],
        ),
    )
    .unwrap();
    assert_eq!(
        result.targets()[0].font_policy().lookup_entry_for_adapter(
            "internal-default",
            "font",
            "File"
        ),
        Some(FontRule::Substitute("Dictionary Sans".into()))
    );
}

#[test]
fn font_scaling_inherits_independently_and_is_scoped_to_capable_adapters() {
    let compiled = resolve(
        &Workflow::new(
            "workflow",
            [WorkflowTarget::new(
                "software-editor",
                AdapterPlan::parallel(["scale", "family"]),
                ["dictionary"],
            )
            .with_font_policy(
                TargetFontPolicy::new(["Sans"], FontCoverage::AllObservations)
                    .with_dictionary_fonts(true)
                    .with_scale_percent(150),
            )],
        ),
        &[software(1)],
        &[Dictionary::new(
            "dictionary",
            "zh-CN",
            [DictionaryEntry::new("File", "文件")],
        )
        .with_font_scale_percent(Some(100))],
        &CompositionEnvironment::new(
            [
                AdapterInput::new("scale", [Feature::TextReplace, Feature::FontScale]),
                AdapterInput::new("family", [Feature::TextReplace, Feature::FontSubstitute]),
            ],
            ["Sans"],
        ),
    )
    .unwrap();
    let policy = compiled.targets()[0].font_policy();
    assert_eq!(
        policy.lookup_entry_for_adapter("internal-default", "scale", "Unmatched"),
        Some(FontRule::Scaled {
            family: None,
            percent: 150
        })
    );
    assert_eq!(
        policy.lookup_entry_for_adapter("internal-default", "scale", "File"),
        Some(FontRule::Unchanged)
    );
    assert_eq!(
        policy.lookup_entry_for_adapter("internal-default", "family", "File"),
        Some(FontRule::Substitute("Sans".into()))
    );
    assert!(compiled.targets()[0]
        .requested_features()
        .contains(&Feature::FontScale));
}
