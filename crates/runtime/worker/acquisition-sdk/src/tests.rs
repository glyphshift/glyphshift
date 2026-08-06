use super::*;
use glyphshift_acquisition::AuthorizedTarget;
use std::io::Cursor;

struct FixtureAdapter {
    provenance: Provenance,
    candidates: Vec<AcquisitionCandidate>,
}

impl AcquisitionAdapter for FixtureAdapter {
    fn provenance(&self) -> Provenance {
        self.provenance
    }

    fn acquire(
        &mut self,
        _request: &AcquisitionRequest,
    ) -> Result<Vec<AcquisitionCandidate>, AcquisitionError> {
        Ok(self.candidates.clone())
    }
}

struct FixtureWorker;

impl AcquisitionWorker for FixtureWorker {
    fn acquire(
        &mut self,
        request: &WorkerAcquisitionRequest,
    ) -> Result<AcquisitionResult, AcquisitionError> {
        let target = AuthorizedTarget::new("worker-local-target")?;
        let request = AcquisitionRequest::new(target, request.selection(), request.source_policy());
        let anchor = match request.selection() {
            InteractiveSelection::Point(point) => {
                DesktopRect::new(point.x() - 1, point.y() - 1, point.x() + 2, point.y() + 2)
                    .map_err(|_| AcquisitionError::NoText)?
            }
            InteractiveSelection::TextRange { start, end } => DesktopRect::new(
                start.x().min(end.x()),
                start.y().min(end.y()),
                start.x().max(end.x()) + 1,
                start.y().max(end.y()) + 1,
            )
            .map_err(|_| AcquisitionError::NoText)?,
            InteractiveSelection::Region(rect) => rect,
        };
        let adapter = FixtureAdapter {
            provenance: Provenance::Structured,
            candidates: vec![AcquisitionCandidate::new(
                "worker text",
                [anchor],
                Granularity::Word,
            )],
        };
        InteractiveTextAcquisition::new([Box::new(adapter) as Box<dyn AcquisitionAdapter>])
            .acquire(&request)
    }
}

fn target() -> AuthorizedTarget {
    AuthorizedTarget::new("desktop-private-target").expect("target")
}

fn grant() -> WorkerTargetGrant {
    WorkerTargetGrant::new("synthetic-process-v1", "opaque-grant-payload").expect("grant")
}

fn request() -> AcquisitionRequest {
    AcquisitionRequest::new(
        target(),
        InteractiveSelection::Point(DesktopPoint::new(-20, 30)),
        SourcePolicy::StructuredOnly,
    )
}

fn result(request: &AcquisitionRequest) -> AcquisitionResult {
    let adapter = FixtureAdapter {
        provenance: Provenance::Structured,
        candidates: vec![AcquisitionCandidate::new(
            "round trip",
            [DesktopRect::new(-24, 25, 12, 50).expect("anchor")],
            Granularity::Word,
        )
        .with_confidence(Confidence::new(9_200).expect("confidence"))],
    };
    InteractiveTextAcquisition::new([Box::new(adapter) as Box<dyn AcquisitionAdapter>])
        .acquire(request)
        .expect("fixture result")
}

#[test]
fn wire_001_request_round_trip_omits_the_desktop_target_token() {
    let request = request();
    let encoded = encode_request(7, "windows.uia.acquire", &grant(), &request).expect("encode");

    assert!(!encoded.contains(request.target().as_str()));
    let (request_id, decoded) = decode_request(&encoded).expect("decode");
    assert_eq!(request_id, 7);
    assert_eq!(decoded.adapter_id(), "windows.uia.acquire");
    assert!(decoded.target_grant() == &grant());
    assert_eq!(decoded.selection(), request.selection());
    assert_eq!(decoded.source_policy(), SourcePolicy::StructuredOnly);

    for selection in [
        InteractiveSelection::TextRange {
            start: DesktopPoint::new(-5, 6),
            end: DesktopPoint::new(10, 20),
        },
        InteractiveSelection::Region(DesktopRect::new(-100, -50, 200, 300).expect("region")),
    ] {
        let request = AcquisitionRequest::new(target(), selection, SourcePolicy::Automatic);
        let encoded = encode_request(8, "synthetic.acquire", &grant(), &request).expect("encode");
        assert_eq!(
            decode_request(&encoded).expect("decode").1.selection(),
            selection
        );
    }
}

#[test]
fn wire_002_result_round_trip_preserves_negative_anchors_and_confidence() {
    let request = request();
    let result = result(&request);
    let encoded = encode_response(9, Ok(&result)).expect("encode response");
    let decoded = decode_response(&encoded, 9, &request)
        .expect("decode response")
        .expect("acquired");

    assert_eq!(decoded, result);
}

