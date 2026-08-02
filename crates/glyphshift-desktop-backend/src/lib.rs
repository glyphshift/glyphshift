//! Desktop-facing product model for software, dictionaries, and workflows.

use glyphshift_adapter_registry::{AdapterRequirement, AdapterVersion, AdapterVersionRequirement};
use glyphshift_domain::{AdapterId, Feature, Generation, RouteLimits, RouteOperator, RouteProgram};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use glyphshift_workflow::{
    resolve as resolve_workflow, CompiledWorkflow, DefaultFontBehavior,
    Dictionary as WorkflowDictionary, DictionaryEntry as WorkflowDictionaryEntry,
    EntryFontBehavior, ResolveError, SoftwareInput, Workflow as WorkflowDefinition,
    WorkflowTarget as WorkflowDefinitionTarget,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const EXTENSION_SCHEMA: &str = "glyphshift.extension/1";
const DICTIONARY_SCHEMA: &str = "glyphshift.dictionary/1";
const WORKFLOW_SCHEMA: &str = "glyphshift.workflow/1";
const WORKFLOW_STATE_SCHEMA: &str = "glyphshift.workflow-state/1";
const DESKTOP_STATE_SCHEMA: &str = "glyphshift.desktop-state/1";
const DEFAULT_LOCALE: &str = "zh-CN";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendError {
    Storage(&'static str),
    InvalidArtifact(&'static str),
    DuplicateSoftware(Box<str>),
    DuplicateDictionary(Box<str>),
    UnknownSoftware(Box<str>),
    UnknownDictionary(Box<str>),
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
            detail: "等待兼容性检测",
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
    id: Box<str>,
    name: Box<str>,
    description: Box<str>,
    locale: Box<str>,
    hook_type_id: Option<Box<str>>,
    revision: u64,
    entry_count: usize,
}

impl DictionarySummaryView {
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
    pub fn locale(&self) -> &str {
        &self.locale
    }

    #[must_use]
    pub fn hook_type_id(&self) -> Option<&str> {
        self.hook_type_id.as_deref()
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

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "family", rename_all = "snake_case")]
pub enum DictionaryDefaultFont {
    #[default]
    Unchanged,
    Substitute(Box<str>),
}

impl DictionaryDefaultFont {
    #[must_use]
    pub fn substitute(family: impl Into<Box<str>>) -> Self {
        Self::Substitute(family.into())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "family", rename_all = "snake_case")]
pub enum DictionaryEntryFont {
    #[default]
    Inherit,
    Unchanged,
    Substitute(Box<str>),
}

impl DictionaryEntryFont {
    #[must_use]
    pub fn substitute(family: impl Into<Box<str>>) -> Self {
        Self::Substitute(family.into())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryRuleCreate {
    location: Box<str>,
    context: Option<DictionaryRuleContext>,
    source: Box<str>,
    translation: Option<Box<str>>,
    font: DictionaryEntryFont,
    #[serde(default)]
    adapter_ids: BTreeSet<Box<str>>,
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
            font: DictionaryEntryFont::Inherit,
            adapter_ids: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn keep(location: impl Into<Box<str>>, source: impl Into<Box<str>>) -> Self {
        Self {
            location: location.into(),
            context: None,
            source: source.into(),
            translation: None,
            font: DictionaryEntryFont::Inherit,
            adapter_ids: BTreeSet::new(),
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

    #[must_use]
    pub fn with_font(mut self, font: DictionaryEntryFont) -> Self {
        self.font = font;
        self
    }

    #[must_use]
    pub fn for_adapters(
        mut self,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.adapter_ids = adapter_ids.into_iter().map(Into::into).collect();
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
    adapter_ids: BTreeSet<Box<str>>,
}

impl DictionaryRuleKey {
    #[must_use]
    pub fn new(location: impl Into<Box<str>>, source: impl Into<Box<str>>) -> Self {
        Self {
            location: location.into(),
            context: None,
            source: source.into(),
            adapter_ids: BTreeSet::new(),
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

    #[must_use]
    pub fn for_adapters(
        mut self,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.adapter_ids = adapter_ids.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryCreate {
    id: Box<str>,
    name: Box<str>,
    #[serde(default)]
    description: Box<str>,
    locale: Box<str>,
    #[serde(default)]
    hook_type_id: Option<Box<str>>,
    default_font: DictionaryDefaultFont,
    entries: Vec<DictionaryRuleCreate>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryEdit {
    id: Box<str>,
    name: Box<str>,
    #[serde(default)]
    description: Box<str>,
    locale: Box<str>,
    #[serde(default)]
    hook_type_id: Option<Box<str>>,
    base_revision: u64,
    default_font: DictionaryDefaultFont,
    entries: Vec<DictionaryRuleCreate>,
}

impl DictionaryEdit {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        locale: impl Into<Box<str>>,
        base_revision: u64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: "".into(),
            locale: locale.into(),
            hook_type_id: None,
            base_revision,
            default_font: DictionaryDefaultFont::Unchanged,
            entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.description = description.into();
        self
    }

    #[must_use]
    pub fn for_adapter(mut self, adapter_id: impl Into<Box<str>>) -> Self {
        self.hook_type_id = Some(adapter_id.into());
        self
    }

    #[must_use]
    pub fn with_default_font(mut self, default_font: DictionaryDefaultFont) -> Self {
        self.default_font = default_font;
        self
    }

    #[must_use]
    pub fn with_entries(mut self, entries: impl IntoIterator<Item = DictionaryRuleCreate>) -> Self {
        self.entries = entries.into_iter().collect();
        self
    }
}

impl DictionaryCreate {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        locale: impl Into<Box<str>>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: "".into(),
            locale: locale.into(),
            hook_type_id: None,
            default_font: DictionaryDefaultFont::Unchanged,
            entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.description = description.into();
        self
    }

    #[must_use]
    pub fn for_adapter(mut self, adapter_id: impl Into<Box<str>>) -> Self {
        self.hook_type_id = Some(adapter_id.into());
        self
    }

    #[must_use]
    pub fn with_default_font(mut self, default_font: DictionaryDefaultFont) -> Self {
        self.default_font = default_font;
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
    font: DictionaryEntryFont,
    adapter_ids: Vec<Box<str>>,
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

    #[must_use]
    pub const fn font(&self) -> &DictionaryEntryFont {
        &self.font
    }

    #[must_use]
    pub fn adapter_ids(&self) -> &[Box<str>] {
        &self.adapter_ids
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryView {
    id: Box<str>,
    name: Box<str>,
    description: Box<str>,
    locale: Box<str>,
    hook_type_id: Option<Box<str>>,
    revision: u64,
    default_font: DictionaryDefaultFont,
    entries: Vec<DictionaryRuleView>,
}

impl DictionaryView {
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
    pub fn locale(&self) -> &str {
        &self.locale
    }

    #[must_use]
    pub fn hook_type_id(&self) -> Option<&str> {
        self.hook_type_id.as_deref()
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn default_font(&self) -> &DictionaryDefaultFont {
        &self.default_font
    }

    #[must_use]
    pub fn entries(&self) -> &[DictionaryRuleView] {
        &self.entries
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowTargetCreate {
    software_id: Box<str>,
    dictionary_ids: Vec<Box<str>>,
}

impl WorkflowTargetCreate {
    #[must_use]
    pub fn new(
        software_id: impl Into<Box<str>>,
        dictionary_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            software_id: software_id.into(),
            dictionary_ids: dictionary_ids.into_iter().map(Into::into).collect(),
        }
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
    dictionary_ids: Vec<Box<str>>,
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
    id: Box<str>,
    name: Box<str>,
    #[serde(default)]
    description: Box<str>,
    locale: Box<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hook_type_id: Option<Box<str>>,
    revision: u64,
    default_font: DictionaryDefaultFontArtifact,
    entries: Vec<DictionaryRuleArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DictionaryDefaultFontArtifact {
    Unchanged,
    Substitute { family: Box<str> },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DictionaryRuleArtifact {
    location: Box<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    context: Option<DictionaryRuleContext>,
    source: Box<str>,
    text: DictionaryTextArtifact,
    font: DictionaryEntryFontArtifact,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    adapter_ids: BTreeSet<Box<str>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DictionaryTextArtifact {
    Keep,
    Replace { text: Box<str> },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DictionaryEntryFontArtifact {
    Inherit,
    Unchanged,
    Substitute { family: Box<str> },
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
    dictionary_ids: Vec<Box<str>>,
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

pub struct DesktopBackend {
    root: PathBuf,
    selected_software_id: Option<Box<str>>,
    software: BTreeMap<Box<str>, SoftwareState>,
    local_software: BTreeMap<Box<str>, DesktopSoftwareArtifact>,
    dictionaries: BTreeMap<Box<str>, DictionaryView>,
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
        let root = root.as_ref().to_path_buf();
        let extension_root = root.join("extensions");
        fs::create_dir_all(&extension_root)
            .map_err(|_| BackendError::Storage("create-extension-directory"))?;
        fs::create_dir_all(root.join("dictionaries"))
            .map_err(|_| BackendError::Storage("create-dictionary-directory"))?;
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
        let workflows = read_workflows(&root)?;
        let enabled_workflows = read_workflow_state(&root, &workflows)?;
        let enabled_workflow_ids = enabled_workflows.keys().cloned().collect();
        let backend = Self {
            root,
            selected_software_id,
            software,
            local_software,
            dictionaries,
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
        if self.dictionaries.contains_key(&create.id) {
            return Err(BackendError::DuplicateDictionary(create.id));
        }
        let artifact = dictionary_artifact(create)?;
        let serialized = serde_json::to_string(&artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-dictionary"))?;
        let path = dictionary_path(&self.root, &artifact.id)?;
        write_atomic(&path, &serialized)?;
        let view = dictionary_view(&artifact);
        self.dictionaries.insert(artifact.id, view.clone());
        Ok(view)
    }

    pub fn update_dictionary(
        &mut self,
        edit: DictionaryEdit,
    ) -> Result<DictionaryView, BackendError> {
        let current = self
            .dictionaries
            .get(&edit.id)
            .ok_or_else(|| BackendError::UnknownDictionary(edit.id.clone()))?;
        if current.revision != edit.base_revision {
            return Err(BackendError::RevisionConflict {
                current: current.revision,
            });
        }
        let artifact = dictionary_artifact_at_revision(
            DictionaryCreate {
                id: edit.id,
                name: edit.name,
                description: edit.description,
                locale: edit.locale,
                hook_type_id: edit.hook_type_id,
                default_font: edit.default_font,
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
                    .any(|dictionary_id| dictionary_id == &artifact.id)
            }) {
                self.resolve_workflow_artifact_with_dictionary(workflow, Some(&view))?;
            }
        }
        let serialized = serde_json::to_string(&artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-dictionary"))?;
        let path = dictionary_path(&self.root, &artifact.id)?;
        write_atomic(&path, &serialized)?;
        self.dictionaries.insert(artifact.id, view.clone());
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
        let edit = DictionaryEdit::new(
            current.id.clone(),
            current.name.clone(),
            current.locale.clone(),
            current.revision,
        )
        .with_description(current.description.clone())
        .with_default_font(current.default_font.clone())
        .with_entries(entries);
        self.update_dictionary(if let Some(hook_type_id) = current.hook_type_id {
            edit.for_adapter(hook_type_id)
        } else {
            edit
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
        let edit = DictionaryEdit::new(
            current.id.clone(),
            current.name.clone(),
            current.locale.clone(),
            current.revision,
        )
        .with_description(current.description.clone())
        .with_default_font(current.default_font.clone())
        .with_entries(entries);
        self.update_dictionary(if let Some(hook_type_id) = current.hook_type_id {
            edit.for_adapter(hook_type_id)
        } else {
            edit
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
                    dictionary_ids: target.dictionary_ids,
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
                    dictionary_ids: target.dictionary_ids,
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
                    target.dictionary_ids.iter().cloned(),
                )
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
        self.resolve_workflow_artifact_with_dictionary(artifact, None)
    }

    fn resolve_workflow_artifact_with_dictionary(
        &self,
        artifact: &WorkflowArtifact,
        dictionary_override: Option<&DictionaryView>,
    ) -> Result<CompiledWorkflow, BackendError> {
        let definition = WorkflowDefinition::new(
            artifact.id.clone(),
            artifact.targets.iter().map(|target| {
                WorkflowDefinitionTarget::new(
                    target.software_id.clone(),
                    target.dictionary_ids.iter().cloned(),
                )
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
        resolve_workflow(&definition, &software, &dictionaries)
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
                let requirements = state.artifact.runtime.as_ref().map_or_else(
                    || Ok(Vec::new()),
                    |runtime| {
                        runtime
                            .capabilities
                            .iter()
                            .map(runtime_requirement)
                            .collect::<Result<Vec<_>, _>>()
                    },
                )?;
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
        let requirements = state.artifact.runtime.as_ref().map_or_else(
            || Ok(Vec::new()),
            |runtime| {
                runtime
                    .capabilities
                    .iter()
                    .map(runtime_requirement)
                    .collect::<Result<Vec<_>, _>>()
            },
        )?;
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
                    id: dictionary.id.clone(),
                    name: dictionary.name.clone(),
                    description: dictionary.description.clone(),
                    locale: dictionary.locale.clone(),
                    hook_type_id: dictionary.hook_type_id.clone(),
                    revision: dictionary.revision,
                    entry_count: dictionary.entries.len(),
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
                            dictionary_ids: target.dictionary_ids.clone(),
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
            vendor: "未知开发者".into(),
            executables: vec![executable_name.into()],
            runtime: None,
            locations: vec![ExtensionLocationArtifact {
                id: "main-ui".into(),
                label: "界面文字".into(),
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
        version: "待检测".into(),
        executable_name: state
            .artifact
            .executables
            .first()
            .cloned()
            .unwrap_or_else(|| "未指定".into()),
        executable_path: local_software.map(|software| software.executable_path.clone()),
        monogram,
        last_used: None,
        locale: state.locale.clone(),
        connected: false,
        translation: CapabilityView::unavailable("尚未获得文字替换证据"),
        font: CapabilityView::unavailable("尚未获得字体替换证据"),
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
        id: create.id,
        name: create.name,
        description: create.description,
        locale: create.locale,
        hook_type_id: create.hook_type_id,
        revision,
        default_font: match create.default_font {
            DictionaryDefaultFont::Unchanged => DictionaryDefaultFontArtifact::Unchanged,
            DictionaryDefaultFont::Substitute(family) => {
                DictionaryDefaultFontArtifact::Substitute { family }
            }
        },
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
                font: match entry.font {
                    DictionaryEntryFont::Inherit => DictionaryEntryFontArtifact::Inherit,
                    DictionaryEntryFont::Unchanged => DictionaryEntryFontArtifact::Unchanged,
                    DictionaryEntryFont::Substitute(family) => {
                        DictionaryEntryFontArtifact::Substitute { family }
                    }
                },
                adapter_ids: entry.adapter_ids,
            })
            .collect(),
    };
    validate_dictionary(&artifact, None)?;
    Ok(artifact)
}

fn dictionary_view(artifact: &DictionaryArtifact) -> DictionaryView {
    DictionaryView {
        id: artifact.id.clone(),
        name: artifact.name.clone(),
        description: artifact.description.clone(),
        locale: artifact.locale.clone(),
        hook_type_id: artifact
            .hook_type_id
            .clone()
            .or_else(|| legacy_dictionary_hook_type(&artifact.entries)),
        revision: artifact.revision,
        default_font: match &artifact.default_font {
            DictionaryDefaultFontArtifact::Unchanged => DictionaryDefaultFont::Unchanged,
            DictionaryDefaultFontArtifact::Substitute { family } => {
                DictionaryDefaultFont::Substitute(family.clone())
            }
        },
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
                font: match &entry.font {
                    DictionaryEntryFontArtifact::Inherit => DictionaryEntryFont::Inherit,
                    DictionaryEntryFontArtifact::Unchanged => DictionaryEntryFont::Unchanged,
                    DictionaryEntryFontArtifact::Substitute { family } => {
                        DictionaryEntryFont::Substitute(family.clone())
                    }
                },
                adapter_ids: entry.adapter_ids.iter().cloned().collect(),
            })
            .collect(),
    }
}

fn dictionary_rule_key(rule: &DictionaryRuleCreate) -> DictionaryRuleKey {
    DictionaryRuleKey {
        location: rule.location.clone(),
        context: rule.context.clone(),
        source: rule.source.clone(),
        adapter_ids: rule.adapter_ids.clone(),
    }
}

fn dictionary_rule_view_key(rule: &DictionaryRuleView) -> DictionaryRuleKey {
    DictionaryRuleKey {
        location: rule.location.clone(),
        context: rule.context.clone(),
        source: rule.source.clone(),
        adapter_ids: rule.adapter_ids.iter().cloned().collect(),
    }
}

fn dictionary_rule_create(rule: &DictionaryRuleView) -> DictionaryRuleCreate {
    DictionaryRuleCreate {
        location: rule.location.clone(),
        context: rule.context.clone(),
        source: rule.source.clone(),
        translation: rule.translation.clone(),
        font: rule.font.clone(),
        adapter_ids: rule.adapter_ids.iter().cloned().collect(),
    }
}

fn dictionary_definition(dictionary: &DictionaryView) -> WorkflowDictionary {
    let default_font = dictionary.default_font.clone();
    let hook_type_id = dictionary.hook_type_id.clone();
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
        let rule = match &entry.font {
            DictionaryEntryFont::Inherit => rule,
            DictionaryEntryFont::Unchanged => rule.with_font(EntryFontBehavior::Unchanged),
            DictionaryEntryFont::Substitute(family) => {
                rule.with_font(EntryFontBehavior::substitute(family.clone()))
            }
        };
        let rule = if let Some(context) = &entry.context {
            rule.with_context(context.kind.clone(), context.key.clone())
        } else {
            rule
        };
        rule.for_adapters(entry.adapter_ids.iter().cloned())
    });
    let dictionary =
        WorkflowDictionary::new(dictionary.id.clone(), dictionary.locale.clone(), entries);
    let dictionary = if let Some(adapter_id) = hook_type_id {
        dictionary.for_adapters([adapter_id])
    } else {
        dictionary
    };
    match default_font {
        DictionaryDefaultFont::Unchanged => dictionary,
        DictionaryDefaultFont::Substitute(family) => {
            dictionary.with_default_font(DefaultFontBehavior::substitute(family))
        }
    }
}

fn legacy_dictionary_hook_type(entries: &[DictionaryRuleArtifact]) -> Option<Box<str>> {
    let mut hook_type_id: Option<&Box<str>> = None;
    for entry in entries {
        let mut adapter_ids = entry.adapter_ids.iter();
        let entry_hook_type_id = adapter_ids.next()?;
        if adapter_ids.next().is_some() {
            return None;
        }
        match hook_type_id {
            None => hook_type_id = Some(entry_hook_type_id),
            Some(current) if current == entry_hook_type_id => {}
            Some(_) => return None,
        }
    }
    hook_type_id.cloned()
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
                dictionary_ids: target.dictionary_ids.clone(),
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
                || target.dictionary_ids.is_empty()
                || target.dictionary_ids.iter().collect::<BTreeSet<_>>().len()
                    != target.dictionary_ids.len()
                || target
                    .dictionary_ids
                    .iter()
                    .any(|dictionary_id| !safe_identifier(dictionary_id))
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
    let valid_default_font = match &artifact.default_font {
        DictionaryDefaultFontArtifact::Unchanged => true,
        DictionaryDefaultFontArtifact::Substitute { family } => !family.trim().is_empty(),
    };
    let valid_entries = artifact.entries.iter().all(|entry| {
        let valid_text = match &entry.text {
            DictionaryTextArtifact::Keep => true,
            DictionaryTextArtifact::Replace { .. } => true,
        };
        let valid_font = match &entry.font {
            DictionaryEntryFontArtifact::Inherit => true,
            DictionaryEntryFontArtifact::Unchanged => true,
            DictionaryEntryFontArtifact::Substitute { family } => !family.trim().is_empty(),
        };
        safe_identifier(&entry.location)
            && entry.context.as_ref().is_none_or(|context| {
                safe_identifier(&context.kind) && !context.key.trim().is_empty()
            })
            && !entry.source.trim().is_empty()
            && valid_text
            && valid_font
            && entry
                .adapter_ids
                .iter()
                .all(|adapter_id| safe_identifier(adapter_id))
    });
    let unique_entries = artifact
        .entries
        .iter()
        .map(|entry| {
            (
                &entry.location,
                &entry.context,
                &entry.source,
                &entry.adapter_ids,
            )
        })
        .collect::<BTreeSet<_>>()
        .len()
        == artifact.entries.len();
    if artifact.schema.as_ref() != DICTIONARY_SCHEMA
        || !safe_identifier(&artifact.id)
        || !safe_identifier(&artifact.locale)
        || artifact
            .hook_type_id
            .as_ref()
            .is_some_and(|hook_type_id| !safe_identifier(hook_type_id))
        || artifact.name.trim().is_empty()
        || artifact.name.chars().count() > 128
        || artifact.description.chars().count() > 512
        || artifact.revision == 0
        || path.is_some_and(|path| {
            path.file_stem().and_then(|value| value.to_str()) != Some(&artifact.id)
        })
        || !valid_default_font
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
        let dictionary_id = artifact.id.clone();
        if dictionaries
            .insert(dictionary_id.clone(), dictionary_view(&artifact))
            .is_some()
        {
            return Err(BackendError::DuplicateDictionary(dictionary_id));
        }
    }
    Ok(dictionaries)
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
