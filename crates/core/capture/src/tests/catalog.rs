use crate::*;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn capture_sink_deduplicates_observations_by_source_and_adapter() {
    let root = tempdir().expect("capture root");
    let output = root.path().join("capture.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("capture-1").expect("session id"),
            &output,
            10,
        )
        .expect("configuration"),
    )
    .expect("capture sink");
    sink.observe("windows.gdi.text-out", "Open");
    sink.observe("windows.gdi.text-out", "Open");
    sink.observe("windows.user32.draw-text", "Open");
    sink.observe("windows.user32.draw-text", "Close");

    let catalog = sink.finish().expect("finished catalog");
    assert_eq!(catalog.entries().len(), 3);
    assert_eq!(
        catalog
            .entries()
            .iter()
            .find(|entry| {
                entry.source() == "Open" && entry.adapter_id() == "windows.gdi.text-out"
            })
            .map(CaptureCatalogEntry::count),
        Some(2),
    );
    assert_eq!(
        CaptureCatalog::read_current(&output).expect("saved catalog"),
        catalog
    );
    let encoded = catalog.encode_json().expect("catalog json");
    assert!(encoded.contains("adapterId"));
    assert!(!encoded.contains("translation"));
}

#[test]
fn cloned_ingresses_merge_multiple_producers_through_one_checkpoint_owner() {
    let root = tempdir().expect("capture root");
    let output = root.path().join("capture.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("capture-ingress").expect("session id"),
            &output,
            10,
        )
        .expect("configuration"),
    )
    .expect("capture sink");
    let target_process = sink.ingress();
    let isolated_worker = sink.ingress();
    let stopped_worker = sink.ingress();

    let target_thread = std::thread::spawn(move || {
        target_process.try_observe("windows.gdi.text-out", "Target text")
    });
    let worker_thread = std::thread::spawn(move || {
        isolated_worker.try_observe("windows.uia.observe", "Worker text")
    });

    assert_eq!(
        target_thread.join().expect("target producer"),
        CaptureIngressStatus::Accepted
    );
    assert_eq!(
        worker_thread.join().expect("worker producer"),
        CaptureIngressStatus::Accepted
    );
    let catalog = sink.finish().expect("single capture owner");

    assert_eq!(catalog.entries().len(), 2);
    assert!(catalog.entries().iter().any(|entry| {
        entry.adapter_id() == "windows.gdi.text-out" && entry.source() == "Target text"
    }));
    assert!(catalog.entries().iter().any(|entry| {
        entry.adapter_id() == "windows.uia.observe" && entry.source() == "Worker text"
    }));
    assert_eq!(
        CaptureCatalog::read_current(&output).expect("single checkpoint"),
        catalog
    );
    assert_eq!(
        stopped_worker.try_observe("windows.uia.observe", "Too late"),
        CaptureIngressStatus::Dropped
    );
    assert_eq!(stopped_worker.dropped_observations(), 1);
}

#[test]
fn capture_sink_caps_unique_entries_without_blocking_the_observer() {
    let root = tempdir().expect("capture root");
    let output = root.path().join("capture.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("capture-limit").expect("session id"),
            &output,
            1,
        )
        .expect("configuration"),
    )
    .expect("capture sink");
    sink.observe("windows.gdi.text-out", "One");
    sink.observe("windows.gdi.text-out", "Two");
    let catalog = sink.finish().expect("finished catalog");

    assert_eq!(catalog.entries().len(), 1);
    assert_eq!(catalog.dropped_observations(), 1);
}

#[test]
fn preferred_sources_displace_fallback_evidence_when_capacity_is_full() {
    let root = tempdir().expect("capture root");
    let output = root.path().join("capture.json");
    let configuration = CaptureConfiguration::new(
        CaptureSessionId::new("capture-source-priority").expect("session id"),
        &output,
        1,
    )
    .expect("configuration")
    .with_fallback_adapters(["synthetic.a-observer"])
    .expect("fallback source policy");
    let sink = FileCaptureSink::start(configuration).expect("capture sink");

    sink.observe("synthetic.a-observer", "Fallback text");
    sink.observe("synthetic.z-writeback", "Writeback text");
    let catalog = sink.finish().expect("finished catalog");

    assert_eq!(catalog.entries().len(), 1);
    assert_eq!(catalog.entries()[0].adapter_id(), "synthetic.z-writeback");
    assert_eq!(catalog.entries()[0].source(), "Writeback text");
    assert_eq!(catalog.dropped_observations(), 1);
}

