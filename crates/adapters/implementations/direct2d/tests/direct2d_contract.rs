use glyphshift_adapter_direct2d::{
    descriptor, Direct2DDrawTextAdapter, Direct2DRect, Direct2DTextCall, ADAPTER_ID,
};
use glyphshift_adapter_sdk::{ActivationGrant, AdapterError, AdapterVersion};
use glyphshift_domain::{
    ApplyModel, Feature, FontDecision, Generation, Placement, RenderDecision, TextDecision,
};
use std::cell::Cell;

fn layout() -> Direct2DRect {
    Direct2DRect {
        left: 12.0,
        top: 12.0,
        right: 348.0,
        bottom: 84.0,
    }
}

fn decision(text: TextDecision) -> RenderDecision {
    RenderDecision {
        text,
        font: FontDecision::Keep,
        generation: Generation::new(1),
    }
}

#[test]
fn d2d_001_descriptor_is_a_narrow_inline_text_adapter_without_font_claims() {
    let descriptor = descriptor();

    assert_eq!(descriptor.adapter_id().as_str(), ADAPTER_ID);
    assert_eq!(descriptor.version(), AdapterVersion::new(1, 0, 0));
    assert_eq!(descriptor.apply_model(), ApplyModel::InlineRender);
    assert_eq!(descriptor.placement(), Placement::TargetProcess);
    assert_eq!(
        descriptor.features().collect::<Vec<_>>(),
        [Feature::TextObserve, Feature::TextReplace]
    );
}

#[test]
fn d2d_002_observe_only_reports_unicode_and_keeps_the_original_draw() {
    let mut adapter = Direct2DDrawTextAdapter::activate(
        [Feature::TextObserve],
        &ActivationGrant::new([Feature::TextObserve]),
    )
    .expect("observe activation");
    let observed = Cell::new(false);

    let prepared = adapter.invoke(
        Direct2DTextCall::utf16("Open", layout(), 3, 2),
        |source| {
            assert_eq!(source, "Open");
            observed.set(true);
            Ok(decision(TextDecision::Replace("打开".into())))
        },
        |prepared| prepared,
    );

    assert!(observed.get());
    assert_eq!(prepared.text(), Some("Open"));
    assert!(prepared.is_original());
}

#[test]
fn d2d_003_replace_changes_only_text_and_preserves_draw_arguments() {
    let calls = Cell::new(0);

    let prepared = Direct2DDrawTextAdapter::new().invoke(
        Direct2DTextCall::utf16("Open", layout(), 3, 2),
        |source| {
            assert_eq!(source, "Open");
            Ok(decision(TextDecision::Replace("打开".into())))
        },
        |prepared| {
            calls.set(calls.get() + 1);
            prepared
        },
    );

    assert_eq!(calls.get(), 1);
    assert_eq!(prepared.text(), Some("打开"));
    assert_eq!(prepared.layout(), layout());
    assert_eq!(prepared.options(), 3);
    assert_eq!(prepared.measuring_mode(), 2);
    assert!(!prepared.is_original());
}

#[test]
fn d2d_004_invalid_excessive_reentry_and_decision_failure_fail_open_once() {
    let cases = [
        (Direct2DTextCall::invalid_utf16(layout(), 0, 0), 0),
        (
            Direct2DTextCall::utf16("Open", layout(), 0, 0).with_excessive_length(),
            0,
        ),
        (
            Direct2DTextCall::utf16("Open", layout(), 0, 0).with_reentry_detected(),
            0,
        ),
        (Direct2DTextCall::utf16("Open", layout(), 0, 0), 1),
    ];
    for (input, expected_decisions) in cases {
        let decisions = Cell::new(0);
        let originals = Cell::new(0);
        let prepared = Direct2DDrawTextAdapter::new().invoke(
            input,
            |_| {
                decisions.set(decisions.get() + 1);
                Err(AdapterError::DecisionUnavailable)
            },
            |prepared| {
                originals.set(originals.get() + 1);
                prepared
            },
        );
        assert_eq!(originals.get(), 1);
        assert!(prepared.is_original());
        assert_eq!(decisions.get(), expected_decisions);
    }
}
