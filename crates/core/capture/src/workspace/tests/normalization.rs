use super::*;
use glyphshift_domain::SourceTextPolicy;

#[test]
fn layout_variants_project_as_one_row_without_deleting_legacy_translations() {
    let (_root, mut store) = run_store();
    let summary = create_run(&mut store);
    let variants = ["Strong flexible \r\nmaterial for tools.", "Strong \nflexible material \r\nfor tools."];
    let sink = FileCaptureSink::start(store.capture_configuration(summary.id(), 100).unwrap()).unwrap();
    sink.observe("windows.gdi.text-out", variants[0]);
    sink.observe("windows.gdi.text-out", variants[1]);
    sink.observe("windows.gdi.text-out", variants[0]);
    sink.observe("windows.gdi.text-out", "Heading\nBody");
    sink.finish().unwrap();
    let dictionary = ProbeDictionarySnapshot::new(1, [
        ProbeDictionaryEntry::new(variants[0], "结实的材料"),
        ProbeDictionaryEntry::new(variants[1], "坚韧的材料"),
    ]).unwrap();
    let query = ProbeQuery::new("Strong", 1, 50).unwrap();
    assert_eq!(store.query_entries(summary.id(), &query, &dictionary).unwrap().total, 2);
    // The rule is producer metadata, not an adapter-name special case.
    store.set_source_policy("windows.gdi.text-out", SourceTextPolicy::SpacePaddedSoftWrap);
    let page = store.query_entries(summary.id(), &query, &dictionary).unwrap();
    assert_eq!(page.total, 1);
    let row = &page.rows[0];
    assert_eq!(row.source(), "Strong flexible material for tools.");
    assert_eq!(row.count, 3);
    assert!(row.has_translation_conflict());
    assert_eq!(row.translation_variants.len(), 2);
    assert!(row.translation().is_empty());
    assert_eq!(store.summary(summary.id()).unwrap().observed_count, 2);
    assert_eq!(CaptureCatalog::read_current(&store.observation_path(summary.id())).unwrap().entries().len(), 3);
    assert_eq!(dictionary.entries.len(), 2);
    assert_eq!(store.dictionary_sources_for_rows(summary.id(), &[row.source.clone()], &dictionary).unwrap().len(), 2);
    store.set_ignored(summary.id(), &[row.source.clone()], true).unwrap();
    assert_eq!(store.query_entries(summary.id(), &query, &dictionary).unwrap().rows[0].state, ProbeEntryState::Ignored);
    assert!(store.preview_entries(summary.id(), &dictionary).unwrap().is_empty());
    store.set_ignored(summary.id(), &[row.source.clone()], false).unwrap();
    let canonical = ProbeDictionarySnapshot::new(2, [
        ProbeDictionaryEntry::new(variants[0], "结实的材料"), ProbeDictionaryEntry::new(variants[1], "坚韧的材料"),
        ProbeDictionaryEntry::new(row.source.clone(), "选定的统一译文"),
    ]).unwrap();
    let resolved = store.query_entries(summary.id(), &query, &canonical).unwrap();
    assert_eq!(resolved.total, 1);
    assert_eq!(resolved.rows[0].translation(), "选定的统一译文");
    assert!(!resolved.rows[0].has_translation_conflict());
}

#[test]
fn mixed_probe_uses_observed_producer_policy_without_normalizing_other_sources() {
    let (_root, mut store) = run_store();
    let run = store.create(ProbeRunCreate::new("mixed-wrap", "Mixed", "software-one", "dictionary-one", ["synthetic.wrap", "synthetic.exact"], true).unwrap()).unwrap();
    store.set_source_policy("synthetic.wrap", SourceTextPolicy::SpacePaddedSoftWrap);
    let source1 = "A strong \nmaterial.";
    let source2 = "A \nstrong material.";
    let sink = FileCaptureSink::start(store.capture_configuration(run.id(), 100).unwrap()).unwrap();
    sink.observe("synthetic.wrap", source1); sink.observe("synthetic.wrap", source2);
    sink.observe("synthetic.exact", "Other \nparagraph.");
    sink.finish().unwrap();
    let dictionary = ProbeDictionarySnapshot::new(1, [ProbeDictionaryEntry::new(source1, "材料甲"), ProbeDictionaryEntry::new(source2, "材料乙"), ProbeDictionaryEntry::new("Other \nparagraph.", "另一段")]).unwrap();
    let page = store.query_entries(run.id(), &ProbeQuery::new("",1,100).unwrap(), &dictionary).unwrap();
    assert_eq!(page.total, 2);
    assert!(page.rows.iter().any(|row| row.source() == "A strong material." && row.has_translation_conflict()));
    assert!(page.rows.iter().any(|row| row.source() == "Other \nparagraph."));
    store.set_ignored(run.id(), &["A strong material.".into()], true).unwrap();
    assert_eq!(store.preview_entries(run.id(), &dictionary).unwrap().len(), 1);
}

#[test]
fn identical_legacy_translations_are_reused_and_hard_breaks_remain_distinct() {
    let (_root, mut store) = run_store();
    let summary = create_run(&mut store);
    store.set_source_policy("windows.gdi.text-out", SourceTextPolicy::SpacePaddedSoftWrap);
    let dictionary = ProbeDictionarySnapshot::new(1, [
        ProbeDictionaryEntry::new("A strong \nmaterial.", "材料"),
        ProbeDictionaryEntry::new("A \nstrong material.", "材料"),
        ProbeDictionaryEntry::new("A strong\nmaterial.", "真实换行"),
    ]).unwrap();
    let page = store.query_entries(summary.id(), &ProbeQuery::new("",1,50).unwrap(), &dictionary).unwrap();
    assert_eq!(page.total, 2);
    let canonical = page.rows.iter().find(|row| row.source() == "A strong material.").unwrap();
    assert_eq!(canonical.translation(), "材料");
    assert!(!canonical.has_translation_conflict());
}

#[test]
fn excluded_source_write_checks_follow_the_same_soft_wrap_policy_as_rows() {
    let (_root, mut store) = run_store();
    let run = create_run(&mut store);
    store.set_source_policy("windows.gdi.text-out", SourceTextPolicy::SpacePaddedSoftWrap);
    let source = "A long \r\nsource text";
    let sink = FileCaptureSink::start(store.capture_configuration(run.id(), 100).unwrap()).unwrap();
    sink.observe("windows.gdi.text-out", source);
    sink.finish().unwrap();
    let snapshot = ProbeDictionarySnapshot::new(1, []).unwrap()
        .with_excluded_sources([source.to_owned()].into());
    assert_eq!(store.query_entries(run.id(), &ProbeQuery::new("", 1, 50).unwrap(), &snapshot).unwrap().total, 0);
    let sources = vec![Box::<str>::from("A long source text"), "Unrelated text".into()];
    assert_eq!(store.excluded_sources_for(run.id(), &snapshot, &sources).unwrap(), [Box::<str>::from("A long source text")].into());
}
