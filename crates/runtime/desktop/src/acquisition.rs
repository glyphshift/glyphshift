use super::*;
use crate::bundle::AcquisitionWorkerCatalog;

#[derive(Clone, Default)]
pub struct DesktopAcquisitionCancellation(CancellationToken);

impl DesktopAcquisitionCancellation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.cancel();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }

    pub(super) const fn worker_token(&self) -> &CancellationToken {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopAcquisitionError {
    UnknownTarget,
    InvalidState,
    ControllerUnavailable,
    TargetUnavailable,
    WorkerUnavailable,
    PermissionDenied,
    NoText,
    ProviderUnavailable,
    TimedOut,
    Cancelled,
}

pub(super) trait AcquisitionExecutor: Send {
    fn acquire(
        &self,
        adapter_id: &str,
        target: AuthorizedTarget,
        grant: ControllerWorkerTargetGrant,
        selection: InteractiveSelection,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError>;
}

impl AcquisitionExecutor for AcquisitionWorkerCatalog {
    fn acquire(
        &self,
        adapter_id: &str,
        target: AuthorizedTarget,
        grant: ControllerWorkerTargetGrant,
        selection: InteractiveSelection,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        let host = self
            .host(adapter_id)
            .map_err(|_| DesktopAcquisitionError::WorkerUnavailable)?;
        let binding = AcquisitionWorkerBinding::new(
            target.clone(),
            adapter_id,
            grant.platform(),
            grant.payload(),
        )
        .map_err(map_acquisition_host_error)?;
        let request = AcquisitionRequest::new(target, selection, source_policy_for(selection));
        host.acquire(&binding, &request, cancellation.worker_token())
            .map_err(map_acquisition_host_error)
    }
}

pub(super) const fn source_policy_for(selection: InteractiveSelection) -> SourcePolicy {
    match selection {
        InteractiveSelection::Point(_) | InteractiveSelection::TextRange { .. } => {
            SourcePolicy::StructuredOnly
        }
        InteractiveSelection::Region(_) => SourcePolicy::VisualOnly,
    }
}

pub(super) fn map_acquisition_protocol_error(
    error: ControllerProtocolError,
) -> DesktopAcquisitionError {
    match error {
        ControllerProtocolError::UnknownTarget(_) => DesktopAcquisitionError::TargetUnavailable,
        ControllerProtocolError::Transport(glyphshift_protocol::TransportFailure::Timeout) => {
            DesktopAcquisitionError::TimedOut
        }
        ControllerProtocolError::Transport(glyphshift_protocol::TransportFailure::Cancelled) => {
            DesktopAcquisitionError::Cancelled
        }
        ControllerProtocolError::Transport(glyphshift_protocol::TransportFailure::Rejected(_)) => {
            DesktopAcquisitionError::TargetUnavailable
        }
        _ => DesktopAcquisitionError::ControllerUnavailable,
    }
}

pub(super) const fn map_acquisition_host_error(
    error: AcquisitionWorkerHostError,
) -> DesktopAcquisitionError {
    match error {
        AcquisitionWorkerHostError::Timeout
        | AcquisitionWorkerHostError::AcquisitionRejected(
            glyphshift_acquisition::AcquisitionError::TimedOut,
        ) => DesktopAcquisitionError::TimedOut,
        AcquisitionWorkerHostError::Cancelled
        | AcquisitionWorkerHostError::AcquisitionRejected(
            glyphshift_acquisition::AcquisitionError::Cancelled,
        ) => DesktopAcquisitionError::Cancelled,
        AcquisitionWorkerHostError::AcquisitionRejected(
            glyphshift_acquisition::AcquisitionError::PermissionDenied,
        ) => DesktopAcquisitionError::PermissionDenied,
        AcquisitionWorkerHostError::AcquisitionRejected(
            glyphshift_acquisition::AcquisitionError::NoText,
        ) => DesktopAcquisitionError::NoText,
        AcquisitionWorkerHostError::AcquisitionRejected(
            glyphshift_acquisition::AcquisitionError::TargetMismatch,
        ) => DesktopAcquisitionError::TargetUnavailable,
        AcquisitionWorkerHostError::AcquisitionRejected(
            glyphshift_acquisition::AcquisitionError::ProviderUnavailable,
        ) => DesktopAcquisitionError::ProviderUnavailable,
        AcquisitionWorkerHostError::ArtifactUnavailable
        | AcquisitionWorkerHostError::InvalidBinding
        | AcquisitionWorkerHostError::SpawnFailed
        | AcquisitionWorkerHostError::Crashed
        | AcquisitionWorkerHostError::MalformedMessage => {
            DesktopAcquisitionError::WorkerUnavailable
        }
    }
}
