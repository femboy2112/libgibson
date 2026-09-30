//! The composition entry points: a semantic trace becomes a [`Score`] under a [`MusicWorld`].
//!
//! [`compose_full`] composes one [`SongMap`] ([`SongMap::build`]: the trace walked into a causal
//! [`IntentTimeline`](super::timeline::IntentTimeline) — each semantic event a sequence of intent morphisms evolving a running
//! [`MusicIntent`] — and the [`CompositionPlan`]) and [`perform`]s it: one [`PerformancePlan`], every
//! player a projection of that performance. Swapping the world or the language keeps the song (its
//! theme, its chart, its rhythm — [`super::song::SongMapConformance`] checks it); the room
//! re-modes and colours it, the idiom declares its rhythm transform and plays its own fiber.

use super::action::{ActionCause, ActionKind, Agent};
use super::contract::CompositionGrammar;
use super::form::{Section, BEATS_PER_BAR};
use super::harmonic_state::HarmonicEnsembleState;
use super::ids::ActionId;
use super::intent::{IntentMorphism, MusicIntent};
use super::performance::{EnsembleCoupling, PerformanceOptions, PerformancePlan};
use super::plan::{ArrangementRole, CompositionPlan};
use super::score::{Hearing, Note, PitchFunction, Provenance, Role, Score, SfxEvent, SfxKind};
use super::semantic::{EventKind, SemanticTrace, Tone};
use super::song::SongMap;
use super::sonority::{plan_sonority, ColorPolicy};
use super::theory::{pitch_class, Chord, Midi};
use super::world::MusicWorld;

/// Compose a full score for `trace` under `world`, deterministic in `seed`.
pub fn compose(trace: &SemanticTrace, world: &MusicWorld, seed: u64) -> Score {
    compose_with_plan(trace, world, seed).0
}

/// Everything one performance of a song produced: the realized [`Score`], the [`SongMap`] it
/// performs (the song) and the [`PerformancePlan`] (the shared performance every instrument
/// realized a projection of).
pub struct Composition {
    pub score: Score,
    pub song: SongMap,
    pub perf: PerformancePlan,
}

/// Like [`compose`], but also returns the [`CompositionPlan`] the score was realized from —
/// for structural dumps (`plan.dump()`) and coherence diagnostics.
pub fn compose_with_plan(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
) -> (Score, CompositionPlan) {
    let c = compose_full(trace, world, seed, None, PerformanceOptions::default());
    (c.score, c.song.plan)
}

/// Like [`compose_with_plan`], but the grammar is **chosen**, not inferred — the calibration path.
/// The plan is built under [`CoherenceContract::for_grammar`](super::contract::CoherenceContract::for_grammar), so the piece exercises that grammar's
/// `ResolutionPolicy` (Functional cadences vs a Loop's cyclic return vs a modal pedal), budgets and
/// anchors, whatever the trace shape would otherwise infer.
pub fn compose_with_grammar(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
    grammar: CompositionGrammar,
) -> (Score, CompositionPlan) {
    let c = compose_full(
        trace,
        world,
        seed,
        Some(grammar),
        PerformanceOptions::default(),
    );
    (c.score, c.song.plan)
}

/// The full composition path with every calibration knob: an optional forced grammar and the
/// performance options (language, actions on/off, free vs clockwork responses). Composes the
/// song ([`SongMap::build`]) and performs it once ([`perform`]).
pub fn compose_full(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
    grammar: Option<CompositionGrammar>,
    opts: PerformanceOptions,
) -> Composition {
    perform(&SongMap::build(trace, seed, grammar), world, opts)
}

/// Perform `song` in `world`'s room under `opts`: the one performance boundary. The song is read,
/// never re-derived; the performance plan is built from it and every player realizes a projection
/// of that performance.
pub fn perform(song: &SongMap, world: &MusicWorld, opts: PerformanceOptions) -> Composition {
    let perf = PerformancePlan::from_song(song, world, opts);
    let score = realize(song, world, &perf, false);
    Composition {
        score,
        song: song.clone(),
        perf,
    }
}

/// Round XII opt-in pitch-path realization of the same song and performance plan.
/// The default [`perform`] remains the exact Round XI listening control.
pub fn perform_temporal(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
) -> Composition {
    let perf = PerformancePlan::from_song(song, world, opts);
    let score = realize(song, world, &perf, true);
    Composition {
        score,
        song: song.clone(),
        perf,
    }
}

/// Round XIII opt-in: the Round XII realization with the support voicings' temporal-mass contract
/// ([`super::comp::gate_support_mass`]). Same song, same performance plan, same lead and bass;
/// [`perform_temporal`] remains the exact Round XII listening control.
pub fn perform_mass(song: &SongMap, world: &MusicWorld, opts: PerformanceOptions) -> Composition {
    let perf = PerformancePlan::from_song(song, world, opts);
    let score = realize_arm(song, world, &perf, true, Contract::Mass);
    Composition {
        score,
        song: song.clone(),
        perf,
    }
}

/// Round XIIIb opt-in: [`perform_mass`], then the sounding-tension law over the whole band
/// ([`super::tension::gate_sounding_tension`]): a note that clashes with what sounds must be
/// transient or foreshadow its resolution. Same song, same performance plan, same drums;
/// [`perform_mass`] remains the exact Round XIII listening control.
pub fn perform_tension(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
) -> Composition {
    let perf = PerformancePlan::from_song(song, world, opts);
    let score = realize_arm(song, world, &perf, true, Contract::Tension);
    Composition {
        score,
        song: song.clone(),
        perf,
    }
}

/// Round XIV opt-in: Round XII's realization, with the pad realized last among the pitched
/// players so it hears the band (lead, keys and bass, none of whom read the pad). It spaces
/// minor-2nd/9th clusters in its own voicings by octaves, and sounds the chart's root wherever the
/// heard band would otherwise flip the chord's identity ([`super::comp::realize_pad_heard`]).
/// Nothing is edited after a dependent has heard it: no post-hoc gate runs.
/// [`perform_temporal`] remains the exact Round XII listening control.
pub fn perform_coherent(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
) -> Composition {
    let perf = PerformancePlan::from_song(song, world, opts);
    let score = realize_arm(song, world, &perf, true, Contract::Coherent);
    Composition {
        score,
        song: song.clone(),
        perf,
    }
}

/// Round XV opt-in: the Round XIV harmonic solution with source-level expressive lead/bass.
/// Connective performance changes precede all downstream hearings; the pad solver is unchanged.
///
/// # Panics
/// Panics for a non-default coupling: the historical coupled/surgical experiments have different
/// source ordering or post-hoc repairs and cannot satisfy this arm's final-hearing contract.
pub fn perform_expressive(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
) -> Composition {
    assert_eq!(
        opts.coupling,
        EnsembleCoupling::Independent,
        "Round XV expression requires Independent coupling; legacy repair arms cannot run after final hearings"
    );
    let perf = PerformancePlan::from_song(song, world, opts);
    let score = realize_arm(song, world, &perf, true, Contract::Expressive);
    Composition {
        score,
        song: song.clone(),
        perf,
    }
}

/// Independent experimental factors. Historical R14/R15 entry points never read these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhraseOptions {
    pub phrase_expression: bool,
    pub semantic_occupancy: bool,
    pub support_voicing: bool,
}
impl Default for PhraseOptions {
    fn default() -> Self {
        Self {
            phrase_expression: true,
            semantic_occupancy: true,
            support_voicing: true,
        }
    }
}

/// Round XVI opt-in: phrase expression, semantic ownership, and source support voice paths.
///
/// # Panics
/// Requires Independent coupling so no historical post-hoc repair can invalidate hearings.
pub fn perform_phrased(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
) -> Composition {
    perform_phrase_experiment(song, world, opts, PhraseOptions::default())
}

/// Explicit factorial ablation of Round XVI's three source-level mechanisms.
///
/// # Panics
/// Requires Independent coupling, like [`perform_phrased`].
pub fn perform_phrase_experiment(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
    factors: PhraseOptions,
) -> Composition {
    assert_eq!(
        opts.coupling,
        EnsembleCoupling::Independent,
        "Round XVI requires Independent coupling and final-source hearings"
    );
    let perf = PerformancePlan::from_song(song, world, opts);
    let score = realize_arm(song, world, &perf, true, Contract::Phrased(factors));
    Composition {
        score,
        song: song.clone(),
        perf,
    }
}

