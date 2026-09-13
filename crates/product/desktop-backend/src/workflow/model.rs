use super::super::*;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowActivationSnapshot {
    pub(crate) workflow_id: Box<str>,
    pub(crate) revision: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSummaryView {
    pub(crate) id: Box<str>,
    pub(crate) name: Box<str>,
    pub(crate) description: Box<str>,
    pub(crate) global_shortcut: Box<str>,
    pub(crate) revision: u64,
    pub(crate) software_ids: Vec<Box<str>>,
    pub(crate) dictionary_ids: Vec<Box<str>>,
    pub(crate) targets: Vec<WorkflowTargetView>,
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
    pub(super) strategy: WorkflowAdapterStrategy,
    pub(super) adapter_ids: Vec<Box<str>>,
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
    #[serde(default)]
    pub(super) dictionary_overrides: BTreeMap<Box<str>, WorkflowDictionaryFont>,
    #[serde(default = "default_font_scale")]
    pub(super) scale_percent: u16,
    pub(super) families: Vec<Box<str>>,
    pub(super) coverage: FontCoverage,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDictionaryFont {
    #[serde(default)]
    pub(super) families: Vec<Box<str>>,
    #[serde(default)]
    pub(super) scale_percent: Option<u16>,
}

fn default_font_scale() -> u16 { 100 }

impl WorkflowFontPolicy {
    #[must_use]
    pub fn with_scale_percent(mut self, percent: u16) -> Self { self.scale_percent = percent; self }

    #[must_use]
    pub fn new(
        families: impl IntoIterator<Item = impl Into<Box<str>>>,
        coverage: FontCoverage,
    ) -> Self {
        Self {
            dictionary_overrides: BTreeMap::new(),
            scale_percent: 100,
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
            write_dictionary_id: None,
            collect_new_sources: None,
            font_policy: None,
        }
    }

    #[must_use]
    pub fn with_write_dictionary(mut self, dictionary_id: impl Into<Box<str>>) -> Self {
        self.write_dictionary_id = Some(dictionary_id.into());
        self
    }

    #[must_use]
    pub fn with_optional_write_dictionary(mut self, dictionary_id: Option<Box<str>>) -> Self {
        self.write_dictionary_id = dictionary_id;
        self
    }

    pub fn with_collection_enabled(mut self, enabled: Option<bool>) -> Self {
        self.collect_new_sources = enabled;
        self
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
    pub(super) id: Box<str>,
    pub(super) name: Box<str>,
    #[serde(default)]
    pub(super) description: Box<str>,
    #[serde(default)]
    pub(super) global_shortcut: Box<str>,
    pub(super) targets: Vec<WorkflowTargetCreate>,
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
    pub(super) id: Box<str>,
    pub(super) name: Box<str>,
    #[serde(default)]
    pub(super) description: Box<str>,
    #[serde(default)]
    pub(super) global_shortcut: Box<str>,
    pub(super) base_revision: u64,
    pub(super) targets: Vec<WorkflowTargetCreate>,
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
    pub(crate) software_id: Box<str>,
    pub(crate) adapter_plan: WorkflowAdapterPlan,
    pub(crate) dictionary_ids: Vec<Box<str>>,
    pub(crate) write_dictionary_id: Option<Box<str>>,
    #[serde(default)]
    pub(crate) collect_new_sources: Option<bool>,
    pub(crate) font_policy: Option<WorkflowFontPolicy>,
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
    pub fn write_dictionary_id(&self) -> Option<&str> {
        self.write_dictionary_id.as_deref()
    }

    #[must_use]
    pub const fn adapter_plan(&self) -> &WorkflowAdapterPlan {
        &self.adapter_plan
    }

    pub fn collection_preference(&self) -> Option<bool> { self.collect_new_sources }
    pub fn collection_enabled(&self) -> bool { self.write_dictionary_id.is_some() && self.collect_new_sources.unwrap_or(true) }

    #[must_use]
    pub const fn font_policy(&self) -> Option<&WorkflowFontPolicy> {
        self.font_policy.as_ref()
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowView {
    pub(super) id: Box<str>,
    pub(super) name: Box<str>,
    pub(super) description: Box<str>,
    pub(super) global_shortcut: Box<str>,
    pub(super) revision: u64,
    pub(super) targets: Vec<WorkflowTargetView>,
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

#[derive(Clone, Debug)]
pub struct EffectiveWorkflowIntent {
    pub(super) workflow_id: Box<str>,
    pub(super) targets: Vec<EffectiveTargetIntent>,
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
    pub(super) software_id: Box<str>,
    pub(super) runtime_spec: DesktopRuntimeSpec,
    pub(super) requested_features: Vec<Feature>,
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
