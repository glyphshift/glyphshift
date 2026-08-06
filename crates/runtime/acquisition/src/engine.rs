use crate::{
    AcquisitionCandidate, AcquisitionError, AcquisitionRequest, AcquisitionResult, Provenance,
    SourceBlock, SourcePolicy,
};

const MAX_SOURCE_UNITS: usize = 16 * 1024;
const MAX_BLOCKS: usize = 256;
const MAX_ANCHORS_PER_BLOCK: usize = 256;

pub trait AcquisitionAdapter {
    fn provenance(&self) -> Provenance;

    fn acquire(
        &mut self,
        request: &AcquisitionRequest,
    ) -> Result<Vec<AcquisitionCandidate>, AcquisitionError>;
}

#[derive(Default)]
pub struct InteractiveTextAcquisition {
    adapters: Vec<Box<dyn AcquisitionAdapter>>,
}

impl InteractiveTextAcquisition {
    #[must_use]
    pub fn new(adapters: impl IntoIterator<Item = Box<dyn AcquisitionAdapter>>) -> Self {
        Self {
            adapters: adapters.into_iter().collect(),
        }
    }

    pub fn acquire(
        &mut self,
        request: &AcquisitionRequest,
    ) -> Result<AcquisitionResult, AcquisitionError> {
        let mut blocks = Vec::new();
        let mut failures = Vec::new();
        let mut matched_adapter = false;

        for provenance in [Provenance::Structured, Provenance::Visual] {
            if !policy_allows(request.source_policy(), provenance) {
                continue;
            }
            for adapter in self
                .adapters
                .iter_mut()
                .filter(|adapter| adapter.provenance() == provenance)
            {
                matched_adapter = true;
                match adapter.acquire(request) {
                    Ok(candidates) => {
                        for candidate in candidates {
                            if blocks.len() == MAX_BLOCKS {
                                break;
                            }
                            let Some(block) = normalize(candidate, provenance) else {
                                continue;
                            };
                            if !blocks.contains(&block) {
                                blocks.push(block);
                            }
                        }
                    }
                    Err(error) if is_terminal(error) => return Err(error),
                    Err(error) => failures.push(error),
                }
            }
            if !blocks.is_empty() {
                break;
            }
        }

        if !blocks.is_empty() {
            return Ok(AcquisitionResult::new(blocks));
        }
        if !matched_adapter {
            return Err(AcquisitionError::ProviderUnavailable);
        }
        Err(select_failure(&failures))
    }
}

fn policy_allows(policy: SourcePolicy, provenance: Provenance) -> bool {
    match policy {
        SourcePolicy::Automatic => true,
        SourcePolicy::StructuredOnly => provenance == Provenance::Structured,
        SourcePolicy::VisualOnly => provenance == Provenance::Visual,
    }
}

fn is_terminal(error: AcquisitionError) -> bool {
    matches!(
        error,
        AcquisitionError::TargetMismatch
            | AcquisitionError::PermissionDenied
            | AcquisitionError::Cancelled
    )
}

fn normalize(candidate: AcquisitionCandidate, provenance: Provenance) -> Option<SourceBlock> {
    let (source, mut anchors, granularity, confidence) = candidate.into_parts();
    let source = source.replace("\r\n", "\n").replace('\r', "\n");
    let source = source.trim();
    if source.is_empty()
        || source.encode_utf16().count() > MAX_SOURCE_UNITS
        || anchors.is_empty()
        || anchors.len() > MAX_ANCHORS_PER_BLOCK
    {
        return None;
    }
    let mut unique_anchors = Vec::with_capacity(anchors.len());
    for anchor in anchors.drain(..) {
        if !unique_anchors.contains(&anchor) {
            unique_anchors.push(anchor);
        }
    }
    Some(SourceBlock::new(
        source.into(),
        unique_anchors,
        granularity,
        provenance,
        confidence,
    ))
}

fn select_failure(failures: &[AcquisitionError]) -> AcquisitionError {
    [
        AcquisitionError::PermissionDenied,
        AcquisitionError::TargetMismatch,
        AcquisitionError::TimedOut,
        AcquisitionError::Cancelled,
        AcquisitionError::ProviderUnavailable,
    ]
    .into_iter()
    .find(|candidate| failures.contains(candidate))
    .unwrap_or(AcquisitionError::NoText)
}
