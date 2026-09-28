//! Temporal display lab: a side-by-side research view of the experimental
//! temporal cell realization.
//!
//! **Default output is STATIC.** Residual temporal luminance modulation is
//! opt-in and requires BOTH `--temporal` and a *caller-supplied measured*
//! presentation profile (`--measured-hz=...`), because LibGibson cannot observe
//! presentation cadence through a PTY — you must measure it externally first with
//! `temporal_cadence_beacon`. Perceptual benefit is setup-dependent and unproven;
//! this lab is a research instrument, not a demonstration of a finished feature.
//!
//! It renders the same procedural target two ways — the best static two-color
//! projection, and (when enabled and safe) that same projection plus a bounded
//! residual temporal correction — with a live diagnostics line showing the safety
//! gate, how many cells are modulating vs frozen, the worst emitted luminance
//! swing, and any missed phase-locked deadlines.
//!
//! Examples:
//!   cargo run --release --example temporal_display_lab
//!   cargo run --release --example temporal_display_lab -- --temporal --measured-hz=120
//!   cargo run --release --example temporal_display_lab -- --temporal --measured-hz=120 \
//!       --survival=0.97 --region=40x12 --seconds=15
//!
//! Flags: `--hz`, `--seconds`, `--region=WxH`, `--temporal`, `--measured-hz`,
//! `--survival`, `--jitter-p95` (measured p95 presentation jitter, ms),
//! `--depth-cap` (emitted linear-luminance swing cap, `0..=1`; the forced-choice
//! staircase varies this), `--reduced-motion`, `--sync`/`--no-sync` (synchronized
//! update frames), `--dither`/`--no-dither` (residual threshold dither; on by
//! default). Safety is resolved against the real terminal color depth.

use gibson::{
    BorderType, Context, FramePacing, Node, PresentationProfile, ResetPolicy, Style,
    SubcellGlyphMode, TemporalDiagnostics, TemporalDisplayProcessor, TemporalGate,
    TemporalSafetyPolicy,
};
use std::io;
use std::sync::Arc;
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

fn arg_str(name: &str) -> Option<String> {
    let prefix = format!("--{name}=");
    std::env::args().find_map(|a| a.strip_prefix(&prefix).map(|v| v.to_string()))
}

fn has(flag: &str) -> bool {
    std::env::args().any(|a| a == flag)
}

/// A gentle diagonal grayscale ramp. Within each cell the eight subpixels span a
/// small range (fractional duty => something for the residual to modulate), while
/// neighbouring cells stay close in luminance (low emitted swing => modulatable
/// rather than frozen). This is the smooth-gradient regime where residual tonal
/// dithering could plausibly help.
fn sample(cols: u16, rows: u16, lx: u16, ly: u16) -> [u8; 3] {
    let span = (2 * cols as u32 + 4 * rows as u32).max(1);
    let v = 30 + ((lx as u32 + ly as u32) * 190 / span);
    let v = v.min(255) as u8;
    [v, v, v]
}

fn diagnostics_line(d: &TemporalDiagnostics) -> String {
    format!(
        "gate={:?}  depth={:?}  modulating={}  cells: {} modulatable / {} frozen  \
         worst-swing={:.3}  static-RMSE={:.4}  missed-periods={}",
        d.gate,
        d.color_depth,
        d.modulating,
        d.modulatable_cells,
        d.frozen_cells,
        d.worst_cell_swing,
        d.mean_emitted_static_rmse,
        d.degraded_hold_frames,
    )
}

fn main() -> io::Result<()> {
    let hz = arg_u32("hz", 120).clamp(1, 1000);
    let seconds = arg_u32("seconds", 10).clamp(1, 120);
    let frames = hz as u64 * seconds as u64;
    let (cols, rows) = match arg_str("region").and_then(|s| {
        let (w, h) = s.split_once('x')?;
        Some((w.parse().ok()?, h.parse().ok()?))
    }) {
        Some((w, h)) => (u16::clamp(w, 4, 120), u16::clamp(h, 2, 60)),
        None => (30u16, 10u16),
    };

    let temporal = has("--temporal");
    let measured_hz = arg_f32("measured-hz");
    let survival = arg_f32("survival").unwrap_or(0.95);
    let jitter_p95 = arg_f32("jitter-p95").unwrap_or(0.5);
    let depth_cap = arg_f32("depth-cap");
    let reduced_motion = has("--reduced-motion");
    let dither = !has("--no-dither");
    let sync = !has("--no-sync");

    let mut processor =
        TemporalDisplayProcessor::new(cols, rows, SubcellGlyphMode::Braille2x4, 0x7E5);
    processor.set_dither(dither);
    if reduced_motion {
        processor.set_reduced_motion(true);
    }
    // Optional custom emitted-swing depth cap for the forced-choice staircase.
    // `TemporalSafetyPolicy` is #[non_exhaustive], so mutate fields off Default
    // rather than a struct literal (the supported external idiom).
    if let Some(cap) = depth_cap {
        let mut policy = TemporalSafetyPolicy::default();
        policy.max_luminance_depth = cap;
        processor.set_policy(policy);
    }
    // Temporal luminance is only eligible with an explicitly supplied *measured*
    // profile. Without one the processor stays unmeasured and renders static.
    if temporal {
        if let Some(mhz) = measured_hz {
            processor.set_profile(PresentationProfile::measured(mhz, survival, jitter_p95));
        } else {
            eprintln!(
                "note: --temporal ignored without --measured-hz=<observed presentation Hz>; \
                 measure it with temporal_cadence_beacon first. Rendering static."
            );
        }
    }
    processor.set_target_image(|lx, ly| sample(cols, rows, lx, ly), ResetPolicy::Reset);

    eprintln!(
        "temporal display lab: {cols}x{rows} cells @ {hz} fps for {seconds}s; gate={:?}. \
         Default is static; temporal residual is experimental and setup-dependent.",
        processor.gate()
    );

    let mut ctx = Context::fullscreen()?;
    if !ctx.is_interactive() {
        return Err(io::Error::other(
            "temporal_display_lab requires an interactive TTY",
        ));
    }
    ctx.set_max_fps(hz);
    ctx.set_frame_pacing(FramePacing::PhaseLocked);
    ctx.set_sync_updates(sync);
    // Resolve the emitted-swing safety against the terminal's real color depth, so
    // ANSI256/ANSI16/Mono freeze exactly the cells whose *wire* colors would swing
    // past the cap (reclassifies the installed target in place).
    processor.set_color_depth(ctx.capabilities().color_depth);

    let header = if matches!(processor.gate(), TemporalGate::Enabled) {
        "TEMPORAL DISPLAY LAB  —  residual temporal dithering ENABLED (experimental, unproven)"
    } else {
        "TEMPORAL DISPLAY LAB  —  STATIC (temporal disabled: unmeasured/gated). Left == Right."
    };

    for _ in 0..frames {
        let missed = ctx.missed_periods_last_frame();
        let temporal_surface = processor.advance(missed);
        let static_surface = processor.static_fallback();
        let diag = processor.diagnostics();

        let root = Node::col()
            .child(Node::text(header, Style::default()))
            .child(
                Node::row()
                    .child(
                        Node::panel("STATIC", BorderType::Rounded, Style::default())
                            .child(Node::surface(Arc::new(static_surface))),
                    )
                    .child(
                        Node::panel("TEMPORAL", BorderType::Rounded, Style::default())
                            .child(Node::surface(Arc::new(temporal_surface))),
                    ),
            )
            .child(Node::text(diagnostics_line(&diag), Style::default()));

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
