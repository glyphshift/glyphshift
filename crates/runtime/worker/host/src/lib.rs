//! Supervised process transport for observe-only isolated workers.

mod hybrid;
mod isolated;
mod supervisor;
mod worker;

pub use glyphshift_isolated_worker_sdk::WorkerTargetGrant;
pub use hybrid::HybridAdapterHost;
pub use isolated::IsolatedWorkerHost;
pub use worker::{
    ProcessIsolatedWorker, WorkerArtifact, WorkerArtifactCatalog, WorkerDrainReport, WorkerHealth,
    WorkerHealthReport, WorkerHostError,
};

#[cfg(test)]
mod tests;
