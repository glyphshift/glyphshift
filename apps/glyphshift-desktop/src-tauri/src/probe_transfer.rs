use super::*;
use crate::probe::{probe_run_error, ProbeRunView};
use std::io::Read;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ImportMode { Overwrite, KeepExisting, Replace }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProbeImportRequest {
    pub(super) run_id: Box<str>,
    pub(super) input_path: PathBuf,
    pub(super) format: String,
    pub(super) mode: ImportMode,
}

use glyphshift_desktop_backend::dictionary_transfer::{self, Entry};
fn transfer_error(error: dictionary_transfer::TransferError) -> CommandError {
    error.args.into_iter().fold(CommandError::new(error.code), |result, (key, value)| result.with_arg(key, value))
}
fn parse_entries(text: &str, format: &str) -> Result<Vec<Entry>, CommandError> {
    dictionary_transfer::parse_entries(text, format).map_err(transfer_error)
}

fn read_import_text(path: &std::path::Path) -> Result<String, CommandError> {
    if !path.is_absolute() { return Err(CommandError::new("import.read_failed")); }
    let mut bytes = Vec::new();
    std::fs::File::open(path).map_err(|_| CommandError::new("import.read_failed"))?
        .take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)
        .map_err(|_| CommandError::new("import.read_failed"))?;
    if bytes.len() > 16 * 1024 * 1024 { return Err(CommandError::new("import.too_large")); }
    String::from_utf8(bytes).map_err(|_| CommandError::new("import.encoding"))
}

impl DesktopApplication {
    pub(super) fn import_probe_entries(&mut self, request: ProbeImportRequest) -> Result<ProbeRunView, CommandError> {
        let invalid = || CommandError::new("capture.import_failed");
        let text = read_import_text(&request.input_path)?;
        let incoming = parse_entries(&text, &request.format)?;
        let summary = self.probe_runs.summary(&request.run_id).map_err(probe_run_error)?;
        self.ensure_ai_dictionary_writable(summary.dictionary_id())?;
        let dictionary = self.backend.dictionary(summary.dictionary_id()).cloned().map_err(|_| invalid())?;
        let existing_sources = dictionary.entries().iter().map(|entry| entry.source()).collect::<BTreeSet<_>>();
        let new_sources = incoming.iter().filter(|entry| !existing_sources.contains(entry.source.as_str()))
            .map(|entry| Box::<str>::from(entry.source.as_str())).collect::<Vec<_>>();
        if !self.probe_runs.excluded_sources_for(&request.run_id, self.probe_entries_snapshot(&summary)?.as_ref(), &new_sources)
            .map_err(probe_run_error)?.is_empty() {
            return Err(CommandError::new("capture.source_owned_by_dictionary"));
        }
        let mut entries = BTreeMap::<String, String>::new();
        if !matches!(request.mode, ImportMode::Replace) {
            entries.extend(dictionary.entries().iter().map(|entry| (entry.source().to_owned(), entry.translation().to_owned())));
        }
        for entry in incoming {
            if matches!(request.mode, ImportMode::KeepExisting) { entries.entry(entry.source).or_insert(entry.translation); }
            else { entries.insert(entry.source, entry.translation); }
        }
        let edit = DictionaryEdit::from_dictionary(&dictionary).with_entries(entries.into_iter().map(|(source, translation)| DictionaryEntryCreate::new(source, translation)));
        self.backend.update_dictionary(edit).map_err(|_| invalid())?;
        self.reconcile_enabled_workflows()?;
        self.publish_probe_preview_if_active(&request.run_id)?;
        self.probe_run_summary(&request.run_id)
    }
}

