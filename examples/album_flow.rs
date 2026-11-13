//! Album Flow — a Cover-Flow spatial browser, the visual-acceptance flagship of the
//! v0.5 "Observable Instruments" spatial axis.
//!
//! This is deliberately NOT three bordered panels labelled PREVIOUS/CURRENT/NEXT. It is
//! a continuous perspective arrangement of textured cards driven by one pose law and the
//! `raster3d::Rasterizer::textured_quad` media primitive. The demo owns its art direction;
//! the only thing it borrows from the library is the generic textured-quad primitive.
//!
//! The arrangement obeys a single pose law Φ(d), where d = cover_index − selection. Because
//! Φ depends only on d, the whole layout is *equivariant* under selection shifts — advancing
//! the selection by one slides every cover into the pose its neighbour just held. Reflections
//! are *derived* from each cover (mirrored across its bottom edge, faded), never authored
//! independently. Motion is a damped spring on the (fractional) selection.
//!
//! Run it:  `cargo run --release --example album_flow`  (Esc / q / Ctrl-C to quit).
//! Offline:  `album_flow capture <sel> <out.ppm>` / `album_flow frames <dir> [w] [h]`.
//!
//! Known limitation — tearing at large/fullscreen sizes. The demo repaints a moving,
//! near-full-frame image every tick, so at large window sizes the cost (both this process's
//! rasterization and the terminal's own per-cell draw bandwidth) can exceed the frame budget
//! and the terminal may show a partially-updated frame. Each frame is already handed over as
//! one atomic write wrapped in DEC 2026 synchronized-update markers, so a terminal that
//! honours 2026 should not tear; a terminal that does not (or one on a load-constrained
//! machine) can. Two things help, neither a full cure: build with `--release` (debug
//! rasterization is several times slower — measured ~4x here), and use a normal-sized window
//! rather than fullscreen. This is a terminal/host-bandwidth confound, not a rendering-logic
//! bug — an adaptive draw-size experiment was tried and reverted because its frame-to-frame
//! resizing was a worse artifact than the minor tearing it chased.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use gibson::geom::Vec3;
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::node::Node;
use gibson::raster::{Rgb, RgbRaster};
use gibson::raster3d::{Camera, Rasterizer};
use gibson::surface::Surface;
use gibson::Context;

// ---- the arrangement, in world units -------------------------------------------------

const NUM_COVERS: usize = 11; // within the 9–15 the milestone asks for
const COVER_W: f32 = 2.0;
const COVER_H: f32 = 2.0;
const BASE_Z: f32 = 4.2; // depth of the selected cover
const SPREAD_NEAR: f32 = 2.35; // x step from centre to the first neighbour
const SPREAD_FAR: f32 = 0.85; // x step per cover beyond the first neighbour (tighter packing)
const DEPTH_NEAR: f32 = 2.3; // z recession reaching the first neighbour
const DEPTH_FAR: f32 = 0.45; // z recession per cover beyond it
const YAW_MAX: f32 = 0.78; // fan angle (~45°) held by all side covers
const BG: Rgb = (13, 15, 26); // a deep ink-blue, lifted so receding covers keep a dark-on-dark halo

/// A cover's world pose: the centre of its card and its rotation about the vertical axis.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    center: Vec3,
    yaw: f32,
}

/// The pose law Φ(d), d = cover_index − selection. Depends ONLY on d, so the whole
/// arrangement is equivariant under selection shifts: Φ((i+1)−(s+1)) = Φ(i−s).
///
/// d = 0 is the selected cover: centred, facing the camera, nearest. As |d| grows the
/// cover steps aside, recedes, and rotates to the fan angle (reached and then held at
/// |d| ≥ 1). The near region (|d| ≤ 1) is a smooth ramp so motion between slots reads
/// as a single cover swinging to the front.
fn pose(d: f32) -> Pose {
    let side = if d >= 0.0 { 1.0 } else { -1.0 };
    let a = d.abs();
    let near = a.min(1.0); // 0..1 inside the central slot
    let far = (a - 1.0).max(0.0); // beyond the first neighbour
    let x = side * (SPREAD_NEAR * near + SPREAD_FAR * far);
    let z = BASE_Z + DEPTH_NEAR * near + DEPTH_FAR * far;
    let yaw = -side * YAW_MAX * near;
    Pose {
        center: Vec3::new(x, 0.0, z),
        yaw,
    }
}

