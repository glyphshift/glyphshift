use crate::*;

#[test]
fn batch_producer_drains_bounded_sorted_batches_without_blocking_ingress() {
    let (mut producer, ingress) = CaptureBatchProducer::start(
        CaptureProducerConfiguration::new(
            CaptureProducerId::new("target-7").expect("producer id"),
            3,
        )
        .expect("producer configuration"),
    )
    .expect("batch producer");
    let second_ingress = ingress.clone();
    let first = std::thread::spawn(move || ingress.try_observe("windows.gdi.text-out", "Open"));
    let second = std::thread::spawn(move || {
        second_ingress.try_observe("windows.console.write-console", "File")
    });
    assert_eq!(
        first.join().expect("first producer"),
        CaptureIngressStatus::Accepted
    );
    assert_eq!(
        second.join().expect("second producer"),
        CaptureIngressStatus::Accepted
    );

    let batch = producer.drain().expect("first batch");
    assert_eq!(batch.producer_id().as_str(), "target-7");
    assert_eq!(batch.generation(), 3);
    assert_eq!(batch.records().len(), 2);
    assert!(batch.records()[0].sequence() < batch.records()[1].sequence());
    assert!(producer.drain().expect("empty batch").records().is_empty());
}
#[test]
fn batch_producer_preserves_pending_records_gaps_pause_and_owner_lifetime() {
    let (mut producer, ingress) = CaptureBatchProducer::start(
        CaptureProducerConfiguration::new(
            CaptureProducerId::new("worker-4").expect("producer id"),
            9,
        )
        .expect("producer configuration"),
    )
    .expect("batch producer");
    for index in 0..300 {
        assert_eq!(
            ingress.try_observe("windows.uia.observe", format!("Source {index}")),
            CaptureIngressStatus::Accepted
        );
    }
    assert_eq!(
        ingress.try_observe("windows.uia.observe", " "),
        CaptureIngressStatus::Dropped
    );
    assert_eq!(
        ingress.try_observe("windows.uia.observe", "After gap"),
        CaptureIngressStatus::Accepted
    );

    let first = producer.drain().expect("bounded first batch");
    let second = producer.drain().expect("pending second batch");
    assert_eq!(first.records().len(), MAX_OBSERVATION_BATCH_RECORDS);
    assert_eq!(second.records().len(), 45);
    assert!(
        first.records().last().expect("first tail").sequence()
            < second.records().first().expect("second head").sequence()
    );
    assert_eq!(second.dropped_total(), 1);
    assert!(second
        .records()
        .windows(2)
        .any(|records| records[1].sequence() > records[0].sequence() + 1));

    producer.set_paused(true);
    assert_eq!(
        ingress.try_observe("windows.uia.observe", "Paused"),
        CaptureIngressStatus::Paused
    );
    producer.set_paused(false);
    drop(producer);
    assert_eq!(
        ingress.try_observe("windows.uia.observe", "Too late"),
        CaptureIngressStatus::Dropped
    );
}
