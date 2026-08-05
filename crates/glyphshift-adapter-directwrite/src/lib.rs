//! Host-independent contracts for DirectWrite TextLayout draw-time replacement.

use glyphshift_adapter_sdk::{
    authorize, ActivationGrant, AdapterDescriptor, AdapterError, AdapterVersion,
};
use glyphshift_domain::{AdapterId, ApplyModel, Feature, Placement, RenderDecision, TextDecision};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const ADAPTER_ID: &str = "windows.directwrite.text-layout";
const MAX_TEXT_UNITS: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextLayoutFormatting {
    Uniform,
    LocalRanges,
    InlineObject,
    Unknown,
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectWriteTextLayoutCall {
    units: Vec<u16>,
    formatting: TextLayoutFormatting,
    excessive_length: bool,
    reentry_detected: bool,
}

impl DirectWriteTextLayoutCall {
    #[must_use]
    pub fn utf16(text: &str, formatting: TextLayoutFormatting) -> Self {
        Self {
            units: text.encode_utf16().collect(),
            formatting,
            excessive_length: false,
            reentry_detected: false,
        }
    }

    #[must_use]
    pub fn invalid_utf16(formatting: TextLayoutFormatting) -> Self {
        Self {
            units: vec![0xd800],
            formatting,
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
pub struct PreparedDirectWriteTextLayout {
    units: Vec<u16>,
    text: Option<Box<str>>,
    original: bool,
}

impl PreparedDirectWriteTextLayout {
    #[must_use]
    pub fn units(&self) -> &[u16] {
        &self.units
    }

    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    #[must_use]
    pub const fn is_original(&self) -> bool {
        self.original
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectWriteTextLayoutAdapter {
    text_observe: bool,
    text_replace: bool,
}

impl DirectWriteTextLayoutAdapter {
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
        call: DirectWriteTextLayoutCall,
        decide: impl FnOnce(&str) -> Result<RenderDecision, AdapterError>,
        draw: impl FnOnce(PreparedDirectWriteTextLayout) -> R,
    ) -> R {
        let original = PreparedDirectWriteTextLayout {
            text: String::from_utf16(&call.units).ok().map(Into::into),
            units: call.units,
            original: true,
        };
        if (!self.text_observe && !self.text_replace)
            || call.excessive_length
            || call.reentry_detected
            || original.units.len() > MAX_TEXT_UNITS
        {
            return draw(original);
        }
        if self.text_replace && call.formatting != TextLayoutFormatting::Uniform {
            return draw(original);
        }
        let Some(source) = original.text.as_deref() else {
            return draw(original);
        };
        let decision = catch_unwind(AssertUnwindSafe(|| decide(source)))
            .ok()
            .and_then(Result::ok);
        let Some(decision) = decision else {
            return draw(original);
        };

        let mut prepared = original;
        if let TextDecision::Replace(text) = decision.text {
            if self.text_replace && !text.is_empty() {
                prepared.units = text.encode_utf16().collect();
                prepared.text = Some(text.as_ref().into());
                prepared.original = false;
            }
        }
        draw(prepared)
    }
}

impl Default for DirectWriteTextLayoutAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphshift_domain::{FontDecision, Generation};
    use std::cell::Cell;

    fn replace(text: &str) -> RenderDecision {
        RenderDecision {
            text: TextDecision::Replace(text.into()),
            font: FontDecision::Keep,
            generation: Generation::new(1),
        }
    }

    #[test]
    fn descriptor_is_a_narrow_target_process_text_layout_adapter() {
        let descriptor = descriptor();
        assert_eq!(descriptor.adapter_id().as_str(), ADAPTER_ID);
        assert_eq!(descriptor.apply_model(), ApplyModel::InlineRender);
        assert_eq!(descriptor.placement(), Placement::TargetProcess);
        assert_eq!(
            descriptor.features().collect::<Vec<_>>(),
            [Feature::TextObserve, Feature::TextReplace]
        );
    }

    #[test]
    fn uniform_layout_uses_dictionary_replacement() {
        let prepared = DirectWriteTextLayoutAdapter::new().invoke(
            DirectWriteTextLayoutCall::utf16("Open", TextLayoutFormatting::Uniform),
            |_| Ok(replace("打开")),
            |prepared| prepared,
        );
        assert_eq!(prepared.text(), Some("打开"));
        assert!(!prepared.is_original());
    }

    #[test]
    fn observe_only_reports_source_and_keeps_original() {
        let mut adapter = DirectWriteTextLayoutAdapter::activate(
            [Feature::TextObserve],
            &ActivationGrant::new([Feature::TextObserve]),
        )
        .expect("observe activation");
        let observed = Cell::new(false);
        let prepared = adapter.invoke(
            DirectWriteTextLayoutCall::utf16("Name", TextLayoutFormatting::LocalRanges),
            |source| {
                assert_eq!(source, "Name");
                observed.set(true);
                Ok(replace("名称"))
            },
            |prepared| prepared,
        );
        assert!(observed.get());
        assert!(prepared.is_original());
    }

    #[test]
    fn non_uniform_invalid_large_reentry_and_failure_fail_open_once() {
        let calls = [
            DirectWriteTextLayoutCall::utf16("Open", TextLayoutFormatting::LocalRanges),
            DirectWriteTextLayoutCall::utf16("Open", TextLayoutFormatting::InlineObject),
            DirectWriteTextLayoutCall::utf16("Open", TextLayoutFormatting::Unknown),
            DirectWriteTextLayoutCall::invalid_utf16(TextLayoutFormatting::Uniform),
            DirectWriteTextLayoutCall::utf16("Open", TextLayoutFormatting::Uniform)
                .with_excessive_length(),
            DirectWriteTextLayoutCall::utf16("Open", TextLayoutFormatting::Uniform)
                .with_reentry_detected(),
        ];
        for call in calls {
            let draws = Cell::new(0);
            let prepared = DirectWriteTextLayoutAdapter::new().invoke(
                call,
                |_| Err(AdapterError::DecisionUnavailable),
                |prepared| {
                    draws.set(draws.get() + 1);
                    prepared
                },
            );
            assert_eq!(draws.get(), 1);
            assert!(prepared.is_original());
        }
    }
}
