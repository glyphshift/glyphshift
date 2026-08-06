use glyphshift_acquisition::{AcquisitionError, AuthorizedTarget, DesktopRect};
use glyphshift_adapter_ocr::{FrameSource, TargetFrame};
use glyphshift_worker_process_grant::{process_started_at, AuthorizedProcess};
use std::error::Error;
use std::io;
use std::sync::{Arc, Mutex};
use windows_capture::capture::{Context, GraphicsCaptureApiError, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::{
    Error as GraphicsCaptureError, InternalCaptureControl,
};
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowDisplayAffinity, GetWindowRect, GetWindowThreadProcessId, WDA_NONE,
};

pub struct WindowsGraphicsFrameSource {
    target: AuthorizedTarget,
    process: AuthorizedProcess,
    window: Window,
    bounds: DesktopRect,
}

impl WindowsGraphicsFrameSource {
    pub fn for_region(
        target: AuthorizedTarget,
        process: AuthorizedProcess,
        selection: DesktopRect,
    ) -> Result<Self, AcquisitionError> {
        if process_started_at(process.process_id()) != Some(process.started_at()) {
            return Err(AcquisitionError::TargetMismatch);
        }
        set_physical_pixel_awareness();
        let window = window_for_region(selection, process.process_id())?;
        let bounds = window_bounds(window)?;
        if bounds.intersection(selection).is_none() {
            return Err(AcquisitionError::TargetMismatch);
        }
        Ok(Self {
            target,
            process,
            window,
            bounds,
        })
    }
}

impl FrameSource for WindowsGraphicsFrameSource {
    fn capture(&mut self, target: &AuthorizedTarget) -> Result<TargetFrame, AcquisitionError> {
        if target != &self.target
            || process_started_at(self.process.process_id()) != Some(self.process.started_at())
        {
            return Err(AcquisitionError::TargetMismatch);
        }
        validate_window_process(self.window, self.process.process_id())?;
        reject_capture_excluded_window(self.window)?;
        let captured = capture_first_frame(self.window)?;
        let expected_width = rect_dimension(self.bounds.left(), self.bounds.right())?;
        let expected_height = rect_dimension(self.bounds.top(), self.bounds.bottom())?;
        if captured.width != expected_width || captured.height != expected_height {
            return Err(AcquisitionError::ProviderUnavailable);
        }
        if protected_or_blank(&captured.rgba8) {
            return Err(AcquisitionError::PermissionDenied);
        }
        TargetFrame::new(self.target.clone(), self.bounds, captured.rgba8)
            .map_err(|_| AcquisitionError::ProviderUnavailable)
    }
}

struct CapturedFrame {
    rgba8: Vec<u8>,
    width: u32,
    height: u32,
}

struct SingleFrameCapture {
    output: Arc<Mutex<Option<CapturedFrame>>>,
}

type CaptureHandlerError = Box<dyn Error + Send + Sync>;

impl GraphicsCaptureApiHandler for SingleFrameCapture {
    type Flags = Arc<Mutex<Option<CapturedFrame>>>;
    type Error = CaptureHandlerError;

    fn new(context: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            output: context.flags,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let width = frame.width();
        let height = frame.height();
        let buffer = frame.buffer()?;
        let mut packed = Vec::new();
        let rgba8 = buffer.as_nopadding_buffer(&mut packed).to_vec();
        *self
            .output
            .lock()
            .map_err(|_| io::Error::other("capture output lock poisoned"))? = Some(CapturedFrame {
            rgba8,
            width,
            height,
        });
        capture_control.stop();
        Ok(())
    }
}

fn capture_first_frame(window: Window) -> Result<CapturedFrame, AcquisitionError> {
    let output = Arc::new(Mutex::new(None));
    let first = SingleFrameCapture::start(capture_settings(
        window,
        CursorCaptureSettings::WithoutCursor,
        Arc::clone(&output),
    ));
    if matches!(
        &first,
        Err(GraphicsCaptureApiError::GraphicsCaptureApiError(
            GraphicsCaptureError::CursorConfigUnsupported
        ))
    ) {
        SingleFrameCapture::start(capture_settings(
            window,
            CursorCaptureSettings::Default,
            Arc::clone(&output),
        ))
        .map_err(|_| AcquisitionError::ProviderUnavailable)?;
    } else {
        first.map_err(|_| AcquisitionError::ProviderUnavailable)?;
    }
    let captured = output
        .lock()
        .map_err(|_| AcquisitionError::ProviderUnavailable)?
        .take()
        .ok_or(AcquisitionError::ProviderUnavailable)?;
    Ok(captured)
}