#[test]
fn capture_sink_checkpoints_during_continuous_observations() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    let root = tempdir().expect("capture root");
    let output = root.path().join("capture.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("capture-continuous").expect("session id"),
            &output,
            10,
        )
        .expect("configuration"),
    )
    .expect("capture sink");
    let ingress = sink.ingress();
    let running = Arc::new(AtomicBool::new(true));
    let producing = running.clone();
    let producer = std::thread::spawn(move || {
        while producing.load(Ordering::Acquire) {
            let _ = ingress.try_observe("synthetic.draw-text", "Synthetic effect parameter");
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut first_revision = None;
    let mut visible_while_producing = false;
    while Instant::now() < deadline {
        if let Ok(catalog) = CaptureCatalog::read_current(&output) {
            if catalog
                .entries()
                .iter()
                .any(|entry| entry.source() == "Synthetic effect parameter")
            {
                if first_revision.is_some_and(|revision| catalog.revision() > revision) {
                    visible_while_producing = true;
                    break;
                }
                first_revision.get_or_insert(catalog.revision());
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    running.store(false, Ordering::Release);
    producer.join().expect("continuous producer");
    sink.finish().expect("finish capture");
    assert!(
        visible_while_producing,
        "live observations must keep reaching the catalog without an idle gap"
    );
}

#[test]
fn capture_sink_checkpoints_while_running_and_pause_does_not_end_the_session() {
    let root = tempdir().expect("capture root");
    let output = root.path().join("capture.json");
    let sink = FileCaptureSink::start(
        CaptureConfiguration::new(
            CaptureSessionId::new("capture-live").expect("session id"),
            &output,
            10,
        )
        .expect("configuration"),
    )
    .expect("capture sink");
    sink.observe("windows.gdi.text-out", "Before pause");
    std::thread::sleep(Duration::from_millis(1_100));
    let live = CaptureCatalog::read_current(&output).expect("live checkpoint");
    assert_eq!(live.entries().len(), 1);

    let ingress = sink.ingress();
    sink.set_paused(true);
    assert_eq!(
        ingress.try_observe("windows.gdi.text-out", "Ignored while paused"),
        CaptureIngressStatus::Paused
    );
    sink.set_paused(false);
    assert_eq!(
        ingress.try_observe("windows.gdi.text-out", "After resume"),
        CaptureIngressStatus::Accepted
    );
    let finished = sink.finish().expect("finish capture");
    assert_eq!(finished.entries().len(), 2);
    assert!(finished
        .entries()
        .iter()
        .all(|entry| entry.source() != "Ignored while paused"));
}

#[test]
fn capture_sink_resumes_an_existing_catalog_without_resetting_its_revision_or_entries() {
    let root = tempdir().expect("capture root");
    let output = root.path().join("capture.json");
    let configuration = || {
        CaptureConfiguration::new(
            CaptureSessionId::new("capture-resume").expect("session id"),
            &output,
            10,
        )
        .expect("configuration")
    };

    let first = FileCaptureSink::start(configuration()).expect("first capture sink");
    first.observe("windows.gdi.text-out", "Before reconnect");
    let first_catalog = first.finish().expect("first catalog");

    let resumed = FileCaptureSink::start(configuration()).expect("resumed capture sink");
    resumed.observe("windows.gdi.text-out", "After reconnect");
    let resumed_catalog = resumed.finish().expect("resumed catalog");

    assert!(resumed_catalog.revision() > first_catalog.revision());
    assert!(resumed_catalog
        .entries()
        .iter()
        .any(|entry| entry.source() == "Before reconnect"));
    assert!(resumed_catalog
        .entries()
        .iter()
        .any(|entry| entry.source() == "After reconnect"));
    assert_eq!(
        CaptureCatalog::read_current(&output).expect("current catalog"),
        resumed_catalog
    );
}
