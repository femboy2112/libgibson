//! A bounded causal performance over the existing song, source and interaction objects.
//!
//! This first slice admits a four-bar lead statement/breath/return. Composition is partial:
//! incompatible source, stage, register or mono occupancy returns an error. Role decisions are
//! made against the frozen harmonic base; committed events are never repaired after consumption.
//! Historical performers and their fingerprints do not enter this path.

use super::action::{ActionCause, ActionKind, ActionPlan, Agent, EffectVector, MusicalAction};
use super::argument::CompiledArgument;
use super::form::SectionKind;
use super::functor::Composition;
use super::harmony::ChordSpan;
use super::ids::{ActionId, InteractionId, MaterialId};
use super::interaction::{Call, Interaction, InteractionPlan, LeadStatement, Response, Transform};
use super::material::{InteractionMaterial, MaterialSource};
use super::performance::{AdmissionInputs, PerformanceOptions, PerformancePlan};
use super::score::{DrumHit, DrumVoice, Note, PitchFunction, Provenance, Role, Score};
use super::theory::{Chord, PitchBasis, Quality, Scale};
use super::voice::{VoiceContinuation, VoiceEventId};
use super::world::MusicWorld;

/// A refusal at a real partial-composition boundary, never an instruction to repair a take.
#[derive(Debug, Clone, PartialEq)]
pub enum KernelError {
    Source(&'static str),
    Stage(Agent),
    Capacity(Role),
    StaleObservation(usize),
    InvalidScore(String),
    ChangedCommitment(Agent),
}
impl std::fmt::Display for KernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "C137 conversation refused: {self:?}")
    }
}
impl std::error::Error for KernelError {}

/// The actual completed note window consumed by one decision. Dynamics, source event identity,
/// pitch function and explicit continuation are observable here; downstream DSP is not.
/// Kept outside Score so legacy HeardNote, public Score layout and fingerprints stay unchanged.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveHearing {
    pub listener: Agent,
    pub source: Role,
    pub from_beat: f64,
    pub at_beat: f64,
    notes: Vec<(VoiceEventId, f32, f32, Option<PitchFunction>)>,
    links: Vec<VoiceContinuation>,
}
impl EffectiveHearing {
    fn capture(listener: Agent, source: Role, from: f64, at: f64, score: &Score) -> Self {
        let notes: Vec<_> = score
            .role_notes(source)
            .filter(|n| n.start_beat >= from && n.start_beat < at)
            .map(|n| (VoiceEventId::of(n), n.dur_beats, n.velocity, n.function))
            .collect();
        let links = score
            .voice_continuity
            .iter()
            .filter(|e| notes.iter().any(|n| n.0 == e.from))
            .copied()
            .collect();
        Self {
            listener,
            source,
            from_beat: from,
            at_beat: at,
            notes,
            links,
        }
    }
    /// Re-observe exactly the consumed window; future declarations are excluded.
    pub fn matches(&self, score: &Score) -> bool {
        *self
            == Self::capture(
                self.listener,
                self.source,
                self.from_beat,
                self.at_beat,
                score,
            )
    }
    /// Number of actual events consumed; zero records a real withheld window.
    pub fn event_count(&self) -> usize {
        self.notes.len()
    }
}

