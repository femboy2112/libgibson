//! The stage: a ring of outward-facing media planes in 3-D, and the moving camera
//! that orbits it. In a slideshow the frame *is* the content; here the content is a
//! set of objects fixed in a world and the **camera has the opinion**. The grammars
//! and plots are perspective-mapped textured planes on a ring around a luminous
//! core. For each panel the camera dollies straight in on its frontal face, holds,
//! and dollies straight back out before spinning to the next — and while the panel's
//! projected rectangle is large and frontal the driver resolves the *real* UI
//! Surface into that rectangle (the intro cinema's facade law), so the panel grows
//! coarse→crisp→coarse continuously with no cut.
//!
//! Only the public `raster3d` primitives are used — `Camera`, `Rasterizer`,
//! `textured_quad` (perspective-correct, z-buffered), `Camera::project`. Plane
//! geometry, the camera cue sheet and the compositing order are demo-local art
//! direction: a cue sheet for a camera, never a general cinematic framework (the
//! standing injunction in `docs/FRANK_REACTION_CUT_PLAN.md`).

#![allow(dead_code)]

use gibson::geom::Vec3;
use gibson::raster::RgbRaster;
use gibson::raster3d::{Camera, Rasterizer};
use gibson::Rect;

/// How many media planes ride the ring (six grammars + three observatory scenes).
pub const N_PLANES: usize = 9;

const AXIS_Z: f32 = 15.0;
const R: f32 = 7.0;
const TURNS: f32 = 1.0;
const Y_STEP: f32 = 0.42;
/// Plane size in world units (matches the output aspect so a frontal plane fills
/// the frame cleanly at the closest approach).
const PW_WORLD: f32 = 4.6;
const PH_WORLD: f32 = 2.7;

// ---- the camera cue sheet, in seconds ----------------------------------------

/// Opening establishing shot.
pub const EST: f32 = 4.0;
/// Straight dolly-in from the wide pose to the close pose.
pub const TRAVEL_IN: f32 = 1.2;
/// Hold at the closest, frontal pose.
pub const HOLD: f32 = 2.2;
/// Straight dolly-out back to the wide pose (the frontal retreat that keeps the UI
/// resolving coarse again before the spin, so the hand-off back to 3-D is seamless).
pub const TRAVEL_OUT: f32 = 1.2;
/// Total time budget per panel (dolly-in + hold + dolly-out + spin to the next).
pub const PANEL: f32 = 5.8;
/// Closing reveal.
pub const REVEAL: f32 = 6.0;

/// Camera distance (from the panel face) at the close pose — tuned so the frontal
/// plane nearly fills the frame while its corners stay inside the frustum (so the
/// projected rectangle resolves cleanly).
const D_DWELL: f32 = 3.7;
/// Camera distance at the wide pose the dolly starts/ends at.
const D_WIDE: f32 = 9.5;
/// Vertical field of view at the close/hold pose (the frontal plane nearly fills).
const FOV_DWELL: f32 = 0.78;
/// Vertical field of view at the wide pose (frames the panel from outside the ring).
const FOV_WIDE: f32 = 0.98;

/// Minimum projected rectangle (in cells) for the real UI to be resolved into it.
const MIN_BW: u16 = 18;
const MIN_BH: u16 = 8;

/// Total film length: establish + every panel + reveal.
pub fn duration() -> f32 {
    EST + N_PLANES as f32 * PANEL + REVEAL
}

// ---- small Vec3 helpers (the library's Vec3 carries no operator overloads) ----

fn v_add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}
fn v_sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}
fn v_scale(a: Vec3, s: f32) -> Vec3 {
    Vec3::new(a.x * s, a.y * s, a.z * s)
}
fn v_lerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    v_add(a, v_scale(v_sub(b, a), t))
}
fn dist2(a: Vec3, b: Vec3) -> f32 {
    let d = v_sub(a, b);
    d.x * d.x + d.y * d.y + d.z * d.z
}

// ---- the ring ----------------------------------------------------------------

fn plane_angle(i: usize) -> f32 {
    use std::f32::consts::TAU;
    let base = -std::f32::consts::FRAC_PI_2;
    base + (i as f32) * (TAU * TURNS / N_PLANES as f32)
}

fn plane_y(i: usize) -> f32 {
    (i as f32 - (N_PLANES as f32 - 1.0) * 0.5) * Y_STEP
}

