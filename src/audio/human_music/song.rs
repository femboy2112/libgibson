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
//! (gestures, cycles, slot grid), the [`ThematicMap`] — the germ, its hook and cells as
//! contours with declared pitch units, and the theme site of every phrase the lead is seated in — and the
//! [`HarmonicMap`]: the DeflectedLift journey (lift, pointer, expected arrival, the deflection
//! that misses it, the open, home) as relational chart roots.
//!
//! Relative coordinates are charted against one declared [`REFERENCE_FRAME`] (Nashville-number
//! practice: degrees relative to a major-scale reference). A room re-modes them; it never
//! chooses them.
//!
//! **Performance freedom** (the fiber, NOT here): register, voicing, chord colour and extensions,
//! articulation, dynamics, timbre, pan, swing, microtiming, passing and approach tones, fills,
//! discretionary calls and answers, who answers, response latency, density, ornamentation.

use super::backbone::ChartCell;
use super::composer::{compose_meaning, Composer, CompositionalPrior};
use super::contract::{CoherenceContract, CompositionGrammar};
use super::discourse::DiscourseRole;
use super::form::BEATS_PER_BAR;
use super::meaning::MeaningPlan;
use super::motif::{Handoff, Motif, MotifBank, ThematicTrajectory};
use super::phenomenal::{PhenomenalRegime, PhenomenalTarget};
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
    /// The material stated, developed from the previous site (declared pitch units + rhythm).
    pub motif: Motif,
    /// How it connects to the previous site.
    pub handoff: Handoff,
}

impl ThemeSite {
    /// Whether the site states the song's IDENTITY — the thesis coming home (Establish, Restate,
    /// Return), its hook (Culminate), or — Round X — the thesis restated wherever the song teaches
    /// it (a [`Handoff::Restatement`] on any role: the first statement a listener hears, before it
    /// can be developed) and the thesis's consequent ([`Handoff::Consequent`]: its landing is the
    /// answer the listener is promised). A performance may develop other sites (a Fragment verb);
    /// these it must state as written.
    /// Whether a performance can keep this site at all: its phrase is long enough to state the
    /// motif (the statement planner's own fit rule). A site that cannot be kept is never planned —
    /// a finite form does not promise an identity statement it has no room to make.
    pub fn statable(&self, plan: &CompositionPlan) -> bool {
        plan.form
            .phrases
            .get(self.phrase as usize)
            .is_some_and(|p| {
                f64::from(self.motif.total_beats()) <= p.end_beat() - p.start_beat() + 1e-6
            })
    }

    pub fn is_identity(&self) -> bool {
        matches!(
            self.role,
            DiscourseRole::Establish
                | DiscourseRole::Restate
                | DiscourseRole::Return
                | DiscourseRole::Culminate
        ) || matches!(self.handoff, Handoff::Restatement | Handoff::Consequent)
    }
}

/// The song's thematic identity: the motif bank (germ, hook, cells — every member a declared-unit pitch
/// contour, no room's pitch in it) and the theme site of every lead-seated phrase, developed along
/// the discourse by the [`ThematicTrajectory`] exactly once, before any performance.
#[derive(Debug, Clone, PartialEq)]
pub struct ThematicMap {
    pub bank: MotifBank,
    pub sites: Vec<ThemeSite>,
}

impl ThematicMap {
    fn build(plan: &CompositionPlan, frame: Mode, seed: u64) -> ThematicMap {
        Self::from_bank(plan, MotifBank::generate(frame, seed ^ 0x3E10_D1E5))
    }

    /// The theme sites `bank`'s trajectory yields over `plan` (only sites a performance can keep).
    pub(crate) fn from_bank(plan: &CompositionPlan, bank: MotifBank) -> ThematicMap {
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
            // The trajectory develops through every seated phrase; a site is only kept where it
            // can be kept (later sites are unchanged by the drop).
            .filter(|site| site.statable(plan))
            .collect();
        ThematicMap { bank, sites }
    }

    /// The theme site of `phrase`, if the song states material there.
    pub fn site(&self, phrase: u32) -> Option<&ThemeSite> {
        self.sites.iter().find(|s| s.phrase == phrase)
    }
}