/// A committed take exposes only shared references. Editing an exported Score creates a
/// candidate that must be judged again; it cannot mutate what a dependent actually consumed.
pub struct CommittedPerformance {
    composition: Composition,
    hearings: Vec<EffectiveHearing>,
    source: CompiledArgument,
    world: MusicWorld,
}
impl CommittedPerformance {
    pub fn composition(&self) -> &Composition {
        &self.composition
    }
    pub fn score(&self) -> &Score {
        &self.composition.score
    }
    pub fn hearings(&self) -> &[EffectiveHearing] {
        &self.hearings
    }
    /// Judge exported mutations without blessing or modifying them. This is an event/source
    /// contract, not a claim about perception, mastering or general musical quality.
    /// Every emitted event is a commitment, including events nobody else consumed. A different
    /// valid source must be performed again; exporting and changing a Score cannot reseal it.
    pub fn validate_candidate(&self, score: &Score) -> Result<(), KernelError> {
        validate(score, &self.composition.perf, &self.source, &self.world)?;
        for (i, h) in self.hearings.iter().enumerate() {
            if !h.matches(score) {
                return Err(KernelError::StaleObservation(i));
            }
        }
        let committed = self.score();
        for (role, agent) in [
            (Role::Lead, Agent::Lead),
            (Role::Keys, Agent::Keys),
            (Role::Bass, Agent::Bass),
            (Role::Pad, Agent::Pad),
        ] {
            let signature = |n: &Note| {
                (
                    n.start_beat,
                    n.dur_beats,
                    n.pitch,
                    n.velocity,
                    n.role,
                    n.prov,
                    n.function,
                )
            };
            if !score
                .role_notes(role)
                .map(signature)
                .eq(committed.role_notes(role).map(signature))
            {
                return Err(KernelError::ChangedCommitment(agent));
            }
        }
        let stroke = |h: &DrumHit| (h.start_beat, h.voice, h.velocity, h.prov);
        if !score
            .drums
            .iter()
            .map(stroke)
            .eq(committed.drums.iter().map(stroke))
        {
            return Err(KernelError::ChangedCommitment(Agent::Drums));
        }
        if score.voice_continuity != committed.voice_continuity {
            return Err(KernelError::Source("changed committed voice ownership"));
        }
        Ok(())
    }
}

pub(crate) fn source_harmony(
    argument: &CompiledArgument,
    world: &MusicWorld,
    lang: &super::language::MusicalLanguage,
) -> Result<Vec<ChordSpan>, super::cover::CoverError> {
    let err = || super::cover::CoverError::Invalid("invalid final argument harmony");
    argument.argument.validate().map_err(|_| err())?;
    let end = argument.song.plan.form.total_beats;
    if !end.is_finite()
        || end <= 0.0
        || argument.route.len() != super::form::bars_spanning(end, 4.0) as usize
    {
        return Err(err());
    }
    let region = Scale::new(world.tonic_pc, world.mode);
    let vocabulary = super::vocabulary::HarmonicVocabulary::of(world, lang);
    argument
        .route
        .iter()
        .enumerate()
        .map(|(bar, &(offset, quality))| {
            let chord = Chord::new(region.tonic_pc + offset.rem_euclid(12), quality);
            if !vocabulary.admits(chord) {
                return Err(err());
            }
            Ok(ChordSpan {
                start_beat: bar as f64 * 4.0,
                dur_beats: (end - bar as f64 * 4.0).min(4.0) as f32,
                chord,
                function: super::context::contextual_function(&chord, &region),
                degree: super::backbone::ChartRoot::degree_of_pc(chord.root_pc, &region)
                    .unwrap_or(-1),
                note: "argument-chart",
            })
        })
        .collect()
}

fn action(
    actions: &mut ActionPlan,
    agent: Agent,
    kind: ActionKind,
    at: f64,
    dur: f64,
    call: Option<ActionId>,
    phrase: u32,
) -> ActionId {
    actions.push(MusicalAction {
        id: ActionId(0),
        cause: call.map_or(ActionCause::Statement { phrase }, |call| {
            ActionCause::Interaction { call }
        }),
        initiator: agent,
        start_beat: at,
        dur_beats: dur,
        kind,
        target_beat: None,
        responders: Vec::new(),
        binding: None,
        pays: call,
        effect: EffectVector::NEUTRAL,
    })
}