#[tauri::command]
pub(super) fn desktop_import_probe_entries(request: ProbeImportRequest, application: State<'_, Mutex<DesktopApplication>>) -> Result<ProbeRunView, CommandError> {
    application.lock().map_err(|_| workspace_unavailable())?.import_probe_entries(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transfer_preserves_multiline_unicode_quotes_and_pending_entries() {
        let rows = parse_entries("\u{feff}source,translation\r\n\"A,\"\"B\"\"\r\n对话\",\"译文\"\r\nPending,\r\n", "csv").unwrap();
        assert_eq!(rows[0].source, "A,\"B\"\r\n对话");
        assert_eq!(rows[1].translation, "");
        let rows = parse_entries(r#"{"schema":"glyphshift.probe-entries/1","entries":[{"source":"Pending","translation":""}]}"#, "json").unwrap();
        assert_eq!(rows[0].source, "Pending");
    }
    #[test]
    fn repeated_pending_and_identical_entries_can_be_reimported() {
        let rows = parse_entries("source,translation\n0,\n1,\n0,\n1,\nOpen,打开\nOpen,打开\n", "csv")
            .expect("repeated identical entries should be merged");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].source, "0");
        assert_eq!(rows[0].translation, "");
        assert_eq!(rows[2].translation, "打开");
        assert!(parse_entries("source,translation\nOpen,\nOpen,打开", "csv").is_err());
    }

    #[test]
    fn csv_errors_report_physical_lines_and_actionable_reasons() {
        for (text, code, line) in [
            ("source,translation\nAssign \"Default\" Workspace,Text", "import.csv_quote", 2),
            ("source,translation\r\n\"Multi\r\nline\",Text\r\nLast", "import.csv_columns", 4),
            ("source,translation\n,Text", "import.csv_source_empty", 2),
            ("source,translation\nSame,One\nSame,Two", "import.csv_conflict", 3),
            ("source,translation\n\"Unclosed,Text", "import.csv_unclosed_quote", 2),
        ] {
            let error = parse_entries(text, "csv").err().unwrap();
            let value = serde_json::to_value(error).unwrap();
            assert_eq!(value["code"], code);
            assert_eq!(value["args"]["line"], line);
        }
    }

    #[test]
    fn json_and_file_errors_keep_their_specific_reason() {
        let error = parse_entries("{\n\"entries\": [}", "json").err().unwrap();
        let value = serde_json::to_value(error).unwrap();
        assert_eq!(value["code"], "import.json_syntax");
        assert_eq!(value["args"]["line"], 2);
        let error = parse_entries(r#"[{"source":42}]"#, "json").err().unwrap();
        assert_eq!(serde_json::to_value(error).unwrap()["code"], "import.json_entry");
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("invalid.csv");
        std::fs::write(&path, [0xff, 0xfe]).unwrap();
        let error = read_entry_file(&path).err().unwrap();
        assert_eq!(serde_json::to_value(error).unwrap()["code"], "import.encoding");
        let error = read_entry_file(&root.path().join("missing.csv")).err().unwrap();
        assert_eq!(serde_json::to_value(error).unwrap()["code"], "import.read_failed");
    }

    #[test]
    fn malformed_and_conflicting_imports_are_rejected_before_writing() {
        for text in ["source,translation\n\"open,x", "source,translation\n\"closed\"oops,x", "source,translation\nsame,a\nsame,b", "source,translation\nonly-one"] {
            assert!(parse_entries(text, "csv").is_err());
        }
    }
}


type EntryFilePreview = dictionary_transfer::ImportPreview;
fn read_entry_file(input_path: &std::path::Path) -> Result<EntryFilePreview, CommandError> {
    let format = input_path.extension().and_then(|extension| extension.to_str()).unwrap_or("");
    let input = read_import_text(input_path)?;
    dictionary_transfer::decode_document(&input, format).map_err(transfer_error)
}

#[tauri::command]
pub(super) fn desktop_preview_dictionary_import(input_path: PathBuf) -> Result<EntryFilePreview, CommandError> {
    read_entry_file(&input_path)
}


#[cfg(test)]
mod dictionary_import_tests {
    use super::*;
    #[test]
    fn file_preview_preserves_portable_metadata_and_pending_entries_and_rejects_invalid_csv() {
        let root = tempfile::tempdir().unwrap();
        let json = root.path().join("dictionary.json");
        let package = glyphshift_dictionary_package::DictionaryPackage::create(
            glyphshift_dictionary_package::DictionaryCreate::new("dictionary.import", "Import", "ja-JP", "zh-CN")
                .with_entries([glyphshift_dictionary_package::DictionaryEntryCreate::pending("Pending"), glyphshift_dictionary_package::DictionaryEntryCreate::new("Done", "完成")])
        ).unwrap();
        std::fs::write(&json, package.encode_json().unwrap()).unwrap();
        let preview = read_entry_file(&json).unwrap();
        assert_eq!(preview.metadata.unwrap()["sourceLocale"], "ja-JP");
        assert!(preview.entries.iter().any(|entry| entry.source == "Pending" && entry.translation.is_empty()));
        let csv = root.path().join("dictionary.csv");
        std::fs::write(&csv, "source,translation\nPending,\nDone,完成").unwrap();
        let preview = read_entry_file(&csv).unwrap();
        assert!(preview.metadata.is_none());
        assert_eq!(preview.entries.len(), 2);
        std::fs::write(&csv, "source,translation\nPending,\nPending,").unwrap();
        assert_eq!(read_entry_file(&csv).unwrap().entries.len(), 1);
        std::fs::write(&csv, "source,translation\nSame,First\nSame,Second").unwrap();
        let error = read_entry_file(&csv).err().unwrap();
        assert_eq!(serde_json::to_value(error).unwrap()["code"], "import.csv_conflict");
        for invalid in ["wrong,header\nA,B", "source,translation\nA,B\nA,C", "source,translation\nA,B,extra"] {
            std::fs::write(&csv, invalid).unwrap();
            assert!(read_entry_file(&csv).is_err());
        }
    }
}
