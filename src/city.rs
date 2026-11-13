//! Experimental wireframe-city helpers for [`raster3d`](crate::raster3d): pure geometry.
//!
//! Meaning, tone and labels stay with the caller; these only place and draw
//! lines. Every drawing helper returns a [`Drawn`](crate::city::Drawn) count of the primitives it
//! *submitted* (a submitted line may be fully clipped and paint nothing), so a
//! caller can keep frame statistics without re-deriving them.
use crate::geom::Vec3;
use crate::raster::Rgb;
use crate::raster3d::{Camera, Rasterizer};

/// Primitives submitted to the rasterizer by one helper call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Drawn {
    pub lines: u32,
    pub triangles: u32,
}

impl std::ops::Add for Drawn {
    type Output = Drawn;
    fn add(self, o: Drawn) -> Drawn {
        Drawn {
            lines: self.lines + o.lines,
            triangles: self.triangles + o.triangles,
        }
    }
}
impl std::ops::AddAssign for Drawn {
    fn add_assign(&mut self, o: Drawn) {
        *self = *self + o;
    }
}

/// Lines per side of a grid: `floor(2·half_extent / step) + 1`, capped.
const MAX_GRID_LINES: u32 = 1024;

/// A ground grid on `y = 0`, from `-half_extent` to `+half_extent` on x and z,
/// one line every `step`. Line positions are computed from their index
/// (`-half_extent + i·step`), so they do not drift. Draws nothing for a
/// non-finite or non-positive `half_extent`/`step`; at most 1024 lines per axis.
pub fn grid(r: &mut Rasterizer, cam: &Camera, half_extent: f32, step: f32, rgb: Rgb) -> Drawn {
    if !(half_extent.is_finite() && step.is_finite() && half_extent > 0.0 && step > 0.0) {
        return Drawn::default();
    }
    // Clamp before the +1: `as u32` saturates for huge ratios and would overflow.
    let n = ((((2.0 * half_extent) / step).floor() as u32).min(MAX_GRID_LINES - 1)) + 1;
    for i in 0..n {
        let c = -half_extent + i as f32 * step;
        r.line(
            Vec3::new(c, 0.0, -half_extent),
            Vec3::new(c, 0.0, half_extent),
            cam,
            rgb,
        );
        r.line(
            Vec3::new(-half_extent, 0.0, c),
            Vec3::new(half_extent, 0.0, c),
            cam,
            rgb,
        );
    }
    Drawn {
        lines: 2 * n,
        triangles: 0,
    }
}

/// Whether `cam` is on the front side of `quad` (corners `TL → TR → BR → BL`,
/// as for [`Rasterizer::textured_quad`]): a half-space test of the camera
/// position against the quad's plane. From the front a texture on the quad reads
/// left to right; from behind it would be mirrored. `false` from behind, exactly
/// in the plane, or for a degenerate quad.
///
/// For a [`facade`] (front facing −z) this is `cam.position.z < plane z`.
pub fn faces_camera(quad: &[Vec3; 4], cam: &Camera) -> bool {
    let [tl, tr, _, bl] = *quad;
    // (TR − TL) × (BL − TL) points to the *back* of a quad that reads left to
    // right from the front (with raster3d's `right = up × forward`).
    let back = tr.minus(tl).cross(bl.minus(tl));
    let side = back.dot(cam.position.minus(tl));
    side.is_finite() && side > 0.0
}

