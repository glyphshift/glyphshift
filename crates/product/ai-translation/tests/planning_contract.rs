use glyphshift_ai_translation::{
    AiTranslation, FilterPolicy, SkipReason, TranslationItem, TranslationPlanRequest,
};

#[test]
fn planner_selects_only_untranslated_meaningful_text_and_explains_every_skip() {
    let mut translation = AiTranslation::new();
    let request = TranslationPlanRequest::new(
        "dictionary.pending",
        7,
        "en-US",
        "zh-CN",
        [
            TranslationItem::untranslated("help", "&Help"),
            TranslationItem::untranslated("zero", "0"),
            TranslationItem::untranslated("counter", "0/4"),
            TranslationItem::untranslated("size", "1920 x 1080"),
            TranslationItem::untranslated("rate", "25.00fps"),
            TranslationItem::untranslated("letter", "A"),
            TranslationItem::untranslated("save", "Save"),
            TranslationItem::translated("open", "Open", "打开"),
            TranslationItem::untranslated("ignored", "Hidden").ignored(),
        ],
    )
    .with_filter_policy(FilterPolicy::default());

    let plan = translation
        .plan_translation(request)
        .expect("plan translation");

    assert_eq!(plan.snapshot_revision(), 7);
    assert_eq!(
        plan.candidates()
            .iter()
            .map(|candidate| candidate.source())
            .collect::<Vec<_>>(),
        vec!["&Help", "Save"]
    );
    assert_eq!(
        plan.skip_reason("zero"),
        Some(SkipReason::PureNumberOrSymbols)
    );
    assert_eq!(
        plan.skip_reason("counter"),
        Some(SkipReason::NumericMeasurement)
    );
    assert_eq!(
        plan.skip_reason("size"),
        Some(SkipReason::NumericMeasurement)
    );
    assert_eq!(
        plan.skip_reason("rate"),
        Some(SkipReason::NumericMeasurement)
    );
    assert_eq!(
        plan.skip_reason("letter"),
        Some(SkipReason::SingleCharacter)
    );
    assert_eq!(
        plan.skip_reason("open"),
        Some(SkipReason::AlreadyTranslated)
    );
    assert_eq!(plan.skip_reason("ignored"), Some(SkipReason::Ignored));
    assert_eq!((plan.eligible_count(), plan.skipped_count()), (2, 7));
}

#[test]
fn containing_digits_is_an_opt_in_filter_not_a_hidden_default() {
    let mut translation = AiTranslation::new();
    let items = [TranslationItem::untranslated("version", "Version 2")];

    let default_plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            1,
            "en-US",
            "zh-CN",
            items.clone(),
        ))
        .expect("default plan");
    let strict_plan = translation
        .plan_translation(
            TranslationPlanRequest::new("dictionary.pending", 1, "en-US", "zh-CN", items)
                .with_filter_policy(FilterPolicy::default().with_skip_text_containing_digits(true)),
        )
        .expect("strict plan");

    assert_eq!(default_plan.eligible_count(), 1);
    assert_eq!(
        strict_plan.skip_reason("version"),
        Some(SkipReason::ContainsDigit)
    );
}

#[test]
fn planner_filters_common_non_translatable_formats_and_protects_format_tokens() {
    let mut translation = AiTranslation::new();
    let policy = FilterPolicy::default()
        .with_max_source_chars(Some(24))
        .with_excluded_patterns([r"(?i)^debug_"]);
    let plan = translation
        .plan_translation(
            TranslationPlanRequest::new(
                "probe.synthetic",
                9,
                "en-US",
                "zh-CN",
                [
                    TranslationItem::untranslated("url", "https://example.invalid/guide"),
                    TranslationItem::untranslated("email", "person@example.invalid"),
                    TranslationItem::untranslated("path", "assets/ui/file.json"),
                    TranslationItem::untranslated("shortcut", "Ctrl+Shift+S"),
                    TranslationItem::untranslated("long", "This source is longer than allowed"),
                    TranslationItem::untranslated("custom", "DEBUG_INTERNAL"),
                    TranslationItem::untranslated("first-save", "Save"),
                    TranslationItem::untranslated("duplicate-save", "Save"),
                    TranslationItem::untranslated("formatted", "File {count} of %1"),
                ],
            )
            .with_filter_policy(policy),
        )
        .expect("plan filtered translation");

    assert_eq!(plan.skip_reason("url"), Some(SkipReason::Url));
    assert_eq!(plan.skip_reason("email"), Some(SkipReason::Email));
    assert_eq!(plan.skip_reason("path"), Some(SkipReason::FilePath));
    assert_eq!(plan.skip_reason("shortcut"), Some(SkipReason::Shortcut));
    assert_eq!(plan.skip_reason("long"), Some(SkipReason::TooLong));
    assert_eq!(plan.skip_reason("custom"), Some(SkipReason::CustomPattern));
    assert_eq!(
        plan.skip_reason("duplicate-save"),
        Some(SkipReason::DuplicateSource)
    );
    assert_eq!(plan.skip_reason("formatted"), None);
    let formatted = plan
        .candidates()
        .iter()
        .find(|candidate| candidate.item_id() == "formatted")
        .expect("formatted candidate");
    assert_eq!(
        formatted
            .protected_tokens()
            .iter()
            .map(Box::as_ref)
            .collect::<Vec<_>>(),
        vec!["{count}", "%1"]
    );
}
