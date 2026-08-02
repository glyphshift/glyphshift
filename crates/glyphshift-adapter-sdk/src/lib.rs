//! Host-independent contracts implemented by Capability Adapters.

use glyphshift_domain::{AbiVersion, AdapterId, ApplyModel, Feature, Placement};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivationGrant {
    features: BTreeSet<Feature>,
}

impl ActivationGrant {
    #[must_use]
    pub fn new(features: impl IntoIterator<Item = Feature>) -> Self {
        Self {
            features: features.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn allows(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterError {
    UnauthorizedFeature(Feature),
    DecisionUnavailable,
    InvalidText,
    Reentry,
    ObjectAlreadyExists,
    ObjectNotFound,
    ObservationExpired,
    Disconnected,
}

pub fn authorize(
    requested: impl IntoIterator<Item = Feature>,
    grant: &ActivationGrant,
) -> Result<Vec<Feature>, AdapterError> {
    let features: BTreeSet<_> = requested.into_iter().collect();
    if let Some(feature) = features.iter().find(|feature| !grant.allows(**feature)) {
        return Err(AdapterError::UnauthorizedFeature(*feature));
    }
    Ok(features.into_iter().collect())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdapterVersion {
    major: u16,
    minor: u16,
    patch: u16,
}

impl AdapterVersion {
    #[must_use]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }

    #[must_use]
    pub const fn patch(self) -> u16 {
        self.patch
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterDescriptor {
    adapter_id: AdapterId,
    version: AdapterVersion,
    apply_model: ApplyModel,
    placement: Placement,
    features: BTreeSet<Feature>,
    architectures: BTreeSet<Box<str>>,
    abi: AbiVersion,
}

impl AdapterDescriptor {
    #[must_use]
    pub fn new(
        adapter_id: AdapterId,
        version: AdapterVersion,
        apply_model: ApplyModel,
        placement: Placement,
        features: impl IntoIterator<Item = Feature>,
    ) -> Self {
        Self {
            adapter_id,
            version,
            apply_model,
            placement,
            features: features.into_iter().collect(),
            architectures: BTreeSet::new(),
            abi: AbiVersion::new(1, 0),
        }
    }

    #[must_use]
    pub fn with_architectures(
        mut self,
        architectures: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.architectures = architectures.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub const fn with_abi(mut self, abi: AbiVersion) -> Self {
        self.abi = abi;
        self
    }

    #[must_use]
    pub const fn adapter_id(&self) -> &AdapterId {
        &self.adapter_id
    }

    #[must_use]
    pub const fn version(&self) -> AdapterVersion {
        self.version
    }

    #[must_use]
    pub const fn apply_model(&self) -> ApplyModel {
        self.apply_model
    }

    #[must_use]
    pub const fn placement(&self) -> Placement {
        self.placement
    }

    pub fn features(&self) -> impl Iterator<Item = Feature> + '_ {
        self.features.iter().copied()
    }

    pub fn architectures(&self) -> impl Iterator<Item = &str> {
        self.architectures.iter().map(AsRef::as_ref)
    }

    #[must_use]
    pub const fn abi(&self) -> AbiVersion {
        self.abi
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawCommand {
    text: Box<str>,
    font: Box<str>,
}

impl DrawCommand {
    #[must_use]
    pub fn new(text: impl Into<Box<str>>, font: impl Into<Box<str>>) -> Self {
        Self {
            text: text.into(),
            font: font.into(),
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn font(&self) -> &str {
        &self.font
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineTextInput {
    units: Vec<u16>,
    original: DrawCommand,
    excessive_length: bool,
    reentry_detected: bool,
}

impl InlineTextInput {
    #[must_use]
    pub fn utf16(units: impl IntoIterator<Item = u16>, font: impl Into<Box<str>>) -> Self {
        let units: Vec<_> = units.into_iter().collect();
        let original_text = String::from_utf16(&units).unwrap_or_else(|_| String::from("Open"));
        Self {
            units,
            original: DrawCommand::new(original_text, font),
            excessive_length: false,
            reentry_detected: false,
        }
    }

    #[must_use]
    pub fn invalid_utf16(font: impl Into<Box<str>>) -> Self {
        Self {
            units: vec![0xd800],
            original: DrawCommand::new("Open", font),
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

    #[must_use]
    pub const fn original(&self) -> &DrawCommand {
        &self.original
    }

    pub fn decode(&self) -> Result<String, AdapterError> {
        if self.excessive_length {
            return Err(AdapterError::InvalidText);
        }
        if self.reentry_detected {
            return Err(AdapterError::Reentry);
        }
        String::from_utf16(&self.units).map_err(|_| AdapterError::InvalidText)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId {
    key: Box<str>,
    revision: u64,
}

impl ObjectId {
    #[must_use]
    pub fn new(key: impl Into<Box<str>>, revision: u64) -> Self {
        Self {
            key: key.into(),
            revision,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObservationId(u64);

impl ObservationId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntheticTarget {
    pixels: Vec<Box<str>>,
    objects: Vec<ObjectId>,
    subscriptions: Vec<Box<str>>,
}

impl SyntheticTarget {
    #[must_use]
    pub fn new(
        pixels: impl IntoIterator<Item = impl Into<Box<str>>>,
        objects: impl IntoIterator<Item = ObjectId>,
        subscriptions: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            pixels: pixels.into_iter().map(Into::into).collect(),
            objects: objects.into_iter().collect(),
            subscriptions: subscriptions.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn evidence_size(&self) -> usize {
        self.pixels.len() + self.objects.len() + self.subscriptions.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeEvidence {
    apply_model: ApplyModel,
    placement: Placement,
    features: Vec<Feature>,
    evidence_size: usize,
}

impl ProbeEvidence {
    #[must_use]
    pub fn new(
        apply_model: ApplyModel,
        placement: Placement,
        features: impl IntoIterator<Item = Feature>,
        target: &SyntheticTarget,
    ) -> Self {
        let features: BTreeSet<_> = features.into_iter().collect();
        Self {
            apply_model,
            placement,
            features: features.into_iter().collect(),
            evidence_size: target.evidence_size(),
        }
    }

    #[must_use]
    pub fn features(&self) -> &[Feature] {
        &self.features
    }
}
