use super::*;
use crate::{CaptureIngressStatus, CaptureTranslationContext};

#[test]
fn qt_translation_context_keeps_rows_distinct_but_dictionary_collection_source_only() {
    let (_root, mut store) = run_store();
    let summary = store
        .create(
            ProbeRunCreate::new(
                "probe-qt-context",
                "Qt context probe",
                "software-one",
                "dictionary-one",
                ["windows.qt.translation-service"],
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
    let ingress = sink.ingress();
    for context in ["MainMenu", "Toolbar"] {
        assert_eq!(
            ingress.try_observe_with_context(
                "windows.qt.translation-service",
                "Open",
                Some(CaptureTranslationContext::new(
                    Some(context),
                    Option::<&str>::None,
                    None,
                )),
            ),
            CaptureIngressStatus::Accepted
        );
    }
    assert_eq!(
        ingress.try_observe_with_context(
            "windows.qt.translation-service",
            "Open",
            Some(CaptureTranslationContext::new(
                Some("MainMenu"),
                Option::<&str>::None,
                Some(2),
            )),
        ),
        CaptureIngressStatus::Accepted
    );
    let catalog = sink.finish().expect("finish capture");
    assert_eq!(catalog.entries().len(), 3);

    let empty = ProbeDictionarySnapshot::new(1, []).expect("empty dictionary");
    let page = store
        .query_entries(
            summary.id(),
            &ProbeQuery::new("", 1, 20).expect("query"),
            &empty,
        )
        .expect("context rows");
    assert_eq!(page.total(), 3);
    assert_eq!(
        page.rows()
            .iter()
            .filter_map(|row| row.translation_context())
            .map(|context| (context.context().unwrap_or_default(), context.plural_n()))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([("MainMenu", None), ("MainMenu", Some(2)), ("Toolbar", None)])
    );

    let pending = store
        .uncollected_sources_mapped(summary.id(), &empty, |source| vec![source.into()])
        .expect("uncollected sources");
    assert_eq!(pending, vec![Box::<str>::from("Open")]);

    let generic = ProbeDictionarySnapshot::new(3, [ProbeDictionaryEntry::new("Open", "打开")])
        .expect("generic dictionary");
    assert!(store
        .uncollected_sources_mapped(summary.id(), &generic, |source| vec![source.into()])
        .expect("generic fallback")
        .is_empty());

    let page = store
        .query_entries(
            summary.id(),
            &ProbeQuery::new("", 1, 20).expect("query"),
            &generic,
        )
        .expect("source-only dictionary rows");
    assert_eq!(
        page.rows()
            .iter()
            .filter(|row| row.translation_context().is_some_and(|context| context.plural_n().is_none()))
            .map(|row| row.translation())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["打开"])
    );
}

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

#[test]
fn translation_filter_runs_on_the_complete_joined_set_before_paging() {
    let (_root, mut store) = run_store();
    let summary = store
        .create(
            ProbeRunCreate::new(
                "probe-translation-filter",
                "Translation filter probe",
                "software-one",
                "dictionary-one",
                ["synthetic.adapter-one"],
                false,
            )
            .expect("probe create"),
        )
        .expect("create run");
    let sink = FileCaptureSink::start(
        store
            .capture_configuration(summary.id(), 200)
            .expect("capture config"),
    )
    .expect("capture sink");
    for index in 1..=101 {
        sink.observe("synthetic.adapter-one", format!("Source {index:03}"));
    }
    sink.finish().expect("finish capture");
    let dictionary = ProbeDictionarySnapshot::new(
        2,
        (1..=51).map(|index| {
            ProbeDictionaryEntry::new(format!("Source {index:03}"), format!("Translation {index}"))
        }),
    )
    .expect("dictionary snapshot");

    let untranslated = store
        .query_entries(
            summary.id(),
            &ProbeQuery::new("", 1, 20)
                .expect("query")
                .with_translation_filter(ProbeTranslationFilter::Untranslated),
            &dictionary,
        )
        .expect("untranslated page");
    assert_eq!(untranslated.total, 50);
    assert_eq!(untranslated.rows.len(), 20);
    assert!(untranslated
        .rows
        .iter()
        .all(|row| row.translation().is_empty()));

    let translated = store
        .query_entries(
            summary.id(),
            &ProbeQuery::new("", 2, 20)
                .expect("query")
                .with_translation_filter(ProbeTranslationFilter::Translated),
            &dictionary,
        )
        .expect("translated page");
    assert_eq!(translated.total, 51);
    assert_eq!(translated.rows.len(), 20);
    assert!(translated
        .rows
        .iter()
        .all(|row| !row.translation().is_empty()));
}

#[test]
fn exclusions_apply_before_paging_and_export_without_removing_evidence() {
    let (root, mut store) = run_store();
    let create = ProbeRunCreate::new("probe-exclusions", "Excluded work", "software", "main", ["synthetic.adapter"], false).unwrap()
        .with_excluded_dictionaries(vec!["completed-one".into(), "completed-two".into()]).unwrap();
    let run = store.create(create).unwrap();
    let sink = FileCaptureSink::start(store.capture_configuration(run.id(), 100).unwrap()).unwrap();
    for source in ["Done one", "New one", "Done two", "New two"] { sink.observe("synthetic.adapter", source); }
    sink.finish().unwrap();
    let snapshot = ProbeDictionarySnapshot::new(1, [ProbeDictionaryEntry::new("Imported pending", "")]).unwrap()
        .with_excluded_sources(["Done one".to_owned(), "Done two".to_owned()].into());
    let page = store.query_entries(run.id(), &ProbeQuery::new("", 1, 1).unwrap(), &snapshot).unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.rows.len(), 1);
    for format in [ProbeExportFormat::EntriesCsv, ProbeExportFormat::EntriesJson] {
        let output = String::from_utf8(store.export(run.id(), format, &snapshot).unwrap()).unwrap();
        assert!(!output.contains("Done one") && !output.contains("Done two"));
        assert!(output.contains("Imported pending"));
    }
    assert!(store.preview_entries(run.id(), &snapshot).unwrap().is_empty());
    let mut reopened = ProbeRunStore::open(root.path()).unwrap();
    assert_eq!(reopened.summary(run.id()).unwrap().excluded_dictionary_ids(), &[Box::<str>::from("completed-one"), "completed-two".into()]);
    assert_eq!(reopened.summary(run.id()).unwrap().observed_count, 4);
}
