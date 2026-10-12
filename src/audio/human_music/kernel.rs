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
    pub fn validate_candidate(&self, score: &Score) -> Result<(), KernelError> {
        validate(score, &self.composition.perf, &self.source, &self.world)?;
        for (i, h) in self.hearings.iter().enumerate() {
            if !h.matches(score) {
                return Err(KernelError::StaleObservation(i));
            }
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
            let chord = Chord::new(region.tonic_pc + offset, quality);
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
) -> InteractionPlan {
    let mut statements = Vec::new();
    let mut materials = Vec::new();
    for (i, p) in argument.placements.iter().enumerate() {
        let site = &argument.song.thematic.sites[i];
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
    InteractionPlan {
        statements,
        materials,
        interactions: Vec::new(),
        opportunities: Vec::new(),
    }
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
        if p.carriers != [Agent::Lead]
            || p.pitch_basis != PitchBasis::Semitones
            || p.events.is_empty()
            || p.events.iter().any(|e| {
                e.onset < 0.0 || e.onset + e.dur > 3.75 || (e.onset * 4.0).fract().abs() > 1e-8
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
    // Required source transport into an EMPTY lane: nothing is generated then deleted.
    super::argument_transport::replace_required_role(
        &mut score.notes,
        Role::Lead,
        &argument.placements,
        world.tonic_pc,
        &perf.contexts,
        &argument.song.plan,
    )
    .map_err(|_| KernelError::Source("source realization refused"))?;
    for (i, p) in argument.placements.iter().enumerate() {
        let notes: Vec<_> = score
            .notes
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
        let ns: Vec<_> = score
            .role_notes(Role::Lead)
            .filter(|n| n.start_beat >= p.start_beat && n.start_beat < p.start_beat + p.span_beats)
            .copied()
            .collect();
        for pair in ns.windows(2) {
            score
                .voice_continuity
                .push(VoiceContinuation::new(&pair[0], &pair[1]).unwrap());
        }
    }
    validate_lane(&score, Role::Lead)?;
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
            score.notes.push(pitched(
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
    let keys_hearing = EffectiveHearing::capture(Agent::Drums, Role::Keys, 4.0, 8.0, &score);
    // The bassist owns one uninterrupted foundation lane. The optional approach is planned
    // against the next source chord, not spliced into an already sounded melody.
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
            score.notes.push(pitched(
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
        score.notes.push(note);
    }
    let mut bass: Vec<_> = score.role_notes(Role::Bass).copied().collect();
    bass.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    for pair in bass.windows(2) {
        score
            .voice_continuity
            .push(VoiceContinuation::new(&pair[0], &pair[1]).unwrap());
    }
    validate_lane(&score, Role::Bass)?;
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

fn validate_lane(score: &Score, role: Role) -> Result<(), KernelError> {
    let mut notes: Vec<_> = score.role_notes(role).collect();
    notes.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    if notes
        .iter()
        .any(|n| !n.dur_beats.is_finite() || (role == Role::Bass && !(36..=55).contains(&n.pitch)))
        || notes.windows(2).any(|w| {
            w[0].start_beat + f64::from(w[0].dur_beats) > w[1].start_beat + 1e-6
                || (w[0].pitch == w[1].pitch && w[1].start_beat - w[0].start_beat < 0.5)
                || !score
                    .voice_continuity
                    .iter()
                    .any(|e| e.from == VoiceEventId::of(w[0]) && e.to == VoiceEventId::of(w[1]))
                    && w[1].start_beat - (w[0].start_beat + f64::from(w[0].dur_beats)) < 1.0
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
        || !score.mono_voice
        || !score.sfx.is_empty()
    {
        return Err(KernelError::Source("changed performance domain"));
    }
    if !super::argument_witness::observe_argument(source, score, world.tonic_pc).valid() {
        return Err(KernelError::Source(
            "source witness refused realized content",
        ));
    }
    if !super::voice::continuity_violations(&score.notes, &score.voice_continuity).is_empty() {
        return Err(KernelError::Capacity(Role::Bass));
    }
    validate_lane(score, Role::Bass)?;
    validate_lane(score, Role::Lead)?;
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