/// Which opt-in pitch contract a realization honours on top of the written one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Contract {
    /// The realizers as they are.
    Written,
    /// Round XIII: asserted support colours pay rent.
    Mass,
    /// Round XIIIb: Mass, then sounding tension must be transient or foreshadowing.
    Tension,
    /// Round XIV: Written, with the pad hearing the band at its source. Neither Mass nor Tension.
    Coherent,
    /// Round XV: source expression within the Round XIV causal order.
    Expressive,
    /// Round XVI: optional phrase, semantic ownership and support path experiments.
    Phrased(PhraseOptions),
}

impl Contract {
    fn phrase(self) -> Option<PhraseOptions> {
        if let Self::Phrased(opts) = self {
            Some(opts)
        } else {
            None
        }
    }

    /// Whether the Round XIII support mass gate runs.
    fn mass(self) -> bool {
        matches!(self, Contract::Mass | Contract::Tension)
    }

    /// Whether the Round XIIIb post-hoc sounding-tension gate runs.
    fn tension(self) -> bool {
        self == Contract::Tension
    }
}

/// Realize a score from an explicit (possibly hand-mutated) song and performance — the entry the
/// adversarial probes use to inject a call, veto an arrangement, or license a burst and watch what
/// the players do with it.
pub fn realize_performance(song: &SongMap, world: &MusicWorld, perf: &PerformancePlan) -> Score {
    realize(song, world, perf, false)
}

/// Realize a score from a finished plan and its performance. The players are realized in
/// listening order — the lead first, then the keys (who hear the lead), the bass (who hears both)
/// and the drums (who hear the bass) — each reading the same [`PerformancePlan`]; then
/// `apply_arrangement` gates the voices.
fn realize(song: &SongMap, world: &MusicWorld, perf: &PerformancePlan, temporal: bool) -> Score {
    realize_arm(song, world, perf, temporal, Contract::Written)
}

/// [`realize`], with the Round XIII support mass gate when `mass` (only [`perform_mass`]).
fn realize_arm(
    song: &SongMap,
    world: &MusicWorld,
    perf: &PerformancePlan,
    temporal: bool,
    contract: Contract,
) -> Score {
    let (trace, seed, plan) = (&song.trace, song.seed, &song.plan);
    let total_beats = plan.form.total_beats;
    let mut score = Score::new(world.tempo_bpm, BEATS_PER_BAR, total_beats);
    score.sections = sections_from_plan(plan);
    score.chords = perf.chords.clone();

    let phrase = contract.phrase();
    let lead = if phrase.is_some_and(|p| p.phrase_expression) {
        super::melody::realize_lead_phrased(perf, plan, world)
    } else if contract == Contract::Expressive || phrase.is_some() {
        super::melody::realize_lead_expressive(perf, plan, world)
    } else if temporal {
        super::melody::realize_lead_temporal(perf, plan)
    } else {
        super::melody::realize_lead(perf, plan)
    };
    let lead_occupancy = super::occupancy::AuthoredOccupancy::from_lead(perf, &lead.authored);
    if phrase.is_some() {
        score.occupancy.push(lead_occupancy.clone());
        score.phrase_plans = lead.phrase_plans.clone();
    }
    score.expression_decisions = lead.expression;
    score.melody_repairs = lead.repairs;
    score.melody_rejudged = lead.rejudged;
    // Round XIV: the keys, the bass and the drums consume the lead as it is now.
    for listener in ["keys", "bass", "drums"] {
        score
            .hearings
            .push(Hearing::of(listener, Role::Lead, &lead.notes));
    }
    let (pad, keys, bass) = match perf.coupling {
        // The surgical arm realizes the R7b band first, note for note; it repairs afterwards.
        EnsembleCoupling::Independent | EnsembleCoupling::Surgical => {
            let keys = if phrase.is_some_and(|p| p.semantic_occupancy) {
                super::comp::realize_keys_owned(
                    perf,
                    plan,
                    world,
                    &lead.notes,
                    &lead_occupancy,
                    seed,
                )
            } else if temporal {
                super::comp::realize_keys_temporal(perf, plan, world, &lead.notes, seed)
            } else {
                super::comp::realize_keys(perf, plan, world, &lead.notes, seed)
            };
            if matches!(
                contract,
                Contract::Coherent | Contract::Expressive | Contract::Phrased(_)
            ) {
                // Round XIV keeps temporal bass; Round XV also hears final keys when expressing
                // its connectives. The frozen coherent pad comes last and hears the band.
                let bass = if phrase.is_some_and(|p| p.phrase_expression) {
                    score.hearings.push(Hearing::of("bass", Role::Keys, &keys));
                    let result = super::bass::realize_bass_phrased(
                        perf,
                        plan,
                        world,
                        &lead.notes,
                        &keys,
                        &lead.phrase_plans,
                    );
                    score
                        .occupancy
                        .push(super::occupancy::AuthoredOccupancy::from_bass(
                            perf,
                            &result.authored,
                            &score.expression_decisions,
                        ));
                    score.phrase_plans.extend(result.plans);
                    score.expression_decisions.extend(result.decisions);
                    result.notes
                } else if contract == Contract::Expressive || phrase.is_some() {
                    score.hearings.push(Hearing::of("bass", Role::Keys, &keys));
                    if phrase.is_some() {
                        let source = super::bass::realize_bass_temporal(
                            perf,
                            plan,
                            world,
                            &lead.notes,
                            &keys,
                        );
                        score
                            .occupancy
                            .push(super::occupancy::AuthoredOccupancy::from_bass(
                                perf,
                                &source,
                                &score.expression_decisions,
                            ));
                    }
                    let (notes, decisions) =
                        super::bass::realize_bass_expressive(perf, plan, world, &lead.notes, &keys);
                    score.expression_decisions.extend(decisions);
                    notes
                } else {
                    super::bass::realize_bass_temporal(perf, plan, world, &lead.notes, &keys)
                };
                let band: Vec<Note> = lead
                    .notes
                    .iter()
                    .chain(&keys)
                    .chain(&bass)
                    .copied()
                    .collect();
                for (source, notes) in [
                    (Role::Lead, &lead.notes),
                    (Role::Keys, &keys),
                    (Role::Bass, &bass),
                ] {
                    score.hearings.push(Hearing::of("pad", source, notes));
                }
                let (pad, edits) = if phrase.is_some_and(|p| p.support_voicing) {
                    let (notes, edits, decisions) =
                        super::comp::realize_pad_phrased(perf, plan, world, &band);
                    score.support_voicing_decisions = decisions;
                    (notes, edits)
                } else {
                    super::comp::realize_pad_heard(perf, plan, world, &band)
                };
                score.pad_voicing_edits = edits;
                (pad, keys, bass)
            } else {
                let mut pad = super::comp::realize_pad(perf, plan, world);
                let mut keys = keys;
                if contract.mass() {
                    super::comp::gate_support_mass(perf, world, &mut pad, &mut keys);
                }
                let bass = if temporal {
                    super::bass::realize_bass_temporal(perf, plan, world, &lead.notes, &keys)
                } else {
                    super::bass::realize_bass(perf, plan, world, &lead.notes, &keys)
                };
                (pad, keys, bass)
            }
        }
        EnsembleCoupling::CoupledR8 => {
            let r = realize_coupled(world, seed, plan, perf, &lead.notes);
            score.vertical_decisions = r.decisions;
            score.support_report = Some(r.support);
            (r.pad, r.keys, r.bass)
        }
    };
    score.hearings.push(Hearing::of("drums", Role::Bass, &bass));
    score.drums = if phrase.is_some_and(|p| p.semantic_occupancy) {
        let intent = score
            .occupancy
            .iter()
            .find(|o| o.role == Role::Bass)
            .expect("bass source occupancy");
        super::groove::realize_drums_owned(perf, plan, world, seed, &bass, &lead.notes, intent)
    } else {
        super::groove::realize_drums(perf, plan, world, seed, &bass, &lead.notes)
    };
    score.notes.extend(pad);
    score.notes.extend(keys);
    score.notes.extend(bass);
    score.notes.extend(lead.notes);
    // Round XIIIb: after the drums have heard the band as written, so the groove is unchanged.
    // Every interaction receipt the band witnesses must survive each edit.
    if contract.tension() {
        let witnessed = |s: &Score| -> Vec<bool> {
            super::witness::audit(perf, s)
                .rows
                .iter()
                .map(|r| r.witnessed)
                .collect()
        };
        let before = witnessed(&score);
        let mut notes = std::mem::take(&mut score.notes);
        let template = score.clone();
        let keeps = |band: &[Note]| -> bool {
            let mut s = template.clone();
            s.notes = band.to_vec();
            witnessed(&s)
                .iter()
                .zip(&before)
                .all(|(&now, &was)| now || !was)
        };
        score.tension_edits = super::tension::gate_sounding_tension(
            &mut notes,
            &perf.contexts,
            world,
            score.tempo_bpm,
            score.beats_per_bar,
            Some(&keeps),
        );
        score.notes = notes;
    }

    // --- SFX from significant semantic events, pitched in the local harmony. ---
    add_sfx_and_provenance(&mut score, trace, plan, perf, world);
    // --- The piece ends where it was asked to: nothing starts at or after the end, nothing rings
    //     past it. ---
    clip_to_end(&mut score);
    // --- Provenance only: the stage already decided who plays and how loud. ---
    stamp_arrangement(&mut score, plan, perf);
    // --- Round VIIIb: the finished R7b score, its real hard vertical defects repaired one note at
    //     a time (every edit on the ledger; a receipt surrendered to harmony is reported, not kept
    //     by keeping the bad note). ---
    if perf.coupling == EnsembleCoupling::Surgical {
        let policy = ColorPolicy::for_world(world.id, &perf.language);
        let witnessed = |s: &Score| -> Vec<(ActionId, bool)> {
            super::witness::audit(perf, s)
                .rows
                .iter()
                .map(|r| (r.action, r.witnessed))
                .collect()
        };
        score.vertical_repairs =
            super::surgical::repair(&mut score, &perf.contexts, world, &policy, Some(&witnessed));
    }
    debug_assert!(
        orchestration_violations(perf, &score).is_empty(),
        "a realizer played somebody the stage had out"
    );
    score
}

