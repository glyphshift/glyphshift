use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeRunCreateRequest {
    pub(crate) id: Box<str>,
    pub(crate) name: Box<str>,
    pub(crate) software_id: Box<str>,
    pub(crate) adapter_ids: Vec<Box<str>>,
    pub(crate) live_preview_enabled: bool,
    #[serde(default)]
    pub(crate) excluded_dictionary_ids: Vec<Box<str>>,
    pub(crate) dictionary: ProbeDictionaryBindingRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeRunUpdateRequest {
    pub(crate) run_id: Box<str>,
    pub(crate) name: Box<str>,
    pub(crate) dictionary_id: Box<str>,
    #[serde(default)]
    pub(crate) excluded_dictionary_ids: Vec<Box<str>>,
    pub(crate) adapter_ids: Vec<Box<str>>,
    pub(crate) live_preview_enabled: bool,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ProbeDictionaryBindingRequest {
    Existing {
        dictionary_id: Box<str>,
    },
    New {
        id: Box<str>,
        name: Box<str>,
        description: Box<str>,
        source_locale: Box<str>,
        target_locale: Box<str>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeRunQueryRequest {
    pub(crate) run_id: Box<str>,
    pub(crate) search: Box<str>,
    #[serde(default)]
    pub(crate) adapter_ids: Vec<Box<str>>,
    #[serde(default)]
    pub(crate) translation_filter: ProbeTranslationFilter,
    pub(crate) merge_rules: Option<bool>,
    pub(crate) page: usize,
    pub(crate) page_size: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeTranslationEditRequest {
    pub(crate) run_id: Box<str>,
    pub(crate) source: Box<str>,
    pub(crate) translation: Box<str>,
    #[serde(default)]
    pub(crate) translation_context: Option<CaptureTranslationContext>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeDictionarySyncEntryRequest {
    pub(crate) source: Box<str>,
    pub(crate) translation: Box<str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeDictionarySyncRequest {
    pub(crate) run_id: Box<str>,
    pub(crate) entries: Vec<ProbeDictionarySyncEntryRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeBulkRequest {
    pub(crate) run_id: Box<str>,
    pub(crate) sources: Vec<Box<str>>,
    pub(crate) action: Box<str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeExportRequest {
    pub(crate) run_id: Box<str>,
    pub(crate) format: ProbeExportFormat,
    pub(crate) output_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeRunView {
    pub(crate) workflow_runtime: Option<WorkflowRuntimeView>,
    #[serde(flatten)]
    pub(crate) summary: ProbeRunSummary,
    pub(crate) dictionary_revision: u64,
    pub(crate) exclusion_revisions: Vec<u64>,
    pub(crate) dictionary_entry_count: usize,
    pub(crate) runtime_capability: Option<ProbeRuntimeCapability>,
    pub(crate) quick_probe: bool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProbeRuntimeCapability {
    DirectReplace,
    CollectionOnly,
    NoSignal,
}

impl ProbeRuntimeCapability {
    pub(crate) fn from_status(status: &DesktopRuntimeStatus) -> Self {
        if status.is_feature_active(Feature::TextReplace) {
            Self::DirectReplace
        } else if status.is_feature_active(Feature::TextObserve) {
            Self::CollectionOnly
        } else {
            Self::NoSignal
        }
    }
}
