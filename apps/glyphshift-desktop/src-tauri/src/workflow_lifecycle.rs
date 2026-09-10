use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn next_revision() -> u64 {
    static REVISION: AtomicU64 = AtomicU64::new(1);
    REVISION.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkflowLifecycle {
    pub(super) phase: &'static str,
    pub(super) enabled: bool,
    pub(super) collect_new_sources: bool,
    pub(super) checked_at_ms: u64,
    pub(super) revision: u64,
}

impl DesktopApplication {
    pub(super) fn project_workflow_runtime(&self, mut runtime: WorkflowRuntimeView) -> WorkflowRuntimeView {
        let enabled = self.backend.enabled_workflow_ids().iter().any(|id| id == &runtime.workflow_id);
        let active = runtime.targets.iter().any(|target| target.active);
        let actionable = runtime.errors.values().any(|error| error.code() != "runtime.target_not_found");
        let phase = if !enabled {
            if active || actionable { "stop_failed" } else { "stopped" }
        } else if actionable { "failed" }
        else if active { "running" }
        else { "waiting" };
        let collect_new_sources = self.backend.workflow(&runtime.workflow_id).ok()
            .is_some_and(|workflow| workflow.targets().iter().any(|target| target.collection_enabled()));
        runtime.lifecycle = Some(WorkflowLifecycle { phase, enabled, collect_new_sources,
            checked_at_ms: runtime.checked_at_ms, revision: runtime.revision });
        runtime
    }
}

// Live sessions still need liveness checks. Only inactive transient failures may
// reconnect automatically; configuration, privilege and resident-module errors wait for the user.
pub(super) fn automatic_retry_allowed(runtime: &WorkflowRuntimeView, now: u64) -> bool {
    if runtime.targets.iter().any(|target| target.active) { return true; }
    let errors = runtime.errors.values().filter(|error| error.code() != "runtime.target_not_found").collect::<Vec<_>>();
    errors.is_empty() || (runtime.retry_attempt < 3 && now >= runtime.retry_after_ms
        && errors.iter().all(|error| error.code() == "runtime.activation_timed_out"))
}