/// The pitched support players realized as ONE harmonic state (Round VIII).
struct Coupled {
    pad: Vec<Note>,
    keys: Vec<Note>,
    bass: Vec<Note>,
    decisions: Vec<super::harmonic_state::VerticalDecision>,
    support: super::support::JointReport,
}

/// Realize bass, keys and pad against one [`HarmonicEnsembleState`], in RIGIDITY order: the lead is
/// already fixed (the first mover); the bass states the floor against it; the keys' material lines
/// (answers, figures, unison — pitch free within their contour) are placed against lead and bass;
/// then the pad and the keys' comping are voiced JOINTLY against everything already sounding.
fn realize_coupled(
    world: &MusicWorld,
    seed: u64,
    plan: &CompositionPlan,
    perf: &PerformancePlan,
    lead: &[Note],
) -> Coupled {
    let policy = ColorPolicy::for_world(world.id, &perf.language);
    let plans = plan_sonority(&perf.contexts, lead, &policy);
    let mut state = HarmonicEnsembleState::new(&perf.contexts, plans, policy);
    state.commit(lead);
    let bass = super::bass::realize_bass_coupled(perf, plan, world, lead, &mut state);
    state.commit(&bass);
    // The keys' material lines, heard against lead and bass before they speak (answers and figures
    // re-placed by whole octaves, an answer's colliding connector stepped aside); the unison as is.
    let (lines, placed) =
        super::comp::keys_lines_coupled(perf, lead, super::comp::keys_velocity(world), &state);
    state.commit(&lines);
    for d in placed {
        state.record(d.beat, d.role, d.what, d.reason);
    }
    // The bed: the pad and the keys' comping voiced as ONE decision against everything above.
    let n = super::comp::keys_shell_n(perf);
    let sp = super::support::joint_support_paths(perf, world.voicing_spread, lead, n, &state);
    // The keys' comping and holds on the joint path, then their material lines: realized BEFORE
    // the pad, so the pad's tails hear what the keys actually play at the next change.
    let mut keys = super::comp::keys_comp(perf, world, lead, seed, &sp.keys);
    keys.extend(lines);
    let keys = super::comp::finish_keys_coupled(keys, perf);
    // A pad tail that would meet another player a minor 2nd / 9th away at the next harmony (the
    // ledger's lead, bass and keys lines, and every keys note realized there) lifts early.
    let guard = |a: f64, b: f64, p: Midi| -> bool {
        let semi = |q: Midi| matches!((q - p).abs(), 1 | 13);
        state
            .sounding(a, b)
            .any(|v| v.role != super::score::Role::Pad && semi(v.pitch))
            || keys.iter().any(|n| {
                n.start_beat < b - 1e-6
                    && n.start_beat + n.dur_beats as f64 > a + 1e-6
                    && semi(n.pitch)
            })
    };
    let pad = super::comp::realize_pad_on(perf, world, &sp.pad, Some(&guard));
    Coupled {
        pad,
        keys,
        bass,
        decisions: state.log().to_vec(),
        support: sp.report,
    }
}

/// The final end-of-piece safety pass: no note, drum stroke, SFX or chord span starts at or after
/// `score.total_beats`, and every note and chord that would ring past it is clipped to end there.
/// The planners already bound their windows by the exact total; this pass makes the invariant
/// unconditional (a keys stab on the last sixteenth, or a realizer whose horizon is still the bar
/// grid, cannot leak past the requested end). A piece that fits inside its request is untouched.
fn clip_to_end(score: &mut Score) {
    let end = score.total_beats;
    let starts_in = |beat: f64| beat < end - 1e-9;
    score.notes.retain_mut(|n| {
        if !starts_in(n.start_beat) {
            return false;
        }
        if n.start_beat + n.dur_beats as f64 > end + 1e-9 {
            n.dur_beats = (end - n.start_beat) as f32;
        }
        n.dur_beats > 0.0
    });
    score.drums.retain(|d| starts_in(d.start_beat));
    score.sfx.retain(|e| starts_in(e.start_beat));
    score.chords.retain_mut(|c| {
        if !starts_in(c.start_beat) {
            return false;
        }
        if c.start_beat + c.dur_beats as f64 > end + 1e-9 {
            c.dur_beats = (end - c.start_beat) as f32;
        }
        c.dur_beats > 0.0
    });
}

/// Every Score event whose player the stage had OFF at its onset (neither seated nor admitted by
/// an action) — `(agent, beat)`. Empty by construction: the realizers ask the stage first. Kept as
/// an assertable invariant so a regression cannot sneak a second orchestration authority back in.
pub fn orchestration_violations(perf: &PerformancePlan, score: &Score) -> Vec<(Agent, f64)> {
    let mut v: Vec<(Agent, f64)> = score
        .notes
        .iter()
        .filter_map(|n| {
            let agent = match n.role {
                super::score::Role::Lead => Agent::Lead,
                super::score::Role::Keys => Agent::Keys,
                super::score::Role::Pad => Agent::Pad,
                super::score::Role::Bass => Agent::Bass,
            };
            (!perf.on_stage(agent, n.start_beat)).then_some((agent, n.start_beat))
        })
        .collect();
    v.extend(
        score
            .drums
            .iter()
            // Micro-timing can nudge a stroke a few ms before its bar line.
            .filter(|d| !perf.on_stage(Agent::Drums, d.start_beat + 0.01))
            .map(|d| (Agent::Drums, d.start_beat)),
    );
    v
}

