use glyphshift_decision::{DecisionDiagnostic, DecisionEngine, DecisionState};
use glyphshift_domain::{
    FontDecision, Generation, RenderDecision, RouteLimits, RouteOperator, RouteProgram,
    TextDecision, TextObservation,
};
use glyphshift_translation::{FontPolicy, FontRule, TranslationSnapshot};

fn decide(
    observation: &TextObservation,
    route: &RouteProgram,
    snapshot: &TranslationSnapshot,
    font_policy: &FontPolicy,
    state: &mut DecisionState,
) -> glyphshift_decision::DecisionResult {
    DecisionEngine::new().decide(observation, route, snapshot, font_policy, state)
}

#[test]
fn dec_001_to_004_keep_text_and_font_decisions_independent() {
    let cases = [
        (
            TranslationSnapshot::empty(Generation::new(7)),
            FontPolicy::empty(),
            TextDecision::Keep,
            FontDecision::Keep,
        ),
        (
            TranslationSnapshot::empty(Generation::new(8)).with_entry("menu", "Open", "打开"),
            FontPolicy::empty(),
            TextDecision::Replace("打开".into()),
            FontDecision::Keep,
        ),
        (
            TranslationSnapshot::empty(Generation::new(9)),
            FontPolicy::empty().with_location("menu", "Example Sans CJK"),
            TextDecision::Keep,
            FontDecision::Substitute("Example Sans CJK".into()),
        ),
        (
            TranslationSnapshot::empty(Generation::new(10)).with_entry("menu", "Open", "打开"),
            FontPolicy::empty().with_location("menu", "Example Sans CJK"),
            TextDecision::Replace("打开".into()),
            FontDecision::Substitute("Example Sans CJK".into()),
        ),
    ];

    for (snapshot, font_policy, text, font) in cases {
        let result = decide(
            &TextObservation::new("example.synthetic.writeback", "Open", "surface-main"),
            &RouteProgram::direct("menu"),
            &snapshot,
            &font_policy,
            &mut DecisionState::new(),
        );

        assert_eq!(
            result.decision(),
            &RenderDecision {
                text,
                font,
                generation: snapshot.generation(),
            }
        );
        assert!(result.diagnostics().is_empty());
    }
}

#[test]
fn dictionary_default_font_is_blocked_by_an_unchanged_entry() {
    let snapshot =
        TranslationSnapshot::empty(Generation::new(18)).with_entry("menu", "Open", "打开");
    let font_policy = FontPolicy::empty()
        .with_location("menu", "Dictionary Default")
        .with_entry("menu", "Open", FontRule::Unchanged);
    let route = RouteProgram::direct("menu");
    let mut state = DecisionState::new();

    let decisions = ["Open", "Close"].map(|source| {
        decide(
            &TextObservation::new("adapter-a", source, "surface-main"),
            &route,
            &snapshot,
            &font_policy,
            &mut state,
        )
        .into_decision()
    });

    assert_eq!(
        decisions,
        [
            RenderDecision {
                text: TextDecision::Replace("打开".into()),
                font: FontDecision::Keep,
                generation: Generation::new(18),
            },
            RenderDecision {
                text: TextDecision::Keep,
                font: FontDecision::Substitute("Dictionary Default".into()),
                generation: Generation::new(18),
            },
        ]
    );
}

#[test]
fn adapter_scoped_font_override_only_applies_to_the_selected_hook_path() {
    let snapshot = TranslationSnapshot::empty(Generation::new(19));
    let font_policy = FontPolicy::empty()
        .with_location("menu", "Dictionary Default")
        .with_entry_for_adapters(
            "menu",
            "Open",
            FontRule::Substitute("Entry Override".into()),
            ["adapter-a"],
        );
    let route = RouteProgram::direct("menu");
    let mut state = DecisionState::new();

    let decisions = ["adapter-a", "adapter-b"].map(|adapter| {
        decide(
            &TextObservation::new(adapter, "Open", "surface-main"),
            &route,
            &snapshot,
            &font_policy,
            &mut state,
        )
        .into_decision()
        .font
    });

    assert_eq!(
        decisions,
        [
            FontDecision::Substitute("Entry Override".into()),
            FontDecision::Substitute("Dictionary Default".into()),
        ]
    );
}

