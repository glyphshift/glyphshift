//! First-party Qt `QPainter::drawText` inline-render Adapter.

use glyphshift_adapter_sdk::{
    authorize, ActivationGrant, AdapterDescriptor, AdapterError, AdapterVersion,
};
use glyphshift_domain::{AdapterId, ApplyModel, Feature, Placement, RenderDecision, TextDecision};
use std::panic::{catch_unwind, AssertUnwindSafe};

const MAX_TEXT_UNITS: usize = 16_384;
pub const ADAPTER_ID: &str = "windows.qt.painter-draw-text";

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
    .with_architectures(["x86_64"])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QtDrawTextKind {
    Point,
    RectangleFlags,
    RectangleOption,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QtPainterCall {
    units: Vec<u16>,
    kind: QtDrawTextKind,
    excessive_length: bool,
    reentry_detected: bool,
}

impl QtPainterCall {
    #[must_use]
    pub fn utf16(text: &str, kind: QtDrawTextKind) -> Self {
        Self {
            units: text.encode_utf16().collect(),
            kind,
            excessive_length: false,
            reentry_detected: false,
        }
    }

    #[must_use]
    pub fn units(units: impl Into<Vec<u16>>, kind: QtDrawTextKind) -> Self {
        Self {
            units: units.into(),
            kind,
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
pub struct PreparedQtPainterCall {
    units: Vec<u16>,
    text: Option<Box<str>>,
    kind: QtDrawTextKind,
    original: bool,
}

impl PreparedQtPainterCall {
    #[must_use]
    pub fn units(&self) -> &[u16] {
        &self.units
    }

    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    #[must_use]
    pub const fn kind(&self) -> QtDrawTextKind {
        self.kind
    }

    #[must_use]
    pub const fn is_original(&self) -> bool {
        self.original
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QtPainterInlineAdapter {
    text_replace: bool,
}

impl QtPainterInlineAdapter {
    #[must_use]
    pub const fn new() -> Self {
        Self { text_replace: true }
    }

    pub fn activate(
        requested: impl IntoIterator<Item = Feature>,
        grant: &ActivationGrant,
    ) -> Result<Self, AdapterError> {
        let active = authorize(requested, grant)?;
        Ok(Self {
            text_replace: active.contains(&Feature::TextReplace),
        })
    }

    pub fn invoke<R>(
        &mut self,
        call: QtPainterCall,
        decide: impl FnOnce(&str) -> Result<RenderDecision, AdapterError>,
        original: impl FnOnce(PreparedQtPainterCall) -> R,
    ) -> R {
        let original_call = PreparedQtPainterCall {
            text: String::from_utf16(&call.units).ok().map(Into::into),
            units: call.units,
            kind: call.kind,
            original: true,
        };
        if call.excessive_length
            || call.reentry_detected
            || original_call.units.len() > MAX_TEXT_UNITS
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

        let mut prepared = original_call;
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

impl Default for QtPainterInlineAdapter {
    fn default() -> Self {
        Self::new()
    }
}
