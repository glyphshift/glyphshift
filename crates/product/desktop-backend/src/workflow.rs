use super::*;

mod model;
mod persistence;

pub use model::*;
pub(super) use persistence::{read_workflow_state, read_workflows};
use persistence::write_workflow_state;

const WORKFLOW_SCHEMA: &str = "glyphshift.workflow/4";
const WORKFLOW_STATE_SCHEMA: &str = "glyphshift.workflow-state/1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkflowArtifact {
    pub(super) schema: Box<str>,
    pub(super) id: Box<str>,
    pub(super) name: Box<str>,
    #[serde(default)]
    pub(super) description: Box<str>,
    #[serde(default)]
    pub(super) global_shortcut: Box<str>,
    pub(super) revision: u64,
    pub(super) targets: Vec<WorkflowTargetArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkflowTargetArtifact {
    pub(super) software_id: Box<str>,
    pub(super) adapter_plan: WorkflowAdapterPlan,
    pub(super) dictionary_ids: Vec<Box<str>>,
    #[serde(default)]
    pub(super) write_dictionary_id: Option<Box<str>>,
    #[serde(default)]
    pub(super) collect_new_sources: Option<bool>,
    #[serde(default)]
    pub(super) font_policy: Option<WorkflowFontPolicy>,
}

impl DesktopBackend {
    pub fn workflow(&self, workflow_id: &str) -> Result<WorkflowView, BackendError> {
        self.workflows
            .get(workflow_id)
            .map(workflow_view)
            .ok_or_else(|| BackendError::UnknownWorkflow(workflow_id.into()))
    }

    #[must_use]
    pub fn enabled_workflow_ids(&self) -> &[Box<str>] {
        &self.enabled_workflow_ids
    }

