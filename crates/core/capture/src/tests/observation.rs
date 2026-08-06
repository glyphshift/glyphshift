use crate::*;

#[test]
fn observation_batch_round_trips_generation_sequence_gap_and_drop_evidence() {
    let batch = CaptureObservationBatch::new(
        CaptureProducerId::new("target-1").expect("producer id"),
        7,
        1,
        [
            CaptureObservationRecord::new(4, "windows.gdi.text-out", "Open")
                .expect("first observation"),
            CaptureObservationRecord::new(6, "windows.console.write-console", "File")
                .expect("second observation"),
        ],
    )
    .expect("observation batch");

    let encoded = batch.encode_json().expect("batch json");
    assert_eq!(
        CaptureObservationBatch::decode_json(&encoded).expect("decoded batch"),
        batch
    );
    assert_eq!(batch.producer_id().as_str(), "target-1");
    assert_eq!(batch.generation(), 7);
    assert_eq!(batch.dropped_total(), 1);
    assert_eq!(batch.records()[0].sequence(), 4);
    assert!(!encoded.contains("outputPath"));
    assert!(!encoded.contains("translation"));
}
#[test]
fn observation_batch_rejects_unknown_unbounded_or_ambiguous_input() {
    let producer = || CaptureProducerId::new("worker-1").expect("producer id");
    let record = |sequence| {
        CaptureObservationRecord::new(sequence, "windows.uia.observe", "Name").expect("observation")
    };

    assert_eq!(
        CaptureProducerId::new("target:1"),
        Err(CaptureError::InvalidObservationBatch)
    );
    assert_eq!(
        CaptureObservationRecord::new(1, "windows.uia.observe", "x".repeat(MAX_SOURCE_UNITS + 1),),
        Err(CaptureError::InvalidObservationBatch)
    );
    assert_eq!(
        CaptureObservationBatch::new(producer(), 0, 0, [record(1)]),
        Err(CaptureError::InvalidObservationBatch)
    );
    assert_eq!(
        CaptureObservationBatch::new(producer(), 1, 0, [record(2), record(2)]),
        Err(CaptureError::InvalidObservationBatch)
    );
    assert_eq!(
        CaptureObservationBatch::new(
            producer(),
            1,
            0,
            (1..=MAX_OBSERVATION_BATCH_RECORDS + 1).map(|sequence| record(sequence as u64)),
        ),
        Err(CaptureError::InvalidObservationBatch)
    );

    let valid = CaptureObservationBatch::new(producer(), 1, 0, [record(1)])
        .expect("valid batch")
        .encode_json()
        .expect("valid json");
    let unknown = valid.replacen("\"records\"", "\"unknown\":true,\"records\"", 1);
    assert_eq!(
        CaptureObservationBatch::decode_json(&unknown),
        Err(CaptureError::InvalidObservationBatch)
    );
    assert_eq!(
        CaptureObservationBatch::decode_json(&"x".repeat(MAX_OBSERVATION_BATCH_BYTES + 1)),
        Err(CaptureError::InvalidObservationBatch)
    );
}
#[test]
fn observation_cursor_rejects_replay_swaps_and_unexplained_sequence_gaps() {
    let configuration = CaptureProducerConfiguration::new(
        CaptureProducerId::new("target-1").expect("producer id"),
        7,
    )
    .expect("producer configuration");
    let mut cursor = CaptureObservationCursor::new(&configuration);
    let batch = |producer: &str, generation, dropped, sequence| {
        CaptureObservationBatch::new(
            CaptureProducerId::new(producer).expect("batch producer id"),
            generation,
            dropped,
            [
                CaptureObservationRecord::new(sequence, "windows.gdi.text-out", "Open")
                    .expect("observation"),
            ],
        )
        .expect("observation batch")
    };

    assert_eq!(cursor.accept(&batch("target-1", 7, 0, 1)), Ok(0));
    assert_eq!(cursor.accept(&batch("target-1", 7, 1, 3)), Ok(1));
    assert_eq!(
        cursor.accept(&batch("target-1", 7, 1, 3)),
        Err(CaptureError::InvalidObservationBatch)
    );
    assert_eq!(
        cursor.accept(&batch("target-2", 7, 2, 4)),
        Err(CaptureError::InvalidObservationBatch)
    );
    assert_eq!(
        cursor.accept(&batch("target-1", 8, 2, 4)),
        Err(CaptureError::InvalidObservationBatch)
    );

    let mut unexplained = CaptureObservationCursor::new(&configuration);
    assert_eq!(
        unexplained.accept(&batch("target-1", 7, 0, 2)),
        Err(CaptureError::InvalidObservationBatch)
    );
}
