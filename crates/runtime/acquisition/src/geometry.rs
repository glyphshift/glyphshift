const DESKTOP_DPI: i64 = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DesktopPoint {
    x: i32,
    y: i32,
}

impl DesktopPoint {
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub const fn x(self) -> i32 {
        self.x
    }

    #[must_use]
    pub const fn y(self) -> i32 {
        self.y
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DesktopRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl DesktopRect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Result<Self, GeometryError> {
        if left >= right || top >= bottom {
            return Err(GeometryError::EmptyRect);
        }
        Ok(Self {
            left,
            top,
            right,
            bottom,
        })
    }

    #[must_use]
    pub const fn left(self) -> i32 {
        self.left
    }

    #[must_use]
    pub const fn top(self) -> i32 {
        self.top
    }

    #[must_use]
    pub const fn right(self) -> i32 {
        self.right
    }

    #[must_use]
    pub const fn bottom(self) -> i32 {
        self.bottom
    }

    #[must_use]
    pub const fn contains(self, point: DesktopPoint) -> bool {
        point.x >= self.left && point.x < self.right && point.y >= self.top && point.y < self.bottom
    }

    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        Self::new(
            self.left.max(other.left),
            self.top.max(other.top),
            self.right.min(other.right),
            self.bottom.min(other.bottom),
        )
        .ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalPoint {
    x: i32,
    y: i32,
}

impl LogicalPoint {
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl LogicalRect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Result<Self, GeometryError> {
        if left >= right || top >= bottom {
            return Err(GeometryError::EmptyRect);
        }
        Ok(Self {
            left,
            top,
            right,
            bottom,
        })
    }
}

/// Maps viewport-local device-independent pixels to virtual-desktop physical pixels.
///
/// `scroll` is expressed in the same local DIP coordinate space. Updating the origin or scroll
/// creates a new geometry snapshot, so anchors cannot silently survive window movement or scroll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceGeometry {
    desktop_origin: DesktopPoint,
    dpi_x: u32,
    dpi_y: u32,
    scroll: LogicalPoint,
}

impl SurfaceGeometry {
    pub fn new(
        desktop_origin: DesktopPoint,
        dpi_x: u32,
        dpi_y: u32,
    ) -> Result<Self, GeometryError> {
        if dpi_x == 0 || dpi_y == 0 {
            return Err(GeometryError::InvalidDpi);
        }
        Ok(Self {
            desktop_origin,
            dpi_x,
            dpi_y,
            scroll: LogicalPoint::new(0, 0),
        })
    }

    #[must_use]
    pub const fn with_scroll(mut self, scroll: LogicalPoint) -> Self {
        self.scroll = scroll;
        self
    }

    pub fn map_point(self, point: LogicalPoint) -> Result<DesktopPoint, GeometryError> {
        let x = map_floor(
            point
                .x
                .checked_sub(self.scroll.x)
                .ok_or(GeometryError::Overflow)?,
            self.dpi_x,
            self.desktop_origin.x,
        )?;
        let y = map_floor(
            point
                .y
                .checked_sub(self.scroll.y)
                .ok_or(GeometryError::Overflow)?,
            self.dpi_y,
            self.desktop_origin.y,
        )?;
        Ok(DesktopPoint::new(x, y))
    }

    pub fn map_rect(self, rect: LogicalRect) -> Result<DesktopRect, GeometryError> {
        let left = checked_local(rect.left, self.scroll.x)?;
        let top = checked_local(rect.top, self.scroll.y)?;
        let right = checked_local(rect.right, self.scroll.x)?;
        let bottom = checked_local(rect.bottom, self.scroll.y)?;
        let left = map_floor(left, self.dpi_x, self.desktop_origin.x)?;
        let top = map_floor(top, self.dpi_y, self.desktop_origin.y)?;
        let right = map_ceil(right, self.dpi_x, self.desktop_origin.x)?;
        let bottom = map_ceil(bottom, self.dpi_y, self.desktop_origin.y)?;
        DesktopRect::new(left, top, right, bottom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryError {
    EmptyRect,
    InvalidDpi,
    Overflow,
}

fn map_floor(local: i32, dpi: u32, origin: i32) -> Result<i32, GeometryError> {
    let scaled = i64::from(local)
        .checked_mul(i64::from(dpi))
        .ok_or(GeometryError::Overflow)?
        .div_euclid(DESKTOP_DPI);
    add_origin(scaled, origin)
}

fn map_ceil(local: i32, dpi: u32, origin: i32) -> Result<i32, GeometryError> {
    let scaled = i64::from(local)
        .checked_mul(i64::from(dpi))
        .ok_or(GeometryError::Overflow)?;
    let scaled = -(-scaled).div_euclid(DESKTOP_DPI);
    add_origin(scaled, origin)
}

fn add_origin(scaled: i64, origin: i32) -> Result<i32, GeometryError> {
    scaled
        .checked_add(i64::from(origin))
        .and_then(|value| i32::try_from(value).ok())
        .ok_or(GeometryError::Overflow)
}

fn checked_local(value: i32, scroll: i32) -> Result<i32, GeometryError> {
    value.checked_sub(scroll).ok_or(GeometryError::Overflow)
}
