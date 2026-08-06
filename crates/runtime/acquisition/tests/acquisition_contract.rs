use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionCandidate, AcquisitionError, AcquisitionRequest,
    AuthorizedTarget, DesktopPoint, DesktopRect, Granularity, InteractiveSelection,
    InteractiveTextAcquisition, Provenance, SourcePolicy,
};

struct FixtureAdapter {
    provenance: Provenance,
    outcome: Result<Vec<AcquisitionCandidate>, AcquisitionError>,
}

impl AcquisitionAdapter for FixtureAdapter {
    fn provenance(&self) -> Provenance {
        self.provenance
    }

    fn acquire(
        &mut self,
        _request: &AcquisitionRequest,
    ) -> Result<Vec<AcquisitionCandidate>, AcquisitionError> {
        self.outcome.clone()
    }
}

fn target() -> AuthorizedTarget {
    AuthorizedTarget::new("opaque-target").expect("authorized target")
}

fn rect(left: i32, top: i32, right: i32, bottom: i32) -> DesktopRect {
    DesktopRect::new(left, top, right, bottom).expect("valid fixture rectangle")
}

fn request(policy: SourcePolicy) -> AcquisitionRequest {
    AcquisitionRequest::new(
        target(),
        InteractiveSelection::Point(DesktopPoint::new(12, 18)),
        policy,
    )
}

#[test]
fn acquisition_001_normalizes_deduplicates_and_prefers_structured_over_visual() {
    let first_anchor = rect(10, 10, 30, 30);
    let second_anchor = rect(30, 10, 50, 30);
    let duplicate = AcquisitionCandidate::new(
        "  hello\r\nworld  ",
        [first_anchor, second_anchor, first_anchor],
        Granularity::Word,
    );
    let mut acquisition = InteractiveTextAcquisition::new([
        Box::new(FixtureAdapter {
            provenance: Provenance::Visual,
            outcome: Ok(vec![AcquisitionCandidate::new(
                "visual",
                [rect(10, 10, 30, 30)],
                Granularity::Region,
            )]),
        }) as Box<dyn AcquisitionAdapter>,
        Box::new(FixtureAdapter {
            provenance: Provenance::Structured,
            outcome: Ok(vec![duplicate.clone(), duplicate]),
        }),
    ]);

    let result = acquisition
        .acquire(&request(SourcePolicy::Automatic))
        .expect("partial acquisition");

    assert_eq!(result.blocks().len(), 1);
    assert_eq!(result.blocks()[0].source(), "hello\nworld");
    assert_eq!(result.blocks()[0].anchors(), &[first_anchor, second_anchor]);
    assert_eq!(result.blocks()[0].provenance(), Provenance::Structured);
}

#[test]
fn acquisition_002_preserves_success_when_a_peer_structured_provider_fails() {
    let mut acquisition = InteractiveTextAcquisition::new([
        Box::new(FixtureAdapter {
            provenance: Provenance::Structured,
            outcome: Ok(vec![AcquisitionCandidate::new(
                "public text",
                [rect(1, 2, 20, 12)],
                Granularity::Control,
            )]),
        }) as Box<dyn AcquisitionAdapter>,
        Box::new(FixtureAdapter {
            provenance: Provenance::Structured,
            outcome: Err(AcquisitionError::TimedOut),
        }),
    ]);

    let result = acquisition
        .acquire(&request(SourcePolicy::Automatic))
        .expect("structured partial success");

    assert_eq!(result.blocks().len(), 1);
    assert_eq!(result.blocks()[0].source(), "public text");
}

#[test]
fn acquisition_003_automatic_uses_visual_only_as_a_structured_fallback() {
    let mut acquisition = InteractiveTextAcquisition::new([
        Box::new(FixtureAdapter {
            provenance: Provenance::Structured,
            outcome: Err(AcquisitionError::NoText),
        }) as Box<dyn AcquisitionAdapter>,
        Box::new(FixtureAdapter {
            provenance: Provenance::Visual,
            outcome: Ok(vec![AcquisitionCandidate::new(
                "visual fallback",
                [rect(1, 1, 20, 20)],
                Granularity::Region,
            )]),
        }),
    ]);

    let result = acquisition
        .acquire(&request(SourcePolicy::Automatic))
        .expect("visual fallback");

    assert_eq!(result.blocks().len(), 1);
    assert_eq!(result.blocks()[0].source(), "visual fallback");
    assert_eq!(result.blocks()[0].provenance(), Provenance::Visual);
}

#[test]
fn acquisition_004_source_policy_does_not_invoke_an_unavailable_class() {
    let mut acquisition = InteractiveTextAcquisition::new([Box::new(FixtureAdapter {
        provenance: Provenance::Visual,
        outcome: Ok(vec![AcquisitionCandidate::new(
            "visual",
            [rect(1, 1, 2, 2)],
            Granularity::Region,
        )]),
    }) as Box<dyn AcquisitionAdapter>]);

    assert_eq!(
        acquisition.acquire(&request(SourcePolicy::StructuredOnly)),
        Err(AcquisitionError::ProviderUnavailable)
    );
}

#[test]
fn acquisition_005_permission_denied_stops_automatic_visual_fallback() {
    let mut acquisition = InteractiveTextAcquisition::new([
        Box::new(FixtureAdapter {
            provenance: Provenance::Structured,
            outcome: Err(AcquisitionError::PermissionDenied),
        }) as Box<dyn AcquisitionAdapter>,
        Box::new(FixtureAdapter {
            provenance: Provenance::Visual,
            outcome: Ok(vec![AcquisitionCandidate::new(
                "must not bypass privacy refusal",
                [rect(1, 1, 20, 20)],
                Granularity::Region,
            )]),
        }),
    ]);

    assert_eq!(
        acquisition.acquire(&request(SourcePolicy::Automatic)),
        Err(AcquisitionError::PermissionDenied)
    );
}
