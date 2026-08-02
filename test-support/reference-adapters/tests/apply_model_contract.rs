use glyphshift_adapter_sdk::{
    ActivationGrant, AdapterError, DrawCommand, InlineTextInput, ObjectId, ObservationId,
    SyntheticTarget,
};
use glyphshift_domain::{Feature, FontDecision, Generation, RenderDecision, TextDecision};
use glyphshift_reference_adapters::{
    ExternalReferenceAdapter, InlineReferenceAdapter, ObserveReferenceAdapter,
    RetainedReferenceAdapter,
};
use std::cell::Cell;

fn decision(text: TextDecision, font: FontDecision) -> RenderDecision {
    RenderDecision {
        text,
        font,
        generation: Generation::new(7),
    }
}

#[test]
fn app_001_to_004_inline_calls_original_once_for_every_decision_shape() {
    let cases = [
        (
            decision(TextDecision::Keep, FontDecision::Keep),
            "Open",
            "Example Sans",
        ),
        (
            decision(TextDecision::Replace("打开".into()), FontDecision::Keep),
            "打开",
            "Example Sans",
        ),
        (
            decision(
                TextDecision::Keep,
                FontDecision::Substitute("Example CJK".into()),
            ),
            "Open",
            "Example CJK",
        ),
        (
            decision(
                TextDecision::Replace("打开".into()),
                FontDecision::Substitute("Example CJK".into()),
            ),
            "打开",
            "Example CJK",
        ),
    ];
    let grant = ActivationGrant::new([Feature::TextReplace, Feature::FontSubstitute]);

    for (render_decision, expected_text, expected_font) in cases {
        let mut adapter = InlineReferenceAdapter::activate(
            [Feature::TextReplace, Feature::FontSubstitute],
            &grant,
        )
        .expect("authorized inline adapter");
        let calls = Cell::new(0);
        let rendered = adapter.invoke(
            InlineTextInput::utf16("Open".encode_utf16(), "Example Sans"),
            |_| Ok(render_decision),
            |command| {
                calls.set(calls.get() + 1);
                command
            },
        );

        assert_eq!(calls.get(), 1);
        assert_eq!(rendered.text(), expected_text);
        assert_eq!(rendered.font(), expected_font);
    }
}

