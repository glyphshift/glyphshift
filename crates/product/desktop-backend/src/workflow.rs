use super::*;

const WORKFLOW_SCHEMA: &str = "glyphshift.workflow/3";
const WORKFLOW_STATE_SCHEMA: &str = "glyphshift.workflow-state/1";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowActivationSnapshot {
    pub(super) workflow_id: Box<str>,
    pub(super) revision: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSummaryView {
    pub(super) id: Box<str>,
    pub(super) name: Box<str>,
    pub(super) description: Box<str>,
    pub(super) global_shortcut: Box<str>,
    pub(super) revision: u64,
    pub(super) software_ids: Vec<Box<str>>,
    pub(super) dictionary_ids: Vec<Box<str>>,
    pub(super) targets: Vec<WorkflowTargetView>,
}

impl WorkflowSummaryView {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
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
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn software_ids(&self) -> &[Box<str>] {
        &self.software_ids
    }

    #[must_use]
    pub fn dictionary_ids(&self) -> &[Box<str>] {
        &self.dictionary_ids
    }

    #[must_use]
    pub fn targets(&self) -> &[WorkflowTargetView] {
        &self.targets
    }
    #[must_use]
    pub fn global_shortcut(&self) -> &str {
        &self.global_shortcut
    }
}

impl WorkflowActivationSnapshot {
    #[must_use]
    pub fn workflow_id(&self) -> &str {
        &self.workflow_id
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAdapterStrategy {
    Parallel,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAdapterPlan {
    strategy: WorkflowAdapterStrategy,
    adapter_ids: Vec<Box<str>>,
}

impl WorkflowAdapterPlan {
    #[must_use]
    pub fn parallel(adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        Self {
            strategy: WorkflowAdapterStrategy::Parallel,
            adapter_ids: adapter_ids.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub const fn strategy(&self) -> WorkflowAdapterStrategy {
        self.strategy
    }

    #[must_use]
    pub fn adapter_ids(&self) -> &[Box<str>] {
        &self.adapter_ids
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FontCoverage {
    DictionaryMatches,
    AllObservations,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowFontPolicy {
    families: Vec<Box<str>>,
    coverage: FontCoverage,
}

impl WorkflowFontPolicy {
    #[must_use]
    pub fn new(
        families: impl IntoIterator<Item = impl Into<Box<str>>>,
        coverage: FontCoverage,
    ) -> Self {
        Self {
            families: families.into_iter().map(Into::into).collect(),
            coverage,
        }
    }

    #[must_use]
    pub fn families(&self) -> &[Box<str>] {
        &self.families
    }

    #[must_use]
    pub const fn coverage(&self) -> FontCoverage {
        self.coverage
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowTargetCreate {
    software_id: Box<str>,
    adapter_plan: WorkflowAdapterPlan,
    dictionary_ids: Vec<Box<str>>,
    #[serde(default)]
    font_policy: Option<WorkflowFontPolicy>,
}

impl WorkflowTargetCreate {
    #[must_use]
    pub fn new(
        software_id: impl Into<Box<str>>,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
        dictionary_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            software_id: software_id.into(),
            adapter_plan: WorkflowAdapterPlan::parallel(adapter_ids),
            dictionary_ids: dictionary_ids.into_iter().map(Into::into).collect(),
            font_policy: None,
        }
    }

    #[must_use]
    pub fn with_font_policy(mut self, policy: WorkflowFontPolicy) -> Self {
        self.font_policy = Some(policy);
        self
    }

    #[must_use]
    pub fn with_optional_font_policy(mut self, policy: Option<WorkflowFontPolicy>) -> Self {
        self.font_policy = policy;
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCreate {
    id: Box<str>,
    name: Box<str>,
    #[serde(default)]
    description: Box<str>,
    #[serde(default)]
    global_shortcut: Box<str>,
    targets: Vec<WorkflowTargetCreate>,
}

impl WorkflowCreate {
    #[must_use]
    pub fn new(id: impl Into<Box<str>>, name: impl Into<Box<str>>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: "".into(),
            global_shortcut: "".into(),
            targets: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.description = description.into();
        self
    }

    #[must_use]
    pub fn with_targets(mut self, targets: impl IntoIterator<Item = WorkflowTargetCreate>) -> Self {
        self.targets = targets.into_iter().collect();
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn global_shortcut(&self) -> &str {
        &self.global_shortcut
    }
    #[must_use]
    pub fn with_global_shortcut(mut self, value: impl Into<Box<str>>) -> Self {
        self.global_shortcut = value.into();
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEdit {
    id: Box<str>,
    name: Box<str>,
    #[serde(default)]
    description: Box<str>,
    #[serde(default)]
    global_shortcut: Box<str>,
    base_revision: u64,
    targets: Vec<WorkflowTargetCreate>,
}

impl WorkflowEdit {
    #[must_use]
    pub fn new(id: impl Into<Box<str>>, name: impl Into<Box<str>>, base_revision: u64) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: "".into(),
            global_shortcut: "".into(),
            base_revision,
            targets: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.description = description.into();
        self
    }

    #[must_use]
    pub fn with_targets(mut self, targets: impl IntoIterator<Item = WorkflowTargetCreate>) -> Self {
        self.targets = targets.into_iter().collect();
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn global_shortcut(&self) -> &str {
        &self.global_shortcut
    }
    #[must_use]
    pub fn with_global_shortcut(mut self, value: impl Into<Box<str>>) -> Self {
        self.global_shortcut = value.into();
        self
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowTargetView {
    pub(super) software_id: Box<str>,
    pub(super) adapter_plan: WorkflowAdapterPlan,
    pub(super) dictionary_ids: Vec<Box<str>>,
    pub(super) font_policy: Option<WorkflowFontPolicy>,
}

impl WorkflowTargetView {
    #[must_use]
    pub fn software_id(&self) -> &str {
        &self.software_id
    }

    #[must_use]
    pub fn dictionary_ids(&self) -> &[Box<str>] {
        &self.dictionary_ids
    }

    #[must_use]
    pub const fn adapter_plan(&self) -> &WorkflowAdapterPlan {
        &self.adapter_plan
    }

    #[must_use]
    pub const fn font_policy(&self) -> Option<&WorkflowFontPolicy> {
        self.font_policy.as_ref()
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowView {
    id: Box<str>,
    name: Box<str>,
    description: Box<str>,
    global_shortcut: Box<str>,
    revision: u64,
    targets: Vec<WorkflowTargetView>,
}

impl WorkflowView {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
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
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn targets(&self) -> &[WorkflowTargetView] {
        &self.targets
    }
    #[must_use]
    pub fn global_shortcut(&self) -> &str {
        &self.global_shortcut
    }
}
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
    pub(super) font_policy: Option<WorkflowFontPolicy>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowStateArtifact {
    schema: Box<str>,
    enabled: BTreeMap<Box<str>, u64>,
}
#[derive(Clone, Debug)]
pub struct EffectiveWorkflowIntent {
    workflow_id: Box<str>,
    targets: Vec<EffectiveTargetIntent>,
}

impl EffectiveWorkflowIntent {
    #[must_use]
    pub fn workflow_id(&self) -> &str {
        &self.workflow_id
    }

    #[must_use]
    pub fn targets(&self) -> &[EffectiveTargetIntent] {
        &self.targets
    }
}

#[derive(Clone, Debug)]
pub struct EffectiveTargetIntent {
    software_id: Box<str>,
    runtime_spec: DesktopRuntimeSpec,
    requested_features: Vec<Feature>,
}

impl EffectiveTargetIntent {
    #[must_use]
    pub fn software_id(&self) -> &str {
        &self.software_id
    }

    #[must_use]
    pub const fn runtime_spec(&self) -> &DesktopRuntimeSpec {
        &self.runtime_spec
    }

    #[must_use]
    pub fn requested_features(&self) -> &[Feature] {
        &self.requested_features
    }
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
                    font_policy: target.font_policy,
                })
                .collect(),
        };
        validate_workflow(&artifact, None)?;
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
                );
                target
                    .font_policy
                    .as_ref()
                    .map_or(definition_target.clone(), |policy| {
                        definition_target.with_font_policy(CompiledTargetFontPolicy::new(
                            policy.families.iter().cloned(),
                            match policy.coverage {
                                FontCoverage::DictionaryMatches => {
                                    CompiledFontCoverage::DictionaryMatches
                                }
                                FontCoverage::AllObservations => {
                                    CompiledFontCoverage::AllObservations
                                }
                            },
                        ))
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
                Ok(SoftwareInput::new(
                    state.artifact.id.clone(),
                    state.locale.clone(),
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
                        executable_paths: self
                            .local_software
                            .get(target.software_id())
                            .map(|software| vec![software.executable_path.clone()])
                            .unwrap_or_default(),
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
            executable_paths: self
                .local_software
                .get(software_id)
                .map(|software| vec![software.executable_path.clone()])
                .unwrap_or_default(),
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
                font_policy: target.font_policy.clone(),
            })
            .collect(),
    }
}

fn validate_workflow(artifact: &WorkflowArtifact, path: Option<&Path>) -> Result<(), BackendError> {
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
        || artifact.targets.is_empty()
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
                || target.font_policy.as_ref().is_some_and(|policy| {
                    policy.families.is_empty()
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

pub(super) struct WorkflowLoad {
    pub(super) workflows: BTreeMap<Box<str>, WorkflowArtifact>,
    pub(super) warnings: Vec<ArtifactWarningView>,
}

pub(super) fn read_workflows(root: &Path) -> Result<WorkflowLoad, BackendError> {
    let directory = root.join("workflows");
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
        .filter_map(|target| serde_json::from_value(target.clone()).ok())
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

pub(super) fn read_workflow_state(
    root: &Path,
    workflows: &BTreeMap<Box<str>, WorkflowArtifact>,
) -> Result<BTreeMap<Box<str>, u64>, BackendError> {
    let path = root.join("workflow-state.json");
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

fn write_workflow_state(
    root: &Path,
    enabled: &BTreeMap<Box<str>, u64>,
) -> Result<(), BackendError> {
    let state = WorkflowStateArtifact {
        schema: WORKFLOW_STATE_SCHEMA.into(),
        enabled: enabled.clone(),
    };
    let serialized = serde_json::to_string(&state)
        .map_err(|_| BackendError::InvalidArtifact("serialize-workflow-state"))?;
    write_atomic(&root.join("workflow-state.json"), &serialized)
}
