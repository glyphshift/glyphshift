//! Windows production boundary for visual OCR acquisition.
//!
//! Platform-neutral crop and OCR contracts remain in `glyphshift-adapter-ocr`. This crate owns
//! the Windows Graphics Capture resources and will host the supervised one-shot OCR worker.

#[cfg(windows)]
mod acquisition_worker;
#[cfg(windows)]
mod windows_frame;
#[cfg(windows)]
mod windows_tesseract;

#[cfg(windows)]
pub use acquisition_worker::WindowsOcrAcquisitionWorker;
#[cfg(windows)]
pub use windows_frame::WindowsGraphicsFrameSource;
#[cfg(windows)]
pub use windows_tesseract::{TesseractEngine, TesseractEngineError};

pub const ACQUISITION_ADAPTER_ID: &str = "windows.ocr.acquire";

#[cfg(not(windows))]
pub struct WindowsOcrAcquisitionWorker;

#[cfg(not(windows))]
impl glyphshift_acquisition_worker_sdk::AcquisitionWorker for WindowsOcrAcquisitionWorker {
    fn acquire(
        &mut self,
        _request: &glyphshift_acquisition_worker_sdk::WorkerAcquisitionRequest,
    ) -> Result<glyphshift_acquisition::AcquisitionResult, glyphshift_acquisition::AcquisitionError>
    {
        Err(glyphshift_acquisition::AcquisitionError::ProviderUnavailable)
    }
}
