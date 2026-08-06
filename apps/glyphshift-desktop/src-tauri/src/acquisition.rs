use super::*;
use glyphshift_desktop_runtime::{DesktopRect, Granularity, Provenance};
use std::sync::Arc;

const MAX_PRODUCT_ID_BYTES: usize = 256;
const MAX_CANCELLATION_ID_BYTES: usize = 128;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct DesktopPointAcquisitionRequest {
    software_id: Box<str>,
    target_id: u64,
    adapter_id: Box<str>,
    x: i32,
    y: i32,
    cancellation_id: Box<str>,
}

impl DesktopPointAcquisitionRequest {
    #[cfg(test)]
    pub(super) fn new(
        software_id: impl Into<Box<str>>,
        target_id: u64,
        adapter_id: impl Into<Box<str>>,
        x: i32,
        y: i32,
        cancellation_id: impl Into<Box<str>>,
    ) -> Self {
        Self {
            software_id: software_id.into(),
            target_id,
            adapter_id: adapter_id.into(),
            x,
            y,
            cancellation_id: cancellation_id.into(),
        }
    }

    fn validate(&self) -> Result<(), CommandError> {
        if !valid_id(&self.software_id, MAX_PRODUCT_ID_BYTES)
            || self.target_id == 0
            || !valid_id(&self.adapter_id, MAX_PRODUCT_ID_BYTES)
            || !valid_id(&self.cancellation_id, MAX_CANCELLATION_ID_BYTES)
        {
            return Err(CommandError::new("acquisition.invalid_request"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct DesktopPointAcquisitionView {
    blocks: Vec<DesktopAcquisitionBlockView>,
}

impl DesktopPointAcquisitionView {
    pub(super) fn from_runtime(result: glyphshift_desktop_runtime::AcquisitionResult) -> Self {
        Self {
            blocks: result
                .blocks()
                .iter()
                .map(DesktopAcquisitionBlockView::from_runtime)
                .collect(),
        }
    }

    #[cfg(test)]
    pub(super) fn synthetic(source: impl Into<Box<str>>) -> Self {
        Self {
            blocks: vec![DesktopAcquisitionBlockView {
                source: source.into(),
                anchors: vec![DesktopRectView {
                    left: 10,
                    top: 20,
                    right: 80,
                    bottom: 44,
                }],
                granularity: "control",
                provenance: "structured",
                confidence_basis_points: None,
            }],
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct DesktopAcquisitionBlockView {
    source: Box<str>,
    anchors: Vec<DesktopRectView>,
    granularity: &'static str,
    provenance: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_basis_points: Option<u16>,
}

impl DesktopAcquisitionBlockView {
    pub(super) fn from_runtime(block: &glyphshift_desktop_runtime::SourceBlock) -> Self {
        Self {
            source: block.source().into(),
            anchors: block
                .anchors()
                .iter()
                .copied()
                .map(DesktopRectView::from_runtime)
                .collect(),
            granularity: granularity_id(block.granularity()),
            provenance: provenance_id(block.provenance()),
            confidence_basis_points: block.confidence().map(|value| value.basis_points()),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct DesktopRectView {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl DesktopRectView {
    const fn from_runtime(rect: DesktopRect) -> Self {
        Self {
            left: rect.left(),
            top: rect.top(),
            right: rect.right(),
            bottom: rect.bottom(),
        }
    }
}

const fn granularity_id(granularity: Granularity) -> &'static str {
    match granularity {
        Granularity::Word => "word",
        Granularity::Control => "control",
        Granularity::Line => "line",
        Granularity::Region => "region",
    }
}

const fn provenance_id(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::Structured => "structured",
        Provenance::Visual => "visual",
    }
}

type ActiveAcquisitions = Arc<Mutex<BTreeMap<Box<str>, DesktopAcquisitionCancellation>>>;

#[derive(Clone)]
struct AcquisitionRegistry {
    active: ActiveAcquisitions,
}

impl AcquisitionRegistry {
    fn register(
        &self,
        cancellation_id: &str,
    ) -> Result<DesktopAcquisitionCancellation, CommandError> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| CommandError::new("acquisition.state_unavailable"))?;
        if active.contains_key(cancellation_id) {
            return Err(CommandError::new("acquisition.request_in_progress"));
        }
        let cancellation = DesktopAcquisitionCancellation::new();
        active.insert(cancellation_id.into(), cancellation.clone());
        Ok(cancellation)
    }

    fn complete(&self, cancellation_id: &str) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(cancellation_id);
        }
    }

    fn cancel(&self, cancellation_id: &str) -> Result<(), CommandError> {
        let active = self
            .active
            .lock()
            .map_err(|_| CommandError::new("acquisition.state_unavailable"))?;
        let cancellation = active
            .get(cancellation_id)
            .ok_or_else(|| CommandError::new("acquisition.request_not_found"))?;
        cancellation.cancel();
        Ok(())
    }

    fn cancel_all(&self) {
        if let Ok(active) = self.active.lock() {
            for cancellation in active.values() {
                cancellation.cancel();
            }
        }
    }
}

pub(super) struct AcquisitionCommandState {
    registry: AcquisitionRegistry,
}

impl Default for AcquisitionCommandState {
    fn default() -> Self {
        Self {
            registry: AcquisitionRegistry {
                active: Arc::new(Mutex::new(BTreeMap::new())),
            },
        }
    }
}

impl Drop for AcquisitionCommandState {
    fn drop(&mut self) {
        self.registry.cancel_all();
    }
}

impl DesktopApplication {
    pub(super) fn acquire_point(
        &mut self,
        request: &DesktopPointAcquisitionRequest,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<DesktopPointAcquisitionView, CommandError> {
        let spec = self
            .backend
            .runtime_spec(&request.software_id)
            .map_err(acquisition_backend_error)?;
        self.runtimes
            .as_mut()
            .ok_or_else(runtime_unavailable)?
            .acquire_point(
                &request.software_id,
                &spec,
                request.target_id,
                &request.adapter_id,
                DesktopPoint::new(request.x, request.y),
                cancellation,
            )
            .map(DesktopPointAcquisitionView::from_runtime)
            .map_err(acquisition_command_error)
    }
}

#[tauri::command]
pub(super) async fn desktop_acquire_point(
    request: DesktopPointAcquisitionRequest,
    app: tauri::AppHandle,
    state: State<'_, AcquisitionCommandState>,
) -> Result<DesktopPointAcquisitionView, CommandError> {
    request.validate()?;
    let registry = state.registry.clone();
    let cancellation = registry.register(&request.cancellation_id)?;
    let cancellation_id = request.cancellation_id.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let application = app.state::<Mutex<DesktopApplication>>();
        let result = application
            .lock()
            .map_err(|_| workspace_unavailable())?
            .acquire_point(&request, &cancellation);
        result
    })
    .await
    .map_err(|_| CommandError::new("acquisition.execution_failed"));
    registry.complete(&cancellation_id);
    task?
}

#[tauri::command]
pub(super) fn desktop_cancel_point_acquisition(
    cancellation_id: Box<str>,
    state: State<'_, AcquisitionCommandState>,
) -> Result<(), CommandError> {
    if !valid_id(&cancellation_id, MAX_CANCELLATION_ID_BYTES) {
        return Err(CommandError::new("acquisition.invalid_request"));
    }
    state.registry.cancel(&cancellation_id)
}

fn valid_id(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value == value.trim()
        && value.len() <= max_bytes
        && !value.chars().any(char::is_control)
}

fn acquisition_backend_error(error: BackendError) -> CommandError {
    match error {
        BackendError::UnknownSoftware(_) => CommandError::new("acquisition.software_not_found"),
        _ => CommandError::new("acquisition.runtime_spec_unavailable"),
    }
}

pub(super) fn acquisition_command_error(error: DesktopAcquisitionError) -> CommandError {
    let code = match error {
        DesktopAcquisitionError::UnknownTarget => "acquisition.target_not_found",
        DesktopAcquisitionError::InvalidState => "acquisition.invalid_state",
        DesktopAcquisitionError::ControllerUnavailable => "acquisition.controller_unavailable",
        DesktopAcquisitionError::TargetUnavailable => "acquisition.target_unavailable",
        DesktopAcquisitionError::WorkerUnavailable => "acquisition.adapter_unavailable",
        DesktopAcquisitionError::PermissionDenied => "acquisition.permission_denied",
        DesktopAcquisitionError::NoText => "acquisition.no_text",
        DesktopAcquisitionError::ProviderUnavailable => "acquisition.provider_unavailable",
        DesktopAcquisitionError::TimedOut => "acquisition.timed_out",
        DesktopAcquisitionError::Cancelled => "acquisition.cancelled",
    };
    CommandError::new(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_ids_are_rejected_until_the_original_request_completes() {
        let state = AcquisitionCommandState::default();
        let first = state.registry.register("request-1").expect("register");

        assert_eq!(
            state.registry.register("request-1").err(),
            Some(CommandError::new("acquisition.request_in_progress"))
        );
        state.registry.cancel("request-1").expect("cancel");
        assert!(first.is_cancelled());
        state.registry.complete("request-1");
        assert!(state.registry.register("request-1").is_ok());
    }

    #[test]
    fn dropping_command_state_cancels_every_active_request() {
        let state = AcquisitionCommandState::default();
        let first = state
            .registry
            .register("request-1")
            .expect("register first");
        let second = state
            .registry
            .register("request-2")
            .expect("register second");

        drop(state);

        assert!(first.is_cancelled());
        assert!(second.is_cancelled());
    }

    #[test]
    fn runtime_errors_map_to_stable_codes_without_worker_text() {
        let cases = [
            (
                DesktopAcquisitionError::UnknownTarget,
                "acquisition.target_not_found",
            ),
            (
                DesktopAcquisitionError::TargetUnavailable,
                "acquisition.target_unavailable",
            ),
            (
                DesktopAcquisitionError::WorkerUnavailable,
                "acquisition.adapter_unavailable",
            ),
            (
                DesktopAcquisitionError::PermissionDenied,
                "acquisition.permission_denied",
            ),
            (DesktopAcquisitionError::NoText, "acquisition.no_text"),
            (
                DesktopAcquisitionError::ProviderUnavailable,
                "acquisition.provider_unavailable",
            ),
            (DesktopAcquisitionError::TimedOut, "acquisition.timed_out"),
            (DesktopAcquisitionError::Cancelled, "acquisition.cancelled"),
        ];

        for (error, code) in cases {
            assert_eq!(acquisition_command_error(error), CommandError::new(code));
        }
    }

    #[test]
    fn product_result_contains_only_text_geometry_and_semantic_metadata() {
        let value = serde_json::to_value(DesktopPointAcquisitionView::synthetic("Open"))
            .expect("serialize product acquisition");

        assert_eq!(value["blocks"][0]["source"], "Open");
        assert_eq!(value["blocks"][0]["granularity"], "control");
        assert_eq!(value["blocks"][0]["provenance"], "structured");
        assert!(value["blocks"][0].get("anchors").is_some());
        for forbidden in ["pid", "grant", "token", "path", "worker", "executable"] {
            assert!(!value.to_string().to_ascii_lowercase().contains(forbidden));
        }
    }

    #[test]
    fn request_validation_rejects_empty_ids_zero_targets_and_oversized_cancellation_ids() {
        for request in [
            DesktopPointAcquisitionRequest::new("", 1, "test.acquire", 1, 2, "request-1"),
            DesktopPointAcquisitionRequest::new(
                "software.fixture",
                0,
                "test.acquire",
                1,
                2,
                "request-1",
            ),
            DesktopPointAcquisitionRequest::new(
                "software.fixture",
                1,
                "test.acquire",
                1,
                2,
                "x".repeat(MAX_CANCELLATION_ID_BYTES + 1),
            ),
        ] {
            assert_eq!(
                request.validate(),
                Err(CommandError::new("acquisition.invalid_request"))
            );
        }
    }
}
