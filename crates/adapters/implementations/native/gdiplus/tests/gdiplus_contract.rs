use glyphshift_adapter_gdiplus::{
    descriptor, GdiPlusCall, GdiPlusFont, GdiPlusInlineAdapter, ADAPTER_ID,
};
use glyphshift_adapter_sdk::{ActivationGrant, AdapterError, AdapterVersion};
use glyphshift_domain::{
    ApplyModel, Feature, FontDecision, Generation, Placement, RenderDecision, TextDecision,
};
use std::cell::Cell;

fn decision(text: TextDecision, font: FontDecision) -> RenderDecision {
    RenderDecision {
        text,
        font,
        generation: Generation::new(1),
    }
}

#[test]
fn gdp_001_to_004_preserve_or_replace_text_and_font_in_one_original_call() {
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

    for (render_decision, expected_text, expected_family) in cases {
        let calls = Cell::new(0);
        let output = GdiPlusInlineAdapter::new().invoke(
            GdiPlusCall::utf16(
                "Open",
                GdiPlusFont::new("Example Sans", 18.0, 1, 2),
                11,
                12,
                13,
            ),
            |_| Ok(render_decision),
            |prepared| {
                calls.set(calls.get() + 1);
                prepared
            },
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(output.text(), Some(expected_text));
        assert_eq!(output.font().family(), expected_family);
        assert_eq!(output.font().size(), 18.0);
        assert_eq!(output.font().style(), 1);
        assert_eq!(output.font().unit(), 2);
        assert_eq!(output.layout_id(), 11);
        assert_eq!(output.format_id(), 12);
        assert_eq!(output.brush_id(), 13);
    }
}

#[test]
fn gdp_005_invalid_excessive_reentry_and_decision_failure_fail_open_once() {
    let font = GdiPlusFont::new("Example Sans", 18.0, 0, 2);
    let inputs = [
        GdiPlusCall::invalid_utf16(font.clone(), 11, 12, 13),
        GdiPlusCall::utf16("Open", font.clone(), 11, 12, 13).with_excessive_length(),
        GdiPlusCall::utf16("Open", font, 11, 12, 13).with_reentry_detected(),
    ];
    for input in inputs {
        let calls = Cell::new(0);
        let output = GdiPlusInlineAdapter::new().invoke(
            input,
            |_| Err(AdapterError::DecisionUnavailable),
            |prepared| {
                calls.set(calls.get() + 1);
                prepared
            },
        );
        assert_eq!(calls.get(), 1);
        assert!(output.is_original());
    }
}

#[test]
fn gdp_006_descriptor_and_activation_keep_text_and_font_capabilities_independent() {
    let descriptor = descriptor();
    assert_eq!(descriptor.adapter_id().as_str(), ADAPTER_ID);
    assert_eq!(descriptor.version(), AdapterVersion::new(1, 0, 0));
    assert_eq!(descriptor.apply_model(), ApplyModel::InlineRender);
    assert_eq!(descriptor.placement(), Placement::TargetProcess);
    assert_eq!(
        descriptor.features().collect::<Vec<_>>(),
        [
            Feature::TextObserve,
            Feature::TextReplace,
            Feature::FontSubstitute
        ]
    );

    let grant = ActivationGrant::new([Feature::TextReplace, Feature::FontSubstitute]);
    let cases = [
        (
            GdiPlusInlineAdapter::activate([Feature::TextReplace], &grant)
                .expect("text activation"),
            "Localized",
            "Source Sans",
        ),
        (
            GdiPlusInlineAdapter::activate([Feature::FontSubstitute], &grant)
                .expect("font activation"),
            "Open",
            "Target Sans",
        ),
    ];
    for (mut adapter, expected_text, expected_font) in cases {
        let output = adapter.invoke(
            GdiPlusCall::utf16("Open", GdiPlusFont::new("Source Sans", 16.0, 0, 2), 1, 2, 3),
            |_| {
                Ok(decision(
                    TextDecision::Replace("Localized".into()),
                    FontDecision::Substitute("Target Sans".into()),
                ))
            },
            |prepared| prepared,
        );
        assert_eq!(output.text(), Some(expected_text));
        assert_eq!(output.font().family(), expected_font);
    }
}
