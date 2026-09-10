use super::*;
use std::time::Instant;

fn checkpoint(store: &ProbeRunStore, revision: u64, count: usize) {
    let entries = (0..count).map(|index| serde_json::json!({
        "source": format!("Source {index:05}"), "adapterId": "windows.gdi.text-out",
        "count": revision, "firstSeenMs": 1, "lastSeenMs": revision + 1
    })).collect::<Vec<_>>();
    let value = serde_json::json!({"schema": crate::CAPTURE_CATALOG_SCHEMA, "sessionId": "probe-one",
        "revision": revision, "startedAtMs": 1, "updatedAtMs": revision + 1,
        "droppedObservations": 0, "entries": entries});
    fs::write(store.observation_path("probe-one").with_extension("a.json"), value.to_string()).unwrap();
}

#[test]
fn repeated_large_queries_reuse_decode_and_join_and_invalidate_on_updates() {
    let (_root, mut store) = run_store();
    create_run(&mut store);
    store.attach_workflow("probe-one", "workflow-one").unwrap();
    checkpoint(&store, 1, 10_000);
    let dictionary = ProbeDictionarySnapshot::new(1, (0..5_000).map(|index|
        ProbeDictionaryEntry::new(format!("Source {index:05}"), "Translated"))).unwrap();
    let start = Instant::now();
    for page in 1..=20 {
        let result = store.query_entries("probe-one", &ProbeQuery::new("", page, 200).unwrap(), &dictionary).unwrap();
        assert_eq!(result.total(), 10_000);
        assert_eq!(result.rows().len(), 200);
    }
    let warm = start.elapsed();
    assert_eq!(store.cache.borrow().decodes, 1);
    assert_eq!(store.cache.borrow().row_builds, 1);
    let start = Instant::now();
    for page in 1..=20 {
        *store.cache.get_mut() = ReadCache::default();
        store.query_entries("probe-one", &ProbeQuery::new("", page, 200).unwrap(), &dictionary).unwrap();
    }
    eprintln!("synthetic 10000 sources / 20 pages: cached={warm:?}, forced rebuild={:?}", start.elapsed());
    // Same-size count/timestamp changes must be visible without relying on file metadata.
    checkpoint(&store, 2, 10_000);
    let query = ProbeQuery::new("", 1, 50).unwrap();
    let result = store.query_entries("probe-one", &query, &dictionary).unwrap();
    assert_eq!(result.rows()[0].count, 2);
    let excluded = dictionary.clone().with_excluded_sources(BTreeSet::from(["Source 00000".into()]));
    assert_eq!(store.query_entries("probe-one", &query, &excluded).unwrap().total(), 9_999);
    let updated = ProbeDictionarySnapshot::new(2, [ProbeDictionaryEntry::new("Source 00001", "Changed")]).unwrap();
    let result = store.query_entries("probe-one", &ProbeQuery::new("Changed", 1, 50).unwrap(), &updated).unwrap();
    assert_eq!(result.total(), 1);
    // Visibility is evaluated before counting/paging, including after a rule change.
    assert_eq!(store.query_entries_visible("probe-one", &query, &updated, |source| source.ends_with('1')).unwrap().total(), 1_000);
    assert_eq!(store.query_entries_visible("probe-one", &query, &updated, |_| true).unwrap().total(), 10_000);
    store.clear_observations("probe-one").unwrap();
    assert_eq!(store.query_entries("probe-one", &query, &updated).unwrap().total(), 0);
}

#[test]
fn source_collection_respects_empty_exclusions_and_ignored_sources() {
    let (_root, mut store) = run_store();
    create_run(&mut store);
    store.attach_workflow("probe-one", "workflow-one").unwrap();
    checkpoint(&store, 1, 100);
    let dictionary = ProbeDictionarySnapshot::new(1, [ProbeDictionaryEntry::new("Source 00000", "")]).unwrap()
        .with_excluded_sources(BTreeSet::from(["Source 00001".into()]));
    let sources = store.uncollected_sources("probe-one", &dictionary).unwrap();
    assert_eq!(sources.len(), 98);
    assert!(!sources.iter().any(|source| source.as_ref() == "Source 00000" || source.as_ref() == "Source 00001"));
    assert_eq!(store.cache.borrow().row_builds, 0);
}