/// Source statements enter before ensemble/budget planning, not after a generic tune was played.
pub(super) fn source_plan(
    argument: &CompiledArgument,
    actions: &mut ActionPlan,
) -> Result<InteractionPlan, super::cover::CoverError> {
    let mut statements = Vec::new();
    let mut materials = Vec::new();
    for (i, p) in argument.placements.iter().enumerate() {
        let site = argument
            .song
            .thematic
            .sites
            .iter()
            .find(|s| s.phrase == p.phrase)
            .ok_or(super::cover::CoverError::Invalid(
                "source has no matching thematic site",
            ))?;
        let id = MaterialId(materials.len() as u32);
        let length = p.events.iter().map(|e| e.onset + e.dur).fold(0.0, f64::max);
        let call = action(
            actions,
            Agent::Lead,
            ActionKind::Call,
            p.start_beat,
            length,
            None,
            p.phrase,
        );
        materials.push(InteractionMaterial {
            id,
            owner: Agent::Lead,
            source: MaterialSource::Statement {
                statement: i,
                motif: site.motif.id,
            },
            start_beat: p.start_beat,
            events: p.events.clone(),
            pitch_basis: p.pitch_basis,
        });
        statements.push(LeadStatement {
            phrase: p.phrase,
            start_beat: p.start_beat,
            motif: site.motif.clone(),
            handoff: site.handoff,
            role: site.role,
            energy: p.section_gain,
            register: 0.5,
            is_rupture: false,
            call: Some(call),
            material: id,
            answers: None,
            fragment: None,
        });
    }
    Ok(InteractionPlan {
        statements,
        materials,
        interactions: Vec::new(),
        opportunities: Vec::new(),
    })
}

fn provenance(perf: &PerformancePlan, at: f64, label: &'static str) -> Provenance {
    let mut p = Provenance::new(SectionKind::A);
    p.role_note = label;
    p.role_kind = Some(if label == "foundation" {
        "foundation"
    } else {
        "support"
    });
    let _ = (perf, at);
    p
}
fn pitched(
    perf: &PerformancePlan,
    role: Role,
    at: f64,
    dur: f32,
    pitch: i32,
    velocity: f32,
    prov: Provenance,
) -> Note {
    let mut n = Note::new(at, dur, pitch, velocity, role, prov);
    n.function = perf.context_at(at).and_then(|c| {
        c.chord
            .pitch_classes()
            .contains(&pitch.rem_euclid(12))
            .then_some(PitchFunction::ChordTone)
    });
    n
}
fn register(pc: i32, low: i32) -> i32 {
    low + (pc - low).rem_euclid(12)
}

fn source_domain(argument: &CompiledArgument) -> Result<(), KernelError> {
    if argument.song.plan.form.total_beats != 16.0
        || argument.route.len() != 4
        || argument.placements.len() != 2
        || argument.song.thematic.sites.len() != 2
    {
        return Err(KernelError::Source(
            "first kernel domain is two lead statements in four bars",
        ));
    }
    argument
        .argument
        .validate()
        .map_err(|_| KernelError::Source("invalid source argument"))?;
    let [first, last] = argument.placements.as_slice() else {
        unreachable!()
    };
    if first.relation != super::argument::ArgumentRelation::Establish
        || last.relation != (super::argument::ArgumentRelation::Return { source: first.step })
        || argument.argument.ending != super::argument::ArgumentEnding::Resolved
    {
        return Err(KernelError::Source(
            "conversation requires an established source and its resolved return",
        ));
    }
    let arc = argument.argument.arc.as_ref().ok_or(KernelError::Source(
        "conversation requires source-bound reservations",
    ))?;
    if argument.argument.steps.len() != 2 || arc.len() != 2 {
        return Err(KernelError::Source("inconsistent source arc"));
    }
    for ((p, span), step) in argument
        .placements
        .iter()
        .zip(arc)
        .zip(&argument.argument.steps)
    {
        let phrase = argument
            .song
            .plan
            .form
            .phrases
            .get(p.phrase as usize)
            .ok_or(KernelError::Source("foreign source phrase"))?;
        let final_ix = span
            .start_phrase
            .checked_add(span.phrase_count)
            .and_then(|i| i.checked_sub(1))
            .ok_or(KernelError::Source("invalid source span"))?;
        let final_phrase = argument
            .song
            .plan
            .form
            .phrases
            .get(final_ix as usize)
            .ok_or(KernelError::Source("source span exceeds form"))?;
        if span.phrase_count == 0
            || span.step != step.id
            || p.step != step.id
            || span.start_phrase != p.phrase
            || span.start_beat != p.start_beat
            || span.span_beats != p.span_beats
            || phrase.start_beat() != p.start_beat
            || final_phrase.end_beat() != p.start_beat + p.span_beats
        {
            return Err(KernelError::Source(
                "compiled reservation differs from source arc",
            ));
        }
    }

    if first.start_beat != 0.0
        || last.start_beat != 12.0
        || first.span_beats != 12.0
        || last.span_beats != 4.0
    {
        return Err(KernelError::Source(
            "call/breath/return reservations must be [0,12) and [12,16)",
        ));
    }
    for p in &argument.placements {
        if !argument
            .song
            .thematic
            .sites
            .iter()
            .any(|s| *s == p.theme_site())
        {
            return Err(KernelError::Source(
                "thematic projection differs from its source",
            ));
        }
        if p.anchor_semitones != 0
            || !p.section_gain.is_finite()
            || !(0.0 < p.section_gain && p.section_gain <= 1.0)
            || p.carriers != [Agent::Lead]
            || p.pitch_basis != PitchBasis::Semitones
            || p.events.is_empty()
            || p.events.iter().any(|e| {
                e.accent <= 0.0
                    || e.onset < 0.0
                    || e.onset + e.dur > 3.75
                    || (e.onset * 4.0).fract().abs() > 1e-8
            })
        {
            return Err(KernelError::Source(
                "source must fit its own quarter-beat phrase lattice and breath",
            ));
        }
        let step = argument
            .argument
            .steps
            .iter()
            .find(|s| s.id == p.step)
            .ok_or(KernelError::Source("foreign source step"))?;
        let referent = argument
            .argument
            .referents
            .iter()
            .find(|r| r.id == step.referent)
            .ok_or(KernelError::Source("foreign referent"))?;
        if p.referent != step.referent
            || p.relation != step.relation
            || p.carriers != step.carriers
            || step
                .transform
                .apply(referent)
                .map_err(|_| KernelError::Source("invalid source transform"))?
                != p.events
        {
            return Err(KernelError::Source(
                "compiled placement differs from its source",
            ));
        }
    }
    if argument.route[3] != (0, Quality::Min)
        || argument.route[2] != (7, Quality::Dom7)
        || last
            .events
            .last()
            .and_then(|e| e.step)
            .is_none_or(|p| p.rem_euclid(12) != 0)
    {
        return Err(KernelError::Source(
            "return requires a declared dominant-to-tonic route and tonic arrival",
        ));
    }
    Ok(())
}