/// Project the legacy [`Section`] list (for the Score IR and `Score::summary`) FROM the plan.
/// There is no independent `Form::from_trace` on the musical path any more: the summary and the
/// per-event provenance are two views of the *same* plan-derived decomposition.
fn sections_from_plan(plan: &CompositionPlan) -> Vec<Section> {
    plan.form
        .phrases
        .iter()
        .map(|p| Section {
            kind: p.family.to_section_kind(),
            start_bar: p.start_bar,
            bars: p.bars,
            energy: p.span.peak_energy.energy,
            tension: p.span.peak_tension.tension,
            density: p.intent.density,
        })
        .collect()
}

/// Stamp every event's phrase / family / role / closure provenance from the plan, and its
/// arrangement role from the STAGE seat it played in. Round VII's `apply_arrangement` also
/// deleted the notes of voices a phrase role had silenced and rescaled the rest — a second
/// orchestration authority acting after the performance was realized (it silently deleted a
/// planned intro fill and a coda pullback and let the witness count the silence). The stage now
/// decides before anybody plays; this pass only writes provenance.
fn stamp_arrangement(score: &mut Score, plan: &CompositionPlan, perf: &PerformancePlan) {
    let form = &plan.form;
    // The discourse debt an event helps settle: the obligation whose settlement witness is an
    // action the event performs.
    let owed: Vec<(super::ids::ActionId, super::ids::ObligationId)> = perf
        .obligations
        .obligations
        .iter()
        .filter_map(|o| Some((o.settlement?.witness?, o.id)))
        .collect();
    let stamp = |prov: &mut Provenance, beat: f64, agent: Option<Agent>| {
        if prov.obligation.is_none() {
            prov.obligation = owed
                .iter()
                .find(|(a, _)| prov.actions.has(*a))
                .map(|&(_, o)| o);
        }
        let phrase = *form.phrase_at(beat);
        prov.section = phrase.family.to_section_kind();
        prov.phrase = Some(phrase.ix);
        prov.family = Some(phrase.family.label());
        if let Some(a) = agent {
            let seat = perf.stage.seat(a, beat);
            prov.role_kind = Some(if seat.on {
                seat.role.label()
            } else {
                ArrangementRole::Punctuation.label()
            });
        }
        let goal = plan.discourse.goal(phrase.ix as usize);
        prov.role = Some(goal.role.label());
        prov.closure = Some(goal.closure.label());
    };
    for n in &mut score.notes {
        let agent = match n.role {
            super::score::Role::Lead => Agent::Lead,
            super::score::Role::Keys => Agent::Keys,
            super::score::Role::Pad => Agent::Pad,
            super::score::Role::Bass => Agent::Bass,
        };
        stamp(&mut n.prov, n.start_beat, Some(agent));
    }
    for d in &mut score.drums {
        stamp(&mut d.prov, d.start_beat, Some(Agent::Drums));
    }
    // SFX are punctuation tied to semantic beats: provenance only.
    for e in &mut score.sfx {
        stamp(&mut e.prov, e.start_beat, None);
    }
}

/// One sting per significant semantic event, placed on the event's MUSICAL beat and pitched in the
/// harmony sounding there.
///
/// The beat is the event's quantized beat ([`super::timeline::quantize_event_beat`], the same map
/// the intent timeline — and so every action — uses); the raw semantic beat used to flam the sting
/// a few tens of milliseconds against the ensemble hit the same event produced.
fn add_sfx_and_provenance(
    score: &mut Score,
    trace: &SemanticTrace,
    plan: &CompositionPlan,
    perf: &PerformancePlan,
    world: &MusicWorld,
) {
    let mut prev = trace.events.first().map(|e| e.state);
    // `ti` is the event's intent-timeline transition: the walk emits exactly one per event.
    for (ti, ev) in trace.events.iter().enumerate() {
        let significant = ev.kind.requires_event()
            && prev
                .map(|p| p.is_significant_change(&ev.state))
                .unwrap_or(true);
        prev = Some(ev.state);
        if !significant {
            continue;
        }
        let kind = match (ev.kind, ev.state.tone) {
            (EventKind::Impact, _) | (_, Tone::Danger) => SfxKind::Impact,
            (EventKind::Confirmation, _) | (_, Tone::Success) => SfxKind::Confirm,
            (EventKind::ModalEntered, _) | (_, Tone::Warning) => SfxKind::Warning,
            (EventKind::FocusAcquired, _) => SfxKind::Acquire,
            (EventKind::ActChanged, _) | (EventKind::SectionResolved, _) => SfxKind::Transition,
            _ => SfxKind::Acquire,
        };
        let at = super::timeline::quantize_event_beat(ev.at_beat);
        if at >= score.total_beats - 1e-9 {
            continue; // the event lands on the end of the piece: no time left to sound it
        }
        let sec_kind = plan.form.phrase_at(at).family.to_section_kind();
        let mut v = voice_sfx(kind, at, ti, score.tempo_bpm, plan, perf);
        // Round VIII: the sting joins the band's one harmony — under the coupled realization it keeps
        // its shape and pitch classes (the chord verdict and an owned tritone are untouched) and moves
        // by whole octaves to the placement with the fewest unowned clashes against what sounds.
        if perf.coupling == EnsembleCoupling::CoupledR8 && v.pitches != SfxEvent::UNPITCHED {
            if let Some((shift, reason)) = sfx_octave(score, perf, world, kind, at, &v) {
                v.pitches = v.pitches.map(|p| p + shift);
                score
                    .vertical_decisions
                    .push(super::harmonic_state::VerticalDecision {
                        beat: at,
                        role: super::score::Role::Lead,
                        what: "sfx-octave",
                        reason,
                    });
            }
        }
        // When the band itself accents this beat (the same event's hit or push), the sting sits
        // under the ensemble instead of stacking on top of it: the accent is the band's.
        let band_accents = perf
            .actions_starting(
                &[
                    super::action::ActionKind::Hit,
                    super::action::ActionKind::Push,
                ],
                at,
                0.125,
                None,
            )
            .next()
            .is_some();
        score.sfx.push(SfxEvent {
            start_beat: at,
            kind,
            velocity: ev.state.dynamic() * if band_accents { 0.75 } else { 1.0 },
            prov: Provenance {
                section: sec_kind,
                role_note: "sfx",
                ..Provenance::new(sec_kind)
            },
            pitches: v.pitches,
            function: v.function,
            owned_by: v.owned_by,
            dissonance_beats: v.dissonance_beats,
        });
    }
}

/// The whole-octave shift (within MIDI 24..=96) that gives an SFX gesture the fewest unowned clashes
/// against the notes sounding during its gated life, if it beats the composer's own register —
/// with the reason. Ties keep the original register.
fn sfx_octave(
    score: &Score,
    perf: &PerformancePlan,
    world: &MusicWorld,
    kind: SfxKind,
    at: f64,
    v: &SfxVoicing,
) -> Option<(Midi, String)> {
    use super::sonority::{classify_clash, sfx_voice, Clash, VerticalClass, Voice};
    let (a, d, _, _) = kind.envelope();
    let end = at + (a + d + kind.hold_secs()) as f64 * score.tempo_bpm.max(1.0) as f64 / 60.0;
    let ctx = perf.context_at(at)?;
    // What actually rings under the sting: each note to its audible end at the masking floor (a
    // pad tail from the last chord is still there).
    let world_patch = |r: super::score::Role| match r {
        super::score::Role::Pad => &world.pad,
        super::score::Role::Keys => &world.keys,
        super::score::Role::Bass => &world.bass,
        super::score::Role::Lead => &world.lead,
    };
    let sounding: Vec<Voice> = score
        .notes
        .iter()
        .map(|n| {
            let mut v = super::harmonic_state::voice_of(n);
            v.end = super::sonority::audible_end_at(
                v.start,
                n.dur_beats as f64,
                world_patch(n.role),
                score.tempo_bpm,
                super::sonority::MASKING_FLOOR_DB,
            );
            v
        })
        .filter(|v| v.start < end - 1e-6 && v.end > at + 1e-6)
        .collect();
    let clashes = |shift: Midi| -> (u32, Vec<String>) {
        let mut n = 0;
        let mut why = Vec::new();
        for (k, &p) in v.pitches.iter().enumerate() {
            let me = sfx_voice(p + shift, v.function[k], at, end, v.owned_by.is_some());
            for o in &sounding {
                if Clash::of(me.pitch, o.pitch).is_some()
                    && classify_clash(ctx, &me, o, None) == VerticalClass::UnownedCollision
                {
                    n += 1;
                    why.push(format!(
                        "{} against {} {}",
                        super::theory::note_name(p),
                        o.role.label(),
                        super::theory::note_name(o.pitch)
                    ));
                }
            }
        }
        (n, why)
    };
    let (base, why) = clashes(0);
    if base == 0 {
        return None;
    }
    let lo = v.pitches.iter().min().copied().unwrap_or(60);
    let hi = v.pitches.iter().max().copied().unwrap_or(60);
    let best = [12, -12, 24, -24, 36, -36]
        .into_iter()
        .filter(|s| lo + s >= 24 && hi + s <= 96)
        .map(|s| (clashes(s).0, s.abs(), s))
        .min()?;
    (best.0 < base).then(|| {
        (
            best.2,
            format!(
                "moved {:+} semitones: {} -> {} unowned clashes ({})",
                best.2,
                base,
                best.0,
                why.join(", ")
            ),
        )
    })
}

