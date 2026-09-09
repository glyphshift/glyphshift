//! First-party GDI inline-render Adapter.

use glyphshift_adapter_sdk::{
    authorize, ActivationGrant, AdapterDescriptor, AdapterError, AdapterVersion,
};
use glyphshift_domain::{
    AdapterId, ApplyModel, Feature, FontDecision, Placement, RenderDecision, TextDecision,
};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const ETO_GLYPH_INDEX: u32 = 0x0010;
const MAX_TEXT_UNITS: usize = 16_384;
pub const EXT_TEXT_OUT_ADAPTER_ID: &str = "windows.gdi.ext-text-out";
pub const TEXT_OUT_ADAPTER_ID: &str = "windows.gdi.text-out";
pub const DRAW_TEXT_ADAPTER_ID: &str = "windows.user32.draw-text";
pub const ADAPTER_ID: &str = EXT_TEXT_OUT_ADAPTER_ID;

#[must_use]
pub fn descriptor() -> AdapterDescriptor {
    descriptor_for(EXT_TEXT_OUT_ADAPTER_ID)
}

#[must_use]
pub fn text_out_descriptor() -> AdapterDescriptor {
    descriptor_for(TEXT_OUT_ADAPTER_ID)
}

#[must_use]
pub fn draw_text_descriptor() -> AdapterDescriptor {
    descriptor_for(DRAW_TEXT_ADAPTER_ID)
}

