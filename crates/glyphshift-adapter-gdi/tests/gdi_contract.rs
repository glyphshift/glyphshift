use glyphshift_adapter_gdi::{
    descriptor, GdiCall, GdiFont, GdiGlyphMap, GdiInlineAdapter, ADAPTER_ID, ETO_GLYPH_INDEX,
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
fn gdi_001_unicode_pass_and_replace_call_original_once() {
    let font = GdiFont::new("Example Sans", -16, 400);
    let cases = [
        (
            decision(TextDecision::Keep, FontDecision::Keep),
            "Open",
            0x0004,
            true,
        ),
        (
            decision(TextDecision::Replace("打开".into()), FontDecision::Keep),
            "打开",
            0x0004,
            false,
        ),
    ];
    for (render_decision, expected, expected_options, keeps_spacing) in cases {
        let calls = Cell::new(0);
        let output = GdiInlineAdapter::new().invoke(
            GdiCall::unicode("Open", 0x0004, Some([7, 8, 9, 10]), font.clone()),
            None,
            |_| Ok(render_decision),
            |prepared| {
                calls.set(calls.get() + 1);
                prepared
            },
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(output.text(), Some(expected));
        assert_eq!(output.options(), expected_options);
        assert_eq!(output.spacing().is_some(), keeps_spacing);
    }
}

#[test]
fn gdi_002_003_decodes_current_font_cmap_and_rewrites_glyph_replace_arguments() {
    let map = GdiGlyphMap::new([(91, 'O'), (7, 'p'), (203, 'e'), (44, 'n')]);
    let calls = Cell::new(0);

    let output = GdiInlineAdapter::new().invoke(
        GdiCall::glyph_indices(
            [91, 7, 203, 44],
            ETO_GLYPH_INDEX | 0x0004,
            Some([4, 4, 4, 4]),
            GdiFont::new("Example Sans", -16, 400),
        ),
        Some(&map),
        |text| {
            assert_eq!(text, "Open");
            Ok(decision(
                TextDecision::Replace("打开".into()),
                FontDecision::Keep,
            ))
        },
        |prepared| {
            calls.set(calls.get() + 1);
            prepared
        },
    );

    assert_eq!(calls.get(), 1);
    assert_eq!(output.text(), Some("打开"));
    assert_eq!(output.options() & ETO_GLYPH_INDEX, 0);
    assert!(output.spacing().is_none());
}

#[test]
fn gdi_004_font_decision_preserves_size_and_weight_semantics() {
    let output = GdiInlineAdapter::new().invoke(
        GdiCall::unicode(
            "Open",
            0,
            None::<[i32; 0]>,
            GdiFont::new("Example Sans", -21, 600),
        ),
        None,
        |_| {
            Ok(decision(
                TextDecision::Replace("打开".into()),
                FontDecision::Substitute("Example CJK".into()),
            ))
        },
        |prepared| prepared,
    );

    assert_eq!(output.font().family(), "Example CJK");
    assert_eq!(output.font().height(), -21);
    assert_eq!(output.font().weight(), 600);
}

#[test]
fn gdi_005_invalid_excessive_reentry_and_decision_failure_fail_open_once() {
    let font = GdiFont::new("Example Sans", -16, 400);
    let inputs = [
        GdiCall::invalid_utf16(0, font.clone()),
        GdiCall::unicode("Open", 0, None::<[i32; 0]>, font.clone()).with_excessive_length(),
        GdiCall::unicode("Open", 0, None::<[i32; 0]>, font.clone()).with_reentry_detected(),
        GdiCall::glyph_indices([999], ETO_GLYPH_INDEX, None::<[i32; 0]>, font),
    ];
    for input in inputs {
        let calls = Cell::new(0);
        let output = GdiInlineAdapter::new().invoke(
            input,
            Some(&GdiGlyphMap::new([])),
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
fn gdi_006_descriptor_and_activation_keep_text_and_font_capabilities_independent() {
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
            GdiInlineAdapter::activate([Feature::TextReplace], &grant).expect("text activation"),
            "Localized",
            "Source Sans",
        ),
        (
            GdiInlineAdapter::activate([Feature::FontSubstitute], &grant).expect("font activation"),
            "Open",
            "Target Sans",
        ),
    ];
    for (mut adapter, expected_text, expected_font) in cases {
        let output = adapter.invoke(
            GdiCall::unicode(
                "Open",
                0,
                None::<[i32; 0]>,
                GdiFont::new("Source Sans", -16, 400),
            ),
            None,
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