/// An SFX gesture's pitches and their justification.
struct SfxVoicing {
    pitches: [Midi; 2],
    function: [Option<PitchFunction>; 2],
    owned_by: Option<ActionId>,
    dissonance_beats: Option<f32>,
}

impl SfxVoicing {
    fn chord_tones(pitches: [Midi; 2]) -> SfxVoicing {
        SfxVoicing {
            pitches,
            function: [Some(PitchFunction::ChordTone); 2],
            owned_by: None,
            dissonance_beats: None,
        }
    }
}

/// The lower pitch class of a tritone the chord itself contains (Dom7 3–b7, m7b5 1–b5, dim 1–b5),
/// if it has one.
fn own_tritone(chord: &Chord) -> Option<i32> {
    let iv = chord.quality.intervals();
    iv.iter().enumerate().find_map(|(k, &a)| {
        iv[k + 1..]
            .iter()
            .any(|&b| (b - a).rem_euclid(12) == 6)
            .then_some((chord.root_pc + a).rem_euclid(12))
    })
}

/// Pitch one SFX gesture against the harmony sounding at `at`, in the register the world-scale
/// version used (so a sting whose local chord IS the tonic sounds exactly as before):
///
/// - Acquire: the chord's root high, its fifth just below.
/// - Confirm: fifth → root, rising.
/// - Transition: the guide tones (3rd, 7th/6th — the 5th for a triad) above the root.
/// - Impact: the bass pitch class, low, doubled at the octave.
/// - Warning: the chord's OWN tritone when it has one (both chord tones). Otherwise the alarm
///   tritone is kept on the chord's root as an OWNED dissonance: owned by the planned action the
///   same semantic event produced (the Hold a Suspend lifted, the Deflect bound to it) whose window
///   holds the voice's whole bounded lifetime. With no such owner the Warning sounds the guide
///   tones instead — a dissonance nobody owns is not played.
/// - Danger (not produced by the functor today): root and fifth, low.
///
/// With no harmony at all (an empty chord plan) the gesture is left unpitched: the synth's
/// world-scale fallback.
fn voice_sfx(
    kind: SfxKind,
    at: f64,
    transition: usize,
    tempo_bpm: f32,
    plan: &CompositionPlan,
    perf: &PerformancePlan,
) -> SfxVoicing {
    let Some(ctx) = perf.context_at(at) else {
        return SfxVoicing {
            pitches: SfxEvent::UNPITCHED,
            function: [None; 2],
            owned_by: None,
            dissonance_beats: None,
        };
    };
    let chord = ctx.chord;
    let root = chord.root_pc;
    let iv = chord.quality.intervals();
    let fifth = iv
        .iter()
        .find(|&&i| i == 7)
        .or_else(|| iv.iter().find(|&&i| i == 6 || i == 8))
        .map(|&i| (root + i).rem_euclid(12))
        .unwrap_or(root);
    // The lowest pitch of class `pc` at or above `floor`.
    let place = |pc: i32, floor: Midi| floor + (pc - floor).rem_euclid(12);
    let guides = |base: Midi| {
        let g = super::context::guide_tones(&chord);
        let lo = g.first().copied().unwrap_or(fifth);
        let hi = g.get(1).copied().unwrap_or(fifth);
        [place(lo, base), place(hi, base)]
    };
    match kind {
        SfxKind::Acquire => {
            let r = place(root, 84);
            SfxVoicing::chord_tones([r, place(fifth, r - 12)])
        }
        SfxKind::Confirm => {
            let r = place(root, 84);
            SfxVoicing::chord_tones([place(fifth, r - 12), r])
        }
        SfxKind::Transition => SfxVoicing::chord_tones(guides(place(root, 60))),
        SfxKind::Impact => {
            let b = if chord.contains_pc(ctx.bass_pc) {
                ctx.bass_pc
            } else {
                root
            };
            SfxVoicing::chord_tones([place(b, 24), place(b, 36)])
        }
        SfxKind::Danger => {
            let r = place(root, 36);
            SfxVoicing::chord_tones([r, place(fifth, r)])
        }
        SfxKind::Warning => {
            if let Some(lo) = own_tritone(&chord) {
                let p = place(lo, 60);
                return SfxVoicing::chord_tones([p, p + 6]);
            }
            let life = kind.max_lifetime_beats(tempo_bpm);
            match sfx_owner(perf, plan, transition, at, life) {
                Some(owner) => {
                    let p = place(root, 60);
                    SfxVoicing {
                        pitches: [p, p + 6],
                        function: [Some(PitchFunction::ChordTone), None],
                        owned_by: Some(owner),
                        dissonance_beats: Some(life),
                    }
                }
                None => SfxVoicing::chord_tones(guides(place(root, 60))),
            }
        }
    }
}

/// The planned action that owns an SFX dissonance: one the SAME semantic event produced (a
/// morphism its transition applied, or a backbone gesture bound to it) whose window holds
/// `[at, at + span]`. The Hold a Suspend lifted is preferred (the alarm is the held tension), then
/// the event's own morphisms over its gesture, then the longest window.
fn sfx_owner(
    perf: &PerformancePlan,
    plan: &CompositionPlan,
    transition: usize,
    at: f64,
    span: f32,
) -> Option<ActionId> {
    let binding = plan
        .backbone
        .as_ref()
        .and_then(|bb| bb.bindings.iter().position(|b| b.transition == transition));
    perf.actions
        .actions
        .iter()
        .filter_map(|a| {
            let tier = match a.cause {
                ActionCause::Morphism { transition: t, .. } if t == transition => 0u8,
                ActionCause::Gesture { .. } if binding.is_some() && a.binding == binding => 1,
                _ => return None,
            };
            let holds = a.start_beat <= at + 1e-6 && at + span as f64 <= a.end_beat() + 1e-6;
            holds.then_some((a.kind != ActionKind::Hold, tier, a.end_beat(), a.id))
        })
        .min_by(|x, y| {
            x.0.cmp(&y.0)
                .then(x.1.cmp(&y.1))
                .then(y.2.total_cmp(&x.2))
                .then(x.3.cmp(&y.3))
        })
        .map(|c| c.3)
}

/// How one SFX gesture stands against the harmony under it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SfxVerdict {
    /// Every pitch is a tone of the chord sounding under it.
    Chord,
    /// A dissonant pitch, owned by a planned action whose window holds the voice's whole
    /// lifetime (the payload is the owned span in beats).
    Owned(f32),
    /// An unowned or outliving dissonance, a pitch whose consonant label is false, no harmony to
    /// judge against, or an unpitched (world-scale) gesture.
    Unjustified,
}

/// The **SFX audit** (Round VIIb): is every sting in the local harmony, or honestly owned? Judged
/// from the realized Score's own chords and the performance's actions — not from the labels the
/// composer wrote, which it only cross-checks.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SfxAudit {
    pub total: usize,
    /// Gestures whose every pitch is a chord tone of its context.
    pub chord: usize,
    /// Gestures carrying an owned dissonance that fits inside its owner.
    pub owned: usize,
    /// Everything else (target 0).
    pub unjustified: usize,
    /// The longest owned dissonance, in beats.
    pub max_owned_beats: f32,
}

