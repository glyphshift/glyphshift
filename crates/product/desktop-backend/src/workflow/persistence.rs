use super::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowStateArtifact {
    schema: Box<str>,
    enabled: BTreeMap<Box<str>, u64>,
}

pub(crate) struct WorkflowLoad {
    pub(crate) workflows: BTreeMap<Box<str>, WorkflowArtifact>,
    pub(crate) warnings: Vec<ArtifactWarningView>,
}

pub(crate) fn read_workflows(root: &Path) -> Result<WorkflowLoad, BackendError> {
    let directory = root.join("workflows-v4");
    let mut paths = fs::read_dir(&directory)
        .map_err(|_| BackendError::Storage("read-workflow-directory"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut workflows = BTreeMap::new();
    let mut warnings = Vec::new();
    for path in paths {
        let artifact_id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("unreadable-file-name");
        let Ok(source) = fs::read_to_string(&path) else {
            warnings.push(ArtifactWarningView::workflow(artifact_id, "unreadable"));
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&source) else {
            warnings.push(ArtifactWarningView::workflow(artifact_id, "unreadable"));
            continue;
        };
        let Some(artifact) = workflow_artifact_from_value(&value) else {
            warnings.push(ArtifactWarningView::workflow(artifact_id, "invalid"));
            continue;
        };
        if validate_workflow(&artifact, Some(&path)).is_err() {
            warnings.push(ArtifactWarningView::workflow(artifact_id, "invalid"));
            continue;
        }
        let workflow_id = artifact.id.clone();
        if workflows.insert(workflow_id.clone(), artifact).is_some() {
            warnings.push(ArtifactWarningView::workflow(
                workflow_id,
                "duplicate_identity",
            ));
        }
    }
    Ok(WorkflowLoad {
        workflows,
        warnings,
    })
}

fn workflow_artifact_from_value(value: &serde_json::Value) -> Option<WorkflowArtifact> {
    let object = value.as_object()?;
    let id = object.get("id")?.as_str()?;
    let name = object
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(id);
    let description = object
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let revision = object
        .get("revision")
        .and_then(serde_json::Value::as_u64)
        .filter(|revision| *revision > 0)
        .unwrap_or(1);
    let targets = object
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|target| {
            let mut target = target.clone();
            if let Some(object) = target.as_object_mut() {
                let valid = object.get("writeDictionaryId").and_then(serde_json::Value::as_str)
                    .filter(|id| object.get("dictionaryIds").and_then(serde_json::Value::as_array)
                        .is_some_and(|ids| ids.iter().any(|value| value.as_str() == Some(*id))))
                    .map(str::to_owned);
                object.insert("writeDictionaryId".into(), valid.map_or(serde_json::Value::Null, serde_json::Value::String));
            }
            serde_json::from_value(target).ok()
        })
        .collect();
    Some(WorkflowArtifact {
        schema: WORKFLOW_SCHEMA.into(),
        id: id.into(),
        name: name.into(),
        description: description.into(),
        global_shortcut: object
            .get("globalShortcut")
            .and_then(serde_json::Value::as_str)
            .filter(|value| value.len() <= 64 && !value.chars().any(char::is_control))
            .unwrap_or_default()
            .into(),
        revision,
        targets,
    })
}

pub(crate) fn read_workflow_state(
    root: &Path,
    workflows: &BTreeMap<Box<str>, WorkflowArtifact>,
) -> Result<BTreeMap<Box<str>, u64>, BackendError> {
    let path = root.join("workflow-state-v2.json");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let source =
        fs::read_to_string(&path).map_err(|_| BackendError::Storage("read-workflow-state"))?;
    let value =
        serde_json::from_str::<serde_json::Value>(&source).unwrap_or(serde_json::Value::Null);
    let enabled = value
        .get("enabled")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(workflow_id, revision)| {
            let revision = revision.as_u64()?;
            workflows
                .get(workflow_id.as_str())
                .is_some_and(|workflow| workflow.revision == revision)
                .then(|| (Box::<str>::from(workflow_id.as_str()), revision))
        })
        .collect();
    Ok(enabled)
}

pub(crate) fn write_workflow_state(
    root: &Path,
    enabled: &BTreeMap<Box<str>, u64>,
) -> Result<(), BackendError> {
    let state = WorkflowStateArtifact {
        schema: WORKFLOW_STATE_SCHEMA.into(),
        enabled: enabled.clone(),
    };
    let serialized = serde_json::to_string(&state)
        .map_err(|_| BackendError::InvalidArtifact("serialize-workflow-state"))?;
    write_atomic(&root.join("workflow-state-v2.json"), &serialized)
}