/// The world centre of plane `i`.
pub fn plane_center(i: usize) -> Vec3 {
    let a = plane_angle(i);
    Vec3::new(R * a.cos(), plane_y(i), AXIS_Z + R * a.sin())
}

fn axis_at(y: f32) -> Vec3 {
    Vec3::new(0.0, y, AXIS_Z)
}

/// The outward (horizontal) unit normal of plane `i` — it faces away from the ring
/// axis, so a camera orbiting outside the ring sees its face.
fn plane_outward(i: usize) -> Vec3 {
    let c = plane_center(i);
    v_sub(c, axis_at(c.y)).normalize()
}

fn plane_basis(i: usize) -> (Vec3, Vec3) {
    let up = Vec3::new(0.0, 1.0, 0.0);
    let right = plane_outward(i).cross(up).normalize();
    (right, up)
}

/// The four world corners of plane `i`, in TL → TR → BR → BL order (matching the
/// `textured_quad` UV winding) as seen from outside the ring.
pub fn plane_corners(i: usize) -> [Vec3; 4] {
    let c = plane_center(i);
    let (right, up) = plane_basis(i);
    let hw = v_scale(right, PW_WORLD * 0.5);
    let hh = v_scale(up, PH_WORLD * 0.5);
    [
        v_add(v_sub(c, hw), hh), // TL
        v_add(v_add(c, hw), hh), // TR
        v_sub(v_add(c, hw), hh), // BR
        v_sub(v_sub(c, hw), hh), // BL
    ]
}

/// The reflection corners for plane `i`: mirrored downward across its bottom edge.
pub fn reflection_corners(i: usize) -> [Vec3; 4] {
    let bottom = plane_center(i).y - PH_WORLD * 0.5;
    plane_corners(i).map(|v| Vec3::new(v.x, 2.0 * bottom - v.y, v.z))
}

// ---- the camera path ---------------------------------------------------------

#[derive(Clone, Copy)]
struct Key {
    t: f32,
    eye: Vec3,
    look: Vec3,
    fov: f32,
}

/// The close pose for panel `i`: straight out from the panel face, near enough that
/// the frontal plane fills the frame.
fn dwell_eye(i: usize) -> Vec3 {
    let c = plane_center(i);
    v_add(
        v_add(c, v_scale(plane_outward(i), D_DWELL)),
        Vec3::new(0.0, 0.3, 0.0),
    )
}

/// The wide pose the straight dolly starts and ends at (further out, slightly high,
/// still on the panel's outward axis so the dolly stays frontal).
fn wide_eye(i: usize) -> Vec3 {
    let c = plane_center(i);
    v_add(
        v_add(c, v_scale(plane_outward(i), D_WIDE)),
        Vec3::new(0.0, 1.4, 0.0),
    )
}

/// The scripted flight as absolute-time keyframes: establish; then per panel a
/// straight dolly-in, a hold, and a straight dolly-out (all frontal) before an arc
/// to the next; then the rise-out reveal.
fn camera_keys() -> Vec<Key> {
    let mut keys = Vec::with_capacity(N_PLANES * 4 + 3);
    keys.push(Key {
        t: 0.0,
        eye: Vec3::new(0.0, 6.0, AXIS_Z - 23.0),
        look: axis_at(0.0),
        fov: 1.12,
    });
    for i in 0..N_PLANES {
        let base = EST + i as f32 * PANEL;
        let c = plane_center(i);
        keys.push(Key {
            t: base,
            eye: wide_eye(i),
            look: c,
            fov: FOV_WIDE,
        });
        keys.push(Key {
            t: base + TRAVEL_IN,
            eye: dwell_eye(i),
            look: c,
            fov: FOV_DWELL,
        });
        keys.push(Key {
            t: base + TRAVEL_IN + HOLD,
            eye: dwell_eye(i),
            look: c,
            fov: FOV_DWELL,
        });
        keys.push(Key {
            t: base + TRAVEL_IN + HOLD + TRAVEL_OUT,
            eye: wide_eye(i),
            look: c,
            fov: FOV_WIDE,
        });
    }
    let rt = EST + N_PLANES as f32 * PANEL;
    keys.push(Key {
        t: rt,
        eye: Vec3::new(0.0, 11.0, AXIS_Z - 17.0),
        look: axis_at(0.0),
        fov: 1.15,
    });
    keys.push(Key {
        t: rt + REVEAL,
        eye: Vec3::new(0.0, 12.5, AXIS_Z - 19.0),
        look: axis_at(0.0),
        fov: 1.18,
    });
    keys
}

