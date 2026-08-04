use glyphshift_adapter_sdk::AdapterVersion;
use glyphshift_adapter_uia::{
    descriptor, UiaElementSnapshot, UiaIgnoreReason, UiaObservationOutcome, UiaObserver,
    UiaTextChannel, ADAPTER_ID,
};
use glyphshift_domain::{ApplyModel, Feature, Placement};

fn observed(outcome: UiaObservationOutcome) -> glyphshift_adapter_uia::UiaTextObservation {
    let UiaObservationOutcome::Observed(observation) = outcome else {
        panic!("expected observation");
    };
    observation
}

#[test]
fn uia_001_descriptor_is_an_observe_only_isolated_worker() {
    let descriptor = descriptor();
    assert_eq!(descriptor.adapter_id().as_str(), ADAPTER_ID);
    assert_eq!(descriptor.version(), AdapterVersion::new(1, 0, 0));
    assert_eq!(descriptor.apply_model(), ApplyModel::ObserveOnly);
    assert_eq!(descriptor.placement(), Placement::IsolatedWorker);
    assert_eq!(
        descriptor.features().collect::<Vec<_>>(),
        [Feature::TextObserve]
    );
}

#[test]
fn uia_002_selects_one_primary_channel_per_element() {
    let mut observer = UiaObserver::default();
    let label = observed(observer.observe(UiaElementSnapshot::new("label").with_name("Open")));
    let document = observed(
        observer.observe(
            UiaElementSnapshot::new("document")
                .with_name("Document")
                .with_text("Line 1\r\nLine 2"),
        ),
    );
    let edit = observed(
        observer.observe(
            UiaElementSnapshot::new("edit")
                .with_name("Search")
                .with_value("query"),
        ),
    );

    assert_eq!(label.channel(), UiaTextChannel::Name);
    assert_eq!(label.text(), "Open");
    assert_eq!(document.channel(), UiaTextChannel::TextPattern);
    assert_eq!(document.text(), "Line 1\nLine 2");
    assert_eq!(edit.channel(), UiaTextChannel::ValuePattern);
    assert_eq!(edit.text(), "query");
}

#[test]
fn uia_003_rejects_password_empty_invalid_and_excessive_text() {
    let mut observer = UiaObserver::default();
    let cases = [
        (
            UiaElementSnapshot::new("password")
                .password(true)
                .with_value("secret"),
            UiaIgnoreReason::Sensitive,
        ),
        (
            UiaElementSnapshot::new("empty").with_name("  "),
            UiaIgnoreReason::NoText,
        ),
        (
            UiaElementSnapshot::new("").with_name("Open"),
            UiaIgnoreReason::InvalidElement,
        ),
        (
            UiaElementSnapshot::new("large").with_text("x".repeat(16 * 1024 + 1)),
            UiaIgnoreReason::TextTooLong,
        ),
    ];
    for (snapshot, expected) in cases {
        assert_eq!(
            observer.observe(snapshot),
            UiaObservationOutcome::Ignored(expected)
        );
    }
}

#[test]
fn uia_004_emits_changes_including_a_to_b_to_a_but_not_duplicates() {
    let mut observer = UiaObserver::default();
    assert!(matches!(
        observer.observe(UiaElementSnapshot::new("field").with_value("A")),
        UiaObservationOutcome::Observed(_)
    ));
    assert_eq!(
        observer.observe(UiaElementSnapshot::new("field").with_value("A")),
        UiaObservationOutcome::Ignored(UiaIgnoreReason::Unchanged)
    );
    assert!(matches!(
        observer.observe(UiaElementSnapshot::new("field").with_value("B")),
        UiaObservationOutcome::Observed(_)
    ));
    assert!(matches!(
        observer.observe(UiaElementSnapshot::new("field").with_value("A")),
        UiaObservationOutcome::Observed(_)
    ));
}

#[test]
fn uia_005_invalidation_allows_a_recreated_element_to_emit_again() {
    let mut observer = UiaObserver::default();
    let snapshot = || UiaElementSnapshot::new("recreated").with_name("Open");
    assert!(matches!(
        observer.observe(snapshot()),
        UiaObservationOutcome::Observed(_)
    ));
    observer.invalidate("recreated");
    assert!(matches!(
        observer.observe(snapshot()),
        UiaObservationOutcome::Observed(_)
    ));
}
