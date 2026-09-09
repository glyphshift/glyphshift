//! Host-independent value types for Glyphshift.

use std::sync::Arc;
mod deferred_text;
pub use deferred_text::{DeferredGlyph, DeferredTextCommit, DeferredTextDraw, GlyphStyle};
mod source_text;
pub use source_text::SourceTextPolicy;
mod text_run;
pub use text_run::{
    ResolvedText, TextRunEvent, TextRunKey, TextRunOutcome, TextRunResolver, TextUse,
    MAX_TEXT_RUN_UNITS,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdapterId(Box<str>);

impl AdapterId {
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Feature {
    TextObserve,
    TextReplace,
    FontSubstitute,
    FontScale,
    LayoutAdjust,
    ResourceReplace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyModel {
    InlineRender,
    RetainedObject,
    ExternalProtocol,
    ObserveOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    TargetProcess,
    IsolatedWorker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbiVersion {
    major: u16,
    minor: u16,
}

impl AbiVersion {
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistryRevision(u64);

impl RegistryRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetFacts {
    operating_system: Box<str>,
    architecture: Box<str>,
}

impl TargetFacts {
    #[must_use]
    pub fn new(operating_system: impl Into<Box<str>>, architecture: impl Into<Box<str>>) -> Self {
        Self {
            operating_system: operating_system.into(),
            architecture: architecture.into(),
        }
    }

    #[must_use]
    pub fn operating_system(&self) -> &str {
        &self.operating_system
    }

    #[must_use]
    pub fn architecture(&self) -> &str {
        &self.architecture
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextObservation {
    adapter_id: Box<str>,
    source_text: Box<str>,
    surface_token: Box<str>,
    context_heading: Option<ObservationContext>,
}

impl TextObservation {
    #[must_use]
    pub fn new(
        adapter_id: impl Into<Box<str>>,
        source_text: impl Into<Box<str>>,
        surface_token: impl Into<Box<str>>,
    ) -> Self {
        Self {
            adapter_id: adapter_id.into(),
            source_text: source_text.into(),
            surface_token: surface_token.into(),
            context_heading: None,
        }
    }

    #[must_use]
    pub fn with_context_heading(
        mut self,
        kind: impl Into<Box<str>>,
        key: impl Into<Box<str>>,
        label: impl Into<Box<str>>,
    ) -> Self {
        self.context_heading = Some(ObservationContext::new(kind, key, label));
        self
    }

    #[must_use]
    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }

    #[must_use]
    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    #[must_use]
    pub fn surface_token(&self) -> &str {
        &self.surface_token
    }

    #[must_use]
    pub const fn context_heading(&self) -> Option<&ObservationContext> {
        self.context_heading.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservationContext {
    kind: Box<str>,
    key: Box<str>,
    label: Box<str>,
}

impl ObservationContext {
    #[must_use]
    pub fn new(
        kind: impl Into<Box<str>>,
        key: impl Into<Box<str>>,
        label: impl Into<Box<str>>,
    ) -> Self {
        Self {
            kind: kind.into(),
            key: key.into(),
            label: label.into(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteProgram {
    operators: Vec<RouteOperator>,
    limits: RouteLimits,
}

impl RouteProgram {
    #[must_use]
    pub fn direct(location: impl Into<Box<str>>) -> Self {
        Self {
            operators: vec![RouteOperator::direct(location)],
            limits: RouteLimits::new(32, 64),
        }
    }

    #[must_use]
    pub fn new(operators: impl IntoIterator<Item = RouteOperator>, limits: RouteLimits) -> Self {
        Self {
            operators: operators.into_iter().collect(),
            limits,
        }
    }

    #[must_use]
    pub fn operators(&self) -> &[RouteOperator] {
        &self.operators
    }

    #[must_use]
    pub const fn limits(&self) -> RouteLimits {
        self.limits
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteLimits {
    max_state_entries: u16,
    max_steps: u16,
}

impl RouteLimits {
    #[must_use]
    pub const fn new(max_state_entries: u16, max_steps: u16) -> Self {
        Self {
            max_state_entries,
            max_steps,
        }
    }

    #[must_use]
    pub const fn max_state_entries(self) -> u16 {
        self.max_state_entries
    }

    #[must_use]
    pub const fn max_steps(self) -> u16 {
        self.max_steps
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteOperator {
    Direct {
        location: Box<str>,
    },
    Fallback {
        locations: Vec<Box<str>>,
    },
    ContextualHeading {
        location: Box<str>,
        context_kind: Box<str>,
    },
    Unknown {
        operator: Box<str>,
    },
    NativeCode,
    Script,
    Io,
}

impl RouteOperator {
    #[must_use]
    pub fn direct(location: impl Into<Box<str>>) -> Self {
        Self::Direct {
            location: location.into(),
        }
    }

    #[must_use]
    pub fn fallback(locations: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        Self::Fallback {
            locations: locations.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn contextual_heading(
        location: impl Into<Box<str>>,
        context_kind: impl Into<Box<str>>,
    ) -> Self {
        Self::ContextualHeading {
            location: location.into(),
            context_kind: context_kind.into(),
        }
    }

    #[must_use]
    pub fn unknown(operator: impl Into<Box<str>>) -> Self {
        Self::Unknown {
            operator: operator.into(),
        }
    }

    #[must_use]
    pub const fn native_code() -> Self {
        Self::NativeCode
    }

    #[must_use]
    pub const fn script() -> Self {
        Self::Script
    }

    #[must_use]
    pub const fn io() -> Self {
        Self::Io
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextDecision {
    Keep,
    Replace(Arc<str>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontDecision {
    Scaled {
        family: Option<Arc<str>>,
        percent: u16,
    },
    Keep,
    Substitute(Arc<str>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderDecision {
    pub text: TextDecision,
    pub font: FontDecision,
    pub generation: Generation,
}