/// The chart's canonical harmonic rhythm: one chart chord every two bars inside a gesture slot.
pub const CHART_BARS_PER_CHORD: u32 = 2;

/// The song's harmonic identity (DeflectedLift): the chart every room realizes. A room re-modes and
/// colours it, and an idiom applies a declared [`super::language::HarmonicRhythm`] transform to its
/// rhythm; neither searches for its own journey or owns its own change points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarmonicMap {
    /// The journey's relational roots, charted once in the song's reference frame.
    pub cell: ChartCell,
    /// The canonical harmonic rhythm: bars per chart chord inside a gesture slot.
    pub bars_per_chord: u32,
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
    /// The composition seed. The song-level draws take it (the germ's transposition, hook and cells;
    /// the chart search's exact-tie breaks — with the frame fixed, few seeds differ in the chart);
    /// performances reuse it for their own (fiber) randomness.
    pub seed: u64,
    /// The reference frame the song's relative coordinates are charted in ([`REFERENCE_FRAME`]).
    pub frame: Mode,
    /// The germ and where and what the song states of it.
    pub thematic: ThematicMap,
    /// The chart (present when the grammar has a backbone — DeflectedLift). The phrase-engine
    /// grammars have no song-level chart yet: their harmony is still the room's (PARKED).
    pub harmonic: Option<HarmonicMap>,
    /// The listener plan the song was composed toward (Round X, [`Composer::MeaningDirected`]);
    /// `None` for the Round IX composer, which composed toward none.
    pub meaning: Option<MeaningPlan>,
    /// Optional Round XI meaning request. Kept separate to preserve the R10 compatibility hash.
    pub phenomenal: Option<PhenomenalTarget>,
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
        let mut thematic = ThematicMap::build(&plan, frame, seed);
        let stable = plan.contract.grammar == CompositionGrammar::PropulsiveReturn;
        if stable {
            // A rhythmic cell, its octave expansion and a tonic landing. Literal recognition
            // precedes any performance development. No question or miss is a prerequisite.
            let thesis = Motif {
                pitch_basis: super::theory::PitchBasis::ScaleSteps,
                id: 0,
                degrees: vec![0, 2, 4, 2, 7, 4, 2, 0],
                rhythm: vec![0.5, 0.5, 1.0, 0.5, 0.5, 1.0, 0.5, 1.5],
            };
            thematic.bank = MotifBank {
                identity: thesis.clone(),
                hook: thesis.clone(),
                rhythmic_cell: thesis.fragment(4),
                bass_cell: thesis.fragment(4).transpose(-7),
                countermotif: Some(thesis.invert()),
            };
            for site in &mut thematic.sites {
                site.motif = thesis.clone();
                site.handoff = Handoff::Restatement;
            }
        }
        let harmonic = plan.backbone.as_ref().map(|_| HarmonicMap {
            cell: if stable {
                ChartCell::propulsive_return()
            } else {
                ChartCell::chart(frame, seed)
            },
            bars_per_chord: if stable { 1 } else { CHART_BARS_PER_CHORD },
        });
        SongMap {
            trace: trace.clone(),
            timeline,
            plan,
            seed,
            frame,
            thematic,
            harmonic,
            meaning: None,
            phenomenal: stable
                .then(|| PhenomenalTarget::from_trace(trace, PhenomenalRegime::StablePropulsion)),
        }
    }

    /// Compose the song for `trace` with an explicit `composer`. [`Composer::StructuralR9`] is
    /// exactly [`SongMap::build`] (the control); [`Composer::MeaningDirected`] keeps that same form
    /// and writes the theme, its placement and the chart toward the story's
    /// [`MeaningPlan`] under [`CompositionalPrior::HOOKY_FUSION`] (experimental, not the default).
    pub fn compose(
        trace: &SemanticTrace,
        seed: u64,
        grammar: Option<CompositionGrammar>,
        composer: Composer,
    ) -> SongMap {
        if composer == Composer::StablePropulsion {
            // F selects the grammar. The legacy grammar argument applies only to R9/R10.
            let target = PhenomenalTarget::from_trace(trace, PhenomenalRegime::StablePropulsion);
            let mut song = SongMap::build(trace, seed, Some(target.grammar()));
            // This experiment holds the R10 control's orchestration envelope fixed as well as
            // the band. Only the harmonic/thematic page and its discourse are the intervention.
            // Reuse the existing planner, then schedule the new thesis in those exact seats.
            let control = CompositionPlan::build_with_contract_for_beats(
                &song.timeline,
                trace.total_beats,
                CoherenceContract::for_grammar(CompositionGrammar::DeflectedLift),
            );
            song.plan.arrangement = control.arrangement;
            song.thematic.sites = song
                .plan
                .targets()
                .into_iter()
                .filter(|t| {
                    song.plan
                        .arrangement
                        .at(t.phrase.ix as usize)
                        .lead
                        .is_audible()
                })
                .map(|t| ThemeSite {
                    phrase: t.phrase.ix,
                    role: t.goal.role,
                    motif: song.thematic.bank.identity.clone(),
                    handoff: Handoff::Restatement,
                })
                .filter(|site| site.statable(&song.plan))
                .collect();
            return song;
        }
        let song = SongMap::build(trace, seed, grammar);
        match composer {
            Composer::StructuralR9 => song,
            Composer::MeaningDirected => compose_meaning(song, &CompositionalPrior::HOOKY_FUSION).0,
            Composer::StablePropulsion => unreachable!("handled before the legacy composers"),
        }
    }

    /// Which composer chose this song's content.
    pub fn composer(&self) -> Composer {
        if self.phenomenal.is_some() {
            Composer::StablePropulsion
        } else if self.meaning.is_some() {
            Composer::MeaningDirected
        } else {
            Composer::StructuralR9
        }
    }
}

