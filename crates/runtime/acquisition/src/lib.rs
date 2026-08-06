//! One-shot acquisition of bounded text from an authorized target.
//!
//! Callers provide an authorized opaque target and a short-lived desktop selection. The module
//! owns adapter ordering, output normalization, limits, deduplication, partial success, and stable
//! failures; platform handles and provider-specific identities stay behind the adapter seam.

mod engine;
mod geometry;
mod model;

pub use engine::{AcquisitionAdapter, InteractiveTextAcquisition};
pub use geometry::{
    DesktopPoint, DesktopRect, GeometryError, LogicalPoint, LogicalRect, SurfaceGeometry,
};
pub use model::{
    AcquisitionCandidate, AcquisitionError, AcquisitionRequest, AcquisitionResult,
    AuthorizedTarget, Confidence, Granularity, InteractiveSelection, Provenance, SourceBlock,
    SourcePolicy,
};
