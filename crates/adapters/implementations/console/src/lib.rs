//! Host-independent contracts for observing Unicode console writes.

use glyphshift_adapter_sdk::{
    authorize, ActivationGrant, AdapterDescriptor, AdapterError, AdapterVersion,
};
use glyphshift_domain::{AdapterId, ApplyModel, Feature, Placement, RenderDecision};
use std::iter::Peekable;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::str::Chars;

pub const ADAPTER_ID: &str = "windows.console.write-console";
const MAX_TEXT_UNITS: usize = 16_384;

/// Removes terminal protocol sequences while preserving text that is actually visible.
///
/// Console clients such as line editors can send cursor movement, styling and hyperlink
/// instructions through `WriteConsoleW`. Those instructions are valid terminal protocol, but they
/// are not translation candidates.
#[must_use]
pub fn visible_console_text(source: &str) -> String {
    let mut visible = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\u{1b}' => consume_escape(&mut chars),
            '\u{009b}' => consume_csi(&mut chars),
            '\u{0090}' | '\u{0098}' | '\u{009d}' | '\u{009e}' | '\u{009f}' => {
                consume_control_string(&mut chars)
            }
            '\r' | '\n' => visible.push(character),
            '\t' => visible.push(' '),
            character if character.is_control() => {}
            character => visible.push(character),
        }
    }
    visible
}

fn consume_escape(chars: &mut Peekable<Chars<'_>>) {
    match chars.next() {
        Some('[') => consume_csi(chars),
        Some('P' | 'X' | ']' | '^' | '_') => consume_control_string(chars),
        Some(_) | None => {}
    }
}

fn consume_csi(chars: &mut Peekable<Chars<'_>>) {
    for character in chars.by_ref() {
        if ('\u{0040}'..='\u{007e}').contains(&character) {
            break;
        }
    }
}

fn consume_control_string(chars: &mut Peekable<Chars<'_>>) {
    while let Some(character) = chars.next() {
        match character {
            '\u{0007}' | '\u{009c}' => break,
            '\u{1b}' if chars.next_if_eq(&'\\').is_some() => break,
            _ => {}
        }
    }
}

#[must_use]
pub fn descriptor() -> AdapterDescriptor {
    AdapterDescriptor::new(
        AdapterId::new(ADAPTER_ID),
        AdapterVersion::new(1, 0, 0),
        ApplyModel::ObserveOnly,
        Placement::TargetProcess,
        [Feature::TextObserve],
    )
    .with_platforms(["windows"])
    .with_architectures(["x86", "x86_64"])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleWriteCall {
    units: Vec<u16>,
    excessive_length: bool,
    reentry_detected: bool,
}

impl ConsoleWriteCall {
    #[must_use]
    pub fn utf16(text: &str) -> Self {
        Self {
            units: text.encode_utf16().collect(),
            excessive_length: false,
            reentry_detected: false,
        }
    }

    #[must_use]
    pub fn invalid_utf16() -> Self {
        Self {
            units: vec![0xd800],
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
pub struct PreparedConsoleWrite {
    units: Vec<u16>,
    text: Option<Box<str>>,
}

impl PreparedConsoleWrite {
    #[must_use]
    pub fn units(&self) -> &[u16] {
        &self.units
    }

    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsoleWriteObserver {
    text_observe: bool,
}

impl ConsoleWriteObserver {
    #[must_use]
    pub const fn new() -> Self {
        Self { text_observe: true }
    }

    pub fn activate(
        requested: impl IntoIterator<Item = Feature>,
        grant: &ActivationGrant,
    ) -> Result<Self, AdapterError> {
        let active = authorize(requested, grant)?;
        Ok(Self {
            text_observe: active.contains(&Feature::TextObserve),
        })
    }

    pub fn invoke<R>(
        &mut self,
        call: ConsoleWriteCall,
        mut observe: impl FnMut(&str) -> Result<RenderDecision, AdapterError>,
        original: impl FnOnce(PreparedConsoleWrite) -> R,
    ) -> R {
        let prepared = PreparedConsoleWrite {
            text: String::from_utf16(&call.units).ok().map(Into::into),
            units: call.units,
        };
        if !self.text_observe
            || call.excessive_length
            || call.reentry_detected
            || prepared.units.len() > MAX_TEXT_UNITS
        {
            return original(prepared);
        }
        if let Some(source) = prepared.text.as_deref() {
            let visible = visible_console_text(source);
            for segment in visible
                .split(['\r', '\n'])
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
            {
                let _ = catch_unwind(AssertUnwindSafe(|| observe(segment)));
            }
        }
        original(prepared)
    }
}

impl Default for ConsoleWriteObserver {
    fn default() -> Self {
        Self::new()
    }
}