/// Legacy FNV-1a over text. The hash arithmetic is stable; callers that format `Debug`
/// data do NOT have a canonical cross-toolchain encoding. New identities use
/// [`super::fingerprint::CanonicalFingerprint`].
pub fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

impl ThematicMap {
    /// The thematic identity's fingerprint: the bank (germ, hook, cells) and every theme site.
    pub fn fingerprint(&self) -> u64 {
        fnv1a(&format!("{:?}|{:?}", self.bank, self.sites))
    }
}

impl HarmonicMap {
    /// The chart's fingerprint: its relational roots and canonical harmonic rhythm.
    pub fn fingerprint(&self) -> u64 {
        fnv1a(&format!("{self:?}"))
    }
}

impl SongMap {
    /// The song's fingerprint, over SONG-DEFINING data only: the reference frame, the causal intent
    /// timeline (every performance lifts its actions — and so its key excursions — from it), the
    /// contract, the form (phrases, families, exact length), the discourse (thesis, goals,
    /// culmination, answer, obligation ledger), the arrangement envelope (who is seated per phrase
    /// — roles, not gains), the whole backbone (slot grid, bindings, clocks) and the thematic and
    /// harmonic maps. Not the raw trace or seed (their song-level consequences are all hashed; what
    /// else the seed drives is the performance's own randomness), and never a world, a language, a
    /// patch, a mix, a voicing, or anything a performance decides. An implementation receipt, not a
    /// proof of sameness: the listen decides whether two performances are one song.
    pub fn fingerprint(&self) -> u64 {
        let p = &self.plan;
        let mut text = format!(
            "frame={:?}|timeline={:?}|contract={:?}|form={:?}|discourse={:?}|arrangement={:?}|backbone={:?}|thematic={:#x}|harmonic={:?}",
            self.frame,
            self.timeline,
            p.contract,
            p.form,
            p.discourse,
            p.arrangement,
            p.backbone,
            self.thematic.fingerprint(),
            self.harmonic.map(|h| h.fingerprint()),
        );
        // Round X: the listener plan a meaning-directed song was composed toward is song identity
        // too. Appended only when present, so a Round IX song hashes exactly as it always did.
        if let Some(m) = &self.meaning {
            text.push_str(&format!("|meaning={m:?}"));
        }
        if let Some(p) = &self.phenomenal {
            text.push_str(&format!("|phenomenal={p:?}"));
        }
        fnv1a(&text)
    }

