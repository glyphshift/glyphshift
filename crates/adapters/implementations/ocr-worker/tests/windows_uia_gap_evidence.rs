#![cfg(windows)]

use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionError, AcquisitionRequest, AuthorizedTarget, DesktopPoint,
    DesktopRect, InteractiveSelection, InteractiveTextAcquisition, SourcePolicy,
};
use glyphshift_acquisition_worker_sdk::{
    decode_request, encode_request, AcquisitionWorker, WorkerTargetGrant,
};
use glyphshift_adapter_ocr::VisualOcrAcquisitionAdapter;
use glyphshift_adapter_ocr_worker::{
    TesseractEngine, WindowsGraphicsFrameSource, ACQUISITION_ADAPTER_ID as OCR_ADAPTER_ID,
};
use glyphshift_adapter_uia::ACQUISITION_ADAPTER_ID as UIA_ADAPTER_ID;
use glyphshift_adapter_uia_worker::WindowsUiaAcquisitionWorker;
use glyphshift_worker_process_grant::{
    authorize_process_target, process_started_at, WINDOWS_PROCESS_GRANT_PLATFORM,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use windows_capture::window::Window;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

const MAX_UIA_PROBES: usize = 64;

#[test]
#[ignore = "requires an authorized visible target and local verified Tesseract artifacts"]
fn visible_ocr_blocks_include_a_point_with_no_structured_uia_text() {
    let process_id = required_process_id();
    let started_at = process_started_at(process_id).expect("target start time");
    let grant_payload = format!("{process_id}:{started_at}");
    let process = authorize_process_target(
        OCR_ADAPTER_ID,
        OCR_ADAPTER_ID,
        WINDOWS_PROCESS_GRANT_PLATFORM,
        &grant_payload,
    )
    .expect("authorized OCR gap target");
    let bounds = largest_window_bounds(process_id);
    let target = AuthorizedTarget::new("authorized-gap-target").expect("target");
    let frames = WindowsGraphicsFrameSource::for_region(target.clone(), process, bounds)
        .expect("authorized target frame source");
    let engine = TesseractEngine::load(
        required_path("GLYPHSHIFT_OCR_TESSERACT_DLL"),
        required_path("GLYPHSHIFT_OCR_TESSDATA"),
        "chi_sim+eng",
    )
    .expect("Tesseract candidate");
    let adapter = VisualOcrAcquisitionAdapter::new(frames, engine);
    let mut acquisition =
        InteractiveTextAcquisition::new([Box::new(adapter) as Box<dyn AcquisitionAdapter>]);
    let request = AcquisitionRequest::new(
        target,
        InteractiveSelection::Region(bounds),
        SourcePolicy::VisualOnly,
    );
    let visual = acquisition.acquire(&request).expect("visual OCR evidence");
    let (expected_terms, matched_terms) = expected_term_evidence(&visual);

    let grant = WorkerTargetGrant::new(WINDOWS_PROCESS_GRANT_PLATFORM, grant_payload)
        .expect("UIA target grant");
    let mut probed = BTreeSet::new();
    let mut structured = 0_usize;
    let mut no_text = 0_usize;
    let mut other_errors = 0_usize;
    for point in visual
        .blocks()
        .iter()
        .flat_map(|block| block.anchors().iter().copied())
        .filter_map(rect_center)
    {
        if probed.len() >= MAX_UIA_PROBES {
            break;
        }
        if !probed.insert((point.x(), point.y())) {
            continue;
        }
        match acquire_uia_point(&grant, point) {
            Ok(_) => structured += 1,
            Err(AcquisitionError::NoText) => no_text += 1,
            Err(_) => other_errors += 1,
        }
    }

    eprintln!(
        "OCR_GAP_VISUAL_BLOCKS={} OCR_GAP_EXPECTED_TERMS={expected_terms} OCR_GAP_MATCHED_TERMS={matched_terms} OCR_GAP_UIA_PROBES={} OCR_GAP_UIA_STRUCTURED={} OCR_GAP_UIA_NO_TEXT={no_text} OCR_GAP_UIA_OTHER_ERRORS={other_errors}",
        visual.blocks().len(),
        probed.len(),
        structured,
    );
    assert!(
        no_text > 0,
        "authorized target did not expose a strict UIA no-text point at sampled OCR anchors"
    );
}

fn expected_term_evidence(result: &glyphshift_acquisition::AcquisitionResult) -> (usize, usize) {
    let Some(expected) = std::env::var_os("GLYPHSHIFT_OCR_GAP_EXPECTED_TERMS") else {
        return (0, 0);
    };
    let expected = expected
        .to_string_lossy()
        .split('|')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    let recognized = result
        .blocks()
        .iter()
        .map(|block| block.source())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let matched = expected
        .iter()
        .filter(|term| recognized.contains(term.as_str()))
        .count();
    assert_eq!(
        matched,
        expected.len(),
        "authorized OCR target did not contain every expected term"
    );
    (expected.len(), matched)
}

fn acquire_uia_point(
    grant: &WorkerTargetGrant,
    point: DesktopPoint,
) -> Result<glyphshift_acquisition::AcquisitionResult, AcquisitionError> {
    let target = AuthorizedTarget::new("authorized-gap-target").expect("target");
    let request = AcquisitionRequest::new(
        target,
        InteractiveSelection::Point(point),
        SourcePolicy::StructuredOnly,
    );
    let encoded = encode_request(1, UIA_ADAPTER_ID, grant, &request).expect("encode UIA request");
    let (_, worker_request) = decode_request(&encoded).expect("decode UIA request");
    WindowsUiaAcquisitionWorker.acquire(&worker_request)
}

fn rect_center(rect: DesktopRect) -> Option<DesktopPoint> {
    let x = i64::from(rect.left()) + (i64::from(rect.right()) - i64::from(rect.left())) / 2;
    let y = i64::from(rect.top()) + (i64::from(rect.bottom()) - i64::from(rect.top())) / 2;
    Some(DesktopPoint::new(
        i32::try_from(x).ok()?,
        i32::try_from(y).ok()?,
    ))
}

fn largest_window_bounds(process_id: u32) -> DesktopRect {
    Window::enumerate()
        .expect("enumerate target windows")
        .into_iter()
        .filter(|window| window.process_id().ok() == Some(process_id))
        .filter_map(window_bounds)
        .max_by_key(|bounds| {
            let width = i64::from(bounds.right()) - i64::from(bounds.left());
            let height = i64::from(bounds.bottom()) - i64::from(bounds.top());
            width.saturating_mul(height)
        })
        .expect("authorized visible target window")
}

fn window_bounds(window: Window) -> Option<DesktopRect> {
    let mut rect = RECT::default();
    (unsafe { GetWindowRect(window.as_raw_hwnd(), &mut rect) } != 0)
        .then(|| DesktopRect::new(rect.left, rect.top, rect.right, rect.bottom).ok())
        .flatten()
}

fn required_process_id() -> u32 {
    std::env::var("GLYPHSHIFT_OCR_GAP_TARGET_PID")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .expect("authorized OCR/UIA gap target process")
}

fn required_path(variable: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(variable).unwrap_or_else(|| panic!("missing {variable}")))
}