/// The four world corners of a cover in TL → TR → BR → BL order (matching the
/// textured-quad UV order). The card is rotated about the vertical (Y) axis by `yaw`:
/// a local offset (lx, ly, 0) maps to the world offset (lx·cos, ly, −lx·sin).
fn cover_corners(p: Pose) -> [Vec3; 4] {
    let (hw, hh) = (COVER_W * 0.5, COVER_H * 0.5);
    let (c, s) = (p.yaw.cos(), p.yaw.sin());
    let corner =
        |lx: f32, ly: f32| Vec3::new(p.center.x + lx * c, p.center.y + ly, p.center.z - lx * s);
    [
        corner(-hw, hh),  // TL
        corner(hw, hh),   // TR
        corner(hw, -hh),  // BR
        corner(-hw, -hh), // BL
    ]
}

/// The reflection corners, DERIVED from the cover by mirroring across its bottom edge.
/// Geometry (not the texture) is flipped, so sampling the same texture yields a correct
/// upside-down mirror image hanging below the card.
fn reflection_corners(p: Pose) -> [Vec3; 4] {
    let bottom = p.center.y - COVER_H * 0.5;
    cover_corners(p).map(|v| Vec3::new(v.x, 2.0 * bottom - v.y, v.z))
}

/// Poses for every cover at a (fractional) selection.
fn arrangement(selection: f32, n: usize) -> Vec<Pose> {
    (0..n).map(|i| pose(i as f32 - selection)).collect()
}

// ---- procedural cover art (no external images, no copyrighted art) --------------------

const ART_SIZE: u16 = 64;

