use super::*;
use glyphshift_interactive_translation::{
    CancellationSignal, DictionaryLookup, DictionaryTranslation, ExternalTranslationPresenter,
    InteractiveTranslationError, InteractiveTranslationSession, PresentationBlock,
    PresentationError, ProviderRequest, ProviderTranslation, TranslationLocales, TranslationOrigin,
    TranslationProvider, TranslationProviderError,
};

const INTERACTIVE_TRANSLATION_EVENT: &str = "interactive-translation";
const INTERACTIVE_TRANSLATION_SHORTCUT: &str = "Ctrl+Shift+F9";
const UIA_ACQUISITION_ADAPTER_ID: &str = "windows.uia.acquire";
const MAX_SELECTION_ID_BYTES: usize = 256;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct InteractiveTranslationArmRequest {
    software_id: Box<str>,
    dictionary_id: Box<str>,
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
        }
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
        shortcut: &'static str,
    },
    Presented {
        shortcut: &'static str,
        result: InteractiveTranslationResultView,
    },
    Failed {
        shortcut: &'static str,
        error: CommandError,
    },
}

#[derive(Clone)]
struct ActiveInteractiveTranslation {
    generation: u64,
    cancellation: DesktopAcquisitionCancellation,
}

#[derive(Default)]
pub(super) struct InteractiveTranslationCaptureState {
    shortcut_available: bool,
    armed: Option<InteractiveTranslationArmRequest>,
    active: Option<ActiveInteractiveTranslation>,
    next_generation: u64,
}

impl InteractiveTranslationCaptureState {
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
            cancellation: cancellation.clone(),
        });
        Some((generation, request, cancellation))
    }

    fn finish(&mut self, generation: u64) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.generation == generation)
        {
            self.active = None;
        }
    }

    fn cancel(&mut self) {
        self.armed = None;
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
}

impl DesktopApplication {
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
        let acquisition = self
            .runtimes
            .as_mut()
            .ok_or_else(runtime_unavailable)?
            .acquire_primary_point(
                &prepared.software_id,
                &prepared.runtime_spec,
                UIA_ACQUISITION_ADAPTER_ID,
                point,
                cancellation,
            )
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

#[tauri::command]
pub(super) fn desktop_arm_interactive_translation(
    request: InteractiveTranslationArmRequest,
    application: State<'_, Mutex<DesktopApplication>>,
    capture: State<'_, Mutex<InteractiveTranslationCaptureState>>,
) -> Result<&'static str, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .prepare_interactive_translation(&request)?;
    capture
        .lock()
        .map_err(|_| CommandError::new("interactive_translation.state_unavailable"))?
        .arm(request)?;
    Ok(INTERACTIVE_TRANSLATION_SHORTCUT)
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

#[cfg(windows)]
pub(super) fn interactive_translation_shortcut() -> tauri_plugin_global_shortcut::Shortcut {
    use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F9)
}

#[cfg(windows)]
pub(super) fn handle_interactive_translation_shortcut(app: &tauri::AppHandle) {
    let capture = app
        .state::<Mutex<InteractiveTranslationCaptureState>>()
        .lock()
        .ok()
        .and_then(|mut capture| capture.begin());
    let Some((generation, request, cancellation)) = capture else {
        return;
    };
    let _ = app.emit(
        INTERACTIVE_TRANSLATION_EVENT,
        InteractiveTranslationEvent::Capturing {
            shortcut: INTERACTIVE_TRANSLATION_SHORTCUT,
        },
    );
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let task_app = app_handle.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let foreground = foreground_windows_point()
                .map_err(|_| CommandError::new("interactive_translation.foreground_unavailable"))?;
            task_app
                .state::<Mutex<DesktopApplication>>()
                .lock()
                .map_err(|_| workspace_unavailable())?
                .translate_foreground_point(&request, &foreground, &cancellation)
        })
        .await
        .map_err(|_| CommandError::new("interactive_translation.execution_failed"))
        .and_then(|result| result);
        if let Ok(mut capture) = app_handle
            .state::<Mutex<InteractiveTranslationCaptureState>>()
            .lock()
        {
            capture.finish(generation);
        }
        let event = match result {
            Ok(result) => InteractiveTranslationEvent::Presented {
                shortcut: INTERACTIVE_TRANSLATION_SHORTCUT,
                result,
            },
            Err(error) => InteractiveTranslationEvent::Failed {
                shortcut: INTERACTIVE_TRANSLATION_SHORTCUT,
                error,
            },
        };
        let _ = app_handle.emit(INTERACTIVE_TRANSLATION_EVENT, event);
        software::focus_main_window(&app_handle);
    });
}

pub(super) fn manage_interactive_translation(app: &mut tauri::App) {
    app.manage(Mutex::new(InteractiveTranslationCaptureState::default()));
    #[cfg(windows)]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        let shortcut_available = app
            .global_shortcut()
            .register(interactive_translation_shortcut())
            .is_ok();
        if let Ok(mut capture) = app
            .state::<Mutex<InteractiveTranslationCaptureState>>()
            .lock()
        {
            capture.shortcut_available = shortcut_available;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        state.finish(generation);
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
}