impl SfxAudit {
    /// Audit every SFX of `score` against its chords and `perf`'s actions.
    pub fn measure(perf: &PerformancePlan, score: &Score) -> SfxAudit {
        let mut a = SfxAudit {
            total: score.sfx.len(),
            ..SfxAudit::default()
        };
        for e in &score.sfx {
            match SfxAudit::verdict(e, perf, score) {
                SfxVerdict::Chord => a.chord += 1,
                SfxVerdict::Owned(d) => {
                    a.owned += 1;
                    a.max_owned_beats = a.max_owned_beats.max(d);
                }
                SfxVerdict::Unjustified => a.unjustified += 1,
            }
        }
        a
    }

    /// The verdict on one gesture `e` of `score`.
    pub fn verdict(e: &SfxEvent, perf: &PerformancePlan, score: &Score) -> SfxVerdict {
        if !e.is_pitched() {
            return SfxVerdict::Unjustified;
        }
        let b = e.start_beat;
        let Some(chord) = score
            .chords
            .iter()
            .filter(|c| c.start_beat <= b + 1e-6 && b < c.start_beat + c.dur_beats as f64)
            .max_by(|x, y| x.start_beat.total_cmp(&y.start_beat))
            .map(|c| c.chord)
        else {
            return SfxVerdict::Unjustified;
        };
        let mut dissonant = false;
        for (p, f) in e.pitches.iter().zip(e.function) {
            let tone = chord.contains_pc(pitch_class(*p));
            if f.is_some_and(PitchFunction::is_consonant) && !tone {
                return SfxVerdict::Unjustified; // labelled consonant, but it is not
            }
            dissonant |= !tone;
        }
        if !dissonant {
            return SfxVerdict::Chord;
        }
        // An owned dissonance: an existing action whose window holds the whole voice lifetime,
        // and a claimed span no shorter than that lifetime.
        let life = e.kind.max_lifetime_beats(score.tempo_bpm);
        match (
            e.owned_by.and_then(|id| perf.actions.get(id)),
            e.dissonance_beats,
        ) {
            (Some(owner), Some(d))
                if d + 1e-4 >= life
                    && d as f64 <= owner.dur_beats + 1e-6
                    && owner.start_beat <= b + 1e-6
                    && b + d as f64 <= owner.end_beat() + 1e-6 =>
            {
                SfxVerdict::Owned(d)
            }
            _ => SfxVerdict::Unjustified,
        }
    }

    /// A one-line receipt.
    pub fn report(&self) -> String {
        format!(
            "sfx: total={} chord={} owned={} unjustified={} max_owned_beats={:.2}\n",
            self.total, self.chord, self.owned, self.unjustified, self.max_owned_beats
        )
    }
}

/// The intent-morphism gesture a semantic event maps to (used for provenance + testing the
/// functor's action on morphisms; the concrete note choices above are its realization).
pub fn event_to_morphisms(kind: EventKind, state_tone: Tone) -> Vec<IntentMorphism> {
    use IntentMorphism::*;
    match kind {
        EventKind::Prolong => vec![Prolong],
        EventKind::FocusAcquired => vec![Intensify, FragmentMotif],
        EventKind::ToneShift => vec![Reharmonize],
        EventKind::ModalEntered => vec![Suspend, ThickenTexture],
        EventKind::Impact => vec![Intensify, Syncopate, Modulate],
        EventKind::Confirmation => vec![Resolve, Cadence],
        EventKind::SectionResolved => vec![Relax, Cadence],
        EventKind::ActChanged => match state_tone {
            Tone::Danger => vec![Intensify, Modulate],
            _ => vec![Prepare],
        },
    }
}

/// The endpoint intent of the causal walk — delegates to [`super::timeline::IntentTimeline`],
/// the single source of truth for intent evolution. Kept for the category-law tests and any
/// caller that only wants the final settled intent rather than the whole timeline.
pub fn walk_intent(trace: &SemanticTrace) -> MusicIntent {
    super::timeline::IntentTimeline::walk(trace).final_intent
}

#[cfg(test)]
mod tests {
    use super::super::score::{PitchFunction, Role};
    use super::super::semantic::demo_trace;
    use super::super::theory::pitch_class;
    use super::super::timeline::IntentTimeline;
    use super::*;

    /// The duration probes: the bounce under its own grammar and forced onto HookArc (no
    /// backbone), the cinematic demo under the grammar it infers and forced onto DeflectedLift.
    fn duration_probes(
        beats: f64,
    ) -> Vec<(&'static str, SemanticTrace, Option<CompositionGrammar>)> {
        use super::super::semantic::deflected_lift_trace;
        vec![
            (
                "bounce/deflected",
                deflected_lift_trace(beats),
                Some(CompositionGrammar::DeflectedLift),
            ),
            ("demo/inferred", demo_trace(beats), None),
            (
                "demo/deflected",
                demo_trace(beats),
                Some(CompositionGrammar::DeflectedLift),
            ),
            (
                "bounce/hookarc",
                deflected_lift_trace(beats),
                Some(CompositionGrammar::HookArc),
            ),
        ]
    }

