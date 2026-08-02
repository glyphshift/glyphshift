use glyphshift_domain::{Feature, Generation, RouteProgram};
use glyphshift_translation::FontPolicy;
use glyphshift_workflow::{
    resolve, CompositionDiagnostic, DefaultFontBehavior, Dictionary, DictionaryEntry,
    EntryFontBehavior, ResolveError, SoftwareInput, Workflow, WorkflowTarget,
};

#[test]
fn wf_001_one_software_and_dictionary_compile_to_text_only() {
    let workflow = Workflow::new(
        "workflow-main",
        [WorkflowTarget::new("software-editor", ["dictionary-zh-cn"])],
    );
    let software = [SoftwareInput::new(
        "software-editor",
        "zh-CN",
        Generation::new(7),
        RouteProgram::direct("menu"),
        ["menu"],
    )];
    let dictionaries = [Dictionary::new(
        "dictionary-zh-cn",
        "zh-CN",
        [DictionaryEntry::replace("menu", "File", "文件")],
    )];

    let compiled = resolve(&workflow, &software, &dictionaries)
        .expect("one valid target should compile completely");
    let target = compiled
        .targets()
        .first()
        .expect("the compiled workflow should contain its target");

    assert_eq!(
        (
            compiled.id(),
            target.software_id(),
            target.generation(),
            target.route(),
            target.requested_features(),
            target.snapshot().lookup("menu", "File").as_deref(),
            target.font_policy(),
        ),
        (
            "workflow-main",
            "software-editor",
            Generation::new(7),
            &RouteProgram::direct("menu"),
            &[Feature::TextReplace][..],
            Some("文件"),
            &FontPolicy::empty(),
        )
    );
}

#[test]
fn wf_002_entry_font_override_compiles_to_text_and_font() {
    let workflow = Workflow::new(
        "workflow-main",
        [WorkflowTarget::new("software-editor", ["dictionary-zh-cn"])],
    );
    let software = [SoftwareInput::new(
        "software-editor",
        "zh-CN",
        Generation::new(8),
        RouteProgram::direct("menu"),
        ["menu"],
    )];
    let dictionaries = [Dictionary::new(
        "dictionary-zh-cn",
        "zh-CN",
        [DictionaryEntry::replace("menu", "File", "文件")
            .with_font(EntryFontBehavior::substitute("Entry Font"))],
    )];

    let compiled = resolve(&workflow, &software, &dictionaries)
        .expect("a text and font rule should compile completely");
    let target = &compiled.targets()[0];

    assert_eq!(
        (target.requested_features(), target.font_policy()),
        (
            &[Feature::TextReplace, Feature::FontSubstitute][..],
            &FontPolicy::empty().with_entry(
                "menu",
                "File",
                glyphshift_translation::FontRule::Substitute("Entry Font".into()),
            ),
        )
    );
}

#[test]
fn wf_003_unchanged_entry_blocks_the_dictionary_default_font() {
    let workflow = Workflow::new(
        "workflow-main",
        [WorkflowTarget::new("software-editor", ["dictionary-zh-cn"])],
    );
    let software = [SoftwareInput::new(
        "software-editor",
        "zh-CN",
        Generation::new(9),
        RouteProgram::direct("menu"),
        ["menu"],
    )];
    let dictionaries =
        [Dictionary::new(
            "dictionary-zh-cn",
            "zh-CN",
            [DictionaryEntry::replace("menu", "File", "文件")
                .with_font(EntryFontBehavior::Unchanged)],
        )
        .with_default_font(DefaultFontBehavior::substitute("Dictionary Default"))];

    let compiled = resolve(&workflow, &software, &dictionaries)
        .expect("a dictionary default and unchanged entry should compile");
    let target = &compiled.targets()[0];

    assert_eq!(
        (
            target.requested_features(),
            target
                .font_policy()
                .lookup_entry_for_adapter("menu", "adapter-a", "File"),
            target
                .font_policy()
                .lookup_entry_for_adapter("menu", "adapter-a", "Edit"),
        ),
        (
            &[Feature::TextReplace, Feature::FontSubstitute][..],
            Some(glyphshift_translation::FontRule::Unchanged),
            Some(glyphshift_translation::FontRule::Substitute(
                "Dictionary Default".into()
            )),
        )
    );
}

