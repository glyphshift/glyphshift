//! Desktop-facing product model for software, dictionaries, and workflows.

mod dictionary;
pub mod dictionary_transfer;
mod snapshot;
mod software;
mod storage;
mod workflow;

use dictionary::{dictionary_definition, read_dictionaries, read_dictionary_installations};
pub use dictionary::{
    DictionaryCreate, DictionaryEdit, DictionaryEntryCreate, DictionaryEntryView,
    DictionaryInstallationSummaryView, DictionaryMetadata, DictionarySummaryView, DictionaryView,
};
use glyphshift_adapter_registry::{AdapterRequirement, AdapterVersion, AdapterVersionRequirement};
use glyphshift_dictionary_distribution::{
    DictionaryInstallStore, DictionaryInstallationState, FileDictionaryInstallStore,
};
use glyphshift_dictionary_package as dictionary_package;
use glyphshift_domain::{AdapterId, Feature, Generation, RouteLimits, RouteOperator, RouteProgram};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use glyphshift_workflow::{
    resolve as resolve_workflow, AdapterInput, AdapterPlan, CompiledWorkflow,
    CompositionEnvironment, Dictionary as WorkflowDictionary,
    DictionaryEntry as WorkflowDictionaryEntry, FontCoverage as CompiledFontCoverage, ResolveError,
    SoftwareInput, TargetFontPolicy as CompiledTargetFontPolicy, Workflow as WorkflowDefinition,
    WorkflowTarget as WorkflowDefinitionTarget,
};
use serde::{Deserialize, Serialize};
pub use snapshot::DesktopSnapshot;
use software::{runtime_route, DesktopSoftwareArtifact, SoftwareState};
pub use software::{
    CapabilityView, DesktopRuntimeSpec, ExecutableSelection, SoftwareEdit, SoftwareView,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use storage::{dictionary_path, read_json, safe_identifier, workflow_path, write_atomic};
use tempfile::NamedTempFile;
use workflow::{read_workflow_state, read_workflows, WorkflowArtifact};
pub use workflow::{
    EffectiveTargetIntent, EffectiveWorkflowIntent, FontCoverage, WorkflowActivationSnapshot,
    WorkflowAdapterPlan, WorkflowAdapterStrategy, WorkflowCreate, WorkflowEdit, WorkflowFontPolicy,
    WorkflowSummaryView, WorkflowTargetCreate, WorkflowTargetView, WorkflowView,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendError {
    Storage(&'static str),
    InvalidArtifact(&'static str),
    DuplicateSoftware(Box<str>),
    DuplicateDictionary(Box<str>),
    UnknownSoftware(Box<str>),
    SoftwareBindingMissing(Box<str>),
    UnknownDictionary(Box<str>),
    UnknownAdapter(Box<str>),
    DuplicateWorkflow(Box<str>),
    UnknownWorkflow(Box<str>),
    WorkflowRejected(ResolveError),
    DictionaryReferenced {
        dictionary_id: Box<str>,
        workflow_ids: Vec<Box<str>>,
    },
    SoftwareReferenced {
        software_id: Box<str>,
        workflow_ids: Vec<Box<str>>,
    },
    WorkflowEnabled(Box<str>),
    SoftwareOccupied {
        software_id: Box<str>,
        workflow_id: Box<str>,
    },
    RevisionConflict {
        current: u64,
    },
    InvalidInput(&'static str),
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactWarningView {
    artifact_kind: &'static str,
    artifact_id: Box<str>,
    issue: &'static str,
}

impl ArtifactWarningView {
    fn dictionary(artifact_id: impl Into<Box<str>>, issue: &'static str) -> Self {
        Self {
            artifact_kind: "dictionary",
            artifact_id: artifact_id.into(),
            issue,
        }
    }

    fn workflow(artifact_id: impl Into<Box<str>>, issue: &'static str) -> Self {
        Self {
            artifact_kind: "workflow",
            artifact_id: artifact_id.into(),
            issue,
        }
    }

    fn software(artifact_id: impl Into<Box<str>>, issue: &'static str) -> Self {
        Self {
            artifact_kind: "software",
            artifact_id: artifact_id.into(),
            issue,
        }
    }

    #[must_use]
    pub const fn artifact_kind(&self) -> &str {
        self.artifact_kind
    }

    #[must_use]
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    #[must_use]
    pub const fn issue(&self) -> &str {
        self.issue
    }
}

#[derive(Clone, Debug)]
pub struct DesktopEnvironment {
    composition: CompositionEnvironment,
    adapter_requirements: BTreeMap<Box<str>, AdapterRequirement>,
    font_families: BTreeSet<Box<str>>,
}

impl DesktopEnvironment {
    #[must_use]
    pub fn new(
        requirements: impl IntoIterator<Item = AdapterRequirement>,
        font_families: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        let adapter_requirements = requirements
            .into_iter()
            .map(|requirement| (requirement.adapter_id().as_str().into(), requirement))
            .collect::<BTreeMap<_, _>>();
        let font_families = font_families
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        let composition = composition_environment(&adapter_requirements, &font_families);
        Self {
            composition,
            adapter_requirements,
            font_families,
        }
    }

    fn add_requirements(&mut self, requirements: impl IntoIterator<Item = AdapterRequirement>) {
        for requirement in requirements {
            self.adapter_requirements
                .entry(requirement.adapter_id().as_str().into())
                .or_insert(requirement);
        }
        self.composition = composition_environment(&self.adapter_requirements, &self.font_families);
    }

    fn replace_font_families(
        &mut self,
        font_families: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) {
        self.font_families = font_families.into_iter().map(Into::into).collect();
        self.composition = composition_environment(&self.adapter_requirements, &self.font_families);
    }
}

fn composition_environment(
    requirements: &BTreeMap<Box<str>, AdapterRequirement>,
    font_families: &BTreeSet<Box<str>>,
) -> CompositionEnvironment {
    CompositionEnvironment::new(
        requirements.values().map(|requirement| {
            AdapterInput::new(requirement.adapter_id().as_str(), requirement.features())
        }),
        font_families.iter().cloned(),
    )
}

pub struct DesktopBackend {
    root: PathBuf,
    environment: DesktopEnvironment,
    selected_software_id: Option<Box<str>>,
    software: BTreeMap<Box<str>, SoftwareState>,
    local_software: BTreeMap<Box<str>, DesktopSoftwareArtifact>,
    dictionaries: BTreeMap<Box<str>, DictionaryView>,
    dictionary_installations: BTreeMap<Box<str>, DictionaryInstallationSummaryView>,
    artifact_warnings: Vec<ArtifactWarningView>,
    workflows: BTreeMap<Box<str>, WorkflowArtifact>,
    enabled_workflows: BTreeMap<Box<str>, u64>,
    enabled_workflow_ids: Vec<Box<str>>,
}

impl DesktopBackend {


    /// The dictionary directory of this workspace, including custom data roots.
    pub fn dictionary_directory(&self) -> PathBuf {
        self.root.join("dictionaries")
    }

    pub fn open(root: impl AsRef<Path>) -> Result<Self, BackendError> {
        Self::open_with_environment(root, DesktopEnvironment::new([], Vec::<Box<str>>::new()))
    }

    pub fn open_with_environment(
        root: impl AsRef<Path>,
        mut environment: DesktopEnvironment,
    ) -> Result<Self, BackendError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("dictionaries"))
            .map_err(|_| BackendError::Storage("create-dictionary-directory"))?;
        fs::create_dir_all(root.join("workflows-v4"))
            .map_err(|_| BackendError::Storage("create-workflow-directory"))?;

        let loaded_software = software::load(&root)?;
        environment.add_requirements(loaded_software.requirements);
        let selected_software_id = loaded_software.selected_software_id;
        let software = loaded_software.software;
        let local_software = loaded_software.local_software;
        let dictionary_load = read_dictionaries(&root)?;
        let dictionaries = dictionary_load.dictionaries;
        let mut artifact_warnings = loaded_software.warnings;
        artifact_warnings.extend(dictionary_load.warnings);
        let dictionary_installations = read_dictionary_installations(&root)?;
        let workflow_load = read_workflows(&root)?;
        let workflows = workflow_load.workflows;
        artifact_warnings.extend(workflow_load.warnings);
        let enabled_workflows = read_workflow_state(&root, &workflows)?;
        let enabled_workflow_ids = enabled_workflows.keys().cloned().collect();
        let mut backend = Self {
            root,
            environment,
            selected_software_id,
            software,
            local_software,
            dictionaries,
            dictionary_installations,
            artifact_warnings,
            workflows,
            enabled_workflows,
            enabled_workflow_ids,
        };
        let invalid_enabled = backend
            .enabled_workflows
            .keys()
            .filter(|workflow_id| {
                backend.workflows.get(*workflow_id).is_none_or(|artifact| {
                    backend.validate_workflow_activation(artifact).is_err()
                        || backend.activation_conflict(artifact).is_some()
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        for workflow_id in invalid_enabled {
            backend.enabled_workflows.remove(&workflow_id);
            backend
                .artifact_warnings
                .push(ArtifactWarningView::workflow(
                    workflow_id,
                    "disabled_invalid_dependency",
                ));
        }
        backend.enabled_workflow_ids = backend.enabled_workflows.keys().cloned().collect();
        Ok(backend)
    }

    pub fn replace_font_families(
        &mut self,
        font_families: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) {
        self.environment.replace_font_families(font_families);
    }
}