    /// The chart's landmarks: every backbone slot's entry anchor on its downbeat, and the pointer
    /// as the harmony closing every Lift — `(beat, landmark, chart root)`. Empty without a chart.
    pub fn landmarks(&self) -> Vec<(f64, &'static str, super::backbone::ChartRoot)> {
        use super::backbone::HarmonicGesture as G;
        let (Some(tl), Some(hm)) = (&self.plan.backbone, &self.harmonic) else {
            return Vec::new();
        };
        let c = hm.cell;
        let mut v = Vec::new();
        for sl in &tl.slots {
            if sl.start_beat() >= self.plan.form.total_beats - 1e-9 {
                continue;
            }
            let root = match sl.gesture {
                G::Lift => c.lift,
                G::Deflect => c.deflect,
                G::Open => c.open,
                G::Reset => c.reset,
            };
            v.push((sl.start_beat(), sl.gesture.label(), root));
            if sl.gesture == G::Lift {
                let end = sl.end_beat().min(self.plan.form.total_beats);
                v.push((end - 1e-3, "pointer", c.pointer));
            }
        }
        v
    }
}

/// An identity site a performance did not state as the song states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeMiss {
    pub phrase: u32,
    pub why: &'static str,
}

/// A chart landmark that sounded another root.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LandmarkMiss {
    pub beat: f64,
    pub landmark: &'static str,
    pub expected_root_pc: i32,
    pub heard_root_pc: Option<i32>,
}

/// A chord change neither the chart nor the idiom's declared rhythm transform licenses, and no
/// recorded harmonic action explains.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformMiss {
    pub beat: f64,
    pub root_pc: i32,
    pub why: &'static str,
}

/// What a declared coherence anchor became in one finite performance. The contract states what a
/// listener is meant to recognize; this separates a form that had no room to state an anchor from
/// a performance that had room and omitted it. Exact, per anchor — never a score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorPresence {
    /// The performance states the anchor.
    Realized,
    /// This finite form offers the anchor no place to be stated (why); the song never promised it.
    StructurallyInapplicable(&'static str),
    /// The form had room and the contract declared it, but the performance never stated it: a
    /// composition-generation violation, not an observation gap.
    DeclaredButMissing(&'static str),
}

/// Every anchor the song's contract declares, with what the performance made of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorReport {
    pub anchors: Vec<(super::contract::CoherenceAnchor, AnchorPresence)>,
}

impl AnchorReport {
    /// Read the contract's declared anchors against the song's finite form and the realized score.
    pub fn check(
        song: &SongMap,
        perf: &super::performance::PerformancePlan,
        score: &super::score::Score,
    ) -> AnchorReport {
        use super::contract::CoherenceAnchor as A;
        use super::score::{DrumVoice, Role};
        use AnchorPresence::*;
        let full_bar = song
            .plan
            .form
            .phrases
            .iter()
            .any(|p| p.end_beat() - p.start_beat() >= BEATS_PER_BAR - 1e-9);
        let sounds = |role: Role| score.notes.iter().any(|n| n.role == role);
        let presence = |anchor: A| -> AnchorPresence {
            match anchor {
                A::Groove => {
                    let strokes = score
                        .drums
                        .iter()
                        .any(|d| matches!(d.voice, DrumVoice::Kick | DrumVoice::Snare));
                    if strokes {
                        Realized
                    } else if !full_bar {
                        StructurallyInapplicable("no phrase holds a full bar")
                    } else {
                        DeclaredButMissing("no kick or snare stroke sounds")
                    }
                }
                A::Motif => {
                    let stated = perf.statements.iter().any(|st| {
                        song.thematic.sites.iter().any(|s| s.phrase == st.phrase)
                            && score.notes.iter().any(|n| {
                                n.role == Role::Lead && n.prov.material == Some(st.material)
                            })
                    });
                    if stated {
                        Realized
                    } else if song.thematic.sites.is_empty() {
                        StructurallyInapplicable("no seated phrase can hold a motif statement")
                    } else {
                        DeclaredButMissing("no planned theme site is stated")
                    }
                }
                A::Riff => {
                    if sounds(Role::Lead) || sounds(Role::Bass) {
                        Realized
                    } else {
                        DeclaredButMissing("neither lead nor bass sounds")
                    }
                }
                A::BassFigure => {
                    if sounds(Role::Bass) {
                        Realized
                    } else {
                        DeclaredButMissing("the bass never sounds")
                    }
                }
                A::HarmonicContour | A::HarmonicLoop => {
                    if score.chords.is_empty() {
                        DeclaredButMissing("no chord span")
                    } else {
                        Realized
                    }
                }
                A::Form | A::Orchestration => Realized,
            }
        };
        AnchorReport {
            anchors: song
                .plan
                .contract
                .anchors
                .iter()
                .map(|&a| (a, presence(a)))
                .collect(),
        }
    }

