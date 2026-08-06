use crate::{DesktopPoint, DesktopRect};

const MAX_TARGET_TOKEN_BYTES: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuthorizedTarget(Box<str>);

impl AuthorizedTarget {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, AcquisitionError> {
        let value = value.into();
        if value.trim().is_empty() || value.len() > MAX_TARGET_TOKEN_BYTES {
            return Err(AcquisitionError::TargetMismatch);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractiveSelection {
    Point(DesktopPoint),
    TextRange {
        start: DesktopPoint,
        end: DesktopPoint,
    },
    Region(DesktopRect),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourcePolicy {
    Automatic,
    StructuredOnly,
    VisualOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcquisitionRequest {
    target: AuthorizedTarget,
    selection: InteractiveSelection,
    source_policy: SourcePolicy,
}

impl AcquisitionRequest {
    #[must_use]
    pub const fn new(
        target: AuthorizedTarget,
        selection: InteractiveSelection,
        source_policy: SourcePolicy,
    ) -> Self {
        Self {
            target,
            selection,
            source_policy,
        }
    }

    #[must_use]
    pub const fn target(&self) -> &AuthorizedTarget {
        &self.target
    }

    #[must_use]
    pub const fn selection(&self) -> InteractiveSelection {
        self.selection
    }

    #[must_use]
    pub const fn source_policy(&self) -> SourcePolicy {
        self.source_policy
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Granularity {
    Word,
    Control,
    Line,
    Region,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Provenance {
    Structured,
    Visual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Confidence(u16);

impl Confidence {
    pub const fn new(basis_points: u16) -> Option<Self> {
        if basis_points <= 10_000 {
            Some(Self(basis_points))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn basis_points(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcquisitionCandidate {
    source: Box<str>,
    anchors: Vec<DesktopRect>,
    granularity: Granularity,
    confidence: Option<Confidence>,
}

impl AcquisitionCandidate {
    #[must_use]
    pub fn new(
        source: impl Into<Box<str>>,
        anchors: impl IntoIterator<Item = DesktopRect>,
        granularity: Granularity,
    ) -> Self {
        Self {
            source: source.into(),
            anchors: anchors.into_iter().collect(),
            granularity,
            confidence: None,
        }
    }

    #[must_use]
    pub const fn with_confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = Some(confidence);
        self
    }

    pub(crate) fn into_parts(
        self,
    ) -> (Box<str>, Vec<DesktopRect>, Granularity, Option<Confidence>) {
        (self.source, self.anchors, self.granularity, self.confidence)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceBlock {
    source: Box<str>,
    anchors: Vec<DesktopRect>,
    granularity: Granularity,
    provenance: Provenance,
    confidence: Option<Confidence>,
}

impl SourceBlock {
    pub(crate) const fn new(
        source: Box<str>,
        anchors: Vec<DesktopRect>,
        granularity: Granularity,
        provenance: Provenance,
        confidence: Option<Confidence>,
    ) -> Self {
        Self {
            source,
            anchors,
            granularity,
            provenance,
            confidence,
        }
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn anchors(&self) -> &[DesktopRect] {
        &self.anchors
    }

    #[must_use]
    pub const fn granularity(&self) -> Granularity {
        self.granularity
    }

    #[must_use]
    pub const fn provenance(&self) -> Provenance {
        self.provenance
    }

    #[must_use]
    pub const fn confidence(&self) -> Option<Confidence> {
        self.confidence
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcquisitionResult {
    blocks: Vec<SourceBlock>,
}

impl AcquisitionResult {
    pub(crate) const fn new(blocks: Vec<SourceBlock>) -> Self {
        Self { blocks }
    }

    #[must_use]
    pub fn blocks(&self) -> &[SourceBlock] {
        &self.blocks
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcquisitionError {
    TargetMismatch,
    PermissionDenied,
    NoText,
    ProviderUnavailable,
    TimedOut,
    Cancelled,
}