#[test]
fn wire_003_stable_failures_round_trip_without_provider_strings() {
    for error in [
        AcquisitionError::TargetMismatch,
        AcquisitionError::PermissionDenied,
        AcquisitionError::NoText,
        AcquisitionError::ProviderUnavailable,
        AcquisitionError::TimedOut,
        AcquisitionError::Cancelled,
    ] {
        let encoded = encode_response(3, Err(error)).expect("encode error");
        assert_eq!(
            decode_response(&encoded, 3, &request()).expect("decode error"),
            Err(error)
        );
    }
}

#[test]
fn wire_004_unknown_fields_invalid_rectangles_and_oversize_messages_are_rejected() {
    let encoded = encode_request(1, "synthetic.acquire", &grant(), &request()).expect("request");
    let mut value: serde_json::Value = serde_json::from_str(&encoded).expect("request JSON");
    value
        .as_object_mut()
        .expect("request object")
        .insert("unknown".into(), true.into());
    assert!(matches!(
        decode_request(&value.to_string()),
        Err(WireError::InvalidMessage)
    ));

    let invalid_rect = encoded.replace(
        r#"{"kind":"point","x":-20,"y":30}"#,
        r#"{"kind":"region","left":1,"top":1,"right":1,"bottom":2}"#,
    );
    assert!(matches!(
        decode_request(&invalid_rect),
        Err(WireError::InvalidMessage)
    ));
    assert!(matches!(
        decode_request(&"x".repeat(MAX_WIRE_BYTES + 1)),
        Err(WireError::LimitExceeded)
    ));
}

#[test]
fn wire_005_serve_processes_exactly_one_bounded_request() {
    let request = request();
    let encoded = encode_request(11, "synthetic.acquire", &grant(), &request).expect("request");
    let input = Cursor::new(format!("{encoded}\nignored second line\n"));
    let mut output = Vec::new();

    serve(FixtureWorker, input, &mut output).expect("serve");

    let output = String::from_utf8(output).expect("response UTF-8");
    assert_eq!(output.lines().count(), 1);
    let result = decode_response(output.lines().next().expect("response line"), 11, &request)
        .expect("response")
        .expect("acquired");
    assert_eq!(result.blocks()[0].source(), "worker text");
}

#[test]
fn wire_006_result_block_text_anchor_confidence_and_provenance_limits_are_enforced() {
    let anchor = WireRect {
        left: 0,
        top: 0,
        right: 10,
        bottom: 10,
    };
    let block = |source: Box<str>, anchors: Vec<WireRect>, provenance| WireBlock {
        source,
        anchors,
        granularity: WireGranularity::Word,
        provenance,
        confidence: Some(9_000),
    };
    let decode = |blocks| {
        let encoded = encode_limited(&ResponseEnvelope {
            schema: PROTOCOL_SCHEMA.into(),
            request_id: 1,
            response: WireResponse::Acquired { blocks },
        })
        .expect("bounded fixture response");
        decode_response(&encoded, 1, &request())
    };

    let too_many_blocks = (0..=MAX_BLOCKS)
        .map(|_| {
            block(
                "x".into(),
                vec![WireRect {
                    left: anchor.left,
                    top: anchor.top,
                    right: anchor.right,
                    bottom: anchor.bottom,
                }],
                WireProvenance::Structured,
            )
        })
        .collect();
    assert_eq!(decode(too_many_blocks), Err(WireError::LimitExceeded));

    let too_many_anchors = (0..=MAX_ANCHORS_PER_BLOCK)
        .map(|index| WireRect {
            left: index as i32,
            top: 0,
            right: index as i32 + 1,
            bottom: 1,
        })
        .collect();
    assert_eq!(
        decode(vec![block(
            "x".into(),
            too_many_anchors,
            WireProvenance::Structured,
        )]),
        Err(WireError::LimitExceeded)
    );

    assert_eq!(
        decode(vec![block(
            "x".repeat(MAX_SOURCE_UNITS + 1).into(),
            vec![anchor],
            WireProvenance::Structured,
        )]),
        Err(WireError::LimitExceeded)
    );

    let invalid_confidence = WireBlock {
        source: "x".into(),
        anchors: vec![WireRect {
            left: 0,
            top: 0,
            right: 1,
            bottom: 1,
        }],
        granularity: WireGranularity::Word,
        provenance: WireProvenance::Structured,
        confidence: Some(10_001),
    };
    assert_eq!(
        decode(vec![invalid_confidence]),
        Err(WireError::LimitExceeded)
    );

    let mixed = vec![
        block(
            "structured".into(),
            vec![WireRect {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            }],
            WireProvenance::Structured,
        ),
        block(
            "visual".into(),
            vec![WireRect {
                left: 1,
                top: 0,
                right: 2,
                bottom: 1,
            }],
            WireProvenance::Visual,
        ),
    ];
    assert_eq!(decode(mixed), Err(WireError::InvalidMessage));
}
