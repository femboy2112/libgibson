//! Temporal Observatory — the LibGibson temporal-rendering flagship.
//!
//! A procedurally-generated cinematic scene (a gravitationally-lensed black hole
//! with a Doppler-beamed accretion disk over a lensed starfield) rendered as the
//! **exact static two-color Braille projection** LibGibson always produces — and,
//! when the camera settles and a *measured* high-refresh presentation profile is
//! supplied, refined in place by the experimental residual temporal path.
//!
//! There is no external asset and there is **one renderer**: every frame is an
//! ordinary [`gibson::Surface`] composed through an ordinary [`gibson::Node`].
//!
//! ## The acquire-lock behaviour (the signature)
//! While the camera slews, the whole field is held static (moving content is never
//! temporally modulated). When motion stops the instrument *acquires a lock*: a
//! brief static hold, then — only if the presentation gate passes — the settled,
//! low-contrast regions begin temporally refining while sharp HUD/edge cells stay
//! frozen. This is scientific-acquisition language, not a decorative flicker.
//!
//! ## Honesty
//! **Default output is STATIC** and looks the same as the released engine. Temporal
//! refinement is opt-in and requires `--temporal` *and* a caller-supplied measured
//! profile (`--measured-hz=...`), because LibGibson cannot observe presentation
//! cadence through a PTY — measure it first with `temporal_cadence_beacon`. Any
//! perceptual gain is setup-dependent and unproven; on an unmeasured or 60 Hz path
//! the observatory simply renders the (already high-quality) static projection.
//!
//! Examples:
//!   cargo run --release --example temporal_observatory
//!   cargo run --release --example temporal_observatory -- --temporal --measured-hz=120
//!   cargo run --release --example temporal_observatory -- --seconds=30 --region=140x44

use gibson::{
    BorderType, Color, Context, FramePacing, Node, PresentationProfile, ResetPolicy, Style,
    SubcellGlyphMode, TemporalDiagnostics, TemporalDisplayProcessor, TemporalSafetyPolicy,
};
use std::io;
use std::time::Duration;

fn arg_u32(name: &str, default: u32) -> u32 {
    let prefix = format!("--{name}=");
    std::env::args()
        .find_map(|a| a.strip_prefix(&prefix).and_then(|v| v.parse().ok()))
        .unwrap_or(default)
}

fn arg_f32(name: &str) -> Option<f32> {
    let prefix = format!("--{name}=");
    std::env::args().find_map(|a| a.strip_prefix(&prefix).and_then(|v| v.parse().ok()))
}

fn arg_region() -> Option<(u16, u16)> {
    let s = std::env::args().find_map(|a| a.strip_prefix("--region=").map(|v| v.to_string()))?;
    let (w, h) = s.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

fn has(flag: &str) -> bool {
    std::env::args().any(|a| a == flag)
}

/// Deterministic 2D hash in `[0, 1)`.
fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / (0x0100_0000u32 as f32)
}

/// Accretion-disk temperature ramp: inner hot blue-white -> outer warm orange-red.
fn temperature_ramp(t: f32) -> [f32; 3] {
    const STOPS: [[f32; 3]; 4] = [
        [205.0, 224.0, 255.0],
        [255.0, 240.0, 205.0],
        [255.0, 170.0, 85.0],
        [186.0, 66.0, 28.0],
    ];
    let x = t.clamp(0.0, 1.0) * 3.0;
    let i = (x.floor() as usize).min(2);
    let f = x - i as f32;
    [
        STOPS[i][0] + (STOPS[i + 1][0] - STOPS[i][0]) * f,
        STOPS[i][1] + (STOPS[i + 1][1] - STOPS[i][1]) * f,
        STOPS[i][2] + (STOPS[i + 1][2] - STOPS[i][2]) * f,
    ]
}

/// Lensed starfield + faint nebula background at normalized coords `(px, py)` with
/// circular radius `rr`. Near the hole the apparent star positions are deflected
/// outward (a cheap gravitational-lensing stand-in); `orbit` gives slow parallax.
fn starfield(px: f32, py: f32, rr: f32, orbit: f32) -> [f32; 3] {
    let deflect = 1.0 + 0.10 / (rr * rr + 0.02);
    let (sx, sy) = (px * deflect, py * deflect);
    let (s, c) = (orbit * 0.15).sin_cos();
    let (rx, ry) = (sx * c - sy * s, sx * s + sy * c);

    // Faint smooth nebula (low-swing -> a good temporal-refinement region), kept
    // dim so the bright disk dominates.
    let neb = 3.0 + 3.5 * (px * 1.7 + py * 0.6).sin().abs();
    let mut col = [neb * 0.45, neb * 0.35, neb];

    // Sparse stars on a hashed grid.
    let (gx, gy) = ((rx * 44.0).floor() as i32, (ry * 44.0).floor() as i32);
    let h = hash2(gx, gy);
    if h > 0.991 {
        let b = 150.0 + 105.0 * (h - 0.991) / 0.009;
        col = [b, b, (b * 0.94).min(255.0)];
    }
    col
}

