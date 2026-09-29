//! **SongMap** — the song, defined once, upstream of every performance (Round IX).
//!
//! The chart every musician already knows before anybody plays: what the piece IS, independent of
//! the room it is played in ([`super::world::MusicWorld`]: timbre, production, the home mode) and
//! the idiom it is spoken in ([`super::language::MusicalLanguage`]: colour, surface, conversation).
//!
//! There is exactly one place a song is generated — [`SongMap::build`] — and exactly one place a
//! performance of it is generated — [`super::performance::PerformancePlan::from_song`], realized
//! by [`super::functor::perform`]. The law this module exists for:
//!
//! ```text
//!   Performance ──π──▶ SongMap        π(P) = S for every lawful performance P of S;
//!                                      the fiber π⁻¹(S) is the band.
//! ```
//!
//! **Song identity** (this object): the coherence contract, the form graph (phrases, families,
//! exact length), the discourse (roles, closures, culmination, the obligation ledger), the
//! arrangement envelope (who is seated per phrase), the DeflectedLift backbone timeline
//! (gestures, cycles, slot grid), and the [`ThematicMap`] — the germ, its hook and cells as
//! scale-degree contours, and the theme site of every phrase the lead is seated in.
//!
//! Relative coordinates are charted against one declared [`REFERENCE_FRAME`] (Nashville-number
//! practice: degrees relative to a major-scale reference). A room re-modes them; it never
//! chooses them.
//!
//! **Performance freedom** (the fiber, NOT here): register, voicing, chord colour and extensions,
//! articulation, dynamics, timbre, pan, swing, microtiming, passing and approach tones, fills,
//! discretionary calls and answers, who answers, response latency, density, ornamentation.

use super::contract::{CoherenceContract, CompositionGrammar};
use super::discourse::DiscourseRole;
use super::motif::{Handoff, Motif, MotifBank, ThematicTrajectory};
use super::plan::CompositionPlan;
use super::semantic::SemanticTrace;
use super::theory::Mode;
use super::timeline::IntentTimeline;

/// The reference frame every song is charted in. Scale-degree coordinates (the germ's contour,
/// the chart's roots) are chosen against it once; each room realizes them in its own mode.
pub const REFERENCE_FRAME: Mode = Mode::Ionian;

/// Where the song states thematic material: one site per phrase the arrangement envelope seats
/// the lead in, carrying exactly what is stated there.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSite {
    /// The phrase index.
    pub phrase: u32,
    /// The phrase's discourse role (what the statement is FOR).
    pub role: DiscourseRole,
    /// The material stated, developed from the previous site (scale degrees + rhythm).
    pub motif: Motif,
    /// How it connects to the previous site.
    pub handoff: Handoff,
}

impl ThemeSite {
    /// Whether the site states the song's IDENTITY — the thesis coming home (Establish, Restate,
    /// Return) or its hook (Culminate). A performance may develop other sites (a Fragment verb);
    /// these it must state as written.
    pub fn is_identity(&self) -> bool {
        matches!(
            self.role,
            DiscourseRole::Establish
                | DiscourseRole::Restate
                | DiscourseRole::Return
                | DiscourseRole::Culminate
        )
    }
}

/// The song's thematic identity: the motif bank (germ, hook, cells — every member a scale-degree
/// contour, no room's pitch in it) and the theme site of every lead-seated phrase, developed along
/// the discourse by the [`ThematicTrajectory`] exactly once, before any performance.
#[derive(Debug, Clone, PartialEq)]
pub struct ThematicMap {
    pub bank: MotifBank,
    pub sites: Vec<ThemeSite>,
}

impl ThematicMap {
    fn build(plan: &CompositionPlan, frame: Mode, seed: u64) -> ThematicMap {
        let bank = MotifBank::generate(frame, seed ^ 0x3E10_D1E5);
        let mut traj = ThematicTrajectory::new(&bank);
        let sites = plan
            .targets()
            .into_iter()
            .filter(|t| plan.arrangement.at(t.phrase.ix as usize).lead.is_audible())
            .map(|t| {
                let (motif, handoff) = traj.next_for(t.goal.role);
                ThemeSite {
                    phrase: t.phrase.ix,
                    role: t.goal.role,
                    motif,
                    handoff,
                }
            })
            .collect();
        ThematicMap { bank, sites }
    }

    /// The theme site of `phrase`, if the song states material there.
    pub fn site(&self, phrase: u32) -> Option<&ThemeSite> {
        self.sites.iter().find(|s| s.phrase == phrase)
    }
}

/// The song: every coordinate a performance must preserve, built before any world or language is
/// known.
#[derive(Debug, Clone)]
pub struct SongMap {
    /// The semantic trace the song was composed from (its events also pitch the reaction SFX).
    pub trace: SemanticTrace,
    /// The causal intent timeline walked from the trace.
    pub timeline: IntentTimeline,
    /// Contract, form, discourse, arrangement envelope and (DeflectedLift) the backbone timeline.
    pub plan: CompositionPlan,
    /// The composition seed. Every song-level choice draws from it; performances reuse it for
    /// their own (fiber) randomness.
    pub seed: u64,
    /// The reference frame the song's relative coordinates are charted in ([`REFERENCE_FRAME`]).
    pub frame: Mode,
    /// The germ and where and what the song states of it.
    pub thematic: ThematicMap,
}

impl SongMap {
    /// Compose the song for `trace`, deterministic in `seed`. `grammar` forces a grammar (the
    /// calibration path); `None` infers one from the trace's shape.
    pub fn build(trace: &SemanticTrace, seed: u64, grammar: Option<CompositionGrammar>) -> SongMap {
        // The piece is exactly as long as the request: a partial final bar is represented, not
        // rounded away (9 beats used to render 8, 10.5 → 12, 17 → 16).
        let timeline = IntentTimeline::walk(trace);
        let plan = match grammar {
            Some(g) => CompositionPlan::build_with_contract_for_beats(
                &timeline,
                trace.total_beats,
                CoherenceContract::for_grammar(g),
            ),
            None => CompositionPlan::build_for_beats(&timeline, trace.total_beats),
        };
        let frame = REFERENCE_FRAME;
        let thematic = ThematicMap::build(&plan, frame, seed);
        SongMap {
            trace: trace.clone(),
            timeline,
            plan,
            seed,
            frame,
            thematic,
        }
    }
}
