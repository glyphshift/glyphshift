//! Desktop-facing product model for software, dictionaries, and workflows.

use glyphshift_adapter_registry::{AdapterRequirement, AdapterVersion, AdapterVersionRequirement};
use glyphshift_domain::{AdapterId, Feature, Generation, RouteLimits, RouteOperator, RouteProgram};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use glyphshift_workflow::{
    resolve as resolve_workflow, AdapterInput, AdapterPlan, CompiledWorkflow,
    CompositionEnvironment, Dictionary as WorkflowDictionary,
    DictionaryEntry as WorkflowDictionaryEntry, FontProfile as WorkflowFontProfile,
    FontProfileBinding as WorkflowFontProfileBinding, ResolveError, SoftwareInput,
    Workflow as WorkflowDefinition, WorkflowTarget as WorkflowDefinitionTarget,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const EXTENSION_SCHEMA: &str = "glyphshift.extension/1";
const DICTIONARY_SCHEMA: &str = "glyphshift.dictionary/2";
const FONT_PROFILE_SCHEMA: &str = "glyphshift.font-profile/1";
const WORKFLOW_SCHEMA: &str = "glyphshift.workflow/2";
const WORKFLOW_STATE_SCHEMA: &str = "glyphshift.workflow-state/1";
const DESKTOP_STATE_SCHEMA: &str = "glyphshift.desktop-state/1";
const DEFAULT_LOCALE: &str = "zh-CN";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendError {
    Storage(&'static str),
    InvalidArtifact(&'static str),
    DuplicateSoftware(Box<str>),
    DuplicateDictionary(Box<str>),
    DuplicateFontProfile(Box<str>),
    UnknownSoftware(Box<str>),
    UnknownDictionary(Box<str>),
    UnknownFontProfile(Box<str>),
    DuplicateWorkflow(Box<str>),
    UnknownWorkflow(Box<str>),
    WorkflowRejected(ResolveError),
    DictionaryReferenced {
        dictionary_id: Box<str>,
        workflow_ids: Vec<Box<str>>,
    },
    FontProfileReferenced {
        font_profile_id: Box<str>,
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
pub struct CapabilityView {
    state: &'static str,
    enabled: bool,
    coverage: u8,
    detail: &'static str,
    generation: Option<u64>,
}

impl CapabilityView {
    fn unavailable(detail: &'static str) -> Self {
        Self {
            state: "unavailable",
            enabled: false,
            coverage: 0,
            detail,
            generation: None,
        }
    }

    fn pending_observation() -> Self {
        Self {
            state: "limited",
            enabled: false,
            coverage: 0,
            detail: "capability.compatibility-pending",
            generation: None,
        }
    }
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocationView {
    id: Box<str>,
    label: Box<str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareView {
    id: Box<str>,
    name: Box<str>,
    description: Box<str>,
    vendor: Box<str>,
    version: Box<str>,
    executable_name: Box<str>,
    executable_path: Option<Box<str>>,
    monogram: Box<str>,
    last_used: Option<Box<str>>,
    locale: Box<str>,
    connected: bool,
    translation: CapabilityView,
    font: CapabilityView,
    observe: CapabilityView,
    locations: Vec<LocationView>,
}

impl SoftwareView {
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
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn executable_name(&self) -> &str {
        &self.executable_name
    }

    #[must_use]
    pub fn executable_path(&self) -> Option<&str> {
        self.executable_path.as_deref()
    }

    #[must_use]
    pub const fn translation_capability(&self) -> &CapabilityView {
        &self.translation
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSnapshot {
    selected_software_id: Option<Box<str>>,
    software: Vec<SoftwareView>,
    dictionaries: Vec<DictionarySummaryView>,
    font_profiles: Vec<FontProfileSummaryView>,
    workflows: Vec<WorkflowSummaryView>,
    activations: Vec<WorkflowActivationSnapshot>,
}

impl DesktopSnapshot {
    #[must_use]
    pub fn selected_software_id(&self) -> Option<&str> {
        self.selected_software_id.as_deref()
    }

    #[must_use]
    pub fn software(&self) -> &[SoftwareView] {
        &self.software
    }

    #[must_use]
    pub fn dictionaries(&self) -> &[DictionarySummaryView] {
        &self.dictionaries
    }

    #[must_use]
    pub fn font_profiles(&self) -> &[FontProfileSummaryView] {
        &self.font_profiles
    }

    #[must_use]
    pub fn workflows(&self) -> &[WorkflowSummaryView] {
        &self.workflows
    }

    #[must_use]
    pub fn activations(&self) -> &[WorkflowActivationSnapshot] {
        &self.activations
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowActivationSnapshot {
    workflow_id: Box<str>,
    revision: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionarySummaryView {
    metadata: DictionaryMetadata,
    revision: u64,
    entry_count: usize,
}

impl DictionarySummaryView {
    #[must_use]
    pub fn id(&self) -> &str {
        self.metadata.id()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.metadata.name()
    }

    #[must_use]
    pub fn description(&self) -> &str {
        self.metadata.description()
    }

    #[must_use]
    pub const fn metadata(&self) -> &DictionaryMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn entry_count(&self) -> usize {
        self.entry_count
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontProfileSummaryView {
    metadata: FontProfileMetadata,
    revision: u64,
    families: Vec<Box<str>>,
    resolved_family: Option<Box<str>>,
}

impl FontProfileSummaryView {
    #[must_use]
    pub fn id(&self) -> &str {
        self.metadata.id()
    }

    #[must_use]
    pub const fn metadata(&self) -> &FontProfileMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn families(&self) -> &[Box<str>] {
        &self.families
    }

    #[must_use]
    pub fn resolved_family(&self) -> Option<&str> {
        self.resolved_family.as_deref()
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSummaryView {
    id: Box<str>,
    name: Box<str>,
    description: Box<str>,
    revision: u64,
    software_ids: Vec<Box<str>>,
    dictionary_ids: Vec<Box<str>>,
    targets: Vec<WorkflowTargetView>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutableSelection {
    path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareEdit {
    extension_id: Box<str>,
    display_name: Box<str>,
    description: Box<str>,
    executable_path: PathBuf,
}

impl SoftwareEdit {
    #[must_use]
    pub fn new(
        extension_id: impl Into<Box<str>>,
        display_name: impl Into<Box<str>>,
        executable_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            extension_id: extension_id.into(),
            display_name: display_name.into(),
            description: "".into(),
            executable_path: executable_path.into(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.description = description.into();
        self
    }
}

impl ExecutableSelection {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryRuleCreate {
    location: Box<str>,
    context: Option<DictionaryRuleContext>,
    source: Box<str>,
    translation: Option<Box<str>>,
}

impl DictionaryRuleCreate {
    #[must_use]
    pub fn replace(
        location: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        translation: impl Into<Box<str>>,
    ) -> Self {
        Self {
            location: location.into(),
            context: None,
            source: source.into(),
            translation: Some(translation.into()),
        }
    }

    #[must_use]
    pub fn keep(location: impl Into<Box<str>>, source: impl Into<Box<str>>) -> Self {
        Self {
            location: location.into(),
            context: None,
            source: source.into(),
            translation: None,
        }
    }

    #[must_use]
    pub fn with_context(mut self, kind: impl Into<Box<str>>, key: impl Into<Box<str>>) -> Self {
        self.context = Some(DictionaryRuleContext {
            kind: kind.into(),
            key: key.into(),
        });
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct DictionaryRuleContext {
    kind: Box<str>,
    key: Box<str>,
}

impl DictionaryRuleContext {
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DictionaryRuleKey {
    location: Box<str>,
    context: Option<DictionaryRuleContext>,
    source: Box<str>,
}

impl DictionaryRuleKey {
    #[must_use]
    pub fn new(location: impl Into<Box<str>>, source: impl Into<Box<str>>) -> Self {
        Self {
            location: location.into(),
            context: None,
            source: source.into(),
        }
    }

    #[must_use]
    pub fn with_context(mut self, kind: impl Into<Box<str>>, key: impl Into<Box<str>>) -> Self {
        self.context = Some(DictionaryRuleContext {
            kind: kind.into(),
            key: key.into(),
        });
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryMetadata {
    id: Box<str>,
    release_version: Box<str>,
    name: Box<str>,
    description: Box<str>,
    source_locale: Box<str>,
    target_locale: Box<str>,
    authors: Vec<Box<str>>,
    license: Option<Box<str>>,
    homepage: Option<Box<str>>,
    tags: Vec<Box<str>>,
}

impl DictionaryMetadata {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn release_version(&self) -> &str {
        &self.release_version
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
    pub fn source_locale(&self) -> &str {
        &self.source_locale
    }
    #[must_use]
    pub fn target_locale(&self) -> &str {
        &self.target_locale
    }
    #[must_use]
    pub fn authors(&self) -> &[Box<str>] {
        &self.authors
    }
    #[must_use]
    pub fn license(&self) -> Option<&str> {
        self.license.as_deref()
    }
    #[must_use]
    pub fn homepage(&self) -> Option<&str> {
        self.homepage.as_deref()
    }
    #[must_use]
    pub fn tags(&self) -> &[Box<str>] {
        &self.tags
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryCreate {
    metadata: DictionaryMetadata,
    entries: Vec<DictionaryRuleCreate>,
}

impl DictionaryCreate {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        source_locale: impl Into<Box<str>>,
        target_locale: impl Into<Box<str>>,
    ) -> Self {
        Self {
            metadata: DictionaryMetadata {
                id: id.into(),
                release_version: "0.1.0".into(),
                name: name.into(),
                description: "".into(),
                source_locale: source_locale.into(),
                target_locale: target_locale.into(),
                authors: Vec::new(),
                license: None,
                homepage: None,
                tags: Vec::new(),
            },
            entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.metadata.description = description.into();
        self
    }

    #[must_use]
    pub fn with_release_version(mut self, version: impl Into<Box<str>>) -> Self {
        self.metadata.release_version = version.into();
        self
    }

    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_authors(mut self, authors: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.authors = authors.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_license(mut self, license: impl Into<Box<str>>) -> Self {
        self.metadata.license = Some(license.into());
        self
    }

    #[must_use]
    pub fn with_homepage(mut self, homepage: impl Into<Box<str>>) -> Self {
        self.metadata.homepage = Some(homepage.into());
        self
    }

    #[must_use]
    pub fn with_entries(mut self, entries: impl IntoIterator<Item = DictionaryRuleCreate>) -> Self {
        self.entries = entries.into_iter().collect();
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryEdit {
    metadata: DictionaryMetadata,
    base_revision: u64,
    entries: Vec<DictionaryRuleCreate>,
}

impl DictionaryEdit {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        source_locale: impl Into<Box<str>>,
        target_locale: impl Into<Box<str>>,
        base_revision: u64,
    ) -> Self {
        let create = DictionaryCreate::new(id, name, source_locale, target_locale);
        Self {
            metadata: create.metadata,
            base_revision,
            entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.metadata.description = description.into();
        self
    }

    #[must_use]
    pub fn with_release_version(mut self, version: impl Into<Box<str>>) -> Self {
        self.metadata.release_version = version.into();
        self
    }

    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_authors(mut self, authors: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.authors = authors.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_license(mut self, license: impl Into<Box<str>>) -> Self {
        self.metadata.license = Some(license.into());
        self
    }

    #[must_use]
    pub fn with_homepage(mut self, homepage: impl Into<Box<str>>) -> Self {
        self.metadata.homepage = Some(homepage.into());
        self
    }

    #[must_use]
    pub fn with_entries(mut self, entries: impl IntoIterator<Item = DictionaryRuleCreate>) -> Self {
        self.entries = entries.into_iter().collect();
        self
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryRuleView {
    location: Box<str>,
    context: Option<DictionaryRuleContext>,
    source: Box<str>,
    translation: Option<Box<str>>,
}

impl DictionaryRuleView {
    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }

    #[must_use]
    pub const fn context(&self) -> Option<&DictionaryRuleContext> {
        self.context.as_ref()
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translation(&self) -> Option<&str> {
        self.translation.as_deref()
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryView {
    metadata: DictionaryMetadata,
    revision: u64,
    entries: Vec<DictionaryRuleView>,
}

impl DictionaryView {
    #[must_use]
    pub const fn metadata(&self) -> &DictionaryMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn id(&self) -> &str {
        self.metadata.id()
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn entries(&self) -> &[DictionaryRuleView] {
        &self.entries
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontProfileMetadata {
    id: Box<str>,
    name: Box<str>,
    description: Box<str>,
}

impl FontProfileMetadata {
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
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontProfileCreate {
    metadata: FontProfileMetadata,
    families: Vec<Box<str>>,
}

impl FontProfileCreate {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        families: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            metadata: FontProfileMetadata {
                id: id.into(),
                name: name.into(),
                description: "".into(),
            },
            families: families.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.metadata.description = description.into();
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontProfileEdit {
    metadata: FontProfileMetadata,
    base_revision: u64,
    families: Vec<Box<str>>,
}

impl FontProfileEdit {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        families: impl IntoIterator<Item = impl Into<Box<str>>>,
        base_revision: u64,
    ) -> Self {
        Self {
            metadata: FontProfileMetadata {
                id: id.into(),
                name: name.into(),
                description: "".into(),
            },
            base_revision,
            families: families.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.metadata.description = description.into();
        self
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontProfileView {
    metadata: FontProfileMetadata,
    revision: u64,
    families: Vec<Box<str>>,
    resolved_family: Option<Box<str>>,
}

impl FontProfileView {
    #[must_use]
    pub const fn metadata(&self) -> &FontProfileMetadata {
        &self.metadata
    }
    #[must_use]
    pub fn id(&self) -> &str {
        self.metadata.id()
    }
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    #[must_use]
    pub fn families(&self) -> &[Box<str>] {
        &self.families
    }
    #[must_use]
    pub fn resolved_family(&self) -> Option<&str> {
        self.resolved_family.as_deref()
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum FontProfileScope {
    All,
    Locations { location_ids: Vec<Box<str>> },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontProfileBinding {
    font_profile_id: Box<str>,
    scope: FontProfileScope,
}

impl FontProfileBinding {
    #[must_use]
    pub fn all(font_profile_id: impl Into<Box<str>>) -> Self {
        Self {
            font_profile_id: font_profile_id.into(),
            scope: FontProfileScope::All,
        }
    }

    #[must_use]
    pub fn locations(
        font_profile_id: impl Into<Box<str>>,
        location_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            font_profile_id: font_profile_id.into(),
            scope: FontProfileScope::Locations {
                location_ids: location_ids.into_iter().map(Into::into).collect(),
            },
        }
    }

    #[must_use]
    pub fn font_profile_id(&self) -> &str {
        &self.font_profile_id
    }

    #[must_use]
    pub const fn scope(&self) -> &FontProfileScope {
        &self.scope
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowTargetCreate {
    software_id: Box<str>,
    adapter_plan: WorkflowAdapterPlan,
    dictionary_ids: Vec<Box<str>>,
    #[serde(default)]
    font_bindings: Vec<FontProfileBinding>,
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
            font_bindings: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_font_bindings(
        mut self,
        bindings: impl IntoIterator<Item = FontProfileBinding>,
    ) -> Self {
        self.font_bindings = bindings.into_iter().collect();
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
    targets: Vec<WorkflowTargetCreate>,
}

impl WorkflowCreate {
    #[must_use]
    pub fn new(id: impl Into<Box<str>>, name: impl Into<Box<str>>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: "".into(),
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
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEdit {
    id: Box<str>,
    name: Box<str>,
    #[serde(default)]
    description: Box<str>,
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
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowTargetView {
    software_id: Box<str>,
    adapter_plan: WorkflowAdapterPlan,
    dictionary_ids: Vec<Box<str>>,
    font_bindings: Vec<FontProfileBinding>,
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
    pub fn font_bindings(&self) -> &[FontProfileBinding] {
        &self.font_bindings
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowView {
    id: Box<str>,
    name: Box<str>,
    description: Box<str>,
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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ExtensionArtifact {
    schema: Box<str>,
    id: Box<str>,
    version: Box<str>,
    name: Box<str>,
    vendor: Box<str>,
    #[serde(default)]
    executables: Vec<Box<str>>,
    #[serde(default)]
    runtime: Option<ExtensionRuntimeArtifact>,
    locations: Vec<ExtensionLocationArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ExtensionRuntimeArtifact {
    #[serde(default)]
    capabilities: Vec<RuntimeCapabilityArtifact>,
    route: RuntimeRouteArtifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RuntimeCapabilityArtifact {
    adapter: Box<str>,
    version: [u16; 3],
    features: Vec<Box<str>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RuntimeRouteArtifact {
    kind: Box<str>,
    locations: Vec<Box<str>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ExtensionLocationArtifact {
    id: Box<str>,
    label: Box<str>,
    #[serde(default)]
    context: Option<ContextSchemaArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ContextSchemaArtifact {
    kind: Box<str>,
    label: Box<str>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DictionaryArtifact {
    schema: Box<str>,
    revision: u64,
    metadata: DictionaryMetadata,
    entries: Vec<DictionaryRuleArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DictionaryRuleArtifact {
    location: Box<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    context: Option<DictionaryRuleContext>,
    source: Box<str>,
    text: DictionaryTextArtifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DictionaryTextArtifact {
    Keep,
    Replace { text: Box<str> },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FontProfileArtifact {
    schema: Box<str>,
    revision: u64,
    metadata: FontProfileMetadata,
    families: Vec<Box<str>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowArtifact {
    schema: Box<str>,
    id: Box<str>,
    name: Box<str>,
    #[serde(default)]
    description: Box<str>,
    revision: u64,
    targets: Vec<WorkflowTargetArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowTargetArtifact {
    software_id: Box<str>,
    adapter_plan: WorkflowAdapterPlan,
    dictionary_ids: Vec<Box<str>>,
    #[serde(default)]
    font_bindings: Vec<FontProfileBinding>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowStateArtifact {
    schema: Box<str>,
    enabled: BTreeMap<Box<str>, u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopStateArtifact {
    schema: Box<str>,
    selected_software_id: Option<Box<str>>,
    #[serde(default)]
    software: BTreeMap<Box<str>, DesktopSoftwareArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopSoftwareArtifact {
    display_name: Box<str>,
    #[serde(default)]
    description: Box<str>,
    executable_path: Box<str>,
}

struct SoftwareState {
    artifact: ExtensionArtifact,
    locale: Box<str>,
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
    font_profiles: BTreeMap<Box<str>, FontProfileView>,
    workflows: BTreeMap<Box<str>, WorkflowArtifact>,
    enabled_workflows: BTreeMap<Box<str>, u64>,
    enabled_workflow_ids: Vec<Box<str>>,
}

#[derive(Clone, Debug)]
pub struct DesktopRuntimeSpec {
    executable_names: Vec<Box<str>>,
    executable_paths: Vec<Box<str>>,
    requirements: Vec<AdapterRequirement>,
    publication: RuntimePublication,
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

impl DesktopRuntimeSpec {
    #[must_use]
    pub fn executable_names(&self) -> &[Box<str>] {
        &self.executable_names
    }

    #[must_use]
    pub fn executable_paths(&self) -> &[Box<str>] {
        &self.executable_paths
    }

    #[must_use]
    pub fn requirements(&self) -> &[AdapterRequirement] {
        &self.requirements
    }

    #[must_use]
    pub const fn publication(&self) -> &RuntimePublication {
        &self.publication
    }
}

impl DesktopBackend {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, BackendError> {
        Self::open_with_environment(root, DesktopEnvironment::new([], Vec::<Box<str>>::new()))
    }

    pub fn open_with_environment(
        root: impl AsRef<Path>,
        mut environment: DesktopEnvironment,
    ) -> Result<Self, BackendError> {
        let root = root.as_ref().to_path_buf();
        let extension_root = root.join("extensions");
        fs::create_dir_all(&extension_root)
            .map_err(|_| BackendError::Storage("create-extension-directory"))?;
        fs::create_dir_all(root.join("dictionaries"))
            .map_err(|_| BackendError::Storage("create-dictionary-directory"))?;
        fs::create_dir_all(root.join("font-profiles"))
            .map_err(|_| BackendError::Storage("create-font-profile-directory"))?;
        fs::create_dir_all(root.join("workflows"))
            .map_err(|_| BackendError::Storage("create-workflow-directory"))?;

        let mut extension_paths = fs::read_dir(&extension_root)
            .map_err(|_| BackendError::Storage("read-extension-directory"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        extension_paths.sort();

        let mut software = BTreeMap::new();
        for path in extension_paths {
            let artifact: ExtensionArtifact = read_json(&path, "extension-json")?;
            validate_extension(&artifact, &path)?;
            if software
                .insert(
                    artifact.id.clone(),
                    SoftwareState {
                        artifact,
                        locale: DEFAULT_LOCALE.into(),
                    },
                )
                .is_some()
            {
                return Err(BackendError::DuplicateSoftware(
                    "duplicate-extension".into(),
                ));
            }
        }
        environment.add_requirements(
            software
                .values()
                .filter_map(|state| state.artifact.runtime.as_ref())
                .flat_map(|runtime| runtime.capabilities.iter())
                .map(runtime_requirement)
                .collect::<Result<Vec<_>, _>>()?,
        );
        let desktop_state = read_desktop_state(&root)?;
        let selected_software_id = desktop_state
            .selected_software_id
            .and_then(|selected| software.contains_key(&selected).then_some(selected))
            .or_else(|| software.keys().next().cloned());
        let local_software = desktop_state
            .software
            .into_iter()
            .filter(|(extension_id, _)| software.contains_key(extension_id))
            .collect();
        let dictionaries = read_dictionaries(&root)?;
        let font_profiles = read_font_profiles(&root, &environment.font_families)?;
        let workflows = read_workflows(&root)?;
        let enabled_workflows = read_workflow_state(&root, &workflows)?;
        let enabled_workflow_ids = enabled_workflows.keys().cloned().collect();
        let backend = Self {
            root,
            environment,
            selected_software_id,
            software,
            local_software,
            dictionaries,
            font_profiles,
            workflows,
            enabled_workflows,
            enabled_workflow_ids,
        };
        for workflow_id in backend.enabled_workflows.keys() {
            let artifact = backend
                .workflows
                .get(workflow_id)
                .ok_or(BackendError::InvalidArtifact("workflow-state-contract"))?;
            backend
                .validate_workflow_activation(artifact)
                .map_err(|_| BackendError::InvalidArtifact("workflow-state-contract"))?;
            if backend.activation_conflict(artifact).is_some() {
                return Err(BackendError::InvalidArtifact("workflow-state-contract"));
            }
        }
        Ok(backend)
    }

    pub fn dictionary(&self, dictionary_id: &str) -> Result<&DictionaryView, BackendError> {
        self.dictionaries
            .get(dictionary_id)
            .ok_or_else(|| BackendError::UnknownDictionary(dictionary_id.into()))
    }

    pub fn create_dictionary(
        &mut self,
        create: DictionaryCreate,
    ) -> Result<DictionaryView, BackendError> {
        if self.dictionaries.contains_key(&create.metadata.id) {
            return Err(BackendError::DuplicateDictionary(create.metadata.id));
        }
        let artifact = dictionary_artifact(create)?;
        let serialized = serde_json::to_string(&artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-dictionary"))?;
        let path = dictionary_path(&self.root, artifact.metadata.id())?;
        write_atomic(&path, &serialized)?;
        let view = dictionary_view(&artifact);
        self.dictionaries
            .insert(artifact.metadata.id.clone(), view.clone());
        Ok(view)
    }

    pub fn update_dictionary(
        &mut self,
        edit: DictionaryEdit,
    ) -> Result<DictionaryView, BackendError> {
        let current = self
            .dictionaries
            .get(&edit.metadata.id)
            .ok_or_else(|| BackendError::UnknownDictionary(edit.metadata.id.clone()))?;
        if current.revision != edit.base_revision {
            return Err(BackendError::RevisionConflict {
                current: current.revision,
            });
        }
        let artifact = dictionary_artifact_at_revision(
            DictionaryCreate {
                metadata: edit.metadata,
                entries: edit.entries,
            },
            current.revision + 1,
        )?;
        let view = dictionary_view(&artifact);
        for workflow_id in self.enabled_workflows.keys() {
            let workflow = &self.workflows[workflow_id];
            if workflow.targets.iter().any(|target| {
                target
                    .dictionary_ids
                    .iter()
                    .any(|dictionary_id| dictionary_id == &artifact.metadata.id)
            }) {
                self.resolve_workflow_artifact_with_dictionary(workflow, Some(&view))?;
            }
        }
        let serialized = serde_json::to_string(&artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-dictionary"))?;
        let path = dictionary_path(&self.root, artifact.metadata.id())?;
        write_atomic(&path, &serialized)?;
        self.dictionaries
            .insert(artifact.metadata.id.clone(), view.clone());
        Ok(view)
    }

    pub fn upsert_dictionary_rule(
        &mut self,
        dictionary_id: &str,
        rule: DictionaryRuleCreate,
        replacing: Option<DictionaryRuleKey>,
        base_revision: u64,
    ) -> Result<DictionaryView, BackendError> {
        let current = self
            .dictionaries
            .get(dictionary_id)
            .ok_or_else(|| BackendError::UnknownDictionary(dictionary_id.into()))?
            .clone();
        if current.revision != base_revision {
            return Err(BackendError::RevisionConflict {
                current: current.revision,
            });
        }
        let mut entries = current
            .entries
            .iter()
            .map(dictionary_rule_create)
            .collect::<Vec<_>>();
        let next_key = dictionary_rule_key(&rule);
        let index = replacing.as_ref().and_then(|key| {
            entries
                .iter()
                .position(|entry| dictionary_rule_key(entry) == *key)
        });
        if replacing.is_some() && index.is_none() {
            return Err(BackendError::InvalidInput("dictionary-rule"));
        }
        if entries.iter().enumerate().any(|(candidate_index, entry)| {
            Some(candidate_index) != index && dictionary_rule_key(entry) == next_key
        }) {
            return Err(BackendError::InvalidInput("dictionary-rule-duplicate"));
        }
        if let Some(index) = index {
            entries[index] = rule;
        } else {
            entries.push(rule);
        }
        self.update_dictionary(DictionaryEdit {
            metadata: current.metadata,
            base_revision: current.revision,
            entries,
        })
    }

    pub fn delete_dictionary_rules(
        &mut self,
        dictionary_id: &str,
        rule_keys: impl IntoIterator<Item = DictionaryRuleKey>,
        base_revision: u64,
    ) -> Result<DictionaryView, BackendError> {
        let current = self
            .dictionaries
            .get(dictionary_id)
            .ok_or_else(|| BackendError::UnknownDictionary(dictionary_id.into()))?
            .clone();
        if current.revision != base_revision {
            return Err(BackendError::RevisionConflict {
                current: current.revision,
            });
        }
        let rule_keys = rule_keys.into_iter().collect::<BTreeSet<_>>();
        let existing_keys = current
            .entries
            .iter()
            .map(dictionary_rule_view_key)
            .collect::<BTreeSet<_>>();
        if rule_keys.is_empty() || !rule_keys.is_subset(&existing_keys) {
            return Err(BackendError::InvalidInput("dictionary-rule"));
        }
        let entries = current
            .entries
            .iter()
            .filter(|entry| !rule_keys.contains(&dictionary_rule_view_key(entry)))
            .map(dictionary_rule_create)
            .collect::<Vec<_>>();
        self.update_dictionary(DictionaryEdit {
            metadata: current.metadata,
            base_revision: current.revision,
            entries,
        })
    }

    pub fn delete_dictionaries<'a>(
        &mut self,
        dictionary_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), BackendError> {
        let dictionary_ids = dictionary_ids.into_iter().collect::<BTreeSet<_>>();
        for dictionary_id in &dictionary_ids {
            if !self.dictionaries.contains_key(*dictionary_id) {
                return Err(BackendError::UnknownDictionary((*dictionary_id).into()));
            }
            let workflow_ids = self
                .workflows
                .values()
                .filter(|workflow| {
                    workflow.targets.iter().any(|target| {
                        target
                            .dictionary_ids
                            .iter()
                            .any(|candidate| candidate.as_ref() == *dictionary_id)
                    })
                })
                .map(|workflow| workflow.id.clone())
                .collect::<Vec<_>>();
            if !workflow_ids.is_empty() {
                return Err(BackendError::DictionaryReferenced {
                    dictionary_id: (*dictionary_id).into(),
                    workflow_ids,
                });
            }
        }
        for dictionary_id in dictionary_ids {
            let path = dictionary_path(&self.root, dictionary_id)?;
            fs::remove_file(path).map_err(|_| BackendError::Storage("remove-dictionary"))?;
            self.dictionaries.remove(dictionary_id);
        }
        Ok(())
    }

    pub fn font_profile(&self, font_profile_id: &str) -> Result<&FontProfileView, BackendError> {
        self.font_profiles
            .get(font_profile_id)
            .ok_or_else(|| BackendError::UnknownFontProfile(font_profile_id.into()))
    }

    pub fn create_font_profile(
        &mut self,
        create: FontProfileCreate,
    ) -> Result<FontProfileView, BackendError> {
        if self.font_profiles.contains_key(&create.metadata.id) {
            return Err(BackendError::DuplicateFontProfile(create.metadata.id));
        }
        let artifact = font_profile_artifact(create, 1)?;
        let serialized = serde_json::to_string(&artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-font-profile"))?;
        write_atomic(
            &font_profile_path(&self.root, artifact.metadata.id())?,
            &serialized,
        )?;
        let view = font_profile_view(&artifact, &self.environment.font_families);
        self.font_profiles
            .insert(artifact.metadata.id.clone(), view.clone());
        Ok(view)
    }

    pub fn update_font_profile(
        &mut self,
        edit: FontProfileEdit,
    ) -> Result<FontProfileView, BackendError> {
        let current = self
            .font_profiles
            .get(edit.metadata.id())
            .ok_or_else(|| BackendError::UnknownFontProfile(edit.metadata.id.clone()))?;
        if current.revision != edit.base_revision {
            return Err(BackendError::RevisionConflict {
                current: current.revision,
            });
        }
        let artifact = font_profile_artifact(
            FontProfileCreate {
                metadata: edit.metadata,
                families: edit.families,
            },
            current.revision + 1,
        )?;
        let view = font_profile_view(&artifact, &self.environment.font_families);
        for workflow_id in self.enabled_workflows.keys() {
            let workflow = &self.workflows[workflow_id];
            if workflow.targets.iter().any(|target| {
                target
                    .font_bindings
                    .iter()
                    .any(|binding| binding.font_profile_id() == view.id())
            }) {
                self.resolve_workflow_artifact_with_overrides(workflow, None, Some(&view))?;
            }
        }
        let serialized = serde_json::to_string(&artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-font-profile"))?;
        write_atomic(
            &font_profile_path(&self.root, artifact.metadata.id())?,
            &serialized,
        )?;
        self.font_profiles
            .insert(artifact.metadata.id.clone(), view.clone());
        Ok(view)
    }

    pub fn delete_font_profiles<'a>(
        &mut self,
        font_profile_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), BackendError> {
        let font_profile_ids = font_profile_ids.into_iter().collect::<BTreeSet<_>>();
        for font_profile_id in &font_profile_ids {
            if !self.font_profiles.contains_key(*font_profile_id) {
                return Err(BackendError::UnknownFontProfile((*font_profile_id).into()));
            }
            let workflow_ids = self
                .workflows
                .values()
                .filter(|workflow| {
                    workflow.targets.iter().any(|target| {
                        target
                            .font_bindings
                            .iter()
                            .any(|binding| binding.font_profile_id() == *font_profile_id)
                    })
                })
                .map(|workflow| workflow.id.clone())
                .collect::<Vec<_>>();
            if !workflow_ids.is_empty() {
                return Err(BackendError::FontProfileReferenced {
                    font_profile_id: (*font_profile_id).into(),
                    workflow_ids,
                });
            }
        }
        for font_profile_id in font_profile_ids {
            fs::remove_file(font_profile_path(&self.root, font_profile_id)?)
                .map_err(|_| BackendError::Storage("remove-font-profile"))?;
            self.font_profiles.remove(font_profile_id);
        }
        Ok(())
    }

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
            revision: 1,
            targets: create
                .targets
                .into_iter()
                .map(|target| WorkflowTargetArtifact {
                    software_id: target.software_id,
                    adapter_plan: target.adapter_plan,
                    dictionary_ids: target.dictionary_ids,
                    font_bindings: target.font_bindings,
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
            revision: current.revision + 1,
            targets: edit
                .targets
                .into_iter()
                .map(|target| WorkflowTargetArtifact {
                    software_id: target.software_id,
                    adapter_plan: target.adapter_plan,
                    dictionary_ids: target.dictionary_ids,
                    font_bindings: target.font_bindings,
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
                .with_font_bindings(target.font_bindings.iter().cloned())
            })
            .collect::<Vec<_>>();
        self.create_workflow(WorkflowCreate {
            id: new_workflow_id.into(),
            name: name.into(),
            description: source.description.clone(),
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

    fn validate_workflow_activation(
        &self,
        artifact: &WorkflowArtifact,
    ) -> Result<(), BackendError> {
        self.resolve_workflow_artifact(artifact).map(|_| ())
    }

    fn resolve_workflow_artifact(
        &self,
        artifact: &WorkflowArtifact,
    ) -> Result<CompiledWorkflow, BackendError> {
        self.resolve_workflow_artifact_with_overrides(artifact, None, None)
    }

    fn resolve_workflow_artifact_with_dictionary(
        &self,
        artifact: &WorkflowArtifact,
        dictionary_override: Option<&DictionaryView>,
    ) -> Result<CompiledWorkflow, BackendError> {
        self.resolve_workflow_artifact_with_overrides(artifact, dictionary_override, None)
    }

    fn resolve_workflow_artifact_with_overrides(
        &self,
        artifact: &WorkflowArtifact,
        dictionary_override: Option<&DictionaryView>,
        font_profile_override: Option<&FontProfileView>,
    ) -> Result<CompiledWorkflow, BackendError> {
        let definition = WorkflowDefinition::new(
            artifact.id.clone(),
            artifact.targets.iter().map(|target| {
                WorkflowDefinitionTarget::new(
                    target.software_id.clone(),
                    AdapterPlan::parallel(target.adapter_plan.adapter_ids.iter().cloned()),
                    target.dictionary_ids.iter().cloned(),
                )
                .with_font_bindings(target.font_bindings.iter().map(|binding| {
                    match &binding.scope {
                        FontProfileScope::All => {
                            WorkflowFontProfileBinding::all(binding.font_profile_id.clone())
                        }
                        FontProfileScope::Locations { location_ids } => {
                            WorkflowFontProfileBinding::locations(
                                binding.font_profile_id.clone(),
                                location_ids.iter().cloned(),
                            )
                        }
                    }
                }))
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
                    glyphshift_domain::Generation::new(target.font_bindings.iter().fold(
                        target.dictionary_ids.iter().fold(
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
                        ),
                        |generation, binding| {
                            let revision = font_profile_override
                                .filter(|profile| profile.id() == binding.font_profile_id())
                                .map(FontProfileView::revision)
                                .or_else(|| {
                                    self.font_profiles
                                        .get(binding.font_profile_id())
                                        .map(FontProfileView::revision)
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
        let font_profiles = self
            .font_profiles
            .values()
            .map(|profile| {
                font_profile_override
                    .filter(|candidate| candidate.id() == profile.id())
                    .map_or_else(|| font_profile_definition(profile), font_profile_definition)
            })
            .collect::<Vec<_>>();
        resolve_workflow(
            &definition,
            &software,
            &dictionaries,
            &font_profiles,
            &self.environment.composition,
        )
        .map_err(BackendError::WorkflowRejected)
    }

    fn activation_conflict(&self, artifact: &WorkflowArtifact) -> Option<BackendError> {
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
            requirements,
            publication: RuntimePublication::new(
                target.route().clone(),
                target.snapshot().clone(),
                target.font_policy().clone(),
            ),
        })
    }

    pub fn runtime_spec(&self, extension_id: &str) -> Result<DesktopRuntimeSpec, BackendError> {
        let state = self
            .software
            .get(extension_id)
            .ok_or_else(|| BackendError::UnknownSoftware(extension_id.into()))?;
        if state.artifact.executables.is_empty() {
            return Err(BackendError::InvalidArtifact("runtime-executable-missing"));
        }
        let (requirements, route) = state.artifact.runtime.as_ref().map_or_else(
            || {
                let location = state
                    .artifact
                    .locations
                    .first()
                    .ok_or(BackendError::InvalidArtifact("runtime-location-missing"))?;
                Ok((Vec::new(), RouteProgram::direct(location.id.clone())))
            },
            |runtime| {
                if runtime.capabilities.is_empty() {
                    return Err(BackendError::InvalidArtifact("runtime-capability-empty"));
                }
                let requirements = runtime
                    .capabilities
                    .iter()
                    .map(runtime_requirement)
                    .collect::<Result<Vec<_>, _>>()?;
                let route = runtime_route(&runtime.route, &state.artifact.locations)?;
                Ok((requirements, route))
            },
        )?;
        Ok(DesktopRuntimeSpec {
            executable_names: state.artifact.executables.clone(),
            executable_paths: self
                .local_software
                .get(extension_id)
                .map(|software| vec![software.executable_path.clone()])
                .unwrap_or_default(),
            requirements,
            publication: RuntimePublication::new(
                route,
                TranslationSnapshot::empty(Generation::new(0)),
                FontPolicy::empty(),
            ),
        })
    }

    fn target_requirements(
        &self,
        target: &glyphshift_workflow::CompiledTarget,
    ) -> Result<Vec<AdapterRequirement>, BackendError> {
        target
            .adapter_ids()
            .iter()
            .map(|adapter_id| {
                let requirement = self
                    .environment
                    .adapter_requirements
                    .get(adapter_id)
                    .ok_or(BackendError::InvalidArtifact("adapter-requirement-missing"))?;
                let supported = requirement.features().collect::<BTreeSet<_>>();
                Ok(AdapterRequirement::new(
                    requirement.adapter_id().clone(),
                    requirement.version_requirement(),
                    target
                        .requested_features()
                        .iter()
                        .copied()
                        .filter(|feature| supported.contains(feature)),
                ))
            })
            .collect()
    }

    #[must_use]
    pub fn snapshot(&self) -> DesktopSnapshot {
        let software = self
            .software
            .iter()
            .map(|(extension_id, state)| {
                software_view(state, self.local_software.get(extension_id))
            })
            .collect();
        DesktopSnapshot {
            selected_software_id: self.selected_software_id.clone(),
            software,
            dictionaries: self
                .dictionaries
                .values()
                .map(|dictionary| DictionarySummaryView {
                    metadata: dictionary.metadata.clone(),
                    revision: dictionary.revision,
                    entry_count: dictionary.entries.len(),
                })
                .collect(),
            font_profiles: self
                .font_profiles
                .values()
                .map(|profile| FontProfileSummaryView {
                    metadata: profile.metadata.clone(),
                    revision: profile.revision,
                    families: profile.families.clone(),
                    resolved_family: profile.resolved_family.clone(),
                })
                .collect(),
            workflows: self
                .workflows
                .values()
                .map(|workflow| WorkflowSummaryView {
                    id: workflow.id.clone(),
                    name: workflow.name.clone(),
                    description: workflow.description.clone(),
                    revision: workflow.revision,
                    software_ids: workflow
                        .targets
                        .iter()
                        .map(|target| target.software_id.clone())
                        .collect(),
                    dictionary_ids: workflow
                        .targets
                        .iter()
                        .flat_map(|target| target.dictionary_ids.iter().cloned())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                    targets: workflow
                        .targets
                        .iter()
                        .map(|target| WorkflowTargetView {
                            software_id: target.software_id.clone(),
                            adapter_plan: target.adapter_plan.clone(),
                            dictionary_ids: target.dictionary_ids.clone(),
                            font_bindings: target.font_bindings.clone(),
                        })
                        .collect(),
                })
                .collect(),
            activations: self
                .enabled_workflows
                .iter()
                .map(|(workflow_id, revision)| WorkflowActivationSnapshot {
                    workflow_id: workflow_id.clone(),
                    revision: *revision,
                })
                .collect(),
        }
    }

    pub fn add_software(
        &mut self,
        selection: ExecutableSelection,
    ) -> Result<DesktopSnapshot, BackendError> {
        let metadata = fs::metadata(&selection.path)
            .map_err(|_| BackendError::InvalidInput("software-executable"))?;
        let executable_name = selection
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|_| metadata.is_file())
            .filter(|value| {
                Path::new(value)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            })
            .ok_or(BackendError::InvalidInput("software-executable"))?;
        let name = Path::new(executable_name)
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or(BackendError::InvalidInput("software-executable"))?;
        let extension_id = next_local_extension_id(&self.software, name);
        let executable_path = selection
            .path
            .to_str()
            .filter(|path| Path::new(path).is_absolute())
            .ok_or(BackendError::InvalidInput("software-executable"))?;
        let artifact = ExtensionArtifact {
            schema: EXTENSION_SCHEMA.into(),
            id: extension_id.clone(),
            version: "0.1.0".into(),
            name: name.into(),
            vendor: "—".into(),
            executables: vec![executable_name.into()],
            runtime: None,
            locations: vec![ExtensionLocationArtifact {
                id: "main-ui".into(),
                label: "main-ui".into(),
                context: None,
            }],
        };
        let serialized = serde_json::to_string(&artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-extension"))?;
        let extension_path = self
            .root
            .join("extensions")
            .join(format!("{extension_id}.json"));
        write_atomic(&extension_path, &serialized)?;
        self.local_software.insert(
            extension_id.clone(),
            DesktopSoftwareArtifact {
                display_name: name.into(),
                description: "".into(),
                executable_path: executable_path.into(),
            },
        );
        write_desktop_state(&self.root, Some(&extension_id), &self.local_software)?;

        self.software.insert(
            extension_id.clone(),
            SoftwareState {
                artifact,
                locale: DEFAULT_LOCALE.into(),
            },
        );
        self.selected_software_id = Some(extension_id);
        Ok(self.snapshot())
    }

    pub fn validate_software_edit(&self, edit: &SoftwareEdit) -> Result<(), BackendError> {
        self.validated_software_edit(edit).map(|_| ())
    }

    pub fn update_software(&mut self, edit: SoftwareEdit) -> Result<DesktopSnapshot, BackendError> {
        let (display_name, description, executable_name, executable_path) =
            self.validated_software_edit(&edit)?;
        let state = self
            .software
            .get_mut(&edit.extension_id)
            .ok_or_else(|| BackendError::UnknownSoftware(edit.extension_id.clone()))?;
        state.artifact.executables = vec![executable_name.into()];
        let serialized = serde_json::to_string(&state.artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-extension"))?;
        write_atomic(
            &self
                .root
                .join("extensions")
                .join(format!("{}.json", edit.extension_id)),
            &serialized,
        )?;
        self.local_software.insert(
            edit.extension_id.clone(),
            DesktopSoftwareArtifact {
                display_name: display_name.into(),
                description: description.into(),
                executable_path: executable_path.into(),
            },
        );
        write_desktop_state(
            &self.root,
            self.selected_software_id.as_deref(),
            &self.local_software,
        )?;
        Ok(self.snapshot())
    }

    fn validated_software_edit<'a>(
        &self,
        edit: &'a SoftwareEdit,
    ) -> Result<(&'a str, &'a str, &'a str, &'a str), BackendError> {
        if !self.software.contains_key(&edit.extension_id) {
            return Err(BackendError::UnknownSoftware(edit.extension_id.clone()));
        }
        let display_name = edit.display_name.trim();
        if display_name.is_empty() || display_name.chars().count() > 128 {
            return Err(BackendError::InvalidInput("software-display-name"));
        }
        let description = edit.description.trim();
        if description.chars().count() > 512 {
            return Err(BackendError::InvalidInput("software-description"));
        }
        let metadata = fs::metadata(&edit.executable_path)
            .map_err(|_| BackendError::InvalidInput("software-executable"))?;
        let executable_name = edit
            .executable_path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|_| metadata.is_file())
            .filter(|value| {
                Path::new(value)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            })
            .ok_or(BackendError::InvalidInput("software-executable"))?;
        let executable_path = edit
            .executable_path
            .to_str()
            .filter(|path| Path::new(path).is_absolute())
            .ok_or(BackendError::InvalidInput("software-executable"))?;
        Ok((display_name, description, executable_name, executable_path))
    }

    pub fn select_software(&mut self, extension_id: &str) -> Result<DesktopSnapshot, BackendError> {
        if !self.software.contains_key(extension_id) {
            return Err(BackendError::UnknownSoftware(extension_id.into()));
        }
        write_desktop_state(&self.root, Some(extension_id), &self.local_software)?;
        self.selected_software_id = Some(extension_id.into());
        Ok(self.snapshot())
    }

    pub fn remove_software(&mut self, extension_id: &str) -> Result<DesktopSnapshot, BackendError> {
        if !safe_identifier(extension_id) || !self.software.contains_key(extension_id) {
            return Err(BackendError::UnknownSoftware(extension_id.into()));
        }
        let workflow_ids = self
            .workflows
            .values()
            .filter(|workflow| {
                workflow
                    .targets
                    .iter()
                    .any(|target| target.software_id.as_ref() == extension_id)
            })
            .map(|workflow| workflow.id.clone())
            .collect::<Vec<_>>();
        if !workflow_ids.is_empty() {
            return Err(BackendError::SoftwareReferenced {
                software_id: extension_id.into(),
                workflow_ids,
            });
        }
        let extension_path = self
            .root
            .join("extensions")
            .join(format!("{extension_id}.json"));
        if extension_path.exists() {
            fs::remove_file(&extension_path)
                .map_err(|_| BackendError::Storage("remove-extension"))?;
        }
        self.software.remove(extension_id);
        self.local_software.remove(extension_id);
        if self.selected_software_id.as_deref() == Some(extension_id) {
            self.selected_software_id = self.software.keys().next().cloned();
        }
        write_desktop_state(
            &self.root,
            self.selected_software_id.as_deref(),
            &self.local_software,
        )?;
        Ok(self.snapshot())
    }
}

fn validate_extension(artifact: &ExtensionArtifact, path: &Path) -> Result<(), BackendError> {
    if artifact.schema.as_ref() != EXTENSION_SCHEMA
        || !safe_identifier(&artifact.id)
        || artifact.name.trim().is_empty()
        || artifact.version.trim().is_empty()
        || path.file_stem().and_then(|value| value.to_str()) != Some(&artifact.id)
        || artifact.executables.iter().any(|executable| {
            executable.trim().is_empty()
                || Path::new(executable.as_ref())
                    .file_name()
                    .and_then(|value| value.to_str())
                    != Some(executable)
        })
        || artifact
            .locations
            .iter()
            .any(|location| !safe_identifier(&location.id) || location.label.trim().is_empty())
    {
        return Err(BackendError::InvalidArtifact("extension-contract"));
    }
    Ok(())
}

fn runtime_requirement(
    artifact: &RuntimeCapabilityArtifact,
) -> Result<AdapterRequirement, BackendError> {
    if !safe_identifier(&artifact.adapter) || artifact.features.is_empty() {
        return Err(BackendError::InvalidArtifact("runtime-capability"));
    }
    let features = artifact
        .features
        .iter()
        .map(|feature| match feature.as_ref() {
            "text_observe" => Ok(Feature::TextObserve),
            "text_replace" => Ok(Feature::TextReplace),
            "font_substitute" => Ok(Feature::FontSubstitute),
            "layout_adjust" => Ok(Feature::LayoutAdjust),
            "resource_replace" => Ok(Feature::ResourceReplace),
            _ => Err(BackendError::InvalidArtifact("runtime-feature")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AdapterRequirement::new(
        AdapterId::new(artifact.adapter.clone()),
        AdapterVersionRequirement::Exact(AdapterVersion::new(
            artifact.version[0],
            artifact.version[1],
            artifact.version[2],
        )),
        features,
    ))
}

fn runtime_route(
    route: &RuntimeRouteArtifact,
    locations: &[ExtensionLocationArtifact],
) -> Result<RouteProgram, BackendError> {
    if route.locations.is_empty()
        || route.locations.iter().any(|route_location| {
            !locations
                .iter()
                .any(|location| location.id == *route_location)
        })
    {
        return Err(BackendError::InvalidArtifact("runtime-route"));
    }
    match route.kind.as_ref() {
        "direct" if route.locations.len() == 1 => {
            Ok(RouteProgram::direct(route.locations[0].clone()))
        }
        "fallback" => Ok(RouteProgram::new(
            [RouteOperator::fallback(route.locations.iter().cloned())],
            RouteLimits::new(32, 64),
        )),
        _ => Err(BackendError::InvalidArtifact("runtime-route")),
    }
}

fn software_view(
    state: &SoftwareState,
    local_software: Option<&DesktopSoftwareArtifact>,
) -> SoftwareView {
    let display_name = local_software
        .map(|software| software.display_name.clone())
        .unwrap_or_else(|| state.artifact.name.clone());
    let monogram = display_name
        .chars()
        .take(2)
        .collect::<String>()
        .to_uppercase()
        .into_boxed_str();
    SoftwareView {
        id: state.artifact.id.clone(),
        name: display_name,
        description: local_software
            .map(|software| software.description.clone())
            .unwrap_or_default(),
        vendor: state.artifact.vendor.clone(),
        version: "—".into(),
        executable_name: state
            .artifact
            .executables
            .first()
            .cloned()
            .unwrap_or_default(),
        executable_path: local_software.map(|software| software.executable_path.clone()),
        monogram,
        last_used: None,
        locale: state.locale.clone(),
        connected: false,
        translation: CapabilityView::unavailable("capability.text-unavailable"),
        font: CapabilityView::unavailable("capability.font-unavailable"),
        observe: CapabilityView::pending_observation(),
        locations: state
            .artifact
            .locations
            .iter()
            .map(|location| LocationView {
                id: location.id.clone(),
                label: location.label.clone(),
            })
            .collect(),
    }
}

fn dictionary_artifact(create: DictionaryCreate) -> Result<DictionaryArtifact, BackendError> {
    dictionary_artifact_at_revision(create, 1)
}

fn dictionary_artifact_at_revision(
    create: DictionaryCreate,
    revision: u64,
) -> Result<DictionaryArtifact, BackendError> {
    let artifact = DictionaryArtifact {
        schema: DICTIONARY_SCHEMA.into(),
        revision,
        metadata: create.metadata,
        entries: create
            .entries
            .into_iter()
            .map(|entry| DictionaryRuleArtifact {
                location: entry.location,
                context: entry.context,
                source: entry.source,
                text: entry
                    .translation
                    .map_or(DictionaryTextArtifact::Keep, |text| {
                        DictionaryTextArtifact::Replace { text }
                    }),
            })
            .collect(),
    };
    validate_dictionary(&artifact, None)?;
    Ok(artifact)
}

fn dictionary_view(artifact: &DictionaryArtifact) -> DictionaryView {
    DictionaryView {
        metadata: artifact.metadata.clone(),
        revision: artifact.revision,
        entries: artifact
            .entries
            .iter()
            .map(|entry| DictionaryRuleView {
                location: entry.location.clone(),
                context: entry.context.clone(),
                source: entry.source.clone(),
                translation: match &entry.text {
                    DictionaryTextArtifact::Keep => None,
                    DictionaryTextArtifact::Replace { text } => Some(text.clone()),
                },
            })
            .collect(),
    }
}

fn dictionary_rule_key(rule: &DictionaryRuleCreate) -> DictionaryRuleKey {
    DictionaryRuleKey {
        location: rule.location.clone(),
        context: rule.context.clone(),
        source: rule.source.clone(),
    }
}

fn dictionary_rule_view_key(rule: &DictionaryRuleView) -> DictionaryRuleKey {
    DictionaryRuleKey {
        location: rule.location.clone(),
        context: rule.context.clone(),
        source: rule.source.clone(),
    }
}

fn dictionary_rule_create(rule: &DictionaryRuleView) -> DictionaryRuleCreate {
    DictionaryRuleCreate {
        location: rule.location.clone(),
        context: rule.context.clone(),
        source: rule.source.clone(),
        translation: rule.translation.clone(),
    }
}

fn dictionary_definition(dictionary: &DictionaryView) -> WorkflowDictionary {
    let entries = dictionary.entries.iter().map(|entry| {
        let rule = entry.translation.as_ref().map_or_else(
            || WorkflowDictionaryEntry::keep(entry.location.clone(), entry.source.clone()),
            |translation| {
                WorkflowDictionaryEntry::replace(
                    entry.location.clone(),
                    entry.source.clone(),
                    translation.clone(),
                )
            },
        );
        if let Some(context) = &entry.context {
            rule.with_context(context.kind.clone(), context.key.clone())
        } else {
            rule
        }
    });
    WorkflowDictionary::new(
        dictionary.id(),
        dictionary.metadata.target_locale.clone(),
        entries,
    )
}

fn font_profile_artifact(
    create: FontProfileCreate,
    revision: u64,
) -> Result<FontProfileArtifact, BackendError> {
    let artifact = FontProfileArtifact {
        schema: FONT_PROFILE_SCHEMA.into(),
        revision,
        metadata: create.metadata,
        families: create.families,
    };
    validate_font_profile(&artifact, None)?;
    Ok(artifact)
}

fn font_profile_view(
    artifact: &FontProfileArtifact,
    installed_families: &BTreeSet<Box<str>>,
) -> FontProfileView {
    FontProfileView {
        metadata: artifact.metadata.clone(),
        revision: artifact.revision,
        families: artifact.families.clone(),
        resolved_family: artifact
            .families
            .iter()
            .find(|family| installed_families.contains(*family))
            .cloned(),
    }
}

fn font_profile_definition(profile: &FontProfileView) -> WorkflowFontProfile {
    WorkflowFontProfile::new(profile.id(), profile.families.iter().cloned())
}

fn workflow_view(artifact: &WorkflowArtifact) -> WorkflowView {
    WorkflowView {
        id: artifact.id.clone(),
        name: artifact.name.clone(),
        description: artifact.description.clone(),
        revision: artifact.revision,
        targets: artifact
            .targets
            .iter()
            .map(|target| WorkflowTargetView {
                software_id: target.software_id.clone(),
                adapter_plan: target.adapter_plan.clone(),
                dictionary_ids: target.dictionary_ids.clone(),
                font_bindings: target.font_bindings.clone(),
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
                || target.font_bindings.iter().any(|binding| {
                    !safe_identifier(binding.font_profile_id())
                        || match binding.scope() {
                            FontProfileScope::All => false,
                            FontProfileScope::Locations { location_ids } => {
                                location_ids.is_empty()
                                    || location_ids.iter().collect::<BTreeSet<_>>().len()
                                        != location_ids.len()
                                    || location_ids
                                        .iter()
                                        .any(|location_id| !safe_identifier(location_id))
                            }
                        }
                })
        })
    {
        return Err(BackendError::InvalidArtifact("workflow-contract"));
    }
    Ok(())
}

fn read_workflows(root: &Path) -> Result<BTreeMap<Box<str>, WorkflowArtifact>, BackendError> {
    let directory = root.join("workflows");
    let mut paths = fs::read_dir(&directory)
        .map_err(|_| BackendError::Storage("read-workflow-directory"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut workflows = BTreeMap::new();
    for path in paths {
        let artifact: WorkflowArtifact = read_json(&path, "workflow-json")?;
        validate_workflow(&artifact, Some(&path))?;
        let workflow_id = artifact.id.clone();
        if workflows.insert(workflow_id.clone(), artifact).is_some() {
            return Err(BackendError::DuplicateWorkflow(workflow_id));
        }
    }
    Ok(workflows)
}

fn read_workflow_state(
    root: &Path,
    workflows: &BTreeMap<Box<str>, WorkflowArtifact>,
) -> Result<BTreeMap<Box<str>, u64>, BackendError> {
    let path = root.join("workflow-state.json");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let state: WorkflowStateArtifact = read_json(&path, "workflow-state-json")?;
    if state.schema.as_ref() != WORKFLOW_STATE_SCHEMA
        || state.enabled.iter().any(|(workflow_id, revision)| {
            workflows
                .get(workflow_id)
                .is_none_or(|workflow| workflow.revision != *revision)
        })
    {
        return Err(BackendError::InvalidArtifact("workflow-state-contract"));
    }
    Ok(state.enabled)
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

fn validate_dictionary(
    artifact: &DictionaryArtifact,
    path: Option<&Path>,
) -> Result<(), BackendError> {
    let valid_entries = artifact.entries.iter().all(|entry| {
        let valid_text = match &entry.text {
            DictionaryTextArtifact::Keep => true,
            DictionaryTextArtifact::Replace { .. } => true,
        };
        safe_identifier(&entry.location)
            && entry.context.as_ref().is_none_or(|context| {
                safe_identifier(&context.kind) && !context.key.trim().is_empty()
            })
            && !entry.source.trim().is_empty()
            && valid_text
    });
    let unique_entries = artifact
        .entries
        .iter()
        .map(|entry| (&entry.location, &entry.context, &entry.source))
        .collect::<BTreeSet<_>>()
        .len()
        == artifact.entries.len();
    if artifact.schema.as_ref() != DICTIONARY_SCHEMA
        || !safe_identifier(artifact.metadata.id())
        || !safe_identifier(artifact.metadata.source_locale())
        || !safe_identifier(artifact.metadata.target_locale())
        || artifact.metadata.release_version().trim().is_empty()
        || artifact.metadata.name().trim().is_empty()
        || artifact.metadata.name().chars().count() > 128
        || artifact.metadata.description().chars().count() > 512
        || artifact
            .metadata
            .authors()
            .iter()
            .any(|author| author.trim().is_empty())
        || artifact
            .metadata
            .authors()
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != artifact.metadata.authors().len()
        || artifact
            .metadata
            .license()
            .is_some_and(|license| license.trim().is_empty())
        || artifact
            .metadata
            .homepage()
            .is_some_and(|homepage| homepage.trim().is_empty())
        || artifact
            .metadata
            .tags()
            .iter()
            .any(|tag| tag.trim().is_empty())
        || artifact
            .metadata
            .tags()
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != artifact.metadata.tags().len()
        || artifact.revision == 0
        || path.is_some_and(|path| {
            path.file_stem().and_then(|value| value.to_str()) != Some(artifact.metadata.id())
        })
        || !valid_entries
        || !unique_entries
    {
        return Err(BackendError::InvalidArtifact("dictionary-contract"));
    }
    Ok(())
}

fn read_dictionaries(root: &Path) -> Result<BTreeMap<Box<str>, DictionaryView>, BackendError> {
    let directory = root.join("dictionaries");
    let mut paths = fs::read_dir(&directory)
        .map_err(|_| BackendError::Storage("read-dictionary-directory"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut dictionaries = BTreeMap::new();
    for path in paths {
        let artifact: DictionaryArtifact = read_json(&path, "dictionary-json")?;
        validate_dictionary(&artifact, Some(&path))?;
        let dictionary_id = artifact.metadata.id.clone();
        if dictionaries
            .insert(dictionary_id.clone(), dictionary_view(&artifact))
            .is_some()
        {
            return Err(BackendError::DuplicateDictionary(dictionary_id));
        }
    }
    Ok(dictionaries)
}

fn validate_font_profile(
    artifact: &FontProfileArtifact,
    path: Option<&Path>,
) -> Result<(), BackendError> {
    let unique_families =
        artifact.families.iter().collect::<BTreeSet<_>>().len() == artifact.families.len();
    if artifact.schema.as_ref() != FONT_PROFILE_SCHEMA
        || !safe_identifier(artifact.metadata.id())
        || artifact.metadata.name().trim().is_empty()
        || artifact.metadata.name().chars().count() > 128
        || artifact.metadata.description().chars().count() > 512
        || artifact.revision == 0
        || artifact.families.is_empty()
        || artifact
            .families
            .iter()
            .any(|family| family.trim().is_empty())
        || !unique_families
        || path.is_some_and(|path| {
            path.file_stem().and_then(|value| value.to_str()) != Some(artifact.metadata.id())
        })
    {
        return Err(BackendError::InvalidArtifact("font-profile-contract"));
    }
    Ok(())
}

fn read_font_profiles(
    root: &Path,
    installed_families: &BTreeSet<Box<str>>,
) -> Result<BTreeMap<Box<str>, FontProfileView>, BackendError> {
    let directory = root.join("font-profiles");
    let mut paths = fs::read_dir(&directory)
        .map_err(|_| BackendError::Storage("read-font-profile-directory"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut profiles = BTreeMap::new();
    for path in paths {
        let artifact: FontProfileArtifact = read_json(&path, "font-profile-json")?;
        validate_font_profile(&artifact, Some(&path))?;
        let profile_id = artifact.metadata.id.clone();
        if profiles
            .insert(
                profile_id.clone(),
                font_profile_view(&artifact, installed_families),
            )
            .is_some()
        {
            return Err(BackendError::DuplicateFontProfile(profile_id));
        }
    }
    Ok(profiles)
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

fn next_local_extension_id(software: &BTreeMap<Box<str>, SoftwareState>, name: &str) -> Box<str> {
    let mut slug = String::new();
    let mut pending_separator = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character.to_ascii_lowercase());
            pending_separator = false;
        } else {
            pending_separator = true;
        }
    }
    if slug.is_empty() {
        slug.push_str("software");
    }
    let base = format!("local.{slug}");
    if !software.contains_key(base.as_str()) {
        return base.into();
    }
    for suffix in 2_u32.. {
        let candidate = format!("{base}.{suffix}");
        if !software.contains_key(candidate.as_str()) {
            return candidate.into();
        }
    }
    unreachable!("an unbounded numeric suffix always has another candidate")
}

fn read_desktop_state(root: &Path) -> Result<DesktopStateArtifact, BackendError> {
    let path = root.join("desktop-state.json");
    if !path.exists() {
        return Ok(DesktopStateArtifact {
            schema: DESKTOP_STATE_SCHEMA.into(),
            selected_software_id: None,
            software: BTreeMap::new(),
        });
    }
    let state: DesktopStateArtifact = read_json(&path, "desktop-state-json")?;
    if state.schema.as_ref() != DESKTOP_STATE_SCHEMA
        || state
            .selected_software_id
            .as_deref()
            .is_some_and(|value| !safe_identifier(value))
        || state.software.iter().any(|(extension_id, software)| {
            !safe_identifier(extension_id)
                || software.display_name.trim().is_empty()
                || software.display_name.chars().count() > 128
                || !Path::new(software.executable_path.as_ref()).is_absolute()
        })
    {
        return Err(BackendError::InvalidArtifact("desktop-state-contract"));
    }
    Ok(state)
}

fn write_desktop_state(
    root: &Path,
    selected_software_id: Option<&str>,
    software: &BTreeMap<Box<str>, DesktopSoftwareArtifact>,
) -> Result<(), BackendError> {
    let state = DesktopStateArtifact {
        schema: DESKTOP_STATE_SCHEMA.into(),
        selected_software_id: selected_software_id.map(Into::into),
        software: software.clone(),
    };
    let serialized = serde_json::to_string(&state)
        .map_err(|_| BackendError::InvalidArtifact("serialize-desktop-state"))?;
    write_atomic(&root.join("desktop-state.json"), &serialized)
}

fn dictionary_path(root: &Path, dictionary_id: &str) -> Result<PathBuf, BackendError> {
    if !safe_identifier(dictionary_id) {
        return Err(BackendError::InvalidArtifact("unsafe-artifact-id"));
    }
    Ok(root
        .join("dictionaries")
        .join(format!("{dictionary_id}.json")))
}

fn font_profile_path(root: &Path, font_profile_id: &str) -> Result<PathBuf, BackendError> {
    if !safe_identifier(font_profile_id) {
        return Err(BackendError::InvalidArtifact("unsafe-artifact-id"));
    }
    Ok(root
        .join("font-profiles")
        .join(format!("{font_profile_id}.json")))
}

fn workflow_path(root: &Path, workflow_id: &str) -> Result<PathBuf, BackendError> {
    if !safe_identifier(workflow_id) {
        return Err(BackendError::InvalidArtifact("unsafe-artifact-id"));
    }
    Ok(root.join("workflows").join(format!("{workflow_id}.json")))
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    operation: &'static str,
) -> Result<T, BackendError> {
    let source = fs::read_to_string(path).map_err(|_| BackendError::Storage(operation))?;
    serde_json::from_str(&source).map_err(|_| BackendError::InvalidArtifact(operation))
}

fn write_atomic(path: &Path, content: &str) -> Result<(), BackendError> {
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
