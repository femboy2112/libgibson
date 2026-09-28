//! Deterministic, finite transform + effect vocabulary for the reaction cut.
//!
//! This is not a general video-editing framework. It is exactly the small set of
//! affine placements and time-bounded effect envelopes the directed cut needs, each
//! a pure function of a cue's normalized local progress `p in [0,1]` so that a
//! reaction beat is fully determined by edit time (seekable, replayable, testable).
//!
//! A [`Transform`] maps the keyed source image into the intro's *logical subpixel*
//! grid (`2*cols` by `4*rows`). The compositor inverse-samples it (dest subpixel ->
//! source pixel), so a scale/stretch never punches holes in the subject.

#![allow(dead_code)]

use std::f32::consts::PI;

/// How the source rectangle is fitted into the destination grid before any
/// per-cue scale/offset is applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Preserve source aspect, fit entirely inside the grid (letterbox). Default.
    Contain,
    /// Preserve source aspect, cover the grid (crop overflow).
    Cover,
    /// Independent x/y scale to exactly fill the grid (distorts aspect).
    Stretch,
}

impl Fit {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "contain" => Some(Fit::Contain),
            "cover" => Some(Fit::Cover),
            "stretch" => Some(Fit::Stretch),
            _ => None,
        }
    }
}

/// A cue's base framing: how the subject sits when no effect is displacing it.
/// `off_x`/`off_y` are fractions of the destination grid (0 = centred, +0.25 =
/// a quarter-grid toward the right/bottom).
#[derive(Clone, Copy, Debug)]
pub struct Framing {
    pub fit: Fit,
    pub scale: f32,
    pub off_x: f32,
    pub off_y: f32,
    pub mirror: bool,
}

impl Framing {
    pub const fn new(fit: Fit, scale: f32, off_x: f32, off_y: f32, mirror: bool) -> Self {
        Self {
            fit,
            scale,
            off_x,
            off_y,
            mirror,
        }
    }
}

/// A finite, deterministic effect envelope evaluated over a cue's local progress.
#[derive(Clone, Copy, Debug)]
pub enum Effect {
    /// The subject sits at its framing; no time modulation.
    Normal,
    /// A quick zoom-in to `peak` over the first ~quarter of the cue, then hold —
    /// strongest when a normal composition existed immediately before it.
    PunchZoom { peak: f32 },
    /// The subject slams in oversized and offset, snapping to framing by ~18%.
    SmashIn { from: f32 },
    /// A one-shot aspect squash/stretch that swells and returns within the cue.
    AspectStretch { amp: f32 },
    /// Deliberate restraint: the subject holds essentially still (the calm beat).
    QuietHold,
    /// The source frame is quantized into `steps` held chunks with a tiny
    /// deterministic positional jitter — a stutter, not a smooth play.
    FreezeStutter { steps: f32 },
}

impl Effect {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "normal" => Some(Effect::Normal),
            "punch" => Some(Effect::PunchZoom { peak: 1.8 }),
            "smash" => Some(Effect::SmashIn { from: 2.4 }),
            "stretch" => Some(Effect::AspectStretch { amp: 0.35 }),
            "quiet" => Some(Effect::QuietHold),
            "stutter" => Some(Effect::FreezeStutter { steps: 6.0 }),
            _ => None,
        }
    }
}

/// The resolved per-frame modulation an effect contributes at progress `p`.
#[derive(Clone, Copy, Debug)]
pub struct Beat {
    /// Uniform scale multiplier on top of the framing scale.
    pub scale_mul: f32,
    /// Extra destination-fraction offset added to the framing offset.
    pub off_dx: f32,
    pub off_dy: f32,
    /// Aspect skew: `sx *= aspect`, `sy /= aspect` (1.0 = no skew).
    pub aspect: f32,
    /// If set, overrides the normalized source progress used to pick a frame
    /// (`[0,1]` across the cue's baked window) — the stutter/freeze hook.
    pub src_progress: Option<f32>,
    /// Chromatic-offset magnitude in dest subpixels (0 = off), a restrained beat.
    pub chroma: f32,
}

impl Beat {
    pub const IDENTITY: Beat = Beat {
        scale_mul: 1.0,
        off_dx: 0.0,
        off_dy: 0.0,
        aspect: 1.0,
        src_progress: None,
        chroma: 0.0,
    };
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Deterministic tiny hash in `[0,1)` for stutter jitter.
fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x = x.wrapping_mul(2_246_822_519);
    x ^= x >> 13;
    (x & 0xffff) as f32 / 65536.0
}

