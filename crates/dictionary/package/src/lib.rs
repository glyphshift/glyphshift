//! Portable Dictionary `/3` package codec and validation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const DICTIONARY_SCHEMA: &str = "glyphshift.dictionary/3";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageError {
    InvalidJson,
    InvalidContract,
    Serialization,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DictionaryMutationError {
    RevisionConflict { current: u64 },
    UnknownEntry,
    DuplicateEntry,
    EmptySelection,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DictionaryEntryCreate {
    source: Box<str>,
    #[serde(default)]
    translation: Option<Box<str>>,
}

impl DictionaryEntryCreate {
    #[must_use]
    pub fn new(source: impl Into<Box<str>>, translation: impl Into<Box<str>>) -> Self {
        Self {
            source: source.into(),
            translation: Some(translation.into()),
        }
    }

    #[must_use]
    pub fn pending(source: impl Into<Box<str>>) -> Self {
        Self {
            source: source.into(),
            translation: None,
        }
    }

    fn key(&self) -> Box<str> {
        self.source.clone()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DictionaryMetadata {
    id: Box<str>,
    release_version: Box<str>,
    name: Box<str>,
    description: Box<str>,
    source_locale: Box<str>,
    target_locale: Box<str>,
    authors: Vec<Box<str>>,
    license: Option<Box<str>>,
    homepage: Option<Box<str>>,
    tags: Vec<Box<str>>,
}

impl DictionaryMetadata {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn release_version(&self) -> &str {
        &self.release_version
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn source_locale(&self) -> &str {
        &self.source_locale
    }

    #[must_use]
    pub fn target_locale(&self) -> &str {
        &self.target_locale
    }

    #[must_use]
    pub fn authors(&self) -> &[Box<str>] {
        &self.authors
    }

    #[must_use]
    pub fn license(&self) -> Option<&str> {
        self.license.as_deref()
    }

    #[must_use]
    pub fn homepage(&self) -> Option<&str> {
        self.homepage.as_deref()
    }

    #[must_use]
    pub fn tags(&self) -> &[Box<str>] {
        &self.tags
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DictionaryCreate {
    metadata: DictionaryMetadata,
    entries: Vec<DictionaryEntryCreate>,
}

impl DictionaryCreate {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        source_locale: impl Into<Box<str>>,
        target_locale: impl Into<Box<str>>,
    ) -> Self {
        Self {
            metadata: DictionaryMetadata {
                id: id.into(),
                release_version: "0.1.0".into(),
                name: name.into(),
                description: "".into(),
                source_locale: source_locale.into(),
                target_locale: target_locale.into(),
                authors: Vec::new(),
                license: None,
                homepage: None,
                tags: Vec::new(),
            },
            entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        self.metadata.id()
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.metadata.description = description.into();
        self
    }

    #[must_use]
    pub fn with_release_version(mut self, version: impl Into<Box<str>>) -> Self {
        self.metadata.release_version = version.into();
        self
    }

    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_authors(mut self, authors: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.authors = authors.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_license(mut self, license: impl Into<Box<str>>) -> Self {
        self.metadata.license = Some(license.into());
        self
    }

    #[must_use]
    pub fn with_homepage(mut self, homepage: impl Into<Box<str>>) -> Self {
        self.metadata.homepage = Some(homepage.into());
        self
    }

    #[must_use]
    pub fn with_entries(
        mut self,
        entries: impl IntoIterator<Item = DictionaryEntryCreate>,
    ) -> Self {
        self.entries = entries.into_iter().collect();
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DictionaryEdit {
    metadata: DictionaryMetadata,
    base_revision: u64,
    entries: Vec<DictionaryEntryCreate>,
}

impl DictionaryEdit {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        source_locale: impl Into<Box<str>>,
        target_locale: impl Into<Box<str>>,
        base_revision: u64,
    ) -> Self {
        let create = DictionaryCreate::new(id, name, source_locale, target_locale);
        Self {
            metadata: create.metadata,
            base_revision,
            entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        self.metadata.id()
    }

    #[must_use]
    pub const fn base_revision(&self) -> u64 {
        self.base_revision
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.metadata.description = description.into();
        self
    }

    #[must_use]
    pub fn with_release_version(mut self, version: impl Into<Box<str>>) -> Self {
        self.metadata.release_version = version.into();
        self
    }

    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_authors(mut self, authors: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.authors = authors.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_license(mut self, license: impl Into<Box<str>>) -> Self {
        self.metadata.license = Some(license.into());
        self
    }

    #[must_use]
    pub fn with_homepage(mut self, homepage: impl Into<Box<str>>) -> Self {
        self.metadata.homepage = Some(homepage.into());
        self
    }

    #[must_use]
    pub fn with_entries(
        mut self,
        entries: impl IntoIterator<Item = DictionaryEntryCreate>,
    ) -> Self {
        self.entries = entries.into_iter().collect();
        self
    }

    fn from_view(
        view: &DictionaryView,
        entries: impl IntoIterator<Item = DictionaryEntryCreate>,
    ) -> Self {
        Self {
            metadata: view.metadata.clone(),
            base_revision: view.revision,
            entries: entries.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryEntryView {
    source: Box<str>,
    translation: Option<Box<str>>,
}

impl DictionaryEntryView {
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translation(&self) -> Option<&str> {
        self.translation.as_deref()
    }

    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.translation.is_none()
    }

    fn key(&self) -> Box<str> {
        self.source.clone()
    }

    fn to_create(&self) -> DictionaryEntryCreate {
        DictionaryEntryCreate {
            source: self.source.clone(),
            translation: self.translation.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryView {
    metadata: DictionaryMetadata,
    revision: u64,
    entries: Vec<DictionaryEntryView>,
}

impl DictionaryView {
    #[must_use]
    pub const fn metadata(&self) -> &DictionaryMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn id(&self) -> &str {
        self.metadata.id()
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn entries(&self) -> &[DictionaryEntryView] {
        &self.entries
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DictionaryArtifact {
    schema: Box<str>,
    revision: u64,
    metadata: DictionaryMetadata,
    entries: Vec<DictionaryEntryArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DictionaryEntryArtifact {
    source: Box<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    translation: Option<Box<str>>,
}

#[derive(Clone, Debug)]
pub struct DictionaryPackage {
    artifact: DictionaryArtifact,
}

impl DictionaryPackage {
    pub fn create(create: DictionaryCreate) -> Result<Self, PackageError> {
        Self::at_revision(create, 1)
    }

    pub fn from_edit(edit: DictionaryEdit, revision: u64) -> Result<Self, PackageError> {
        Self::at_revision(
            DictionaryCreate {
                metadata: edit.metadata,
                entries: edit.entries,
            },
            revision,
        )
    }

    pub fn at_revision(create: DictionaryCreate, revision: u64) -> Result<Self, PackageError> {
        let artifact = DictionaryArtifact {
            schema: DICTIONARY_SCHEMA.into(),
            revision,
            metadata: create.metadata,
            entries: create
                .entries
                .into_iter()
                .map(|entry| DictionaryEntryArtifact {
                    source: entry.source,
                    translation: entry.translation,
                })
                .collect(),
        };
        validate_dictionary(&artifact, None)?;
        Ok(Self { artifact })
    }

    pub fn decode_json(source: &str, expected_id: Option<&str>) -> Result<Self, PackageError> {
        let artifact = serde_json::from_str(source).map_err(|_| PackageError::InvalidJson)?;
        validate_dictionary(&artifact, expected_id)?;
        Ok(Self { artifact })
    }

    pub fn encode_json(&self) -> Result<String, PackageError> {
        serde_json::to_string(&self.artifact).map_err(|_| PackageError::Serialization)
    }

    #[must_use]
    pub fn id(&self) -> &str {
        self.artifact.metadata.id()
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.artifact.revision
    }

    #[must_use]
    pub fn view(&self) -> DictionaryView {
        DictionaryView {
            metadata: self.artifact.metadata.clone(),
            revision: self.artifact.revision,
            entries: self
                .artifact
                .entries
                .iter()
                .map(|entry| DictionaryEntryView {
                    source: entry.source.clone(),
                    translation: entry.translation.clone(),
                })
                .collect(),
        }
    }
}

pub fn prepare_entry_upsert(
    current: &DictionaryView,
    entry: DictionaryEntryCreate,
    replacing_source: Option<&str>,
    base_revision: u64,
) -> Result<DictionaryEdit, DictionaryMutationError> {
    if current.revision != base_revision {
        return Err(DictionaryMutationError::RevisionConflict {
            current: current.revision,
        });
    }
    let mut entries = current
        .entries
        .iter()
        .map(DictionaryEntryView::to_create)
        .collect::<Vec<_>>();
    let next_key = entry.key();
    let index = replacing_source.and_then(|source| {
        entries
            .iter()
            .position(|entry| entry.source.as_ref() == source)
    });
    if replacing_source.is_some() && index.is_none() {
        return Err(DictionaryMutationError::UnknownEntry);
    }
    if entries
        .iter()
        .enumerate()
        .any(|(candidate_index, entry)| Some(candidate_index) != index && entry.key() == next_key)
    {
        return Err(DictionaryMutationError::DuplicateEntry);
    }
    if let Some(index) = index {
        entries[index] = entry;
    } else {
        entries.push(entry);
    }
    Ok(DictionaryEdit::from_view(current, entries))
}

pub fn prepare_entry_deletion(
    current: &DictionaryView,
    sources: impl IntoIterator<Item = impl Into<Box<str>>>,
    base_revision: u64,
) -> Result<DictionaryEdit, DictionaryMutationError> {
    if current.revision != base_revision {
        return Err(DictionaryMutationError::RevisionConflict {
            current: current.revision,
        });
    }
    let sources = sources.into_iter().map(Into::into).collect::<BTreeSet<_>>();
    if sources.is_empty() {
        return Err(DictionaryMutationError::EmptySelection);
    }
    let existing_keys = current
        .entries
        .iter()
        .map(DictionaryEntryView::key)
        .collect::<BTreeSet<_>>();
    if !sources.is_subset(&existing_keys) {
        return Err(DictionaryMutationError::UnknownEntry);
    }
    let entries = current
        .entries
        .iter()
        .filter(|entry| !sources.contains(&entry.key()))
        .map(DictionaryEntryView::to_create)
        .collect::<Vec<_>>();
    Ok(DictionaryEdit::from_view(current, entries))
}

fn validate_dictionary(
    artifact: &DictionaryArtifact,
    expected_id: Option<&str>,
) -> Result<(), PackageError> {
    let valid_entries = artifact.entries.iter().all(|entry| {
        !entry.source.trim().is_empty()
            && entry
                .translation
                .as_deref()
                .is_none_or(|translation| !translation.trim().is_empty())
    });
    let unique_entries = artifact
        .entries
        .iter()
        .map(|entry| &entry.source)
        .collect::<BTreeSet<_>>()
        .len()
        == artifact.entries.len();
    if artifact.schema.as_ref() != DICTIONARY_SCHEMA
        || !safe_identifier(artifact.metadata.id())
        || !safe_identifier(artifact.metadata.source_locale())
        || !safe_identifier(artifact.metadata.target_locale())
        || artifact.metadata.release_version().trim().is_empty()
        || artifact.metadata.name().trim().is_empty()
        || artifact.metadata.name().chars().count() > 128
        || artifact.metadata.description().chars().count() > 512
        || artifact
            .metadata
            .authors()
            .iter()
            .any(|author| author.trim().is_empty())
        || artifact
            .metadata
            .authors()
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != artifact.metadata.authors().len()
        || artifact
            .metadata
            .license()
            .is_some_and(|license| license.trim().is_empty())
        || artifact
            .metadata
            .homepage()
            .is_some_and(|homepage| homepage.trim().is_empty())
        || artifact
            .metadata
            .tags()
            .iter()
            .any(|tag| tag.trim().is_empty())
        || artifact
            .metadata
            .tags()
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != artifact.metadata.tags().len()
        || artifact.revision == 0
        || expected_id.is_some_and(|expected| expected != artifact.metadata.id())
        || !valid_entries
        || !unique_entries
    {
        return Err(PackageError::InvalidContract);
    }
    Ok(())
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_round_trips_the_dictionary_v3_contract() {
        let package = DictionaryPackage::create(
            DictionaryCreate::new("dictionary.ui", "UI", "en-US", "zh-CN")
                .with_release_version("1.2.0")
                .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
        )
        .expect("create package");
        let encoded = package.encode_json().expect("encode package");
        let decoded = DictionaryPackage::decode_json(&encoded, Some("dictionary.ui"))
            .expect("decode package");

        assert!(encoded.contains("glyphshift.dictionary/3"));
        assert_eq!(decoded.id(), "dictionary.ui");
        assert_eq!(decoded.view().entries()[0].translation(), Some("打开"));
        assert!(encoded.contains("\"source\":\"Open\",\"translation\":\"打开\""));
        assert!(!encoded.contains("location"));
        assert!(!encoded.contains("context"));
        assert!(!encoded.contains("kind"));
    }

    #[test]
    fn package_round_trips_a_source_only_pending_entry() {
        let package = DictionaryPackage::create(
            DictionaryCreate::new("dictionary.pending", "Pending", "en-US", "zh-CN").with_entries(
                [
                    DictionaryEntryCreate::new("Open", "打开"),
                    DictionaryEntryCreate::pending("Save"),
                ],
            ),
        )
        .expect("create package with pending entry");

        let encoded = package.encode_json().expect("encode package");
        let reopened = DictionaryPackage::decode_json(&encoded, Some("dictionary.pending"))
            .expect("decode package with pending entry");

        assert_eq!(reopened.view().entries()[0].translation(), Some("打开"));
        assert_eq!(reopened.view().entries()[1].translation(), None);
        assert!(reopened.view().entries()[1].is_pending());
        assert!(encoded.contains(r#"{"source":"Save"}"#));
    }

    #[test]
    fn package_rejects_unknown_schema_and_filename_identity() {
        let source = DictionaryPackage::create(DictionaryCreate::new(
            "dictionary.ui",
            "UI",
            "en-US",
            "zh-CN",
        ))
        .expect("create package")
        .encode_json()
        .expect("encode package");
        let unknown_schema = source.replace("glyphshift.dictionary/3", "glyphshift.dictionary/2");

        assert_eq!(
            DictionaryPackage::decode_json(&unknown_schema, Some("dictionary.ui"))
                .expect_err("reject schema"),
            PackageError::InvalidContract
        );
        assert_eq!(
            DictionaryPackage::decode_json(&source, Some("dictionary.other"))
                .expect_err("reject identity"),
            PackageError::InvalidContract
        );
    }

    #[test]
    fn package_rejects_obsolete_location_context_and_text_behavior_fields() {
        let obsolete = r#"{
            "schema":"glyphshift.dictionary/3",
            "revision":1,
            "metadata":{
                "id":"dictionary.ui",
                "releaseVersion":"0.1.0",
                "name":"UI",
                "description":"",
                "sourceLocale":"en-US",
                "targetLocale":"zh-CN",
                "authors":[],
                "license":null,
                "homepage":null,
                "tags":[]
            },
            "entries":[{
                "location":"main-ui",
                "context":null,
                "source":"Open",
                "text":{"kind":"replace","text":"打开"}
            }]
        }"#;

        assert_eq!(
            DictionaryPackage::decode_json(obsolete, Some("dictionary.ui"))
                .expect_err("obsolete entry fields must not survive inside Dictionary /3"),
            PackageError::InvalidJson,
        );
    }

    #[test]
    fn entry_mutations_enforce_revision_identity_and_uniqueness() {
        let current = DictionaryPackage::create(
            DictionaryCreate::new("dictionary.ui", "UI", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
        )
        .expect("create package")
        .view();

        assert!(matches!(
            prepare_entry_upsert(
                &current,
                DictionaryEntryCreate::new("Close", "关闭"),
                None,
                2,
            ),
            Err(DictionaryMutationError::RevisionConflict { current: 1 })
        ));
        assert_eq!(
            prepare_entry_upsert(
                &current,
                DictionaryEntryCreate::new("Open", "开启"),
                None,
                1,
            )
            .expect_err("reject duplicate"),
            DictionaryMutationError::DuplicateEntry
        );
    }
}
