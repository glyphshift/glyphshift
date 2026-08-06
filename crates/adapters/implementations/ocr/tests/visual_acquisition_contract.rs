use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionError, AcquisitionRequest, AuthorizedTarget, Confidence,
    DesktopPoint, DesktopRect, Granularity, InteractiveSelection, InteractiveTextAcquisition,
    LogicalRect, Provenance, SourcePolicy, SurfaceGeometry,
};
use glyphshift_adapter_ocr::{
    FrameError, FrameSource, OcrEngine, OcrInputFrame, OcrLocalRect, OcrTextBlock, TargetFrame,
    VisualOcrAcquisitionAdapter,
};
use std::cell::RefCell;
use std::rc::Rc;

type SeenFrame = Rc<RefCell<Option<(usize, usize, Vec<u8>)>>>;

struct FixtureFrameSource {
    frame: Option<TargetFrame>,
}

impl FrameSource for FixtureFrameSource {
    fn capture(&mut self, _target: &AuthorizedTarget) -> Result<TargetFrame, AcquisitionError> {
        self.frame
            .take()
            .ok_or(AcquisitionError::ProviderUnavailable)
    }
}

struct FixtureOcr {
    seen: SeenFrame,
    blocks: Vec<OcrTextBlock>,
}

impl OcrEngine for FixtureOcr {
    fn recognize(&mut self, frame: &OcrInputFrame) -> Result<Vec<OcrTextBlock>, AcquisitionError> {
        self.seen.replace(Some((
            frame.width(),
            frame.height(),
            frame.rgba8().to_vec(),
        )));
        Ok(self.blocks.clone())
    }
}

fn target(value: &str) -> AuthorizedTarget {
    AuthorizedTarget::new(value).expect("authorized target fixture")
}

fn rect(left: i32, top: i32, right: i32, bottom: i32) -> DesktopRect {
    DesktopRect::new(left, top, right, bottom).expect("valid desktop fixture")
}

fn local_rect(left: u32, top: u32, right: u32, bottom: u32) -> OcrLocalRect {
    OcrLocalRect::new(left, top, right, bottom).expect("valid OCR fixture")
}

fn rgba_fixture(width: usize, height: usize) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&[x as u8, y as u8, 0, 255]);
        }
    }
    pixels
}

fn acquire(
    request_target: &str,
    selection: DesktopRect,
    frame: TargetFrame,
    ocr: FixtureOcr,
) -> Result<glyphshift_acquisition::AcquisitionResult, AcquisitionError> {
    let adapter = VisualOcrAcquisitionAdapter::new(FixtureFrameSource { frame: Some(frame) }, ocr);
    let mut acquisition =
        InteractiveTextAcquisition::new([Box::new(adapter) as Box<dyn AcquisitionAdapter>]);
    acquisition.acquire(&AcquisitionRequest::new(
        target(request_target),
        InteractiveSelection::Region(selection),
        SourcePolicy::VisualOnly,
    ))
}

#[test]
fn visual_acquisition_001_ocr_sees_only_the_target_frame_intersection() {
    let seen = Rc::new(RefCell::new(None));
    let frame = TargetFrame::new(target("target-a"), rect(-4, -2, 2, 1), rgba_fixture(6, 3))
        .expect("target frame");
    let confidence = Confidence::new(8_500).expect("bounded confidence");

    let result = acquire(
        "target-a",
        rect(-2, -3, 4, 0),
        frame,
        FixtureOcr {
            seen: Rc::clone(&seen),
            blocks: vec![OcrTextBlock::new("cropped text", local_rect(1, 0, 4, 2))
                .with_confidence(confidence)],
        },
    )
    .expect("visual text");

    let mut expected_pixels = Vec::new();
    for y in 0..2 {
        for x in 2..6 {
            expected_pixels.extend_from_slice(&[x, y, 0, 255]);
        }
    }
    assert_eq!(*seen.borrow(), Some((4, 2, expected_pixels)));
    assert_eq!(result.blocks().len(), 1);
    assert_eq!(result.blocks()[0].source(), "cropped text");
    assert_eq!(result.blocks()[0].anchors(), &[rect(-1, -2, 2, 0)]);
    assert_eq!(result.blocks()[0].granularity(), Granularity::Region);
    assert_eq!(result.blocks()[0].provenance(), Provenance::Visual);
    assert_eq!(result.blocks()[0].confidence(), Some(confidence));
}

