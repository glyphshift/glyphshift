use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionCandidate, AcquisitionError, AcquisitionRequest,
    AcquisitionResult, AuthorizedTarget, DesktopPoint, DesktopRect, Granularity,
    InteractiveSelection, InteractiveTextAcquisition, Provenance,
};
use glyphshift_acquisition_worker_sdk::{serve_stdio, AcquisitionWorker, WorkerAcquisitionRequest};

struct SyntheticAdapter {
    anchor: DesktopRect,
}

impl AcquisitionAdapter for SyntheticAdapter {
    fn provenance(&self) -> Provenance {
        Provenance::Structured
    }

    fn acquire(
        &mut self,
        _request: &AcquisitionRequest,
    ) -> Result<Vec<AcquisitionCandidate>, AcquisitionError> {
        Ok(vec![AcquisitionCandidate::new(
            "synthetic worker text",
            [self.anchor],
            Granularity::Word,
        )])
    }
}

struct SyntheticWorker;

impl AcquisitionWorker for SyntheticWorker {
    fn acquire(
        &mut self,
        request: &WorkerAcquisitionRequest,
    ) -> Result<AcquisitionResult, AcquisitionError> {
        match request.target_grant().payload() {
            "permission-denied" => return Err(AcquisitionError::PermissionDenied),
            "target-mismatch" => return Err(AcquisitionError::TargetMismatch),
            "no-text" => return Err(AcquisitionError::NoText),
            "hang" => std::thread::sleep(std::time::Duration::from_secs(10)),
            "crash" => std::process::exit(7),
            "authorized" => {}
            _ => return Err(AcquisitionError::ProviderUnavailable),
        }
        if request.adapter_id() != "synthetic.acquisition"
            || request.target_grant().platform() != "synthetic-process-v1"
        {
            return Err(AcquisitionError::TargetMismatch);
        }
        let anchor = anchor_for(request.selection())?;
        let target = AuthorizedTarget::new("worker-local-target")?;
        let acquisition_request =
            AcquisitionRequest::new(target, request.selection(), request.source_policy());
        InteractiveTextAcquisition::new([
            Box::new(SyntheticAdapter { anchor }) as Box<dyn AcquisitionAdapter>
        ])
        .acquire(&acquisition_request)
    }
}

fn anchor_for(selection: InteractiveSelection) -> Result<DesktopRect, AcquisitionError> {
    match selection {
        InteractiveSelection::Point(point) => point_anchor(point),
        InteractiveSelection::TextRange { start, end } => DesktopRect::new(
            start.x().min(end.x()),
            start.y().min(end.y()),
            start.x().max(end.x()).saturating_add(1),
            start.y().max(end.y()).saturating_add(1),
        )
        .map_err(|_| AcquisitionError::NoText),
        InteractiveSelection::Region(rect) => Ok(rect),
    }
}

fn point_anchor(point: DesktopPoint) -> Result<DesktopRect, AcquisitionError> {
    DesktopRect::new(
        point.x().saturating_sub(2),
        point.y().saturating_sub(2),
        point.x().saturating_add(3),
        point.y().saturating_add(3),
    )
    .map_err(|_| AcquisitionError::NoText)
}

fn main() -> std::io::Result<()> {
    serve_stdio(SyntheticWorker)
}
