use super::*;

pub(super) fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

pub(super) fn dictionary_path(root: &Path, dictionary_id: &str) -> Result<PathBuf, BackendError> {
    if !safe_identifier(dictionary_id) {
        return Err(BackendError::InvalidArtifact("unsafe-artifact-id"));
    }
    Ok(root
        .join("dictionaries")
        .join(format!("{dictionary_id}.json")))
}

pub(super) fn workflow_path(root: &Path, workflow_id: &str) -> Result<PathBuf, BackendError> {
    if !safe_identifier(workflow_id) {
        return Err(BackendError::InvalidArtifact("unsafe-artifact-id"));
    }
    Ok(root.join("workflows").join(format!("{workflow_id}.json")))
}

pub(super) fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    operation: &'static str,
) -> Result<T, BackendError> {
    let source = fs::read_to_string(path).map_err(|_| BackendError::Storage(operation))?;
    serde_json::from_str(&source).map_err(|_| BackendError::InvalidArtifact(operation))
}

pub(super) fn write_atomic(path: &Path, content: &str) -> Result<(), BackendError> {
    let parent = path
        .parent()
        .ok_or(BackendError::Storage("resolve-artifact-directory"))?;
    fs::create_dir_all(parent).map_err(|_| BackendError::Storage("create-artifact-directory"))?;
    let mut staged =
        NamedTempFile::new_in(parent).map_err(|_| BackendError::Storage("stage-artifact"))?;
    staged
        .write_all(content.as_bytes())
        .and_then(|()| staged.as_file().sync_all())
        .map_err(|_| BackendError::Storage("write-artifact"))?;
    staged
        .persist(path)
        .map_err(|_| BackendError::Storage("publish-artifact"))?;
    Ok(())
}
