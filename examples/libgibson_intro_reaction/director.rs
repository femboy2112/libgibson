//! The reaction cut's two-clock director.
//!
//! There are two independent clocks and the original film's is never mutated:
//!
//! * the **edit clock** — the reaction cut's own timeline, what `--at` seeks;
//! * the **intro narrative clock** — the original 72-second film's deterministic
//!   time, which each cue may `Continue`, `Hold`, or `Slow` while its reaction
//!   plays. That lets a reaction extend past 72 s over a held final frame without
//!   pretending the original film ever became longer.
//!
//! Given an edit time this resolves, deterministically: which cue is active, the
//! base-intro narrative time to render, and the normalized source progress used to
//! pick a keyed frame. Nothing reads wall-clock time here.

#![allow(dead_code)]

use super::transform::{Effect, Fit, Framing};

/// How the base film's narrative clock behaves during a cue.
#[derive(Clone, Copy, Debug)]
pub enum IntroTime {
    /// Advance 1:1 with edit time from `intro_start`.
    Continue,
    /// Freeze the base film at `intro_start`.
    Hold,
    /// Advance at `factor` x edit rate from `intro_start`.
    Slow(f32),
}

/// One directed beat: a slice of the edit clock bound to a base-intro moment and a
/// source window, with a framing and a finite effect.
#[derive(Clone, Copy, Debug)]
pub struct Cue {
    pub name: &'static str,
    /// Duration on the edit clock (seconds).
    pub edit_dur: f32,
    /// Base narrative time at the cue's start.
    pub intro_start: f32,
    pub intro_time: IntroTime,
    /// Source window to bake (seconds into the clip).
    pub src_start: f32,
    pub src_dur: f32,
    pub framing: Framing,
    pub effect: Effect,
}

#[allow(clippy::too_many_arguments)] // a cue IS these fields; the table reads best positionally
const fn cue(
    name: &'static str,
    edit_dur: f32,
    intro_start: f32,
    intro_time: IntroTime,
    src_start: f32,
    src_dur: f32,
    framing: Framing,
    effect: Effect,
) -> Cue {
    Cue {
        name,
        edit_dur,
        intro_start,
        intro_time,
        src_start,
        src_dur,
        framing,
        effect,
    }
}