/// Assemble a `Camera` from an eye, a look-at target and a vertical fov, with the
/// ring's fixed up / near / far — the one place a `Camera` is constructed, so the
/// scripted flight and the interactive poses stay identical in everything but pose.
fn camera_from(eye: Vec3, look: Vec3, fov: f32) -> Camera {
    Camera {
        position: eye,
        target: look,
        up: Vec3::new(0.0, 1.0, 0.0),
        fov_y: fov,
        near: 0.1,
        far: 120.0,
    }
}

/// The camera at edit time `edit` (seconds), eased (`ease_in_out`) between the
/// surrounding keyframe pair.
pub fn camera_at(edit: f32) -> Camera {
    let keys = camera_keys();
    let t = if edit.is_finite() {
        edit.clamp(0.0, keys[keys.len() - 1].t)
    } else {
        0.0
    };
    let (a, b) = surrounding(&keys, t);
    let span = (b.t - a.t).max(1e-4);
    let e = gibson::clock::ease_in_out(((t - a.t) / span).clamp(0.0, 1.0));
    camera_from(
        v_lerp(a.eye, b.eye, e),
        v_lerp(a.look, b.look, e),
        a.fov + (b.fov - a.fov) * e,
    )
}

/// The close, frontal pose featuring panel `i` — the same pose the scripted film
/// holds on. A settled interactive camera at this pose projects panel `i` to exactly
/// [`hold_rect`], so its live UI resolves as a 1:1 blit of the master (the congruence
/// law holds under interaction, not just the scripted flight).
pub fn dwell_camera(i: usize) -> Camera {
    camera_from(dwell_eye(i), plane_center(i), FOV_DWELL)
}

/// The wide orbit pose framing panel `i` from outside the ring (where the film's
/// per-panel dolly starts and ends) — the resting pose when not zoomed in.
pub fn orbit_camera(i: usize) -> Camera {
    camera_from(wide_eye(i), plane_center(i), FOV_WIDE)
}

fn surrounding(keys: &[Key], t: f32) -> (Key, Key) {
    for w in keys.windows(2) {
        if t >= w[0].t && t <= w[1].t {
            return (w[0], w[1]);
        }
    }
    if t <= keys[0].t {
        (keys[0], keys[0])
    } else {
        let last = keys[keys.len() - 1];
        (last, last)
    }
}

/// Which panel the camera is featuring, and the live time to animate it at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Focus {
    pub panel: usize,
    /// Seconds since this panel's hold began (`>= 0`), for live animation.
    pub local: f32,
}

/// The panel featured at edit time `edit`, or `None` during the establish / reveal.
pub fn focus_at(edit: f32) -> Option<Focus> {
    if !edit.is_finite() || edit < EST || edit >= EST + N_PLANES as f32 * PANEL {
        return None;
    }
    let rel = edit - EST;
    let i = (rel / PANEL) as usize;
    if i >= N_PLANES {
        return None;
    }
    let p = rel - i as f32 * PANEL;
    Some(Focus {
        panel: i,
        local: (p - TRAVEL_IN).max(0.0),
    })
}

/// The projected screen rectangle (in cells) of panel `i`'s face at edit time
/// `edit`, for an output of `w × h` cells — or `None` when the plane is too small,
/// behind the camera, or oblique (the inscribed axis-aligned rect collapses). When
/// `Some`, the driver resolves the real UI Surface into this rectangle; when `None`
/// the depth-tested textured plane stands in. This is the intro facade law's
/// `cell_bounds`, adapted to the ring's outward planes.
pub fn panel_cell_bounds(i: usize, edit: f32, w: u16, h: u16) -> Option<Rect> {
    panel_cell_bounds_cam(i, &camera_at(edit), w, h)
}