/// Perform one admitted source-driven conversation. This deliberately bounded first kernel does
/// not claim to replace the full fusion arranger. No note arrays are stored here as a tune.
pub fn perform(
    argument: &CompiledArgument,
    world: &MusicWorld,
) -> Result<CommittedPerformance, KernelError> {
    source_domain(argument)?;
    if !world.tempo_bpm.is_finite() || world.tempo_bpm <= 0.0 {
        return Err(KernelError::Source("invalid shared pulse"));
    }
    let inputs = AdmissionInputs {
        argument: Some(argument.clone()),
        harmony: super::policy::HarmonyPolicy::Vocabulary,
        ..Default::default()
    };
    let mut perf = PerformancePlan::from_song_admitted(
        &argument.song,
        world,
        PerformanceOptions {
            actions: false,
            ..Default::default()
        },
        None,
        &inputs,
    )
    .map_err(|_| KernelError::Source("final source harmony refused"))?;
    super::argument_transport::validate_required_placements(
        &argument.placements,
        world.tonic_pc,
        &perf.contexts,
        &argument.song.plan,
    )
    .map_err(|_| KernelError::Source("source placement is infeasible"))?;
    let mut score = Score::new(world.tempo_bpm, 4.0, 16.0);
    score.chords = perf.chords.clone();
    score.mono_voice = true;
    score.observed_lifetime = Some(super::voice::ObservedLifetimePolicy::ExplicitContinuity);
    let mut lead = Vec::new();
    let mut lead_links = Vec::new();
    // Required source transport into an EMPTY lane: nothing is generated then deleted.
    super::argument_transport::replace_required_role(
        &mut lead,
        Role::Lead,
        &argument.placements,
        world.tonic_pc,
        &perf.contexts,
        &argument.song.plan,
    )
    .map_err(|_| KernelError::Source("source realization refused"))?;
    for (i, p) in argument.placements.iter().enumerate() {
        let notes: Vec<_> = lead
            .iter_mut()
            .filter(|n| n.start_beat >= p.start_beat && n.start_beat < p.start_beat + p.span_beats)
            .collect();
        for n in notes {
            n.prov.material = Some(perf.statements[i].material);
            n.prov.actions = n.prov.actions.with(perf.statements[i].call.unwrap());
        }
    }
    // Explicit links only inside a single declared lead gesture, never across its breath.
    for p in &argument.placements {
        let ns: Vec<_> = lead
            .iter()
            .filter(|n| n.start_beat >= p.start_beat && n.start_beat < p.start_beat + p.span_beats)
            .copied()
            .collect();
        for pair in ns.windows(2) {
            lead_links.push(VoiceContinuation::new(&pair[0], &pair[1]).unwrap());
        }
    }
    score = commit_role(score, &perf, world, Role::Lead, lead, lead_links)?;
    let lead_hearing = EffectiveHearing::capture(Agent::Keys, Role::Lead, 0.0, 4.0, &score);
    let heard: Vec<_> = score
        .role_notes(Role::Lead)
        .filter(|n| n.start_beat < 4.0)
        .copied()
        .collect();
    let call_material = perf.materials[0].clone();
    let call = Call {
        action: perf.statements[0].call.unwrap(),
        initiator: Agent::Lead,
        start_beat: 0.0,
        end_beat: call_material.length(),
        statement: Some(0),
        material: call_material.id,
    };
    // A tonic close stands alone. An open rising line invites a contrary answer; a falling
    // line is quoted. Equal counts and energies cannot determine these distinct choices.
    let closed = heard
        .last()
        .is_some_and(|n| n.pitch.rem_euclid(12) == world.tonic_pc.rem_euclid(12));
    let transform = if closed || !perf.stage.on_stage_span(Agent::Keys, 4.0, 8.0) {
        Transform::Silence
    } else if heard.last().unwrap().pitch > heard[0].pitch {
        Transform::Invert
    } else {
        Transform::Quote
    };
    let keys_at = 4.0625; // one sixteenth of a beat behind the common pulse, never accumulated.
    let mut response = Response {
        action: None,
        responder: Agent::Keys,
        start_beat: keys_at,
        dur_beats: 3.0,
        latency: keys_at - call.end_beat,
        overlap: false,
        transform,
        crosses_chord: false,
        material: None,
        realizes: None,
    };
    let mut keys = Vec::new();
    if let Some(material) = super::material::transform_material(
        MaterialId(perf.materials.len() as u32),
        &call_material,
        transform,
        Agent::Keys,
        keys_at,
        3.0,
    ) {
        let aid = action(
            &mut perf.actions,
            Agent::Keys,
            ActionKind::Answer,
            keys_at,
            material.length(),
            Some(call.action),
            0,
        );
        response.action = Some(aid);
        response.material = Some(material.id);
        let anchor = register(perf.context_at(keys_at).unwrap().chord.root_pc, 60);
        let velocity = heard.iter().map(|n| n.velocity).fold(0.0, f32::max) * 0.78;
        for (at, dur, pitch, accent) in
            super::material::project_pitched(&material, keys_at, 7.5, anchor, &perf, &|ctx, pc| {
                ctx.chord.pitch_classes().contains(&pc)
            })
        {
            let mut prov = provenance(&perf, at, "answer").realizing(aid);
            prov.material = Some(material.id);
            prov.interaction = Some(InteractionId(0));
            keys.push(pitched(
                &perf,
                Role::Keys,
                at,
                dur as f32 * 0.8,
                pitch.unwrap(),
                velocity * accent,
                prov,
            ));
        }
        perf.materials.push(material);
    }
    perf.interactions.push(Interaction {
        id: InteractionId(0),
        call,
        response: Some(response),
    });
    // Memory: repeating the just-made answer would obscure the prepared return. The second
    // opportunity is deliberately withheld even while Stage permits the keyboard to play.
    perf.interactions.push(Interaction {
        id: InteractionId(1),
        call,
        response: Some(Response {
            start_beat: 8.0,
            dur_beats: 4.0,
            latency: 8.0 - call.end_beat,
            action: None,
            material: None,
            transform: Transform::Silence,
            ..response
        }),
    });
    score = commit_role(score, &perf, world, Role::Keys, keys, Vec::new())?;
    let keys_hearing = EffectiveHearing::capture(Agent::Drums, Role::Keys, 4.0, 8.0, &score);
    // The bassist owns one uninterrupted foundation lane. The optional approach is planned
    // against the next source chord, not spliced into an already sounded melody.
    let mut bass = Vec::new();
    for bar in 0..4 {
        let at = bar as f64 * 4.0;
        let ctx = perf.context_at(at).unwrap();
        let root = register(ctx.chord.root_pc, 36);
        let tones = ctx.chord.pitch_classes();
        for (offset, pitch, velocity) in [
            (0.0, root, 0.76),
            (
                2.0,
                register(tones[if bar % 2 == 0 { 1 } else { tones.len() - 1 }], 36),
                0.57,
            ),
        ] {
            let prov = provenance(&perf, at + offset, "foundation");
            bass.push(pitched(
                &perf,
                Role::Bass,
                at + offset,
                if offset == 0.0 { 1.25 } else { 0.8 },
                pitch,
                velocity,
                prov,
            ));
        }
    }
    let arrival = register(perf.context_at(12.0).unwrap().chord.root_pc, 36);
    if arrival > 36 && perf.stage.on_stage_span(Agent::Bass, 11.5, 12.5) {
        let id = action(
            &mut perf.actions,
            Agent::Bass,
            ActionKind::Pickup,
            11.5,
            0.5,
            None,
            0,
        );
        perf.actions.actions[id.0 as usize].target_beat = Some(12.0);
        let mut note = pitched(
            &perf,
            Role::Bass,
            11.5,
            0.3,
            arrival - 1,
            0.62,
            provenance(&perf, 11.5, "approach").realizing(id),
        );
        note.function = Some(PitchFunction::ChromaticApproach);
        bass.push(note);
    }
    bass.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    let bass_links = bass
        .windows(2)
        .map(|pair| VoiceContinuation::new(&pair[0], &pair[1]).unwrap())
        .collect();
    score = commit_role(score, &perf, world, Role::Bass, bass, bass_links)?;
    // Kit anchors own the shared beat. Only the later fill consumes the completed keys window.
    for step in 0..32 {
        let at = step as f64 * 0.5;
        for (play, voice, velocity) in [
            (
                true,
                DrumVoice::ClosedHat,
                if step % 2 == 0 { 0.38 } else { 0.25 },
            ),
            (step % 8 == 0 || step % 8 == 4, DrumVoice::Kick, 0.78),
            (step % 8 == 2 || step % 8 == 6, DrumVoice::Snare, 0.65),
        ] {
            if play {
                score.drums.push(DrumHit {
                    start_beat: at,
                    voice,
                    velocity,
                    prov: provenance(&perf, at, "pulse"),
                });
            }
        }
    }
    if keys_hearing.event_count() > 0 {
        let keys: Vec<_> = score
            .role_notes(Role::Keys)
            .filter(|n| n.start_beat < 8.0)
            .copied()
            .collect();
        let aid = action(
            &mut perf.actions,
            Agent::Drums,
            ActionKind::Fill,
            10.5,
            1.0,
            response.action,
            0,
        );
        // A rhythm echo of the actual final two attacks, bounded before the bass approach.
        let tail = &keys[keys.len().saturating_sub(2)..];
        let first = tail[0].start_beat;
        for n in tail {
            let at = 10.5 + (n.start_beat - first) * 0.5;
            if at < 11.5 {
                score.drums.push(DrumHit {
                    start_beat: at,
                    voice: DrumVoice::Snare,
                    velocity: n.velocity * 0.65,
                    prov: provenance(&perf, at, "heard-keys").realizing(aid),
                });
            }
        }
    }
    // Recompute the existing texture cap for the completed decisions; it is a readout, not
    // permission to overwrite source events or an estimator of musical meaning.
    perf.budget = super::budget::allocate(&perf);
    score
        .drums
        .sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    let take = CommittedPerformance {
        composition: Composition {
            score,
            perf,
            song: argument.song.clone(),
        },
        hearings: vec![lead_hearing, keys_hearing],
        source: argument.clone(),
        world: world.clone(),
    };
    take.validate_candidate(take.score())?;
    Ok(take)
}

