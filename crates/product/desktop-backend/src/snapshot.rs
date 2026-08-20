use super::*;
use crate::software::software_view;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSnapshot {
    selected_software_id: Option<Box<str>>,
    software: Vec<SoftwareView>,
    dictionaries: Vec<DictionarySummaryView>,
    workflows: Vec<WorkflowSummaryView>,
    activations: Vec<WorkflowActivationSnapshot>,
    artifact_warnings: Vec<ArtifactWarningView>,
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

    #[must_use]
    pub fn artifact_warnings(&self) -> &[ArtifactWarningView] {
        &self.artifact_warnings
    }
}
impl DesktopBackend {
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
                    metadata: dictionary.metadata().clone(),
                    revision: dictionary.revision(),
                    entry_count: dictionary.entries().len(),
                    installation: self
                        .dictionary_installations
                        .get(dictionary.id())
                        .cloned()
                        .unwrap_or_else(DictionaryInstallationSummaryView::unmanaged),
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
                            font_policy: target.font_policy.clone(),
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
            artifact_warnings: self.artifact_warnings.clone(),
        }
    }
}
