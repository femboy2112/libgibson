//! A pure description of one view-to-view transition (experimental, Rust-only).
//!
//! RED stub: v0.9.0 design, not implemented. Every method below compiles and is
//! deliberately wrong; `tests/view_transition.rs` defines the contract and fails until
//! it is implemented. Design: `docs/design/v0.9-view-transitions.md`.
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
    /// `Reduced` runs a Blend-only fade capped at 80 ms and never slides.
    #[allow(unused_variables)]
    pub fn progress(&self, now: Duration, pref: MotionPreference) -> f32 {
        // RED stub: v0.9.0 design, not implemented
        0.0
    }

    /// True once `progress` is 1: the app may drop the outgoing view and clear the fx.
    #[allow(unused_variables)]
    pub fn is_settled(&self, now: Duration, pref: MotionPreference) -> bool {
        // RED stub: v0.9.0 design, not implemented
        false
    }

    /// Whether pointer input should be delivered now. The decision is to freeze the
    /// pointer while the transition is not settled (hit targets lag the moving pixels);
    /// keys stay live.
    #[allow(unused_variables)]
    pub fn accepts_pointer(&self, now: Duration, pref: MotionPreference) -> bool {
        // RED stub: v0.9.0 design, not implemented
        true
    }

    /// `(outgoing, incoming)` effect chains for a body `width` cells wide.
    ///
    /// A slide yields one `Offset` per side with `dy == 0` and `dx` of opposite signs; a fade
    /// yields one `Blend` per side toward the same colour with complementary `t`. At and
    /// after settling both chains are empty, so the settled tree is the plain incoming view.
    #[allow(unused_variables)]
    pub fn effects(
        &self,
        now: Duration,
        pref: MotionPreference,
        width: u16,
    ) -> (Vec<SurfaceFx>, Vec<SurfaceFx>) {
        // RED stub: v0.9.0 design, not implemented
        (Vec::new(), Vec::new())
    }
}