#[test]
fn contextual_font_rules_share_the_translation_match_key() {
    let snapshot = TranslationSnapshot::empty(Generation::new(20))
        .with_context_entry("parameter", "tool", "blur", "Radius", "模糊半径")
        .with_context_entry("parameter", "tool", "sharpen", "Radius", "锐化半径");
    let font_policy = FontPolicy::empty()
        .with_location("parameter", "Dictionary Default")
        .with_context_entry("parameter", "tool", "blur", "Radius", FontRule::Unchanged)
        .with_context_entry(
            "parameter",
            "tool",
            "sharpen",
            "Radius",
            FontRule::Substitute("Sharp Override".into()),
        );
    let route = RouteProgram::new(
        [RouteOperator::contextual_heading("parameter", "tool")],
        RouteLimits::new(8, 16),
    );
    let mut state = DecisionState::new();

    for (surface, key, label) in [
        ("surface-blur", "blur", "Blur"),
        ("surface-sharpen", "sharpen", "Sharpen"),
    ] {
        decide(
            &TextObservation::new("adapter-a", label, surface)
                .with_context_heading("tool", key, label),
            &route,
            &snapshot,
            &font_policy,
            &mut state,
        );
    }

    let decisions = ["surface-blur", "surface-sharpen"].map(|surface| {
        decide(
            &TextObservation::new("adapter-a", "Radius", surface),
            &route,
            &snapshot,
            &font_policy,
            &mut state,
        )
        .into_decision()
    });

    assert_eq!(
        decisions,
        [
            RenderDecision {
                text: TextDecision::Replace("模糊半径".into()),
                font: FontDecision::Keep,
                generation: Generation::new(20),
            },
            RenderDecision {
                text: TextDecision::Replace("锐化半径".into()),
                font: FontDecision::Substitute("Sharp Override".into()),
                generation: Generation::new(20),
            },
        ]
    );
}

#[test]
fn font_policy_digest_changes_with_replacement_rules_not_insertion_order() {
    let first = FontPolicy::empty()
        .with_location("menu", "Dictionary Default")
        .with_entry("menu", "File", FontRule::Unchanged)
        .with_entry(
            "menu",
            "Open",
            FontRule::Substitute("Entry Override".into()),
        );
    let equivalent = FontPolicy::empty()
        .with_entry(
            "menu",
            "Open",
            FontRule::Substitute("Entry Override".into()),
        )
        .with_entry("menu", "File", FontRule::Unchanged)
        .with_location("menu", "Dictionary Default");
    let changed = equivalent.clone().with_entry(
        "menu",
        "Open",
        FontRule::Substitute("Another Override".into()),
    );

    assert_eq!(
        [
            first.digest() == equivalent.digest(),
            first.digest() == changed.digest()
        ],
        [true, false]
    );
}

#[test]
fn dec_005_routes_directly_to_the_declared_location() {
    let snapshot = TranslationSnapshot::empty(Generation::new(11))
        .with_entry("menu", "Open", "打开")
        .with_entry("panel", "Open", "开启面板");

    let result = decide(
        &TextObservation::new("adapter-a", "Open", "surface-main"),
        &RouteProgram::direct("panel"),
        &snapshot,
        &FontPolicy::empty(),
        &mut DecisionState::new(),
    );

    assert_eq!(
        result.decision().text,
        TextDecision::Replace("开启面板".into())
    );
}

#[test]
fn dec_006_uses_only_declared_fallback_locations_in_order() {
    let snapshot = TranslationSnapshot::empty(Generation::new(12))
        .with_entry("global", "Open", "不得命中")
        .with_entry("menu", "Open", "打开");
    let route = RouteProgram::new(
        [RouteOperator::fallback(["panel", "menu"])],
        RouteLimits::new(8, 16),
    );

    let result = decide(
        &TextObservation::new("adapter-a", "Open", "surface-main"),
        &route,
        &snapshot,
        &FontPolicy::empty(),
        &mut DecisionState::new(),
    );

    assert_eq!(result.decision().text, TextDecision::Replace("打开".into()));
}

#[test]
fn dec_007_and_009_use_heading_context_for_following_parameters() {
    let snapshot = TranslationSnapshot::empty(Generation::new(13))
        .with_context_entry("parameter", "tool", "blur", "Radius", "模糊半径")
        .with_context_entry("parameter", "tool", "sharpen", "Radius", "锐化半径");
    let route = RouteProgram::new(
        [RouteOperator::contextual_heading("parameter", "tool")],
        RouteLimits::new(8, 16),
    );
    let mut state = DecisionState::new();

    let heading = decide(
        &TextObservation::new("adapter-a", "Blur", "surface-main")
            .with_context_heading("tool", "blur", "Blur"),
        &route,
        &snapshot,
        &FontPolicy::empty(),
        &mut state,
    );
    assert_eq!(heading.decision().text, TextDecision::Keep);

    let parameter = decide(
        &TextObservation::new("adapter-a", "Radius", "surface-main"),
        &route,
        &snapshot,
        &FontPolicy::empty(),
        &mut state,
    );
    assert_eq!(
        parameter.decision().text,
        TextDecision::Replace("模糊半径".into())
    );
}