    /// The anchors this performance actually establishes.
    pub fn realized(&self) -> impl Iterator<Item = super::contract::CoherenceAnchor> + '_ {
        self.anchors
            .iter()
            .filter(|(_, p)| *p == AnchorPresence::Realized)
            .map(|(a, _)| *a)
    }

    /// Declared anchors the form had room for and the performance omitted.
    pub fn violations(&self) -> Vec<(super::contract::CoherenceAnchor, &'static str)> {
        self.anchors
            .iter()
            .filter_map(|&(a, p)| match p {
                AnchorPresence::DeclaredButMissing(why) => Some((a, why)),
                _ => None,
            })
            .collect()
    }
}

/// **π, checked.** Whether a performance (its plan and its realized score) preserves the song's
/// coordinates — exact structural checks, no weighted total, no similarity score. Every list names
/// its failures; the `*_checked` counts make a pass over nothing visible.
///
/// What it reads: the performance's plan (bank, statements, edits, declared regions, obligations)
/// and the score's chord spans — the harmony every pitched realizer is justified against
/// (`RealizationDiagnostics::unjustified_by_role` is that separate receipt). It does not audit
/// realized pitch content. Some fields hold by construction for a [`super::functor::perform`]
/// output — the bank, the form, the performance's song claim, a site "stated otherwise" — and
/// guard hand-built or mutated performances; the landmarks, the chord changes and the obligations
/// are where engine output can and did fail.
#[derive(Debug, Clone, PartialEq)]
pub struct SongMapConformance {
    /// The song's fingerprint, and the one the performance says it performs.
    pub song: u64,
    pub performed: u64,
    /// The performance's motif bank is not the song's.
    pub bank_mismatch: bool,
    /// Identity sites (the thesis coming home, the hook) not stated as written, or with no lead
    /// note stamped with the statement's material.
    pub missing_theme_sites: Vec<ThemeMiss>,
    /// Chart landmarks whose chord span has another root (read in the region the performance
    /// declares in force there).
    pub wrong_harmonic_landmarks: Vec<LandmarkMiss>,
    /// Chord changes outside the chart's vocabulary for their gesture, or off the declared rhythm
    /// transform's grid, that no recorded harmonic action (a typed, logged edit) explains.
    pub illegal_harmonic_transforms: Vec<TransformMiss>,
    /// Song obligations settled in the performance with no witnessing action.
    pub unwitnessed_song_obligations: usize,
    /// The score's length or sections disagree with the song's form.
    pub form_mismatch: Vec<String>,
    pub sites_checked: usize,
    pub landmarks_checked: usize,
    pub changes_checked: usize,
}

