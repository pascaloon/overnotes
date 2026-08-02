//! Canvas geometry shared by the viewport, the selection frame and the
//! pointer state machine: camera limits, minimum object size, the resize
//! handle layout, and the resize/rotate math.

use crate::store::CanvasObject;

pub const MIN_ZOOM: f64 = 0.2;
pub const MAX_ZOOM: f64 = 4.0;
pub const MIN_W: f64 = 40.0;
pub const MIN_H: f64 = 30.0;

/// `(direction, fx, fy, cursor)` for the eight resize handles, where `fx`/`fy`
/// are fractions of the resized rect's width/height.
pub const RESIZE_DIRS: [(&str, f64, f64, &str); 8] = [
    ("nw", 0.0, 0.0, "nwse-resize"),
    ("n", 0.5, 0.0, "ns-resize"),
    ("ne", 1.0, 0.0, "nesw-resize"),
    ("e", 1.0, 0.5, "ew-resize"),
    ("se", 1.0, 1.0, "nwse-resize"),
    ("s", 0.5, 1.0, "ns-resize"),
    ("sw", 0.0, 1.0, "nesw-resize"),
    ("w", 0.0, 0.5, "ew-resize"),
];

/// Rotate a vector by `-angle_deg` (world -> object-local space).
pub fn to_local(dx: f64, dy: f64, angle_deg: f64) -> (f64, f64) {
    let a = angle_deg.to_radians();
    (dx * a.cos() + dy * a.sin(), -dx * a.sin() + dy * a.cos())
}

/// Uniform scale implied by a drag on `dir`, ignoring the axis the handle
/// cannot control.
fn aspect_scale(dir: &str, w: f64, h: f64, ow: f64, oh: f64) -> f64 {
    let min_scale = (MIN_W / ow).max(MIN_H / oh);
    let sx = (w / ow).max(min_scale);
    let sy = (h / oh).max(min_scale);

    if dir == "e" || dir == "w" {
        sx
    } else if dir == "n" || dir == "s" {
        sy
    } else if (sx - 1.0).abs() >= (sy - 1.0).abs() {
        sx
    } else {
        sy
    }
}

/// Apply a handle drag of `(dx, dy)` to `orig`, optionally keeping
/// `aspect_ratio` (width / height). Deltas are in the rect's own space, so
/// rotated objects must pass locally rotated deltas (see [`to_local`]).
pub fn resized_bounds(
    dir: &str,
    dx: f64,
    dy: f64,
    orig: (f64, f64, f64, f64),
    aspect_ratio: Option<f64>,
) -> (f64, f64, f64, f64) {
    let (ox, oy, ow, oh) = orig;
    let mut x = ox;
    let mut y = oy;
    let mut w = ow;
    let mut h = oh;

    if dir.contains('e') {
        w = (ow + dx).max(MIN_W);
    }
    if dir.contains('w') {
        let dx = dx.min(ow - MIN_W);
        x = ox + dx;
        w = ow - dx;
    }
    if dir.contains('s') {
        h = (oh + dy).max(MIN_H);
    }
    if dir.contains('n') {
        let dy = dy.min(oh - MIN_H);
        y = oy + dy;
        h = oh - dy;
    }

    if let Some(ratio) = aspect_ratio {
        let scale = aspect_scale(dir, w, h, ow, oh);
        w = (ow * scale).max(MIN_W);
        h = (oh * scale).max(MIN_H);
        if h > w / ratio {
            w = h * ratio;
        } else {
            h = w / ratio;
        }

        if dir.contains('w') {
            x = ox + ow - w;
        } else if !dir.contains('e') {
            x = ox + (ow - w) / 2.0;
        } else {
            x = ox;
        }

        if dir.contains('n') {
            y = oy + oh - h;
        } else if !dir.contains('s') {
            y = oy + (oh - h) / 2.0;
        } else {
            y = oy;
        }
    }

    (x, y, w, h)
}

/// Axis-aligned bounds of the given ids, as `(x, y, w, h)`.
pub fn selection_bounds(objects: &[CanvasObject], ids: &[u64]) -> Option<(f64, f64, f64, f64)> {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    let mut found = false;

    for obj in objects.iter().filter(|obj| ids.contains(&obj.id)) {
        min_x = min_x.min(obj.x);
        min_y = min_y.min(obj.y);
        max_x = max_x.max(obj.x + obj.w);
        max_y = max_y.max(obj.y + obj.h);
        found = true;
    }

    found.then_some((min_x, min_y, max_x - min_x, max_y - min_y))
}

/// Pointer angle in degrees around `center`.
pub fn angle_at(center: (f64, f64), point: (f64, f64)) -> f64 {
    (point.1 - center.1).atan2(point.0 - center.0).to_degrees()
}

/// Normalize to `0..360` and snap to the nearest cardinal angle when close.
pub fn snap_rotation(rotation: f64) -> f64 {
    let snapped = (rotation / 90.0).round() * 90.0;
    let rotation = if (rotation - snapped).abs() < 4.0 {
        snapped
    } else {
        rotation
    };
    rotation.rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_resize_grows_from_the_dragged_edge() {
        let (x, y, w, h) = resized_bounds("se", 50.0, 20.0, (10.0, 10.0, 100.0, 100.0), None);
        assert_eq!((x, y, w, h), (10.0, 10.0, 150.0, 120.0));

        let (x, y, w, _) = resized_bounds("w", 30.0, 0.0, (10.0, 10.0, 100.0, 100.0), None);
        assert_eq!((x, y, w), (40.0, 10.0, 70.0));
    }

    #[test]
    fn resize_never_shrinks_below_the_minimum() {
        let (_, _, w, h) = resized_bounds("se", -500.0, -500.0, (0.0, 0.0, 100.0, 100.0), None);
        assert_eq!((w, h), (MIN_W, MIN_H));
    }

    #[test]
    fn locked_ratio_resize_keeps_the_source_ratio() {
        let orig = (0.0, 0.0, 200.0, 100.0);
        let (_, _, w, h) = resized_bounds("se", 100.0, 0.0, orig, Some(2.0));
        assert!((w / h - 2.0).abs() < 1e-9);
        assert_eq!(w, 300.0);
    }

    #[test]
    fn rotation_snaps_near_cardinals_and_wraps() {
        assert_eq!(snap_rotation(88.0), 90.0);
        assert_eq!(snap_rotation(45.0), 45.0);
        assert_eq!(snap_rotation(-10.0), 350.0);
    }

    #[test]
    fn local_deltas_undo_the_object_rotation() {
        let (dx, dy) = to_local(0.0, 10.0, 90.0);
        assert!((dx - 10.0).abs() < 1e-9);
        assert!(dy.abs() < 1e-9);
    }
}
