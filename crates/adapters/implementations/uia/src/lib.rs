//! Host-independent UI Automation observation policy.

mod acquisition;

pub use acquisition::{
    UiaAcquisitionAdapter, UiaAcquisitionSnapshot, UiaSelectionSource, UiaTextSelection,
};

use glyphshift_adapter_sdk::{AdapterDescriptor, AdapterVersion};
use glyphshift_domain::{AdapterId, ApplyModel, Feature, Placement};
use std::collections::BTreeMap;

pub const ADAPTER_ID: &str = "windows.uia.observe";
pub const ACQUISITION_ADAPTER_ID: &str = "windows.uia.acquire";
const MAX_ELEMENT_KEY_BYTES: usize = 512;
const MAX_TEXT_UNITS: usize = 16 * 1024;

#[must_use]
pub fn descriptor() -> AdapterDescriptor {
    AdapterDescriptor::new(
        AdapterId::new(ADAPTER_ID),
        AdapterVersion::new(1, 0, 0),
        ApplyModel::ObserveOnly,
        Placement::IsolatedWorker,
        [Feature::TextObserve],
    )
    .with_platforms(["windows"])
    .with_architectures(["x86", "x86_64"])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum UiaTextChannel {
    Name,
    TextPattern,
    ValuePattern,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiaElementSnapshot {
    element_key: Box<str>,
    password: bool,
    name: Option<Box<str>>,
    text: Option<Box<str>>,
    value: Option<Box<str>>,
}

impl UiaElementSnapshot {
    #[must_use]
    pub fn new(element_key: impl Into<Box<str>>) -> Self {
        Self {
            element_key: element_key.into(),
            password: false,
            name: None,
            text: None,
            value: None,
        }
    }

    #[must_use]
    pub const fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    #[must_use]
    pub fn with_name(mut self, name: impl Into<Box<str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[must_use]
    pub fn with_text(mut self, text: impl Into<Box<str>>) -> Self {
        self.text = Some(text.into());
        self
    }

    #[must_use]
    pub fn with_value(mut self, value: impl Into<Box<str>>) -> Self {
        self.value = Some(value.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiaTextObservation {
    element_key: Box<str>,
    channel: UiaTextChannel,
    text: Box<str>,
}

impl UiaTextObservation {
    #[must_use]
    pub fn element_key(&self) -> &str {
        &self.element_key
    }

    #[must_use]
    pub const fn channel(&self) -> UiaTextChannel {
        self.channel
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiaIgnoreReason {
    InvalidElement,
    Sensitive,
    NoText,
    TextTooLong,
    Unchanged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiaObservationOutcome {
    Observed(UiaTextObservation),
    Ignored(UiaIgnoreReason),
}

#[derive(Default)]
pub struct UiaObserver {
    last_text: BTreeMap<(Box<str>, UiaTextChannel), Box<str>>,
}

impl UiaObserver {
    #[must_use]
    pub fn observe(&mut self, snapshot: UiaElementSnapshot) -> UiaObservationOutcome {
        if snapshot.element_key.trim().is_empty()
            || snapshot.element_key.len() > MAX_ELEMENT_KEY_BYTES
        {
            return UiaObservationOutcome::Ignored(UiaIgnoreReason::InvalidElement);
        }
        if snapshot.password {
            return UiaObservationOutcome::Ignored(UiaIgnoreReason::Sensitive);
        }
        let selected = [
            (UiaTextChannel::TextPattern, snapshot.text),
            (UiaTextChannel::ValuePattern, snapshot.value),
            (UiaTextChannel::Name, snapshot.name),
        ]
        .into_iter()
        .find_map(|(channel, text)| normalize(text.as_deref()).map(|text| (channel, text)));
        let Some((channel, text)) = selected else {
            return UiaObservationOutcome::Ignored(UiaIgnoreReason::NoText);
        };
        if text.encode_utf16().count() > MAX_TEXT_UNITS {
            return UiaObservationOutcome::Ignored(UiaIgnoreReason::TextTooLong);
        }
        let key = (snapshot.element_key.clone(), channel);
        if self
            .last_text
            .get(&key)
            .is_some_and(|previous| previous.as_ref() == text)
        {
            return UiaObservationOutcome::Ignored(UiaIgnoreReason::Unchanged);
        }
        let text: Box<str> = text.into();
        self.last_text.insert(key, text.clone());
        UiaObservationOutcome::Observed(UiaTextObservation {
            element_key: snapshot.element_key,
            channel,
            text,
        })
    }

    pub fn invalidate(&mut self, element_key: &str) {
        self.last_text
            .retain(|(observed_element, _), _| observed_element.as_ref() != element_key);
    }
}

fn normalize(text: Option<&str>) -> Option<String> {
    let text = text?.replace("\r\n", "\n").replace('\r', "\n");
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}
