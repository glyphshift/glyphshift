use super::CaptureCatalog;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

mod export;
mod model;
mod query;
mod store;

pub use model::*;

pub const PROBE_RUN_SCHEMA: &str = "glyphshift.probe-run/1";
pub const MAX_PROBE_QUERY_PAGE_SIZE: usize = 200;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProbeRunDocument {
    schema: Box<str>,
    storage_revision: u64,
    catalog_revision: u64,
    summary: ProbeRunSummary,
    ignored_sources: Vec<Box<str>>,
}

pub struct ProbeRunStore {
    root: PathBuf,
    source_policies: BTreeMap<Box<str>, glyphshift_domain::SourceTextPolicy>,
    cache: RefCell<ReadCache>,
}

// One active run is retained so browsing historical runs cannot grow this cache indefinitely.
#[derive(Default)]
struct ReadCache {
    run_id: String,
    sources: [Option<String>; 2],
    catalog: Option<Arc<CaptureCatalog>>,
    synchronized: Option<ProbeRunDocument>,
    rows: Option<RowCache>,
    decodes: usize,
    row_builds: usize,
}
struct RowCache {
    document: ProbeRunDocument,
    dictionary: ProbeDictionarySnapshot,
    rows: Arc<Vec<ProbeEntryRow>>,
    search_texts: Arc<Vec<Vec<String>>>,
}

struct ProbeSourceKeys {
    common: glyphshift_domain::SourceTextPolicy,
    normalized: BTreeMap<glyphshift_domain::SourceTextPolicy, BTreeSet<String>>,
    exact: BTreeSet<String>,
}
impl ProbeSourceKeys {
    fn key(&self, source: &str) -> String {
        use glyphshift_domain::SourceTextPolicy;
        if self.common != SourceTextPolicy::Exact {
            return self.common.key(source);
        }
        if self.exact.contains(source) {
            return source.to_owned();
        }
        let candidates = self
            .normalized
            .iter()
            .filter_map(|(policy, observed)| {
                let key = policy.key(source);
                observed.contains(&key).then_some(key)
            })
            .collect::<BTreeSet<_>>();
        if candidates.len() == 1 {
            candidates.into_iter().next().unwrap()
        } else {
            source.to_owned()
        }
    }
}

#[cfg(test)]
#[path = "workspace/tests/mod.rs"]
mod tests;

#[cfg(test)]
mod pagination_boundary_tests {
    use super::*;
    #[test]
    fn dictionary_collection_accepts_supported_page_sizes() {
        for size in [50, 100, 200] {
            assert!(ProbeQuery::new("", 1, size).is_ok());
        }
        assert!(ProbeQuery::new("", 1, 201).is_err());
    }
}
