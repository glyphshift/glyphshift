use super::{DictionaryPackage, PackageError};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Upload {
    schema: String,
    revision: u64,
    metadata: Metadata,
    entries: Vec<Entry>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Metadata {
    id: String,
    release_version: String,
    name: String,
    source_locale: String,
    target_locale: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    authors: Vec<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    font_families: Vec<String>,
    #[serde(default)]
    font_scale_percent: Option<u16>,
    #[serde(default)]
    text_rules: Vec<Rule>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    source: String,
    #[serde(default)]
    translation: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    pattern: String,
    replacement: String,
    enabled: bool,
}

pub(super) fn decode(
    source: &str,
    expected_id: Option<&str>,
) -> Result<DictionaryPackage, PackageError> {
    if source.len() > 16 * 1024 * 1024 {
        return Err(PackageError::InvalidContract);
    }
    // Typed deserialization rejects duplicate/unknown fields before the tolerant local codec.
    let upload: Upload = serde_json::from_str(source).map_err(|_| PackageError::InvalidJson)?;
    semver::Version::parse(&upload.metadata.release_version)
        .map_err(|_| PackageError::InvalidContract)?;
    let checked = serde_json::to_string(&upload).map_err(|_| PackageError::Serialization)?;
    DictionaryPackage::decode_json(&checked, expected_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DictionaryCreate, DictionaryEntryCreate};

    fn sample() -> String {
        DictionaryPackage::create(
            DictionaryCreate::new("fixture.ui", "Fixture", "en-US", "zh-CN").with_entries([
                DictionaryEntryCreate::new("Open", "打开"),
                DictionaryEntryCreate::pending("Save"),
            ]),
        )
        .unwrap()
        .encode_json()
        .unwrap()
    }

    #[test]
    fn publication_preserves_pending_and_translated_entries() {
        let p = decode(&sample(), Some("fixture.ui")).unwrap();
        assert_eq!(p.view().entries().len(), 2);
        assert!(p.view().entries()[1].is_pending());
    }

    #[test]
    fn publication_rejects_values_local_recovery_would_discard() {
        for bad in [
            sample().replace("\"translation\":\"打开\"", "\"translation\":42"),
            sample().replace("\"revision\":1", "\"revision\":null"),
            sample().replace("\"authors\":[]", "\"authors\":[17]"),
            sample().replace("\"releaseVersion\":\"0.1.0\"", "\"releaseVersion\":false"),
            sample().replace("\"entries\":[", "\"entries\":[null,"),
        ] {
            assert!(decode(&bad, None).is_err(), "{bad}");
        }
    }

    #[test]
    fn publication_rejects_duplicates_unknown_fields_and_invalid_semver() {
        for bad in [
            sample().replace("\"revision\":1", "\"revision\":1,\"revision\":2"),
            sample().replace(
                "\"source\":\"Open\"",
                "\"source\":\"Open\",\"context\":\"discarded\"",
            ),
            sample().replace(
                "\"releaseVersion\":\"0.1.0\"",
                "\"releaseVersion\":\"latest\"",
            ),
            sample().replace("glyphshift.dictionary/3", "glyphshift.dictionary/2"),
            sample().replace("\"source\":\"Save\"", "\"source\":\"Open\""),
        ] {
            assert!(decode(&bad, None).is_err(), "{bad}");
        }
    }

    #[test]
    fn publication_uses_runtime_regex_validation() {
        let bad = sample().replace(
            "\"textRules\":[]",
            "\"textRules\":[{\"pattern\":\"(\",\"replacement\":\"x\",\"enabled\":true}]",
        );
        assert!(decode(&bad, None).is_err());
    }
}