#[test]
fn visual_acquisition_002_maps_crop_local_blocks_after_per_monitor_dpi_conversion() {
    let geometry =
        SurfaceGeometry::new(DesktopPoint::new(-1600, 120), 144, 144).expect("monitor geometry");
    let frame_bounds = geometry
        .map_rect(LogicalRect::new(0, 0, 200, 100).expect("frame DIPs"))
        .expect("frame pixels");
    let selection = geometry
        .map_rect(LogicalRect::new(20, 10, 120, 50).expect("selection DIPs"))
        .expect("selection pixels");
    let frame = TargetFrame::new(target("target-a"), frame_bounds, rgba_fixture(300, 150))
        .expect("target frame");

    let result = acquire(
        "target-a",
        selection,
        frame,
        FixtureOcr {
            seen: Rc::new(RefCell::new(None)),
            blocks: vec![OcrTextBlock::new("scaled", local_rect(15, 6, 75, 30))],
        },
    )
    .expect("scaled OCR result");

    assert_eq!(
        result.blocks()[0].anchors(),
        &[rect(-1555, 141, -1495, 165)]
    );
}

#[test]
fn visual_acquisition_003_target_mismatch_protected_frame_and_outside_region_fail_closed() {
    let empty_ocr = || FixtureOcr {
        seen: Rc::new(RefCell::new(None)),
        blocks: Vec::new(),
    };
    let cross_target = acquire(
        "target-a",
        rect(0, 0, 10, 10),
        TargetFrame::new(target("target-b"), rect(0, 0, 10, 10), rgba_fixture(10, 10))
            .expect("cross-target frame"),
        empty_ocr(),
    );
    let protected = acquire(
        "target-a",
        rect(0, 0, 10, 10),
        TargetFrame::protected(target("target-a"), rect(0, 0, 10, 10)).expect("protected frame"),
        empty_ocr(),
    );
    let outside = acquire(
        "target-a",
        rect(20, 20, 30, 30),
        TargetFrame::new(target("target-a"), rect(0, 0, 10, 10), rgba_fixture(10, 10))
            .expect("target frame"),
        empty_ocr(),
    );

    assert_eq!(cross_target, Err(AcquisitionError::TargetMismatch));
    assert_eq!(protected, Err(AcquisitionError::PermissionDenied));
    assert_eq!(outside, Err(AcquisitionError::TargetMismatch));
}

#[test]
fn visual_acquisition_004_discards_invalid_ocr_blocks_but_preserves_valid_partial_output() {
    let frame = TargetFrame::new(
        target("target-a"),
        rect(100, 200, 120, 210),
        rgba_fixture(20, 10),
    )
    .expect("target frame");
    let result = acquire(
        "target-a",
        rect(100, 200, 120, 210),
        frame,
        FixtureOcr {
            seen: Rc::new(RefCell::new(None)),
            blocks: vec![
                OcrTextBlock::new("outside", local_rect(0, 0, 21, 2)),
                OcrTextBlock::new("inside", local_rect(2, 3, 8, 7)),
            ],
        },
    )
    .expect("partial OCR output");

    assert_eq!(result.blocks().len(), 1);
    assert_eq!(result.blocks()[0].source(), "inside");
    assert_eq!(result.blocks()[0].anchors(), &[rect(102, 203, 108, 207)]);
}

#[test]
fn visual_acquisition_005_rejects_invalid_frames_and_empty_ocr_output() {
    assert_eq!(
        TargetFrame::new(target("target-a"), rect(0, 0, 2, 2), vec![0; 15]).err(),
        Some(FrameError::InvalidPixelBuffer)
    );
    assert!(Confidence::new(10_001).is_none());

    let result = acquire(
        "target-a",
        rect(0, 0, 2, 2),
        TargetFrame::new(target("target-a"), rect(0, 0, 2, 2), vec![0; 16]).expect("target frame"),
        FixtureOcr {
            seen: Rc::new(RefCell::new(None)),
            blocks: Vec::new(),
        },
    );
    assert_eq!(result, Err(AcquisitionError::NoText));
}
