use glyphshift_acquisition::{
    DesktopPoint, DesktopRect, GeometryError, LogicalPoint, LogicalRect, SurfaceGeometry,
};

fn logical_rect(left: i32, top: i32, right: i32, bottom: i32) -> LogicalRect {
    LogicalRect::new(left, top, right, bottom).expect("valid logical fixture")
}

fn desktop_rect(left: i32, top: i32, right: i32, bottom: i32) -> DesktopRect {
    DesktopRect::new(left, top, right, bottom).expect("valid desktop fixture")
}

#[test]
fn geometry_001_maps_per_monitor_dpi_into_negative_virtual_desktop_pixels() {
    let geometry = SurfaceGeometry::new(DesktopPoint::new(-1920, -180), 144, 144)
        .expect("valid monitor geometry");

    assert_eq!(
        geometry
            .map_rect(logical_rect(16, 20, 216, 60))
            .expect("mapped rectangle"),
        desktop_rect(-1896, -150, -1596, -90)
    );
}

#[test]
fn geometry_002_window_movement_and_scroll_require_a_fresh_mapping_snapshot() {
    let content = logical_rect(40, 120, 136, 152);
    let before = SurfaceGeometry::new(DesktopPoint::new(-1200, 80), 120, 120)
        .expect("first geometry")
        .with_scroll(LogicalPoint::new(0, 40));
    let after = SurfaceGeometry::new(DesktopPoint::new(300, 200), 120, 120)
        .expect("second geometry")
        .with_scroll(LogicalPoint::new(0, 80));

    assert_eq!(
        before.map_rect(content).expect("before mapping"),
        desktop_rect(-1150, 180, -1030, 220)
    );
    assert_eq!(
        after.map_rect(content).expect("after mapping"),
        desktop_rect(350, 250, 470, 290)
    );
}

#[test]
fn geometry_003_rectangles_are_top_left_inclusive_and_bottom_right_exclusive() {
    let bounds = desktop_rect(-10, -5, 10, 5);

    assert!(bounds.contains(DesktopPoint::new(-10, -5)));
    assert!(bounds.contains(DesktopPoint::new(9, 4)));
    assert!(!bounds.contains(DesktopPoint::new(10, 4)));
    assert!(!bounds.contains(DesktopPoint::new(9, 5)));
    assert_eq!(DesktopRect::new(1, 1, 1, 2), Err(GeometryError::EmptyRect));
}

#[test]
fn geometry_004_extreme_scroll_offsets_fail_without_wrapping_or_panicking() {
    let geometry = SurfaceGeometry::new(DesktopPoint::new(0, 0), 96, 96)
        .expect("valid geometry")
        .with_scroll(LogicalPoint::new(i32::MIN, i32::MIN));

    assert_eq!(
        geometry.map_point(LogicalPoint::new(i32::MAX, i32::MAX)),
        Err(GeometryError::Overflow)
    );
}