fn capture_settings(
    window: Window,
    cursor: CursorCaptureSettings,
    output: Arc<Mutex<Option<CapturedFrame>>>,
) -> Settings<Arc<Mutex<Option<CapturedFrame>>>, Window> {
    Settings::new(
        window,
        cursor,
        DrawBorderSettings::Default,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        output,
    )
}

fn window_for_region(selection: DesktopRect, process_id: u32) -> Result<Window, AcquisitionError> {
    Window::enumerate()
        .map_err(|_| AcquisitionError::ProviderUnavailable)?
        .into_iter()
        .filter(|window| window.process_id().ok() == Some(process_id))
        .filter_map(|window| {
            let bounds = window_bounds(window).ok()?;
            intersection_area(bounds, selection).map(|area| (area, window))
        })
        .max_by_key(|(area, _)| *area)
        .map(|(_, window)| window)
        .ok_or(AcquisitionError::TargetMismatch)
}

fn intersection_area(first: DesktopRect, second: DesktopRect) -> Option<u64> {
    let intersection = first.intersection(second)?;
    let width =
        u64::try_from(i64::from(intersection.right()) - i64::from(intersection.left())).ok()?;
    let height =
        u64::try_from(i64::from(intersection.bottom()) - i64::from(intersection.top())).ok()?;
    width.checked_mul(height)
}

fn validate_window_process(window: Window, expected: u32) -> Result<(), AcquisitionError> {
    let mut process_id = 0;
    unsafe {
        GetWindowThreadProcessId(window.as_raw_hwnd(), &mut process_id);
    }
    if process_id == expected {
        Ok(())
    } else {
        Err(AcquisitionError::TargetMismatch)
    }
}

fn window_bounds(window: Window) -> Result<DesktopRect, AcquisitionError> {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(window.as_raw_hwnd(), &mut rect) } == 0 {
        return Err(AcquisitionError::ProviderUnavailable);
    }
    DesktopRect::new(rect.left, rect.top, rect.right, rect.bottom)
        .map_err(|_| AcquisitionError::ProviderUnavailable)
}

fn reject_capture_excluded_window(window: Window) -> Result<(), AcquisitionError> {
    let mut affinity = WDA_NONE;
    if unsafe { GetWindowDisplayAffinity(window.as_raw_hwnd(), &mut affinity) } != 0
        && affinity != WDA_NONE
    {
        return Err(AcquisitionError::PermissionDenied);
    }
    Ok(())
}

fn set_physical_pixel_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

fn rect_dimension(start: i32, end: i32) -> Result<u32, AcquisitionError> {
    u32::try_from(i64::from(end) - i64::from(start))
        .ok()
        .filter(|value| *value > 0)
        .ok_or(AcquisitionError::ProviderUnavailable)
}

