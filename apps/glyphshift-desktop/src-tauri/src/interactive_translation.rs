use super::*;
use glyphshift_interactive_translation::{
    CancellationSignal, DictionaryLookup, DictionaryTranslation, ExternalTranslationPresenter,
    InteractiveTranslationError, InteractiveTranslationSession, PresentationBlock,
    PresentationError, ProviderRequest, ProviderTranslation, TranslationLocales, TranslationOrigin,
    TranslationProvider, TranslationProviderError,
};

const INTERACTIVE_TRANSLATION_EVENT: &str = "interactive-translation";
const INTERACTIVE_TRANSLATION_BUBBLE_EVENT: &str = "interactive-translation-bubble";
const INTERACTIVE_TRANSLATION_BUBBLE_LABEL: &str = "interactive-translation-bubble";
const UIA_ACQUISITION_ADAPTER_ID: &str = "windows.uia.acquire";
const OCR_ACQUISITION_ADAPTER_ID: &str = "windows.ocr.acquire";
const OCR_REGION_HALF_WIDTH: i32 = 320;
const OCR_REGION_HALF_HEIGHT: i32 = 120;
const MAX_SELECTION_ID_BYTES: usize = 256;
const BUBBLE_POINTER_OFFSET: i32 = 18;
const BUBBLE_SCREEN_MARGIN: i32 = 12;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum InteractiveTranslationAcquisitionMode {
    Structured,
    VisualOcr,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct InteractiveTranslationCapabilitiesView {
    visual_ocr_available: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct InteractiveTranslationArmRequest {
    software_id: Box<str>,
    dictionary_id: Box<str>,
    acquisition_mode: InteractiveTranslationAcquisitionMode,
}

impl InteractiveTranslationArmRequest {
    fn validate(&self) -> Result<(), CommandError> {
        if !valid_selection_id(&self.software_id) || !valid_selection_id(&self.dictionary_id) {
            return Err(CommandError::new("interactive_translation.invalid_request"));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn new(
        software_id: impl Into<Box<str>>,
        dictionary_id: impl Into<Box<str>>,
    ) -> Self {
        Self {
            software_id: software_id.into(),
            dictionary_id: dictionary_id.into(),
            acquisition_mode: InteractiveTranslationAcquisitionMode::Structured,
        }
    }

    #[cfg(test)]
    pub(super) fn visual_ocr(
        software_id: impl Into<Box<str>>,
        dictionary_id: impl Into<Box<str>>,
    ) -> Self {
        Self {
            software_id: software_id.into(),
            dictionary_id: dictionary_id.into(),
            acquisition_mode: InteractiveTranslationAcquisitionMode::VisualOcr,
        }
    }

    fn same_selection(&self, other: &Self) -> bool {
        self.software_id == other.software_id && self.dictionary_id == other.dictionary_id
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct InteractiveTranslationResultView {
    blocks: Vec<InteractiveTranslationBlockView>,
    partial: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct InteractiveTranslationBubbleView {
    result: InteractiveTranslationResultView,
    focus_block_index: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct InteractiveTranslationBlockView {
    #[serde(flatten)]
    acquisition: acquisition::DesktopAcquisitionBlockView,
    #[serde(skip_serializing_if = "Option::is_none")]
    translation: Option<Box<str>>,
    translation_state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    origin: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum InteractiveTranslationEvent {
    Capturing {
        shortcut: Box<str>,
    },
    Presented {
        shortcut: Box<str>,
        result: InteractiveTranslationResultView,
    },
    Failed {
        shortcut: Box<str>,
        error: CommandError,
    },
}

#[derive(Clone)]
struct ActiveInteractiveTranslation {
    generation: u64,
    request: InteractiveTranslationArmRequest,
    cancellation: DesktopAcquisitionCancellation,
}

#[derive(Default)]
pub(super) struct InteractiveTranslationBubbleState {
    presentation: Option<InteractiveTranslationBubbleView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BubbleScreenRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

pub(super) struct InteractiveTranslationCaptureState {
    shortcut_available: bool,
    shortcut_id: Option<u32>,
    shortcut_label: Box<str>,
    armed: Option<InteractiveTranslationArmRequest>,
    active: Option<ActiveInteractiveTranslation>,
    ocr_retry: Option<InteractiveTranslationArmRequest>,
    next_generation: u64,
}

impl Default for InteractiveTranslationCaptureState {
    fn default() -> Self {
        Self {
            shortcut_available: false,
            shortcut_id: None,
            shortcut_label: DEFAULT_INTERACTIVE_TRANSLATION_SHORTCUT.into(),
            armed: None,
            active: None,
            ocr_retry: None,
            next_generation: 0,
        }
    }
}

impl InteractiveTranslationCaptureState {
    fn shortcut_label(&self) -> Box<str> {
        self.shortcut_label.clone()
    }

    fn shortcut_matches(&self, shortcut_id: u32) -> bool {
        self.shortcut_available && self.shortcut_id == Some(shortcut_id)
    }

    fn registered_shortcut_id(&self) -> Option<u32> {
        self.shortcut_available
            .then_some(self.shortcut_id)
            .flatten()
    }

    fn can_rebind_shortcut(&self) -> bool {
        self.armed.is_none() && self.active.is_none()
    }

    fn set_shortcut_registration(
        &mut self,
        shortcut_label: Box<str>,
        shortcut_id: u32,
        available: bool,
    ) {
        self.shortcut_label = shortcut_label;
        self.shortcut_id = Some(shortcut_id);
        self.shortcut_available = available;
    }

    fn arm(&mut self, request: InteractiveTranslationArmRequest) -> Result<(), CommandError> {
        if !self.shortcut_available {
            return Err(CommandError::new(
                "interactive_translation.shortcut_unavailable",
            ));
        }
        if self.armed.is_some() || self.active.is_some() {
            return Err(CommandError::new(
                "interactive_translation.request_in_progress",
            ));
        }
        match request.acquisition_mode {
            InteractiveTranslationAcquisitionMode::Structured => self.ocr_retry = None,
            InteractiveTranslationAcquisitionMode::VisualOcr => {
                let eligible = self
                    .ocr_retry
                    .as_ref()
                    .is_some_and(|previous| previous.same_selection(&request));
                if !eligible {
                    return Err(CommandError::new(
                        "interactive_translation.ocr_not_eligible",
                    ));
                }
                self.ocr_retry = None;
            }
        }
        self.armed = Some(request);
        Ok(())
    }

    fn begin(
        &mut self,
    ) -> Option<(
        u64,
        InteractiveTranslationArmRequest,
        DesktopAcquisitionCancellation,
    )> {
        if self.active.is_some() {
            return None;
        }
        let request = self.armed.take()?;
        self.next_generation = self.next_generation.saturating_add(1).max(1);
        let generation = self.next_generation;
        let cancellation = DesktopAcquisitionCancellation::new();
        self.active = Some(ActiveInteractiveTranslation {
            generation,
            request: request.clone(),
            cancellation: cancellation.clone(),
        });
        Some((generation, request, cancellation))
    }

    fn finish(&mut self, generation: u64, ocr_eligible: bool) {
        if self.active.as_ref().map(|active| active.generation) == Some(generation) {
            if let Some(active) = self.active.take() {
                self.ocr_retry = (ocr_eligible
                    && active.request.acquisition_mode
                        == InteractiveTranslationAcquisitionMode::Structured)
                    .then_some(active.request);
            }
        }
    }

    fn cancel(&mut self) {
        self.armed = None;
        self.ocr_retry = None;
        if let Some(active) = self.active.take() {
            active.cancellation.cancel();
        }
    }
}

impl Drop for InteractiveTranslationCaptureState {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Clone)]
struct PreparedInteractiveTranslation {
    software_id: Box<str>,
    executable_path: PathBuf,
    runtime_spec: glyphshift_desktop_backend::DesktopRuntimeSpec,
    dictionary_id: Box<str>,
    dictionary_entries: BTreeMap<Box<str>, Box<str>>,
    locales: TranslationLocales,
    acquisition_mode: InteractiveTranslationAcquisitionMode,
}

impl DesktopApplication {
    pub(super) fn interactive_translation_capabilities(
        &self,
    ) -> InteractiveTranslationCapabilitiesView {
        InteractiveTranslationCapabilitiesView {
            visual_ocr_available: self.runtimes.as_ref().is_some_and(|runtimes| {
                runtimes.supports_acquisition_adapter(OCR_ACQUISITION_ADAPTER_ID)
            }),
        }
    }

    #[cfg(test)]
    pub(super) fn run_interactive_translation_request(
        &mut self,
        request: &InteractiveTranslationArmRequest,
        point: DesktopPoint,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<InteractiveTranslationResultView, CommandError> {
        let prepared = self.prepare_interactive_translation(request)?;
        self.run_interactive_translation(prepared, point, cancellation)
    }

    fn prepare_interactive_translation(
        &self,
        request: &InteractiveTranslationArmRequest,
    ) -> Result<PreparedInteractiveTranslation, CommandError> {
        request.validate()?;
        let snapshot = self.backend.snapshot();
        let software = snapshot
            .software()
            .iter()
            .find(|software| software.id() == request.software_id.as_ref())
            .ok_or_else(|| CommandError::new("interactive_translation.software_not_found"))?;
        let executable_path = software
            .executable_path()
            .map(PathBuf::from)
            .ok_or_else(|| CommandError::new("interactive_translation.software_not_configured"))?;
        let dictionary = self
            .backend
            .dictionary(&request.dictionary_id)
            .map_err(|_| CommandError::new("interactive_translation.dictionary_not_found"))?;
        let locales = TranslationLocales::new(
            dictionary.metadata().source_locale(),
            dictionary.metadata().target_locale(),
        )
        .map_err(|_| CommandError::new("interactive_translation.dictionary_invalid"))?;
        let dictionary_entries = dictionary
            .entries()
            .iter()
            .map(|entry| {
                (
                    Box::<str>::from(entry.source()),
                    Box::<str>::from(entry.translation()),
                )
            })
            .collect();
        let runtime_spec = self
            .backend
            .runtime_spec(&request.software_id)
            .map_err(|_| CommandError::new("interactive_translation.runtime_spec_unavailable"))?;
        Ok(PreparedInteractiveTranslation {
            software_id: request.software_id.clone(),
            executable_path,
            runtime_spec,
            dictionary_id: request.dictionary_id.clone(),
            dictionary_entries,
            locales,
            acquisition_mode: request.acquisition_mode,
        })
    }

    fn translate_foreground_point(
        &mut self,
        request: &InteractiveTranslationArmRequest,
        foreground: &glyphshift_controller_windows::WindowsForegroundPoint,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<InteractiveTranslationResultView, CommandError> {
        let prepared = self.prepare_interactive_translation(request)?;
        if !software::same_windows_path(&prepared.executable_path, foreground.executable().path()) {
            return Err(CommandError::new(
                "interactive_translation.foreground_mismatch",
            ));
        }
        self.run_interactive_translation(
            prepared,
            DesktopPoint::new(foreground.x(), foreground.y()),
            cancellation,
        )
    }

    fn run_interactive_translation(
        &mut self,
        prepared: PreparedInteractiveTranslation,
        point: DesktopPoint,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<InteractiveTranslationResultView, CommandError> {
        let runtimes = self.runtimes.as_mut().ok_or_else(runtime_unavailable)?;
        let acquisition = match prepared.acquisition_mode {
            InteractiveTranslationAcquisitionMode::Structured => runtimes.acquire_primary_point(
                &prepared.software_id,
                &prepared.runtime_spec,
                UIA_ACQUISITION_ADAPTER_ID,
                point,
                cancellation,
            ),
            InteractiveTranslationAcquisitionMode::VisualOcr => runtimes.acquire_primary_region(
                &prepared.software_id,
                &prepared.runtime_spec,
                OCR_ACQUISITION_ADAPTER_ID,
                ocr_region_around(point),
                cancellation,
            ),
        }
        .map_err(acquisition::acquisition_command_error)?;
        let acquisition_blocks = acquisition
            .blocks()
            .iter()
            .map(acquisition::DesktopAcquisitionBlockView::from_runtime)
            .collect::<Vec<_>>();
        let mut session = InteractiveTranslationSession::new(
            SelectedDictionaryLookup {
                dictionary_id: prepared.dictionary_id,
                entries: prepared.dictionary_entries,
            },
            UnavailableTranslationProvider,
            CommandResultPresenter,
        );
        let cancellation_signal = TranslationCancellation(cancellation);
        match session.present(Ok(acquisition), &prepared.locales, &cancellation_signal) {
            Ok(outcome) => Ok(result_view(acquisition_blocks, &outcome)),
            Err(InteractiveTranslationError::TranslationUnavailable(
                TranslationProviderError::NoTranslation,
            )) => Ok(missing_result_view(acquisition_blocks)),
            Err(error) => Err(interactive_translation_error(error)),
        }
    }
}

fn ocr_region_around(point: DesktopPoint) -> DesktopRect {
    DesktopRect::new(
        point.x().saturating_sub(OCR_REGION_HALF_WIDTH),
        point.y().saturating_sub(OCR_REGION_HALF_HEIGHT),
        point.x().saturating_add(OCR_REGION_HALF_WIDTH),
        point.y().saturating_add(OCR_REGION_HALF_HEIGHT),
    )
    .expect("the bounded OCR region is non-empty for every desktop point")
}

struct SelectedDictionaryLookup {
    dictionary_id: Box<str>,
    entries: BTreeMap<Box<str>, Box<str>>,
}

impl DictionaryLookup for SelectedDictionaryLookup {
    fn lookup(&mut self, source: &str) -> Option<DictionaryTranslation> {
        self.entries.get(source).and_then(|translation| {
            DictionaryTranslation::new(self.dictionary_id.clone(), translation.clone()).ok()
        })
    }
}

struct UnavailableTranslationProvider;

impl TranslationProvider for UnavailableTranslationProvider {
    fn translate(
        &mut self,
        _request: ProviderRequest<'_>,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<ProviderTranslation, TranslationProviderError> {
        Err(TranslationProviderError::NoTranslation)
    }
}

struct CommandResultPresenter;

impl ExternalTranslationPresenter for CommandResultPresenter {
    fn replace_session(&mut self, _blocks: &[PresentationBlock]) -> Result<(), PresentationError> {
        Ok(())
    }

    fn clear_session(&mut self) -> Result<(), PresentationError> {
        Ok(())
    }
}

struct TranslationCancellation<'a>(&'a DesktopAcquisitionCancellation);

impl CancellationSignal for TranslationCancellation<'_> {
    fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}

fn result_view(
    acquisition_blocks: Vec<acquisition::DesktopAcquisitionBlockView>,
    outcome: &glyphshift_interactive_translation::InteractiveTranslationOutcome,
) -> InteractiveTranslationResultView {
    let failed = outcome
        .failures()
        .iter()
        .map(|failure| failure.block_index())
        .collect::<BTreeSet<_>>();
    let mut translations = outcome.blocks().iter();
    let blocks = acquisition_blocks
        .into_iter()
        .enumerate()
        .map(|(index, acquisition)| {
            if failed.contains(&index) {
                return missing_block_view(acquisition);
            }
            match translations.next() {
                Some(translation) => InteractiveTranslationBlockView {
                    acquisition,
                    translation: Some(translation.translation().into()),
                    translation_state: "translated",
                    origin: Some(match translation.origin() {
                        TranslationOrigin::Dictionary(_) => "dictionary",
                        TranslationOrigin::Provider(_) => "provider",
                    }),
                },
                None => missing_block_view(acquisition),
            }
        })
        .collect();
    InteractiveTranslationResultView {
        blocks,
        partial: outcome.is_partial(),
    }
}

fn missing_result_view(
    acquisition_blocks: Vec<acquisition::DesktopAcquisitionBlockView>,
) -> InteractiveTranslationResultView {
    InteractiveTranslationResultView {
        blocks: acquisition_blocks
            .into_iter()
            .map(missing_block_view)
            .collect(),
        partial: true,
    }
}

fn missing_block_view(
    acquisition: acquisition::DesktopAcquisitionBlockView,
) -> InteractiveTranslationBlockView {
    InteractiveTranslationBlockView {
        acquisition,
        translation: None,
        translation_state: "missing",
        origin: None,
    }
}

fn interactive_translation_error(error: InteractiveTranslationError) -> CommandError {
    let code = match error {
        InteractiveTranslationError::Cancelled => "interactive_translation.cancelled",
        InteractiveTranslationError::Acquisition(_) => "interactive_translation.acquisition_failed",
        InteractiveTranslationError::TranslationUnavailable(TranslationProviderError::TimedOut) => {
            "interactive_translation.provider_timed_out"
        }
        InteractiveTranslationError::TranslationUnavailable(_) => {
            "interactive_translation.translation_unavailable"
        }
        InteractiveTranslationError::Presentation(_) => {
            "interactive_translation.presentation_failed"
        }
    };
    CommandError::new(code)
}

fn valid_selection_id(value: &str) -> bool {
    !value.is_empty()
        && value == value.trim()
        && value.len() <= MAX_SELECTION_ID_BYTES
        && !value.chars().any(char::is_control)
}

#[cfg(windows)]
pub(super) fn rebind_interactive_translation_shortcut(
    app: &tauri::AppHandle,
    value: &str,
) -> Result<Box<str>, CommandError> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let (candidate, label) = shortcut::parse_global_shortcut(value)?;
    let capture_state = app.state::<Mutex<InteractiveTranslationCaptureState>>();
    let mut capture = capture_state
        .lock()
        .map_err(|_| CommandError::new("interactive_translation.state_unavailable"))?;
    if capture.shortcut_id == Some(candidate.id()) && capture.shortcut_available {
        capture.shortcut_label = label.clone();
        return Ok(label);
    }
    if !capture.can_rebind_shortcut() {
        return Err(CommandError::new("settings.shortcut_busy"));
    }
    let previous_shortcut = capture
        .shortcut_available
        .then(|| shortcut::parse_global_shortcut(&capture.shortcut_label))
        .transpose()
        .map_err(|_| CommandError::new("settings.shortcut_update_failed"))?
        .map(|(shortcut, _)| shortcut);

    app.global_shortcut()
        .register(candidate)
        .map_err(|_| CommandError::new("settings.shortcut_unavailable"))?;
    if let Some(previous_shortcut) = previous_shortcut {
        if app.global_shortcut().unregister(previous_shortcut).is_err() {
            let _ = app.global_shortcut().unregister(candidate);
            return Err(CommandError::new("settings.shortcut_update_failed"));
        }
    }
    capture.set_shortcut_registration(label.clone(), candidate.id(), true);
    Ok(label)
}

#[cfg(not(windows))]
pub(super) fn rebind_interactive_translation_shortcut(
    _app: &tauri::AppHandle,
    _value: &str,
) -> Result<Box<str>, CommandError> {
    Err(CommandError::new("settings.shortcut_unavailable"))
}

#[cfg(windows)]
pub(super) fn matches_interactive_translation_shortcut(
    app: &tauri::AppHandle,
    shortcut: &tauri_plugin_global_shortcut::Shortcut,
) -> bool {
    app.state::<Mutex<InteractiveTranslationCaptureState>>()
        .lock()
        .is_ok_and(|capture| capture.shortcut_matches(shortcut.id()))
}

#[tauri::command]
pub(super) fn desktop_probe_interactive_translation_shortcut(
    app: tauri::AppHandle,
    shortcut: String,
    capture: State<'_, Mutex<InteractiveTranslationCaptureState>>,
) -> Result<shortcut::GlobalShortcutProbeView, CommandError> {
    let registered_shortcut_id = capture
        .lock()
        .map_err(|_| CommandError::new("interactive_translation.state_unavailable"))?
        .registered_shortcut_id();
    shortcut::probe_global_shortcut(&app, &shortcut, registered_shortcut_id)
}

#[tauri::command]
pub(super) fn desktop_arm_interactive_translation(
    request: InteractiveTranslationArmRequest,
    application: State<'_, Mutex<DesktopApplication>>,
    capture: State<'_, Mutex<InteractiveTranslationCaptureState>>,
) -> Result<Box<str>, CommandError> {
    let application = application.lock().map_err(|_| workspace_unavailable())?;
    application.prepare_interactive_translation(&request)?;
    if request.acquisition_mode == InteractiveTranslationAcquisitionMode::VisualOcr
        && !application
            .interactive_translation_capabilities()
            .visual_ocr_available
    {
        return Err(CommandError::new("interactive_translation.ocr_unavailable"));
    }
    drop(application);
    let mut capture = capture
        .lock()
        .map_err(|_| CommandError::new("interactive_translation.state_unavailable"))?;
    capture.arm(request)?;
    Ok(capture.shortcut_label())
}

#[tauri::command]
pub(super) fn desktop_interactive_translation_capabilities(
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<InteractiveTranslationCapabilitiesView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())
        .map(|application| application.interactive_translation_capabilities())
}

#[tauri::command]
pub(super) fn desktop_cancel_interactive_translation(
    capture: State<'_, Mutex<InteractiveTranslationCaptureState>>,
) -> Result<(), CommandError> {
    capture
        .lock()
        .map_err(|_| CommandError::new("interactive_translation.state_unavailable"))?
        .cancel();
    Ok(())
}

#[tauri::command]
pub(super) fn desktop_interactive_translation_bubble(
    bubble: State<'_, Mutex<InteractiveTranslationBubbleState>>,
) -> Result<Option<InteractiveTranslationBubbleView>, CommandError> {
    bubble
        .lock()
        .map_err(|_| CommandError::new("interactive_translation.state_unavailable"))
        .map(|bubble| bubble.presentation.clone())
}

#[tauri::command]
pub(super) fn desktop_dismiss_interactive_translation_bubble(
    app: tauri::AppHandle,
    bubble: State<'_, Mutex<InteractiveTranslationBubbleState>>,
) -> Result<(), CommandError> {
    bubble
        .lock()
        .map_err(|_| CommandError::new("interactive_translation.state_unavailable"))?
        .presentation = None;
    if let Some(window) = app.get_webview_window(INTERACTIVE_TRANSLATION_BUBBLE_LABEL) {
        window
            .hide()
            .map_err(|_| CommandError::new("interactive_translation.presentation_failed"))?;
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn handle_interactive_translation_shortcut(app: &tauri::AppHandle) {
    let capture = app
        .state::<Mutex<InteractiveTranslationCaptureState>>()
        .lock()
        .ok()
        .and_then(|mut capture| {
            let shortcut = capture.shortcut_label();
            capture.begin().map(|(generation, request, cancellation)| {
                (generation, request, cancellation, shortcut)
            })
        });
    let Some((generation, request, cancellation, shortcut)) = capture else {
        return;
    };
    hide_interactive_translation_bubble(app);
    let _ = app.emit(
        INTERACTIVE_TRANSLATION_EVENT,
        InteractiveTranslationEvent::Capturing {
            shortcut: shortcut.clone(),
        },
    );
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let task_app = app_handle.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let foreground = foreground_windows_point()
                .map_err(|_| CommandError::new("interactive_translation.foreground_unavailable"))?;
            let point = DesktopPoint::new(foreground.x(), foreground.y());
            task_app
                .state::<Mutex<DesktopApplication>>()
                .lock()
                .map_err(|_| workspace_unavailable())?
                .translate_foreground_point(&request, &foreground, &cancellation)
                .map(|result| (point, result))
        })
        .await
        .map_err(|_| CommandError::new("interactive_translation.execution_failed"))
        .and_then(|result| result);
        let ocr_eligible = result
            .as_ref()
            .err()
            .is_some_and(|error| error.code() == "acquisition.no_text");
        if let Ok(mut capture) = app_handle
            .state::<Mutex<InteractiveTranslationCaptureState>>()
            .lock()
        {
            capture.finish(generation, ocr_eligible);
        }
        let (event, presented_near_target) = match result {
            Ok((point, result)) => {
                let presented_near_target =
                    present_interactive_translation_bubble(&app_handle, point, &result).is_ok();
                (
                    InteractiveTranslationEvent::Presented {
                        shortcut: shortcut.clone(),
                        result,
                    },
                    presented_near_target,
                )
            }
            Err(error) => (
                InteractiveTranslationEvent::Failed { shortcut, error },
                false,
            ),
        };
        let _ = app_handle.emit(INTERACTIVE_TRANSLATION_EVENT, event);
        if !presented_near_target {
            software::focus_main_window(&app_handle);
        }
    });
}

fn present_interactive_translation_bubble(
    app: &tauri::AppHandle,
    point: DesktopPoint,
    result: &InteractiveTranslationResultView,
) -> tauri::Result<()> {
    let presentation = InteractiveTranslationBubbleView {
        focus_block_index: closest_result_block(result, point),
        result: result.clone(),
    };
    if let Ok(mut state) = app
        .state::<Mutex<InteractiveTranslationBubbleState>>()
        .lock()
    {
        state.presentation = Some(presentation.clone());
    }
    let presented = show_interactive_translation_bubble(app, point, &presentation);
    if presented.is_err() {
        hide_interactive_translation_bubble(app);
    }
    presented
}

fn show_interactive_translation_bubble(
    app: &tauri::AppHandle,
    point: DesktopPoint,
    presentation: &InteractiveTranslationBubbleView,
) -> tauri::Result<()> {
    let window = app
        .get_webview_window(INTERACTIVE_TRANSLATION_BUBBLE_LABEL)
        .ok_or(tauri::Error::WindowNotFound)?;
    let window_size = window.outer_size()?;
    let screens = window
        .available_monitors()?
        .into_iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            BubbleScreenRect {
                left: position.x,
                top: position.y,
                right: position
                    .x
                    .saturating_add(i32::try_from(size.width).unwrap_or(i32::MAX)),
                bottom: position
                    .y
                    .saturating_add(i32::try_from(size.height).unwrap_or(i32::MAX)),
            }
        })
        .collect::<Vec<_>>();
    let position = bubble_position(
        point,
        i32::try_from(window_size.width).unwrap_or(i32::MAX),
        i32::try_from(window_size.height).unwrap_or(i32::MAX),
        &screens,
    );
    window.set_position(tauri::PhysicalPosition::new(position.0, position.1))?;
    app.emit_to(
        INTERACTIVE_TRANSLATION_BUBBLE_LABEL,
        INTERACTIVE_TRANSLATION_BUBBLE_EVENT,
        presentation,
    )?;
    window.show()
}

fn hide_interactive_translation_bubble(app: &tauri::AppHandle) {
    if let Ok(mut state) = app
        .state::<Mutex<InteractiveTranslationBubbleState>>()
        .lock()
    {
        state.presentation = None;
    }
    if let Some(window) = app.get_webview_window(INTERACTIVE_TRANSLATION_BUBBLE_LABEL) {
        let _ = window.hide();
    }
}

fn closest_result_block(result: &InteractiveTranslationResultView, point: DesktopPoint) -> usize {
    result
        .blocks
        .iter()
        .enumerate()
        .min_by_key(|(_, block)| block.acquisition.distance_squared_to(point))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn bubble_position(
    point: DesktopPoint,
    width: i32,
    height: i32,
    screens: &[BubbleScreenRect],
) -> (i32, i32) {
    let screen = screens
        .iter()
        .min_by_key(|screen| {
            let x = point.x().clamp(screen.left, screen.right);
            let y = point.y().clamp(screen.top, screen.bottom);
            let dx = i64::from(point.x()) - i64::from(x);
            let dy = i64::from(point.y()) - i64::from(y);
            dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
        })
        .copied()
        .unwrap_or(BubbleScreenRect {
            left: i32::MIN,
            top: i32::MIN,
            right: i32::MAX,
            bottom: i32::MAX,
        });
    let min_x = screen.left.saturating_add(BUBBLE_SCREEN_MARGIN);
    let max_x = screen
        .right
        .saturating_sub(BUBBLE_SCREEN_MARGIN)
        .saturating_sub(width)
        .max(min_x);
    let min_y = screen.top.saturating_add(BUBBLE_SCREEN_MARGIN);
    let max_y = screen
        .bottom
        .saturating_sub(BUBBLE_SCREEN_MARGIN)
        .saturating_sub(height)
        .max(min_y);
    let right = point.x().saturating_add(BUBBLE_POINTER_OFFSET);
    let left = point
        .x()
        .saturating_sub(BUBBLE_POINTER_OFFSET)
        .saturating_sub(width);
    let below = point.y().saturating_add(BUBBLE_POINTER_OFFSET);
    let above = point
        .y()
        .saturating_sub(BUBBLE_POINTER_OFFSET)
        .saturating_sub(height);
    let x = if right <= max_x { right } else { left }.clamp(min_x, max_x);
    let y = if below <= max_y { below } else { above }.clamp(min_y, max_y);
    (x, y)
}

pub(super) fn manage_interactive_translation(app: &mut tauri::App) {
    app.manage(Mutex::new(InteractiveTranslationBubbleState::default()));
    #[cfg(windows)]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        let configured: Box<str> = app
            .state::<Mutex<AppSettingsStore>>()
            .lock()
            .ok()
            .map(|settings| settings.interactive_translation_shortcut().into())
            .unwrap_or_else(|| DEFAULT_INTERACTIVE_TRANSLATION_SHORTCUT.into());
        let (shortcut, label) = shortcut::parse_global_shortcut(&configured)
            .or_else(|_| shortcut::parse_global_shortcut(DEFAULT_INTERACTIVE_TRANSLATION_SHORTCUT))
            .expect("default interactive translation shortcut must be valid");
        let shortcut_available = app.global_shortcut().register(shortcut).is_ok();
        let mut capture = InteractiveTranslationCaptureState::default();
        capture.set_shortcut_registration(label, shortcut.id(), shortcut_available);
        app.manage(Mutex::new(capture));
    }
    #[cfg(not(windows))]
    {
        app.manage(Mutex::new(InteractiveTranslationCaptureState::default()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result_block(
        source: &str,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    ) -> InteractiveTranslationBlockView {
        InteractiveTranslationBlockView {
            acquisition: acquisition::DesktopAcquisitionBlockView::synthetic_at(
                source,
                DesktopRect::new(left, top, right, bottom).expect("valid result anchor"),
            ),
            translation: None,
            translation_state: "missing",
            origin: None,
        }
    }

    fn available_state() -> InteractiveTranslationCaptureState {
        let mut state = InteractiveTranslationCaptureState::default();
        state.shortcut_available = true;
        state
    }

    #[test]
    fn one_armed_request_becomes_one_cancellable_active_capture() {
        let mut state = available_state();
        state
            .arm(InteractiveTranslationArmRequest::new(
                "software.product",
                "dictionary.product",
            ))
            .expect("arm capture");
        assert_eq!(
            state
                .arm(InteractiveTranslationArmRequest::new(
                    "software.other",
                    "dictionary.product",
                ))
                .err(),
            Some(CommandError::new(
                "interactive_translation.request_in_progress"
            ))
        );

        let (generation, request, cancellation) = state.begin().expect("begin capture");
        assert_eq!(request.software_id.as_ref(), "software.product");
        assert!(state.begin().is_none());
        state.cancel();
        assert!(cancellation.is_cancelled());
        state.finish(generation, false);
    }

    #[test]
    fn dropping_capture_state_cancels_an_active_request() {
        let mut state = available_state();
        state
            .arm(InteractiveTranslationArmRequest::new(
                "software.product",
                "dictionary.product",
            ))
            .expect("arm capture");
        let (_, _, cancellation) = state.begin().expect("begin capture");

        drop(state);

        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn visual_ocr_can_only_be_armed_after_no_text_for_the_same_selection() {
        let mut state = available_state();
        assert_eq!(
            state
                .arm(InteractiveTranslationArmRequest::visual_ocr(
                    "software.product",
                    "dictionary.product",
                ))
                .err(),
            Some(CommandError::new(
                "interactive_translation.ocr_not_eligible"
            ))
        );

        state
            .arm(InteractiveTranslationArmRequest::new(
                "software.product",
                "dictionary.product",
            ))
            .expect("arm structured capture");
        let (generation, _, _) = state.begin().expect("begin structured capture");
        state.finish(generation, true);

        assert_eq!(
            state
                .arm(InteractiveTranslationArmRequest::visual_ocr(
                    "software.other",
                    "dictionary.product",
                ))
                .err(),
            Some(CommandError::new(
                "interactive_translation.ocr_not_eligible"
            ))
        );
        state
            .arm(InteractiveTranslationArmRequest::visual_ocr(
                "software.product",
                "dictionary.product",
            ))
            .expect("arm eligible visual OCR retry");
    }

    #[test]
    fn successful_or_cancelled_structured_requests_do_not_enable_ocr() {
        for cancelled in [false, true] {
            let mut state = available_state();
            state
                .arm(InteractiveTranslationArmRequest::new(
                    "software.product",
                    "dictionary.product",
                ))
                .expect("arm structured capture");
            let (generation, _, _) = state.begin().expect("begin structured capture");
            if cancelled {
                state.cancel();
            } else {
                state.finish(generation, false);
            }
            assert!(state
                .arm(InteractiveTranslationArmRequest::visual_ocr(
                    "software.product",
                    "dictionary.product",
                ))
                .is_err());
        }
    }

    #[test]
    fn ocr_region_is_bounded_even_at_extreme_desktop_coordinates() {
        for point in [
            DesktopPoint::new(i32::MIN, i32::MIN),
            DesktopPoint::new(i32::MAX, i32::MAX),
        ] {
            let region = ocr_region_around(point);
            assert!(region.right() > region.left());
            assert!(region.bottom() > region.top());
            assert!(region.right().saturating_sub(region.left()) <= OCR_REGION_HALF_WIDTH * 2);
            assert!(region.bottom().saturating_sub(region.top()) <= OCR_REGION_HALF_HEIGHT * 2);
        }
    }

    #[test]
    fn arm_request_requires_an_explicit_acquisition_mode() {
        let missing =
            serde_json::from_value::<InteractiveTranslationArmRequest>(serde_json::json!({
                "softwareId": "software.product",
                "dictionaryId": "dictionary.product"
            }));
        assert!(missing.is_err());

        let visual =
            serde_json::from_value::<InteractiveTranslationArmRequest>(serde_json::json!({
                "softwareId": "software.product",
                "dictionaryId": "dictionary.product",
                "acquisitionMode": "visualOcr"
            }))
            .expect("explicit visual OCR arm request");
        assert_eq!(
            visual.acquisition_mode,
            InteractiveTranslationAcquisitionMode::VisualOcr
        );
    }

    #[test]
    fn bubble_focuses_the_result_block_nearest_the_pointer() {
        let result = InteractiveTranslationResultView {
            blocks: vec![
                result_block("far", 20, 20, 80, 50),
                result_block("near", 500, 300, 620, 350),
            ],
            partial: false,
        };

        assert_eq!(
            closest_result_block(&result, DesktopPoint::new(540, 320)),
            1
        );
        assert_eq!(closest_result_block(&result, DesktopPoint::new(30, 30)), 0);
    }

    #[test]
    fn bubble_position_stays_inside_the_pointer_monitor() {
        let screens = [
            BubbleScreenRect {
                left: -1920,
                top: 0,
                right: 0,
                bottom: 1080,
            },
            BubbleScreenRect {
                left: 0,
                top: 0,
                right: 2560,
                bottom: 1440,
            },
        ];

        let (left, top) = bubble_position(DesktopPoint::new(-20, 1060), 420, 210, &screens);
        assert!((-1908..=-432).contains(&left));
        assert!((12..=858).contains(&top));

        let (left, top) = bubble_position(DesktopPoint::new(2540, 1420), 420, 210, &screens);
        assert!((12..=2128).contains(&left));
        assert!((12..=1218).contains(&top));
    }
}