/// Linear blend of two colours (`t` = 0 → `a`, 1 → `b`).
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// A deterministic abstract "album cover" for a seed. Hues walk the colour wheel by the
/// golden angle so neighbours on the shelf never rhyme; the motif cycles through six bold
/// shapes; the rim is a one-texel tonal lift of the art itself (never a white frame).
/// Pure function of the seed.
fn cover_art(seed: u32) -> RgbRaster {
    let mut r = RgbRaster::new(ART_SIZE, ART_SIZE);
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(40_503).max(1);
    let mut rng = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let jitter = (rng() % 24) as f32;
    let hue = (seed as f32 * 137.508 + 14.0 + jitter) % 360.0;
    let hue2 = (hue + 150.0 + (rng() % 60) as f32) % 360.0; // a lively near-complement
    let motif = seed % 6;
    let wobble = (rng() % 100) as f32 / 100.0; // 0..1 motif variation
    let denom = (ART_SIZE - 1) as f32;
    for y in 0..ART_SIZE {
        for x in 0..ART_SIZE {
            let fx = x as f32 / denom;
            let fy = y as f32 / denom;
            // Rich vertical-ish gradient: bright enough to stay legible when it recedes.
            let t = fy * 0.7 + fx * 0.3;
            let mut c = hsv(hue + (hue2 - hue) * 0.35 * t, 0.74, 0.50 + 0.38 * (1.0 - t));
            let (dx, dy) = (fx - 0.5, fy - 0.5);
            let d = (dx * dx + dy * dy).sqrt();
            let ink = hsv(hue2, 0.55, 1.0); // the bright motif colour
            let pale = hsv(hue + 20.0, 0.18, 0.98); // near-white accent, tinted
            match motif {
                // Sun disc with a soft halo ring.
                0 => {
                    let cy = 0.44 + 0.08 * wobble;
                    let dd = ((fx - 0.5).powi(2) + (fy - cy).powi(2)).sqrt();
                    if dd < 0.25 {
                        c = ink;
                    } else if dd < 0.31 {
                        c = mix(c, ink, 0.35);
                    }
                }
                // Horizon: a half-sun over banded water.
                1 => {
                    let horizon = 0.58;
                    if fy < horizon && ((fx - 0.5).powi(2) + (fy - horizon).powi(2)).sqrt() < 0.30 {
                        c = ink;
                    } else if fy >= horizon {
                        let band = ((fy - horizon) * 17.0) as i32 % 2 == 0;
                        c = mix(c, hsv(hue2, 0.7, 0.35), if band { 0.65 } else { 0.25 });
                    }
                }
                // Bold diagonal slash with a thin companion line.
                2 => {
                    let k = (fx - fy).abs();
                    if k < 0.13 {
                        c = ink;
                    } else if (k - 0.24).abs() < 0.025 {
                        c = pale;
                    }
                }
                // Concentric rings.
                3 => {
                    let ring = (d * 7.0) as i32;
                    if d < 0.43 && ring % 2 == 0 {
                        c = mix(c, ink, if ring == 0 { 1.0 } else { 0.8 });
                    }
                }
                // A peak: big triangle with a pale snowcap.
                4 => {
                    let apex = 0.22 + 0.10 * wobble;
                    let half = (fy - apex) * 0.62;
                    if fy > apex && fy < 0.82 && dx.abs() < half {
                        c = if fy < apex + 0.12 { pale } else { ink };
                    }
                }
                // Equaliser bars.
                _ => {
                    let col = (fx * 7.0) as usize;
                    let within = (fx * 7.0).fract();
                    let h = 0.25 + 0.55 * (((col as f32 * 1.7 + wobble * 6.0).sin() + 1.0) * 0.5);
                    if within > 0.18 && within < 0.88 && fy > 0.88 - h && fy < 0.88 {
                        c = if fy < 0.88 - h + 0.05 { pale } else { ink };
                    }
                }
            }
            // A one-texel tonal rim: the art's own colour, lifted — not a white frame.
            let edge = x == 0 || y == 0 || x == ART_SIZE - 1 || y == ART_SIZE - 1;
            if edge {
                c = mix(c, (255, 255, 255), 0.28);
            }
            r.set(x as i32, y as i32, c);
        }
    }
    r
}

/// The reflection texture: the cover dimmed and faded toward its far (lower) end. The
/// reflection geometry is flipped, so texture row v=0 (the cover's top) lands at the
/// reflection's far end — hence the fade brightens with v.
fn reflection_texture(cover: &RgbRaster) -> RgbRaster {
    let (w, h) = (cover.width(), cover.height());
    let mut r = RgbRaster::new(w, h);
    let denom = (h.max(2) - 1) as f32;
    for y in 0..h {
        let v = y as f32 / denom;
        let f = 0.12 + 0.46 * v;
        for x in 0..w {
            let c = cover.get(x as i32, y as i32).unwrap_or_default();
            r.set(
                x as i32,
                y as i32,
                (
                    (c.0 as f32 * f) as u8,
                    (c.1 as f32 * f) as u8,
                    (c.2 as f32 * f) as u8,
                ),
            );
        }
    }
    r
}

