//! Host-independent Direct2D `DrawText` inline Adapter contracts.

use glyphshift_adapter_sdk::{
    authorize, ActivationGrant, AdapterDescriptor, AdapterError, AdapterVersion,
};
use glyphshift_domain::{AdapterId, ApplyModel, Feature, Placement, RenderDecision, TextDecision};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const ADAPTER_ID: &str = "windows.direct2d.draw-text";
const MAX_TEXT_UNITS: usize = 16_384;

#[must_use]
pub fn descriptor() -> AdapterDescriptor {
    AdapterDescriptor::new(
        AdapterId::new(ADAPTER_ID),
        AdapterVersion::new(1, 0, 0),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextObserve, Feature::TextReplace],
    )
    .with_platforms(["windows"])
    .with_architectures(["x86", "x86_64"])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Direct2DRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Direct2DTextCall {
    units: Vec<u16>,
    layout: Direct2DRect,
    options: u32,
    measuring_mode: u32,
    excessive_length: bool,
    reentry_detected: bool,
}

impl Direct2DTextCall {
    #[must_use]
    pub fn utf16(text: &str, layout: Direct2DRect, options: u32, measuring_mode: u32) -> Self {
        Self {
            units: text.encode_utf16().collect(),
            layout,
            options,
            measuring_mode,
            excessive_length: false,
            reentry_detected: false,
        }
    }

    #[must_use]
    pub fn invalid_utf16(layout: Direct2DRect, options: u32, measuring_mode: u32) -> Self {
        Self {
            units: vec![0xd800],
            layout,
            options,
            measuring_mode,
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
pub struct PreparedDirect2DText {
    units: Vec<u16>,
    text: Option<Box<str>>,
    layout: Direct2DRect,
    options: u32,
    measuring_mode: u32,
    original: bool,
}

impl PreparedDirect2DText {
    #[must_use]
    pub fn units(&self) -> &[u16] {
        &self.units
    }

    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    #[must_use]
    pub const fn layout(&self) -> Direct2DRect {
        self.layout
    }

    #[must_use]
    pub const fn options(&self) -> u32 {
        self.options
    }

    #[must_use]
    pub const fn measuring_mode(&self) -> u32 {
        self.measuring_mode
    }

    #[must_use]
    pub const fn is_original(&self) -> bool {
        self.original
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Direct2DDrawTextAdapter {
    text_observe: bool,
    text_replace: bool,
}

impl Direct2DDrawTextAdapter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            text_observe: true,
            text_replace: true,
        }
    }

    pub fn activate(
        requested: impl IntoIterator<Item = Feature>,
        grant: &ActivationGrant,
    ) -> Result<Self, AdapterError> {
        let active = authorize(requested, grant)?;
        Ok(Self {
            text_observe: active.contains(&Feature::TextObserve),
            text_replace: active.contains(&Feature::TextReplace),
        })
    }

    pub fn invoke<R>(
        &mut self,
        call: Direct2DTextCall,
        decide: impl FnOnce(&str) -> Result<RenderDecision, AdapterError>,
        original: impl FnOnce(PreparedDirect2DText) -> R,
    ) -> R {
        let original_call = PreparedDirect2DText {
            text: String::from_utf16(&call.units).ok().map(Into::into),
            units: call.units.clone(),
            layout: call.layout,
            options: call.options,
            measuring_mode: call.measuring_mode,
            original: true,
        };
        if (!self.text_observe && !self.text_replace)
            || call.excessive_length
            || call.reentry_detected
            || call.units.len() > MAX_TEXT_UNITS
        {
            return original(original_call);
        }
        let Some(source) = original_call.text.as_deref() else {
            return original(original_call);
        };
        let decision = catch_unwind(AssertUnwindSafe(|| decide(source)))
            .ok()
            .and_then(Result::ok);
        let Some(decision) = decision else {
            return original(original_call);
        };

        let mut prepared = original_call.clone();
        if let TextDecision::Replace(text) = decision.text {
            if self.text_replace {
                prepared.units = text.encode_utf16().collect();
                prepared.text = Some(text.as_ref().into());
                prepared.original = false;
            }
        }
        original(prepared)
    }
}

impl Default for Direct2DDrawTextAdapter {
    fn default() -> Self {
        Self::new()
    }
}
