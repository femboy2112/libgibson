//! The showcase film's **edit** — one deterministic cue sheet on a single edit
//! clock (seconds; capture runs at 20 fps). Pure data: which shot plays when.
//! Nothing here renders anything; the frame driver reads it through
//! [`ShowTimeline::resolve`] / [`ShowTimeline::top`].
//!
//! The film is three movements bracketed by a title and a sign-off:
//!
//! 1. **Cold open** — the film title, a hard cut in. A dip reads cleaner than a
//!    dissolve against a title card, so every card is entered and left on a cut.
//! 2. **THE GRAMMARS** — all six experience grammars in
//!    [`GrammarId::REEL_ORDER`]: the deliberately plain `Standard` list first (the
//!    "floor", with the GL layer off on purpose), then escalating spatiality up to
//!    the `Orbital` ring, which gets the longest hold because it is the payoff.
//!    Inside the act the shots dissolve into each other: same library, same data,
//!    a different grammar — the dissolve *is* the argument.
//! 3. **THE OBSERVATORY** — the three `gibson::plot` scenes (scope, spectrum,
//!    phase space) as their own movement, ordered signal → resonance → structure,
//!    so the film ends on the plot layer "precipitating" meaning from noise.
//! 4. **Sign-off** — the closing title.
//!
//! Holds over churn: no cue is shorter than a title card's 2.5 s, and the
//! grammars/scenes each get 7+ seconds so the motion has time to settle and be
//! read. Demo-local film direction, deliberately not a framework.

#![allow(dead_code)]

use crate::shot::{GrammarId, SceneId, Shot, TitleCard};
use crate::show_timeline::ShowTimeline;

/// Crossfade length (seconds) between adjacent content shots within an act.
const DISSOLVE: f32 = 0.6;

/// How long each grammar holds. `Standard` is brisk — it is the plain floor, the
/// point is the contrast — and `Orbital` lingers as the act's climax.
fn grammar_hold(g: GrammarId) -> f32 {
    match g {
        GrammarId::Standard => 7.0,
        GrammarId::Orbital => 9.0,
        _ => 7.5,
    }
}

/// How long each Observatory scene holds.
fn scene_hold(s: SceneId) -> f32 {
    match s {
        SceneId::PhaseSpace => 7.5,
        _ => 7.0,
    }
}

fn card(l1: &str, l2: &str) -> Shot {
    Shot::Title(TitleCard::new(l1, l2))
}

/// The showcase film's edit, as one deterministic cue sheet on a single edit clock.
pub fn reel() -> ShowTimeline<Shot> {
    // Cold open and the act-one card: hard cuts around title cards.
    let mut tl = ShowTimeline::new()
        .cut(
            3.5,
            card("GIBSON LIBRARY", "one experience · many grammars"),
        )
        .cut(2.5, card("THE GRAMMARS", "six ways to hold the same shelf"));

    // Act one: the first grammar cuts in off the card, the rest dissolve.
    for (i, g) in GrammarId::REEL_ORDER.into_iter().enumerate() {
        tl = if i == 0 {
            tl.cut(grammar_hold(g), Shot::Grammar(g))
        } else {
            tl.crossfade(grammar_hold(g), DISSOLVE, Shot::Grammar(g))
        };
    }

    // Act two: a card marks the change of register, then the instruments.
    tl = tl.cut(
        2.5,
        card("THE OBSERVATORY", "gibson::plot — observable geometry"),
    );
    for (i, s) in SceneId::ALL.into_iter().enumerate() {
        tl = if i == 0 {
            tl.cut(scene_hold(s), Shot::Observatory(s))
        } else {
            tl.crossfade(scene_hold(s), DISSOLVE, Shot::Observatory(s))
        };
    }

    // Sign-off.
    tl.cut(
        4.0,
        card("gibson::ui · gibson::plot", "observable instruments"),
    )
}