    pub fn create_workflow(
        &mut self,
        create: WorkflowCreate,
    ) -> Result<WorkflowView, BackendError> {
        if self.workflows.contains_key(&create.id) {
            return Err(BackendError::DuplicateWorkflow(create.id));
        }
        let artifact = WorkflowArtifact {
            schema: WORKFLOW_SCHEMA.into(),
            id: create.id,
            name: create.name,
            description: create.description,
            global_shortcut: create.global_shortcut,
            revision: 1,
            targets: create
                .targets
                .into_iter()
                .map(|target| WorkflowTargetArtifact {
                    software_id: target.software_id,
                    adapter_plan: target.adapter_plan,
                    dictionary_ids: target.dictionary_ids,
                    write_dictionary_id: target.write_dictionary_id,
                    collect_new_sources: Some(target.collect_new_sources.unwrap_or(true)),
                    font_policy: target.font_policy,
                })
                .collect(),
        };
        validate_workflow(&artifact, None)?;
        let serialized = serde_json::to_string(&artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-workflow"))?;
        write_atomic(&workflow_path(&self.root, &artifact.id)?, &serialized)?;
        let view = workflow_view(&artifact);
        self.workflows.insert(artifact.id.clone(), artifact);
        Ok(view)
    }

    pub fn update_workflow(&mut self, edit: WorkflowEdit) -> Result<WorkflowView, BackendError> {
        let current = self
            .workflows
            .get(&edit.id)
            .ok_or_else(|| BackendError::UnknownWorkflow(edit.id.clone()))?;
        if current.revision != edit.base_revision {
            return Err(BackendError::RevisionConflict {
                current: current.revision,
            });
        }
        let artifact = WorkflowArtifact {
            schema: WORKFLOW_SCHEMA.into(),
            id: edit.id,
            name: edit.name,
            description: edit.description,
            global_shortcut: edit.global_shortcut,
            revision: current.revision + 1,
            targets: edit
                .targets
                .into_iter()
                .map(|target| WorkflowTargetArtifact {
                    software_id: target.software_id,
                    adapter_plan: target.adapter_plan,
                    dictionary_ids: target.dictionary_ids,
                    write_dictionary_id: target.write_dictionary_id,
                    collect_new_sources: Some(target.collect_new_sources.unwrap_or(true)),
                    font_policy: target.font_policy,
                })
                .collect(),
        };
        validate_workflow(&artifact, None)?;
        if self.enabled_workflows.contains_key(&artifact.id)
            && current.targets.iter().chain(&artifact.targets).any(|target| target.write_dictionary_id.is_some())
            && (current.targets.len() != artifact.targets.len() || current.targets.iter().zip(&artifact.targets).any(|(old, new)|
                old.software_id != new.software_id || old.adapter_plan != new.adapter_plan
                || old.dictionary_ids != new.dictionary_ids || old.write_dictionary_id != new.write_dictionary_id)) {
            return Err(BackendError::InvalidArtifact("workflow-collection-active"));
        }
        if self.enabled_workflows.contains_key(&artifact.id) {
            self.validate_workflow_activation(&artifact)?;
            if let Some(error) = self.activation_conflict(&artifact) {
                return Err(error);
            }
        }
        let serialized = serde_json::to_string(&artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-workflow"))?;
        write_atomic(&workflow_path(&self.root, &artifact.id)?, &serialized)?;
        if self.enabled_workflows.contains_key(&artifact.id) {
            let mut enabled = self.enabled_workflows.clone();
            enabled.insert(artifact.id.clone(), artifact.revision);
            write_workflow_state(&self.root, &enabled)?;
            self.enabled_workflows = enabled;
            self.enabled_workflow_ids = self.enabled_workflows.keys().cloned().collect();
        }
        let view = workflow_view(&artifact);
        self.workflows.insert(artifact.id.clone(), artifact);
        Ok(view)
    }

    pub fn set_workflow_collection_enabled(&mut self, workflow_id: &str, enabled: bool) -> Result<WorkflowView, BackendError> {
        let source = self.workflows.get(workflow_id).cloned()
            .ok_or_else(|| BackendError::UnknownWorkflow(workflow_id.into()))?;
        if enabled && source.targets.iter().any(|target| target.write_dictionary_id.is_none()) {
            return Err(BackendError::InvalidArtifact("workflow-collection-writer"));
        }
        let targets = source.targets.iter().map(|target| WorkflowTargetCreate::new(
            target.software_id.clone(), target.adapter_plan.adapter_ids.iter().cloned(), target.dictionary_ids.iter().cloned())
            .with_optional_write_dictionary(target.write_dictionary_id.clone())
            .with_collection_enabled(Some(enabled)).with_optional_font_policy(target.font_policy.clone())).collect();
        self.update_workflow(WorkflowEdit { id: source.id, name: source.name, description: source.description,
            global_shortcut: source.global_shortcut, base_revision: source.revision, targets })
    }

    pub fn copy_workflow(
        &mut self,
        source_workflow_id: &str,
        new_workflow_id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
    ) -> Result<WorkflowView, BackendError> {
        let source = self
            .workflows
            .get(source_workflow_id)
            .ok_or_else(|| BackendError::UnknownWorkflow(source_workflow_id.into()))?;
        let targets = source
            .targets
            .iter()
            .map(|target| {
                WorkflowTargetCreate::new(
                    target.software_id.clone(),
                    target.adapter_plan.adapter_ids.iter().cloned(),
                    target.dictionary_ids.iter().cloned(),
                )
                .with_optional_write_dictionary(target.write_dictionary_id.clone())
                .with_collection_enabled(target.collect_new_sources)
                .with_optional_font_policy(target.font_policy.clone())
            })
            .collect::<Vec<_>>();
        self.create_workflow(WorkflowCreate {
            id: new_workflow_id.into(),
            name: name.into(),
            description: source.description.clone(),
            global_shortcut: "".into(),
            targets,
        })
    }

    pub fn enable_workflow(&mut self, workflow_id: &str) -> Result<(), BackendError> {
        let artifact = self.compile_workflow_definition(workflow_id)?;
        if let Some(error) = self.activation_conflict(&artifact) {
            return Err(error);
        }
        let mut next = self.enabled_workflows.clone();
        next.insert(artifact.id.clone(), artifact.revision);
        self.publish_workflow_state(next)
    }

    pub fn replace_workflow_activation(&mut self, workflow_id: &str) -> Result<(), BackendError> {
        let artifact = self.compile_workflow_definition(workflow_id)?;
        let occupied = artifact
            .targets
            .iter()
            .map(|target| target.software_id.as_ref())
            .collect::<BTreeSet<_>>();
        let mut next = self.enabled_workflows.clone();
        next.retain(|owner_id, _| {
            owner_id.as_ref() == workflow_id
                || self.workflows.get(owner_id.as_ref()).is_some_and(|owner| {
                    owner
                        .targets
                        .iter()
                        .all(|target| !occupied.contains(target.software_id.as_ref()))
                })
        });
        next.insert(artifact.id.clone(), artifact.revision);
        self.publish_workflow_state(next)
    }

    pub fn disable_workflow(&mut self, workflow_id: &str) -> Result<(), BackendError> {
        if !self.workflows.contains_key(workflow_id) {
            return Err(BackendError::UnknownWorkflow(workflow_id.into()));
        }
        let mut next = self.enabled_workflows.clone();
        next.remove(workflow_id);
        self.publish_workflow_state(next)
    }

    pub fn delete_workflows<'a>(
        &mut self,
        workflow_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), BackendError> {
        let workflow_ids = workflow_ids.into_iter().collect::<BTreeSet<_>>();
        for workflow_id in &workflow_ids {
            if !self.workflows.contains_key(*workflow_id) {
                return Err(BackendError::UnknownWorkflow((*workflow_id).into()));
            }
            if self.enabled_workflows.contains_key(*workflow_id) {
                return Err(BackendError::WorkflowEnabled((*workflow_id).into()));
            }
        }
        for workflow_id in workflow_ids {
            let path = workflow_path(&self.root, workflow_id)?;
            fs::remove_file(path).map_err(|_| BackendError::Storage("remove-workflow"))?;
            self.workflows.remove(workflow_id);
        }
        Ok(())
    }

    fn compile_workflow_definition(
        &self,
        workflow_id: &str,
    ) -> Result<WorkflowArtifact, BackendError> {
        let artifact = self
            .workflows
            .get(workflow_id)
            .ok_or_else(|| BackendError::UnknownWorkflow(workflow_id.into()))?
            .clone();
        self.validate_workflow_activation(&artifact)?;
        Ok(artifact)
    }

    pub(super) fn validate_workflow_activation(
        &self,
        artifact: &WorkflowArtifact,
    ) -> Result<(), BackendError> {
        self.resolve_workflow_artifact(artifact).map(|_| ())
    }

    fn resolve_workflow_artifact(
        &self,
        artifact: &WorkflowArtifact,
    ) -> Result<CompiledWorkflow, BackendError> {
        self.resolve_workflow_artifact_with_dictionary(artifact, None)
    }

    pub(super) fn resolve_workflow_artifact_with_dictionary(
        &self,
        artifact: &WorkflowArtifact,
        dictionary_override: Option<&DictionaryView>,
    ) -> Result<CompiledWorkflow, BackendError> {
        let definition = WorkflowDefinition::new(
            artifact.id.clone(),
            artifact.targets.iter().map(|target| {
                let definition_target = WorkflowDefinitionTarget::new(
                    target.software_id.clone(),
                    AdapterPlan::parallel(target.adapter_plan.adapter_ids.iter().cloned()),
                    target.dictionary_ids.iter().cloned(),
                ).with_collection(target.write_dictionary_id.is_some());
                target
                    .font_policy
                    .as_ref()
                    .map_or(definition_target.clone(), |policy| {
                        let mut compiled = CompiledTargetFontPolicy::new(
                            policy.families.iter().cloned(),
                            match policy.coverage {
                                FontCoverage::DictionaryMatches => {
                                    CompiledFontCoverage::DictionaryMatches
                                }
                                FontCoverage::AllObservations => {
                                    CompiledFontCoverage::AllObservations
                                }
                            },
                        ).with_scale_percent(policy.scale_percent);
                        for (id, font) in &policy.dictionary_overrides {
                            compiled = compiled.with_dictionary_override(id.clone(), font.families.iter().cloned(), font.scale_percent);
                        }
                        definition_target.with_font_policy(compiled)
                    })
            }),
        );
        let software = artifact
            .targets
            .iter()
            .map(|target| {
                let state = self
                    .software
                    .get(&target.software_id)
                    .ok_or_else(|| BackendError::UnknownSoftware(target.software_id.clone()))?;
                self.software_binding_paths(&target.software_id)?;
                let route = state.artifact.runtime.as_ref().map_or_else(
                    || {
                        state
                            .artifact
                            .locations
                            .first()
                            .map(|location| RouteProgram::direct(location.id.clone()))
                            .ok_or(BackendError::InvalidArtifact("runtime-location-missing"))
                    },
                    |runtime| runtime_route(&runtime.route, &state.artifact.locations),
                )?;
                let language_dictionary = target.write_dictionary_id.as_ref().or_else(|| target.dictionary_ids.first());
                let target_locale = language_dictionary.and_then(|id| {
                    dictionary_override.filter(|dictionary| dictionary.id() == id.as_ref())
                        .or_else(|| self.dictionaries.get(id.as_ref()))
                }).map_or_else(|| state.locale.clone(), |dictionary| dictionary.metadata().target_locale().into());
                Ok(SoftwareInput::new(
                    state.artifact.id.clone(),
                    target_locale,
                    glyphshift_domain::Generation::new(target.dictionary_ids.iter().fold(
                        artifact.revision,
                        |generation, dictionary_id| {
                            let revision = dictionary_override
                                .filter(|dictionary| dictionary.id() == dictionary_id.as_ref())
                                .map(DictionaryView::revision)
                                .or_else(|| {
                                    self.dictionaries
                                        .get(dictionary_id)
                                        .map(DictionaryView::revision)
                                })
                                .unwrap_or(0);
                            generation.saturating_add(revision)
                        },
                    )),
                    route,
                    state
                        .artifact
                        .locations
                        .iter()
                        .map(|location| location.id.clone()),
                ))
            })
            .collect::<Result<Vec<_>, BackendError>>()?;
        let dictionaries = self
            .dictionaries
            .values()
            .map(|dictionary| {
                dictionary_override
                    .filter(|candidate| candidate.id() == dictionary.id())
                    .map_or_else(|| dictionary_definition(dictionary), dictionary_definition)
            })
            .collect::<Vec<_>>();
        resolve_workflow(
            &definition,
            &software,
            &dictionaries,
            &self.environment.composition,
        )
        .map_err(BackendError::WorkflowRejected)
    }

    pub(super) fn activation_conflict(&self, artifact: &WorkflowArtifact) -> Option<BackendError> {
        artifact.targets.iter().find_map(|target| {
            self.enabled_workflows
                .keys()
                .find(|owner_id| {
                    owner_id.as_ref() != artifact.id.as_ref()
                        && self.workflows.get(owner_id.as_ref()).is_some_and(|owner| {
                            owner
                                .targets
                                .iter()
                                .any(|owner_target| owner_target.software_id == target.software_id)
                        })
                })
                .map(|owner_id| BackendError::SoftwareOccupied {
                    software_id: target.software_id.clone(),
                    workflow_id: owner_id.clone(),
                })
        })
    }

    fn publish_workflow_state(
        &mut self,
        enabled: BTreeMap<Box<str>, u64>,
    ) -> Result<(), BackendError> {
        write_workflow_state(&self.root, &enabled)?;
        self.enabled_workflow_ids = enabled.keys().cloned().collect();
        self.enabled_workflows = enabled;
        Ok(())
    }

    pub fn effective_workflow_intent(
        &self,
        workflow_id: &str,
    ) -> Result<EffectiveWorkflowIntent, BackendError> {
        let workflow = self
            .workflows
            .get(workflow_id)
            .ok_or_else(|| BackendError::UnknownWorkflow(workflow_id.into()))?;
        let compiled = self.resolve_workflow_artifact(workflow)?;
        let targets = compiled
            .targets()
            .iter()
            .map(|target| {
                let state = self
                    .software
                    .get(target.software_id())
                    .ok_or_else(|| BackendError::UnknownSoftware(target.software_id().into()))?;
                let requirements = self.target_requirements(target)?;
                Ok(EffectiveTargetIntent {
                    software_id: target.software_id().into(),
                    runtime_spec: DesktopRuntimeSpec {
                        executable_names: state.artifact.executables.clone(),
                        executable_paths: self.software_binding_paths(target.software_id())?,
                        descendant_executable_names: state.artifact.descendant_executables.clone(),
                        requirements,
                        publication: RuntimePublication::new(
                            target.route().clone(),
                            target.snapshot().clone(),
                            target.font_policy().clone(),
                        ),
                    },
                    requested_features: target.requested_features().to_vec(),
                })
            })
            .collect::<Result<Vec<_>, BackendError>>()?;
        Ok(EffectiveWorkflowIntent {
            workflow_id: compiled.id().into(),
            targets,
        })
    }

    pub fn workflow_runtime_spec(
        &self,
        workflow_id: &str,
        software_id: &str,
    ) -> Result<DesktopRuntimeSpec, BackendError> {
        let workflow = self
            .workflows
            .get(workflow_id)
            .ok_or_else(|| BackendError::UnknownWorkflow(workflow_id.into()))?;
        let compiled = self.resolve_workflow_artifact(workflow)?;
        let target = compiled
            .targets()
            .iter()
            .find(|target| target.software_id() == software_id)
            .ok_or_else(|| BackendError::UnknownSoftware(software_id.into()))?;
        let state = self
            .software
            .get(software_id)
            .ok_or_else(|| BackendError::UnknownSoftware(software_id.into()))?;
        let requirements = self.target_requirements(target)?;
        Ok(DesktopRuntimeSpec {
            executable_names: state.artifact.executables.clone(),
            executable_paths: self.software_binding_paths(software_id)?,
            descendant_executable_names: state.artifact.descendant_executables.clone(),
            requirements,
            publication: RuntimePublication::new(
                target.route().clone(),
                target.snapshot().clone(),
                target.font_policy().clone(),
            ),
        })
    }
}

fn workflow_view(artifact: &WorkflowArtifact) -> WorkflowView {
    WorkflowView {
        id: artifact.id.clone(),
        name: artifact.name.clone(),
        description: artifact.description.clone(),
        global_shortcut: artifact.global_shortcut.clone(),
        revision: artifact.revision,
        targets: artifact
            .targets
            .iter()
            .map(|target| WorkflowTargetView {
                software_id: target.software_id.clone(),
                adapter_plan: target.adapter_plan.clone(),
                dictionary_ids: target.dictionary_ids.clone(),
                write_dictionary_id: target.write_dictionary_id.clone(),
                    collect_new_sources: target.collect_new_sources,
                font_policy: target.font_policy.clone(),
            })
            .collect(),
    }
}

pub(super) fn validate_workflow(artifact: &WorkflowArtifact, path: Option<&Path>) -> Result<(), BackendError> {
    let unique_software = artifact
        .targets
        .iter()
        .map(|target| &target.software_id)
        .collect::<BTreeSet<_>>()
        .len()
        == artifact.targets.len();
    if artifact.schema.as_ref() != WORKFLOW_SCHEMA
        || !safe_identifier(&artifact.id)
        || artifact.name.trim().is_empty()
        || artifact.name.chars().count() > 128
        || artifact.description.chars().count() > 512
        || artifact.global_shortcut.len() > 64
        || artifact.global_shortcut.chars().any(char::is_control)
        || artifact.revision == 0
        || artifact.targets.len() != 1
        || !unique_software
        || path.is_some_and(|path| {
            path.file_stem().and_then(|value| value.to_str()) != Some(&artifact.id)
        })
        || artifact.targets.iter().any(|target| {
            !safe_identifier(&target.software_id)
                || target.adapter_plan.adapter_ids.is_empty()
                || target
                    .adapter_plan
                    .adapter_ids
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != target.adapter_plan.adapter_ids.len()
                || target
                    .adapter_plan
                    .adapter_ids
                    .iter()
                    .any(|adapter_id| !safe_identifier(adapter_id))
                || target.dictionary_ids.iter().collect::<BTreeSet<_>>().len()
                    != target.dictionary_ids.len()
                || target
                    .dictionary_ids
                    .iter()
                    .any(|dictionary_id| !safe_identifier(dictionary_id))
                || target.write_dictionary_id.as_ref().is_some_and(|id| !target.dictionary_ids.contains(id))
                || target.font_policy.as_ref().is_some_and(|policy| {
                    (!(50..=200).contains(&policy.scale_percent))
                        || policy.dictionary_overrides.len() > target.dictionary_ids.len()
                        || policy.dictionary_overrides.iter().any(|(id, font)| {
                            !target.dictionary_ids.contains(id)
                                || font.scale_percent.is_some_and(|scale| !(50..=200).contains(&scale))
                                || font.families.len() > 16
                                || font.families.iter().any(|family| family.trim().is_empty() || family.len() > 512 || family.chars().any(char::is_control))
                                || font.families.iter().collect::<BTreeSet<_>>().len() != font.families.len()
                        })
                        || policy
                            .families
                            .iter()
                            .any(|family| family.trim().is_empty())
                        || policy.families.iter().collect::<BTreeSet<_>>().len()
                            != policy.families.len()
                })
        })
    {
        return Err(BackendError::InvalidArtifact("workflow-contract"));
    }
    Ok(())
}