#[test]
fn dec_008_isolates_context_between_surfaces() {
    let snapshot = TranslationSnapshot::empty(Generation::new(14))
        .with_context_entry("parameter", "tool", "blur", "Radius", "模糊半径")
        .with_context_entry("parameter", "tool", "sharpen", "Radius", "锐化半径");
    let route = RouteProgram::new(
        [RouteOperator::contextual_heading("parameter", "tool")],
        RouteLimits::new(8, 16),
    );
    let mut state = DecisionState::new();

    for (surface, key, label) in [
        ("surface-a", "blur", "Blur"),
        ("surface-b", "sharpen", "Sharpen"),
    ] {
        decide(
            &TextObservation::new("adapter-a", label, surface)
                .with_context_heading("tool", key, label),
            &route,
            &snapshot,
            &FontPolicy::empty(),
            &mut state,
        );
    }

    let surface_a = decide(
        &TextObservation::new("adapter-a", "Radius", "surface-a"),
        &route,
        &snapshot,
        &FontPolicy::empty(),
        &mut state,
    );
    let surface_b = decide(
        &TextObservation::new("adapter-a", "Radius", "surface-b"),
        &route,
        &snapshot,
        &FontPolicy::empty(),
        &mut state,
    );
    assert_eq!(
        surface_a.decision().text,
        TextDecision::Replace("模糊半径".into())
    );
    assert_eq!(
        surface_b.decision().text,
        TextDecision::Replace("锐化半径".into())
    );
}

#[test]
fn dec_010_and_011_ignore_adapter_identity_and_location_labels() {
    let snapshot =
        TranslationSnapshot::empty(Generation::new(15)).with_entry("menu", "Open", "打开");
    let route = RouteProgram::direct("menu");
    let mut state = DecisionState::new();

    let adapter_a = decide(
        &TextObservation::new("adapter-a", "Open", "surface-main"),
        &route,
        &snapshot,
        &FontPolicy::empty(),
        &mut state,
    );
    let adapter_b = decide(
        &TextObservation::new("adapter-b", "Open", "surface-main"),
        &route,
        &snapshot,
        &FontPolicy::empty(),
        &mut state,
    );

    assert_eq!(adapter_a, adapter_b);
    assert_eq!(
        adapter_a.decision().text,
        TextDecision::Replace("打开".into())
    );
}

#[test]
fn adapter_scoped_translation_only_applies_to_the_selected_hook_path() {
    let snapshot = TranslationSnapshot::empty(Generation::new(16)).with_entry_for_adapters(
        "menu",
        "Open",
        "打开",
        ["adapter-a"],
    );
    let route = RouteProgram::direct("menu");
    let mut state = DecisionState::new();

    let selected = decide(
        &TextObservation::new("adapter-a", "Open", "surface-main"),
        &route,
        &snapshot,
        &FontPolicy::empty(),
        &mut state,
    );
    let excluded = decide(
        &TextObservation::new("adapter-b", "Open", "surface-main"),
        &route,
        &snapshot,
        &FontPolicy::empty(),
        &mut state,
    );

    assert_eq!(
        selected.decision().text,
        TextDecision::Replace("打开".into())
    );
    assert_eq!(excluded.decision().text, TextDecision::Keep);
}

#[test]
fn dec_012_rejects_invalid_or_excessive_observation_metadata() {
    let result = decide(
        &TextObservation::new("adapter-a", "x".repeat(16_385), "surface-main"),
        &RouteProgram::direct("menu"),
        &TranslationSnapshot::empty(Generation::new(16)),
        &FontPolicy::empty(),
        &mut DecisionState::new(),
    );

    assert_eq!(result.decision().text, TextDecision::Keep);
    assert_eq!(
        result.diagnostics(),
        &[DecisionDiagnostic::InvalidObservation]
    );
}

#[test]
fn dec_013_stops_at_execution_and_state_limits_with_a_diagnostic() {
    let snapshot =
        TranslationSnapshot::empty(Generation::new(17)).with_entry("menu", "Open", "打开");
    let execution_limited = RouteProgram::new(
        [RouteOperator::fallback(["panel", "menu"])],
        RouteLimits::new(8, 1),
    );
    let execution_result = decide(
        &TextObservation::new("adapter-a", "Open", "surface-main"),
        &execution_limited,
        &snapshot,
        &FontPolicy::empty(),
        &mut DecisionState::new(),
    );
    assert_eq!(execution_result.decision().text, TextDecision::Keep);
    assert_eq!(
        execution_result.diagnostics(),
        &[DecisionDiagnostic::ExecutionLimitExceeded]
    );

    let state_limited = RouteProgram::new(
        [RouteOperator::contextual_heading("parameter", "tool")],
        RouteLimits::new(1, 8),
    );
    let mut state = DecisionState::new();
    let first = decide(
        &TextObservation::new("adapter-a", "Blur", "surface-a")
            .with_context_heading("tool", "blur", "Blur"),
        &state_limited,
        &snapshot,
        &FontPolicy::empty(),
        &mut state,
    );
    assert!(first.diagnostics().is_empty());
    let second = decide(
        &TextObservation::new("adapter-a", "Sharpen", "surface-b")
            .with_context_heading("tool", "sharpen", "Sharpen"),
        &state_limited,
        &snapshot,
        &FontPolicy::empty(),
        &mut state,
    );
    assert_eq!(
        second.diagnostics(),
        &[DecisionDiagnostic::StateLimitExceeded]
    );
}
