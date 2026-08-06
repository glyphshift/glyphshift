use glyphshift_acquisition::{
    AcquisitionError, AuthorizedTarget, Confidence, DesktopPoint, DesktopRect,
};

const RGBA_CHANNELS: usize = 4;
const MAX_FRAME_DIMENSION: usize = 16 * 1024;
const MAX_FRAME_PIXELS: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetFrameAccess {
    Public,
    Protected,
}

/// One ephemeral RGBA8 snapshot of an authorized target surface.
///
/// The type deliberately implements neither `Clone` nor `Debug`, reducing accidental duplication
/// or logging of captured pixels. A frame source must create a fresh value for each request.
pub struct TargetFrame {
    target: AuthorizedTarget,
    desktop_bounds: DesktopRect,
    access: TargetFrameAccess,
    rgba8: Vec<u8>,
    width: usize,
}

impl TargetFrame {
    pub fn new(
        target: AuthorizedTarget,
        desktop_bounds: DesktopRect,
        rgba8: Vec<u8>,
    ) -> Result<Self, FrameError> {
        let (width, _, expected_bytes) = dimensions(desktop_bounds)?;
        if rgba8.len() != expected_bytes {
            return Err(FrameError::InvalidPixelBuffer);
        }
        Ok(Self {
            target,
            desktop_bounds,
            access: TargetFrameAccess::Public,
            rgba8,
            width,
        })
    }

    pub fn protected(
        target: AuthorizedTarget,
        desktop_bounds: DesktopRect,
    ) -> Result<Self, FrameError> {
        let (width, _, _) = dimensions(desktop_bounds)?;
        Ok(Self {
            target,
            desktop_bounds,
            access: TargetFrameAccess::Protected,
            rgba8: Vec::new(),
            width,
        })
    }

    pub(crate) fn crop(
        &self,
        target: &AuthorizedTarget,
        selection: DesktopRect,
    ) -> Result<(DesktopPoint, OcrInputFrame), AcquisitionError> {
        if &self.target != target {
            return Err(AcquisitionError::TargetMismatch);
        }
        if self.access == TargetFrameAccess::Protected {
            return Err(AcquisitionError::PermissionDenied);
        }
        let crop = self
            .desktop_bounds
            .intersection(selection)
            .ok_or(AcquisitionError::TargetMismatch)?;
        let crop_width = rect_width(crop).ok_or(AcquisitionError::ProviderUnavailable)?;
        let crop_height = rect_height(crop).ok_or(AcquisitionError::ProviderUnavailable)?;
        let x_offset = usize::try_from(crop.left() - self.desktop_bounds.left())
            .map_err(|_| AcquisitionError::ProviderUnavailable)?;
        let y_offset = usize::try_from(crop.top() - self.desktop_bounds.top())
            .map_err(|_| AcquisitionError::ProviderUnavailable)?;
        let row_bytes = crop_width
            .checked_mul(RGBA_CHANNELS)
            .ok_or(AcquisitionError::ProviderUnavailable)?;
        let capacity = row_bytes
            .checked_mul(crop_height)
            .ok_or(AcquisitionError::ProviderUnavailable)?;
        let mut pixels = Vec::with_capacity(capacity);
        for row in 0..crop_height {
            let pixel_offset = (y_offset + row)
                .checked_mul(self.width)
                .and_then(|value| value.checked_add(x_offset))
                .ok_or(AcquisitionError::ProviderUnavailable)?;
            let start = pixel_offset
                .checked_mul(RGBA_CHANNELS)
                .ok_or(AcquisitionError::ProviderUnavailable)?;
            let end = start
                .checked_add(row_bytes)
                .ok_or(AcquisitionError::ProviderUnavailable)?;
            let source = self
                .rgba8
                .get(start..end)
                .ok_or(AcquisitionError::ProviderUnavailable)?;
            pixels.extend_from_slice(source);
        }
        Ok((
            DesktopPoint::new(crop.left(), crop.top()),
            OcrInputFrame {
                rgba8: pixels,
                width: crop_width,
                height: crop_height,
            },
        ))
    }
}

/// Cropped RGBA8 pixels with no target identity or desktop geometry.
/// Implementations must not retain or persist the borrowed pixels after `recognize` returns.
pub struct OcrInputFrame {
    rgba8: Vec<u8>,
    width: usize,
    height: usize,
}

impl OcrInputFrame {
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn rgba8(&self) -> &[u8] {
        &self.rgba8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OcrLocalRect {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl OcrLocalRect {
    pub const fn new(left: u32, top: u32, right: u32, bottom: u32) -> Option<Self> {
        if left >= right || top >= bottom {
            return None;
        }
        Some(Self {
            left,
            top,
            right,
            bottom,
        })
    }

    pub(crate) fn to_desktop(
        self,
        origin: DesktopPoint,
        frame: &OcrInputFrame,
    ) -> Option<DesktopRect> {
        if usize::try_from(self.right).ok()? > frame.width
            || usize::try_from(self.bottom).ok()? > frame.height
        {
            return None;
        }
        let left = origin.x().checked_add(i32::try_from(self.left).ok()?)?;
        let top = origin.y().checked_add(i32::try_from(self.top).ok()?)?;
        let right = origin.x().checked_add(i32::try_from(self.right).ok()?)?;
        let bottom = origin.y().checked_add(i32::try_from(self.bottom).ok()?)?;
        DesktopRect::new(left, top, right, bottom).ok()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OcrTextBlock {
    source: Box<str>,
    bounds: OcrLocalRect,
    confidence: Option<Confidence>,
}

impl OcrTextBlock {
    #[must_use]
    pub fn new(source: impl Into<Box<str>>, bounds: OcrLocalRect) -> Self {
        Self {
            source: source.into(),
            bounds,
            confidence: None,
        }
    }

    #[must_use]
    pub const fn with_confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = Some(confidence);
        self
    }

    pub(crate) fn into_parts(self) -> (Box<str>, OcrLocalRect, Option<Confidence>) {
        (self.source, self.bounds, self.confidence)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    InvalidPixelBuffer,
    FrameTooLarge,
}

fn dimensions(bounds: DesktopRect) -> Result<(usize, usize, usize), FrameError> {
    let width = rect_width(bounds).ok_or(FrameError::FrameTooLarge)?;
    let height = rect_height(bounds).ok_or(FrameError::FrameTooLarge)?;
    let pixels = width
        .checked_mul(height)
        .filter(|pixels| *pixels <= MAX_FRAME_PIXELS)
        .ok_or(FrameError::FrameTooLarge)?;
    let bytes = pixels
        .checked_mul(RGBA_CHANNELS)
        .ok_or(FrameError::FrameTooLarge)?;
    Ok((width, height, bytes))
}

fn rect_width(bounds: DesktopRect) -> Option<usize> {
    usize::try_from(i64::from(bounds.right()) - i64::from(bounds.left()))
        .ok()
        .filter(|value| *value <= MAX_FRAME_DIMENSION)
}

fn rect_height(bounds: DesktopRect) -> Option<usize> {
    usize::try_from(i64::from(bounds.bottom()) - i64::from(bounds.top()))
        .ok()
        .filter(|value| *value <= MAX_FRAME_DIMENSION)
}
