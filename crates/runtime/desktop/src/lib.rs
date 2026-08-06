//! Desktop composition root for a verified Controller, target Runtime, and Adapter bundle.
//!
//! The desktop shell sees only application instances and session state. Process tokens, artifact
//! paths, deployment payloads, and controller acknowledgements stay behind this module boundary.

mod acquisition;
mod bundle;
mod pool;
mod target;

pub use acquisition::{DesktopAcquisitionCancellation, DesktopAcquisitionError};
#[cfg(test)]
use bundle::{
    parse_documentation_url, parse_hash, AcquisitionSupportFileManifest, AcquisitionWorkerCatalog,
    AcquisitionWorkerManifest, BundleManifest,
};
pub use bundle::{RuntimeAdapterOption, RuntimeBundle};
use glyphshift_acquisition::{
    AcquisitionRequest, AuthorizedTarget, InteractiveSelection, SourcePolicy,
};
pub use glyphshift_acquisition::{
    AcquisitionResult, DesktopPoint, DesktopRect, Granularity, Provenance, SourceBlock,
};
use glyphshift_acquisition_worker_host::{
    AcquisitionWorkerArtifact, AcquisitionWorkerBinding, AcquisitionWorkerHost,
    AcquisitionWorkerHostError, CancellationToken,
};
use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_adapter_registry::{
    AdapterDescriptor, AdapterPackage, AdapterPackageSet, AdapterRegistry, AdapterRequirement,
    AdapterTrustPolicy, AdapterVersion, AdapterVersionRequirement, ArtifactHash, PackageArtifactId,
    SignerId,
};
use glyphshift_capture::{CaptureConfiguration, FileCaptureSink};
use glyphshift_controller_host::{
    ControllerStartupConfig, ControllerTrustPolicy, ProcessControllerTransport,
    VerifiedControllerArtifact,
};
use glyphshift_desktop_backend::{DesktopRuntimeSpec, EffectiveWorkflowIntent};
use glyphshift_domain::{AdapterId, ApplyModel, Feature, Generation, Placement, TargetFacts};
use glyphshift_extension::{
    CodeHash, ControllerArtifactId, ControllerCodeIdentity, ControllerSignerId, ExtensionId,
    ProtocolVersion,
};
use glyphshift_isolated_worker_host::{
    HybridAdapterHost, IsolatedWorkerHost, WorkerArtifactCatalog, WorkerTargetGrant,
};
use glyphshift_protocol::{
    ControllerConnection, ControllerNonce, ControllerProtocolError, ControllerTransport,
    ControllerWorkerTargetGrant, NonceLedger, OpaqueTargetId, PreparedRecipe,
    RecipeControllerLossPolicy,
};
use glyphshift_session::{
    ControllerFailure, ControllerHealth, ControllerLossPolicy, ControllerRecipePort, HostFailure,
    SessionError, SessionId, SessionManager, SessionRecipe, TargetHealth, TargetInstance,
    TargetInstanceId, TargetLifecyclePort,
};
pub use glyphshift_session::{
    HostOperationFailure, RuntimeFontOutcome, RuntimeTextOutcome, RuntimeTraceBatch,
    RuntimeTraceRecord, RuntimeTraceStatus,
};
use glyphshift_target_process_host::{RuntimeArtifact, TargetArtifactCatalog, TargetProcessHost};
#[cfg(test)]
use pool::RuntimeFactory;
pub use pool::{DesktopRuntimePool, DesktopRuntimeStatus, WorkflowReconcileReport};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use target::ManagedRuntime;
pub use target::{DesktopRuntime, RuntimeTarget, WindowsDesktopRuntime};

const ISOLATED_WORKER_TIMEOUT: Duration = Duration::from_secs(2);
const ACQUISITION_WORKER_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopRuntimeError {
    BundleUnavailable,
    InvalidManifest,
    InvalidArtifactPath,
    InvalidArtifactHash,
    ArtifactHashMismatch,
    AdapterInspectionFailed,
    AdapterRegistryRejected,
    ControllerRejected,
    ControllerUnavailable,
    ProtocolRejected,
    UnknownTarget,
    AcquisitionWorkerUnavailable,
    InvalidState,
    SessionRejected,
    ActivationRejected(HostOperationFailure),
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
