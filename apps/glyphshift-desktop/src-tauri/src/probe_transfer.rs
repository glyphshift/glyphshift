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

#[derive(Deserialize, Serialize)]
struct Entry { source: String, #[serde(default, deserialize_with = "translation_or_empty")] translation: String }

fn translation_or_empty<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn parse_entries(text: &str, format: &str) -> Result<Vec<Entry>, ()> {
    let text = text.trim_start_matches('\u{feff}');
    let entries: Vec<Entry> = match format {
        "json" => {
            let value: serde_json::Value = serde_json::from_str(text).map_err(|_| ())?;
            let entries = if value.is_array() { value } else { value.get("entries").cloned().ok_or(())? };
            serde_json::from_value(entries).map_err(|_| ())?
        }
        "csv" => {
            let rows = parse_csv(text)?;
            let header = rows.first().ok_or(())?;
            let source = header.iter().position(|key| key == "source").ok_or(())?;
            let translation = header.iter().position(|key| key == "translation").ok_or(())?;
            if header.iter().collect::<BTreeSet<_>>().len() != header.len() { return Err(()); }
            rows.iter().skip(1).map(|row| {
                if row.len() != header.len() { return Err(()); }
                Ok(Entry { source: row[source].clone(), translation: row[translation].clone() })
            }).collect::<Result<_, ()>>()?
        }
        _ => return Err(()),
    };
    let mut sources = BTreeSet::new();
    if entries.is_empty() || entries.iter().any(|entry| entry.source.trim().is_empty() || !sources.insert(&entry.source)) { return Err(()); }
    Ok(entries)
}

// CSV quoting applies to fields, including embedded CR/LF and doubled quotes.
fn parse_csv(text: &str) -> Result<Vec<Vec<String>>, ()> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut state = 0; // start, unquoted, quoted, closed quote
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if state == 2 {
            if ch == '"' { state = 3; } else { field.push(ch); }
            continue;
        }
        if state == 3 && ch == '"' { field.push(ch); state = 2; continue; }
        match ch {
            '"' if state == 0 => state = 2,
            ',' => { row.push(std::mem::take(&mut field)); state = 0; }
            '\r' | '\n' => {
                if ch == '\r' && chars.peek() == Some(&'\n') { chars.next(); }
                row.push(std::mem::take(&mut field));
                if row.len() != 1 || !row[0].is_empty() { rows.push(std::mem::take(&mut row)); } else { row.clear(); }
                state = 0;
            }
            '"' => return Err(()),
            _ if state == 3 => return Err(()),
            _ => { field.push(ch); state = 1; }
        }
    }
    if state == 2 { return Err(()); }
    if !field.is_empty() || !row.is_empty() || state == 3 { row.push(field); rows.push(row); }
    Ok(rows)
}

impl DesktopApplication {
    pub(super) fn import_probe_entries(&mut self, request: ProbeImportRequest) -> Result<ProbeRunView, CommandError> {
        let invalid = || CommandError::new("capture.import_failed");
        if !request.input_path.is_absolute() { return Err(invalid()); }
        let mut bytes = Vec::new();
        std::fs::File::open(&request.input_path).map_err(|_| invalid())?.take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes).map_err(|_| invalid())?;
        if bytes.len() > 16 * 1024 * 1024 { return Err(invalid()); }
        let incoming = parse_entries(std::str::from_utf8(&bytes).map_err(|_| invalid())?, &request.format).map_err(|_| invalid())?;
        let summary = self.probe_runs.summary(&request.run_id).map_err(probe_run_error)?;
        self.ensure_ai_dictionary_writable(summary.dictionary_id())?;
        let dictionary = self.backend.dictionary(summary.dictionary_id()).cloned().map_err(|_| invalid())?;
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
    fn malformed_and_duplicate_imports_are_rejected_before_writing() {
        for text in ["source,translation\n\"open,x", "source,translation\n\"closed\"oops,x", "source,translation\nsame,a\nsame,b", "source,translation\nonly-one"] {
            assert!(parse_entries(text, "csv").is_err());
        }
    }
}


#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EntryFilePreview {
    entries: Vec<Entry>,
    metadata: Option<serde_json::Value>,
}

fn read_entry_file(input_path: &std::path::Path) -> Result<EntryFilePreview, CommandError> {
    let invalid = || CommandError::new("dictionary.import_invalid");
    if !input_path.is_absolute() { return Err(invalid()); }
    let format = input_path.extension().and_then(|extension| extension.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut bytes = Vec::new();
    std::fs::File::open(input_path).map_err(|_| invalid())?.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|_| invalid())?;
    if bytes.len() > 16 * 1024 * 1024 { return Err(invalid()); }
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid())?.trim_start_matches('\u{feff}');
    let entries = parse_entries(text, &format).map_err(|_| invalid())?;
    let metadata = if format == "json" {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|_| invalid())?;
        match value.get("metadata") {
            Some(metadata) => {
                let typed: glyphshift_desktop_backend::DictionaryMetadata = serde_json::from_value(metadata.clone()).map_err(|_| invalid())?;
                Some(serde_json::to_value(typed).map_err(|_| invalid())?)
            },
            None => None,
        }
    } else { None };
    Ok(EntryFilePreview { entries, metadata })
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
        for invalid in ["wrong,header\nA,B", "source,translation\nA,B\nA,C", "source,translation\nA,B,extra"] {
            std::fs::write(&csv, invalid).unwrap();
            assert!(read_entry_file(&csv).is_err());
        }
    }
}
