use super::*;

#[test]
fn run_joins_observations_with_one_dictionary_without_persisting_translation_content() {
    let (root, mut store) = run_store();
    let summary = create_run(&mut store);
    let sink = FileCaptureSink::start(
        store
            .capture_configuration(summary.id(), 10_000)
            .expect("capture config"),
    )
    .expect("capture sink");
    for index in 0..5_000 {
        sink.observe("windows.gdi.text-out", format!("Source {index:04}"));
    }
    sink.finish().expect("finish capture");
    let dictionary = ProbeDictionarySnapshot::new(
        7,
        [
            ProbeDictionaryEntry::new("Source 4999", "译文"),
            ProbeDictionaryEntry::new("Imported", "已导入"),
        ],
    )
    .expect("dictionary snapshot");

    let page = store
        .query_entries(
            summary.id(),
            &ProbeQuery::new("", 1, 100).expect("query"),
            &dictionary,
        )
        .expect("joined page");
    assert_eq!(page.total, 5_001);
    assert_eq!(page.rows.len(), 100);
    assert_eq!(page.dictionary_revision, 7);
    let imported = store
        .query_entries(
            summary.id(),
            &ProbeQuery::new("Imported", 1, 20).expect("query"),
            &dictionary,
        )
        .expect("imported row");
    assert_eq!(imported.rows[0].state, ProbeEntryState::Unobserved);

    let document_text = fs::read_to_string(
        store
            .document_paths(summary.id())
            .into_iter()
            .find(|path| path.exists())
            .expect("run document"),
    )
    .expect("read run document");
    assert!(!document_text.contains("译文"));
    assert!(!document_text.contains("已导入"));
    assert!(root.path().exists());
}

#[test]
fn run_recovers_disconnected_state_and_ignore_keeps_dictionary_unchanged() {
    let (root, mut store) = run_store();
    let summary = create_run(&mut store);
    let paused_summary = store
        .create(
            ProbeRunCreate::new(
                "probe-paused",
                "Paused probe",
                "software-two",
                "dictionary-two",
                ["windows.gdi.text-out"],
                false,
            )
            .expect("paused probe create"),
        )
        .expect("create paused run");
    let interrupted_summary = store
        .create(
            ProbeRunCreate::new(
                "probe-interrupted",
                "Interrupted probe",
                "software-three",
                "dictionary-three",
                ["windows.gdi.text-out"],
                false,
            )
            .expect("interrupted probe create"),
        )
        .expect("create interrupted run");
    let sink = FileCaptureSink::start(
        store
            .capture_configuration(summary.id(), 10)
            .expect("capture config"),
    )
    .expect("capture sink");
    sink.observe("windows.gdi.text-out", "Open");
    sink.finish().expect("finish capture");
    store
        .set_status(summary.id(), ProbeRunStatus::Running)
        .expect("mark running");
    store
        .set_status(paused_summary.id(), ProbeRunStatus::Paused)
        .expect("mark paused");
    store
        .set_status(interrupted_summary.id(), ProbeRunStatus::Interrupted)
        .expect("mark interrupted");
    store
        .set_ignored(summary.id(), &[Box::<str>::from("Open")], true)
        .expect("ignore observed source");
    let dictionary = ProbeDictionarySnapshot::new(
        3,
        [
            ProbeDictionaryEntry::new("Open", "打开"),
            ProbeDictionaryEntry::new("Save", "保存"),
        ],
    )
    .expect("dictionary snapshot");
    let preview = store
        .preview_entries(summary.id(), &dictionary)
        .expect("preview");
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].source(), "Save");
    drop(store);

    let mut reopened = ProbeRunStore::open(root.path()).expect("reopen store");
    let recovered = reopened.summary(summary.id()).expect("summary");
    assert_eq!(recovered.status(), ProbeRunStatus::Ready);
    let recovered_paused = reopened
        .summary(paused_summary.id())
        .expect("paused summary");
    assert_eq!(recovered_paused.status(), ProbeRunStatus::Ready);
    let recovered_interrupted = reopened
        .summary(interrupted_summary.id())
        .expect("interrupted summary");
    assert_eq!(recovered_interrupted.status(), ProbeRunStatus::Ready);
    let page = reopened
        .query_entries(
            summary.id(),
            &ProbeQuery::new("Open", 1, 20).expect("query"),
            &dictionary,
        )
        .expect("joined page");
    assert_eq!(page.rows[0].state, ProbeEntryState::Ignored);
    assert_eq!(page.rows[0].translation.as_ref(), "打开");
}
