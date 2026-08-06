use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionError, AcquisitionRequest, AuthorizedTarget, DesktopPoint,
    DesktopRect, Granularity, InteractiveSelection, InteractiveTextAcquisition, Provenance,
    SourcePolicy,
};
use glyphshift_adapter_uia::{
    UiaAcquisitionAdapter, UiaAcquisitionSnapshot, UiaSelectionSource, UiaTextSelection,
};

struct FixtureSource {
    outcome: Result<UiaAcquisitionSnapshot, AcquisitionError>,
}

impl UiaSelectionSource for FixtureSource {
    fn snapshot(
        &mut self,
        _target: &AuthorizedTarget,
        _selection: InteractiveSelection,
    ) -> Result<UiaAcquisitionSnapshot, AcquisitionError> {
        self.outcome.clone()
    }
}

fn target(value: &str) -> AuthorizedTarget {
    AuthorizedTarget::new(value).expect("authorized target fixture")
}

fn rect(left: i32, top: i32, right: i32, bottom: i32) -> DesktopRect {
    DesktopRect::new(left, top, right, bottom).expect("valid fixture rectangle")
}

fn acquire(
    selection: InteractiveSelection,
    snapshot: UiaAcquisitionSnapshot,
) -> Result<glyphshift_acquisition::AcquisitionResult, AcquisitionError> {
    let adapter = UiaAcquisitionAdapter::new(FixtureSource {
        outcome: Ok(snapshot),
    });
    let mut acquisition =
        InteractiveTextAcquisition::new([Box::new(adapter) as Box<dyn AcquisitionAdapter>]);
    acquisition.acquire(&AcquisitionRequest::new(
        target("target-a"),
        selection,
        SourcePolicy::StructuredOnly,
    ))
}

#[test]
fn uia_acquisition_001_point_returns_a_word_and_its_virtual_desktop_anchor() {
    let word_anchor = rect(-310, 120, -246, 148);
    let result = acquire(
        InteractiveSelection::Point(DesktopPoint::new(-280, 132)),
        UiaAcquisitionSnapshot::new(target("target-a")).with_text(
            "GlyphShift",
            [word_anchor],
            UiaTextSelection::Word,
        ),
    )
    .expect("point text");

    assert_eq!(result.blocks().len(), 1);
    assert_eq!(result.blocks()[0].source(), "GlyphShift");
    assert_eq!(result.blocks()[0].anchors(), &[word_anchor]);
    assert_eq!(result.blocks()[0].granularity(), Granularity::Word);
    assert_eq!(result.blocks()[0].provenance(), Provenance::Structured);
}

#[test]
fn uia_acquisition_002_text_range_preserves_multiple_display_line_anchors() {
    let first_line = rect(200, 300, 410, 326);
    let second_line = rect(200, 326, 355, 352);
    let result = acquire(
        InteractiveSelection::TextRange {
            start: DesktopPoint::new(205, 304),
            end: DesktopPoint::new(350, 348),
        },
        UiaAcquisitionSnapshot::new(target("target-a")).with_text(
            "first line\nsecond line",
            [first_line, second_line],
            UiaTextSelection::TextRange,
        ),
    )
    .expect("text range");

    assert_eq!(result.blocks()[0].source(), "first line\nsecond line");
    assert_eq!(result.blocks()[0].anchors(), &[first_line, second_line]);
    assert_eq!(result.blocks()[0].granularity(), Granularity::Line);
}

#[test]
fn uia_acquisition_003_name_only_element_degrades_explicitly_to_control() {
    let control_anchor = rect(40, 50, 180, 86);
    let result = acquire(
        InteractiveSelection::Point(DesktopPoint::new(70, 66)),
        UiaAcquisitionSnapshot::new(target("target-a")).with_name("Open settings", control_anchor),
    )
    .expect("control name");

    assert_eq!(result.blocks()[0].source(), "Open settings");
    assert_eq!(result.blocks()[0].anchors(), &[control_anchor]);
    assert_eq!(result.blocks()[0].granularity(), Granularity::Control);
}

#[test]
fn uia_acquisition_004_password_and_cross_target_snapshots_fail_closed() {
    let password = acquire(
        InteractiveSelection::Point(DesktopPoint::new(1, 1)),
        UiaAcquisitionSnapshot::new(target("target-a"))
            .password(true)
            .with_text("secret", [rect(0, 0, 20, 20)], UiaTextSelection::Word),
    );
    let cross_target = acquire(
        InteractiveSelection::Point(DesktopPoint::new(1, 1)),
        UiaAcquisitionSnapshot::new(target("target-b"))
            .with_name("other process", rect(0, 0, 20, 20)),
    );

    assert_eq!(password, Err(AcquisitionError::PermissionDenied));
    assert_eq!(cross_target, Err(AcquisitionError::TargetMismatch));
}

#[test]
fn uia_acquisition_005_region_is_not_silently_promoted_to_a_point_query() {
    let result = acquire(
        InteractiveSelection::Region(rect(0, 0, 100, 100)),
        UiaAcquisitionSnapshot::new(target("target-a")).with_name("ignored", rect(0, 0, 100, 100)),
    );

    assert_eq!(result, Err(AcquisitionError::ProviderUnavailable));
}

#[test]
fn uia_acquisition_006_text_range_without_text_pattern_does_not_degrade_to_a_control() {
    let result = acquire(
        InteractiveSelection::TextRange {
            start: DesktopPoint::new(10, 10),
            end: DesktopPoint::new(80, 20),
        },
        UiaAcquisitionSnapshot::new(target("target-a"))
            .with_name("whole control", rect(0, 0, 100, 40)),
    );

    assert_eq!(result, Err(AcquisitionError::NoText));
}

#[test]
fn uia_acquisition_007_rejects_a_point_snapshot_with_stale_geometry() {
    let stale_point = acquire(
        InteractiveSelection::Point(DesktopPoint::new(200, 200)),
        UiaAcquisitionSnapshot::new(target("target-a")).with_text(
            "stale word",
            [rect(0, 0, 40, 20)],
            UiaTextSelection::Word,
        ),
    );
    assert_eq!(stale_point, Err(AcquisitionError::NoText));
}
