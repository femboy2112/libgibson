//! The composition entry points: a semantic trace becomes a [`Score`] under a [`MusicWorld`].
//!
//! [`compose_full`] composes one [`SongMap`] ([`SongMap::build`]: the trace walked into a causal
//! [`IntentTimeline`](super::timeline::IntentTimeline) — each semantic event a sequence of intent morphisms evolving a running
//! [`MusicIntent`] — and the [`CompositionPlan`]) and [`perform`]s it: one [`PerformancePlan`], every
//! player a projection of that performance. Swapping the world or the language keeps the song (its
//! theme, its chart, its rhythm — [`super::song::SongMapConformance`] checks it); the room
//! re-modes and colours it, the idiom declares its rhythm transform and plays its own fiber.

use super::action::Agent;
use super::contract::CompositionGrammar;
use super::form::{Section, BEATS_PER_BAR};
use super::harmonic_state::HarmonicEnsembleState;
use super::ids::ActionId;
use super::intent::{IntentMorphism, MusicIntent};
use super::performance::{EnsembleCoupling, PerformanceOptions, PerformancePlan};
use super::plan::{ArrangementRole, CompositionPlan};
use super::policy::{
    ExpressionPolicy, HistoricalRepair, OccupancyPolicy, PerformanceProfile, PitchPolicy,
    PolicyError, SourceEvidencePolicy, SupportPolicy, VoiceLifetimePolicy,
};
use super::score::{Hearing, Note, Provenance, Role, Score};
use super::semantic::{EventKind, SemanticTrace, Tone};
use super::sfx::add_sfx_and_provenance;
use super::song::SongMap;
use super::sonority::{plan_sonority, ColorPolicy};
use super::theory::Midi;
use super::world::MusicWorld;

/// Compatibility re-exports; SFX ownership and audit live in their own subsystem.
pub use super::sfx::{SfxAudit, SfxVerdict};

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
    perform_historical(song, world, opts, PerformanceProfile::WRITTEN)
}

/// Round XII opt-in pitch-path realization of the same song and performance plan.
/// The default [`perform`] remains the exact Round XI listening control.
pub fn perform_temporal(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
) -> Composition {
    perform_historical(song, world, opts, PerformanceProfile::TEMPORAL)
}

