//! Bounded display metadata. Discovery is not proof verification or installation authorization.
use crate::{Error, ReleaseRequest};
use glyphshift_dictionary_distribution::ArtifactPresentation;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct DictionarySearch {
    pub text: String,
    pub source_locale: Option<String>,
    pub target_locale: Option<String>,
    pub tag: Option<String>,
    pub cursor: Option<String>,
    pub size: u16,
}

impl Default for DictionarySearch {
    fn default() -> Self {
        Self {
            text: String::new(),
            source_locale: None,
            target_locale: None,
            tag: None,
            cursor: None,
            size: 50,
        }
    }
}

impl DictionarySearch {
    pub fn from_json(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > 8192 {
            return Err(Error::Configuration);
        }
        let query: Self = crate::strict_json::parse(bytes).map_err(|_| Error::Configuration)?;
        query.validate()?;
        Ok(query)
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.text.chars().count() > 256
            || !(1..=100).contains(&self.size)
            || self
                .source_locale
                .as_deref()
                .is_some_and(|s| !valid_locale(s))
            || self
                .target_locale
                .as_deref()
                .is_some_and(|s| !valid_locale(s))
            || self
                .tag
                .as_deref()
                .is_some_and(|s| s.trim().is_empty() || s.chars().count() > 32)
            || self.cursor.as_deref().is_some_and(|s| !valid_cursor(s))
        {
            return Err(Error::Configuration);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DictionaryEntry {
    pub package_id: String,
    pub version: String,
    pub publisher_user_key: String,
    pub name: String,
    pub summary: String,
    pub source_locale: String,
    pub target_locale: String,
    pub tags: Vec<String>,
}

impl DictionaryEntry {
    pub fn release_request(&self) -> Result<ReleaseRequest, Error> {
        ReleaseRequest::new(
            "dictionary",
            &self.package_id,
            &self.version,
            &self.publisher_user_key,
        )
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        self.release_request().map_err(|_| Error::InvalidArtifact)?;
        if !valid_locale(&self.source_locale) || !valid_locale(&self.target_locale) {
            return Err(Error::InvalidArtifact);
        }
        ArtifactPresentation::new(
            self.source_locale.as_str(),
            self.name.as_str(),
            self.summary.as_str(),
        )
        .and_then(|p| p.with_tags(self.tags.iter().map(String::as_str)))
        .map_err(|_| Error::InvalidArtifact)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DictionaryPage {
    pub items: Vec<DictionaryEntry>,
    pub next_cursor: Option<String>,
}

pub(crate) fn valid_cursor(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn valid_locale(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.split('-')
            .all(|p| !p.is_empty() && p.len() <= 8 && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}