/// [`panel_cell_bounds`] for an arbitrary (e.g. interactively driven) camera rather
/// than the scripted edit clock. The scripted path delegates here, so the facade law
/// is identical whichever clock drives the camera.
pub fn panel_cell_bounds_cam(i: usize, cam: &Camera, w: u16, h: u16) -> Option<Rect> {
    if i >= N_PLANES || w == 0 || h == 0 {
        return None;
    }
    let c = plane_corners(i); // TL, TR, BR, BL
    let proj = |p: Vec3| cam.project(p, w, h.saturating_mul(2));
    let (tl, tr, br, bl) = (proj(c[0])?, proj(c[1])?, proj(c[2])?, proj(c[3])?);
    // Inscribe whole cells inside the projected plate (x in columns; y in pixel
    // rows, halved to cell rows to match `RgbRaster::to_surface`).
    let left = tl.0.max(bl.0).ceil().max(0.0) as u16;
    let right = ((tr.0.min(br.0)).floor().max(0.0) as u16).min(w);
    let top = ((tl.1.max(tr.1)) * 0.5).ceil().max(0.0) as u16;
    let bottom = (((bl.1.min(br.1)) * 0.5).floor().max(0.0) as u16).min(h);
    if right <= left || bottom <= top {
        return None;
    }
    let bounds = Rect::new(left, top, right - left, bottom - top);
    (bounds.width >= MIN_BW && bounds.height >= MIN_BH).then_some(bounds)
}

/// The projected rectangle of panel `i` at the centre of its hold — the LARGEST it
/// reaches on screen (the camera is stationary through the hold, and the eased dolly-in
/// approaches it monotonically, so no approach frame exceeds it). The driver sizes the
/// panel's master UI — and the congruent baked plane texture — to this, so the hold is a
/// 1:1 copy and every other frame a pure down-scale (never an up-scale, which
/// nearest-neighbour would show as duplicated rows). Uniform across the congruent planes,
/// and it tracks the terminal size, so a resize stays 1:1 at the hold.
pub fn hold_rect(i: usize, w: u16, h: u16) -> Option<Rect> {
    let t = EST + i as f32 * PANEL + TRAVEL_IN + HOLD * 0.5;
    panel_cell_bounds(i, t, w, h)
}

// ---- the compositor ----------------------------------------------------------

/// Plant the luminous core along the ring axis as bright 2-D disc sources, left for
/// `look::bloom` to spread into a glow.
fn core_glow(rz: &mut Rasterizer, cam: &Camera, pw: u16, ph: u16) {
    for k in -2..=2 {
        let p = Vec3::new(0.0, k as f32 * 0.7, AXIS_Z);
        if let Some((sx, sy, z)) = cam.project(p, pw, ph) {
            if z <= 0.0 {
                continue;
            }
            let radius = (80.0 / z).clamp(2.0, 30.0);
            rz.raster.disc(sx, sy, radius * 1.9, (52, 22, 86));
            rz.raster.disc(sx, sy, radius, (190, 120, 236));
            rz.raster.disc(sx, sy, radius * 0.45, (245, 226, 255));
        }
    }
}

/// Render one frame of the flight to an `RgbRaster` of `pw × ph` pixels: nebula
/// backdrop, then the panels far-to-near through the moving camera. The driver may
/// then resolve the focused panel's real UI over the top.
pub fn render_frame(
    edit: f32,
    pw: u16,
    ph: u16,
    backdrop: RgbRaster,
    textures: &[RgbRaster],
    reflections: &[RgbRaster],
) -> RgbRaster {
    render_frame_cam(&camera_at(edit), pw, ph, backdrop, textures, reflections)
}

