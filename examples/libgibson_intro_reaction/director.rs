//! The reaction cut's two-clock director.
//!
//! Two independent clocks, and the original film's is never mutated:
//!
//! * the **edit clock** — the reaction cut's own timeline, what `--at`/`--stage` seek;
//! * the **intro narrative clock** — the original film's deterministic time.
//!
//! The intro plays as a *continuous spine*: for the first 72 s the edit clock and the
//! narrative clock advance together, so the original short film plays through exactly
//! as it always does (bootup prologue, agent harness, membrane, city, ascent, title —
//! no skips, no freeze-frame jumps). Reaction **beats** are windows on the edit clock
//! during which the keyed subject is composited *over* that continuously-playing film;
//! outside every window the film plays alone and stays fully readable. After the film
//! reaches its end the edit clock runs a little longer over a **held** final frame so
//! the closing sting can play without pretending the film became longer. Everything is
//! a deterministic function of edit time — nothing reads wall-clock on the frame path.

#![allow(dead_code)]

use super::transform::{Effect, Fit, Framing};

/// The last narrative second the film plays. The film is time-scaled to span the whole
/// reaction cut (see [`intro_seconds`]): the LibGibson story evolves continuously
/// underneath every reaction instead of ending partway through, so the Planet/title act
/// lands under the closing reaction.
pub const INTRO_LAST: f32 = 71.5;
/// Total edit-clock length of the cut. Each reaction plays its full source section (per
/// the maintainer cut sheet) at natural speed, back to back, so a clip is never truncated.
pub const EDIT_END: f32 = 132.0;

/// A reaction beat: a window on the edit clock during which the keyed subject is
/// composited over the (continuously playing) intro, with a framing and finite effect
/// and a source window baked from the clip. Windows are placed at the edit times whose
/// narrative moment matches the act being reacted to.
#[derive(Clone, Copy, Debug)]
pub struct Cue {
    pub name: &'static str,
    pub edit_start: f32,
    pub edit_end: f32,
    pub src_start: f32,
    pub src_dur: f32,
    pub framing: Framing,
    pub effect: Effect,
}

#[allow(clippy::too_many_arguments)] // a cue IS these fields; the table reads best positionally
const fn cue(
    name: &'static str,
    edit_start: f32,
    edit_end: f32,
    src_start: f32,
    src_dur: f32,
    framing: Framing,
    effect: Effect,
) -> Cue {
    Cue {
        name,
        edit_start,
        edit_end,
        src_start,
        src_dur,
        framing,
        effect,
    }
}

/// The directed cut. Edit-clock windows == narrative time (the intro plays through);
/// source windows + framings are the maintainer cue sheet tightened by real visual
/// inspection (the subject is usually off to one side, so the film stays visible).
pub const CUES: [Cue; 9] = [
    // Each edit window has the SAME duration as its source, so every clip plays at natural
    // speed and is never truncated in or out. Source in-points sit just past the compilation's
    // cross-dissolves (which key as ghost frames); some clips are delayed a beat on the edit
    // clock so the film plays underneath before the reaction lands.
    //
    // Welcome / "replace the Filthy Frank show with libgibson" (src 0-16 s), over the
    // bootup + agent harness. Anchored right so the workspace text stays readable.
    cue(
        "welcome",
        0.0,
        16.0,
        0.0,
        16.0,
        Framing::new(Fit::Contain, 0.72, 0.24, 0.14, false),
        Effect::Normal,
    ),
    // Prank 1 (src 46-50 s): the karate/dance prank, smashes in as the harness gives way to
    // the membrane. Out point at 50 s so the pose completes (it was cut a beat early at 49 s);
    // 45-50 s is one continuous shot, no hard cut.
    cue(
        "prank_smash",
        16.0,
        20.0,
        46.0,
        4.0,
        Framing::new(Fit::Contain, 0.8, -0.24, 0.05, false),
        Effect::SmashIn { from: 2.4 },
    ),
    // The "fuck you" rant (src 59-91 s): the long escalation, steady and central. In point at
    // 59 s — src 58 s is the bent-over tail of the previous bit plus a cross-dissolve half-frame
    // (the upright rant only begins at 59 s); window slid forward 1 s to clear it, full length kept.
    cue(
        "rant",
        20.0,
        52.0,
        59.0,
        32.0,
        Framing::new(Fit::Contain, 0.7, 0.08, 0.05, false),
        Effect::Normal,
    ),
    // Stop 1 — "it's time to stop" (src 101.3-116.3 s): subject right, the arms-extended push at
    // the city. In point at 101.3 s — the spoken line begins at ~101.45 s, so the window opens just
    // before it; still clear of the 100.5 s dissolve. Out point extended from 114.3 to 116.3 s so the
    // line finishes: speech runs in bursts to ~115.97 s, so 116.3 s lands in the breath-gap
    // (115.97-116.79 s) instead of guillotining a word at 114.3 s. Frank bends into a hunched pose
    // from ~114.2 s but stays fully keyed on-screen (no dissolve, no walk-off). The window now ends
    // flush against stop2's edit-69 in-point — the former 67-69 s film-alone gap is consumed, no overlap.
    cue(
        "stop1",
        54.0,
        69.0,
        101.3,
        15.0,
        Framing::new(Fit::Contain, 0.62, 0.28, 0.06, false),
        Effect::Normal,
    ),
    // Stop 2 (src 189-199.4 s): a restrained punch-in over the facades, holding the full spoken line
    // (he says "it's time to stop" twice; the second ends ~199.3 s as he walks out of the source
    // frame ~199.2 s). off_x pulled in to 0.08 so his exit stays on-screen instead of clipping to the
    // composite's right edge; video and audio run together to the end of the line.
    cue(
        "stop2",
        69.0,
        79.4,
        189.0,
        10.4,
        Framing::new(Fit::Contain, 0.6, 0.08, 0.05, false),
        Effect::PunchZoom { peak: 1.3 },
    ),
    // Stop 3 (src 201-211 s): the "time to STOP" clock-hug over the couriers/routing. In point
    // translated forward to 201 s — src 199-200 is a cross-dissolve; the subject picks up the giant
    // clock at ~203 s. Entrance delayed to edit 81 s so the film plays a beat before the gag.
    cue(
        "stop3",
        81.0,
        91.0,
        201.0,
        10.0,
        Framing::new(Fit::Contain, 0.62, 0.28, 0.06, false),
        Effect::Normal,
    ),
    // WTF / quiet (src 244-256 s, the full section): the calm beat as the city becomes a
    // planet. Plays its whole source window at natural speed — the clip is translated forward
    // on the edit clock to land at 1:35, not trimmed. Source ends at 256 s, just before the
    // subject walks out of frame and the clock bit begins at 257 s.
    cue(
        "wtf_quiet",
        95.0,
        107.0,
        244.0,
        12.0,
        Framing::new(Fit::Contain, 0.5, 0.3, 0.1, false),
        Effect::QuietHold,
    ),
    // Nice prank (src 276-286 s): the bent-over keyed reaction as the planet/title resolve. In
    // point at 276 s — src 275 s still shows the clock being set down (the clock-exit), so the
    // window starts a second later to clear that ghost; plays at natural speed.
    cue(
        "nice_prank",
        107.0,
        117.0,
        276.0,
        10.0,
        Framing::new(Fit::Contain, 0.6, 0.08, 0.06, false),
        Effect::Normal,
    ),
    // Idiot — the closer (src 286-301 s): the final reaction over the Planet/title payoff; starts
    // as the subject straightens from the nice-prank pose and runs to the arm-out presentation,
    // landing on the closing title.
    cue(
        "idiot",
        117.0,
        132.0,
        286.0,
        15.0,
        Framing::new(Fit::Contain, 0.58, 0.1, 0.06, false),
        Effect::Normal,
    ),
];