/// The 12 edges of an axis-aligned box of size `(w, h, d)` centred at `center`
/// (its geometric centre).
pub fn wire_box(
    r: &mut Rasterizer,
    cam: &Camera,
    center: Vec3,
    size: (f32, f32, f32),
    rgb: Rgb,
) -> Drawn {
    let (hx, hy, hz) = (size.0 / 2.0, size.1 / 2.0, size.2 / 2.0);
    let c = |sx: f32, sy: f32, sz: f32| {
        Vec3::new(center.x + sx * hx, center.y + sy * hy, center.z + sz * hz)
    };
    let ring = |sy: f32| {
        [
            c(-1.0, sy, -1.0),
            c(1.0, sy, -1.0),
            c(1.0, sy, 1.0),
            c(-1.0, sy, 1.0),
        ]
    };
    let (bottom, top) = (ring(-1.0), ring(1.0));
    for i in 0..4 {
        r.line(bottom[i], bottom[(i + 1) % 4], cam, rgb);
        r.line(top[i], top[(i + 1) % 4], cam, rgb);
        r.line(bottom[i], top[i], cam, rgb);
    }
    Drawn {
        lines: 12,
        triangles: 0,
    }
}

/// Horizontal `w × d` rectangles (4 lines each) around `base_center`, one at
/// each height in `ys` (relative to `base_center.y`). Corners run
/// `(−w/2, −d/2) → (w/2, −d/2) → (w/2, d/2) → (−w/2, d/2)`.
pub fn floor_bands(
    r: &mut Rasterizer,
    cam: &Camera,
    base_center: Vec3,
    size: (f32, f32),
    ys: &[f32],
    rgb: Rgb,
) -> Drawn {
    let (hx, hz) = (size.0 / 2.0, size.1 / 2.0);
    for &y in ys {
        let at = |sx: f32, sz: f32| {
            Vec3::new(
                base_center.x + sx * hx,
                base_center.y + y,
                base_center.z + sz * hz,
            )
        };
        let ring = [at(-1.0, -1.0), at(1.0, -1.0), at(1.0, 1.0), at(-1.0, 1.0)];
        for i in 0..4 {
            r.line(ring[i], ring[(i + 1) % 4], cam, rgb);
        }
    }
    Drawn {
        lines: 4 * ys.len() as u32,
        triangles: 0,
    }
}

/// A `w × h` quad in the plane `z = base_center.z`, standing on `base_center`
/// (y from `base_center.y` to `+h`), its front facing −z: corners
/// `(−w/2, top) → (+w/2, top) → (+w/2, bottom) → (−w/2, bottom)` (`TL → TR →
/// BR → BL` seen from −z), ready for [`Rasterizer::textured_quad`]. An inset
/// portal is `facade(base + (0, m, −ε), w − 2m, h − 2m)`.
pub fn facade(base_center: Vec3, w: f32, h: f32) -> [Vec3; 4] {
    let (x, y, z) = (base_center.x, base_center.y, base_center.z);
    [
        Vec3::new(x - w / 2.0, y + h, z),
        Vec3::new(x + w / 2.0, y + h, z),
        Vec3::new(x + w / 2.0, y, z),
        Vec3::new(x - w / 2.0, y, z),
    ]
}

/// Slot `i` of `n` in a row centred on `origin` along +x:
/// `x = origin.x + (i − (n − 1)/2)·spacing`. `n = 0` is treated as 1.
pub fn row_slot(i: u32, n: u32, spacing: f32, origin: Vec3) -> Vec3 {
    let offset = f64::from(i) - f64::from(n.saturating_sub(1)) / 2.0;
    Vec3::new(
        origin.x + (offset * f64::from(spacing)) as f32,
        origin.y,
        origin.z,
    )
}

/// Slot `i` of a row that grows outward from `origin` alternately right and
/// left: even `i` at `+x`, odd at `−x`, distance `start + step·⌊i/2⌋`.
pub fn alternating_slot(i: u32, start: f32, step: f32, origin: Vec3) -> Vec3 {
    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
    let d = f64::from(start) + f64::from(step) * f64::from(i / 2);
    Vec3::new(origin.x + (side * d) as f32, origin.y, origin.z)
}

