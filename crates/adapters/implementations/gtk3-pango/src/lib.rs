//! GTK 3 `gtk_render_layout` draw-time Dictionary Adapter.

use glyphshift_adapter_sdk::{
    authorize, ActivationGrant, AdapterDescriptor, AdapterError, AdapterVersion,
};
use glyphshift_domain::{AdapterId, ApplyModel, Feature, Placement, RenderDecision, TextDecision};
use std::panic::{catch_unwind, AssertUnwindSafe};

const MAX_TEXT_BYTES: usize = 64 * 1024;
pub const ADAPTER_ID: &str = "windows.gtk3.pango-render-layout";

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
pub enum PangoAttributeProfile {
    None,
    FullTextOnly,
    LocalRanges,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gtk3PangoCall {
    bytes: Vec<u8>,
    attributes: PangoAttributeProfile,
    excessive_length: bool,
    reentry_detected: bool,
}

impl Gtk3PangoCall {
    #[must_use]
    pub fn utf8(text: &str, attributes: PangoAttributeProfile) -> Self {
        Self {
            bytes: text.as_bytes().to_vec(),
            attributes,
            excessive_length: false,
            reentry_detected: false,
        }
    }

    #[must_use]
    pub fn bytes(bytes: impl Into<Vec<u8>>, attributes: PangoAttributeProfile) -> Self {
        Self {
            bytes: bytes.into(),
            attributes,
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
pub struct PreparedGtk3PangoCall {
    bytes: Vec<u8>,
    text: Option<Box<str>>,
    original: bool,
}

impl PreparedGtk3PangoCall {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
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
pub struct Gtk3PangoInlineAdapter {
    text_replace: bool,
}

impl Gtk3PangoInlineAdapter {
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
        call: Gtk3PangoCall,
        decide: impl FnOnce(&str) -> Result<RenderDecision, AdapterError>,
        original: impl FnOnce(PreparedGtk3PangoCall) -> R,
    ) -> R {
        let text = std::str::from_utf8(&call.bytes).ok().map(Into::into);
        let original_call = PreparedGtk3PangoCall {
            bytes: call.bytes,
            text,
            original: true,
        };
        if call.excessive_length
            || call.reentry_detected
            || original_call.bytes.len() > MAX_TEXT_BYTES
            || matches!(
                call.attributes,
                PangoAttributeProfile::LocalRanges | PangoAttributeProfile::Unknown
            )
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
            if self.text_replace && !text.is_empty() && !text.as_bytes().contains(&0) {
                prepared.bytes = text.as_bytes().to_vec();
                prepared.text = Some(text.as_ref().into());
                prepared.original = false;
            }
        }
        original(prepared)
    }
}

impl Default for Gtk3PangoInlineAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphshift_adapter_sdk::ActivationGrant;
    use glyphshift_domain::{FontDecision, Generation, TextDecision};
    use std::sync::Arc;

    fn replace(text: &str) -> RenderDecision {
        RenderDecision {
            text: TextDecision::Replace(Arc::from(text)),
            font: FontDecision::Keep,
            generation: Generation::new(1),
        }
    }

    #[test]
    fn descriptor_exposes_inline_text_capabilities() {
        let descriptor = descriptor();
        assert_eq!(descriptor.adapter_id().as_str(), ADAPTER_ID);
        assert_eq!(descriptor.apply_model(), ApplyModel::InlineRender);
        assert!(descriptor
            .features()
            .any(|feature| feature == Feature::TextReplace));
    }

    #[test]
    fn plain_text_uses_dictionary_replacement() {
        let mut adapter = Gtk3PangoInlineAdapter::new();
        let prepared = adapter.invoke(
            Gtk3PangoCall::utf8("Open", PangoAttributeProfile::None),
            |_| Ok(replace("打开")),
            |prepared| prepared,
        );
        assert_eq!(prepared.text(), Some("打开"));
        assert!(!prepared.is_original());
    }

    #[test]
    fn full_text_attributes_allow_replacement() {
        let mut adapter = Gtk3PangoInlineAdapter::new();
        let prepared = adapter.invoke(
            Gtk3PangoCall::utf8("Name", PangoAttributeProfile::FullTextOnly),
            |_| Ok(replace("名称")),
            |prepared| prepared,
        );
        assert_eq!(prepared.text(), Some("名称"));
    }

    #[test]
    fn local_or_unknown_attributes_fail_open() {
        for attributes in [
            PangoAttributeProfile::LocalRanges,
            PangoAttributeProfile::Unknown,
        ] {
            let mut adapter = Gtk3PangoInlineAdapter::new();
            let prepared = adapter.invoke(
                Gtk3PangoCall::utf8("File", attributes),
                |_| Ok(replace("文件")),
                |prepared| prepared,
            );
            assert_eq!(prepared.text(), Some("File"));
            assert!(prepared.is_original());
        }
    }

    #[test]
    fn invalid_utf8_reentry_and_large_inputs_fail_open() {
        let calls = [
            Gtk3PangoCall::bytes([0xff], PangoAttributeProfile::None),
            Gtk3PangoCall::utf8("Open", PangoAttributeProfile::None).with_reentry_detected(),
            Gtk3PangoCall::utf8("Open", PangoAttributeProfile::None).with_excessive_length(),
        ];
        for call in calls {
            let original_bytes = call.bytes.clone();
            let mut adapter = Gtk3PangoInlineAdapter::new();
            let prepared = adapter.invoke(call, |_| Ok(replace("打开")), |prepared| prepared);
            assert_eq!(prepared.bytes(), original_bytes);
            assert!(prepared.is_original());
        }
    }

    #[test]
    fn observe_only_activation_and_callback_failure_preserve_original() {
        let grant = ActivationGrant::new([Feature::TextObserve]);
        let mut adapter = Gtk3PangoInlineAdapter::activate([Feature::TextObserve], &grant)
            .expect("observe-only activation");
        let observed = adapter.invoke(
            Gtk3PangoCall::utf8("Open", PangoAttributeProfile::None),
            |_| Ok(replace("打开")),
            |prepared| prepared,
        );
        assert!(observed.is_original());

        let mut adapter = Gtk3PangoInlineAdapter::new();
        let failed = adapter.invoke(
            Gtk3PangoCall::utf8("Open", PangoAttributeProfile::None),
            |_| panic!("host callback panic"),
            |prepared| prepared,
        );
        assert!(failed.is_original());
    }
}