fn protected_or_blank(rgba8: &[u8]) -> bool {
    rgba8.is_empty()
        || rgba8
            .chunks_exact(4)
            .all(|pixel| pixel[3] == 0 || (pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphshift_acquisition::{
        AcquisitionAdapter, AcquisitionRequest, Confidence, Granularity, InteractiveSelection,
        InteractiveTextAcquisition, Provenance, SourcePolicy,
    };
    use glyphshift_adapter_ocr::{
        OcrEngine, OcrInputFrame, OcrLocalRect, OcrTextBlock, VisualOcrAcquisitionAdapter,
    };
    use glyphshift_worker_process_grant::{
        authorize_process_target, WINDOWS_PROCESS_GRANT_PLATFORM,
    };
    use std::path::PathBuf;
    use std::time::Instant;

    use crate::TesseractEngine;

    struct PixelProbe;

    impl OcrEngine for PixelProbe {
        fn recognize(
            &mut self,
            frame: &OcrInputFrame,
        ) -> Result<Vec<OcrTextBlock>, AcquisitionError> {
            assert!(frame.width() > 0);
            assert!(frame.height() > 0);
            assert!(!protected_or_blank(frame.rgba8()));
            let bounds = OcrLocalRect::new(
                0,
                0,
                u32::try_from(frame.width()).expect("frame width"),
                u32::try_from(frame.height()).expect("frame height"),
            )
            .expect("frame bounds");
            Ok(vec![OcrTextBlock::new("captured", bounds).with_confidence(
                Confidence::new(10_000).expect("confidence"),
            )])
        }
    }

    #[test]
    fn blank_and_transparent_frames_fail_closed() {
        assert!(protected_or_blank(&[0, 0, 0, 255, 0, 0, 0, 255]));
        assert!(protected_or_blank(&[255, 255, 255, 0]));
        assert!(!protected_or_blank(&[1, 0, 0, 255]));
    }

    #[test]
    #[ignore = "requires an explicitly authorized running Windows target process"]
    fn authorized_window_produces_an_ephemeral_frame_for_the_visual_adapter() {
        let (target, bounds, frames) = authorized_frame_source();
        let adapter = VisualOcrAcquisitionAdapter::new(frames, PixelProbe);
        let mut acquisition =
            InteractiveTextAcquisition::new([Box::new(adapter) as Box<dyn AcquisitionAdapter>]);
        let request = AcquisitionRequest::new(
            target,
            InteractiveSelection::Region(bounds),
            SourcePolicy::VisualOnly,
        );

        let result = acquisition.acquire(&request).expect("visual acquisition");

        assert_eq!(result.blocks().len(), 1);
        assert_eq!(result.blocks()[0].source(), "captured");
        assert_eq!(result.blocks()[0].granularity(), Granularity::Region);
        assert_eq!(result.blocks()[0].provenance(), Provenance::Visual);
        assert_eq!(result.blocks()[0].anchors(), [bounds]);
    }

    #[test]
    #[ignore = "requires an authorized target and local verified Tesseract artifacts"]
    fn authorized_window_runs_the_wgc_tesseract_candidate_in_memory() {
        let library = PathBuf::from(
            std::env::var_os("GLYPHSHIFT_OCR_TESSERACT_DLL").expect("Tesseract library artifact"),
        );
        let data = PathBuf::from(
            std::env::var_os("GLYPHSHIFT_OCR_TESSDATA").expect("Tesseract data artifact"),
        );
        let init_started = Instant::now();
        let engine =
            TesseractEngine::load(library, data, "chi_sim+eng").expect("Tesseract candidate");
        let init_millis = init_started.elapsed().as_millis();
        let (target, bounds, frames) = authorized_frame_source();
        let adapter = VisualOcrAcquisitionAdapter::new(frames, engine);
        let mut acquisition =
            InteractiveTextAcquisition::new([Box::new(adapter) as Box<dyn AcquisitionAdapter>]);
        let request = AcquisitionRequest::new(
            target,
            InteractiveSelection::Region(bounds),
            SourcePolicy::VisualOnly,
        );

        let first_started = Instant::now();
        let first = acquisition.acquire(&request).expect("first acquisition");
        let first_millis = first_started.elapsed().as_millis();
        let hot_started = Instant::now();
        let hot = acquisition.acquire(&request).expect("hot acquisition");
        let hot_millis = hot_started.elapsed().as_millis();

        assert_valid_tesseract_result(&first, bounds);
        assert_valid_tesseract_result(&hot, bounds);
        eprintln!(
            "OCR_INIT_MILLIS={init_millis} OCR_FIRST_BLOCKS={} OCR_FIRST_MILLIS={first_millis} OCR_HOT_BLOCKS={} OCR_HOT_MILLIS={hot_millis}",
            first.blocks().len(),
            hot.blocks().len(),
        );
    }

    fn assert_valid_tesseract_result(
        result: &glyphshift_acquisition::AcquisitionResult,
        bounds: DesktopRect,
    ) {
        assert!(!result.blocks().is_empty());
        assert!(result.blocks().len() <= 256);
        assert!(result.blocks().iter().all(|block| {
            block.provenance() == Provenance::Visual
                && !block.source().trim().is_empty()
                && block
                    .anchors()
                    .iter()
                    .all(|anchor| bounds.intersection(*anchor) == Some(*anchor))
        }));
    }

    fn authorized_frame_source() -> (AuthorizedTarget, DesktopRect, WindowsGraphicsFrameSource) {
        let process_id = std::env::var("GLYPHSHIFT_OCR_TARGET_PID")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value > 0)
            .expect("authorized OCR target process");
        let started_at = process_started_at(process_id).expect("target start time");
        let process = authorize_process_target(
            "windows.fixture.ocr",
            "windows.fixture.ocr",
            WINDOWS_PROCESS_GRANT_PLATFORM,
            &format!("{process_id}:{started_at}"),
        )
        .expect("authorized process grant");
        let window = Window::enumerate()
            .expect("enumerate windows")
            .into_iter()
            .find(|window| window.process_id().ok() == Some(process_id))
            .expect("authorized top-level window");
        let bounds = window_bounds(window).expect("authorized window bounds");
        let target = AuthorizedTarget::new("authorized-window").expect("target");
        let frames = WindowsGraphicsFrameSource::for_region(target.clone(), process, bounds)
            .expect("frame source");
        (target, bounds, frames)
    }
}