/// [`render_frame`] for an arbitrary (e.g. interactively driven) camera. The scripted
/// path delegates here, so the ring composites identically whichever clock drives it.
pub fn render_frame_cam(
    cam: &Camera,
    pw: u16,
    ph: u16,
    backdrop: RgbRaster,
    textures: &[RgbRaster],
    reflections: &[RgbRaster],
) -> RgbRaster {
    let cam = *cam;
    let mut rz = Rasterizer::new(pw.max(1), ph.max(1));
    if backdrop.width() == pw.max(1) && backdrop.height() == ph.max(1) {
        rz.raster = backdrop;
    }
    core_glow(&mut rz, &cam, pw.max(1), ph.max(1));

    let n = textures.len().min(N_PLANES);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        let da = dist2(cam.position, plane_center(a));
        let db = dist2(cam.position, plane_center(b));
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });
    for i in order {
        if let Some(refl) = reflections.get(i) {
            rz.textured_quad(reflection_corners(i), refl, &cam);
        }
        rz.textured_quad(plane_corners(i), &textures[i], &cam);
    }
    rz.raster
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planes_sit_on_the_ring_at_the_expected_radius() {
        for i in 0..N_PLANES {
            let c = plane_center(i);
            let r = ((c.x).powi(2) + (c.z - AXIS_Z).powi(2)).sqrt();
            assert!((r - R).abs() < 1e-3, "plane {i} radius {r}");
        }
    }

    #[test]
    fn planes_face_outward_away_from_the_axis() {
        for i in 0..N_PLANES {
            let c = plane_center(i);
            let step = v_add(c, v_scale(plane_outward(i), 0.5));
            let r0 = ((c.x).powi(2) + (c.z - AXIS_Z).powi(2)).sqrt();
            let r1 = ((step.x).powi(2) + (step.z - AXIS_Z).powi(2)).sqrt();
            assert!(r1 > r0, "plane {i} normal must point outward");
        }
    }

    #[test]
    fn plane_corners_are_centred_and_upright() {
        for i in 0..N_PLANES {
            let c = plane_center(i);
            let k = plane_corners(i);
            let mid = Vec3::new(
                (k[0].x + k[1].x + k[2].x + k[3].x) / 4.0,
                (k[0].y + k[1].y + k[2].y + k[3].y) / 4.0,
                (k[0].z + k[1].z + k[2].z + k[3].z) / 4.0,
            );
            assert!((mid.x - c.x).abs() < 1e-3 && (mid.z - c.z).abs() < 1e-3);
            assert!(k[0].y > k[3].y, "top edge above bottom edge");
        }
    }

    #[test]
    fn reflection_hangs_below_the_panel() {
        for i in 0..N_PLANES {
            let p = plane_corners(i);
            let r = reflection_corners(i);
            for j in 0..4 {
                assert!((r[j].x - p[j].x).abs() < 1e-4);
                assert!((r[j].z - p[j].z).abs() < 1e-4);
                assert!(r[j].y <= p[3].y + 1e-4);
            }
        }
    }

    #[test]
    fn camera_is_finite_and_continuous_across_the_whole_film() {
        let dur = duration();
        let mut prev: Option<Camera> = None;
        let mut t = 0.0;
        while t <= dur {
            let cam = camera_at(t);
            assert!(cam.position.is_finite() && cam.target.is_finite());
            assert!(cam.fov_y > 0.0 && cam.fov_y < std::f32::consts::PI);
            if let Some(p) = prev {
                let jump = dist2(p.position, cam.position).sqrt();
                assert!(jump < 4.0, "camera jump {jump} at t={t}");
            }
            prev = Some(cam);
            t += 0.1;
        }
    }

    #[test]
    fn focus_covers_every_panel_and_rests_at_the_ends() {
        assert!(focus_at(0.5).is_none(), "no focus during establish");
        assert!(
            focus_at(duration() - 1.0).is_none(),
            "no focus during reveal"
        );
        let mut seen = [false; N_PLANES];
        let mut t = 0.0;
        while t <= duration() {
            if let Some(f) = focus_at(t) {
                seen[f.panel] = true;
            }
            t += 0.05;
        }
        assert!(seen.iter().all(|&s| s), "every panel featured: {seen:?}");
    }

    #[test]
    fn focused_panel_resolves_to_a_bounded_rect_at_its_hold() {
        // At the hold the frontal plane must present a valid, on-screen rectangle.
        let (w, h) = (120u16, 36u16);
        for i in 0..N_PLANES {
            let hold_t = EST + i as f32 * PANEL + TRAVEL_IN + HOLD * 0.5;
            let b = panel_cell_bounds(i, hold_t, w, h)
                .unwrap_or_else(|| panic!("panel {i} should resolve a rect at its hold"));
            assert!(b.width >= MIN_BW && b.height >= MIN_BH);
            assert!(b.x + b.width <= w && b.y + b.height <= h, "rect on screen");
        }
    }

    #[test]
    fn render_frame_is_bounded_and_deterministic() {
        let textures: Vec<RgbRaster> = (0..N_PLANES)
            .map(|i| {
                let mut r = RgbRaster::new(16, 12);
                r.clear((20 * i as u8, 40, 80));
                r
            })
            .collect();
        let stub = |t: &RgbRaster| {
            let mut r = RgbRaster::new(t.width(), t.height());
            r.clear((0, 0, 0));
            r
        };
        let reflections: Vec<RgbRaster> = textures.iter().map(stub).collect();
        let bg = |w, h| {
            let mut r = RgbRaster::new(w, h);
            r.clear((8, 6, 16));
            r
        };
        let a = render_frame(14.0, 120, 72, bg(120, 72), &textures, &reflections);
        let b = render_frame(14.0, 120, 72, bg(120, 72), &textures, &reflections);
        assert_eq!((a.width(), a.height()), (120, 72));
        assert_eq!(a.pixels(), b.pixels());
    }
}