// The partial union consumes a completed lane once. No role can revise an earlier lane or
// borrow its continuity endpoints; errors yield no partially committed performance.
fn commit_role(
    mut score: Score,
    perf: &PerformancePlan,
    world: &MusicWorld,
    role: Role,
    notes: Vec<Note>,
    links: Vec<VoiceContinuation>,
) -> Result<Score, KernelError> {
    if score.role_notes(role).next().is_some()
        || notes.iter().any(|n| n.role != role)
        || links.iter().any(|e| {
            !notes.iter().any(|n| VoiceEventId::of(n) == e.from)
                || !notes.iter().any(|n| VoiceEventId::of(n) == e.to)
        })
    {
        return Err(KernelError::Capacity(role));
    }
    score.notes.extend(notes);
    score.voice_continuity.extend(links);
    validate_role(&score, perf, role, world)?;
    Ok(score)
}

// Partial composition: the source lane must inhabit the shared domain and stage before any
// dependent can consume it. Independent role identity is retained; no vertical consonance solver.
fn validate_role(
    score: &Score,
    perf: &PerformancePlan,
    role: Role,
    world: &MusicWorld,
) -> Result<(), KernelError> {
    let agent = match role {
        Role::Lead => Agent::Lead,
        Role::Keys => Agent::Keys,
        Role::Bass => Agent::Bass,
        Role::Pad => Agent::Pad,
    };
    for n in score.role_notes(role) {
        if !n.start_beat.is_finite()
            || !n.dur_beats.is_finite()
            || n.dur_beats <= 0.0
            || !n.velocity.is_finite()
            || !(0.0..=1.0).contains(&n.velocity)
            || !(0..=127).contains(&n.pitch)
            || !perf
                .domain()
                .contains(n.start_beat, n.start_beat + f64::from(n.dur_beats))
        {
            return Err(KernelError::InvalidScore(
                "event outside the performance domain".into(),
            ));
        }
        if !perf
            .stage
            .on_stage_span(agent, n.start_beat, n.start_beat + f64::from(n.dur_beats))
        {
            return Err(KernelError::Stage(agent));
        }
    }
    if matches!(role, Role::Lead | Role::Bass) {
        validate_lane(score, role, world)?;
    }
    Ok(())
}

