use crate::{OcrInputFrame, OcrTextBlock, TargetFrame};
use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionCandidate, AcquisitionError, AcquisitionRequest,
    AuthorizedTarget, Granularity, InteractiveSelection, Provenance,
};

pub trait FrameSource {
    fn capture(&mut self, target: &AuthorizedTarget) -> Result<TargetFrame, AcquisitionError>;
}

pub trait OcrEngine {
    fn recognize(&mut self, frame: &OcrInputFrame) -> Result<Vec<OcrTextBlock>, AcquisitionError>;
}

pub struct VisualOcrAcquisitionAdapter<F, O> {
    frames: F,
    ocr: O,
}

impl<F, O> VisualOcrAcquisitionAdapter<F, O> {
    #[must_use]
    pub const fn new(frames: F, ocr: O) -> Self {
        Self { frames, ocr }
    }
}

impl<F, O> AcquisitionAdapter for VisualOcrAcquisitionAdapter<F, O>
where
    F: FrameSource,
    O: OcrEngine,
{
    fn provenance(&self) -> Provenance {
        Provenance::Visual
    }

    fn acquire(
        &mut self,
        request: &AcquisitionRequest,
    ) -> Result<Vec<AcquisitionCandidate>, AcquisitionError> {
        let InteractiveSelection::Region(selection) = request.selection() else {
            return Err(AcquisitionError::ProviderUnavailable);
        };
        let frame = self.frames.capture(request.target())?;
        let (origin, crop) = frame.crop(request.target(), selection)?;
        let blocks = self.ocr.recognize(&crop)?;
        let candidates = blocks
            .into_iter()
            .filter_map(|block| {
                let (source, bounds, confidence) = block.into_parts();
                let anchor = bounds.to_desktop(origin, &crop)?;
                let candidate = AcquisitionCandidate::new(source, [anchor], Granularity::Region);
                Some(match confidence {
                    Some(confidence) => candidate.with_confidence(confidence),
                    None => candidate,
                })
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            Err(AcquisitionError::NoText)
        } else {
            Ok(candidates)
        }
    }
}