impl SongMapConformance {
    /// Check `perf` and its realized `score` against `song`.
    pub fn check(
        song: &SongMap,
        perf: &super::performance::PerformancePlan,
        score: &super::score::Score,
    ) -> SongMapConformance {
        use super::backbone::{ChartRoot, HarmonicGesture as G};
        let heard_at = |beat: f64| {
            score
                .chords
                .iter()
                .find(|sp| {
                    sp.start_beat <= beat + 1e-6 && beat < sp.start_beat + sp.dur_beats as f64
                })
                .map(|sp| sp.chord.root_pc)
        };

        // Theme: the bank, and every identity site stated as written and heard.
        let mut missing_theme_sites = Vec::new();
        let identity: Vec<&ThemeSite> = song
            .thematic
            .sites
            .iter()
            .filter(|s| s.is_identity())
            .collect();
        for site in &identity {
            let Some(st) = perf.statements.iter().find(|st| st.phrase == site.phrase) else {
                missing_theme_sites.push(ThemeMiss {
                    phrase: site.phrase,
                    why: "not stated",
                });
                continue;
            };
            if st.motif != site.motif {
                missing_theme_sites.push(ThemeMiss {
                    phrase: site.phrase,
                    why: "stated otherwise",
                });
            } else if !score
                .notes
                .iter()
                .any(|n| n.role == super::score::Role::Lead && n.prov.material == Some(st.material))
            {
                missing_theme_sites.push(ThemeMiss {
                    phrase: site.phrase,
                    why: "not heard",
                });
            }
        }

        // Chart landmarks, heard.
        let marks = song.landmarks();
        let wrong_harmonic_landmarks: Vec<LandmarkMiss> = marks
            .iter()
            .filter_map(|&(beat, landmark, root)| {
                let expected_root_pc = root.root_pc(&perf.region_at(beat));
                let heard_root_pc = heard_at(beat);
                (heard_root_pc != Some(expected_root_pc)).then_some(LandmarkMiss {
                    beat,
                    landmark,
                    expected_root_pc,
                    heard_root_pc,
                })
            })
            .collect();

        // Every chord change inside the chart: in the gesture's vocabulary and on the declared
        // rhythm transform's grid — or explained by a recorded harmonic action.
        let mut illegal_harmonic_transforms = Vec::new();
        let mut changes_checked = 0;
        if let (Some(tl), Some(hm)) = (&song.plan.backbone, &song.harmonic) {
            let c = hm.cell;
            let bpc = perf
                .language
                .harmonic_rhythm
                .bars_per_chord(hm.bars_per_chord);
            let edited_at = |at: f64| perf.edits.iter().any(|e| (e.at_beat - at).abs() < 1e-6);
            for (i, sp) in score.chords.iter().enumerate() {
                let Some(sl) = tl.slot_at_beat(sp.start_beat) else {
                    continue;
                };
                changes_checked += 1;
                let at = sp.start_beat;
                let root = sp.chord.root_pc;
                let edited = perf
                    .edits
                    .iter()
                    .any(|e| (e.at_beat - at).abs() < 1e-6 && e.after.root_pc == root);
                if edited {
                    continue;
                }
                // The remainder of a span a recorded action split (a modulation's tonic head, an
                // applied dominant's half) starts where that action's edit ends: the action
                // explains the change point; the root is still held to the chart's vocabulary.
                let split = i.checked_sub(1).map(|j| &score.chords[j]).is_some_and(|p| {
                    edited_at(p.start_beat) && (p.start_beat + p.dur_beats as f64 - at).abs() < 1e-6
                });
                let region = perf.region_at(at);
                let vocab: Vec<ChartRoot> = match sl.gesture {
                    G::Lift => vec![c.lift, c.lift_alt, c.pointer],
                    G::Deflect => vec![c.deflect, c.satellites[0]],
                    G::Open => vec![c.open, c.satellites[1]],
                    G::Reset => vec![c.reset, c.satellites[2]],
                };
                // A Lift may also climb through the pointer's own applied dominant (V/V).
                let lawful_in = |region: &super::theory::Scale| {
                    let applied = (c.pointer.root_pc(region) + 7).rem_euclid(12);
                    vocab.iter().any(|r| r.root_pc(region) == root)
                        || (sl.gesture == G::Lift && root == applied)
                };
                // A modulation's return pivot may be the span's last chord itself, kept because it
                // is diatonic to both regions — and the planner starts the Return span ON it. It is
                // the chart's chord as the region it LEAVES transposed it, so it is read there too
                // (Round X: a diatonic satellite made this path reachable; the gap is the Round IX
                // checker's). Only there: every other region boundary is read in its own region.
                let return_pivot = perf.regions.span_at(at).is_some_and(|sp| {
                    matches!(sp.kind, super::region::RegionKind::Return { .. })
                        && (sp.start_beat - at).abs() < 1e-6
                });
                let left = perf.region_at(at - 1e-3);
                let lawful_root = lawful_in(&region) || (return_pivot && lawful_in(&left));
                if !lawful_root {
                    illegal_harmonic_transforms.push(TransformMiss {
                        beat: at,
                        root_pc: root,
                        why: "root outside the chart's vocabulary for this gesture",
                    });
                    continue;
                }
                let total = sl.bars as f64 * super::form::BEATS_PER_BAR;
                let hr = bpc as f64 * super::form::BEATS_PER_BAR;
                let n = ((total / hr).floor() as usize).max(1);
                let unit = total / n as f64;
                let off = at - sl.start_beat();
                let on_grid = split
                    || (0..n).any(|k| (off - k as f64 * unit).abs() < 1e-6)
                    || (sl.gesture == G::Lift && n == 1 && (off - total / 2.0).abs() < 1e-6);
                if !on_grid {
                    illegal_harmonic_transforms.push(TransformMiss {
                        beat: at,
                        root_pc: root,
                        why: "change off the declared rhythm transform's grid",
                    });
                }
            }
        }

        // The form, as realized.
        let mut form_mismatch = Vec::new();
        let form = &song.plan.form;
        if (score.total_beats - form.total_beats).abs() > 1e-9 {
            form_mismatch.push(format!(
                "length {} != {}",
                score.total_beats, form.total_beats
            ));
        }
        let sections: Vec<(u32, u32)> = score
            .sections
            .iter()
            .map(|s| (s.start_bar, s.bars))
            .collect();
        let phrases: Vec<(u32, u32)> = form.phrases.iter().map(|p| (p.start_bar, p.bars)).collect();
        if sections != phrases {
            form_mismatch.push(format!("sections {sections:?} != phrases {phrases:?}"));
        }

        SongMapConformance {
            song: song.fingerprint(),
            performed: perf.song_fingerprint,
            bank_mismatch: perf.bank != song.thematic.bank,
            missing_theme_sites,
            wrong_harmonic_landmarks,
            illegal_harmonic_transforms,
            unwitnessed_song_obligations: perf.obligations.unwitnessed_settlements().count(),
            form_mismatch,
            sites_checked: identity.len(),
            landmarks_checked: marks.len(),
            changes_checked,
        }
    }

