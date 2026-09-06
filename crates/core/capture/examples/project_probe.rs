//! Inspect a copied probe workspace; arguments must point to local test inputs.
use glyphshift_capture::{ProbeDictionaryEntry, ProbeDictionarySnapshot, ProbeQuery, ProbeRunStore};
use glyphshift_domain::SourceTextPolicy;
use std::{fs, path::PathBuf};

fn main() {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    assert_eq!(arguments.len(), 5, "copied workspace, dictionary, run id, adapter id, search");
    let root = PathBuf::from(&arguments[0]);
    let dictionary: serde_json::Value = serde_json::from_slice(&fs::read(&arguments[1]).unwrap()).unwrap();
    let entries = dictionary["entries"].as_array().unwrap().iter().filter_map(|entry| {
        let source = entry["source"].as_str()?; let translation = entry["translation"].as_str()?;
        (!source.trim().is_empty() && !translation.trim().is_empty()).then(|| ProbeDictionaryEntry::new(source, translation))
    });
    let snapshot = ProbeDictionarySnapshot::new(1, entries).unwrap();
    let mut store = ProbeRunStore::open(root).unwrap();
    let query = ProbeQuery::new(arguments[4].to_string_lossy(), 1, 100).unwrap();
    let run = arguments[2].to_string_lossy();
    let before = store.query_entries(&run, &query, &snapshot).unwrap();
    store.set_source_policy(arguments[3].to_string_lossy().as_ref(), SourceTextPolicy::SpacePaddedSoftWrap);
    let after = store.query_entries(&run, &query, &snapshot).unwrap();
    println!("{}", serde_json::json!({"before":before.total(),"after":after.total(),"rows":after.rows()}));
}
