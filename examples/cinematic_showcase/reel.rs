//! The showcase film's **edit** — the single authoritative clock. One deterministic
//! cue sheet of hard cuts on one edit clock (seconds; capture runs at 20 fps), pure
//! data: which shot plays when. Nothing here renders anything; the frame driver
//! reads it through [`ShowTimeline::top`] to decide the active shot, its HUD and the
//! title overlays, and `stage` realizes the camera for the same edit time.
//!
//! Each cue's window is exactly the camera cue sheet's budget for that shot
//! ([`stage::EST`], [`stage::PANEL`], [`stage::REVEAL`]), so there is **one** set of
//! timing numbers in the film: the picture the camera flies and the shot the
//! timeline names can never drift apart. That is what killed the old two-clock
//! split — the reel used to carry its own longer, dissolve-based timing that the
//! executable never actually flew.
//!
//! The film is one gapless sweep:
//!
//! 1. **Establish** — the ring from far, under the title overlay (a hard cut in).
//! 2. **The grammars** — all six experience grammars in [`GrammarId::REEL_ORDER`]:
//!    the deliberately plain `Standard` list first (the "floor"), then escalating
//!    spatiality up to the `Orbital` ring.
//! 3. **The Observatory** — the three `gibson::plot` scenes in [`SceneId::ALL`]
//!    (scope → spectrum → phase space), signal → resonance → structure.
//! 4. **Reveal** — the rise-out, under the sign-off overlay.
//!
//! Demo-local film direction, deliberately not a framework.

#![allow(dead_code)]

use crate::shot::{GrammarId, SceneId, Shot};
use crate::show_timeline::ShowTimeline;
use crate::stage;

/// The showcase film's edit, as one deterministic cue sheet on a single edit clock.
///
/// Gapless hard cuts, each window the camera's budget for that shot: establish
/// (`EST`), the six grammars then the three scenes (each one `PANEL`, laid out in
/// the order the ring hangs its nine planes), and the reveal (`REVEAL`). The total
/// equals [`stage::duration`] by construction.
pub fn reel() -> ShowTimeline<Shot> {
    let mut tl = ShowTimeline::new().cut(stage::EST, Shot::Establish);
    for g in GrammarId::REEL_ORDER {
        tl = tl.cut(stage::PANEL, Shot::Grammar(g));
    }
    for s in SceneId::ALL {
        tl = tl.cut(stage::PANEL, Shot::Observatory(s));
    }
    tl.cut(stage::REVEAL, Shot::Reveal)
}
