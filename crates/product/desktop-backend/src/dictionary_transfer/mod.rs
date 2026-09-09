//! Portable dictionary file formats. No filesystem, UI, or workflow mutations.
mod csv;
mod json;
mod srt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Serialize)]
pub struct TransferError {
    pub code: String,
    pub args: BTreeMap<String, u64>,
}
impl TransferError {
    fn new(code: &str) -> Self {
        Self {
            code: code.into(),
            args: BTreeMap::new(),
        }
    }
    fn with_arg(mut self, key: &str, value: u64) -> Self {
        self.args.insert(key.into(), value);
        self
    }
}
type Decode = fn(&str) -> Result<Vec<(usize, Entry)>, TransferError>;
type Encode = fn(&[Entry], Option<&serde_json::Value>) -> Result<String, TransferError>;
struct Handler {
    extension: &'static str,
    decode: Decode,
    encode: Option<Encode>,
    metadata: fn(&str) -> Result<Option<serde_json::Value>, TransferError>,
}
const HANDLERS: &[Handler] = &[
    Handler {
        extension: "json",
        decode: json::decode,
        encode: Some(json::encode),
        metadata: json::metadata,
    },
    Handler {
        extension: "csv",
        decode: csv::decode,
        encode: Some(csv::encode),
        metadata: |_| Ok(None),
    },
    Handler {
        extension: "srt",
        decode: srt::decode,
        encode: None,
        metadata: |_| Ok(None),
    },
];
fn handler(extension: &str) -> Result<&'static Handler, TransferError> {
    HANDLERS
        .iter()
        .find(|handler| handler.extension.eq_ignore_ascii_case(extension))
        .ok_or_else(|| TransferError::new("import.extension"))
}
pub fn encode_entries(
    extension: &str,
    entries: &[Entry],
    metadata: Option<&serde_json::Value>,
) -> Result<String, TransferError> {
    let encode = handler(extension)?
        .encode
        .ok_or_else(|| TransferError::new("export.unsupported"))?;
    encode(entries, metadata)
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entry {
    pub source: String,
    #[serde(default, deserialize_with = "translation_or_empty")]
    pub translation: String,
}

fn translation_or_empty<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn at_line(code: &str, line: usize) -> TransferError {
    TransferError::new(code).with_arg("line", line as u64)
}

fn json_error(error: serde_json::Error) -> TransferError {
    at_line("import.json_syntax", error.line()).with_arg("column", error.column() as u64)
}

pub fn parse_entries(text: &str, format: &str) -> Result<Vec<Entry>, TransferError> {
    let text = text.trim_start_matches('\u{feff}');
    let handler = handler(format)?;
    let entries = (handler.decode)(text)?;
    if entries.is_empty() {
        return Err(TransferError::new("import.empty"));
    }
    let mut sources = BTreeMap::<String, (String, usize)>::new();
    let mut unique = Vec::new();
    for (position, entry) in entries {
        let csv = format.eq_ignore_ascii_case("csv");
        if entry.source.trim().is_empty() {
            return Err(TransferError::new(if csv {
                "import.csv_source_empty"
            } else {
                "import.json_source_empty"
            })
            .with_arg(if csv { "line" } else { "entry" }, position as u64));
        }
        if let Some((translation, first)) = sources.get(&entry.source) {
            if translation != &entry.translation {
                return Err(TransferError::new(if csv {
                    "import.csv_conflict"
                } else {
                    "import.json_conflict"
                })
                .with_arg(if csv { "line" } else { "entry" }, position as u64)
                .with_arg("first", *first as u64));
            }
            continue;
        }
        sources.insert(entry.source.clone(), (entry.translation.clone(), position));
        unique.push(entry);
    }
    Ok(unique)
}

#[derive(Serialize)]
pub struct ImportPreview {
    pub entries: Vec<Entry>,
    pub metadata: Option<serde_json::Value>,
}
pub fn decode_document(text: &str, extension: &str) -> Result<ImportPreview, TransferError> {
    let entries = parse_entries(text, extension)?;
    let metadata = (handler(extension)?.metadata)(text.trim_start_matches('\u{feff}'))?;
    Ok(ImportPreview { entries, metadata })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn srt_extracts_multiline_text_and_deduplicates_cues() {
        let text = "\u{feff}1\r\n00:00:01,000 --> 00:00:02,000\r\nHello\r\nWorld\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\nHello\r\nWorld\r\n";
        let result = decode_document(text, "SRT").unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].source, "Hello\nWorld");
        assert!(result.entries[0].translation.is_empty());
        assert!(result.metadata.is_none());
        assert_eq!(
            encode_entries("srt", &result.entries, None)
                .unwrap_err()
                .code,
            "export.unsupported"
        );
    }
    #[test]
    fn srt_reports_bad_timing_and_missing_text_with_physical_lines() {
        for (text, code, line) in [
            (
                "1\n00:00:03,000 --> 00:00:02,000\nHello",
                "import.srt_timing",
                2,
            ),
            ("1\n00:00:01,000 --> 00:00:02,000\n", "import.srt_text", 3),
            ("hello", "import.srt_index", 1),
        ] {
            let error = decode_document(text, "srt").err().unwrap();
            assert_eq!(error.code, code);
            assert_eq!(error.args["line"], line);
        }
    }
    #[test]
    fn csv_roundtrip_keeps_quotes_newlines_and_empty_translations() {
        let entries = vec![Entry {
            source: "A,\"B\"\nC".into(),
            translation: String::new(),
        }];
        let encoded = encode_entries("csv", &entries, None).unwrap();
        let decoded = decode_document(&encoded, "csv").unwrap();
        assert_eq!(decoded.entries[0].source, entries[0].source);
        assert!(decoded.entries[0].translation.is_empty());
        assert!(encode_entries("xml", &entries, None).is_err());
    }
}