/// The base-intro narrative time for an edit time. The film is time-scaled so its whole
/// `INTRO_LAST` span spreads across the full `EDIT_END` cut — it plays continuously (and
/// slower) underneath every reaction and reaches its Planet/title finale under the closing
/// clip, instead of ending partway through. Monotonic nondecreasing — never rewinds.
pub fn intro_seconds(edit: f32) -> f32 {
    let t = edit.clamp(0.0, EDIT_END);
    (t / EDIT_END * INTRO_LAST).clamp(0.0, INTRO_LAST)
}

/// A fully resolved reaction-cut frame request, deterministic in edit time.
#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub intro_seconds: f32,
    /// The active reaction beat, or `None` in a gap (the film plays alone).
    pub cue_index: Option<usize>,
    pub cue_local: f32,
    pub cue_dur: f32,
    /// Normalized progress across the active cue's source window, `[0,1]`.
    pub src_progress: f32,
}

/// Total edit-clock duration of the cut.
pub fn total_edit_seconds() -> f32 {
    EDIT_END
}

/// The edit-clock start time of cue `i` (for `--stage=`).
pub fn cue_edit_start(i: usize) -> f32 {
    CUES[i].edit_start
}

/// The reaction beat active at edit time `t`, if any (windows are `[start, end)`).
pub fn active_cue(t: f32) -> Option<usize> {
    CUES.iter()
        .position(|c| t >= c.edit_start && t < c.edit_end)
}

/// Resolve an edit time to a base-intro time and the active reaction beat.
pub fn resolve(edit_seconds: f32) -> Resolved {
    let t = edit_seconds.clamp(0.0, EDIT_END);
    let intro_seconds = intro_seconds(t);
    match active_cue(t) {
        Some(i) => {
            let c = &CUES[i];
            let dur = c.edit_end - c.edit_start;
            let cue_local = (t - c.edit_start).clamp(0.0, dur);
            let src_progress = if dur > 0.0 {
                (cue_local / dur).clamp(0.0, 1.0)
            } else {
                0.0
            };
            Resolved {
                intro_seconds,
                cue_index: Some(i),
                cue_local,
                cue_dur: dur,
                src_progress,
            }
        }
        None => Resolved {
            intro_seconds,
            cue_index: None,
            cue_local: 0.0,
            cue_dur: 0.0,
            src_progress: 0.0,
        },
    }
}

/// Resolve a cue name to its index (for `--stage=`).
pub fn stage_index(name: &str) -> Option<usize> {
    CUES.iter().position(|c| c.name == name)
}