#[test]
fn app_005_inline_decode_decision_panic_and_reentry_fail_open_once() {
    let grant = ActivationGrant::new([Feature::TextReplace]);
    let cases = [
        InlineTextInput::invalid_utf16("Example Sans"),
        InlineTextInput::utf16("Open".encode_utf16(), "Example Sans").with_excessive_length(),
        InlineTextInput::utf16("Open".encode_utf16(), "Example Sans").with_reentry_detected(),
    ];

    for input in cases {
        let mut adapter =
            InlineReferenceAdapter::activate([Feature::TextReplace], &grant).expect("activate");
        let calls = Cell::new(0);
        let rendered = adapter.invoke(
            input,
            |_| {
                Ok(decision(
                    TextDecision::Replace("bad".into()),
                    FontDecision::Keep,
                ))
            },
            |command| {
                calls.set(calls.get() + 1);
                command
            },
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(rendered.text(), "Open");
    }

    let mut adapter =
        InlineReferenceAdapter::activate([Feature::TextReplace], &grant).expect("activate");
    let calls = Cell::new(0);
    let rendered = adapter.invoke(
        InlineTextInput::utf16("Open".encode_utf16(), "Example Sans"),
        |_| Err(AdapterError::DecisionUnavailable),
        |command| {
            calls.set(calls.get() + 1);
            command
        },
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(rendered.text(), "Open");

    let mut adapter =
        InlineReferenceAdapter::activate([Feature::TextReplace], &grant).expect("activate");
    let calls = Cell::new(0);
    let rendered = adapter.invoke(
        InlineTextInput::utf16("Open".encode_utf16(), "Example Sans"),
        |_| -> Result<RenderDecision, AdapterError> { panic!("decision boundary panic") },
        |command| {
            calls.set(calls.get() + 1);
            command
        },
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(rendered.text(), "Open");
}

#[test]
fn app_006_to_008_retained_tracks_identity_and_restores_only_live_objects() {
    let grant = ActivationGrant::new([Feature::TextReplace, Feature::FontSubstitute]);
    let mut adapter =
        RetainedReferenceAdapter::activate([Feature::TextReplace, Feature::FontSubstitute], &grant)
            .expect("activate retained");
    let first = ObjectId::new("node-a", 1);
    adapter
        .create(
            first.clone(),
            DrawCommand::new("Open", "Example Sans"),
            decision(
                TextDecision::Replace("打开".into()),
                FontDecision::Substitute("Example CJK".into()),
            ),
        )
        .expect("create first");
    assert_eq!(adapter.object(&first).expect("live object").text(), "打开");

    adapter.destroy(&first).expect("destroy first");
    assert_eq!(
        adapter.apply(
            &first,
            decision(TextDecision::Replace("旧对象".into()), FontDecision::Keep)
        ),
        Err(AdapterError::ObjectNotFound)
    );

    let second = ObjectId::new("node-a", 2);
    adapter
        .create(
            second.clone(),
            DrawCommand::new("Save", "Example Sans"),
            decision(TextDecision::Replace("保存".into()), FontDecision::Keep),
        )
        .expect("recreated object");
    let restored = adapter.deactivate();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].0, second);
    assert_eq!(restored[0].1.text(), "Save");
}

#[test]
fn app_009_to_011_external_binds_decisions_to_authorized_live_observations() {
    let grant = ActivationGrant::new([Feature::TextReplace]);
    let mut adapter =
        ExternalReferenceAdapter::activate([Feature::TextReplace], &grant).expect("activate");
    let object = ObjectId::new("node-a", 1);
    let first = adapter.observe(object.clone(), DrawCommand::new("Open", "Example Sans"));
    assert_eq!(
        adapter
            .apply(
                first,
                Generation::new(3),
                decision(TextDecision::Replace("打开".into()), FontDecision::Keep),
            )
            .expect("live observation"),
        Generation::new(3)
    );
    assert_eq!(adapter.object(&object).expect("object").text(), "打开");

    adapter.invalidate(&object);
    assert_eq!(
        adapter.apply(
            first,
            Generation::new(4),
            decision(TextDecision::Replace("错误".into()), FontDecision::Keep),
        ),
        Err(AdapterError::ObservationExpired)
    );
    let second = adapter.observe(object.clone(), DrawCommand::new("Open", "Example Sans"));
    assert_ne!(first, second);
    adapter.disconnect();
    assert_eq!(
        adapter.apply(
            second,
            Generation::new(5),
            decision(TextDecision::Replace("错误".into()), FontDecision::Keep),
        ),
        Err(AdapterError::Disconnected)
    );
    assert!(adapter.is_degraded());

    assert_eq!(
        ExternalReferenceAdapter::activate([Feature::TextReplace, Feature::FontSubstitute], &grant,),
        Err(AdapterError::UnauthorizedFeature(Feature::FontSubstitute))
    );
}

#[test]
fn app_012_to_014_observe_probe_and_activation_are_non_mutating_and_bounded() {
    let target = SyntheticTarget::new(
        ["pixel-a", "pixel-b"],
        [ObjectId::new("node-a", 1)],
        ["subscription-a"],
    );
    let before = target.clone();

    let inline_probe = InlineReferenceAdapter::probe(&target);
    let retained_probe = RetainedReferenceAdapter::probe(&target);
    let external_probe = ExternalReferenceAdapter::probe(&target);
    let observe_probe = ObserveReferenceAdapter::probe(&target);

    assert_eq!(target, before);
    assert!(inline_probe.features().contains(&Feature::TextReplace));
    assert!(retained_probe.features().contains(&Feature::FontSubstitute));
    assert!(external_probe.features().contains(&Feature::TextReplace));
    assert_eq!(observe_probe.features(), &[Feature::TextObserve]);

    let grant = ActivationGrant::new([Feature::TextObserve]);
    let mut observe =
        ObserveReferenceAdapter::activate([Feature::TextObserve], &grant).expect("observe");
    observe.observe("Open");
    observe.observe("Save");
    assert_eq!(observe.observation_count(), 2);
    assert_eq!(observe.active_features(), &[Feature::TextObserve]);

    assert_eq!(
        InlineReferenceAdapter::activate([Feature::TextReplace], &grant),
        Err(AdapterError::UnauthorizedFeature(Feature::TextReplace))
    );
    assert_eq!(ObservationId::new(1), ObservationId::new(1));
}