    /// Whether every coordinate was preserved.
    pub fn passes(&self) -> bool {
        self.song == self.performed
            && !self.bank_mismatch
            && self.missing_theme_sites.is_empty()
            && self.wrong_harmonic_landmarks.is_empty()
            && self.illegal_harmonic_transforms.is_empty()
            && self.unwitnessed_song_obligations == 0
            && self.form_mismatch.is_empty()
    }

    /// A human-readable receipt: PASS/FAIL and every exact reason.
    pub fn report(&self) -> String {
        let mut s = format!(
            "SongMapConformance: {} (song {:#018x}, performed {:#018x}; {} identity sites, {} landmarks, {} chord changes checked)",
            if self.passes() { "PASS" } else { "FAIL" },
            self.song,
            self.performed,
            self.sites_checked,
            self.landmarks_checked,
            self.changes_checked
        );
        if self.song != self.performed {
            s.push_str("\n  performs another song");
        }
        if self.bank_mismatch {
            s.push_str("\n  bank_mismatch: the motif bank is not the song's");
        }
        for m in &self.missing_theme_sites {
            s.push_str(&format!(
                "\n  missing_theme_site: phrase {} ({})",
                m.phrase, m.why
            ));
        }
        for m in &self.wrong_harmonic_landmarks {
            s.push_str(&format!(
                "\n  wrong_harmonic_landmark: {} at {:.2}: expected root pc {}, heard {:?}",
                m.landmark, m.beat, m.expected_root_pc, m.heard_root_pc
            ));
        }
        for m in &self.illegal_harmonic_transforms {
            s.push_str(&format!(
                "\n  illegal_harmonic_transform: {:.2} root pc {} ({})",
                m.beat, m.root_pc, m.why
            ));
        }
        if self.unwitnessed_song_obligations > 0 {
            s.push_str(&format!(
                "\n  unwitnessed_song_obligations: {}",
                self.unwitnessed_song_obligations
            ));
        }
        for m in &self.form_mismatch {
            s.push_str(&format!("\n  form_mismatch: {m}"));
        }
        s
    }
}
