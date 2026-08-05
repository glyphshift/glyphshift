//! raylib 5.5-compatible dynamic `DrawTextEx` draw-time Dictionary Adapter.

use glyphshift_adapter_sdk::{
    authorize, ActivationGrant, AdapterDescriptor, AdapterError, AdapterVersion,
};
use glyphshift_domain::{AdapterId, ApplyModel, Feature, Placement, RenderDecision, TextDecision};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const ADAPTER_ID: &str = "windows.raylib.draw-text-ex";
pub const MAX_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_FALLBACK_GLYPHS: usize = 4096;

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
pub struct RaylibInlineAdapter {
    text_replace: bool,
}

impl RaylibInlineAdapter {
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

    #[must_use]
    pub fn prepare(
        self,
        source: &[u8],
        decide: impl FnOnce(&str) -> Result<RenderDecision, AdapterError>,
    ) -> Option<Vec<u8>> {
        if source.is_empty() || source.len() > MAX_TEXT_BYTES {
            return None;
        }
        let source = std::str::from_utf8(source).ok()?;
        let decision = catch_unwind(AssertUnwindSafe(|| decide(source)))
            .ok()
            .and_then(Result::ok)?;
        let TextDecision::Replace(text) = decision.text else {
            return None;
        };
        if !self.text_replace || text.is_empty() || text.as_bytes().contains(&0) {
            return None;
        }
        let replacement = text.as_bytes().to_vec();
        (replacement.len() <= MAX_TEXT_BYTES).then_some(replacement)
    }
}

impl Default for RaylibInlineAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphshift_domain::{FontDecision, Generation};
    use std::sync::Arc;

    fn replace(text: &str) -> RenderDecision {
        RenderDecision {
            text: TextDecision::Replace(Arc::from(text)),
            font: FontDecision::Keep,
            generation: Generation::new(2),
        }
    }

    #[test]
    fn descriptor_exposes_dynamic_inline_text_capabilities() {
        let descriptor = descriptor();
        assert_eq!(descriptor.adapter_id().as_str(), ADAPTER_ID);
        assert_eq!(descriptor.apply_model(), ApplyModel::InlineRender);
        assert_eq!(descriptor.placement(), Placement::TargetProcess);
        assert!(descriptor
            .features()
            .any(|feature| feature == Feature::TextObserve));
        assert!(descriptor
            .features()
            .any(|feature| feature == Feature::TextReplace));
    }

    #[test]
    fn utf8_source_prepares_the_dictionary_replacement() {
        let replacement = RaylibInlineAdapter::new()
            .prepare(b"Click to Select File", |_| Ok(replace("选择文件")));
        assert_eq!(replacement.as_deref(), Some("选择文件".as_bytes()));
    }

    #[test]
    fn invalid_or_unbounded_text_fails_open() {
        assert!(RaylibInlineAdapter::new()
            .prepare(&[0xff], |_| Ok(replace("译文")))
            .is_none());
        assert!(RaylibInlineAdapter::new()
            .prepare(&vec![b'a'; MAX_TEXT_BYTES + 1], |_| Ok(replace("译文")))
            .is_none());
        assert!(RaylibInlineAdapter::new()
            .prepare(b"Open", |_| Ok(replace("含\0空字节")))
            .is_none());
    }

    #[test]
    fn observe_only_or_failed_decision_keeps_the_original_call() {
        let grant = ActivationGrant::new([Feature::TextObserve]);
        let adapter = RaylibInlineAdapter::activate([Feature::TextObserve], &grant)
            .expect("observe-only activation");
        assert!(adapter.prepare(b"Open", |_| Ok(replace("打开"))).is_none());
        assert!(RaylibInlineAdapter::new()
            .prepare(b"Open", |_| panic!("host callback panic"))
            .is_none());
    }
}
