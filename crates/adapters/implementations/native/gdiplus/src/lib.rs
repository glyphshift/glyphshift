//! First-party GDI+ inline-render Adapter.

use glyphshift_adapter_sdk::{
    authorize, ActivationGrant, AdapterDescriptor, AdapterError, AdapterVersion,
};
use glyphshift_domain::{
    AdapterId, ApplyModel, Feature, FontDecision, Placement, RenderDecision, TextDecision,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

const MAX_TEXT_UNITS: usize = 16_384;
pub const ADAPTER_ID: &str = "windows.gdiplus.draw-string";

#[must_use]
pub fn descriptor() -> AdapterDescriptor {
    AdapterDescriptor::new(
        AdapterId::new(ADAPTER_ID),
        AdapterVersion::new(1, 0, 0),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [
            Feature::TextObserve,
            Feature::TextReplace,
            Feature::FontSubstitute,
        ],
    )
    .with_platforms(["windows"])
    .with_architectures(["x86", "x86_64"])
}

#[derive(Clone, Debug, PartialEq)]
pub struct GdiPlusFont {
    family: Box<str>,
    size: f32,
    style: i32,
    unit: i32,
}

impl GdiPlusFont {
    #[must_use]
    pub fn new(family: impl Into<Box<str>>, size: f32, style: i32, unit: i32) -> Self {
        Self {
            family: family.into(),
            size,
            style,
            unit,
        }
    }

    #[must_use]
    pub fn family(&self) -> &str {
        &self.family
    }

    #[must_use]
    pub const fn size(&self) -> f32 {
        self.size
    }

    #[must_use]
    pub const fn style(&self) -> i32 {
        self.style
    }

    #[must_use]
    pub const fn unit(&self) -> i32 {
        self.unit
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GdiPlusCall {
    units: Vec<u16>,
    font: GdiPlusFont,
    layout_id: u64,
    format_id: u64,
    brush_id: u64,
    excessive_length: bool,
    reentry_detected: bool,
}

impl GdiPlusCall {
    #[must_use]
    pub fn utf16(
        text: &str,
        font: GdiPlusFont,
        layout_id: u64,
        format_id: u64,
        brush_id: u64,
    ) -> Self {
        Self {
            units: text.encode_utf16().collect(),
            font,
            layout_id,
            format_id,
            brush_id,
            excessive_length: false,
            reentry_detected: false,
        }
    }

    #[must_use]
    pub fn invalid_utf16(font: GdiPlusFont, layout_id: u64, format_id: u64, brush_id: u64) -> Self {
        Self {
            units: vec![0xd800],
            font,
            layout_id,
            format_id,
            brush_id,
            excessive_length: false,
            reentry_detected: false,
        }
    }

    #[must_use]
    pub const fn with_excessive_length(mut self) -> Self {
        self.excessive_length = true;
        self
    }

    #[must_use]
    pub const fn with_reentry_detected(mut self) -> Self {
        self.reentry_detected = true;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedGdiPlusCall {
    units: Vec<u16>,
    text: Option<Box<str>>,
    font: GdiPlusFont,
    layout_id: u64,
    format_id: u64,
    brush_id: u64,
    original: bool,
}

impl PreparedGdiPlusCall {
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    #[must_use]
    pub fn units(&self) -> &[u16] {
        &self.units
    }

    #[must_use]
    pub const fn font(&self) -> &GdiPlusFont {
        &self.font
    }

    #[must_use]
    pub const fn layout_id(&self) -> u64 {
        self.layout_id
    }

    #[must_use]
    pub const fn format_id(&self) -> u64 {
        self.format_id
    }

    #[must_use]
    pub const fn brush_id(&self) -> u64 {
        self.brush_id
    }

    #[must_use]
    pub const fn is_original(&self) -> bool {
        self.original
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GdiPlusInlineAdapter {
    text_replace: bool,
    font_substitute: bool,
}

impl GdiPlusInlineAdapter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            text_replace: true,
            font_substitute: true,
        }
    }

    pub fn activate(
        requested: impl IntoIterator<Item = Feature>,
        grant: &ActivationGrant,
    ) -> Result<Self, AdapterError> {
        let active = authorize(requested, grant)?;
        Ok(Self {
            text_replace: active.contains(&Feature::TextReplace),
            font_substitute: active.contains(&Feature::FontSubstitute),
        })
    }

    pub fn invoke<R>(
        &mut self,
        call: GdiPlusCall,
        decide: impl FnOnce(&str) -> Result<RenderDecision, AdapterError>,
        original: impl FnOnce(PreparedGdiPlusCall) -> R,
    ) -> R {
        let original_call = PreparedGdiPlusCall {
            units: call.units.clone(),
            text: String::from_utf16(&call.units).ok().map(Into::into),
            font: call.font,
            layout_id: call.layout_id,
            format_id: call.format_id,
            brush_id: call.brush_id,
            original: true,
        };
        if call.excessive_length || call.reentry_detected || call.units.len() > MAX_TEXT_UNITS {
            return original(original_call);
        }
        let Some(decoded) = original_call.text.clone() else {
            return original(original_call);
        };
        let decision = catch_unwind(AssertUnwindSafe(|| decide(&decoded)))
            .ok()
            .and_then(Result::ok);
        let Some(decision) = decision else {
            return original(original_call);
        };

        let mut prepared = original_call.clone();
        match decision.text {
            TextDecision::Replace(text) if self.text_replace => {
                prepared.units = text.encode_utf16().collect();
                prepared.text = Some(text.as_ref().into());
                prepared.original = false;
            }
            TextDecision::Keep | TextDecision::Replace(_) => {}
        }
        match decision.font {
            FontDecision::Substitute(family) if self.font_substitute => {
                prepared.font.family = family.as_ref().into();
                prepared.original = false;
            }
            FontDecision::Keep | FontDecision::Substitute(_) => {}
        }
        original(prepared)
    }
}

impl Default for GdiPlusInlineAdapter {
    fn default() -> Self {
        Self::new()
    }
}
