use glyphshift_adapter_console::{descriptor, ConsoleWriteCall, ConsoleWriteObserver, ADAPTER_ID};
use glyphshift_adapter_sdk::{ActivationGrant, AdapterError, AdapterVersion};
use glyphshift_domain::{
    ApplyModel, Feature, FontDecision, Generation, Placement, RenderDecision, TextDecision,
};
use std::cell::Cell;
use std::cell::RefCell;

fn keep() -> RenderDecision {
    RenderDecision {
        text: TextDecision::Keep,
        font: FontDecision::Keep,
        generation: Generation::new(1),
    }
}

#[test]
fn con_001_descriptor_is_a_target_process_observer() {
    let descriptor = descriptor();

    assert_eq!(descriptor.adapter_id().as_str(), ADAPTER_ID);
    assert_eq!(descriptor.version(), AdapterVersion::new(1, 0, 0));
    assert_eq!(descriptor.apply_model(), ApplyModel::ObserveOnly);
    assert_eq!(descriptor.placement(), Placement::TargetProcess);
    assert_eq!(
        descriptor.features().collect::<Vec<_>>(),
        [Feature::TextObserve]
    );
}

#[test]
fn con_002_observes_unicode_without_changing_the_write() {
    let observed = Cell::new(0);
    let prepared = ConsoleWriteObserver::new().invoke(
        ConsoleWriteCall::utf16("Open\r\nFile\r\n"),
        |source| {
            assert!(matches!(source, "Open" | "File"));
            observed.set(observed.get() + 1);
            Ok(keep())
        },
        |prepared| prepared,
    );

    assert_eq!(observed.get(), 2);
    assert_eq!(prepared.text(), Some("Open\r\nFile\r\n"));
    assert_eq!(
        prepared.units(),
        "Open\r\nFile\r\n".encode_utf16().collect::<Vec<_>>()
    );
}

#[test]
fn con_003_invalid_excessive_reentry_and_observer_failure_pass_through_once() {
    let cases = [
        (ConsoleWriteCall::invalid_utf16(), 0),
        (ConsoleWriteCall::utf16("Open").with_excessive_length(), 0),
        (ConsoleWriteCall::utf16("Open").with_reentry_detected(), 0),
        (ConsoleWriteCall::utf16("Open"), 1),
    ];
    for (call, expected_observations) in cases {
        let observations = Cell::new(0);
        let originals = Cell::new(0);
        let prepared = ConsoleWriteObserver::new().invoke(
            call,
            |_| {
                observations.set(observations.get() + 1);
                Err(AdapterError::DecisionUnavailable)
            },
            |prepared| {
                originals.set(originals.get() + 1);
                prepared
            },
        );
        assert_eq!(originals.get(), 1);
        assert_eq!(observations.get(), expected_observations);
        assert!(!prepared.units().is_empty());
    }
}

#[test]
fn con_004_activation_rejects_writeback_features() {
    let error = ConsoleWriteObserver::activate(
        [Feature::TextReplace],
        &ActivationGrant::new([Feature::TextObserve]),
    )
    .expect_err("observe-only adapter must reject TextReplace");

    assert_eq!(
        error,
        AdapterError::UnauthorizedFeature(Feature::TextReplace)
    );
}

#[test]
fn con_005_strips_terminal_control_sequences_before_observing_text() {
    let observed = RefCell::new(Vec::new());
    let source = concat!(
        "\u{1b}[13G\u{1b}[K\r\n",
        "\u{1b}[0;90mlo\u{1b}[mRight\u{1b}[0;90m\r\n",
        "C:\\Users\\sample>\u{1b}[m\r\n",
        "\u{1b}]8;;https://example.invalid\u{7}Insert Suggestion\u{1b}]8;;\u{7}\r\n",
    );

    let prepared = ConsoleWriteObserver::new().invoke(
        ConsoleWriteCall::utf16(source),
        |text| {
            observed.borrow_mut().push(text.to_owned());
            Ok(keep())
        },
        |prepared| prepared,
    );

    assert_eq!(
        observed.into_inner(),
        ["loRight", "C:\\Users\\sample>", "Insert Suggestion"]
    );
    assert_eq!(prepared.text(), Some(source));
}