#[test]
fn dictionary_hook_scope_applies_once_to_text_and_default_font_rules() {
    let workflow = Workflow::new(
        "workflow-main",
        [WorkflowTarget::new("software-editor", ["dictionary-zh-cn"])],
    );
    let software = [SoftwareInput::new(
        "software-editor",
        "zh-CN",
        Generation::new(10),
        RouteProgram::direct("menu"),
        ["menu"],
    )];
    let dictionaries = [Dictionary::new(
        "dictionary-zh-cn",
        "zh-CN",
        [DictionaryEntry::replace("menu", "File", "文件")],
    )
    .with_default_font(DefaultFontBehavior::substitute("Dictionary Default"))
    .for_adapters(["adapter-gdi"])];

    let compiled = resolve(&workflow, &software, &dictionaries)
        .expect("a dictionary-level hook scope should compile");
    let target = &compiled.targets()[0];

    assert_eq!(
        target
            .snapshot()
            .lookup_for_adapter("menu", "adapter-gdi", "File")
            .as_deref(),
        Some("文件")
    );
    assert!(target
        .snapshot()
        .lookup_for_adapter("menu", "adapter-gdiplus", "File")
        .is_none());
    assert_eq!(
        target
            .font_policy()
            .lookup_entry_for_adapter("menu", "adapter-gdi", "Edit"),
        Some(glyphshift_translation::FontRule::Substitute(
            "Dictionary Default".into(),
        ))
    );
    assert!(target
        .font_policy()
        .lookup_entry_for_adapter("menu", "adapter-gdiplus", "Edit")
        .is_none());
}

#[test]
fn wf_004_higher_priority_dictionary_wins_and_reports_the_conflict_sources() {
    let workflow = Workflow::new(
        "workflow-main",
        [WorkflowTarget::new(
            "software-editor",
            ["dictionary-high", "dictionary-low"],
        )],
    );
    let software = [SoftwareInput::new(
        "software-editor",
        "zh-CN",
        Generation::new(10),
        RouteProgram::direct("menu"),
        ["menu"],
    )];
    let dictionaries = [
        Dictionary::new(
            "dictionary-high",
            "zh-CN",
            [DictionaryEntry::replace("menu", "File", "高优先级")],
        ),
        Dictionary::new(
            "dictionary-low",
            "zh-CN",
            [DictionaryEntry::replace("menu", "File", "低优先级")],
        ),
    ];

    let compiled = resolve(&workflow, &software, &dictionaries)
        .expect("conflicting valid dictionaries should compile deterministically");

    assert_eq!(
        (
            compiled.targets()[0]
                .snapshot()
                .lookup("menu", "File")
                .as_deref(),
            compiled.diagnostics(),
        ),
        (
            Some("高优先级"),
            &[CompositionDiagnostic::RuleConflict {
                software_id: "software-editor".into(),
                location: "menu".into(),
                context_kind: None,
                context_key: None,
                source: "File".into(),
                adapter_ids: Default::default(),
                winning_dictionary_id: "dictionary-high".into(),
                shadowed_dictionary_id: "dictionary-low".into(),
            }][..],
        )
    );
}

#[test]
fn wf_005_reuses_a_dictionary_and_rejects_an_unknown_target_location() {
    let shared_dictionary = Dictionary::new(
        "dictionary-shared",
        "zh-CN",
        [DictionaryEntry::replace("menu", "File", "文件")],
    );
    let workflow = Workflow::new(
        "workflow-shared",
        [
            WorkflowTarget::new("software-a", ["dictionary-shared"]),
            WorkflowTarget::new("software-b", ["dictionary-shared"]),
        ],
    );
    let software = [
        SoftwareInput::new(
            "software-a",
            "zh-CN",
            Generation::new(11),
            RouteProgram::direct("menu"),
            ["menu"],
        ),
        SoftwareInput::new(
            "software-b",
            "zh-CN",
            Generation::new(12),
            RouteProgram::direct("menu"),
            ["menu"],
        ),
    ];
    let compiled = resolve(
        &workflow,
        &software,
        std::slice::from_ref(&shared_dictionary),
    )
    .expect("one dictionary should compile independently for both software targets");

    let invalid = resolve(
        &Workflow::new(
            "workflow-invalid",
            [WorkflowTarget::new(
                "software-invalid",
                ["dictionary-shared"],
            )],
        ),
        &[SoftwareInput::new(
            "software-invalid",
            "zh-CN",
            Generation::new(13),
            RouteProgram::direct("panel"),
            ["panel"],
        )],
        &[shared_dictionary],
    );

    assert_eq!(
        (
            compiled
                .targets()
                .iter()
                .map(|target| (
                    target.software_id(),
                    target.generation(),
                    target.snapshot().lookup("menu", "File")
                ))
                .collect::<Vec<_>>(),
            invalid,
        ),
        (
            vec![
                ("software-a", Generation::new(11), Some("文件".into()),),
                ("software-b", Generation::new(12), Some("文件".into()),),
            ],
            Err(ResolveError::UnknownLocation {
                software_id: "software-invalid".into(),
                dictionary_id: "dictionary-shared".into(),
                location: "menu".into(),
            }),
        )
    );
}

