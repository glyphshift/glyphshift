use glyphshift_domain::{
    AdapterId, FontDecision, Generation, RouteLimits, RouteOperator, RouteProgram, TextDecision,
    TextObservation,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_translation::{FontPolicy, FontRule, TranslationSnapshot};

#[test]
fn rtp_001_round_trips_route_translations_context_and_font_without_machine_data() {
    let publication = RuntimePublication::new(
        RouteProgram::new(
            [
                RouteOperator::contextual_heading("effects", "effect-name"),
                RouteOperator::fallback(["menu", "app"]),
            ],
            RouteLimits::new(24, 48),
        ),
        TranslationSnapshot::empty(Generation::new(7))
            .with_entry_for_adapters("menu", "File", "文件", ["example.synthetic.inline"])
            .with_context_entry("effects", "effect-name", "Color Range", "Fuzziness", "容差"),
        FontPolicy::empty().with_location("effects", "Example Sans CJK"),
    );

    let encoded = publication
        .encode_json()
        .expect("a valid runtime publication should encode");
    let decoded = RuntimePublication::decode_json(&encoded)
        .expect("an encoded runtime publication should decode");

    assert_eq!(decoded, publication);
    assert!(encoded.contains("glyphshift.runtime/2"));
    assert!(encoded.contains("adapter_ids"));
    assert!(!encoded.contains("target/local-test"));
    assert!(!encoded.contains("process_id"));
    assert!(!encoded.contains("driver"));
    let _domain_types_remain_host_independent = (
        AdapterId::new("example.synthetic.inline"),
        TextDecision::Keep,
        FontDecision::Keep,
        TextObservation::new("example.synthetic.inline", "File", "surface"),
    );
}

#[test]
fn rtp_002_rejects_unknown_schema_and_executable_route_operators() {
    let obsolete_schema = r#"{"schema":"glyphshift.runtime/1","generation":1,"route":{"max_state_entries":1,"max_steps":1,"operators":[]},"translations":[],"fonts":[]}"#;
    assert!(RuntimePublication::decode_json(obsolete_schema).is_err());

    let unknown_schema = r#"{"schema":"glyphshift.runtime/99","generation":1,"route":{"max_state_entries":1,"max_steps":1,"operators":[]},"translations":[],"fonts":[]}"#;
    assert!(RuntimePublication::decode_json(unknown_schema).is_err());

    let executable_route = RuntimePublication::new(
        RouteProgram::new([RouteOperator::script()], RouteLimits::new(1, 1)),
        TranslationSnapshot::empty(Generation::new(1)),
        FontPolicy::empty(),
    );
    assert!(executable_route.encode_json().is_err());
}

#[test]
fn runtime_publication_round_trips_default_and_entry_font_rules() {
    let publication = RuntimePublication::new(
        RouteProgram::direct("menu"),
        TranslationSnapshot::empty(Generation::new(8)),
        FontPolicy::empty()
            .with_location_for_adapters("menu", "Dictionary Default", ["example.synthetic.inline"])
            .with_entry("menu", "File", FontRule::Unchanged)
            .with_entry_for_adapters(
                "menu",
                "Open",
                FontRule::Substitute("Entry Override".into()),
                ["example.synthetic.inline"],
            )
            .with_context_entry_for_adapters(
                "effects",
                "effect-name",
                "Color Range",
                "Fuzziness",
                FontRule::Substitute("Context Override".into()),
                ["example.synthetic.inline"],
            ),
    );

    let encoded = publication
        .encode_json()
        .expect("font replacement rules should encode");
    let decoded =
        RuntimePublication::decode_json(&encoded).expect("font replacement rules should decode");

    assert_eq!(decoded, publication);
}