/// The full procedural scene for one logical Braille subpixel `(lx, ly)` in the
/// `2*cols` by `4*rows` grid. `orbit` rotates the Doppler beaming and starfield.
fn scene(lx: u16, ly: u16, cols: u16, rows: u16, orbit: f32) -> [u8; 3] {
    let scale = (2.0 * rows as f32).max(1.0);
    let px = (lx as f32 - cols as f32) / scale;
    let py = (ly as f32 - 2.0 * rows as f32) / scale;

    let rr = (px * px + py * py).sqrt(); // circular radius (horizon, lensing)
    let ry = py * 2.3; // vertical flatten -> elliptical accretion disk
    let r = (px * px + ry * ry).sqrt();
    let ang = py.atan2(px);

    // Event horizon: a black disk.
    let r_h = 0.16;
    if rr < r_h {
        return [0, 0, 0];
    }

    let mut col = starfield(px, py, rr, orbit);

    // Photon ring hugging the horizon, plus the broad accretion disk.
    let ring = (-((rr - r_h * 1.18) / 0.018).powi(2)).exp();
    let disk = (-((r - 0.52) / 0.26).powi(2)).exp();
    let doppler = (1.0 + 0.75 * (ang - orbit).cos()).max(0.0);
    let intensity = (disk * doppler + ring * 1.5).min(2.2);
    let temp = temperature_ramp(((r - r_h) / 0.7).clamp(0.0, 1.0));
    for k in 0..3 {
        col[k] = (col[k] + temp[k] * intensity).min(255.0);
    }

    // HUD reticle: a faint cyan crosshair + one range ring. High-contrast on the
    // dark field -> large emitted swing -> the safety gate freezes these cells, so
    // the instrument frame stays razor-sharp while the smooth disk refines.
    let cross = (px.abs() < 0.006 || py.abs() < 0.006) && rr > r_h * 1.4 && rr < 0.95;
    let range_ring = (rr - 0.72).abs() < 0.004;
    if cross || range_ring {
        col[0] = (col[0] + 40.0).min(255.0);
        col[1] = (col[1] + 150.0).min(255.0);
        col[2] = (col[2] + 150.0).min(255.0);
    }

    [col[0] as u8, col[1] as u8, col[2] as u8]
}

#[derive(Clone, Copy, PartialEq)]
enum Acq {
    Tracking,
    StaticLock,
    Acquiring,
    Resolved,
}

fn acq_line(acq: Acq, d: &TemporalDiagnostics) -> (String, Color) {
    match acq {
        Acq::Tracking => (
            "◈ TRACKING — camera slew (static, no temporal on moving content)".into(),
            Color::Rgb(240, 200, 90),
        ),
        Acq::StaticLock => (
            "◈ STATIC LOCK — acquiring…".into(),
            Color::Rgb(120, 210, 240),
        ),
        Acq::Resolved => (
            format!(
                "● TEMPORAL LOCK — resolved ({} cells refining, {} frozen sharp)",
                d.active_cells, d.frozen_cells
            ),
            Color::Rgb(120, 240, 150),
        ),
        Acq::Acquiring => (
            format!(
                "◈ STATIC — gate {:?}; supply --temporal --measured-hz to acquire temporal lock",
                d.gate
            ),
            Color::Rgb(170, 170, 180),
        ),
    }
}

fn telemetry(d: &TemporalDiagnostics, exact_changed: usize, missed: u32, orbit: f32) -> String {
    format!(
        "gate={:?} depth={:?} | cells: {} refine / {} frozen / {} sharp-frozen | swing={:.3} \
         static-RMSE={:.4} | wire: {} cells changed, missed={} | orbit={:.2}rad",
        d.gate,
        d.color_depth,
        d.active_cells,
        d.motion_static_cells,
        d.frozen_cells,
        d.worst_cell_swing,
        d.mean_emitted_static_rmse,
        exact_changed,
        missed,
        orbit,
    )
}

