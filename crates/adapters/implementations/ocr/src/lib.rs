//! Visual text acquisition from a Controller-authorized in-memory target frame.
//!
//! The Adapter crops a requested desktop region to the target surface before an OCR engine sees
//! any pixels. It maps valid crop-local OCR blocks back to ephemeral virtual-desktop anchors and
//! never exposes target identities or desktop coordinates to the OCR engine.

mod adapter;
mod frame;

pub use adapter::{FrameSource, OcrEngine, VisualOcrAcquisitionAdapter};
pub use frame::{FrameError, OcrInputFrame, OcrLocalRect, OcrTextBlock, TargetFrame};
