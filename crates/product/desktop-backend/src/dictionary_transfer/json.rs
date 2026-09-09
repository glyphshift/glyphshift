use super::*;
pub(super) fn decode(text: &str) -> Result<Vec<(usize, Entry)>, TransferError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(json_error)?;
    let values = if value.is_array() {
        &value
    } else {
        value
            .get("entries")
            .ok_or_else(|| TransferError::new("import.json_entries"))?
    };
    values
        .as_array()
        .ok_or_else(|| TransferError::new("import.json_entries"))?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            serde_json::from_value(value.clone())
                .map(|entry| (index + 1, entry))
                .map_err(|_| {
                    TransferError::new("import.json_entry").with_arg("entry", (index + 1) as u64)
                })
        })
        .collect::<Result<_, _>>()
}
pub(super) fn encode(
    entries: &[Entry],
    metadata: Option<&serde_json::Value>,
) -> Result<String, TransferError> {
    let value = match metadata {
        Some(metadata) => {
            let mut document = metadata.clone();
            let object = document
                .as_object_mut()
                .ok_or_else(|| TransferError::new("import.json_metadata"))?;
            object.insert("entries".into(), serde_json::json!(entries));
            document
        }
        None => serde_json::json!(entries),
    };
    serde_json::to_string_pretty(&value).map_err(json_error)
}

pub(super) fn metadata(text: &str) -> Result<Option<serde_json::Value>, TransferError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(json_error)?;
    value
        .get("metadata")
        .map(|metadata| {
            let typed: crate::DictionaryMetadata = serde_json::from_value(metadata.clone())
                .map_err(|_| TransferError::new("import.json_metadata"))?;
            serde_json::to_value(typed).map_err(json_error)
        })
        .transpose()
}
