//! Portable Dictionary `/3` package codec and validation.

use serde::{Deserialize, Deserializer, Serialize};
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

fn deserialize_string_or_empty<'de, D>(deserializer: D) -> Result<Box<str>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value.as_str().unwrap_or_default().into())
}

fn deserialize_release_version<'de, D>(deserializer: D) -> Result<Box<str>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value.as_str().unwrap_or("0.1.0").into())
}

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<Box<str>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value.as_str().map(Into::into))
}

fn deserialize_string_list<'de, D>(deserializer: D) -> Result<Vec<Box<str>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(Into::into))
        .collect())
}

fn deserialize_revision<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value.as_u64().filter(|revision| *revision > 0).unwrap_or(1))
}

fn deserialize_entries<'de, D>(deserializer: D) -> Result<Vec<DictionaryEntryArtifact>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| serde_json::from_value(entry.clone()).ok())
        .collect())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct DictionaryMetadata {
    id: Box<str>,
    #[serde(
        default = "default_release_version",
        deserialize_with = "deserialize_release_version"
    )]
    release_version: Box<str>,
    name: Box<str>,
    #[serde(default, deserialize_with = "deserialize_string_or_empty")]
    description: Box<str>,
    source_locale: Box<str>,
    target_locale: Box<str>,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    authors: Vec<Box<str>>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    license: Option<Box<str>>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    homepage: Option<Box<str>>,
    #[serde(default, deserialize_with = "deserialize_string_list")]
    tags: Vec<Box<str>>,
    #[serde(default)]
    font_families: Vec<Box<str>>,
    #[serde(default)]
    font_scale_percent: Option<u16>,
}

impl DictionaryMetadata {
    #[must_use]
    pub fn font_scale_percent(&self) -> Option<u16> {
        self.font_scale_percent
    }
    #[must_use]
    pub fn font_families(&self) -> &[Box<str>] {
        &self.font_families
    }

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
#[serde(rename_all = "camelCase")]
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
                font_families: Vec::new(),
                font_scale_percent: None,
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
    pub fn with_font_scale_percent(mut self, percent: Option<u16>) -> Self {
        self.metadata.font_scale_percent = percent;
        self
    }

    #[must_use]
    pub fn with_font_families(
        mut self,
        families: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.metadata.font_families = families.into_iter().map(Into::into).collect();
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
#[serde(rename_all = "camelCase")]
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
    pub fn with_font_scale_percent(mut self, percent: Option<u16>) -> Self {
        self.metadata.font_scale_percent = percent;
        self
    }

    #[must_use]
    pub fn with_font_families(
        mut self,
        families: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.metadata.font_families = families.into_iter().map(Into::into).collect();
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
#[serde(rename_all = "camelCase")]
struct DictionaryArtifact {
    schema: Box<str>,
    #[serde(
        default = "default_revision",
        deserialize_with = "deserialize_revision"
    )]
    revision: u64,
    metadata: DictionaryMetadata,
    #[serde(default, deserialize_with = "deserialize_entries")]
    entries: Vec<DictionaryEntryArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DictionaryEntryArtifact {
    source: Box<str>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    translation: Option<Box<str>>,
}

fn default_release_version() -> Box<str> {
    "0.1.0".into()
}

const fn default_revision() -> u64 {
    1
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

    pub fn decode_local_json(
        source: &str,
        expected_id: Option<&str>,
    ) -> Result<(Self, bool), PackageError> {
        let mut value: serde_json::Value =
            serde_json::from_str(source).map_err(|_| PackageError::InvalidJson)?;
        let mut repaired_font = false;
        if let Some(metadata) = value
            .get_mut("metadata")
            .and_then(serde_json::Value::as_object_mut)
        {
            if let Some(fonts) = metadata.get("fontFamilies") {
                let valid = fonts.as_array().is_some_and(|fonts| {
                    fonts.len() <= 16
                        && fonts.iter().all(|font| {
                            font.as_str().is_some_and(|font| {
                                !font.trim().is_empty()
                                    && font.chars().count() <= 128
                                    && !font.contains('\0')
                            })
                        })
                        && fonts
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .collect::<BTreeSet<_>>()
                            .len()
                            == fonts.len()
                });
                if !valid {
                    metadata.remove("fontFamilies");
                    repaired_font = true;
                }
            }
            if metadata.get("fontScalePercent").is_some_and(|percent| {
                !percent.is_null()
                    && !percent
                        .as_u64()
                        .is_some_and(|percent| (50..=200).contains(&percent))
            }) {
                metadata.remove("fontScalePercent");
                repaired_font = true;
            }
        }
        let mut artifact: DictionaryArtifact =
            serde_json::from_value(value).map_err(|_| PackageError::InvalidJson)?;
        let migrated = artifact.schema.as_ref() != DICTIONARY_SCHEMA || repaired_font;
        artifact.schema = DICTIONARY_SCHEMA.into();
        validate_dictionary(&artifact, expected_id)?;
        Ok((Self { artifact }, migrated))
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
        || artifact
            .metadata
            .font_scale_percent()
            .is_some_and(|percent| !(50..=200).contains(&percent))
        || artifact.metadata.font_families().len() > 16
        || artifact.metadata.font_families().iter().any(|family| {
            family.trim().is_empty() || family.chars().count() > 128 || family.contains('\0')
        })
        || artifact
            .metadata
            .font_families()
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != artifact.metadata.font_families().len()
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
    fn package_ignores_unknown_entry_fields_without_losing_the_dictionary() {
        let obsolete = r#"{
            "schema":"glyphshift.dictionary/3",
            "revision":1,
            "metadata":{
                "id":"dictionary.ui",
                "releaseVersion":"0.1.0",
                "name":"UI",
                "description":42,
                "sourceLocale":"en-US",
                "targetLocale":"zh-CN",
                "authors":["Fixture",42],
                "license":null,
                "homepage":null,
                "tags":"invalid"
            },
            "entries":[{
                "location":"main-ui",
                "context":null,
                "source":"Open",
                "text":{"kind":"replace","text":"打开"}
            }]
        }"#;

        let package = DictionaryPackage::decode_json(obsolete, Some("dictionary.ui"))
            .expect("unknown entry fields do not invalidate the dictionary");
        assert_eq!(package.view().entries().len(), 1);
        assert_eq!(package.view().entries()[0].source(), "Open");
        assert!(package.view().entries()[0].is_pending());
        assert_eq!(package.view().metadata().description(), "");
        assert_eq!(
            package.view().metadata().authors(),
            &[Box::<str>::from("Fixture")]
        );
        assert!(package.view().metadata().tags().is_empty());
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
    #[test]
    fn local_invalid_font_preferences_preserve_entries_but_import_rejects_them() {
        let package = DictionaryPackage::create(
            DictionaryCreate::new("dictionary.font", "Font", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "Translated")]),
        )
        .unwrap();
        let mut value: serde_json::Value =
            serde_json::from_str(&package.encode_json().unwrap()).unwrap();
        value["metadata"]["fontFamilies"] = serde_json::json!([123]);
        value["metadata"]["fontScalePercent"] = serde_json::json!(0);
        let json = value.to_string();
        assert!(DictionaryPackage::decode_json(&json, None).is_err());
        let (recovered, changed) = DictionaryPackage::decode_local_json(&json, None).unwrap();
        assert!(changed);
        assert_eq!(
            recovered.view().entries()[0].translation(),
            Some("Translated")
        );
        assert!(recovered.view().metadata().font_families().is_empty());
        assert_eq!(recovered.view().metadata().font_scale_percent(), None);
    }
}
