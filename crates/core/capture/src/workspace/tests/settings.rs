use super::*;

#[test]
fn run_settings_allow_rename_while_connected_but_protect_runtime_configuration() {
    let (_root, mut store) = run_store();
    let summary = create_run(&mut store);
    store
        .set_status(summary.id(), ProbeRunStatus::Running)
        .expect("mark running");

    let renamed = store
        .update(
            summary.id(),
            ProbeRunUpdate::new("Renamed probe", ["windows.gdi.text-out"], true)
                .expect("rename update"),
        )
        .expect("rename connected run");
    assert_eq!(renamed.name(), "Renamed probe");
    assert_eq!(renamed.status(), ProbeRunStatus::Running);

    assert_eq!(
        store.update(
            summary.id(),
            ProbeRunUpdate::new("Renamed probe", ["windows.gdi.draw-text"], false)
                .expect("configuration update"),
        ),
        Err(ProbeRunError::InvalidState)
    );

    store
        .set_status(summary.id(), ProbeRunStatus::Ready)
        .expect("release run");
    let updated = store
        .update(
            summary.id(),
            ProbeRunUpdate::new("Renamed probe", ["windows.gdi.draw-text"], false)
                .expect("configuration update"),
        )
        .expect("update released run");
    assert_eq!(
        updated.adapter_ids(),
        &[Box::<str>::from("windows.gdi.draw-text")]
    );
    assert!(!updated.live_preview_enabled());
}

#[test]
fn clearing_observations_resets_evidence_and_allows_a_fresh_capture() {
    let (_root, mut store) = run_store();
    let summary = create_run(&mut store);
    let sink = FileCaptureSink::start(
        store
            .capture_configuration(summary.id(), 100)
            .expect("capture config"),
    )
    .expect("capture sink");
    sink.observe("windows.gdi.text-out", "Old text");
    sink.finish().expect("finish capture");
    store
        .set_ignored(summary.id(), &[Box::<str>::from("Old text")], true)
        .expect("ignore old text");

    let cleared = store
        .clear_observations(summary.id())
        .expect("clear observations");
    assert_eq!(cleared.observed_count, 0);
    assert_eq!(cleared.ignored_count, 0);
    assert_eq!(cleared.dropped_observations, 0);
    assert!(CaptureCatalog::read_current(&store.observation_path(summary.id())).is_err());

    let empty_dictionary = ProbeDictionarySnapshot::new(1, []).expect("empty dictionary");
    let empty_page = store
        .query_entries(
            summary.id(),
            &ProbeQuery::new("", 1, 20).expect("empty query"),
            &empty_dictionary,
        )
        .expect("empty page");
    assert_eq!(empty_page.total, 0);

    let sink = FileCaptureSink::start(
        store
            .capture_configuration(summary.id(), 100)
            .expect("fresh capture config"),
    )
    .expect("fresh capture sink");
    sink.observe("windows.gdi.text-out", "Fresh text");
    sink.finish().expect("finish fresh capture");
    let fresh_page = store
        .query_entries(
            summary.id(),
            &ProbeQuery::new("", 1, 20).expect("fresh query"),
            &empty_dictionary,
        )
        .expect("fresh page");
    assert_eq!(fresh_page.total, 1);
    assert_eq!(fresh_page.rows[0].source.as_ref(), "Fresh text");
}