/// The directed cut. Source windows and framings are the maintainer cue sheet
/// (docs/FRANK_REACTION_CUT_PLAN.md) tightened by real visual inspection: the
/// subject is usually off to one side, so the framing keeps the intro visible.
pub const CUES: [Cue; 10] = [
    // Cold open over the agent harness; anchored right and modest so the left
    // orchestration and centre workspace text stay readable beside the reaction.
    cue(
        "cold_open",
        4.0,
        3.0,
        IntroTime::Continue,
        3.0,
        3.5,
        Framing::new(Fit::Contain, 0.72, 0.24, 0.14, false),
        Effect::Normal,
    ),
    // The first impossible transition (harness -> membrane): the prank smashes in
    // as the membrane begins reinterpreting the harness (intro ~23 s).
    cue(
        "prank_smash",
        2.5,
        23.0,
        IntroTime::Hold,
        46.5,
        3.0,
        Framing::new(Fit::Contain, 0.82, -0.24, 0.05, false),
        Effect::SmashIn { from: 2.4 },
    ),
    // Escalation microcut A: a punch-in as the membrane distorts.
    cue(
        "escalate_punch",
        3.0,
        24.0,
        IntroTime::Slow(1.2),
        64.0,
        3.0,
        Framing::new(Fit::Contain, 0.72, 0.16, 0.05, false),
        Effect::PunchZoom { peak: 1.9 },
    ),
    // Escalation microcut B: a one-shot aspect stretch, mirrored.
    cue(
        "escalate_stretch",
        2.5,
        27.0,
        IntroTime::Slow(1.2),
        78.0,
        2.5,
        Framing::new(Fit::Contain, 0.85, -0.12, 0.0, true),
        Effect::AspectStretch { amp: 0.35 },
    ),
    // City reveal: subject anchored right, the impossible wireframe visible left.
    cue(
        "city_reveal",
        4.0,
        30.0,
        IntroTime::Continue,
        104.0,
        4.0,
        Framing::new(Fit::Contain, 0.7, 0.28, 0.06, false),
        Effect::Normal,
    ),
    // Facades: a small edge-anchored PiP so the mounted micro-UIs stay the point.
    cue(
        "facades_pip",
        3.5,
        38.0,
        IntroTime::Continue,
        190.0,
        3.5,
        Framing::new(Fit::Contain, 0.5, 0.32, -0.04, false),
        Effect::QuietHold,
    ),
    // Couriers: the film-reel hold, a restrained punch, anchored left so the courier
    // routing on the right stays the focus.
    cue(
        "couriers_reel",
        4.0,
        45.0,
        IntroTime::Continue,
        201.0,
        4.0,
        Framing::new(Fit::Contain, 0.6, -0.2, 0.02, false),
        Effect::PunchZoom { peak: 1.3 },
    ),
    // Ascent: the calm beat — small, still, right, while the city becomes a planet.
    cue(
        "ascent_quiet",
        4.0,
        54.0,
        IntroTime::Continue,
        247.0,
        4.0,
        Framing::new(Fit::Contain, 0.42, 0.33, 0.1, false),
        Effect::QuietHold,
    ),
    // Earth / title payoff: a clean keyed reaction over the planet and title.
    cue(
        "earth_title",
        4.0,
        62.0,
        IntroTime::Continue,
        278.0,
        4.0,
        Framing::new(Fit::Contain, 0.58, 0.06, 0.06, false),
        Effect::Normal,
    ),
    // Final sting: the base film holds its final frame; the edit clock runs on.
    cue(
        "final_sting",
        4.0,
        71.0,
        IntroTime::Hold,
        290.0,
        4.0,
        Framing::new(Fit::Contain, 0.6, 0.28, 0.06, false),
        Effect::Normal,
    ),
];

/// A fully resolved reaction frame request, deterministic in edit time.
#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub intro_seconds: f32,
    pub cue_index: usize,
    pub cue_local: f32,
    pub cue_dur: f32,
    /// Normalized progress across the active cue's source window, `[0,1]`.
    pub src_progress: f32,
}

/// Total edit-clock duration of the cut.
pub fn total_edit_seconds() -> f32 {
    CUES.iter().map(|c| c.edit_dur).sum()
}

/// The cumulative edit-clock start time of cue `i`.
pub fn cue_edit_start(i: usize) -> f32 {
    CUES[..i].iter().map(|c| c.edit_dur).sum()
}

/// Find the cue index active at edit time `t` (clamped into the cut).
pub fn cue_at(edit_seconds: f32) -> usize {
    let mut acc = 0.0;
    for (i, c) in CUES.iter().enumerate() {
        acc += c.edit_dur;
        if edit_seconds < acc {
            return i;
        }
    }
    CUES.len() - 1
}

/// Resolve an edit time to a base-intro time, active cue, and source progress.
pub fn resolve(edit_seconds: f32) -> Resolved {
    let total = total_edit_seconds();
    let t = edit_seconds.clamp(0.0, total);
    let i = cue_at(t);
    let cue = &CUES[i];
    let start = cue_edit_start(i);
    let cue_local = (t - start).clamp(0.0, cue.edit_dur);
    let intro_seconds = match cue.intro_time {
        IntroTime::Continue => cue.intro_start + cue_local,
        IntroTime::Hold => cue.intro_start,
        IntroTime::Slow(f) => cue.intro_start + f * cue_local,
    };
    let src_progress = if cue.edit_dur > 0.0 {
        (cue_local / cue.edit_dur).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Resolved {
        intro_seconds,
        cue_index: i,
        cue_local,
        cue_dur: cue.edit_dur,
        src_progress,
    }
}

/// Resolve a cue name to its index (for `--stage=`).
pub fn stage_index(name: &str) -> Option<usize> {
    CUES.iter().position(|c| c.name == name)
}
