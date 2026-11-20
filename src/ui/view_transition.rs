//! A pure description of one view-to-view transition (experimental, Rust-only).
//!
//! Contract: `tests/view_transition.rs`. Design: `docs/design/v0.9-view-transitions.md`.
//!
//! A [`ViewTransition`] owns no clock, no tree and no hidden state. The application keeps
//! the outgoing and incoming views, samples the transition with its own `now`, and feeds
//! the returned [`SurfaceFx`] chains to [`super::Element::post_process`].

use std::time::Duration;

use super::motion::MotionPreference;
use crate::{Color, SurfaceFx};

/// Horizontal direction of a slide: the way the outgoing view leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Dir {
    /// The outgoing view leaves to the left; the incoming one enters from the right.
    Left,
    /// The outgoing view leaves to the right; the incoming one enters from the left.
    Right,
}

/// What a transition looks like at full motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TransitionKind {
    /// Both views move horizontally by whole cells (`SurfaceFx::Offset`).
    Slide(Dir),
    /// Both views fade through `toward` (`SurfaceFx::Blend`), normally the surface colour.
    Fade { toward: Color },
}

/// One transition between two views, sampled at caller-supplied times.
///
/// Construct it with [`ViewTransition::new`]; the type is `#[non_exhaustive]` so fields can
/// be added without a breaking change.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ViewTransition {
    /// When the transition began, on the caller's clock.
    pub started: Duration,
    /// Full-motion duration. `Reduced` caps it at 80 ms; `None` ignores it.
    pub duration: Duration,
    pub kind: TransitionKind,
}

/// `MotionPreference::Reduced` caps a transition at this length, as `MotionTokens::resolve` does.
const REDUCED_CAP: Duration = Duration::from_millis(80);

impl ViewTransition {
    pub fn new(started: Duration, duration: Duration, kind: TransitionKind) -> Self {
        Self {
            started,
            duration,
            kind,
        }
    }

    /// Eased progress in `0.0..=1.0` (ease-in-out). Before `started` it is 0; at and after
    /// the effective deadline it is exactly 1. `MotionPreference::None` is 1 immediately.
    /// Under `Reduced` a `Fade` is capped at 80 ms, and a `Slide` is a single cut: 1
    /// immediately, like `None`.
    pub fn progress(&self, now: Duration, pref: MotionPreference) -> f32 {
        let duration = match (pref, self.kind) {
            (MotionPreference::None, _) => return 1.0,
            (MotionPreference::Reduced, TransitionKind::Slide(_)) => return 1.0,
            (MotionPreference::Reduced, _) => self.duration.min(REDUCED_CAP),
            (MotionPreference::Full, _) => self.duration,
        };
        let elapsed = now.saturating_sub(self.started);
        if elapsed >= duration {
            return 1.0;
        }
        let x = elapsed.as_secs_f64() / duration.as_secs_f64();
        // Smoothstep: zero slope at both ends, monotone, exactly 0 at x = 0.
        (x * x * (3.0 - 2.0 * x)).clamp(0.0, 1.0) as f32
    }

    /// True once `progress` is 1: the app may drop the outgoing view and clear the fx.
    pub fn is_settled(&self, now: Duration, pref: MotionPreference) -> bool {
        self.progress(now, pref) >= 1.0
    }

    /// Whether pointer input should be delivered now. The decision is to freeze the
    /// pointer while the transition is not settled (hit targets lag the moving pixels);
    /// keys stay live.
    pub fn accepts_pointer(&self, now: Duration, pref: MotionPreference) -> bool {
        self.is_settled(now, pref)
    }

    /// `(outgoing, incoming)` effect chains for a body `width` cells wide.
    ///
    /// A slide yields one `Offset` per side with `dy == 0` and `dx` of opposite signs; a fade
    /// yields one `Blend` per side toward the same colour with complementary `t`. At and
    /// after settling both chains are empty, so the settled tree is the plain incoming view.
    ///
    /// Under `MotionPreference::Reduced` a `Fade` keeps its `Blend` toward its own colour,
    /// capped at 80 ms. A `Slide` is one cut at `t = 0`: it is settled at once, both chains
    /// are empty and the pointer is live, so the app shows the incoming view unmodified and
    /// drops the outgoing one, exactly as after any settled transition. There is no
    /// intermediate frame, so no dip to the terminal default colour.
    pub fn effects(
        &self,
        now: Duration,
        pref: MotionPreference,
        width: u16,
    ) -> (Vec<SurfaceFx>, Vec<SurfaceFx>) {
        let p = self.progress(now, pref);
        if p >= 1.0 {
            return (Vec::new(), Vec::new());
        }
        let fade = |toward: Color| {
            (
                vec![SurfaceFx::Blend { toward, t: p }],
                vec![SurfaceFx::Blend { toward, t: 1.0 - p }],
            )
        };
        match self.kind {
            TransitionKind::Fade { toward } => fade(toward),
            // Reduced slides settle at once (see `progress`), so only Full reaches here.
            TransitionKind::Slide(dir) => {
                let w = f32::from(width);
                let sign = match dir {
                    Dir::Left => 1,
                    Dir::Right => -1,
                };
                let out = -sign * (p * w).round() as i32;
                let inc = sign * ((1.0 - p) * w).round() as i32;
                (
                    vec![SurfaceFx::Offset { dx: out, dy: 0 }],
                    vec![SurfaceFx::Offset { dx: inc, dy: 0 }],
                )
            }
        }
    }
}