/// Slot `i` of a fan around the vertical axis through the origin, alternating
/// sides: `θ = ±(start + step·⌊i/2⌋)` (even `i` positive), position
/// `(radius·sin θ, y, −radius·cos θ)`. `θ = 0` is −z; positive θ turns toward +x.
///
/// Slots are distinct only while `start + step·⌊i/2⌋ < π`: past that the two
/// sides wrap onto each other. The first index that wraps is returned by
/// [`fan_wrap_index`]; nothing here prevents callers' objects from touching.
pub fn fan_slot(i: u32, radius: f32, start: f32, step: f32, y: f32) -> Vec3 {
    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
    let theta = side * (f64::from(start) + f64::from(step) * f64::from(i / 2));
    let r = f64::from(radius);
    Vec3::new((r * theta.sin()) as f32, y, (-r * theta.cos()) as f32)
}

/// The smallest `i` whose [`fan_slot`] angle reaches π (so it can coincide with
/// or pass a slot on the other side); `None` if `step ≤ 0` and `start < π`.
pub fn fan_wrap_index(start: f32, step: f32) -> Option<u32> {
    let (start, step) = (f64::from(start), f64::from(step));
    if start >= std::f64::consts::PI {
        return Some(0);
    }
    if step.is_nan() || step <= 0.0 {
        return None;
    }
    let k = ((std::f64::consts::PI - start) / step).ceil();
    Some((2.0 * k) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam(position: Vec3) -> Camera {
        Camera {
            position,
            target: Vec3::new(0.0, 0.0, 0.0),
            ..Camera::default()
        }
    }

    /// A quad in z = 0; which side is its front is decided by projection in the
    /// test below, not assumed.
    fn quad() -> [Vec3; 4] {
        [
            Vec3::new(-1.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(-1.0, -1.0, 0.0),
        ]
    }

    #[test]
    fn faces_camera_agrees_with_projected_reading_direction() {
        let q = quad();
        for position in [
            Vec3::new(0.0, 0.0, -5.0),
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(2.0, 1.0, -4.0),
            Vec3::new(-3.0, -1.0, 4.0),
            Vec3::new(4.0, 0.5, 0.5),
        ] {
            let c = cam(position);
            let tl = c.project(q[0], 80, 48).unwrap();
            let tr = c.project(q[1], 80, 48).unwrap();
            let unmirrored = tl.0 < tr.0;
            assert_eq!(faces_camera(&q, &c), unmirrored, "{position:?}");
        }
    }

    #[test]
    fn faces_camera_is_false_edge_on_and_degenerate() {
        let q = quad();
        assert!(!faces_camera(&q, &cam(Vec3::new(5.0, 0.0, 0.0))));
        let flat = [Vec3::new(0.0, 0.0, 0.0); 4];
        assert!(!faces_camera(&flat, &cam(Vec3::new(0.0, 0.0, -5.0))));
    }

    #[test]
    fn citadel_facade_and_its_half_space_rule() {
        // Building (3.2, 2.4, 2.0) on the origin: front face at z = −1.
        let f = facade(Vec3::new(0.0, 0.0, -1.0), 3.2, 2.4);
        assert_eq!(f[0], Vec3::new(-1.6, 2.4, -1.0));
        assert_eq!(f[2], Vec3::new(1.6, 0.0, -1.0));
        for z in [-12.0, -8.2, -5.0, -1.0001] {
            let c = cam(Vec3::new(0.7, 3.0, z));
            assert!(faces_camera(&f, &c), "{z}");
        }
        for z in [-0.9999, 0.0, 5.0] {
            assert!(!faces_camera(&f, &cam(Vec3::new(0.0, 3.0, z))), "{z}");
        }
    }

    #[test]
    fn slots_follow_the_published_layouts() {
        // Actions fan: θ = ±(1.35 + 0.42·⌊i/2⌋), r = 3.3, y = 0.45.
        let s0 = fan_slot(0, 3.3, 1.35, 0.42, 0.45);
        // Same arithmetic as the slot: f32 inputs widened to f64.
        let (r, a) = (f64::from(3.3f32), f64::from(1.35f32));
        let want = Vec3::new((r * a.sin()) as f32, 0.45, (-r * a.cos()) as f32);
        assert_eq!(s0, want);
        assert_eq!(fan_slot(1, 3.3, 1.35, 0.42, 0.45).x, -s0.x);
        assert_eq!(fan_wrap_index(1.35, 0.42), Some(10));
        let a = fan_slot(9, 3.3, 1.35, 0.42, 0.45);
        let b = fan_slot(8, 3.3, 1.35, 0.42, 0.45);
        assert!(a.minus(b).length() > 0.1, "distinct before the wrap");
        // Gates: spacing 1.5 at (0, 0, −2.7).
        let o = Vec3::new(0.0, 0.0, -2.7);
        assert_eq!(row_slot(0, 3, 1.5, o), Vec3::new(-1.5, 0.0, -2.7));
        assert_eq!(row_slot(1, 3, 1.5, o), o);
        assert_eq!(row_slot(0, 0, 1.5, o), o);
        // Ships: x = ±(1.9 + 0.5·⌊i/2⌋) at (0, 3.7, −1.4).
        let s = Vec3::new(0.0, 3.7, -1.4);
        assert_eq!(alternating_slot(0, 1.9, 0.5, s).x, 1.9);
        assert_eq!(alternating_slot(3, 1.9, 0.5, s).x, -2.4);
    }

    #[test]
    fn box_and_band_counts() {
        let mut r = Rasterizer::new(40, 40);
        let c = cam(Vec3::new(0.0, 6.0, -10.0));
        let b = wire_box(
            &mut r,
            &c,
            Vec3::new(0.0, 1.2, 0.0),
            (3.2, 2.4, 2.0),
            (255, 255, 255),
        );
        let f = floor_bands(
            &mut r,
            &c,
            Vec3::new(0.0, 0.0, 0.0),
            (3.2, 2.0),
            &[1.0, 2.0],
            (255, 255, 255),
        );
        assert_eq!(
            b + f,
            Drawn {
                lines: 20,
                triangles: 0
            }
        );
        let painted = (0..40)
            .flat_map(|y| (0..40).map(move |x| (x, y)))
            .filter(|&(x, y)| r.depth(x, y).is_some_and(f32::is_finite))
            .count();
        assert!(painted > 0, "lines reached the raster");
    }

    #[test]
    fn grid_counts_lines_from_index() {
        let mut r = Rasterizer::new(40, 40);
        let c = cam(Vec3::new(0.0, 8.0, -10.0));
        // ±6 step 1.5 → 9 lines per axis.
        assert_eq!(
            grid(&mut r, &c, 6.0, 1.5, (0, 255, 0)),
            Drawn {
                lines: 18,
                triangles: 0
            }
        );
        assert_eq!(grid(&mut r, &c, 6.0, 0.0, (0, 255, 0)), Drawn::default());
        assert_eq!(
            grid(&mut r, &c, f32::NAN, 1.0, (0, 255, 0)),
            Drawn::default()
        );
        assert_eq!(grid(&mut r, &c, 1e9, 1e-9, (0, 255, 0)).lines, 2 * 1024);
    }

    #[test]
    fn camera_basis_matches_projection_axes() {
        let c = cam(Vec3::new(3.0, 2.0, -6.0));
        let b = c.basis().unwrap();
        let (x0, y0, _) = c.project(c.target, 80, 48).unwrap();
        let (xr, _, _) = c
            .project(c.target.plus(b.right.scale(0.1)), 80, 48)
            .unwrap();
        let (_, yu, _) = c.project(c.target.plus(b.up.scale(0.1)), 80, 48).unwrap();
        assert!(xr > x0, "right moves right on screen");
        assert!(yu < y0, "up moves up on screen (rows grow down)");
        assert!((b.forward.dot(b.right)).abs() < 1e-6);
        assert!((b.forward.length() - 1.0).abs() < 1e-6);
        assert!(cam(Vec3::new(0.0, 0.0, 0.0)).basis().is_none());
    }
}