fn validate_lane(score: &Score, role: Role, world: &MusicWorld) -> Result<(), KernelError> {
    let mut notes: Vec<_> = score.role_notes(role).collect();
    notes.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    if notes
        .iter()
        .any(|n| !n.dur_beats.is_finite() || (role == Role::Bass && !(36..=55).contains(&n.pitch)))
        || notes.windows(2).any(|w| {
            w[0].start_beat + f64::from(w[0].dur_beats) > w[1].start_beat + 1e-6
                || (w[0].pitch == w[1].pitch && w[1].start_beat - w[0].start_beat < 0.5)
                || {
                    let linked = score.voice_continuity.iter().any(|e| {
                        e.from == VoiceEventId::of(w[0]) && e.to == VoiceEventId::of(w[1])
                    });
                    let choke = if linked {
                        super::voice::MONO_CHOKE_SECS * f64::from(world.tempo_bpm) / 60.0
                    } else {
                        0.0
                    };
                    super::voice::effective_audible_end(w[0], world, &score.voice_continuity)
                        > w[1].start_beat + choke + 1e-6
                }
        })
    {
        return Err(KernelError::Capacity(role));
    }
    Ok(())
}
fn validate(
    score: &Score,
    perf: &PerformancePlan,
    source: &CompiledArgument,
    world: &MusicWorld,
) -> Result<(), KernelError> {
    score.validate().map_err(KernelError::InvalidScore)?;
    if score.tempo_bpm != world.tempo_bpm
        || score.total_beats != 16.0
        || score.beats_per_bar != 4.0
        || score.observed_lifetime != Some(super::voice::ObservedLifetimePolicy::ExplicitContinuity)
        || !score.mono_voice
        || !score.sfx.is_empty()
    {
        return Err(KernelError::Source("changed performance domain"));
    }
    let chord = |c: &ChordSpan| (c.start_beat, c.dur_beats, c.chord, c.function, c.degree);
    if !score
        .chords
        .iter()
        .map(chord)
        .eq(perf.chords.iter().map(chord))
    {
        return Err(KernelError::Source("changed final harmonic base"));
    }
    if !super::argument_witness::observe_argument(source, score, world.tonic_pc).valid() {
        return Err(KernelError::Source(
            "source witness refused realized content",
        ));
    }
    if !super::voice::continuity_violations(&score.notes, &score.voice_continuity).is_empty() {
        return Err(KernelError::Capacity(Role::Bass));
    }
    for role in [Role::Lead, Role::Keys, Role::Bass, Role::Pad] {
        validate_role(score, perf, role, world)?;
    }
    for n in &score.notes {
        let agent = match n.role {
            Role::Lead => Agent::Lead,
            Role::Keys => Agent::Keys,
            Role::Bass => Agent::Bass,
            Role::Pad => Agent::Pad,
        };
        if !perf
            .stage
            .on_stage_span(agent, n.start_beat, n.start_beat + f64::from(n.dur_beats))
        {
            return Err(KernelError::Stage(agent));
        }
        if n.role == Role::Keys && !(4.0..8.0).contains(&n.start_beat) {
            return Err(KernelError::Source("violated deliberate keyboard silence"));
        }
    }
    for bar in 0..4 {
        let at = bar as f64 * 4.0;
        let root = perf.context_at(at).unwrap().chord.root_pc;
        if !score
            .role_notes(Role::Bass)
            .any(|n| n.start_beat == at && n.pitch.rem_euclid(12) == root)
        {
            return Err(KernelError::Source("missing harmonic foundation"));
        }
    }
    for h in &score.drums {
        if !perf.on_stage(Agent::Drums, h.start_beat) {
            return Err(KernelError::Stage(Agent::Drums));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::human_music::{
        argument::{ArgumentFamily, MusicalArgument},
        composer::Composer,
        contract::CompositionGrammar,
        semantic::deflected_lift_trace,
        SongMap,
    };

    fn source() -> CompiledArgument {
        let song = SongMap::compose(
            &deflected_lift_trace(128.0),
            2112,
            Some(CompositionGrammar::DeflectedLift),
            Composer::MeaningDirected,
        );
        MusicalArgument::calibration(ArgumentFamily::CallAndEarnedAnswer, 91)
            .unwrap()
            .compile(&song)
            .unwrap()
    }
    #[test]
    fn source_harmony_precedes_the_first_pitch_sensitive_settlement() {
        let mut source = source();
        // In the archived planner this debt chose beat120 against the intermediate A chord.
        // The source instead declares F there, then home at124: only the latter can pay it.
        source.route[30] = (8, Quality::Maj);
        source.route[31] = (0, Quality::Min);
        let world = MusicWorld::black_ice();
        let inputs = AdmissionInputs {
            argument: Some(source.clone()),
            rehearsed: true,
            ..Default::default()
        };
        let perf = PerformancePlan::from_song_admitted(
            &source.song,
            &world,
            PerformanceOptions::default(),
            None,
            &inputs,
        )
        .unwrap();
        let resolves: Vec<_> = perf
            .actions
            .of_kind(ActionKind::Resolve)
            .filter(|a| matches!(a.cause, ActionCause::Discourse { .. }))
            .collect();
        assert!(!resolves.is_empty());
        assert!(resolves.iter().all(|a| perf
            .context_at(a.target_beat.unwrap())
            .unwrap()
            .chord
            .root_pc
            == world.tonic_pc));
        assert!(!resolves.iter().any(|a| a.target_beat == Some(120.0)));
        assert!(resolves.iter().any(|a| a.target_beat == Some(124.0)));
    }
    #[test]
    fn frozen_planner_refuses_foreign_owner_and_missing_route() {
        let source = source();
        let world = MusicWorld::black_ice();
        let mut foreign = source.song.clone();
        foreign.seed += 1;
        let inputs = AdmissionInputs {
            argument: Some(source.clone()),
            ..Default::default()
        };
        assert!(PerformancePlan::from_song_admitted(
            &foreign,
            &world,
            PerformanceOptions::default(),
            None,
            &inputs
        )
        .is_err());
        let mut broken = source.clone();
        broken.route.pop();
        assert!(source_harmony(&broken, &world, &Default::default()).is_err());
    }
}