fn main() -> io::Result<()> {
    let seconds = arg_u32("seconds", 24).clamp(2, 600);
    let hz = arg_u32("hz", 60).clamp(1, 1000);
    let temporal = has("--temporal");
    let measured_hz = arg_f32("measured-hz");
    let survival = arg_f32("survival").unwrap_or(0.97);
    let jitter_p95 = arg_f32("jitter-p95").unwrap_or(0.5);
    let depth_cap = arg_f32("depth-cap");
    let reduced_motion = has("--reduced-motion");
    let dither = !has("--no-dither");
    let sync = !has("--no-sync");

    let (tw, th) = arg_region()
        .or_else(|| crossterm::terminal::size().ok())
        .unwrap_or((110, 34));
    // Header (1) + footer status (1) + acquisition line (1) + panel border (2).
    let cols = tw.saturating_sub(2).clamp(20, 400);
    let rows = th.saturating_sub(5).clamp(10, 200);

    let mut processor =
        TemporalDisplayProcessor::new(cols, rows, SubcellGlyphMode::Braille2x4, 0x0B17);
    processor.set_dither(dither);
    if reduced_motion {
        processor.set_reduced_motion(true);
    }
    if let Some(cap) = depth_cap {
        let mut policy = TemporalSafetyPolicy::default();
        policy.max_luminance_depth = cap;
        processor.set_policy(policy);
    }
    if temporal {
        if let Some(mhz) = measured_hz {
            processor.set_profile(PresentationProfile::measured(mhz, survival, jitter_p95));
        } else {
            eprintln!(
                "note: --temporal needs --measured-hz=<observed Hz>; measure with \
                 temporal_cadence_beacon. Rendering static."
            );
        }
    }

    let mut ctx = Context::fullscreen()?;
    if !ctx.is_interactive() {
        return Err(io::Error::other(
            "temporal_observatory requires an interactive TTY",
        ));
    }
    ctx.set_max_fps(hz);
    ctx.set_frame_pacing(FramePacing::PhaseLocked);
    ctx.set_sync_updates(sync);
    processor.set_color_depth(ctx.capabilities().color_depth);

    // Cinematic timeline: orbit, then settle-and-acquire, repeating.
    let orbit_secs = 3.5f32;
    let settle_secs = 5.5f32;
    let cycle = orbit_secs + settle_secs;
    let lock_frames = (hz / 2).max(1); // ~0.5 s acquisition hold
    let dt = 1.0 / hz as f32;

    let mut orbit = 0.6f32;
    let mut was_orbiting = true;
    let mut settle_frame = 0u32;

    // Seed the first projection.
    processor.set_target_image(
        |lx, ly| scene(lx, ly, cols, rows, orbit),
        ResetPolicy::Reset,
    );
    processor.set_motion_static(true);

    let frames = hz as u64 * seconds as u64;
    for frame in 0..frames {
        let t_in_cycle = (frame as f32 * dt) % cycle;
        let orbiting = t_in_cycle < orbit_secs;

        let acq;
        if orbiting {
            orbit += dt * 0.55;
            processor.set_motion_static(true);
            processor.set_target_image(
                |lx, ly| scene(lx, ly, cols, rows, orbit),
                ResetPolicy::Reset,
            );
            acq = Acq::Tracking;
            was_orbiting = true;
        } else {
            if was_orbiting {
                // Just settled: one crisp reprojection at the resting angle, then hold.
                processor.set_target_image(
                    |lx, ly| scene(lx, ly, cols, rows, orbit),
                    ResetPolicy::Reset,
                );
                processor.set_motion_static(true);
                settle_frame = 0;
                was_orbiting = false;
            } else {
                settle_frame += 1;
                if settle_frame == lock_frames {
                    // Release: settled low-swing regions may now temporally refine.
                    processor.set_motion_static(false);
                }
            }
            acq = if settle_frame < lock_frames {
                Acq::StaticLock
            } else if processor.diagnostics().modulating {
                Acq::Resolved
            } else {
                Acq::Acquiring
            };
        }

        let missed = ctx.missed_periods_last_frame();
        let hero = processor.advance(missed);
        let diag = processor.diagnostics();
        let (acq_text, acq_color) = acq_line(acq, &diag);

        let title = format!(
            "EVENT HORIZON OBSERVATORY   ·   LibGibson temporal display   ·   {cols}×{rows} cells"
        );
        let root = Node::col()
            .child(Node::text(
                title,
                Style::default().fg(Color::Rgb(150, 200, 235)),
            ))
            .child(
                Node::panel(
                    "OPTICAL",
                    BorderType::Rounded,
                    Style::default().fg(acq_color),
                )
                .flex_grow(1.0)
                .child(Node::raster(hero)),
            )
            .child(Node::text(acq_text, Style::default().fg(acq_color)))
            .child(Node::text(
                telemetry(&diag, ctx.last_exact_changed_cells(), missed, orbit),
                Style::default().fg(Color::Rgb(130, 130, 140)),
            ));

        ctx.set_root(root);
        while !ctx.render_if_due()? {
            let wait = ctx.time_until_next_frame();
            if wait.is_zero() {
                std::thread::yield_now();
            } else {
                std::thread::sleep(wait.min(Duration::from_millis(2)));
            }
        }
    }

    ctx.restore()?;
    Ok(())
}