/// Evaluate an effect at normalized cue progress `p in [0,1]`.
pub fn resolve_effect(effect: Effect, p: f32) -> Beat {
    let p = p.clamp(0.0, 1.0);
    match effect {
        Effect::Normal => Beat::IDENTITY,
        Effect::PunchZoom { peak } => {
            let k = smoothstep((p / 0.25).min(1.0));
            Beat {
                scale_mul: 1.0 + (peak - 1.0) * k,
                ..Beat::IDENTITY
            }
        }
        Effect::SmashIn { from } => {
            if p < 0.18 {
                let k = smoothstep(p / 0.18);
                Beat {
                    scale_mul: from + (1.0 - from) * k,
                    off_dx: (1.0 - k) * 0.06,
                    off_dy: (1.0 - k) * -0.04,
                    ..Beat::IDENTITY
                }
            } else {
                Beat::IDENTITY
            }
        }
        Effect::AspectStretch { amp } => Beat {
            aspect: 1.0 + amp * (PI * p).sin(),
            ..Beat::IDENTITY
        },
        Effect::QuietHold => Beat {
            // A barely-there drift so the calm beat is not mechanically frozen.
            off_dx: 0.004 * (2.0 * PI * p).sin(),
            ..Beat::IDENTITY
        },
        Effect::FreezeStutter { steps } => {
            let steps = steps.max(1.0);
            let step = (p * steps).floor();
            let held = (step / steps).min(1.0);
            let j = step as u32;
            Beat {
                off_dx: (hash01(j) - 0.5) * 0.02,
                off_dy: (hash01(j ^ 0x9e37) - 0.5) * 0.02,
                src_progress: Some(held),
                ..Beat::IDENTITY
            }
        }
    }
}

/// A resolved affine placement of a `film_w x film_h` source into a `subw x subh`
/// destination subpixel grid. `sx`/`sy` are dest-subpixels per source-pixel;
/// `(ox, oy)` is the dest-subpixel position of source pixel (0,0).
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub sx: f32,
    pub sy: f32,
    pub ox: f32,
    pub oy: f32,
    pub mirror: bool,
    pub chroma: f32,
}

impl Transform {
    /// Compose framing + effect beat into a concrete placement.
    pub fn resolve(
        framing: &Framing,
        beat: &Beat,
        film_w: u32,
        film_h: u32,
        subw: u32,
        subh: u32,
    ) -> Transform {
        let (fw, fh) = (film_w.max(1) as f32, film_h.max(1) as f32);
        let (dw, dh) = (subw.max(1) as f32, subh.max(1) as f32);
        let (mut sx, mut sy) = match framing.fit {
            Fit::Contain => {
                let s = (dw / fw).min(dh / fh);
                (s, s)
            }
            Fit::Cover => {
                let s = (dw / fw).max(dh / fh);
                (s, s)
            }
            Fit::Stretch => (dw / fw, dh / fh),
        };
        let scale = (framing.scale * beat.scale_mul).max(0.01);
        sx *= scale * beat.aspect;
        sy *= scale / beat.aspect;
        let span_w = fw * sx;
        let span_h = fh * sy;
        let ox = (dw - span_w) * 0.5 + (framing.off_x + beat.off_dx) * dw;
        let oy = (dh - span_h) * 0.5 + (framing.off_y + beat.off_dy) * dh;
        Transform {
            sx,
            sy,
            ox,
            oy,
            mirror: framing.mirror,
            chroma: beat.chroma,
        }
    }

    /// Inverse map a destination subpixel centre to a source pixel coordinate
    /// (may fall outside `[0, film-1]`, meaning "no source here").
    #[inline]
    pub fn inverse(&self, dsx: f32, dsy: f32, film_w: u32, film_h: u32) -> (f32, f32) {
        let mut fx = (dsx - self.ox) / self.sx;
        let fy = (dsy - self.oy) / self.sy;
        if self.mirror {
            fx = (film_w.max(1) as f32 - 1.0) - fx;
        }
        let _ = film_h;
        (fx, fy)
    }

    /// The destination subpixel bounding box `(x, y, w, h)` the source covers,
    /// clamped to the grid. Empty (`w`/`h` = 0) if entirely off-grid.
    pub fn dest_bbox_sub(
        &self,
        film_w: u32,
        film_h: u32,
        subw: u32,
        subh: u32,
    ) -> (u32, u32, u32, u32) {
        let span_w = film_w as f32 * self.sx;
        let span_h = film_h as f32 * self.sy;
        let x0 = self.ox.floor().max(0.0);
        let y0 = self.oy.floor().max(0.0);
        let x1 = (self.ox + span_w).ceil().clamp(0.0, subw as f32);
        let y1 = (self.oy + span_h).ceil().clamp(0.0, subh as f32);
        if x1 <= x0 || y1 <= y0 {
            return (0, 0, 0, 0);
        }
        (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32)
    }
}