/// Round XIII opt-in: the Round XII realization with the support voicings' temporal-mass contract
/// ([`super::comp::gate_support_mass`]). Same song, same performance plan, same lead and bass;
/// [`perform_temporal`] remains the exact Round XII listening control.
pub fn perform_mass(song: &SongMap, world: &MusicWorld, opts: PerformanceOptions) -> Composition {
    perform_historical(
        song,
        world,
        opts,
        PerformanceProfile {
            repair: HistoricalRepair::SupportMass,
            ..PerformanceProfile::TEMPORAL
        },
    )
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
    perform_historical(
        song,
        world,
        opts,
        PerformanceProfile {
            repair: HistoricalRepair::SoundingTension,
            ..PerformanceProfile::TEMPORAL
        },
    )
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
    perform_historical(song, world, opts, PerformanceProfile::HEARD)
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
    perform_historical(song, world, opts, PerformanceProfile::EXPRESSIVE)
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

fn phrase_profile(options: PhraseOptions) -> PerformanceProfile {
    PerformanceProfile {
        expression: if options.phrase_expression {
            ExpressionPolicy::Phrase
        } else {
            ExpressionPolicy::LocalConnectives
        },
        occupancy: if options.semantic_occupancy {
            OccupancyPolicy::AuthoredIntent
        } else {
            OccupancyPolicy::Acoustic
        },
        support: if options.support_voicing {
            SupportPolicy::SourceVoicePath
        } else {
            SupportPolicy::HeardHarmony
        },
        ..PerformanceProfile::PHRASED
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
    perform_historical(song, world, opts, phrase_profile(factors))
}

pub use super::pocket::PocketOptions;

/// Round XVII opt-in pocket source arm. The maintainer accepted the BLACK_ICE flagship pocket;
/// general acceptance remains unverified.
///
/// # Panics
/// Requires Independent coupling to preserve final-source hearings.
pub fn perform_pocketed(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
) -> Composition {
    perform_pocket_experiment(song, world, opts, PocketOptions::default())
}

/// Independently toggle lattice selection, legato articulation, explicit voice continuity,
/// and the reserved support hypothesis. Support promotion awaits human knockout evidence.
///
/// # Panics
/// Requires Independent coupling.
pub fn perform_pocket_experiment(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
    factors: PocketOptions,
) -> Composition {
    assert_eq!(
        opts.coupling,
        EnsembleCoupling::Independent,
        "Round XVII requires Independent coupling"
    );
    perform_historical(
        song,
        world,
        opts,
        PerformanceProfile::legacy_pocket(factors),
    )
}

/// Perform with explicit musical laws. Existing `perform` remains the default historical path.
/// Invalid combinations fail before performance planning or source generation.
pub fn perform_with_profile(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
    profile: PerformanceProfile,
) -> Result<Composition, PolicyError> {
    profile.validate(opts.coupling)?;
    // The unconstrained planner has no cover domain; its one refusal is a law the profile made a
    // source law (a chart chord outside the world's harmonic vocabulary).
    let (perf, score) =
        plan_and_realize(song, world, opts, None, profile).map_err(|e| match e {
            super::cover::CoverError::Invalid(why) => PolicyError(why),
            _ => PolicyError("the unconstrained planner refused its own plan"),
        })?;
    Ok(Composition {
        score,
        song: song.clone(),
        perf,
    })
}

/// A performance under explicit laws, not yet judged: exactly [`perform_with_profile`]. The
/// candidate a checked route admits or rejects — kept so a rejected take can still be inspected
/// and heard.
pub fn perform_candidate(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
    profile: PerformanceProfile,
) -> Result<Composition, PolicyError> {
    perform_with_profile(song, world, opts, profile)
}

/// The checked modern route: the candidate is returned only when every general performance law
/// holds under `profile`'s declared laws ([`super::receipt::PerformanceReceipt::measure_under`]);
/// otherwise it is rejected with the receipt naming each failure. A profile such as
/// [`PerformanceProfile::BAND`] thereby rejects an internally invalid performance itself — no
/// cover mode or holdout harness is needed to discover it. The historical `perform` is unchanged.
pub fn perform_checked(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
    profile: PerformanceProfile,
) -> Result<Composition, super::receipt::PerformanceRejection> {
    use super::receipt::{PerformanceReceipt, PerformanceRejection};
    let c = perform_candidate(song, world, opts, profile).map_err(PerformanceRejection::Refused)?;
    let receipt = PerformanceReceipt::measure_under(&c, world, profile);
    if receipt.passes() {
        Ok(c)
    } else {
        Err(PerformanceRejection::Rejected(Box::new(receipt)))
    }
}

/// Plan and realize under `profile`'s verb-admission law (the profile is already validated).
///
/// `Planned` is the historical single pass. `Rehearsed` is the finite normalization of
/// [`super::rehearsal`]: plan, realize, audit every verb against the realized score, then strike
/// the verbs no player performed (and move a settlement nobody played onto the arrival a player
/// made in its phrase, once), and plan again — until every verb on the chart is performed. The
/// take is an ordinary realization of the final plan; the audit is unchanged and still judges it.
/// A normalization that cannot make progress, or exhausts [`super::rehearsal::REHEARSAL_FUEL`], is
/// refused: a take with an unperformed verb is never returned.
pub(crate) fn plan_and_realize(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
    constraints: Option<super::cover::CoverConstraints>,
    profile: PerformanceProfile,
) -> Result<(PerformancePlan, Score), super::cover::CoverError> {
    use super::performance::{ActionKey, AdmissionInputs};
    use super::policy::ActionAdmission;
    use super::rehearsal::{RehearsalOutcome, RehearsedVerb, REHEARSAL_FUEL};
    let observed = Some(profile.observation);
    let rehearsed = profile.admission == ActionAdmission::Rehearsed;
    let mut inputs = AdmissionInputs {
        rehearsed,
        vetoed: Vec::new(),
        recast: Vec::new(),
        harmony: profile.harmony,
        functions: profile.functions,
    };
    let mut judged: Vec<RehearsedVerb> = Vec::new();
    for pass in 0..=REHEARSAL_FUEL {
        let mut perf =
            PerformancePlan::from_song_admitted(song, world, opts, constraints.clone(), &inputs)?;
        let score = realize_policy(song, world, &perf, profile, observed);
        admit_take(&perf, &score).map_err(super::cover::CoverError::Invalid)?;
        if !rehearsed {
            return Ok((perf, score));
        }
        let audit = super::witness::audit(&perf, &score);
        let unperformed: Vec<&super::action::MusicalAction> = audit
            .rows
            .iter()
            .filter(|r| !r.witnessed)
            .filter_map(|r| perf.actions.get(r.action))
            .collect();
        if unperformed.is_empty() {
            // The take: every verb on the chart is performed (a recast one where it moved to).
            for a in &perf.actions.actions {
                let recast = inputs.recast.iter().find(|r| {
                    matches!(a.cause, super::action::ActionCause::Discourse { obligation, .. } if obligation == r.obligation)
                });
                judged.push(RehearsedVerb {
                    key: ActionKey::of(a, &perf.actions),
                    kind: a.kind,
                    planned_beat: a.start_beat,
                    pass,
                    outcome: match recast {
                        Some(r) => RehearsalOutcome::Recast {
                            beat: r.start_beat,
                            by: r.by,
                        },
                        None => RehearsalOutcome::Performed,
                    },
                });
            }
            let trace = perf.rehearsal.get_or_insert_with(Default::default);
            trace.passes = pass;
            trace.verbs = judged;
            return Ok((perf, score));
        }
        let mut progress = false;
        for a in unperformed {
            let key = ActionKey::of(a, &perf.actions);
            if inputs.vetoed.contains(&key) {
                continue; // an ineffective veto: no progress from it
            }
            if let (super::action::ActionKind::Resolve, Some(obligation)) = (a.kind, key.settles())
            {
                if !inputs.recast.iter().any(|r| r.obligation == obligation) {
                    if let Some(r) =
                        super::rehearsal::observed_arrival(song, &perf, &score, a, obligation)
                    {
                        inputs.recast.push(r);
                        progress = true;
                        continue;
                    }
                }
            }
            let outcome = match a.cause {
                super::action::ActionCause::Morphism { .. } => {
                    RehearsalOutcome::Deferred(super::performance::REHEARSAL_REJECTION)
                }
                _ => RehearsalOutcome::Rejected(super::performance::REHEARSAL_REJECTION),
            };
            judged.push(RehearsedVerb {
                key: key.clone(),
                kind: a.kind,
                planned_beat: a.start_beat,
                pass,
                outcome,
            });
            inputs.vetoed.push(key);
            progress = true;
        }
        if !progress {
            return Err(super::cover::CoverError::Invalid(
                "rehearsal cannot strike a verb nobody performs",
            ));
        }
    }
    Err(super::cover::CoverError::Invalid(
        "rehearsal did not converge within its fuel",
    ))
}

fn perform_historical(
    song: &SongMap,
    world: &MusicWorld,
    opts: PerformanceOptions,
    profile: PerformanceProfile,
) -> Composition {
    let perf = PerformancePlan::from_song(song, world, opts);
    let score = realize_policy(song, world, &perf, profile, None);
    stage_evidence(&perf, &score);
    Composition {
        score,
        song: song.clone(),
        perf,
    }
}

/// Realize an already planned (including constrained cover) performance under explicit laws.
/// Constraints belong to the supplied plan and are consumed before source choices.
pub fn realize_with_profile(
    song: &SongMap,
    world: &MusicWorld,
    perf: &PerformancePlan,
    profile: PerformanceProfile,
) -> Result<Score, PolicyError> {
    profile.validate(perf.coupling)?;
    let score = realize_policy(song, world, perf, profile, Some(profile.observation));
    admit_take(perf, &score).map_err(PolicyError)?;
    Ok(score)
}

/// Realize a score from an explicit (possibly hand-mutated) song and performance — the entry the
/// adversarial probes use to inject a call, veto an arrangement, or license a burst and watch what
/// the players do with it.
pub fn realize_performance(song: &SongMap, world: &MusicWorld, perf: &PerformancePlan) -> Score {
    let score = realize_policy(song, world, perf, PerformanceProfile::WRITTEN, None);
    stage_evidence(perf, &score);
    score
}

/// Causal realization under musical policies; historical names end at their adapters.
fn realize_policy(
    song: &SongMap,
    world: &MusicWorld,
    perf: &PerformancePlan,
    profile: PerformanceProfile,
    observed_lifetime: Option<super::voice::ObservedLifetimePolicy>,
) -> Score {
    let (trace, seed, plan) = (&song.trace, song.seed, &song.plan);
    let total_beats = plan.form.total_beats;
    let mut score = Score::new(world.tempo_bpm, BEATS_PER_BAR, total_beats);
    score.observed_lifetime = observed_lifetime;
    score.sections = sections_from_plan(plan);
    score.chords = perf.chords.clone();

    let temporal = profile.pitch == PitchPolicy::Temporal;
    let phrase_evidence = profile.evidence == SourceEvidencePolicy::AuthoredSources;
    let phrase_expression = matches!(
        profile.expression,
        ExpressionPolicy::Phrase | ExpressionPolicy::Pulse(_)
    );
    let semantic_occupancy = profile.occupancy == OccupancyPolicy::AuthoredIntent;
    let pulse = if let ExpressionPolicy::Pulse(policy) = profile.expression {
        Some(policy)
    } else {
        None
    };
    let pocket = pulse.map(|policy| policy.source_options());
    score.mono_voice = profile.lifetime == VoiceLifetimePolicy::ExplicitContinuations;
    let lead = match profile.expression {
        ExpressionPolicy::Pulse(policy) => {
            super::melody::realize_lead_pocketed(perf, plan, world, policy.source_options())
        }
        ExpressionPolicy::Phrase => super::melody::realize_lead_phrased(perf, plan, world),
        ExpressionPolicy::LocalConnectives => {
            super::melody::realize_lead_expressive(perf, plan, world)
        }
        ExpressionPolicy::Unchanged if temporal => super::melody::realize_lead_temporal(perf, plan),
        ExpressionPolicy::Unchanged => super::melody::realize_lead(perf, plan),
    };
    let lead_occupancy = super::occupancy::AuthoredOccupancy::from_lead(perf, &lead.authored);
    let agency = semantic_occupancy.then_some(&lead_occupancy);
    if phrase_evidence {
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
            let keys = if semantic_occupancy {
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
            if profile.support != SupportPolicy::Independent {
                // Round XIV keeps temporal bass; Round XV also hears final keys when expressing
                // its connectives. The frozen coherent pad comes last and hears the band.
                let mut keys = keys;
                let mark = (
                    score.hearings.len(),
                    score.occupancy.len(),
                    score.phrase_plans.len(),
                    score.expression_decisions.len(),
                );
                let answers = perf.functions == super::policy::FunctionPolicy::Earned
                    && (semantic_occupancy || temporal)
                    && profile.support == SupportPolicy::SourceVoicePath;
                // The keys' path once they have answered (`None`: as authored), and the passes left.
                let mut answered_path: Option<super::voicing::RolePath> = None;
                let mut passes = KEYS_ANSWER_PASSES;
                let bass = loop {
                    let bass = if phrase_expression {
                        score.hearings.push(Hearing::of("bass", Role::Keys, &keys));
                        let result = if let Some(factors) = pocket {
                            super::bass::realize_bass_pocketed(
                                perf,
                                plan,
                                world,
                                &lead.notes,
                                &keys,
                                &lead.phrase_plans,
                                agency,
                                factors,
                            )
                        } else {
                            super::bass::realize_bass_phrased(
                                perf,
                                plan,
                                world,
                                &lead.notes,
                                &keys,
                                &lead.phrase_plans,
                                agency,
                            )
                        };
                        let bass_intent = if pulse.is_some_and(|p| p.changes_source()) {
                            // The final lead's unison can shorten/lengthen its predecessor at the
                            // bass source. Recover the whole authored reservation stream, not only
                            // the explicitly retimed event. This is intent, never an acoustic hearing.
                            let source = super::bass::realize_bass_temporal_owned(
                                perf,
                                plan,
                                world,
                                &lead.authored,
                                agency,
                            );
                            super::occupancy::AuthoredOccupancy::from_bass(perf, &source, &[])
                        } else {
                            super::occupancy::AuthoredOccupancy::from_bass(
                                perf,
                                &result.authored,
                                &score.expression_decisions,
                            )
                        };
                        score.occupancy.push(bass_intent);
                        score.phrase_plans.extend(result.plans);
                        score.expression_decisions.extend(result.decisions);
                        result.notes
                    } else if profile.expression == ExpressionPolicy::LocalConnectives {
                        score.hearings.push(Hearing::of("bass", Role::Keys, &keys));
                        if phrase_evidence {
                            let source = super::bass::realize_bass_temporal_owned(
                                perf,
                                plan,
                                world,
                                &lead.notes,
                                agency,
                            );
                            score
                                .occupancy
                                .push(super::occupancy::AuthoredOccupancy::from_bass(
                                    perf,
                                    &source,
                                    &score.expression_decisions,
                                ));
                        }
                        let (notes, decisions) = super::bass::realize_bass_expressive_owned(
                            perf,
                            plan,
                            world,
                            &lead.notes,
                            &keys,
                            agency,
                        );
                        score.expression_decisions.extend(decisions);
                        notes
                    } else {
                        super::bass::realize_bass_temporal(perf, plan, world, &lead.notes, &keys)
                    };
                    if !answers || passes == 0 {
                        break bass;
                    }
                    passes -= 1;
                    // Earned: harmonic identity is the responsibility of the support that sounds the
                    // harmony. Whatever the band, with the pad's answer, still holds as another chord,
                    // the keys answer for at the source; when they change, the bass hears the keys as
                    // they now are (its earlier take is undone) and the keys listen again to that
                    // bass — until they have nothing left to answer. The take always pairs the final
                    // keys with the bass that heard them.
                    let (kp, edits) = {
                        let kp = answered_path.clone().unwrap_or_else(|| {
                            super::voicing::keys_path(
                                perf,
                                world.voicing_spread,
                                &lead.notes,
                                super::comp::keys_shell_n(perf),
                            )
                        });
                        let render = |kp: &super::voicing::RolePath| {
                            if semantic_occupancy {
                                super::comp::realize_keys_owned_on(
                                    perf,
                                    world,
                                    &lead.notes,
                                    &lead_occupancy,
                                    seed,
                                    kp,
                                )
                            } else {
                                super::comp::realize_keys_temporal_on(
                                    perf,
                                    world,
                                    &lead.notes,
                                    seed,
                                    kp,
                                )
                            }
                        };
                        let hear = |keys: &[Note]| {
                            let band: Vec<Note> = lead
                                .notes
                                .iter()
                                .chain(keys)
                                .chain(&bass)
                                .copied()
                                .collect();
                            let mut inside: Vec<Note> = band
                                .iter()
                                .copied()
                                .filter_map(|mut n| clip_note(&mut n, total_beats).then_some(n))
                                .collect();
                            let canonical = score.observed_lifetime_policy()
                                == super::voice::ObservedLifetimePolicy::ExplicitContinuity;
                            let links = if score.mono_voice {
                                super::pocket::continuations(&band, &score.phrase_plans)
                            } else {
                                score.voice_continuity.clone()
                            };
                            // The pad as it will answer this band: its tails sound into the
                            // harmonies it leaves to the keys.
                            let pad = if canonical {
                                super::comp::realize_pad_pocketed(perf, world, &inside, &links).0
                            } else {
                                super::comp::realize_pad_phrased(perf, plan, world, &inside).0
                            };
                            inside.extend(
                                pad.into_iter().filter_map(|mut n| {
                                    clip_note(&mut n, total_beats).then_some(n)
                                }),
                            );
                            if canonical {
                                let heard = super::voice::HeardWindows::explicit(
                                    &inside,
                                    world,
                                    world.tempo_bpm,
                                    &links,
                                );
                                super::identity::IdentityDiagnostics::measure_heard_windows(
                                    &heard,
                                    &perf.contexts,
                                )
                            } else {
                                super::identity::IdentityDiagnostics::measure(
                                    &inside,
                                    &perf.contexts,
                                    world,
                                    world.tempo_bpm,
                                )
                            }
                        };
                        let (kp, edits) =
                            super::comp::keys_answer_for_identity(perf, world, &kp, &render, &hear);
                        let keys = (!edits.is_empty() && passes > 0).then(|| render(&kp));
                        (keys.map(|keys| (kp, keys)), edits)
                    };
                    // No edit, or no pass left for the bass to hear one: this bass heard these keys.
                    let Some((kp, answering)) = kp else {
                        break bass;
                    };
                    keys = answering;
                    answered_path = Some(kp);
                    score.keys_voicing_edits.extend(edits);
                    score.hearings.truncate(mark.0);
                    score.occupancy.truncate(mark.1);
                    score.phrase_plans.truncate(mark.2);
                    score.expression_decisions.truncate(mark.3);
                };
                let band: Vec<Note> = lead
                    .notes
                    .iter()
                    .chain(&keys)
                    .chain(&bass)
                    .copied()
                    .collect();
                if score.mono_voice {
                    score.voice_continuity =
                        super::pocket::continuations(&band, &score.phrase_plans);
                }
                for (source, notes) in [
                    (Role::Lead, &lead.notes),
                    (Role::Keys, &keys),
                    (Role::Bass, &bass),
                ] {
                    score.hearings.push(Hearing::of("pad", source, notes));
                }
                // Earned: the pad judges the band as it will sound, inside the piece
                // (`clip_to_end` trims every realizer's tail only after the pad has heard it).
                let inside: Vec<Note>;
                let band: &[Note] = if perf.functions == super::policy::FunctionPolicy::Earned {
                    inside = band
                        .iter()
                        .copied()
                        .filter_map(|mut n| clip_note(&mut n, total_beats).then_some(n))
                        .collect();
                    &inside
                } else {
                    &band
                };
                let canonical_observation = score.observed_lifetime_policy()
                    == super::voice::ObservedLifetimePolicy::ExplicitContinuity;
                let (pad, edits) =
                    if canonical_observation && profile.support == SupportPolicy::SourceVoicePath {
                        let (notes, edits, decisions) = super::comp::realize_pad_pocketed(
                            perf,
                            world,
                            band,
                            &score.voice_continuity,
                        );
                        score.support_voicing_decisions = decisions;
                        (notes, edits)
                    } else if profile.support == SupportPolicy::SourceVoicePath {
                        let (notes, edits, decisions) =
                            super::comp::realize_pad_phrased(perf, plan, world, band);
                        score.support_voicing_decisions = decisions;
                        (notes, edits)
                    } else if canonical_observation {
                        super::comp::realize_pad_heard_with_continuity(
                            perf,
                            world,
                            band,
                            &score.voice_continuity,
                        )
                    } else {
                        super::comp::realize_pad_heard(perf, plan, world, band)
                    };
                score.pad_voicing_edits = edits;
                (pad, keys, bass)
            } else {
                let mut pad = super::comp::realize_pad(perf, plan, world);
                let mut keys = keys;
                if profile.mass() {
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
    let intent = semantic_occupancy.then(|| {
        score
            .occupancy
            .iter()
            .find(|o| o.role == Role::Bass)
            .expect("bass source occupancy")
    });
    score.drums = match (profile.percussion, intent) {
        (super::percussion::PercussionPolicy::Arbitrated(restraint), intent) => {
            // One percussion surface: the drummer also hears the keys it must leave room for.
            score.hearings.push(Hearing::of("drums", Role::Keys, &keys));
            let (drums, report, origins) = super::groove::realize_drums_arbitrated(
                perf,
                plan,
                world,
                seed,
                [&bass, &keys, &pad],
                &lead.notes,
                intent,
                restraint,
            );
            score.percussion = Some(report);
            score.stroke_origins = Some(origins);
            drums
        }
        (super::percussion::PercussionPolicy::Unarbitrated, Some(intent)) => {
            super::groove::realize_drums_owned(perf, plan, world, seed, &bass, &lead.notes, intent)
        }
        (super::percussion::PercussionPolicy::Unarbitrated, None) => {
            super::groove::realize_drums(perf, plan, world, seed, &bass, &lead.notes)
        }
    };
    score.notes.extend(pad);
    score.notes.extend(keys);
    score.notes.extend(bass);
    score.notes.extend(lead.notes);
    // Round XIIIb: after the drums have heard the band as written, so the groove is unchanged.
    // Every interaction receipt the band witnesses must survive each edit.
    if profile.tension() {
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
    score
}

/// A realized take is admitted only when nobody sounds where the stage has them out — the one
/// orchestration authority, enforced on every `Result` route (a release build never returns a
/// stage violation). The historical adapters, which return no `Result`, keep the same law as a
/// debug assertion ([`stage_evidence`]).
fn admit_take(perf: &PerformancePlan, score: &Score) -> Result<(), &'static str> {
    if orchestration_violations(perf, score).is_empty() {
        Ok(())
    } else {
        Err(STAGE_VIOLATION)
    }
}

/// The refusal for a realized event whose player the stage had out.
pub const STAGE_VIOLATION: &str = "a realizer played somebody the stage had out";

/// Redundant developer evidence on the historical, `Result`-free adapters.
fn stage_evidence(perf: &PerformancePlan, score: &Score) {
    debug_assert!(
        orchestration_violations(perf, score).is_empty(),
        "{STAGE_VIOLATION}"
    );
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
    score.notes.retain_mut(|n| clip_note(n, end));
    score.drums.retain(|d| starts_in(d.start_beat));
    if let Some(origins) = &mut score.stroke_origins {
        origins.retain(|o| starts_in(o.performed));
    }
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

/// The piece-end law for one note: nothing starts at or after `end`, nothing rings past it.
/// `false` when the note does not sound inside the piece at all.
/// Under earned functions: how many times the keys may listen to the bass and answer for the
/// chart's identity (each answer that changes them is heard by a new bass take). A bound, not a
/// tuning: the keys only ever add the root where the band holds another chord.
const KEYS_ANSWER_PASSES: usize = 4;

fn clip_note(n: &mut Note, end: f64) -> bool {
    if n.start_beat >= end - 1e-9 {
        return false;
    }
    if n.start_beat + n.dur_beats as f64 > end + 1e-9 {
        n.dur_beats = (end - n.start_beat) as f32;
    }
    n.dur_beats > 0.0
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
    use super::super::score::{PitchFunction, Role, SfxKind};
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
