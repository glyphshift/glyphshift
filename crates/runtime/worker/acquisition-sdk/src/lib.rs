//! Versioned one-shot protocol and authoring surface for acquisition workers.

use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionCandidate, AcquisitionError, AcquisitionRequest,
    AcquisitionResult, Confidence, DesktopPoint, DesktopRect, Granularity, InteractiveSelection,
    InteractiveTextAcquisition, Provenance, SourcePolicy,
};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Read, Write};

pub const PROTOCOL_SCHEMA: &str = "glyphshift.acquisition-worker/1";
pub const MAX_WIRE_BYTES: usize = 1024 * 1024;
const MAX_ADAPTER_ID_BYTES: usize = 256;
const MAX_GRANT_PLATFORM_BYTES: usize = 128;
const MAX_GRANT_PAYLOAD_BYTES: usize = 4 * 1024;
const MAX_BLOCKS: usize = 256;
const MAX_ANCHORS_PER_BLOCK: usize = 256;
const MAX_SOURCE_UNITS: usize = 16 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct WorkerTargetGrant {
    platform: Box<str>,
    payload: Box<str>,
}

impl WorkerTargetGrant {
    pub fn new(
        platform: impl Into<Box<str>>,
        payload: impl Into<Box<str>>,
    ) -> Result<Self, WireError> {
        let platform = platform.into();
        let payload = payload.into();
        if !valid_identifier(&platform, MAX_GRANT_PLATFORM_BYTES)
            || payload.trim().is_empty()
            || payload.len() > MAX_GRANT_PAYLOAD_BYTES
        {
            return Err(WireError::InvalidMessage);
        }
        Ok(Self { platform, payload })
    }

    #[must_use]
    pub fn platform(&self) -> &str {
        &self.platform
    }

