use glyphshift_adapter_qt_painter::{
    descriptor, PreparedQtPainterCall, QtDrawTextKind, QtPainterCall, QtPainterInlineAdapter,
    ADAPTER_ID,
};
use glyphshift_adapter_sdk::{ActivationGrant, AdapterError};
use glyphshift_domain::{
    ApplyModel, Feature, FontDecision, Generation, RenderDecision, TextDecision,
};
use std::cell::Cell;

fn translated(text: &str) -> RenderDecision {
    RenderDecision {
        text: TextDecision::Replace(text.into()),
        font: FontDecision::Keep,
        generation: Generation::new(1),
    }
}

#[test]
fn descriptor_declares_a_bounded_x64_inline_render_adapter() {
    let descriptor = descriptor();

    assert_eq!(descriptor.adapter_id().as_str(), ADAPTER_ID);
    assert_eq!(descriptor.apply_model(), ApplyModel::InlineRender);
    assert_eq!(descriptor.platforms().collect::<Vec<_>>(), vec!["windows"]);
    assert_eq!(
        descriptor.architectures().collect::<Vec<_>>(),
        vec!["x86_64"]
    );
    assert_eq!(
        descriptor.features().collect::<Vec<_>>(),
        vec![Feature::TextObserve, Feature::TextReplace]
    );
}

#[test]
fn replaces_each_supported_draw_shape_without_losing_its_kind() {
    for kind in [
        QtDrawTextKind::Point,
        QtDrawTextKind::RectangleFlags,
        QtDrawTextKind::RectangleOption,
    ] {
        let mut adapter = QtPainterInlineAdapter::new();
        let prepared = adapter.invoke(
            QtPainterCall::utf16("Open", kind),
            |source| {
                assert_eq!(source, "Open");
                Ok(translated("打开"))
            },
            |prepared| prepared,
        );

        assert_eq!(prepared.text(), Some("打开"));
        assert_eq!(prepared.kind(), kind);
        assert!(!prepared.is_original());
    }
}

#[test]
fn observe_only_activation_calls_the_dictionary_but_keeps_the_original() {
    let grant = ActivationGrant::new([Feature::TextObserve]);
    let mut adapter = QtPainterInlineAdapter::activate([Feature::TextObserve], &grant)
        .expect("observe-only activation");
    let observed = Cell::new(false);

    let prepared = adapter.invoke(
        QtPainterCall::utf16("Save", QtDrawTextKind::Point),
        |source| {
            observed.set(true);
            assert_eq!(source, "Save");
            Ok(translated("保存"))
        },
        |prepared| prepared,
    );

    assert!(observed.get());
    assert_eq!(prepared.text(), Some("Save"));
    assert!(prepared.is_original());
}

fn assert_fail_open(
    call: QtPainterCall,
    decide: impl FnOnce(&str) -> Result<RenderDecision, AdapterError>,
) {
    let original_calls = Cell::new(0);
    let mut adapter = QtPainterInlineAdapter::new();
    let prepared = adapter.invoke(call, decide, |prepared| {
        original_calls.set(original_calls.get() + 1);
        prepared
    });

    assert_eq!(original_calls.get(), 1);
    assert!(prepared.is_original());
}

#[test]
fn invalid_utf16_excessive_input_reentry_error_and_panic_fail_open_once() {
    assert_fail_open(
        QtPainterCall::units([0xd800], QtDrawTextKind::Point),
        |_| panic!("invalid UTF-16 must not reach the decision callback"),
    );
    assert_fail_open(
        QtPainterCall::utf16("Open", QtDrawTextKind::Point).with_excessive_length(),
        |_| panic!("excessive input must not reach the decision callback"),
    );
    assert_fail_open(
        QtPainterCall::utf16("Open", QtDrawTextKind::Point).with_reentry_detected(),
        |_| panic!("reentry must not reach the decision callback"),
    );
    assert_fail_open(QtPainterCall::utf16("Open", QtDrawTextKind::Point), |_| {
        Err(AdapterError::DecisionUnavailable)
    });
    assert_fail_open(QtPainterCall::utf16("Open", QtDrawTextKind::Point), |_| {
        panic!("decision panic")
    });
}

#[test]
fn an_actual_over_limit_buffer_is_not_decoded_or_observed() {
    let units = vec![u16::from(b'A'); 16_385];
    assert_fail_open(QtPainterCall::units(units, QtDrawTextKind::Point), |_| {
        panic!("over-limit input must not reach the decision callback")
    });
}

#[test]
fn prepared_units_match_the_replacement_utf16() {
    let mut adapter = QtPainterInlineAdapter::new();
    let prepared: PreparedQtPainterCall = adapter.invoke(
        QtPainterCall::utf16("Open", QtDrawTextKind::Point),
        |_| Ok(translated("界面")),
        |prepared| prepared,
    );

    assert_eq!(prepared.units(), "界面".encode_utf16().collect::<Vec<_>>());
}
