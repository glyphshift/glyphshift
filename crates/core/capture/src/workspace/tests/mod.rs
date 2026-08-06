use super::*;
use crate::FileCaptureSink;
use tempfile::tempdir;

fn run_store() -> (tempfile::TempDir, ProbeRunStore) {
    let root = tempdir().expect("probe root");
    let store = ProbeRunStore::open(root.path()).expect("open store");
    (root, store)
}

fn create_run(store: &mut ProbeRunStore) -> ProbeRunSummary {
    store
        .create(
            ProbeRunCreate::new(
                "probe-one",
                "UI probe",
                "software-one",
                "dictionary-one",
                ["windows.gdi.text-out"],
                true,
            )
            .expect("probe create"),
        )
        .expect("create run")
}

mod join;
mod query;
mod settings;