    #[must_use]
    pub fn payload(&self) -> &str {
        &self.payload
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct WorkerAcquisitionRequest {
    adapter_id: Box<str>,
    target_grant: WorkerTargetGrant,
    selection: InteractiveSelection,
    source_policy: SourcePolicy,
}

impl WorkerAcquisitionRequest {
    #[must_use]
    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }

    #[must_use]
    pub const fn target_grant(&self) -> &WorkerTargetGrant {
        &self.target_grant
    }

    #[must_use]
    pub const fn selection(&self) -> InteractiveSelection {
        self.selection
    }

    #[must_use]
    pub const fn source_policy(&self) -> SourcePolicy {
        self.source_policy
    }
}

pub trait AcquisitionWorker {
    fn acquire(
        &mut self,
        request: &WorkerAcquisitionRequest,
    ) -> Result<AcquisitionResult, AcquisitionError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    InvalidMessage,
    UnsupportedSchema,
    LimitExceeded,
}

pub fn encode_request(
    request_id: u64,
    adapter_id: &str,
    target_grant: &WorkerTargetGrant,
    request: &AcquisitionRequest,
) -> Result<String, WireError> {
    if request_id == 0 || !valid_identifier(adapter_id, MAX_ADAPTER_ID_BYTES) {
        return Err(WireError::InvalidMessage);
    }
    encode_limited(&RequestEnvelope {
        schema: PROTOCOL_SCHEMA.into(),
        request_id,
        adapter_id: adapter_id.into(),
        target_grant: WireTargetGrant {
            platform: target_grant.platform.clone(),
            payload: target_grant.payload.clone(),
        },
        selection: request.selection().into(),
        source_policy: request.source_policy().into(),
    })
}

pub fn decode_request(input: &str) -> Result<(u64, WorkerAcquisitionRequest), WireError> {
    check_wire_size(input)?;
    let envelope: RequestEnvelope =
        serde_json::from_str(input).map_err(|_| WireError::InvalidMessage)?;
    if envelope.schema.as_ref() != PROTOCOL_SCHEMA {
        return Err(WireError::UnsupportedSchema);
    }
    if envelope.request_id == 0 || !valid_identifier(&envelope.adapter_id, MAX_ADAPTER_ID_BYTES) {
        return Err(WireError::InvalidMessage);
    }
    let target_grant = WorkerTargetGrant::new(
        envelope.target_grant.platform,
        envelope.target_grant.payload,
    )?;
    Ok((
        envelope.request_id,
        WorkerAcquisitionRequest {
            adapter_id: envelope.adapter_id,
            target_grant,
            selection: envelope.selection.try_into()?,
            source_policy: envelope.source_policy.into(),
        },
    ))
}

pub fn encode_response(
    request_id: u64,
    result: Result<&AcquisitionResult, AcquisitionError>,
) -> Result<String, WireError> {
    if request_id == 0 {
        return Err(WireError::InvalidMessage);
    }
    let response = match result {
        Ok(result) if !result.blocks().is_empty() && result.blocks().len() <= MAX_BLOCKS => {
            let blocks = result
                .blocks()
                .iter()
                .map(|block| WireBlock {
                    source: block.source().into(),
                    anchors: block
                        .anchors()
                        .iter()
                        .copied()
                        .map(WireRect::from)
                        .collect(),
                    granularity: block.granularity().into(),
                    provenance: block.provenance().into(),
                    confidence: block.confidence().map(Confidence::basis_points),
                })
                .collect();
            WireResponse::Acquired { blocks }
        }
        Ok(_) => return Err(WireError::LimitExceeded),
        Err(error) => WireResponse::Rejected {
            error: error.into(),
        },
    };
    encode_limited(&ResponseEnvelope {
        schema: PROTOCOL_SCHEMA.into(),
        request_id,
        response,
    })
}

pub fn decode_response(
    input: &str,
    expected_request_id: u64,
    request: &AcquisitionRequest,
) -> Result<Result<AcquisitionResult, AcquisitionError>, WireError> {
    check_wire_size(input)?;
    let envelope: ResponseEnvelope =
        serde_json::from_str(input).map_err(|_| WireError::InvalidMessage)?;
    if envelope.schema.as_ref() != PROTOCOL_SCHEMA || envelope.request_id != expected_request_id {
        return Err(WireError::UnsupportedSchema);
    }
    match envelope.response {
        WireResponse::Rejected { error } => Ok(Err(error.into())),
        WireResponse::Acquired { blocks } => decode_blocks(blocks, request).map(Ok),
    }
}

pub fn serve_stdio(worker: impl AcquisitionWorker) -> io::Result<()> {
    serve(
        worker,
        io::BufReader::new(io::stdin().lock()),
        io::stdout().lock(),
    )
}

pub fn serve(
    mut worker: impl AcquisitionWorker,
    mut input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()> {
    let Some(line) = read_limited_line(&mut input)? else {
        return Ok(());
    };
    let Ok((request_id, request)) = decode_request(&line) else {
        return Ok(());
    };
    let result = worker.acquire(&request);
    let encoded = match encode_response(request_id, result.as_ref().map_err(|error| *error)) {
        Ok(encoded) => encoded,
        Err(_) => encode_response(request_id, Err(AcquisitionError::ProviderUnavailable))
            .expect("bounded fallback response"),
    };
    output.write_all(encoded.as_bytes())?;
    output.write_all(b"\n")?;
    output.flush()
}

fn decode_blocks(
    blocks: Vec<WireBlock>,
    request: &AcquisitionRequest,
) -> Result<AcquisitionResult, WireError> {
    if blocks.is_empty() || blocks.len() > MAX_BLOCKS {
        return Err(WireError::LimitExceeded);
    }
    let mut structured = Vec::new();
    let mut visual = Vec::new();
    let mut result_provenance = None;
    for block in blocks {
        if block.source.trim().is_empty()
            || block.source.encode_utf16().count() > MAX_SOURCE_UNITS
            || block.anchors.is_empty()
            || block.anchors.len() > MAX_ANCHORS_PER_BLOCK
        {
            return Err(WireError::LimitExceeded);
        }
        let anchors = block
            .anchors
            .into_iter()
            .map(DesktopRect::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let mut candidate =
            AcquisitionCandidate::new(block.source, anchors, block.granularity.into());
        if let Some(confidence) = block.confidence {
            candidate = candidate
                .with_confidence(Confidence::new(confidence).ok_or(WireError::LimitExceeded)?);
        }
        let provenance = Provenance::from(block.provenance);
        if result_provenance
            .replace(provenance)
            .is_some_and(|previous| previous != provenance)
        {
            return Err(WireError::InvalidMessage);
        }
        match provenance {
            Provenance::Structured => structured.push(candidate),
            Provenance::Visual => visual.push(candidate),
        }
    }
    let mut adapters: Vec<Box<dyn AcquisitionAdapter>> = Vec::new();
    if !structured.is_empty() {
        adapters.push(Box::new(DecodedAdapter {
            provenance: Provenance::Structured,
            candidates: Some(structured),
        }));
    }
    if !visual.is_empty() {
        adapters.push(Box::new(DecodedAdapter {
            provenance: Provenance::Visual,
            candidates: Some(visual),
        }));
    }
    InteractiveTextAcquisition::new(adapters)
        .acquire(request)
        .map_err(|_| WireError::InvalidMessage)
}

struct DecodedAdapter {
    provenance: Provenance,
    candidates: Option<Vec<AcquisitionCandidate>>,
}

impl AcquisitionAdapter for DecodedAdapter {
    fn provenance(&self) -> Provenance {
        self.provenance
    }

    fn acquire(
        &mut self,
        _request: &AcquisitionRequest,
    ) -> Result<Vec<AcquisitionCandidate>, AcquisitionError> {
        Ok(self.candidates.take().unwrap_or_default())
    }
}

fn encode_limited(value: &impl Serialize) -> Result<String, WireError> {
    let encoded = serde_json::to_string(value).map_err(|_| WireError::InvalidMessage)?;
    check_wire_size(&encoded)?;
    Ok(encoded)
}

fn check_wire_size(input: &str) -> Result<(), WireError> {
    if input.len() > MAX_WIRE_BYTES {
        Err(WireError::LimitExceeded)
    } else {
        Ok(())
    }
}

fn read_limited_line(input: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    let mut limited = Read::take(input, (MAX_WIRE_BYTES + 2) as u64);
    let read = limited.read_until(b'\n', &mut bytes)?;
    if read == 0 || !bytes.ends_with(b"\n") {
        return Ok(None);
    }
    bytes.pop();
    if bytes.len() > MAX_WIRE_BYTES {
        return Ok(None);
    }
    match String::from_utf8(bytes) {
        Ok(line) => Ok(Some(line)),
        Err(_) => Ok(None),
    }
}

fn valid_identifier(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestEnvelope {
    schema: Box<str>,
    request_id: u64,
    adapter_id: Box<str>,
    target_grant: WireTargetGrant,
    selection: WireSelection,
    source_policy: WireSourcePolicy,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTargetGrant {
    platform: Box<str>,
    payload: Box<str>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireSelection {
    Point {
        x: i32,
        y: i32,
    },
    TextRange {
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    },
    Region {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    },
}

impl From<InteractiveSelection> for WireSelection {
    fn from(value: InteractiveSelection) -> Self {
        match value {
            InteractiveSelection::Point(point) => Self::Point {
                x: point.x(),
                y: point.y(),
            },
            InteractiveSelection::TextRange { start, end } => Self::TextRange {
                start_x: start.x(),
                start_y: start.y(),
                end_x: end.x(),
                end_y: end.y(),
            },
            InteractiveSelection::Region(rect) => Self::Region {
                left: rect.left(),
                top: rect.top(),
                right: rect.right(),
                bottom: rect.bottom(),
            },
        }
    }
}

impl TryFrom<WireSelection> for InteractiveSelection {
    type Error = WireError;

    fn try_from(value: WireSelection) -> Result<Self, Self::Error> {
        Ok(match value {
            WireSelection::Point { x, y } => Self::Point(DesktopPoint::new(x, y)),
            WireSelection::TextRange {
                start_x,
                start_y,
                end_x,
                end_y,
            } => Self::TextRange {
                start: DesktopPoint::new(start_x, start_y),
                end: DesktopPoint::new(end_x, end_y),
            },
            WireSelection::Region {
                left,
                top,
                right,
                bottom,
            } => Self::Region(
                DesktopRect::new(left, top, right, bottom)
                    .map_err(|_| WireError::InvalidMessage)?,
            ),
        })
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireSourcePolicy {
    Automatic,
    StructuredOnly,
    VisualOnly,
}

impl From<SourcePolicy> for WireSourcePolicy {
    fn from(value: SourcePolicy) -> Self {
        match value {
            SourcePolicy::Automatic => Self::Automatic,
            SourcePolicy::StructuredOnly => Self::StructuredOnly,
            SourcePolicy::VisualOnly => Self::VisualOnly,
        }
    }
}

impl From<WireSourcePolicy> for SourcePolicy {
    fn from(value: WireSourcePolicy) -> Self {
        match value {
            WireSourcePolicy::Automatic => Self::Automatic,
            WireSourcePolicy::StructuredOnly => Self::StructuredOnly,
            WireSourcePolicy::VisualOnly => Self::VisualOnly,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseEnvelope {
    schema: Box<str>,
    request_id: u64,
    response: WireResponse,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireResponse {
    Acquired { blocks: Vec<WireBlock> },
    Rejected { error: WireAcquisitionError },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBlock {
    source: Box<str>,
    anchors: Vec<WireRect>,
    granularity: WireGranularity,
    provenance: WireProvenance,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<u16>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl From<DesktopRect> for WireRect {
    fn from(value: DesktopRect) -> Self {
        Self {
            left: value.left(),
            top: value.top(),
            right: value.right(),
            bottom: value.bottom(),
        }
    }
}

impl TryFrom<WireRect> for DesktopRect {
    type Error = WireError;

    fn try_from(value: WireRect) -> Result<Self, Self::Error> {
        Self::new(value.left, value.top, value.right, value.bottom)
            .map_err(|_| WireError::InvalidMessage)
    }
}

macro_rules! mirrored_enum {
    ($wire:ident, $model:ty, {$($variant:ident),+ $(,)?}) => {
        #[derive(Clone, Copy, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum $wire { $($variant),+ }

        impl From<$model> for $wire {
            fn from(value: $model) -> Self {
                match value { $(<$model>::$variant => Self::$variant),+ }
            }
        }

        impl From<$wire> for $model {
            fn from(value: $wire) -> Self {
                match value { $($wire::$variant => Self::$variant),+ }
            }
        }
    };
}

mirrored_enum!(WireGranularity, Granularity, { Word, Control, Line, Region });
mirrored_enum!(WireProvenance, Provenance, { Structured, Visual });
mirrored_enum!(WireAcquisitionError, AcquisitionError, {
    TargetMismatch,
    PermissionDenied,
    NoText,
    ProviderUnavailable,
    TimedOut,
    Cancelled,
});

#[cfg(test)]
mod tests;
