use glyphshift_decision::{DecisionEngine, DecisionState};
use glyphshift_domain::{Generation, RouteProgram, TextDecision, TextObservation};
use glyphshift_translation::{FontPolicy, RegexTranslationRule, RegexTranslationRules, TranslationSnapshot, test_regex_rule};
fn rule() -> RegexTranslationRule { RegexTranslationRule { pattern: r"^(.+?)(:[ \t]*[0-9]+)$".into(), replacement: "{{TR}}$2".into(), enabled: true } }
fn rules() -> RegexTranslationRules { RegexTranslationRules::compile(vec![rule()]).unwrap() }
fn snapshot() -> TranslationSnapshot { TranslationSnapshot::empty(Generation::new(1)).with_entry("first", "Total", "总计").with_entry("ordinary", "Total:18", "旧译文").with_dictionary_rules("first", rules()) }
fn translate(source: &str, snap: &TranslationSnapshot) -> TextDecision {
    DecisionEngine::new().decide(&TextObservation::new("synthetic", source, "surface"), &RouteProgram::direct("ordinary"), snap, &FontPolicy::empty(), &mut DecisionState::new()).into_decision().text
}
#[test]
fn dictionary_rule_precedes_exact_and_preserves_every_count() {
    for n in 0..101 { assert_eq!(translate(&format!("Total:{n}"), &snapshot()), TextDecision::Replace(format!("总计:{n}").into())); }
    assert_eq!(translate("Total: 0018", &snapshot()), TextDecision::Replace("总计: 0018".into()));
    assert_eq!(translate("Unknown:18", &snapshot().with_entry("ordinary", "Unknown:18", "旧译文")), TextDecision::Keep);
}
#[test]
fn rules_keep_dictionary_order_and_never_borrow_other_dictionary_translations() {
    let base = TranslationSnapshot::empty(Generation::new(1)).with_entry("second", "Total", "另一译文");
    let first = base.clone().with_dictionary_rules("first", rules()).with_dictionary_rules("second", rules());
    assert_eq!(translate("Total:18", &first), TextDecision::Keep);
    let second = base.with_dictionary_rules("second", rules()).with_dictionary_rules("first", rules());
    assert_eq!(translate("Total:18", &second), TextDecision::Replace("另一译文:18".into()));
    assert_ne!(first.digest(), second.digest());
    let restricted = TranslationSnapshot::empty(Generation::new(1)).with_entry_for_adapters("first", "Total", "总计", ["other"]).with_dictionary_rules("first", rules());
    assert_eq!(translate("Total:18", &restricted), TextDecision::Keep);
}
#[test]
fn disabling_rules_restores_exact_lookup_and_source_collection() {
    let mut disabled = rule(); disabled.enabled = false;
    let disabled = RegexTranslationRules::compile(vec![disabled]).unwrap();
    assert!(disabled.collection_sources("Total:18").is_none());
    assert_eq!(translate("Total:18", &snapshot().with_dictionary_rules("first", disabled)), TextDecision::Replace("旧译文".into()));
    for n in 0..101 { assert_eq!(rules().collection_sources(&format!("Total:{n}")), Some(vec!["Total".into()])); }
}
#[test]
fn pure_rule_tester_uses_runtime_parser_without_dictionary_access() {
    let tested = test_regex_rule(rule(), "Total:18", "总计").unwrap();
    assert!(tested.matched); assert_eq!(tested.output, "总计:18");
    assert_eq!(tested.captures, vec![Some("Total:18".into()), Some("Total".into()), Some(":18".into())]);
    assert!(test_regex_rule(rule(), "Total:18", "").unwrap().missing_translation);
    assert!(!test_regex_rule(rule(), "No count", "总计").unwrap().matched);
    let mut invalid = rule(); invalid.pattern = "[".into(); assert!(test_regex_rule(invalid, "x", "").is_err());
    let mut invalid = rule(); invalid.pattern = "plain".into(); assert!(test_regex_rule(invalid, "plain", "").is_err());
    let mut plain = rule(); plain.pattern = "(cat)".into(); plain.replacement = "[$1]$$".into();
    assert_eq!(test_regex_rule(plain.clone(), "a cat b", "").unwrap().output, "a [cat]$ b");
    plain.replacement = "".into(); assert_eq!(test_regex_rule(plain.clone(), "cat", "").unwrap().output, "");
    assert_eq!(RegexTranslationRules::compile(vec![plain.clone()]).unwrap().collection_sources("cat"), Some(vec![]));
    plain.replacement = "{{TR}}{{TR}}".into(); assert!(test_regex_rule(plain, "cat", &"x".repeat(9000)).is_err());
}
