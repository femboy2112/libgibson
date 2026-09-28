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

/// The last narrative second the film plays; the edit clock and the narrative clock
/// advance 1:1 up to here, then this exact frame is held while the closing sting runs
/// on. Kept just inside the final (Planet/title) act so the hold is a real rendered
/// frame and the play->hold transition never rewinds.
pub const INTRO_LAST: f32 = 71.5;
/// Total edit-clock length of the cut (film + the held sting tail).
pub const EDIT_END: f32 = 78.0;

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
pub const CUES: [Cue; 10] = [
    // Welcome, as the agent harness establishes (intro ~4-10 s). Anchored right and
    // modest so the orchestration/workspace text stays readable beside the reaction.
    cue(
        "welcome",
        4.0,
        10.0,
        3.0,
        5.0,
        Framing::new(Fit::Contain, 0.72, 0.24, 0.14, false),
        Effect::Normal,
    ),
    // First impossible transition: the prank smashes in as harness -> membrane (~20 s).
    cue(
        "prank_smash",
        19.0,
        22.0,
        46.5,
        3.0,
        Framing::new(Fit::Contain, 0.8, -0.24, 0.05, false),
        Effect::SmashIn { from: 2.4 },
    ),
    // Escalation microcut A: a punch-in as the membrane distorts (~24 s).
    cue(
        "escalate_punch",
        24.0,
        27.0,
        64.0,
        3.0,
        Framing::new(Fit::Contain, 0.6, 0.2, 0.05, false),
        Effect::PunchZoom { peak: 1.8 },
    ),
    // Escalation microcut B: a one-shot aspect stretch, mirrored (~28 s, city onset).
    cue(
        "escalate_stretch",
        28.0,
        30.0,
        78.0,
        2.0,
        Framing::new(Fit::Contain, 0.72, -0.12, 0.0, true),
        Effect::AspectStretch { amp: 0.35 },
    ),
    // City reveal (~31-35 s): subject right, the impossible wireframe visible left.
    cue(
        "city_reveal",
        31.0,
        35.0,
        104.0,
        4.0,
        Framing::new(Fit::Contain, 0.62, 0.28, 0.06, false),
        Effect::Normal,
    ),
    // Facades (~37-42 s): a small edge-anchored PiP so the mounted micro-UIs stay.
    cue(
        "facades_pip",
        37.0,
        42.0,
        190.0,
        5.0,
        Framing::new(Fit::Contain, 0.46, 0.32, -0.04, false),
        Effect::QuietHold,
    ),
    // Couriers (~45-50 s): the film-reel hold, restrained punch, anchored left so the
    // routing on the right stays the focus.
    cue(
        "couriers_reel",
        45.0,
        50.0,
        201.0,
        5.0,
        Framing::new(Fit::Contain, 0.55, -0.22, 0.02, false),
        Effect::PunchZoom { peak: 1.3 },
    ),
    // Ascent (~54-59 s): the calm beat — small, still, right, as the city becomes a planet.
    cue(
        "ascent_quiet",
        54.0,
        59.0,
        247.0,
        5.0,
        Framing::new(Fit::Contain, 0.4, 0.33, 0.1, false),
        Effect::QuietHold,
    ),
    // Earth / title payoff (~62-70 s): a clean keyed reaction over the planet and title.
    cue(
        "earth_title",
        62.0,
        70.0,
        278.0,
        6.0,
        Framing::new(Fit::Contain, 0.55, 0.06, 0.06, false),
        Effect::Normal,
    ),
    // Final sting (~72-78 s): the base film holds its final frame; the edit clock runs on.
    cue(
        "final_sting",
        72.0,
        78.0,
        290.0,
        5.0,
        Framing::new(Fit::Contain, 0.58, 0.28, 0.06, false),
        Effect::Normal,
    ),
];

/// The base-intro narrative time for an edit time: 1:1 while the film plays, then the
/// held final frame for the closing sting. Monotonic nondecreasing — never rewinds.
pub fn intro_seconds(edit: f32) -> f32 {
    edit.clamp(0.0, INTRO_LAST)
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