fn descriptor_for(adapter_id: &'static str) -> AdapterDescriptor {
    AdapterDescriptor::new(
        AdapterId::new(adapter_id),
        AdapterVersion::new(1, 0, 0),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [
            Feature::TextObserve,
            Feature::TextReplace,
            Feature::FontSubstitute,
            Feature::FontScale,
        ],
    )
    .with_platforms(["windows"])
    .with_architectures(["x86", "x86_64"])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GdiFont {
    family: Box<str>,
    height: i32,
    weight: i32,
}

impl GdiFont {
    #[must_use]
    pub fn new(family: impl Into<Box<str>>, height: i32, weight: i32) -> Self {
        Self {
            family: family.into(),
            height,
            weight,
        }
    }

    #[must_use]
    pub fn family(&self) -> &str {
        &self.family
    }

    #[must_use]
    pub const fn height(&self) -> i32 {
        self.height
    }

    #[must_use]
    pub const fn weight(&self) -> i32 {
        self.weight
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GdiGlyphMap {
    glyphs: BTreeMap<u16, char>,
}

impl GdiGlyphMap {
    #[must_use]
    pub fn new(mappings: impl IntoIterator<Item = (u16, char)>) -> Self {
        let mut candidates = BTreeMap::<u16, Option<char>>::new();
        for (glyph, character) in mappings {
            if glyph == 0 || glyph == u16::MAX {
                continue;
            }
            candidates
                .entry(glyph)
                .and_modify(|mapped| {
                    if mapped.is_some_and(|mapped| mapped != character) {
                        *mapped = None;
                    }
                })
                .or_insert(Some(character));
        }
        Self {
            glyphs: candidates
                .into_iter()
                .filter_map(|(glyph, character)| character.map(|character| (glyph, character)))
                .collect(),
        }
    }

    fn decode(&self, glyphs: &[u16]) -> Option<String> {
        glyphs
            .iter()
            .map(|glyph| self.glyphs.get(glyph).copied())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GdiCall {
    units: Vec<u16>,
    options: u32,
    spacing: Option<Vec<i32>>,
    font: GdiFont,
    excessive_length: bool,
    reentry_detected: bool,
}

impl GdiCall {
    #[must_use]
    pub fn unicode(
        text: &str,
        options: u32,
        spacing: Option<impl IntoIterator<Item = i32>>,
        font: GdiFont,
    ) -> Self {
        Self {
            units: text.encode_utf16().collect(),
            options: options & !ETO_GLYPH_INDEX,
            spacing: spacing.map(|spacing| spacing.into_iter().collect()),
            font,
            excessive_length: false,
            reentry_detected: false,
        }
    }

    #[must_use]
    pub fn glyph_indices(
        glyphs: impl IntoIterator<Item = u16>,
        options: u32,
        spacing: Option<impl IntoIterator<Item = i32>>,
        font: GdiFont,
    ) -> Self {
        Self {
            units: glyphs.into_iter().collect(),
            options: options | ETO_GLYPH_INDEX,
            spacing: spacing.map(|spacing| spacing.into_iter().collect()),
            font,
            excessive_length: false,
            reentry_detected: false,
        }
    }

    #[must_use]
    pub fn invalid_utf16(options: u32, font: GdiFont) -> Self {
        Self {
            units: vec![0xd800],
            options: options & !ETO_GLYPH_INDEX,
            spacing: None,
            font,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedGdiCall {
    units: Vec<u16>,
    text: Option<Box<str>>,
    options: u32,
    spacing: Option<Vec<i32>>,
    font: GdiFont,
    original: bool,
}

impl PreparedGdiCall {
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    #[must_use]
    pub fn units(&self) -> &[u16] {
        &self.units
    }

    #[must_use]
    pub const fn options(&self) -> u32 {
        self.options
    }

    #[must_use]
    pub fn spacing(&self) -> Option<&[i32]> {
        self.spacing.as_deref()
    }

    #[must_use]
    pub const fn font(&self) -> &GdiFont {
        &self.font
    }

    #[must_use]
    pub const fn is_original(&self) -> bool {
        self.original
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GdiInlineAdapter {
    text_replace: bool,
    font_substitute: bool,
    font_scale: bool,
}

impl GdiInlineAdapter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            text_replace: true,
            font_substitute: true,
            font_scale: true,
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
            font_scale: active.contains(&Feature::FontScale),
        })
    }

    pub fn invoke<R>(
        &mut self,
        call: GdiCall,
        glyph_map: Option<&GdiGlyphMap>,
        decide: impl FnOnce(&str) -> Result<RenderDecision, AdapterError>,
        original: impl FnOnce(PreparedGdiCall) -> R,
    ) -> R {
        let original_call = PreparedGdiCall {
            units: call.units.clone(),
            text: if call.options & ETO_GLYPH_INDEX == 0 {
                String::from_utf16(&call.units).ok().map(Into::into)
            } else {
                None
            },
            options: call.options,
            spacing: call.spacing.clone(),
            font: call.font.clone(),
            original: true,
        };
        let decoded = if call.excessive_length
            || call.reentry_detected
            || call.units.len() > MAX_TEXT_UNITS
        {
            None
        } else if call.options & ETO_GLYPH_INDEX != 0 {
            glyph_map.and_then(|map| map.decode(&call.units))
        } else {
            String::from_utf16(&call.units).ok()
        };
        let Some(decoded) = decoded else {
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
                prepared.options &= !ETO_GLYPH_INDEX;
                prepared.spacing = None;
                prepared.original = false;
            }
            TextDecision::Keep | TextDecision::Replace(_) => {}
        }
        match decision.font {
            FontDecision::Scaled { family, percent } => {
                if self.font_substitute {
                    if let Some(family) = family {
                        prepared.font.family = family.as_ref().into();
                        prepared.original = false;
                    }
                }
                if self.font_scale
                    && (50..=200).contains(&percent)
                    && percent != 100
                    && prepared.font.height != 0
                {
                    let value = i64::from(prepared.font.height);
                    prepared.font.height = (((value.abs() * i64::from(percent) + 50) / 100)
                        .clamp(1, i64::from(i32::MAX))
                        * value.signum()) as i32;
                    prepared.original = false;
                    prepared.spacing = None;
                }
            }
            FontDecision::Substitute(family) if self.font_substitute => {
                prepared.font.family = family.as_ref().into();
                prepared.spacing = None;
                prepared.original = false;
            }
            FontDecision::Keep | FontDecision::Substitute(_) => {}
        }
        original(prepared)
    }
}

impl Default for GdiInlineAdapter {
    fn default() -> Self {
        Self::new()
    }
}