    #[test]
    fn a_request_of_any_length_renders_exactly_that_length() {
        use super::super::diagnostics::ActionDiagnostics;
        use super::super::synth::HumanMusicSynth;
        use crate::audio::SampleRate;
        // compose_full used to round `total_beats / 4` to whole bars: 9 beats rendered 8, 10.5
        // rendered 12, 17 rendered 16. The partial final bar is now represented exactly.
        for beats in [9.0, 10.5, 17.0, 120.0] {
            for (name, trace, grammar) in duration_probes(beats) {
                for world in MusicWorld::all() {
                    let tag = format!("{name} {beats} {}", world.name);
                    let c =
                        compose_full(&trace, &world, 2112, grammar, PerformanceOptions::default());
                    let s = &c.score;
                    assert_eq!(s.total_beats, beats, "{tag}: score length");
                    assert_eq!(c.song.plan.form.total_beats, beats, "{tag}: form length");
                    assert_eq!(c.perf.total_beats, beats, "{tag}: performance length");
                    s.validate().unwrap_or_else(|e| panic!("{tag}: {e}"));
                    for n in &s.notes {
                        assert!(
                            n.start_beat < beats,
                            "{tag}: note starts at {}",
                            n.start_beat
                        );
                        assert!(
                            n.start_beat + n.dur_beats as f64 <= beats + 1e-6,
                            "{tag}: note {}+{} rings past the end",
                            n.start_beat,
                            n.dur_beats
                        );
                    }
                    for d in &s.drums {
                        assert!(d.start_beat < beats, "{tag}: drum at {}", d.start_beat);
                    }
                    for e in &s.sfx {
                        assert!(e.start_beat < beats, "{tag}: sfx at {}", e.start_beat);
                    }
                    for ch in &s.chords {
                        assert!(ch.start_beat + ch.dur_beats as f64 <= beats + 1e-6, "{tag}");
                    }
                    // The synth renders the requested length plus its fixed release tail.
                    let synth = HumanMusicSynth::new(s, &world, SampleRate::STUDIO);
                    let spb = SampleRate::STUDIO.as_f64() * 60.0 / s.tempo_bpm as f64;
                    let tail = (SampleRate::STUDIO.as_f64() as f32 * 2.5) as u64;
                    assert_eq!(
                        synth.total_samples(),
                        (beats * spb).round() as u64 + tail,
                        "{tag}: rendered length"
                    );
                    // Every sting is in its local harmony or owned, whatever the length.
                    let sfx = SfxAudit::measure(&c.perf, s);
                    assert_eq!(sfx.unjustified, 0, "{tag}: {}", sfx.report());
                    // Every live morphism is witnessed by an action or deferred with a reason.
                    let tl = IntentTimeline::walk(&trace);
                    let d = ActionDiagnostics::measure(&tl, &c.song.plan, &c.perf, s);
                    assert_eq!(d.unwitnessed_morphisms, 0, "{tag}: {d:?}");
                    // The accent grid leaves every step at/after the end empty.
                    for bar in 0..c.song.plan.form.total_bars {
                        for step in 0..super::super::performance::STEPS {
                            let at = super::super::performance::AccentGrid::beat_of(bar, step);
                            if at >= beats {
                                let w = c.perf.accent.at(bar, step);
                                assert!(w.hole >= 1.0 && w.hit == 0.0 && w.push == 0.0, "{tag}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn an_event_that_quantizes_onto_the_end_is_deferred_not_dropped() {
        use super::super::action::ActionCause;
        use super::super::diagnostics::ActionDiagnostics;
        use super::super::semantic::{Density, Elevation, Emphasis, SemanticEvent, SemanticState};
        // A FocusAcquired fired at raw beat 10.3 of a 10.5-beat piece snaps to the eighth at 10.5
        // — exactly the end. Its morphisms (Intensify, FragmentMotif) have no time left to be
        // performed; they used to vanish without a Deferral, breaking "witnessed or deferred".
        let st = |tone| SemanticState {
            tone,
            emphasis: Emphasis::Normal,
            density: Density::Normal,
            elevation: Elevation::Raised,
        };
        let trace = SemanticTrace::new(
            vec![
                SemanticEvent {
                    at_beat: 0.0,
                    state: st(Tone::Neutral),
                    kind: EventKind::ActChanged,
                },
                SemanticEvent {
                    at_beat: 10.3,
                    state: st(Tone::Info),
                    kind: EventKind::FocusAcquired,
                },
            ],
            10.5,
        );
        let tl = IntentTimeline::walk(&trace);
        assert_eq!(
            tl.transitions[1].at_beat, 10.5,
            "fixture no longer lands on the end"
        );
        let c = compose_full(
            &trace,
            &MusicWorld::black_ice(),
            7,
            None,
            PerformanceOptions::default(),
        );
        let ap = &c.perf.actions;
        for m in [IntentMorphism::Intensify, IntentMorphism::FragmentMotif] {
            assert!(
                ap.deferred
                    .iter()
                    .any(|d| d.transition == 1 && d.morphism == m),
                "{m:?} at the end was dropped without a deferral"
            );
            assert!(!ap.actions.iter().any(|a| matches!(
                a.cause,
                ActionCause::Morphism { transition: 1, morphism } if morphism == m
            )));
        }
        let d = ActionDiagnostics::measure(&tl, &c.song.plan, &c.perf, &c.score);
        assert_eq!(d.unwitnessed_morphisms, 0, "{d:?}");
        c.score.validate().expect("end-of-piece score");
    }

    #[test]
    fn every_sting_sounds_in_its_local_harmony_or_is_owned() {
        use super::super::semantic::deflected_lift_trace;
        for (name, trace, grammar) in [
            (
                "bounce",
                deflected_lift_trace(120.0),
                Some(CompositionGrammar::DeflectedLift),
            ),
            ("demo", demo_trace(120.0), None),
        ] {
            for world in MusicWorld::all() {
                let tag = format!("{name} {}", world.name);
                let c = compose_full(&trace, &world, 2112, grammar, PerformanceOptions::default());
                let audit = SfxAudit::measure(&c.perf, &c.score);
                eprintln!("{tag}: {}", audit.report().trim_end());
                assert!(audit.total > 0, "{tag}: no stings");
                assert_eq!(audit.unjustified, 0, "{tag}: {}", audit.report());
                assert_eq!(audit.chord + audit.owned, audit.total, "{tag}");
                let mut warnings = 0;
                for e in c.score.sfx.iter().filter(|e| e.kind == SfxKind::Warning) {
                    warnings += 1;
                    let ctx = c
                        .perf
                        .context_at(e.start_beat)
                        .expect("a Warning with no harmony");
                    let in_chord = e
                        .pitches
                        .iter()
                        .all(|&p| ctx.chord.contains_pc(pitch_class(p)));
                    if in_chord {
                        assert_eq!(e.function, [Some(PitchFunction::ChordTone); 2], "{tag}");
                        continue;
                    }
                    // An owned alarm: the owner exists, came from the same event, and outlasts it.
                    let owner = e
                        .owned_by
                        .and_then(|id| c.perf.actions.get(id))
                        .unwrap_or_else(|| panic!("{tag}: an unowned dissonant Warning"));
                    let d = e.dissonance_beats.expect("an owned dissonance has a span");
                    assert!(d as f64 <= owner.dur_beats + 1e-6, "{tag}: {d} > {owner:?}");
                    assert!(d + 1e-4 >= SfxKind::Warning.max_lifetime_beats(c.score.tempo_bpm));
                    assert!(owner.covers(e.start_beat), "{tag}: owner is elsewhere");
                }
                if name == "bounce" {
                    assert_eq!(warnings, 2, "{tag}: the flagship has two Warnings");
                }
                // Stings land on the musical (quantized) beat, like every action, not the raw one.
                for e in &c.score.sfx {
                    let eighths = e.start_beat * 2.0;
                    assert!(
                        (eighths - eighths.round()).abs() < 1e-9,
                        "{tag}: {}",
                        e.start_beat
                    );
                }
            }
        }
    }

    #[test]
    fn negative_control_the_world_scale_stings_ignore_the_harmony() {
        use super::super::semantic::deflected_lift_trace;
        use super::super::synth::world_scale_sfx_pitches;
        // The pre-VIIb path over the flagship: world-scale pitches (the Warning a fixed tonic +
        // tritone), no owner. The audit must catch at least one out-of-context, unowned Warning —
        // or it is not measuring anything.
        for world in MusicWorld::all() {
            let c = compose_full(
                &deflected_lift_trace(120.0),
                &world,
                2112,
                Some(CompositionGrammar::DeflectedLift),
                PerformanceOptions::default(),
            );
            let mut old = c.score.clone();
            for e in &mut old.sfx {
                e.pitches = world_scale_sfx_pitches(e.kind, &c.perf.region);
                e.function = [None; 2];
                e.owned_by = None;
                e.dissonance_beats = None;
            }
            let lost = old
                .sfx
                .iter()
                .filter(|e| {
                    e.kind == SfxKind::Warning
                        && SfxAudit::verdict(e, &c.perf, &old) == SfxVerdict::Unjustified
                })
                .count();
            let audit = SfxAudit::measure(&c.perf, &old);
            eprintln!("{} world-scale: {}", world.name, audit.report().trim_end());
            assert!(lost >= 1, "{}: the old Warnings all fit", world.name);
            assert!(audit.unjustified >= lost);
        }
    }

    #[test]
    fn a_chord_names_its_own_tritone() {
        use super::super::theory::Quality;
        // Dom7: 3–b7 (E–Bb over C7); m7b5: 1–b5 (B–F over Bm7b5); Maj7 and a triad have none.
        assert_eq!(own_tritone(&Chord::new(0, Quality::Dom7)), Some(4));
        assert_eq!(own_tritone(&Chord::new(11, Quality::Min7b5)), Some(11));
        assert_eq!(own_tritone(&Chord::new(5, Quality::Maj7)), None);
        assert_eq!(own_tritone(&Chord::new(9, Quality::Min)), None);
    }

    #[test]
    fn the_realizer_leaves_no_unjustified_lead_notes() {
        // The jazz principle enforced: after the justify-or-snap repair, every lead note either is a
        // chord tone or carries a concrete non-chord justification (approach / passing / neighbour /
        // suspension / anticipation / appoggiatura). None is left `None` — no unjustified "wrong
        // notes". (Before the repair, BLACK_ICE alone left 7 of 36 lead notes unexplained.)
        let trace = demo_trace(120.0);
        for world in MusicWorld::all() {
            let score = compose(&trace, &world, 2112);
            let unjustified = score
                .notes
                .iter()
                .filter(|n| n.role == Role::Lead && n.function.is_none())
                .count();
            assert_eq!(
                unjustified, 0,
                "{}: {} lead notes have no pitch justification",
                world.name, unjustified
            );
        }
    }

    #[test]
    fn every_bass_note_carries_a_justified_function() {
        // Bass used to leave `function = None` on every note, invisible to diagnostics. Now every
        // bass note is typed, and the line actually approaches or walks into a chord change — which
        // also proves the formerly-dead SlidePath / ChromaticApproach variants are emitted.
        let trace = demo_trace(120.0);
        for world in MusicWorld::all() {
            let score = compose(&trace, &world, 2112);
            for n in score.notes.iter().filter(|n| n.role == Role::Bass) {
                assert!(
                    n.function.is_some(),
                    "{}: a bass note carries no PitchFunction",
                    world.name
                );
            }
            let approaches = score.notes.iter().any(|n| {
                n.role == Role::Bass
                    && matches!(
                        n.function,
                        Some(PitchFunction::SlidePath) | Some(PitchFunction::ChromaticApproach)
                    )
            });
            assert!(
                approaches,
                "{}: the bass never approaches or walks into a chord change",
                world.name
            );
        }
    }

    #[test]
    fn a_bass_chord_tone_is_really_a_chord_tone() {
        // The old code played `root_pc + 7` as the "fifth" even on a diminished chord, whose real
        // fifth is a semitone lower — a non-chord tone (a wrong note) no diagnostic ever saw. Every
        // bass note typed as a ChordTone must actually belong to the chord sounding beneath it.
        let trace = demo_trace(120.0);
        for world in MusicWorld::all() {
            let score = compose(&trace, &world, 2112);
            for n in score.notes.iter().filter(|n| n.role == Role::Bass) {
                if n.function != Some(PitchFunction::ChordTone) {
                    continue; // approach / slide are justified by their path, not the local chord
                }
                let chord = score
                    .chords
                    .iter()
                    .filter(|c| c.start_beat <= n.start_beat + 1e-6)
                    .max_by(|a, b| a.start_beat.total_cmp(&b.start_beat))
                    .map(|c| c.chord);
                assert!(
                    chord.is_some_and(|c| c.contains_pc(pitch_class(n.pitch))),
                    "{}: bass ChordTone {} is not a tone of its sounding chord",
                    world.name,
                    n.pitch
                );
            }
        }
    }

    #[test]
    fn deflected_lift_recurs_the_hook_with_a_clean_ledger() {
        use super::super::contract::CompositionGrammar;
        use super::super::diagnostics::{DiscourseDiagnostics, RealizationDiagnostics};
        use super::super::discourse::DiscourseRole;
        let trace = demo_trace(120.0);
        for world in MusicWorld::all() {
            let (score, plan) =
                compose_with_grammar(&trace, &world, 2112, CompositionGrammar::DeflectedLift);
            score
                .validate()
                .unwrap_or_else(|e| panic!("{}: {e}", world.name));
            // The hook recurs — the bittersweet bounce, not one cinematic climax.
            let hooks = plan
                .discourse
                .goals
                .iter()
                .filter(|g| g.role == DiscourseRole::Culminate)
                .count();
            assert!(
                hooks >= 2,
                "{}: the hook did not recur ({hooks} culminations)",
                world.name
            );
            // Each cycle settles the debt it opens — no cinematic obligation left hanging.
            assert_eq!(
                DiscourseDiagnostics::measure(&plan, &score).abandoned_obligations,
                0,
                "{}: DeflectedLift abandoned an obligation",
                world.name
            );
            // The jazz principle still holds across every audible role under the new backbone.
            assert_eq!(
                RealizationDiagnostics::measure(&plan, &score).unjustified_nonchord_notes,
                0,
                "{}: DeflectedLift left an unjustified note",
                world.name
            );
        }
    }

    #[test]
    fn compose_produces_a_valid_multivoice_score() {
        let trace = demo_trace(120.0);
        for world in MusicWorld::all() {
            let score = compose(&trace, &world, 2112);
            score
                .validate()
                .unwrap_or_else(|e| panic!("{}: {e}", world.name));
            // All four roles present.
            for role in [Role::Pad, Role::Bass, Role::Lead, Role::Keys] {
                assert!(
                    score.role_notes(role).count() > 0,
                    "{}: role {:?} empty",
                    world.name,
                    role
                );
            }
            assert!(!score.drums.is_empty(), "{}: no drums", world.name);
            assert!(!score.sfx.is_empty(), "{}: no sfx", world.name);
        }
    }

    #[test]
    fn keys_comp_in_voiced_stabs_not_single_notes() {
        use super::super::contract::CompositionGrammar;
        use super::super::semantic::deflected_lift_trace;
        use std::collections::BTreeMap;
        // Keys are a comping voice now: at some onsets they sound a 2-3 note shell, not a lone note.
        let trace = deflected_lift_trace(120.0);
        let (score, _) = compose_with_grammar(
            &trace,
            &MusicWorld::black_ice(),
            2112,
            CompositionGrammar::DeflectedLift,
        );
        let mut by_onset: BTreeMap<u64, usize> = BTreeMap::new();
        for n in score.notes.iter().filter(|n| n.role == Role::Keys) {
            *by_onset
                .entry((n.start_beat * 1000.0).round() as u64)
                .or_default() += 1;
        }
        assert!(!by_onset.is_empty(), "no keys notes at all");
        let voiced = by_onset.values().filter(|&&c| c >= 2).count();
        assert!(
            voiced > 0,
            "keys never play a voiced stab — still a single-note arp"
        );
    }

    #[test]
    fn same_trace_gives_same_form_across_worlds() {
        // The natural-transformation invariant: form skeleton is world-independent.
        let trace = demo_trace(120.0);
        let a = compose(&trace, &MusicWorld::black_ice(), 1);
        let b = compose(&trace, &MusicWorld::vapor95(), 1);
        let c = compose(&trace, &MusicWorld::swiss_signal(), 1);
        let kinds = |s: &Score| s.sections.iter().map(|x| x.kind).collect::<Vec<_>>();
        assert_eq!(kinds(&a), kinds(&b));
        assert_eq!(kinds(&b), kinds(&c));
        // ...but the dialects differ (tempo).
        assert_ne!(a.tempo_bpm, b.tempo_bpm);
    }

    #[test]
    fn deterministic_for_seed() {
        let trace = demo_trace(120.0);
        let a = compose(&trace, &MusicWorld::black_ice(), 77);
        let b = compose(&trace, &MusicWorld::black_ice(), 77);
        assert_eq!(a.notes.len(), b.notes.len());
        assert_eq!(a.drums.len(), b.drums.len());
        for (x, y) in a.notes.iter().zip(b.notes.iter()) {
            assert_eq!(x.pitch, y.pitch);
            assert!((x.start_beat - y.start_beat).abs() < 1e-9);
        }
    }

    #[test]
    fn walk_intent_resolves_at_the_end() {
        // The demo arc ends in resolution (SectionResolved -> Relax, Cadence).
        let intent = walk_intent(&demo_trace(120.0));
        assert!(
            intent.tension < 0.5,
            "ends unresolved: tension {}",
            intent.tension
        );
        // And it developed the motif along the way (Impact -> Modulate bumps development).
        assert!(intent.motif.development > 0);
    }

    #[test]
    fn grammar_forces_the_resolution_policy_in_the_chords() {
        let trace = demo_trace(120.0);
        let world = MusicWorld::black_ice();
        let hook = compose_with_grammar(&trace, &world, 7, CompositionGrammar::HookArc).0;
        let loopy = compose_with_grammar(&trace, &world, 7, CompositionGrammar::LoopEvolution).0;
        let riff = compose_with_grammar(&trace, &world, 7, CompositionGrammar::RiffDrive).0;
        // Loop cycles home ("loop"), riff pedals ("pedal"), hook uses functional cadences (neither).
        assert!(
            loopy.chords.iter().any(|c| c.note == "loop"),
            "loop grammar never cycled home"
        );
        assert!(
            riff.chords.iter().any(|c| c.note == "pedal"),
            "riff grammar never pedalled"
        );
        assert!(
            hook.chords
                .iter()
                .all(|c| c.note != "loop" && c.note != "pedal"),
            "hook grammar leaked a non-functional cadence tag"
        );
        // The three grammars are genuinely different progressions, not one relabelled.
        assert!(
            hook.chords
                .iter()
                .map(|c| c.chord)
                .ne(loopy.chords.iter().map(|c| c.chord)),
            "hook and loop produced identical chords"
        );
    }
}