#[test]
fn wf_006_rejects_incomplete_targets_without_a_partial_intent() {
    let software = [SoftwareInput::new(
        "software-editor",
        "zh-CN",
        Generation::new(14),
        RouteProgram::direct("menu"),
        ["menu"],
    )];
    let valid_dictionary = Dictionary::new(
        "dictionary-valid",
        "zh-CN",
        [DictionaryEntry::replace("menu", "File", "文件")],
    );
    let empty_dictionary = Dictionary::new("dictionary-empty", "zh-CN", []);
    let wrong_locale = Dictionary::new(
        "dictionary-en",
        "en-US",
        [DictionaryEntry::replace("menu", "File", "File")],
    );

    let results = [
        resolve(
            &Workflow::new(
                "workflow-empty-target",
                [WorkflowTarget::new("software-editor", [] as [&str; 0])],
            ),
            &software,
            std::slice::from_ref(&valid_dictionary),
        ),
        resolve(
            &Workflow::new(
                "workflow-missing-dictionary",
                [WorkflowTarget::new(
                    "software-editor",
                    ["dictionary-missing"],
                )],
            ),
            &software,
            std::slice::from_ref(&valid_dictionary),
        ),
        resolve(
            &Workflow::new(
                "workflow-wrong-locale",
                [WorkflowTarget::new("software-editor", ["dictionary-en"])],
            ),
            &software,
            std::slice::from_ref(&wrong_locale),
        ),
        resolve(
            &Workflow::new(
                "workflow-no-rules",
                [WorkflowTarget::new("software-editor", ["dictionary-empty"])],
            ),
            &software,
            std::slice::from_ref(&empty_dictionary),
        ),
    ];

    assert_eq!(
        results,
        [
            Err(ResolveError::EmptyTarget {
                software_id: "software-editor".into(),
            }),
            Err(ResolveError::UnknownDictionary("dictionary-missing".into())),
            Err(ResolveError::LocaleMismatch {
                software_id: "software-editor".into(),
                dictionary_id: "dictionary-en".into(),
            }),
            Err(ResolveError::NoEffectiveRules {
                software_id: "software-editor".into(),
            }),
        ]
    );
}

#[test]
fn workflow_preserves_context_and_adapter_scope_for_text_and_font() {
    let workflow = Workflow::new(
        "workflow-context",
        [WorkflowTarget::new(
            "software-editor",
            ["dictionary-context"],
        )],
    );
    let software = [SoftwareInput::new(
        "software-editor",
        "zh-CN",
        Generation::new(15),
        RouteProgram::direct("parameter"),
        ["parameter"],
    )];
    let dictionaries = [Dictionary::new(
        "dictionary-context",
        "zh-CN",
        [
            DictionaryEntry::replace("parameter", "Radius", "模糊半径")
                .with_context("tool", "blur")
                .for_adapters(["adapter-a"])
                .with_font(EntryFontBehavior::substitute("Blur Font")),
            DictionaryEntry::replace("parameter", "Radius", "锐化半径")
                .with_context("tool", "sharpen")
                .for_adapters(["adapter-b"]),
        ],
    )];

    let compiled = resolve(&workflow, &software, &dictionaries)
        .expect("different context and adapter keys should compile independently");
    let target = &compiled.targets()[0];

    assert_eq!(
        (
            target.snapshot().lookup_context_for_adapter(
                "parameter",
                "tool",
                "blur",
                "adapter-a",
                "Radius",
            ),
            target.snapshot().lookup_context_for_adapter(
                "parameter",
                "tool",
                "blur",
                "adapter-b",
                "Radius",
            ),
            target.font_policy().lookup_context_entry_for_adapter(
                "parameter",
                "tool",
                "blur",
                "adapter-a",
                "Radius",
            ),
            compiled.diagnostics(),
        ),
        (
            Some("模糊半径".into()),
            None,
            Some(glyphshift_translation::FontRule::Substitute(
                "Blur Font".into()
            )),
            &[][..],
        )
    );
}

#[test]
fn keep_text_with_a_font_override_compiles_to_font_only() {
    let workflow = Workflow::new(
        "workflow-font-only",
        [WorkflowTarget::new(
            "software-editor",
            ["dictionary-font-only"],
        )],
    );
    let software = [SoftwareInput::new(
        "software-editor",
        "zh-CN",
        Generation::new(16),
        RouteProgram::direct("menu"),
        ["menu"],
    )];
    let dictionaries = [Dictionary::new(
        "dictionary-font-only",
        "zh-CN",
        [DictionaryEntry::keep("menu", "File")
            .with_font(EntryFontBehavior::substitute("Entry Font"))],
    )];

    let compiled = resolve(&workflow, &software, &dictionaries)
        .expect("a keep-text font rule should be an effective target intent");
    let target = &compiled.targets()[0];

    assert_eq!(
        (
            target.requested_features(),
            target.snapshot().lookup("menu", "File"),
            target
                .font_policy()
                .lookup_entry_for_adapter("menu", "adapter-a", "File"),
        ),
        (
            &[Feature::FontSubstitute][..],
            None,
            Some(glyphshift_translation::FontRule::Substitute(
                "Entry Font".into()
            )),
        )
    );
}