/// HSV (h in degrees, s/v in 0..1) → RGB.
fn hsv(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to = |z: f32| ((z + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to(r), to(g), to(b))
}

// ---- camera + render ------------------------------------------------------------------

/// The viewing camera: slightly above the row, looking into it, so the reflections read.
fn camera() -> Camera {
    Camera {
        position: Vec3::new(0.0, 0.35, 0.0),
        target: Vec3::new(0.0, -0.1, BASE_Z),
        up: Vec3::new(0.0, 1.0, 0.0),
        fov_y: std::f32::consts::FRAC_PI_3,
        near: 0.1,
        far: 100.0,
    }
}

/// Render the arrangement into an RGB raster (`pw × ph` pixels). Covers are drawn
/// far-to-near; the shared z-buffer resolves overlap regardless.
fn render_raster(
    covers: &[RgbRaster],
    reflections: &[RgbRaster],
    selection: f32,
    pw: u16,
    ph: u16,
) -> RgbRaster {
    let mut rz = Rasterizer::new(pw, ph);
    rz.clear(BG);
    let cam = camera();
    let poses = arrangement(selection, covers.len());
    // Draw far-to-near (the shared z-buffer resolves overlap regardless of order).
    let mut order: Vec<usize> = (0..poses.len()).collect();
    order.sort_by(|&a, &b| {
        poses[b]
            .center
            .z
            .partial_cmp(&poses[a].center.z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for i in order {
        let p = poses[i];
        rz.textured_quad(reflection_corners(p), &reflections[i], &cam);
        rz.textured_quad(cover_corners(p), &covers[i], &cam);
    }
    rz.raster
}

/// Render to a terminal `Surface` of `cols × rows` cells. `mono` selects the Braille
/// monochrome fallback for terminals without colour.
fn render(
    covers: &[RgbRaster],
    reflections: &[RgbRaster],
    selection: f32,
    cols: u16,
    rows: u16,
    mono: bool,
) -> Surface {
    let raster = render_raster(covers, reflections, selection, cols, rows.saturating_mul(2));
    if mono {
        raster.to_mono_surface()
    } else {
        raster.to_surface()
    }
}

// ---- damped-spring motion -------------------------------------------------------------

/// A damped spring on the fractional selection. Deterministic given the dt sequence.
struct Spring {
    x: f32,
    v: f32,
}
impl Spring {
    fn step(&mut self, target: f32, dt: f32, omega: f32, zeta: f32) {
        let accel = -2.0 * zeta * omega * self.v - omega * omega * (self.x - target);
        self.v += accel * dt;
        self.x += self.v * dt;
    }
}

/// The scripted demo motion as a deterministic sequence of (fractional) selections: a
/// damped-spring sweep forward across the whole shelf and part-way back, holding on each
/// cover. Shared by the live animation and the frame-export mode so both move identically.
fn demo_selections() -> Vec<f32> {
    let mut script: Vec<f32> = (0..NUM_COVERS).map(|i| i as f32).collect();
    script.extend((2..NUM_COVERS).rev().map(|i| i as f32));
    let mut spring = Spring { x: 0.0, v: 0.0 };
    let dt = 1.0 / 30.0;
    let mut selections = Vec::new();
    for target in script {
        for _ in 0..16 {
            spring.step(target, dt, 13.0, 0.72);
            selections.push(spring.x);
        }
    }
    selections
}

// ---- the live demo --------------------------------------------------------------------

fn main() -> io::Result<()> {
    let covers: Vec<RgbRaster> = (0..NUM_COVERS).map(|i| cover_art(i as u32)).collect();
    let reflections: Vec<RgbRaster> = covers.iter().map(reflection_texture).collect();

    // Deterministic still capture: `album_flow capture <selection> <path.ppm>`.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("capture") {
        let sel = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3.0);
        let path = args.get(3).map(String::as_str).unwrap_or("album_flow.ppm");
        let raster = render_raster(&covers, &reflections, sel, 360, 220);
        let file = std::fs::File::create(path).expect("create capture file");
        raster
            .write_ppm(std::io::BufWriter::new(file))
            .expect("write ppm");
        eprintln!("captured selection {sel} -> {path}");
        return Ok(());
    }

    // Deterministic frame export of the scripted motion: `album_flow frames <dir> [w] [h]`.
    // Writes frame_0000.ppm … following the exact sequence the live demo animates.
    if args.get(1).map(String::as_str) == Some("frames") {
        let dir = args.get(2).map(String::as_str).unwrap_or(".");
        let w: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(360);
        let h: u16 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(220);
        std::fs::create_dir_all(dir).expect("create frame dir");
        for (i, sel) in demo_selections().iter().enumerate() {
            let raster = render_raster(&covers, &reflections, *sel, w, h);
            let path = format!("{dir}/frame_{i:04}.ppm");
            let file = std::fs::File::create(&path).expect("create frame file");
            raster
                .write_ppm(std::io::BufWriter::new(file))
                .expect("write ppm");
        }
        eprintln!("wrote {} frames to {dir}", demo_selections().len());
        return Ok(());
    }

    // Live demo: take over the terminal through a fullscreen Context (alternate screen,
    // raw mode, hidden cursor, synchronized frames, autowrap off) and render the shelf into
    // a canvas node sized to the ACTUAL terminal, so it is responsive and never wraps.
    // Esc / q / Ctrl-C quits and restores the terminal; resizing is handled per frame.
    let covers = Arc::new(covers);
    let reflections = Arc::new(reflections);
    let mut ctx = Context::fullscreen()?;
    let result = run_live(&mut ctx, &covers, &reflections);
    ctx.restore()?;
    result
}

fn run_live(
    ctx: &mut Context,
    covers: &Arc<Vec<RgbRaster>>,
    reflections: &Arc<Vec<RgbRaster>>,
) -> io::Result<()> {
    let interactive = ctx.session.is_tty;
    let selections = demo_selections();
    loop {
        for &sel in &selections {
            let c = Arc::clone(covers);
            let r = Arc::clone(reflections);
            // The canvas receives the live terminal Rect, so the frame fits exactly.
            // It MUST fill the screen — an unsized canvas collapses to zero (black screen).
            ctx.set_root(
                Node::canvas(move |rect| {
                    render(
                        &c[..],
                        &r[..],
                        sel,
                        rect.width.max(1),
                        rect.height.max(1),
                        false,
                    )
                })
                .percent_width(100.0)
                .percent_height(100.0),
            );
            ctx.render()?;
            if !interactive {
                // Non-TTY (piped/redirected): run one sweep without busy-pacing, then stop.
                std::thread::sleep(Duration::from_millis(33));
                continue;
            }
            if let Some(Event::Key(k)) = ctx.poll_event(Duration::from_millis(33))? {
                let quit = matches!(k.code, KeyCode::Esc | KeyCode::Char('q'))
                    || (k.code == KeyCode::Char('c')
                        && k.modifiers.contains(KeyModifiers::CONTROL));
                if quit {
                    return Ok(());
                }
            }
        }
        if !interactive {
            return Ok(());
        }
    }
}

// ---- tests: the pose law and render invariants ----------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn pose_close(a: Pose, b: Pose) -> bool {
        (a.center.x - b.center.x).abs() < 1e-4
            && (a.center.y - b.center.y).abs() < 1e-4
            && (a.center.z - b.center.z).abs() < 1e-4
            && (a.yaw - b.yaw).abs() < 1e-4
    }

    #[test]
    fn selected_cover_is_centred_facing_and_nearest() {
        let p = pose(0.0);
        assert_eq!(p.center.x, 0.0);
        assert_eq!(p.yaw, 0.0);
        assert_eq!(p.center.z, BASE_Z);
    }

    #[test]
    fn arrangement_is_equivariant_under_selection_shift() {
        // Advancing selection by one puts cover i+1 where cover i used to be.
        for &s in &[0.0f32, 0.3, 1.7, -0.4] {
            let a = arrangement(s, NUM_COVERS);
            let b = arrangement(s + 1.0, NUM_COVERS);
            for i in 0..NUM_COVERS - 1 {
                assert!(
                    pose_close(a[i], b[i + 1]),
                    "equivariance broken at i={i}, s={s}: {:?} vs {:?}",
                    a[i],
                    b[i + 1]
                );
            }
        }
    }

    #[test]
    fn recession_is_monotonic_in_distance() {
        let ds = [0.0f32, 0.5, 1.0, 2.0, 3.0, 4.0];
        for w in ds.windows(2) {
            let (near, far) = (pose(w[0]), pose(w[1]));
            assert!(far.center.z > near.center.z, "z must increase with |d|");
            assert!(
                far.center.x.abs() > near.center.x.abs() - 1e-6,
                "|x| must not decrease with |d|"
            );
        }
    }

    #[test]
    fn pose_is_antisymmetric_and_yaw_bounded() {
        for &d in &[0.3f32, 0.8, 1.0, 2.5, 5.0] {
            let (pos, neg) = (pose(d), pose(-d));
            assert!(
                (pos.center.x + neg.center.x).abs() < 1e-5,
                "x antisymmetric"
            );
            assert!((pos.center.z - neg.center.z).abs() < 1e-5, "z symmetric");
            assert!((pos.yaw + neg.yaw).abs() < 1e-5, "yaw antisymmetric");
            assert!(pos.yaw.abs() <= YAW_MAX + 1e-6, "yaw bounded by YAW_MAX");
        }
    }

    #[test]
    fn side_covers_rotate_about_the_vertical_axis() {
        let c = cover_corners(pose(1.0));
        // Rotation about Y keeps paired top/bottom y-coordinates equal...
        assert!((c[0].y - c[1].y).abs() < 1e-5, "TL/TR share height");
        assert!((c[2].y - c[3].y).abs() < 1e-5, "BR/BL share height");
        // ...and tilts the card in depth (the two vertical edges sit at different z).
        assert!(
            (c[0].z - c[1].z).abs() > 1e-3,
            "a rotated card recedes across its width"
        );
    }

    #[test]
    fn reflection_is_derived_below_the_cover() {
        let p = pose(0.0);
        let cover = cover_corners(p);
        let refl = reflection_corners(p);
        let bottom = p.center.y - COVER_H * 0.5;
        for i in 0..4 {
            assert_eq!(refl[i].x, cover[i].x, "reflection shares x");
            assert_eq!(refl[i].z, cover[i].z, "reflection shares z");
            assert!(
                refl[i].y <= bottom + 1e-6,
                "reflection hangs at/below the cover bottom"
            );
            // Mirror across the bottom edge.
            assert!((refl[i].y - (2.0 * bottom - cover[i].y)).abs() < 1e-5);
        }
    }

    #[test]
    fn render_is_deterministic() {
        let covers: Vec<RgbRaster> = (0..NUM_COVERS).map(|i| cover_art(i as u32)).collect();
        let refl: Vec<RgbRaster> = covers.iter().map(reflection_texture).collect();
        let a = render(&covers, &refl, 2.3, 120, 40, false);
        let b = render(&covers, &refl, 2.3, 120, 40, false);
        assert_eq!(a.to_visible_lines(), b.to_visible_lines());
    }

    #[test]
    fn render_is_responsive_across_terminal_sizes() {
        let covers: Vec<RgbRaster> = (0..NUM_COVERS).map(|i| cover_art(i as u32)).collect();
        let refl: Vec<RgbRaster> = covers.iter().map(reflection_texture).collect();
        for &(cols, rows) in &[(160u16, 50u16), (42u16, 15u16), (120u16, 40u16)] {
            let s = render(&covers, &refl, 4.0, cols, rows, false);
            assert_eq!(s.width, cols);
            assert_eq!(s.height, rows);
            assert_eq!(s.to_visible_lines().len(), rows as usize);
        }
    }

    #[test]
    fn covers_actually_draw_onto_the_frame() {
        let covers: Vec<RgbRaster> = (0..NUM_COVERS).map(|i| cover_art(i as u32)).collect();
        let refl: Vec<RgbRaster> = covers.iter().map(reflection_texture).collect();
        let raster = render_raster(&covers, &refl, 3.0, 160, 80);
        let painted = raster.pixels().iter().filter(|&&px| px != BG).count();
        assert!(
            painted > 1000,
            "expected a populated shelf, got {painted} non-bg pixels"
        );
    }

    #[test]
    fn mono_fallback_emits_braille_and_is_deterministic() {
        let covers: Vec<RgbRaster> = (0..NUM_COVERS).map(|i| cover_art(i as u32)).collect();
        let refl: Vec<RgbRaster> = covers.iter().map(reflection_texture).collect();
        let a = render(&covers, &refl, 3.0, 120, 40, true);
        let b = render(&covers, &refl, 3.0, 120, 40, true);
        assert_eq!(a.to_visible_lines(), b.to_visible_lines());
        let has_braille = a
            .to_visible_lines()
            .iter()
            .flat_map(|l| l.chars())
            .any(|ch| ('\u{2800}'..='\u{28FF}').contains(&ch));
        assert!(has_braille, "mono fallback must emit Braille glyphs");
    }

    #[test]
    fn perspective_shrinks_side_covers_relative_to_the_selected_one() {
        let cam = camera();
        let width_on_screen = |d: f32| {
            let c = cover_corners(pose(d));
            let l = cam.project(c[0], 160, 80);
            let r = cam.project(c[1], 160, 80);
            match (l, r) {
                (Some(l), Some(r)) => Some((r.0 - l.0).abs()),
                _ => None,
            }
        };
        let front = width_on_screen(0.0).expect("selected cover projects");
        let side = width_on_screen(2.0).expect("side cover projects");
        assert!(
            front > side,
            "the selected cover ({front}) must be wider on screen than a receded one ({side})"
        );
    }

    #[test]
    fn spring_settles_at_its_target() {
        let mut s = Spring { x: 0.0, v: 0.0 };
        for _ in 0..600 {
            s.step(5.0, 1.0 / 30.0, 13.0, 0.72);
        }
        assert!((s.x - 5.0).abs() < 1e-2, "spring settled at {}", s.x);
        assert!(s.v.abs() < 1e-2, "spring came to rest");
    }

    #[test]
    fn demo_motion_is_deterministic_and_sweeps_the_shelf() {
        let a = demo_selections();
        let b = demo_selections();
        assert_eq!(a, b, "scripted motion must be deterministic");
        assert!(!a.is_empty());
        assert!(
            a[0].abs() < 0.5,
            "starts near the first cover, got {}",
            a[0]
        );
        let reach = a.iter().cloned().fold(f32::MIN, f32::max);
        assert!(
            reach > (NUM_COVERS - 2) as f32,
            "motion must sweep out to the far end of the shelf, reached {reach}"
        );
    }

    #[test]
    fn canvas_root_fills_the_screen_and_is_not_blank() {
        // Guards the black-screen regression: an unsized canvas collapses to zero.
        let covers: Vec<RgbRaster> = (0..NUM_COVERS).map(|i| cover_art(i as u32)).collect();
        let refl: Vec<RgbRaster> = covers.iter().map(reflection_texture).collect();
        let (cols, rows) = (100u16, 32u16);
        let mut ctx = Context::headless(gibson::RenderMode::Fullscreen, cols, rows);
        let c = Arc::new(covers);
        let r = Arc::new(refl);
        ctx.set_root(
            Node::canvas(move |rect| {
                render(
                    &c[..],
                    &r[..],
                    3.0,
                    rect.width.max(1),
                    rect.height.max(1),
                    false,
                )
            })
            .percent_width(100.0)
            .percent_height(100.0),
        );
        ctx.render().expect("headless render");
        let lines = ctx.last_frame_lines();
        assert_eq!(lines.len(), rows as usize, "canvas must fill every row");
        let non_blank: usize = lines
            .iter()
            .map(|l| l.chars().filter(|ch| !ch.is_whitespace()).count())
            .sum();
        assert!(
            non_blank > (cols as usize * rows as usize) / 4,
            "a filled canvas paints half-block cells across the screen; got {non_blank} non-blank"
        );
    }
}
