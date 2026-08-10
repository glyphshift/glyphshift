use super::*;

#[test]
fn query_filters_joined_rows_by_run_adapter_without_changing_full_exports() {
    let (_root, mut store) = run_store();
    let summary = store
        .create(
            ProbeRunCreate::new(
                "probe-filter",
                "Filter probe",
                "software-one",
                "dictionary-one",
                ["synthetic.adapter-one", "synthetic.adapter-two"],
                false,
            )
            .expect("probe create"),
        )
        .expect("create run");
    let sink = FileCaptureSink::start(
        store
            .capture_configuration(summary.id(), 100)
            .expect("capture config"),
    )
    .expect("capture sink");
    sink.observe("synthetic.adapter-one", "Open");
    sink.observe("synthetic.adapter-one", "Open");
    sink.observe("synthetic.adapter-two", "Open");
    sink.observe("synthetic.adapter-two", "Save");
    sink.finish().expect("finish capture");
    let dictionary = ProbeDictionarySnapshot::new(
        2,
        [
            ProbeDictionaryEntry::new("Open", "打开"),
            ProbeDictionaryEntry::new("Imported", "已导入"),
        ],
    )
    .expect("dictionary snapshot");
    let dictionary_before = dictionary.clone();
    let observations_before = CaptureCatalog::read_current(&store.observation_path(summary.id()))
        .expect("observation index before filtering");

    let gdi_page = store
        .query_entries(
            summary.id(),
            &ProbeQuery::new("", 1, 20)
                .expect("query")
                .with_adapter_ids(["synthetic.adapter-one"])
                .expect("adapter filter"),
            &dictionary,
        )
        .expect("filtered page");
    assert_eq!(gdi_page.total, 1);
    assert_eq!(gdi_page.rows[0].source.as_ref(), "Open");
    assert_eq!(gdi_page.rows[0].count, 3);

    let gdiplus_page = store
        .query_entries(
            summary.id(),
            &ProbeQuery::new("", 1, 20)
                .expect("query")
                .with_adapter_ids(["synthetic.adapter-two"])
                .expect("adapter filter"),
            &dictionary,
        )
        .expect("filtered page");
    assert_eq!(gdiplus_page.total, 2);
    assert!(gdiplus_page
        .rows
        .iter()
        .all(|row| row.source.as_ref() != "Imported"));

    let unknown_filter = ProbeQuery::new("", 1, 20)
        .expect("query")
        .with_adapter_ids(["synthetic.adapter-unknown"])
        .expect("well-formed unknown adapter");
    assert_eq!(
        store.query_entries(summary.id(), &unknown_filter, &dictionary),
        Err(ProbeRunError::InvalidInput)
    );

    let exported = String::from_utf8(
        store
            .export(summary.id(), ProbeExportFormat::EntriesCsv, &dictionary)
            .expect("full joined export"),
    )
    .expect("utf-8 export");
    assert!(exported.contains("Imported"));
    assert!(exported.contains("Save"));
    assert_eq!(dictionary, dictionary_before);
    assert_eq!(
        CaptureCatalog::read_current(&store.observation_path(summary.id()))
            .expect("observation index after filtering"),
        observations_before
    );
}

#[test]
fn joined_rows_preserve_the_run_source_priority_regardless_of_arrival_order() {
    let (_root, mut store) = run_store();
    let summary = store
        .create(
            ProbeRunCreate::new(
                "probe-source-priority",
                "Source priority probe",
                "software-one",
                "dictionary-one",
                ["synthetic.z-writeback", "synthetic.a-observer"],
                false,
            )
            .expect("probe create"),
        )
        .expect("create run");
    let sink = FileCaptureSink::start(
        store
            .capture_configuration(summary.id(), 100)
            .expect("capture config"),
    )
    .expect("capture sink");

    // The fallback observer reports first, but the run's explicit source
    // priority must remain authoritative for the joined presentation.
    sink.observe("synthetic.a-observer", "Open");
    sink.observe("synthetic.z-writeback", "Open");
    sink.finish().expect("finish capture");

    let page = store
        .query_entries(
            summary.id(),
            &ProbeQuery::new("", 1, 20).expect("query"),
            &ProbeDictionarySnapshot::new(1, []).expect("empty dictionary"),
        )
        .expect("joined page");

    assert_eq!(page.rows.len(), 1);
    assert_eq!(
        page.rows[0].adapter_ids,
        vec![
            Box::<str>::from("synthetic.z-writeback"),
            Box::<str>::from("synthetic.a-observer"),
        ]
    );
}
