use crate::{TesseractEngine, WindowsGraphicsFrameSource, ACQUISITION_ADAPTER_ID};
use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionError, AcquisitionRequest, AcquisitionResult, AuthorizedTarget,
    InteractiveSelection, InteractiveTextAcquisition, SourcePolicy,
};
use glyphshift_acquisition_worker_sdk::{AcquisitionWorker, WorkerAcquisitionRequest};
use glyphshift_adapter_ocr::VisualOcrAcquisitionAdapter;
use glyphshift_worker_process_grant::{authorize_process_target, ProcessGrantError};

const LOCAL_TARGET: &str = "worker-authorized-target";
const TESSERACT_LIBRARY: &str = "tesseract55.dll";
const TESSERACT_LANGUAGES: &str = "chi_sim+eng";

pub struct WindowsOcrAcquisitionWorker;

impl AcquisitionWorker for WindowsOcrAcquisitionWorker {
    fn acquire(
        &mut self,
        request: &WorkerAcquisitionRequest,
    ) -> Result<AcquisitionResult, AcquisitionError> {
        let InteractiveSelection::Region(selection) = request.selection() else {
            return Err(AcquisitionError::ProviderUnavailable);
        };
        if request.source_policy() != SourcePolicy::VisualOnly {
            return Err(AcquisitionError::ProviderUnavailable);
        }
        let process = authorize_process_target(
            request.adapter_id(),
            ACQUISITION_ADAPTER_ID,
            request.target_grant().platform(),
            request.target_grant().payload(),
        )
        .map_err(map_target_error)?;
        let local_target = AuthorizedTarget::new(LOCAL_TARGET)?;
        let frames =
            WindowsGraphicsFrameSource::for_region(local_target.clone(), process, selection)?;
        let artifact_root = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(ToOwned::to_owned))
            .ok_or(AcquisitionError::ProviderUnavailable)?;
        let engine = TesseractEngine::load(
            artifact_root.join(TESSERACT_LIBRARY),
            &artifact_root,
            TESSERACT_LANGUAGES,
        )
        .map_err(|_| AcquisitionError::ProviderUnavailable)?;
        let adapter = VisualOcrAcquisitionAdapter::new(frames, engine);
        let acquisition_request = AcquisitionRequest::new(
            local_target,
            InteractiveSelection::Region(selection),
            SourcePolicy::VisualOnly,
        );
        InteractiveTextAcquisition::new([Box::new(adapter) as Box<dyn AcquisitionAdapter>])
            .acquire(&acquisition_request)
    }
}

fn map_target_error(error: ProcessGrantError) -> AcquisitionError {
    match error {
        ProcessGrantError::ActivationRejected
        | ProcessGrantError::InvalidGrant
        | ProcessGrantError::TargetChanged => AcquisitionError::TargetMismatch,
        ProcessGrantError::PermissionDenied => AcquisitionError::PermissionDenied,
    }
}
