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
use super::occupancy::AuthoredOccupancy;
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

fn provenance(
    perf: &PerformancePlan,
    at: f64,
    label: &'static str,
    section: SectionKind,
) -> Provenance {
    let mut p = Provenance::new(section);
    p.role_note = label;
    p.role_kind = Some(if label == "foundation" {
        "foundation"
    } else {
        "support"
    });
    let _ = (perf, at);
    p
}

/// The section a long-form event at `at` belongs to, derived from the SAME source the lead carrier
/// reads — the placement covering the beat, via its phrase family — so the whole band agrees on
/// which section it is playing instead of the accompaniment hardcoding `A`. Falls back to `A` only
/// if no placement covers the beat (there is always one on the declared arc).
fn section_at(argument: &CompiledArgument, at: f64) -> SectionKind {
    argument
        .placements
        .iter()
        .find(|p| at >= p.start_beat && at < p.start_beat + p.span_beats)
        .map(|p| {
            argument.song.plan.form.phrases[p.phrase as usize]
                .family
                .to_section_kind()
        })
        .unwrap_or(SectionKind::A)
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
        // Truthful overlap (J3 repair): true iff the answer starts before the call ends. See the
        // longform site and `Knowledge` for the causal argument.
        overlap: keys_at < call.end_beat,
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
            let mut prov = provenance(&perf, at, "answer", SectionKind::A).realizing(aid);
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
            let prov = provenance(&perf, at + offset, "foundation", SectionKind::A);
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
            provenance(&perf, 11.5, "approach", SectionKind::A).realizing(id),
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
                    prov: provenance(&perf, at, "pulse", SectionKind::A),
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
                    prov: provenance(&perf, at, "heard-keys", SectionKind::A).realizing(aid),
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

// =================================================================================================
// Long-form composer (Stage A): N causally-connected episodes (N >= 2) in ONE continuous Score.
//
// This is a PARALLEL path to `perform`. It reuses the frozen private helpers (`commit_role`,
// `validate_role`, `validate_lane`, `provenance`, `pitched`, `register`, `action`, the transport)
// without altering any of them, and never widens the four-bar `perform`'s own length guard. Where
// `perform` plays one call/breath/return in four bars, `perform_song` lays a sequence of 16-beat
// episodes — the two-episode verse-open → hook-home control, and the four-episode verse → hook →
// bridge-departs → hook-returns arc — threading a BandMemory across every seam so each episode's
// keys decision depends on whether the earlier call it carries was answered.
// =================================================================================================

/// What the band carries from one episode into the next. The keys decision is the only consumer:
/// an episode whose real call went unanswered leaves `open_call` set, and the next episode
/// discharges that debt (source-bound) instead of making its own base decision.
#[derive(Default)]
struct BandMemory {
    open_call: bool,
    prev_answer_events: usize,
}

/// A committed long-form take. Mirrors [`CommittedPerformance`] but over an N-episode (N ≥ 2)
/// domain: keys are admitted inside ANY episode's answer window, and the harmonic foundation is
/// checked in every bar. Editing an exported Score yields a candidate that must be judged again;
/// it cannot mutate what a dependent actually consumed.
pub struct CommittedSong {
    composition: Composition,
    hearings: Vec<EffectiveHearing>,
    source: CompiledArgument,
    world: MusicWorld,
}
impl CommittedSong {
    pub fn composition(&self) -> &Composition {
        &self.composition
    }
    pub fn score(&self) -> &Score {
        &self.composition.score
    }
    pub fn hearings(&self) -> &[EffectiveHearing] {
        &self.hearings
    }
    /// Judge exported mutations without blessing or modifying them — the same event/source contract
    /// as [`CommittedPerformance::validate_candidate`], generalized to the N-episode domain.
    pub fn validate_candidate(&self, score: &Score) -> Result<(), KernelError> {
        validate_song(score, &self.composition.perf, &self.source, &self.world)?;
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

/// How many beats one episode owns. The long-form domain is an integer number of these.
const EPISODE_BEATS: f64 = 16.0;

/// The maximum a source onset may sit off its sixteenth-grid lattice position and still be admitted
/// — the budget for a STRUCTURED performance pocket (a melody laid a few milliseconds behind the
/// beat), never random jitter. At 144 BPM, 0.06 beat ≈ 25 ms. Bounds the deliberate relaxation of
/// the long-form grid guard so a wild onset still fails.
const LONGFORM_POCKET_BEATS: f64 = 0.06;

/// Felt-attack compensation (beats) for the riff-rhythm keys comp only. The warm keys voice speaks a
/// hair late (a soft FM e-piano onset through the chorus + room), so its FELT attack lags the written
/// onset; a RHYTHMIC chordal comp that must lock to the kit is therefore pulled this much EARLIER, so
/// the heard attack lands dead on the band's shared pocket. ~0.03 beat ≈ 17 ms at 104 BPM. Ear-tunable;
/// applied ONLY on the opt-in `comp_onsets` path, so the fixed comp and C137 are untouched.
const KEYS_ATTACK_COMP: f64 = 0.03;

/// Register ceiling (MIDI) for the riff-rhythm keys comp only. This is a LEAD-FREE band, so the keys
/// are the one melodic voice: they carry the sparse theme up top AND comp the chords. A comp whose
/// coloured top (the 7th, and the 9th on the strong hit) climbs to C5–E5 pokes INTO the theme's own
/// register and reads as a second, unresolved tune fighting the first — the "funny" that a real
/// keyboardist avoids by comping BELOW their own right hand. If a stab's top would break above this
/// ceiling, the whole voicing drops an octave (shape and colour intact) so it stays a cushion under
/// the theme. B4: the theme owns C5 and up. Applied ONLY on the opt-in `comp_onsets` path.
const COMP_CEILING: i32 = 71;

/// Generalize `source_domain` to N contiguous episodes (N ≥ 2: the two-episode verse→hook control,
/// and the four-episode verse→hook→bridge→return arc). The per-placement source checks are the same
/// source-fidelity guards, with the four-event CELL constraints dropped (the long-form carrier is a
/// complete generated phrase, not a one-bar prefix): the `onset + dur > 3.75` breath cap and the
/// single-bar lattice no longer apply, but the quarter-beat grid, the finite bounded section dynamic,
/// the non-empty lead-only carriage and the exact source transform still do. The domain is an integer
/// number of 16-beat episodes that each a declared placement owns in order; the SONG must establish,
/// close `Resolved`, and its route must land home on the tonic — the relation lawfulness between
/// episodes (consequent kinship, the departure law, the return's same-head) is the argument's own
/// contract (`validate`) and the content witness, not re-litigated here.
fn longform_domain(argument: &CompiledArgument) -> Result<(), KernelError> {
    let total = argument.song.plan.form.total_beats;
    let episodes = (total / EPISODE_BEATS).round() as usize;
    if episodes < 2
        || total != episodes as f64 * EPISODE_BEATS
        || argument.route.len() != super::form::bars_spanning(total, 4.0) as usize
        || argument.placements.len() != episodes
        || argument.song.thematic.sites.len() != episodes
    {
        return Err(KernelError::Source(
            "long-form domain is N contiguous sixteen-beat episodes (N >= 2)",
        ));
    }
    argument
        .argument
        .validate()
        .map_err(|_| KernelError::Source("invalid source argument"))?;
    use super::argument::ArgumentRelation;
    if argument.placements[0].relation != ArgumentRelation::Establish
        || argument.argument.ending != super::argument::ArgumentEnding::Resolved
    {
        return Err(KernelError::Source(
            "long-form must establish its first episode and close resolved",
        ));
    }
    for (e, p) in argument.placements.iter().enumerate() {
        // Episodes tile the song in order, each a whole sixteen-beat reservation.
        if p.start_beat != e as f64 * EPISODE_BEATS || p.span_beats != EPISODE_BEATS {
            return Err(KernelError::Source(
                "episode reservations must tile the song in sixteen-beat blocks",
            ));
        }
        // Only the long-form arc vocabulary is admitted (establish / consequent / depart / return);
        // question/answer/denial/develop/reconcile belong to the shorter conversation kernels.
        if !matches!(
            p.relation,
            ArgumentRelation::Establish
                | ArgumentRelation::Consequent { .. }
                | ArgumentRelation::Depart { .. }
                | ArgumentRelation::Return { .. }
        ) {
            return Err(KernelError::Source(
                "long-form admits only establish/consequent/depart/return",
            ));
        }
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
        // The LEAD carries every episode by default. The lawful exceptions are a KEYS-carried DEPART
        // (the bridge handoff: the lead withholds, the keys own the departure) and a KEYS-carried
        // ESTABLISH (the intro foreshadow: the keys state the germ on the way in, the lead claims it
        // at the next section). No other carrier, and never under a home-arrival Consequent or the
        // earned-return, which depend on the lead carrying. Split out of the bundled guard below so
        // the failure names the real cause, not the lattice tolerance.
        let carrier_ok = p.carriers == [Agent::Lead]
            || (p.carriers == [Agent::Keys]
                && matches!(
                    p.relation,
                    ArgumentRelation::Depart { .. } | ArgumentRelation::Establish
                ));
        if !carrier_ok {
            return Err(KernelError::Source(
                "long-form carriers must be [Lead], or [Keys] on a Depart (bridge) / Establish (intro)",
            ));
        }
        if p.anchor_semitones != 0
            || !p.section_gain.is_finite()
            || !(0.0 < p.section_gain && p.section_gain <= 1.0)
            || p.pitch_basis != PitchBasis::Semitones
            || p.events.is_empty()
            || p.events.iter().any(|e| {
                // The metric lattice is still the quarter-beat (sixteenth) grid, but a source may
                // now carry a bounded STRUCTURED pocket — a melody laid a few milliseconds behind
                // the grid — so an onset is admitted if it sits within `LONGFORM_POCKET_BEATS` of a
                // lattice position. This is a deliberate relaxation (a named tolerance), not the
                // removal of the grid: a wild or random onset still fails.
                let lattice = (e.onset * 4.0).round() / 4.0;
                e.accent <= 0.0
                    || e.onset < 0.0
                    || (e.onset - lattice).abs() > LONGFORM_POCKET_BEATS
            })
        {
            return Err(KernelError::Source(
                "source must fit its quarter-beat lattice within the pocket tolerance",
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
    // The harmonic closure law is the ONE place C137 and smooth-pop legitimately differ, so it is
    // the only part of long-form admission that is domain-specific. Everything above (shape,
    // validate, relation vocabulary, carriers, lattice, source-transform equality) is shared and
    // unchanged. C137 demands a literal-tonic arrival; a pop loop closes as a repeating groove-cycle.
    match &argument.domain {
        super::argument::SourceDomain::C137 => c137_route_law(&argument.route),
        super::argument::SourceDomain::SmoothPop(grammar) => {
            pop_route_law(&argument.route, grammar)
        }
    }
}

/// The C137 arc's harmonic closure law (lifted verbatim from the original long-form domain): the
/// verse travels pre-dominant → dominant → tonic and hangs open on its own cadence; the first hook
/// lands home on the tonic at bar 7; and the SONG lands home on the tonic at its final bar (the
/// Resolved ending's earned home-coming). Region-relative, reconstructed from the route.
fn c137_route_law(route: &[(i32, Quality)]) -> Result<(), KernelError> {
    if route[2] != (7, Quality::Dom7)
        || route[3] != (0, Quality::Min)
        || route[7] != (0, Quality::Min)
        || route[route.len() - 1] != (0, Quality::Min)
    {
        return Err(KernelError::Source(
            "route must travel to its dominant and land the hook home",
        ));
    }
    Ok(())
}

/// The smooth-pop loop's harmonic closure law: the route is a functional cycle of `grammar.loop_bars`
/// chords, repeated to fill the form, that closes as a GROOVE-CYCLE — it returns to its own first
/// chord every `loop_bars` — instead of arriving on a literal tonic. This is the executable
/// distinction between groove-cycle closure and the tonic/harmonic closure the C137 law demands: a
/// route that is not a whole number of exact loop repetitions (a progression that silently drifts)
/// is refused. The per-chord in-vocabulary check stays upstream in `source_harmony`.
fn pop_route_law(
    route: &[(i32, Quality)],
    grammar: &super::argument::PopSourceGrammar,
) -> Result<(), KernelError> {
    let k = grammar.loop_bars;
    if k == 0 || route.is_empty() || route.len() % k != 0 {
        return Err(KernelError::Source(
            "smooth-pop route length must be a whole number of loop cycles",
        ));
    }
    for (i, chord) in route.iter().enumerate() {
        if *chord != route[i % k] {
            return Err(KernelError::Source(
                "smooth-pop route must repeat its loop cycle exactly (groove-cycle closure)",
            ));
        }
    }
    Ok(())
}

/// Perform one admitted source-driven long-form song: N causally-connected 16-beat episodes (N ≥ 2)
/// in one continuous Score. The lead carries each episode's declared section — a full VERSE (open), a
/// distinct HOOK (home), and, in the four-episode arc, a BRIDGE that departs and a hook that RETURNS;
/// the band answers per episode under the memory law; the pulse is one shared pocket across the seams.
/// How hard the song's OWN harmony is pushing at a given bar — read from the committed per-bar
/// chord, never an external energy fader. This is the endogenous "semantic situation" the whole
/// ensemble allocates its density around: the tonic is the point of rest where the band opens up
/// and lets the chord breathe; a dominant is the point of maximum drive; the harmony in between
/// builds. Bass figure, ride subdivision, comp budget and the shared dynamic all follow it, so the
/// band rises and falls together on the song's energy instead of each voice running flat-out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HarmonicDrive {
    /// On the tonic — harmonic rest. The band anchors and breathes.
    Home,
    /// Pre-dominant / passing harmony — the band walks and rides, carrying motion.
    Building,
    /// A dominant — maximum tension. The band leans in hardest toward the resolution.
    Driving,
}

fn harmonic_drive(chord: &Chord, tonic_pc: i32) -> HarmonicDrive {
    let degree = (chord.root_pc - tonic_pc).rem_euclid(12);
    // Read the dominant FUNCTION before the tonic-rest shortcut: a dominant seventh/ninth is an
    // ACTIVE sound wherever its root sits — even on the tonic pitch class, where it is a secondary
    // dominant (A7 = V7/iv) preparing a move away from home, not tonic rest. Checking `degree == 0`
    // first (the old order) mistook that pivot for Home. The current Am verse/hook has no tonic-root
    // dominant, so this reordering is identical on today's route (its only A-root chord is Am=Min).
    if matches!(chord.quality, Quality::Dom7 | Quality::Dom9) || degree == 7 {
        HarmonicDrive::Driving
    } else if degree == 0 {
        HarmonicDrive::Home
    } else {
        HarmonicDrive::Building
    }
}

/// The shared ensemble dynamic for a bar's harmonic drive: eased at home, leaning in on the
/// dominant. One curve every voice rides, so the whole band breathes together.
fn drive_energy(drive: HarmonicDrive) -> f32 {
    match drive {
        HarmonicDrive::Home => 0.82,
        HarmonicDrive::Building => 0.92,
        HarmonicDrive::Driving => 1.0,
    }
}

/// A gentle within-phrase crescendo: the band enters each episode a touch softer and leans in toward
/// its cadence, so repeated bars of the same harmony still differ by where they sit in the phrase.
/// A reasoned contour (a fixed function of phrase position), riding on top of the harmonic energy.
fn groove_swell(bar: usize, bars_per_episode: usize) -> f32 {
    let in_episode = (bar % bars_per_episode) as f32 / bars_per_episode as f32;
    0.94 + 0.12 * in_episode
}

/// If `bar` falls in the opening INTRO episode of a build-enabled argument, its index WITHIN that
/// episode (`0..bars_per_episode`); otherwise `None`. The intro is placement 0 and ONLY when it is an
/// `Establish` — so the interior verses (also `Establish`) are excluded — which is why this keys on the
/// bar's episode index, not on the relation alone. Drives the intro build: the rhythm section enters
/// reduced and swells into the first verse instead of arriving whole.
fn intro_build_bar(
    argument: &CompiledArgument,
    bar: usize,
    bars_per_episode: usize,
) -> Option<usize> {
    (argument.intro_build
        && bar < bars_per_episode
        && matches!(
            argument.placements[0].relation,
            super::argument::ArgumentRelation::Establish
        ))
    .then_some(bar)
}

/// The intro build's velocity, by intro bar `i` and whether the stroke is part of the SKELETON (the
/// downbeat kick, the backbeat, the bass root) or the MOTION/TEXTURE around it (hats, the bass walk,
/// comps, the hand-off fill). The skeleton barely dips — the pocket is solid from the top — while the
/// texture washes in on a back-loaded, ACCELERATING swell (steps widen toward the verse). Decoupling
/// the two and curving the swell is the point: the energy rises because the arrangement fills and the
/// cymbals open, NOT because one fader is turned up evenly — so it reads as a band leaning in, not a
/// knob slowly turning.
fn intro_build_gain(i: usize, skeleton: bool) -> f32 {
    const SKELETON: [f32; 4] = [0.80, 0.90, 0.96, 1.0];
    const SWELL: [f32; 4] = [0.30, 0.46, 0.72, 1.0];
    let curve = if skeleton { SKELETON } else { SWELL };
    curve[i.min(curve.len() - 1)]
}

/// Whether the lead is genuinely SILENT at `beat` — no lead note sounding across it (within a small
/// margin). A breath an expressive gate opens reads as silent here; a legato line never does. Reads
/// the lead only, so a drummer answering into these silences never perturbs the keys-independent
/// pulse. (A sustained note's whole [onset, onset+gate] span counts as sounding, not just its attack.)
fn lead_silent_at(beat: f64, lead: &[Note]) -> bool {
    const M: f64 = 0.08;
    !lead
        .iter()
        .any(|n| beat >= n.start_beat - M && beat <= n.start_beat + f64::from(n.dur_beats) + M)
}

pub fn perform_song(
    argument: &CompiledArgument,
    world: &MusicWorld,
) -> Result<CommittedSong, KernelError> {
    longform_domain(argument)?;
    if !world.tempo_bpm.is_finite() || world.tempo_bpm <= 0.0 {
        return Err(KernelError::Source("invalid shared pulse"));
    }
    let episodes = (argument.song.plan.form.total_beats / EPISODE_BEATS).round() as usize;
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
    let mut score = Score::new(world.tempo_bpm, 4.0, argument.song.plan.form.total_beats);
    score.chords = perf.chords.clone();
    score.mono_voice = true;
    score.observed_lifetime = Some(super::voice::ObservedLifetimePolicy::ExplicitContinuity);
    // The realized form: one section per declared placement (verse, hook, …), keyed to the same
    // phrase families the lead carrier and every accompaniment event read via `section_at`, so the
    // Score, the lead and the band all name the same regions. Populating this changes the SYMBOLIC
    // Score fingerprint (the section metadata is now truthful) but not a single PCM sample — the
    // synth never reads provenance, so the rendered audio is byte-identical.
    score.sections = argument
        .placements
        .iter()
        .map(|p| {
            let phrase = &argument.song.plan.form.phrases[p.phrase as usize];
            super::form::Section {
                kind: phrase.family.to_section_kind(),
                start_bar: (p.start_beat / 4.0) as u32,
                bars: (p.span_beats / 4.0).round() as u32,
                energy: phrase.span.peak_energy.energy,
                tension: phrase.span.peak_tension.tension,
                density: phrase.intent.density,
            }
        })
        .collect();

    // --- LEAD: carry every episode's required source into an EMPTY lane. ---
    let mut lead = Vec::new();
    let mut lead_links = Vec::new();
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
    // Explicit links inside each declared episode gesture…
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
    // …and ONE legato seam per episode boundary. The long-form lead is continuous — the verse runs
    // straight into the hook with no breath — so its final sustained cadence note and the next
    // episode's downbeat are one voice across the barline. Without the link they read as an illegal
    // mono overlap (the cadence's release tail crosses the downbeat); the choke cancels at the seam.
    for e in 0..episodes.saturating_sub(1) {
        // Don't sew a legato seam across a withheld (non-lead-carried) episode — at the handoff the
        // lead genuinely STOPS, so there is no single voice continuing across the gap. Linking the
        // pre-bridge cadence to the post-bridge return would fake a continuity the lead never played
        // (and VoiceContinuation would span a 16-beat silence).
        if argument.placements[e].carriers != [Agent::Lead]
            || argument.placements[e + 1].carriers != [Agent::Lead]
        {
            continue;
        }
        let boundary = (e + 1) as f64 * EPISODE_BEATS;
        let last = lead
            .iter()
            .filter(|n| n.start_beat < boundary)
            .max_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
        let next = lead
            .iter()
            .filter(|n| n.start_beat >= boundary)
            .min_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
        if let (Some(a), Some(b)) = (last, next) {
            lead_links.push(VoiceContinuation::new(a, b).unwrap());
        }
    }
    score = commit_role(score, &perf, world, Role::Lead, lead, lead_links)?;

    // --- KEYS: per-episode call/answer threaded by the memory law. ---
    let mut hearings = Vec::new();
    let mut keys = Vec::new();
    // THE CARRIER PASS (narrative handoff): a non-lead-carried episode — the keys-carried bridge —
    // puts its whole thematic obligation into the KEYS lane, exactly as the lead pass does for lead
    // episodes. For the shipped all-Lead arms this adds nothing (no placement carries Keys), so the
    // frozen path is untouched. The per-episode answer below is skipped for such an episode (C), and
    // the comp block already skips a Depart, so the keys lane there holds ONLY the carrier notes —
    // exactly what the content witness expects.
    super::argument_transport::replace_required_role(
        &mut keys,
        Role::Keys,
        &argument.placements,
        world.tonic_pc,
        &perf.contexts,
        &argument.song.plan,
    )
    .map_err(|_| KernelError::Source("keys carrier realization refused"))?;
    for (i, p) in argument.placements.iter().enumerate() {
        if p.carriers == [Agent::Lead] {
            continue; // lead-carried episodes own no keys carrier notes
        }
        let notes: Vec<_> = keys
            .iter_mut()
            .filter(|n| n.start_beat >= p.start_beat && n.start_beat < p.start_beat + p.span_beats)
            .collect();
        for n in notes {
            n.prov.material = Some(perf.statements[i].material);
            n.prov.actions = n.prov.actions.with(perf.statements[i].call.unwrap());
        }
    }
    let mut keys_windows = vec![0usize; episodes];
    let mut memory = BandMemory::default();
    // The previous episode's call material: what a later episode discharges if that call went
    // unpaid. Threaded alongside BandMemory (which carries only the law's boolean/count state).
    let mut carried_cell: Option<InteractionMaterial> = None;
    // THE EARNED RETURN — make the declared `Return.depends_on(Depart)` edge LOAD-BEARING in the
    // performance instead of a formal provenance note. For each episode that RETURNS from a bridge,
    // find the bridge episode it depends on (the `Return` step names the bridge in `depends_on`). The
    // keys below QUOTE that bridge's call cell in the return window instead of re-answering the hook:
    // the SAME note budget, but its content is now a function of the SPECIFIC bridge, so two lawful
    // bridges over the same chart and the same returning hook come home sounding different — the band
    // brings a fragment of the journey back. An arc with no Return-from-a-Depart (the two-episode
    // verse→hook control) maps every episode to `None`, so the keys decision — and its render — is
    // unchanged. The episodes are 1:1 with placements/materials/statements, so the bridge episode
    // index is also its material index.
    let steps = &argument.argument.steps;
    let returns_from_bridge: Vec<Option<usize>> = (0..episodes)
        .map(|e| {
            if !matches!(
                argument.placements[e].relation,
                super::argument::ArgumentRelation::Return { .. }
            ) {
                return None;
            }
            let step = steps.iter().find(|s| s.id == argument.placements[e].step)?;
            // The depended-on step that is itself a Depart: the bridge the return travelled from.
            let bridge_step = step.depends_on.iter().copied().find(|dep| {
                steps.iter().any(|s| {
                    s.id == *dep
                        && matches!(s.relation, super::argument::ArgumentRelation::Depart { .. })
                })
            })?;
            // The episode whose placement carries that bridge step (== its material index).
            (0..episodes).find(|&b| argument.placements[b].step == bridge_step)
        })
        .collect();
    // `e` is an episode index into several parallel structures (materials, statements, the window
    // tally, the beat offset); the range loop is the clearest expression of that.
    #[allow(clippy::needless_range_loop)]
    for e in 0..episodes {
        let ws = e as f64 * EPISODE_BEATS;
        // A keys-carried episode (the narrative bridge handoff) already holds its whole thematic
        // obligation in the keys lane from the carrier pass above; it makes NO call/answer here. The
        // lead withheld, so there is nothing to react to, and the content witness expects ONLY the
        // carrier notes in this window — an answer would be an extra onset it rejects.
        let keys_carried_here = argument.placements[e].carriers != [Agent::Lead];
        hearings.push(EffectiveHearing::capture(
            Agent::Keys,
            Role::Lead,
            ws,
            ws + 4.0,
            &score,
        ));
        let heard: Vec<_> = score
            .role_notes(Role::Lead)
            .filter(|n| n.start_beat >= ws && n.start_beat < ws + 4.0)
            .copied()
            .collect();
        // This episode's call material. `transform_material` answers its TAIL (the last <=4
        // events, the phrase's cadence gesture) — which is exactly where the verse (hangs open on
        // the flat-seventh) and the hook (lands home on the tonic) genuinely diverge, so WHICH call
        // a later episode discharges is audible. Answering the shared opening head would not be.
        let cell = perf.materials[e].clone();
        // The base decision reads the heard opening contour exactly as `perform` does: a tonic close
        // stands alone (Silence), an open rising line invites a contrary answer (Invert), a falling
        // line is quoted. Keys off-stage is a real withheld window (Silence). The contour itself is
        // kept separately from the off-stage override so a later episode can discharge it on-stage.
        let closed = heard
            .last()
            .is_some_and(|n| n.pitch.rem_euclid(12) == world.tonic_pc.rem_euclid(12));
        let rising = heard.len() >= 2 && heard.last().unwrap().pitch > heard[0].pitch;
        let on_stage = perf.stage.on_stage_span(Agent::Keys, ws + 4.0, ws + 8.0);
        let base = if !on_stage || closed {
            Transform::Silence
        } else if rising {
            Transform::Invert
        } else {
            Transform::Quote
        };
        let call_real = !heard.is_empty();
        // THE MEMORY LAW: an unpaid call carried from an earlier episode is DISCHARGED here —
        // source-bound to THAT earlier call's material, and QUOTED (the band finally restates the
        // figure it owes), overriding this episode's own base decision (which for a closed hook is
        // Silence/make-room). An in-the-moment answer INVERTS the call (a contrary reply); a late
        // discharge QUOTES it (the thing left hanging, said at last) — so WHICH call drove the
        // keys is audible in the contour, not just in a source label the projection would quantize
        // away. Otherwise this episode makes its own base decision against its own call.
        let (transform, material_src) = if let Some(b) = returns_from_bridge[e].filter(|_| on_stage)
        {
            // THE EARNED RETURN: the home-coming brings the journey back — the keys QUOTE a fragment
            // of the bridge's call cell, laid over the return's home harmony, instead of re-answering
            // the hook they already stated. The returning LEAD melody is untouched (it is still the
            // exact hook); only its accompaniment now carries a consequence of the specific bridge, so
            // a different lawful bridge comes home different. Credited to the bridge's own call, so the
            // causal ledger records a real (bridge-call -> keys-response) edge, not just a label.
            (Transform::Quote, Some(perf.materials[b].clone()))
        } else if memory.open_call && on_stage {
            (Transform::Quote, carried_cell.clone())
        } else {
            (base, Some(cell.clone()))
        };
        // The answer sings across its response window ON the shared grid, from the downbeat of the
        // episode's second bar — a call-and-response that resolves inside bar 2, not a crammed,
        // sixteenth-late burst dangling off the end of it. (Keys remain confined to `[4, 8)` per
        // episode by `validate_song`; this just uses that window honestly instead of its last beat.)
        let keys_at = ws + 4.0;
        let mut produced = 0usize;
        if !matches!(transform, Transform::Silence) && !keys_carried_here {
            if let Some(material) = material_src.as_ref().and_then(|src| {
                super::material::transform_material(
                    MaterialId(perf.materials.len() as u32),
                    src,
                    transform,
                    Agent::Keys,
                    keys_at,
                    3.0,
                )
            }) {
                // The answer credits the call it ACTUALLY discharges, derived from the quoted
                // material's own source — NOT the current episode index. In the in-the-moment
                // branch that is this episode's call; in the memory-law discharge branch it is the
                // EARLIER unpaid call (`carried_cell`'s). Reading the id from `material_src.source`
                // keeps both branches correct by construction, so it does not rot if a later debt
                // is carried more than one episode (the `statements[e]` shortcut was right only by
                // coincidence on the in-the-moment path).
                let origin = material_src
                    .as_ref()
                    .expect("a non-silent answer has a source cell");
                let origin_stmt = match origin.source {
                    super::material::MaterialSource::Statement { statement, .. } => statement,
                    _ => e,
                };
                let origin_call = perf.statements[origin_stmt].call;
                let aid = action(
                    &mut perf.actions,
                    Agent::Keys,
                    ActionKind::Answer,
                    keys_at,
                    material.length(),
                    origin_call,
                    0,
                );
                // THE EARNED RETURN's anchor: the bridge hangs OPEN on a specific tendency pitch, and
                // two lawful bridges hang on DIFFERENT ones. The relative contour alone collapses to
                // the home chord tones under projection (the projection-fidelity collision), so the
                // return keys re-enter from the pitch-class the bridge left hanging, brought into the
                // keys' register — and the projection resolves it into the home harmony. The
                // home-coming thus RESOLVES the exact tension the specific bridge posed; a different
                // lawful bridge resolves differently. Elsewhere the keys anchor on the chord root.
                let anchor = match returns_from_bridge[e] {
                    Some(b) => {
                        let bridge_ws = b as f64 * EPISODE_BEATS;
                        let hanging = score
                            .role_notes(Role::Lead)
                            .filter(|n| {
                                n.start_beat >= bridge_ws
                                    && n.start_beat < bridge_ws + EPISODE_BEATS
                            })
                            .max_by(|a, c| a.start_beat.total_cmp(&c.start_beat))
                            .map_or(world.tonic_pc, |n| n.pitch);
                        register(hanging.rem_euclid(12), 54)
                    }
                    None => register(perf.context_at(keys_at).unwrap().chord.root_pc, 60),
                };
                let velocity = heard.iter().map(|n| n.velocity).fold(0.0, f32::max) * 0.78;
                for (at, dur, pitch, accent) in super::material::project_pitched(
                    &material,
                    keys_at,
                    ws + 8.0,
                    anchor,
                    &perf,
                    &|ctx, pc| ctx.chord.pitch_classes().contains(&pc),
                ) {
                    let mut prov =
                        provenance(&perf, at, "answer", section_at(argument, at)).realizing(aid);
                    prov.material = Some(material.id);
                    prov.interaction = Some(InteractionId(e as u32));
                    keys.push(pitched(
                        &perf,
                        Role::Keys,
                        at,
                        dur as f32 * 0.8,
                        pitch.unwrap(),
                        velocity * accent,
                        prov,
                    ));
                    produced += 1;
                }
                // Close the causal ledger: register the actual (call -> response) row this answer
                // realizes, so every `prov.interaction = InteractionId(e)` resolves to a real
                // Interaction. Registered HERE (not in `source_plan`) because the ensemble planner
                // consumes `interactions` upstream; adding rows after commitment is invisible to it
                // and to the render. The call is the originating statement; the response is this
                // keys answer.
                if produced > 0 {
                    let call = Call {
                        action: origin_call.expect("the originating statement has a call"),
                        initiator: Agent::Lead,
                        start_beat: origin.start_beat,
                        end_beat: origin.start_beat + origin.length(),
                        statement: Some(origin_stmt),
                        material: origin.id,
                    };
                    let response = Response {
                        action: Some(aid),
                        responder: Agent::Keys,
                        start_beat: keys_at,
                        dur_beats: material.length(),
                        latency: keys_at - call.end_beat,
                        // Truthful overlap (J3 repair): the base keys answer begins before the
                        // lead's authored cadence it quotes has finished sounding — negative
                        // latency, hence an overlapping anticipation, not a post-hoc reaction.
                        // Derived from latency so the receipt can never again deny its own overlap.
                        overlap: keys_at < call.end_beat,
                        transform,
                        crosses_chord: false,
                        material: Some(material.id),
                        realizes: None,
                    };
                    perf.interactions.push(Interaction {
                        id: InteractionId(e as u32),
                        call,
                        response: Some(response),
                    });
                }
                perf.materials.push(material);
            }
        }
        keys_windows[e] = produced;
        // The call went unanswered iff it was real and this episode produced nothing: carry the debt.
        memory.open_call = produced == 0 && call_real;
        memory.prev_answer_events = produced;
        carried_cell = Some(cell);
    }
    // KEYS COMP — comp like a pianist, not a one-finger poke. The per-episode answer above is a single
    // gesture in bar 2; on its own the keyboard reads very sparse. Here the keys COMP the harmonic holes
    // the way a fusion keyboardist does (the Casiopea/Takanaka seat): a ROOTLESS CHORD VOICING of the
    // bar's committed harmony — the guide tones and colour it holds (its 3rd/5th/7th), never the root the
    // bass owns — voiced POLYPHONICALLY in a warm mid register, and topped with a song-fitting COLOUR
    // tone (the 9th, admitted only when the mode holds it) decorated by a grace ornament. Polyphony is
    // lawful here because the keys, like the pad, carry no argument line and no continuity edge, so they
    // are exempt from the monophonic lane — a comp is chords, not single notes. Teacher-gated to the HOME
    // ARRIVALS (the hook and the return, where the pad also lays its rug and the texture is meant to be
    // full), so the VERSE stays bare (the open question, lead alone) and the BRIDGE stays thin (the
    // departure's texture contrast). Every stab makes ROOM for the lead (never on its attack) and never
    // enters the answer window `[ws+4, ws+8)` the drums hear — so the keys-independent pulse the causal
    // witness depends on is untouched.
    if argument.comp_keys {
        let lead_onsets: Vec<f64> = score.role_notes(Role::Lead).map(|n| n.start_beat).collect();
        let pocket = super::theme_family::pocket_offset;
        let scale = Scale::new(world.tonic_pc, world.mode);
        let bars_per_ep = (EPISODE_BEATS / 4.0) as usize;
        let room = |at: f64| lead_onsets.iter().all(|&l| (l - at).abs() >= 0.15);
        // Voice a set of pitch classes ascending from a mid-register floor (the pad's rug idiom): each
        // tone seated at or above the running floor, so the voicing climbs instead of stacking in a mud.
        let voice = |floor: i32, pcs: &[i32]| -> Vec<i32> {
            let mut low = floor;
            pcs.iter()
                .map(|&pc| {
                    let p = register(pc, low);
                    low = p + 1;
                    p
                })
                .collect()
        };
        for e in 0..episodes {
            if !argument.comp_all_sections
                && !matches!(
                    argument.placements[e].relation,
                    super::argument::ArgumentRelation::Consequent { .. }
                        | super::argument::ArgumentRelation::Return { .. }
                )
            {
                // Comp only under the home arrivals; verse and bridge keep the floor clear — UNLESS a
                // lead-free smooth-pop band opts into comp-everywhere (the keys hold the chordal carrier
                // throughout). C137 never sets the flag, so its gate is unchanged and its render frozen.
                continue;
            }
            if let Some(onsets) = &argument.comp_onsets {
                // RIFF-RHYTHM COMP: the keys stab chord voicings on the band's OWN riff rhythm (the
                // 2-bar riff stated twice across the 16-beat episode), so the guitarist's figure is
                // HEARD as chordal hits — the band playing its hook — rather than a fixed pattern. Each
                // onset gets a LEAN guide-tone voicing in the bar's committed harmony; the bass owns the
                // root. Opt-in (`comp_onsets`); C137 and every lead-carried song leave it None and keep
                // the fixed comp below, so their render is byte-identical.
                // The keys must sit PERFECTLY in pocket for a rhythmic chordal comp: each stab rides the
                // band's shared pocket but is pulled EARLIER by KEYS_ATTACK_COMP, so its FELT attack (the
                // FM e-piano speaks a hair late through the chorus + room) lands dead on the pocket with
                // the kit. A beat-weighted accent (strong beat > "and" > sixteenth) shapes the groove, and
                // the voicing says more with less — guide tones only, the colour spent on the strong hit,
                // crowding stabs shrunk to a passing tone — so the hook reads as intent, not swagger.
                let mut last_at = f64::NEG_INFINITY;
                for rep in 0..2u32 {
                    let rep_base = e as f64 * EPISODE_BEATS + rep as f64 * 8.0;
                    for &o in onsets {
                        if !(0.0..8.0).contains(&o) {
                            continue;
                        }
                        let posin = o.rem_euclid(4.0);
                        let at_grid = rep_base + o;
                        let at = (at_grid + pocket(posin) - KEYS_ATTACK_COMP).max(0.0);
                        let bar_start = (at_grid / 4.0).floor() * 4.0;
                        let Some(ctx) = perf.context_at(bar_start) else {
                            continue;
                        };
                        let tones = ctx.chord.pitch_classes();
                        if tones.len() < 2 {
                            continue;
                        }
                        let energy = drive_energy(harmonic_drive(&ctx.chord, world.tonic_pc));
                        // SAY MORE WITH LESS: the comp names the chord with its GUIDE TONES (the 3rd and
                        // the 7th — the two notes that fix the quality), never the full stack. The bass
                        // owns the root and the pad holds the bed, so a fat rootless cluster on every hit
                        // is redundant swagger. The 5th is dropped; the colour 9th is spent ONLY on the
                        // structural downbeat; a stab crowding the one before it shrinks to a single
                        // passing tone. Lean by default, open up only where it earns the notes.
                        let q = (posin * 4.0).round() as i64;
                        let strong = q % 4 == 0;
                        let medium = q % 2 == 0;
                        let crowded = at - last_at < 0.3;
                        let third = tones[1];
                        let seventh = tones.get(3).copied().or_else(|| tones.get(2).copied());
                        let ninth = tones.get(4).copied();
                        let mut pcs = vec![third];
                        if !crowded {
                            if let Some(s) = seventh {
                                pcs.push(s); // the chord's identity in two notes
                            }
                            if strong {
                                if let Some(n) = ninth {
                                    pcs.push(n); // the colour, reserved for the structural hit
                                }
                            }
                        }
                        let mut level = if strong {
                            0.40
                        } else if medium {
                            0.30
                        } else {
                            0.22
                        };
                        if crowded {
                            level *= 0.6; // a crowding stab stays a quiet single passing tone
                        }
                        last_at = at;
                        // A stab whose tail would spill past the song's end (a riff onset late in the
                        // final bar — e.g. an anticipation at beat 7.75 stated in the last episode) is
                        // dropped: its 16th-note sibling earlier in the song already states that onset,
                        // so the hook is intact and the Score stays inside the performance domain.
                        if perf.domain().contains(at, at + 0.26)
                            && perf.on_stage(Agent::Keys, at)
                            && room(at)
                        {
                            // Comp UNDER the theme: if the voicing's top would break above the ceiling
                            // (a 9th spiking to D5/E5 into the theme's register), drop the whole stab an
                            // octave so it cushions the tune instead of fighting it. Shape and colour
                            // intact — only the register moves. See COMP_CEILING.
                            let mut voiced = voice(57, &pcs);
                            if voiced.last().is_some_and(|&top| top > COMP_CEILING) {
                                for p in &mut voiced {
                                    *p -= 12;
                                }
                            }
                            for pitch in voiced {
                                keys.push(pitched(
                                    &perf,
                                    Role::Keys,
                                    at,
                                    0.26,
                                    pitch,
                                    (level * energy).clamp(0.02, 1.0),
                                    provenance(&perf, at, "comp", section_at(argument, at)),
                                ));
                            }
                        }
                    }
                }
                continue;
            }
            for b in 0..bars_per_ep {
                if b == 1 {
                    continue; // bar 2 is the answer's own bar — leave it to the answer
                }
                let bar_start = e as f64 * EPISODE_BEATS + b as f64 * 4.0;
                let ctx = perf.context_at(bar_start).unwrap();
                let tones = ctx.chord.pitch_classes();
                if tones.len() < 2 {
                    continue;
                }
                let energy = drive_energy(harmonic_drive(&ctx.chord, world.tonic_pc));
                // The chord above its root (root_pc is tones[0]); the bass owns the root.
                let rootless = &tones[1..];
                // The colour on top: the 9th, but only where the MODE admits it and the chord does not
                // already spell it — so the colour fits the song's key instead of fighting it (on a chord
                // whose 9th leaves the mode, the comp simply states the voicing, no forced colour).
                let ninth = (ctx.chord.root_pc + 2).rem_euclid(12);
                let colour_pc =
                    (scale.contains_pc(ninth) && !tones.contains(&ninth)).then_some(ninth);
                // The "and of two": the full rootless voicing — the body of the comp, planted in pocket.
                let a = bar_start + 2.5 + pocket(2.5);
                if perf.on_stage(Agent::Keys, a) && room(a) {
                    for pitch in voice(57, rootless) {
                        keys.push(pitched(
                            &perf,
                            Role::Keys,
                            a,
                            0.33,
                            pitch,
                            (0.40 * energy).clamp(0.02, 1.0),
                            provenance(&perf, a, "comp", section_at(argument, a)),
                        ));
                    }
                }
                // The "and of four": a lighter guide-tone shell (3rd + 7th) topped with the colour tone
                // and a grace ornament a scale-step above it a 16th before the stab — the pianist's bright
                // answer to its own body chord.
                let c = bar_start + 3.5 + pocket(3.5);
                if perf.on_stage(Agent::Keys, c) && room(c) {
                    let shell: Vec<i32> = rootless.iter().copied().step_by(2).collect();
                    let voiced = voice(60, &shell);
                    for &pitch in &voiced {
                        keys.push(pitched(
                            &perf,
                            Role::Keys,
                            c,
                            0.33,
                            pitch,
                            (0.38 * energy).clamp(0.02, 1.0),
                            provenance(&perf, c, "comp", section_at(argument, c)),
                        ));
                    }
                    if let Some(cpc) = colour_pc {
                        let top = *voiced.last().unwrap_or(&60);
                        let colour = register(cpc, top + 1);
                        keys.push(pitched(
                            &perf,
                            Role::Keys,
                            c,
                            0.33,
                            colour,
                            (0.32 * energy).clamp(0.02, 1.0),
                            provenance(&perf, c, "comp", section_at(argument, c)),
                        ));
                        let g = c - 0.25;
                        if g > bar_start && perf.on_stage(Agent::Keys, g) && room(g) {
                            let grace = scale.nearest_scale_pitch(colour + 2);
                            keys.push(pitched(
                                &perf,
                                Role::Keys,
                                g,
                                0.18,
                                grace,
                                (0.24 * energy).clamp(0.02, 1.0),
                                provenance(&perf, g, "comp", section_at(argument, g)),
                            ));
                        }
                    }
                }
            }
        }
    }
    score = commit_role(score, &perf, world, Role::Keys, keys, Vec::new())?;
    for e in 0..episodes {
        let ws = e as f64 * EPISODE_BEATS;
        hearings.push(EffectiveHearing::capture(
            Agent::Drums,
            Role::Keys,
            ws + 4.0,
            ws + 8.0,
            &score,
        ));
    }

    // --- BASS: a foundation whose DENSITY follows the harmony it is spelling. On the tonic the bass
    // ANCHORS and lets the chord ring (root + fifth — a breath at the point of rest); as the harmony
    // builds and drives it WALKS the changes in punchy quarters (root, fifth, third) and steps into
    // the next downbeat. The downbeat root is the required harmonic anchor in EVERY bar, and every
    // note's velocity rides the shared ensemble energy (eased at home, leaning in on the dominant).
    // A chromatic approach is planned against each later episode's downbeat (every episode seam). ---
    let mut bass = Vec::new();
    let bars_per_episode = (EPISODE_BEATS / 4.0) as usize;
    let total_bars = episodes * bars_per_episode;
    let bass_scale = Scale::new(world.tonic_pc, world.mode);
    for bar in 0..total_bars {
        let at = bar as f64 * 4.0;
        let ctx = perf.context_at(at).unwrap();
        let drive = harmonic_drive(&ctx.chord, world.tonic_pc);
        let energy = drive_energy(drive);
        // The intro build: in the opening episode the foundation enters reduced (early bars lay the
        // pocket on root + fifth only) and swells into the first verse — the downbeat root holds the
        // pocket (skeleton), the walk motion washes in on the swell (applied per note, below).
        let intro_i = intro_build_bar(argument, bar, bars_per_episode);
        let tones = ctx.chord.pitch_classes();
        let root = register(ctx.chord.root_pc, 36);
        let fifth = register(tones[2.min(tones.len().saturating_sub(1))], 36);
        let third = register(tones[1.min(tones.len().saturating_sub(1))], 36);
        // The chord's seventh, voiced from the same low floor. For a dominant seventh (the cadence's
        // E7: D a whole step below the root E) it lands BELOW the root — a dark descending bass tone.
        let seventh = register(tones[3.min(tones.len().saturating_sub(1))], 36);
        // Beat four leads into the next bar. With the foundation ANTICIPATING the destination
        // (`anticipate_bass`), its approach expresses WHERE the harmony is going: the dominant into a
        // home arrival (the V->I the ear hears coming), the leading tone a half-step below into a drive
        // (tension), a diatonic step into a passing change (a smooth walk). Without it (the archived
        // control), it is the blind chromatic half-step into whatever comes next. Either way it is one
        // monophonic note in the bass register; the final bar has no successor, so it settles on the
        // fifth rather than leading away from home.
        let beat_four =
            match (bar + 1 < total_bars).then(|| perf.context_at(at + 4.0).unwrap().chord) {
                None => fifth,
                Some(next) => {
                    let r = register(next.root_pc, 36);
                    let chromatic = if r > 36 { r - 1 } else { r + 1 };
                    if argument.anticipate_bass {
                        match harmonic_drive(&next, world.tonic_pc) {
                            HarmonicDrive::Home => register((next.root_pc + 7).rem_euclid(12), 36),
                            HarmonicDrive::Driving => chromatic,
                            HarmonicDrive::Building => bass_scale.nearest_scale_pitch(r - 2),
                        }
                    } else {
                        chromatic
                    }
                }
            };
        // At HOME the bass lays back: the root rings and the fifth answers on beat three, leaving the
        // tonic room to breathe (the fifth is clipped short of beat four, clearing any planned seam
        // pickup). Everywhere else it walks the changes. One monophonic line in the bass register —
        // motion that voices the harmony, never a doubled or sub-octave weave.
        let figure: Vec<(f64, i32, f32, f32)> = if let Some(groove) = &argument.groove {
            // SMOOTH-POP: the authored per-bar cell for THIS loop position, resolved against this
            // bar's chord. Each cell opens on the root at offset 0.0 (the required downbeat anchor the
            // foundation law reads); the rest is the cell's own off-beat identity, and the cells VARY
            // across the loop so the groove breathes instead of stamping one bar eight times. Every
            // attack still orbits the shared pocket (applied below).
            let cell = &groove.bass_cells[bar % groove.bass_cells.len()];
            cell.iter()
                .map(|&(offset, degree, dur, velocity)| {
                    let pitch = match degree {
                        super::argument::BassDegree::Root => root,
                        super::argument::BassDegree::Third => third,
                        super::argument::BassDegree::Fifth => fifth,
                        super::argument::BassDegree::Seventh => seventh,
                        // The octave pop, folded DOWN by whole octaves into the bass register, so it is
                        // always the root's own pitch class (never the wrong note a hard clamp to 55
                        // would sound on a high root) and never above the bass ceiling.
                        super::argument::BassDegree::Octave => {
                            let mut p = root + 12;
                            while p > 55 {
                                p -= 12;
                            }
                            p
                        }
                    };
                    (offset, pitch, dur, velocity)
                })
                .collect()
        } else if intro_i.is_some_and(|i| i <= 1) {
            // The intro's early bars: the resting root + fifth only, regardless of the chart's drive,
            // so the foundation enters sparse and the walk fills in as the build swells into the verse.
            vec![(0.0, root, 1.8, 0.70), (2.0, fifth, 1.4, 0.52)]
        } else {
            match drive {
                HarmonicDrive::Home => vec![(0.0, root, 1.8, 0.70), (2.0, fifth, 1.4, 0.52)],
                _ => vec![
                    (0.0, root, 0.8, 0.76),
                    (1.0, fifth, 0.45, 0.56),
                    (2.0, third, 0.45, 0.60),
                    (3.0, beat_four.clamp(36, 55), 0.45, 0.54),
                ],
            }
        };
        let swell = groove_swell(bar, bars_per_episode);
        for (offset, pitch, dur, velocity) in figure {
            // The grid position labels the section/phrase; the SHARED pocket leans the attack off it
            // (the downbeat root at `offset==0` stays exactly on `at`, the required anchor), and the
            // velocity rides the harmonic energy and the phrase swell. The bass orbits the SAME
            // pocket as the drums and the lead, so coincident attacks lock — one solar system.
            // The downbeat root is the SKELETON (holds the pocket from the top); the walk notes are the
            // MOTION that washes in on the swell. Both resolve to 1.0 outside the intro.
            let ib = intro_i.map_or(1.0, |i| intro_build_gain(i, offset == 0.0));
            let prov = provenance(
                &perf,
                at + offset,
                "foundation",
                section_at(argument, at + offset),
            );
            bass.push(pitched(
                &perf,
                Role::Bass,
                at + offset + super::theme_family::pocket_offset(offset),
                dur,
                pitch,
                velocity * energy * swell * ib,
                prov,
            ));
        }
    }
    for e in 1..episodes {
        let seam = e as f64 * EPISODE_BEATS;
        let arrival = register(perf.context_at(seam).unwrap().chord.root_pc, 36);
        // The chromatic pickup leads into the next episode's downbeat — but only a RESTING pre-seam
        // bar (home, root + fifth) needs the lead-in: a walking pre-seam bar already steps into the
        // downbeat on its own beat four, so adding the pickup there would be a redundant, colliding
        // second leading tone. (The two-episode control's pre-seam bar is always home, so this guard
        // is always satisfied there and its pickup is unchanged.)
        let pre_seam_rests = matches!(
            harmonic_drive(&perf.context_at(seam - 4.0).unwrap().chord, world.tonic_pc),
            HarmonicDrive::Home
        );
        if pre_seam_rests
            && arrival > 36
            && perf
                .stage
                .on_stage_span(Agent::Bass, seam - 0.5, seam + 0.5)
        {
            let id = action(
                &mut perf.actions,
                Agent::Bass,
                ActionKind::Pickup,
                seam - 0.5,
                0.5,
                None,
                0,
            );
            perf.actions.actions[id.0 as usize].target_beat = Some(seam);
            let mut note = pitched(
                &perf,
                Role::Bass,
                seam - 0.5,
                0.3,
                arrival - 1,
                0.62,
                provenance(
                    &perf,
                    seam - 0.5,
                    "approach",
                    section_at(argument, seam - 0.5),
                )
                .realizing(id),
            );
            note.function = Some(PitchFunction::ChromaticApproach);
            bass.push(note);
        }
    }
    bass.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    let bass_links = bass
        .windows(2)
        .map(|pair| VoiceContinuation::new(&pair[0], &pair[1]).unwrap())
        .collect();
    score = commit_role(score, &perf, world, Role::Bass, bass, bass_links)?;

    // --- DRUMS: one shared pocket played like a fusion KIT whose density is ALLOCATED by the song's
    // state, not held flat with every limb maxed. A steady spine (kick on 1 & 3, backbeat on 2 & 4)
    // locks the band's time in every bar. The ride BREATHES with the harmony — eighths when it is home
    // and resting, sixteenths as it builds and drives — and the whole kit's dynamics ride the shared
    // ensemble energy. Over that, the drummer COMPS into the off-beats the lead leaves open, reading
    // the lead's authored occupancy: softer inside a live phrase, fuller where the phrase has yielded
    // (an answer), and silent where the melody is speaking (make room). A rising fill lifts into each
    // phrase boundary. Every stroke sits within a bounded, STRUCTURED pocket (reasoned micro-timing
    // and dynamics, NEVER random jitter) and is derived from the LEAD and HARMONY only — never the
    // keys — so the pulse stays bit-identical regardless of any keys response (the causal witness
    // depends on this). A separate heard-keys fill (below) carries the keys law per episode. ---
    let lead_notes: Vec<Note> = score.role_notes(Role::Lead).copied().collect();
    let lead_occ = AuthoredOccupancy::from_role(&perf, &score.notes, Role::Lead);
    // OPERATION JOHNNY FIVE: the funk/disco rhythm-section spine, opt-in per source. C137 and the
    // lead-free groove control leave it false, so their drum pattern stays byte-identical.
    let funk_kit = argument.pop_funk_kit;
    for bar in 0..total_bars {
        let bar_start = bar as f64 * 4.0;
        let drive = harmonic_drive(&perf.context_at(bar_start).unwrap().chord, world.tonic_pc);
        let energy = drive_energy(drive);
        // A fill lands because the NEXT bar opens a new phrase (each episode seam, then the final bar).
        let fill_bar = (bar + 1) % bars_per_episode == 0;
        // The whole kit's dynamics ride the harmonic energy AND the within-phrase swell; the `lay`
        // leans a stroke off the grid into the pocket (0 = dead-on, the time reference).
        let swell = groove_swell(bar, bars_per_episode);
        // The intro build: the opening episode's kit enters reduced and swells in. The SKELETON (the
        // downbeat/backbeat) holds its pocket from the top; the TEXTURE (hats, comps, the fill) washes
        // in on the back-loaded swell — so it builds by arrangement, not a uniform fader. The early
        // bars also THIN (bar 0 = kick + eighth hats, bar 1 adds the backbeat, both without comps) so
        // the groove assembles over the intro rather than arriving whole.
        let intro_i = intro_build_bar(argument, bar, bars_per_episode);
        let mut push = |voice: DrumVoice, grid: f64, vel: f32, label: &'static str, lay: f64| {
            // Kick and the backbeat snare are the skeleton; hats, comps and the fill are the texture.
            let skeleton = matches!(voice, DrumVoice::Kick)
                || (matches!(voice, DrumVoice::Snare) && label == "pulse");
            let ib = intro_i.map_or(1.0, |i| intro_build_gain(i, skeleton));
            score.drums.push(DrumHit {
                start_beat: grid + lay,
                voice,
                velocity: (vel * energy * swell * ib).clamp(0.02, 1.0),
                prov: provenance(&perf, grid, label, section_at(argument, grid)),
            });
        };
        // The steady spine — always present, locking the shared time. The downbeat kick and the hats
        // are the dead-on time reference; the backbeat snare (and the beat-three kick) DRAG behind
        // them by the SAME shared pocket the bass and lead use, so the whole band locks as one body.
        let pocket = super::theme_family::pocket_offset;
        if funk_kit {
            // FUNK / DISCO-FUNK spine (opt-in `pop_funk_kit`): the kick LOCKS TO THE BASS. A crisp
            // backbeat, sixteenth hats with the "and"s opened (the disco shimmer) and a ghost pickup —
            // but the kick does NOT plow a rigid four-on-the-floor grid over a syncopated bass (the
            // "doesn't know funk" tell). A funk band is ONE pocket: the kick and the bass are a single
            // unit. So the kick plays the downbeat, every ROOT the bassist lands (the foundation hits)
            // and the bar's end-push — at the SAME pocketed microtime the bass uses — and leaves the
            // space where the bass syncopates off the root (the funk breath). It reads the authored
            // groove cell (the committed bass figure): deterministic, no clairvoyance, and a changed
            // bass cell lawfully changes the later kick (the bass is committed before the drums, so no
            // earlier event moves).
            if let Some(groove) = &argument.groove {
                let cell = &groove.bass_cells[bar % groove.bass_cells.len()];
                for &(offset, degree, _, _) in cell {
                    let is_root = matches!(degree, super::argument::BassDegree::Root);
                    let is_push = offset >= 3.5;
                    if offset == 0.0 || is_root || is_push {
                        let v = if offset == 0.0 {
                            0.95
                        } else if is_push {
                            0.82
                        } else {
                            0.80
                        };
                        push(
                            DrumVoice::Kick,
                            bar_start + offset,
                            v,
                            "pulse",
                            pocket(offset),
                        );
                    }
                }
            } else {
                push(DrumVoice::Kick, bar_start, 0.95, "pulse", 0.0);
            }
            push(
                DrumVoice::Snare,
                bar_start + 1.0,
                0.82,
                "pulse",
                pocket(1.0),
            );
            push(
                DrumVoice::Snare,
                bar_start + 3.0,
                0.86,
                "pulse",
                pocket(3.0),
            );
            // a funk ghost snare pickup into beat four
            push(
                DrumVoice::Snare,
                bar_start + 2.75,
                0.17,
                "pulse",
                pocket(2.75),
            );
            for s in 0..16u32 {
                let at = bar_start + s as f64 * 0.25;
                let accent = match s % 4 {
                    0 => 0.42, // the beat
                    2 => 0.34, // the "and"
                    _ => 0.16, // the "e" and "a"
                };
                // Open the hat on the back-half "ands" (the "and of 2" and "and of 4"): the disco lift.
                let voice = if s == 6 || s == 14 {
                    DrumVoice::OpenHat
                } else {
                    DrumVoice::ClosedHat
                };
                push(voice, at, accent, "pulse", 0.0);
            }
        } else {
            push(DrumVoice::Kick, bar_start, 0.95, "pulse", 0.0);
            push(DrumVoice::Kick, bar_start + 2.0, 0.78, "pulse", pocket(2.0));
            // The backbeat locks the band's time — but the intro's first bar lays the snare OUT, so the kit
            // opens on kick + hats alone and the backbeat arrives as the build fills in.
            if intro_i != Some(0) {
                push(
                    DrumVoice::Snare,
                    bar_start + 1.0,
                    0.80,
                    "pulse",
                    pocket(1.0),
                );
                push(
                    DrumVoice::Snare,
                    bar_start + 3.0,
                    0.84,
                    "pulse",
                    pocket(3.0),
                );
            }
            // The ride carpet: its SUBDIVISION follows the harmonic drive — eighths at rest, sixteenths in
            // motion — with the down/and/e-a accent hierarchy and the hat opening on the "and of four".
            // Eighths at rest, sixteenths in motion — but the intro's thinned early bars stay on eighths,
            // so the ride opens sparse and only fills to sixteenths as the build drives in.
            let sixteenths =
                !matches!(drive, HarmonicDrive::Home) && !intro_i.is_some_and(|i| i <= 1);
            let stride: usize = if sixteenths { 1 } else { 2 };
            for s in (0..16u32).step_by(stride) {
                let at = bar_start + s as f64 * 0.25;
                let accent = match s % 4 {
                    0 => 0.40, // the beat
                    2 => 0.32, // the "and"
                    _ => 0.15, // the "e" and "a"
                };
                let voice = if sixteenths && s == 14 {
                    DrumVoice::OpenHat // the "and of four"
                } else {
                    DrumVoice::ClosedHat
                };
                push(voice, at, accent, "pulse", 0.0);
            }
        }
        // The conversation: comp strokes dropped into the off-beats the lead left open, up to the
        // budget the harmonic drive allows (home lays out; building comps; a dominant drives hardest).
        // A kick leans on the "ands" (a syncopated push with the bass), a ghost snare on the "a" — but
        // ONLY where the lead is not attacking (make room), softer inside a live phrase and fuller
        // where the phrase has yielded. This is the make-room law that turns competing voices into
        // interplay, and it reads the lead only, so it never perturbs the keys-independent pulse.
        let budget = if intro_i.is_some_and(|i| i <= 1) {
            0 // the intro's early bars hold the comp back — the groove is still assembling
        } else {
            match drive {
                HarmonicDrive::Home => 0,
                HarmonicDrive::Building => 2,
                HarmonicDrive::Driving => 3,
            }
        };
        // The band HEARS the lead breathe: where an expressive lead has opened a real silence, the
        // drummer steps INTO it with a full-voiced ANSWER on its OWN budget — a breath answer never
        // spends the comp budget nor the reverse. Off unless the source asked for it
        // (`answer_breaths`), and read from the LEAD only, so the keys-independent pulse is untouched.
        //
        // But the band answers a QUESTION, not a resolution. The breath is LICENSED only where the
        // harmony is still open — a predominant reaching, a dominant driving, the bright non-home
        // arrival — i.e. anywhere the lead's statement left something hanging. On a true home
        // resolution (`HarmonicDrive::Home`: the tonic landing, degree 0 and not a secondary
        // dominant) the breath is left UNANSWERED: the lead said its piece and came home, and the
        // band respects that silence rather than filling it (the teacher lays the kit out at the home
        // outro). So the answer is bound to the source harmony on the route, not to bare silence — and
        // some breaths are deliberately never answered. (The witness `the_band_answers_only_the_…`
        // pins this; mutating a bar's route home↔open moves the answer, proving it is not silence-blind.)
        let breath_licensed = !matches!(drive, HarmonicDrive::Home);
        let breath_budget =
            if argument.answer_breaths && breath_licensed && !intro_i.is_some_and(|i| i <= 1) {
                2
            } else {
                0
            };
        let mut placed = 0;
        let mut answered = 0;
        for s in [2u32, 6, 10, 14, 7, 11] {
            if placed >= budget && answered >= breath_budget {
                break;
            }
            let at = bar_start + s as f64 * 0.25;
            if !lead_occ.allows_comp_at(at, &lead_notes) {
                continue; // the lead is speaking here — leave the space
            }
            let (voice, base) = if s % 4 == 2 && !funk_kit {
                (DrumVoice::Kick, 0.56) // on the "and"
            } else {
                // A ghost on the "a" — and, in the funk kit, the "and" too: the kick belongs to the
                // bass-lock alone, so the lead-reading comp adds only ghost snares, never a stray kick.
                (DrumVoice::Snare, 0.40)
            };
            // comps and answers both ride the shared pocket. A breath answer (lead genuinely silent)
            // steps forward at full velocity on its own budget; an ordinary comp is soft inside a
            // live phrase and fuller where the phrase has yielded.
            if !argument.source_answer
                && breath_budget > 0
                && answered < breath_budget
                && lead_silent_at(at, &lead_notes)
            {
                // The accepted (default) breath answer: a full-voiced ghost in a fixed open slot. It
                // sits in a genuine silence over open harmony, but it answers nothing in particular —
                // the honest, source-grounded answer below replaces it when `source_answer` is set.
                push(voice, at, base, "answer", pocket(at - bar_start));
                answered += 1;
            } else if placed < budget {
                let vel = if lead_occ.yields_at(at) {
                    base
                } else {
                    base * 0.5
                };
                push(voice, at, vel, "comp", pocket(at - bar_start));
                placed += 1;
            }
        }
        // Phrase-end fill: the phrase is ending (into the hook, then into the final cadence) — not on a
        // coin flip. When the drummer is TRANSLATING the lead (`translate_lead_rhythm`) it quotes the
        // melody's own rhythm (below); where the lead gave no figure to echo (fewer than two attacks)
        // it falls back to the archived rising sixteenth run so the fill still drives the downbeat.
        if fill_bar {
            // Translate the melody's phrase-ending FIGURE — the lead's attacks across the first half of
            // this bar, before the line resolves and holds — echoed a half-bar later into the fill, so
            // the kit plays the lead's own rhythm back and drives the band into the downbeat together
            // (the same call->echo the keys law uses, now for the lead). Reads the LEAD only.
            let figure: Vec<f64> = if argument.translate_lead_rhythm {
                lead_notes
                    .iter()
                    .map(|n| n.start_beat)
                    .filter(|&b| b >= bar_start && b < bar_start + 2.0)
                    .collect()
            } else {
                Vec::new()
            };
            if figure.len() >= 2 {
                for &o in &figure {
                    let at = o + 2.0; // the figure, echoed into the closing half-bar
                    let rise = ((at - (bar_start + 2.0)) / 2.0) as f32; // 0..1 toward the downbeat
                    push(DrumVoice::Snare, at, 0.40 + 0.30 * rise, "lead-echo", 0.0);
                }
            } else {
                for k in 0..4u32 {
                    push(
                        DrumVoice::Snare,
                        bar_start + 2.0 + k as f64 * 0.5,
                        0.42 + 0.12 * k as f32,
                        "pulse",
                        0.0,
                    );
                }
            }
        }

        // SOURCE-GROUNDED ANSWER (opt-in `source_answer`): the drummer does not fire a fixed ghost
        // into any open bar — it ANSWERS the lead's question. The question is the lead's gesture in
        // the first half of this bar (the attacks that lead into the breath); the answer QUOTES that
        // gesture's rhythm a half-bar later into the breath (so a different question yields a different
        // answer), voiced by the gesture's contour — a rising question PUSHES with the kick, a falling
        // or level one TAGS with a ghost snare. Each stroke realizes a real `ActionKind::Answer` that
        // CREDITS the lead statement's call it answers, so its initiating source is reconstructible
        // (the existing Action ontology, not a second cue system) — unlike the fixed-slot ghost above,
        // which answered nothing in particular. Licensed exactly as the breath answer is (open harmony
        // via `breath_budget`, a genuine lead silence, the budget), so restraint is preserved: a
        // resolved home bar, or a bar the lead never left, gets no answer. Reads the LEAD only, so the
        // keys-independent pulse the causal witness depends on is untouched.
        if argument.source_answer && breath_budget > 0 {
            let episode = (bar_start / EPISODE_BEATS).floor() as usize;
            if episode < perf.statements.len() {
                let origin_call = perf.statements[episode].call;
                let mut question: Vec<&Note> = lead_notes
                    .iter()
                    .filter(|n| n.start_beat >= bar_start && n.start_beat < bar_start + 2.0)
                    .collect();
                question.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
                let mut answered_src = 0i32;
                let mut answer_aid: Option<ActionId> = None;
                for qn in &question {
                    if answered_src >= breath_budget {
                        break;
                    }
                    let at = qn.start_beat + 2.0; // the question's onset, echoed into the breath
                    if at >= bar_start + 4.0 || !lead_silent_at(at, &lead_notes) {
                        continue; // keep it inside the bar and in a genuine silence
                    }
                    if !lead_occ.allows_comp_at(at, &lead_notes) {
                        continue; // make room — the lead is still speaking there
                    }
                    // The gesture's contour at this onset: a rising question drives with the kick, a
                    // falling or level one tags with a ghost snare.
                    let rising = lead_notes
                        .iter()
                        .filter(|m| m.start_beat > qn.start_beat)
                        .min_by(|a, b| a.start_beat.total_cmp(&b.start_beat))
                        .is_some_and(|nx| nx.pitch > qn.pitch);
                    let (voice, base) = if rising {
                        (DrumVoice::Kick, 0.56)
                    } else {
                        (DrumVoice::Snare, 0.40)
                    };
                    // One Answer action per answering bar, crediting the lead statement's call it
                    // answers (cause = Interaction{call}), so the drum answer has a reconstructible
                    // initiating source instead of a dead stamp.
                    let aid = *answer_aid.get_or_insert_with(|| {
                        action(
                            &mut perf.actions,
                            Agent::Drums,
                            ActionKind::Answer,
                            at,
                            1.0,
                            origin_call,
                            episode as u32,
                        )
                    });
                    score.drums.push(DrumHit {
                        start_beat: at + pocket(at - bar_start),
                        voice,
                        velocity: (base * energy * swell).clamp(0.02, 1.0),
                        prov: provenance(&perf, at, "answer", section_at(argument, at))
                            .realizing(aid),
                    });
                    answered_src += 1;
                }
            }
        }
    }
    #[allow(clippy::needless_range_loop)]
    for e in 0..episodes {
        let ws = e as f64 * EPISODE_BEATS;
        if keys_windows[e] == 0 {
            continue;
        }
        let window_keys: Vec<_> = score
            .role_notes(Role::Keys)
            .filter(|n| n.start_beat >= ws + 4.0 && n.start_beat < ws + 8.0)
            .copied()
            .collect();
        if window_keys.is_empty() {
            continue;
        }
        let aid = action(
            &mut perf.actions,
            Agent::Drums,
            ActionKind::Fill,
            ws + 10.5,
            1.0,
            None,
            0,
        );
        // A rhythm echo of this episode's actual final two keys attacks, bounded before its barline.
        let tail = &window_keys[window_keys.len().saturating_sub(2)..];
        let first = tail[0].start_beat;
        for n in tail {
            let at = ws + 10.5 + (n.start_beat - first) * 0.5;
            if at < ws + 11.5 {
                score.drums.push(DrumHit {
                    start_beat: at,
                    voice: DrumVoice::Snare,
                    velocity: n.velocity * 0.65,
                    prov: provenance(&perf, at, "heard-keys", section_at(argument, at))
                        .realizing(aid),
                });
            }
        }
    }
    // --- PAD: the RUG, laid SELECTIVELY. A sustained chordal bed that ties the room together under
    // the band — the committed harmony voiced as a warm MID-register chord (above the bass, under the
    // lead and the keys' accents, never competing), re-voiced each bar as the chord changes and held
    // across it, soft, riding the shared energy. It is the one voice the kernel voices POLYPHONICALLY
    // (the synth's pad pool is six voices); it carries no argument and no continuity line, so it is
    // exempt from the monophonic lane and seam laws. It only sounds where the arrangement seats the
    // pad on stage — the two-episode verse→hook control keeps it silent, so that render stays
    // byte-identical. Crucially the rug is NOT a blanket: it lays harmony under the HOME ARRIVALS
    // (the hook and the return — the sections that land and want tying together) and clears out of
    // the VERSE (the open question, stated bare) and the BRIDGE (the departure — its thinness, now
    // with no pad either, is the texture contrast that marks the phase change). This is the teacher's
    // law — pad on the chorus and the home outro, never every bar — so a busy arrival reads as ONE
    // settled idea instead of a continuous mid-register wash that masks the lead/keys exchange.
    let mut pad = Vec::new();
    if perf.stage.on_stage_span(Agent::Pad, 0.0, score.total_beats) {
        for bar in 0..total_bars {
            let e = bar / bars_per_episode;
            if !argument.pad_all_sections
                && !matches!(
                    argument.placements[e].relation,
                    super::argument::ArgumentRelation::Consequent { .. }
                        | super::argument::ArgumentRelation::Return { .. }
                )
            {
                continue; // verse and bridge clear the floor for the lead — UNLESS a lead-free
                          // smooth-pop band opts into a pad bed everywhere (the soft continuous rug
                          // that smooths the vibe). C137 never sets the flag, so its render is frozen.
            }
            let bar_start = bar as f64 * 4.0;
            let ctx = perf.context_at(bar_start).unwrap();
            let energy = drive_energy(harmonic_drive(&ctx.chord, world.tonic_pc));
            let swell = groove_swell(bar, bars_per_episode);
            let dur = (4.0_f64).min(score.total_beats - bar_start) as f32;
            // Voice the committed chord tones ascending from a mid-register floor, so the rug fills
            // the harmonic middle instead of the muddy low end the bass already owns.
            let tones = ctx.chord.pitch_classes();
            let mut low = 50;
            for &pc in &tones {
                let pitch = register(pc, low);
                low = pitch + 1;
                pad.push(pitched(
                    &perf,
                    Role::Pad,
                    bar_start,
                    dur,
                    pitch,
                    (0.32 * energy * swell).clamp(0.02, 1.0),
                    provenance(&perf, bar_start, "pad", section_at(argument, bar_start)),
                ));
            }
        }
    }
    score = commit_role(score, &perf, world, Role::Pad, pad, Vec::new())?;
    perf.budget = super::budget::allocate(&perf);
    score
        .drums
        .sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    let take = CommittedSong {
        composition: Composition {
            score,
            perf,
            song: argument.song.clone(),
        },
        hearings,
        source: argument.clone(),
        world: world.clone(),
    };
    take.validate_candidate(take.score())?;
    Ok(take)
}

/// The long-form analogue of [`validate`]: the same event/source/stage/foundation contract over the
/// N-episode domain, with keys admitted inside ANY episode's answer window `[e*16+4, e*16+8)`. The
/// Score must be exactly as long as the source's form (an integer number of 16-beat episodes).
fn validate_song(
    score: &Score,
    perf: &PerformancePlan,
    source: &CompiledArgument,
    world: &MusicWorld,
) -> Result<(), KernelError> {
    score.validate().map_err(KernelError::InvalidScore)?;
    if score.tempo_bpm != world.tempo_bpm
        || score.total_beats != source.song.plan.form.total_beats
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
        if n.role == Role::Keys {
            if n.prov.role_note == "comp" {
                // A comp stab lives outside the answer window, but it must make ROOM for the lead —
                // never landing on a lead attack (the keys stab the holes, they don't fight the melody).
                if score
                    .role_notes(Role::Lead)
                    .any(|l| (l.start_beat - n.start_beat).abs() < 0.12)
                {
                    return Err(KernelError::Source("keyboard comp collides with the lead"));
                }
            } else if n.prov.role_note == "argument" {
                // A KEYS CARRIER note (the narrative bridge handoff): the keys OWN this episode's
                // thematic obligation, witnessed note-for-note by the argument content witness. It is
                // therefore exempt from the answer-window keyboard-silence rule, which governs comps
                // and answers — not carriage. The exemption is safe because `longform_domain` admits a
                // Keys carrier ONLY on a Depart, so no home-arrival or earned-return window is touched.
            } else {
                // The call/response answer stays confined to the episode's second bar — the deliberate
                // keyboard silence the rest of the answer-contour design depends on.
                let within_episode = n.start_beat.rem_euclid(EPISODE_BEATS);
                if !(4.0..8.0).contains(&within_episode) {
                    return Err(KernelError::Source("violated deliberate keyboard silence"));
                }
            }
        }
    }
    for bar in 0..(score.total_beats / 4.0).round() as i64 {
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
    // The causal ledger is closed under referential identity: every committed event that claims an
    // interaction resolves to a real (call -> response) row, and the event actually realizes that
    // response's action — so a quoted answer cannot reference an interaction that was never planned,
    // nor claim one whose response it does not perform.
    for n in &score.notes {
        if let Some(iid) = n.prov.interaction {
            let row =
                perf.interactions
                    .iter()
                    .find(|row| row.id == iid)
                    .ok_or(KernelError::Source(
                        "event references an unregistered interaction",
                    ))?;
            let answered = row
                .response
                .and_then(|r| r.action)
                .is_some_and(|a| n.prov.actions.has(a));
            if !answered {
                return Err(KernelError::Source(
                    "event claims an interaction whose response it does not realize",
                ));
            }
        }
    }
    Ok(())
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

    /// R0 / G5 — a tonic-root dominant seventh is an ACTIVE pivot, not tonic rest. `harmonic_drive`
    /// must read the dominant function, not just whether the root equals the global tonic pitch
    /// class. The current 8-bar Am verse/hook never voices an A7, so this is a latent bug that the
    /// coming bridge (A7 = V7/iv → Dm) would expose; the fix is sound-preserving on the current
    /// route (its only tonic-root chord is Am, still Home).
    #[test]
    fn a_tonic_root_dominant_is_not_mistaken_for_home() {
        let tonic = 9; // A
        let a_minor = Chord::new(tonic, Quality::Min);
        let a_dom7 = Chord::new(tonic, Quality::Dom7);
        assert_eq!(
            harmonic_drive(&a_minor, tonic),
            HarmonicDrive::Home,
            "a static tonic minor is Home"
        );
        assert_ne!(
            harmonic_drive(&a_dom7, tonic),
            HarmonicDrive::Home,
            "a tonic-root dominant seventh is an active pivot, not Home"
        );
        assert_eq!(
            harmonic_drive(&a_dom7, tonic),
            HarmonicDrive::Driving,
            "a dominant seventh drives toward its target, wherever its root sits"
        );
    }

    /// R1 — the harmonic closure law is domain-specific. The C137 arc law demands a literal-tonic
    /// arrival on its pinned bars; the smooth-pop law demands a repeating groove-cycle instead. Each
    /// admits its own kind and rejects the other, so opting a source into the pop domain can never
    /// silently relax the C137 tonic guard.
    #[test]
    fn the_c137_and_pop_route_laws_are_distinct_closure_conditions() {
        use crate::audio::human_music::argument::PopSourceGrammar;
        // A canonical C137 16-bar route: pre-dominant → dominant → tonic, hook home at bar 7, home end.
        let mut c137 = vec![(0, Quality::Min); 16];
        c137[2] = (7, Quality::Dom7);
        assert!(
            c137_route_law(&c137).is_ok(),
            "the canonical C137 route is admitted by its own law"
        );

        // An Aeolian-diatonic smooth-pop loop Am9–Fmaj7–Cmaj7–G6 (i–VI–III–VII), two 4-bar cycles.
        // Region-relative offsets above A: F=+8, C=+3, G=+10. No (7,Dom7) at bar 2; never (0,Min) at 3.
        let loop4 = [
            (0, Quality::Min9),
            (8, Quality::Maj7),
            (3, Quality::Maj7),
            (10, Quality::Maj6),
        ];
        let pop: Vec<(i32, Quality)> = (0..8).map(|i| loop4[i % 4]).collect();
        let grammar = PopSourceGrammar { loop_bars: 4 };

        assert!(
            pop_route_law(&pop, &grammar).is_ok(),
            "a periodic pop loop closes its groove-cycle"
        );
        assert!(
            c137_route_law(&pop).is_err(),
            "the pop loop never lands the C137 tonic pins — the C137 guard is not relaxed"
        );
    }

    /// R1 — the pop law is a real, falsifiable gate: a route that silently DRIFTS off its declared
    /// loop (a fabricated progression) is refused, and a length that is not a whole number of cycles
    /// is refused.
    #[test]
    fn the_pop_route_law_refuses_a_drifting_or_mis_sized_loop() {
        use crate::audio::human_music::argument::PopSourceGrammar;
        let grammar = PopSourceGrammar { loop_bars: 4 };
        let loop4 = [
            (0, Quality::Min9),
            (8, Quality::Maj7),
            (3, Quality::Maj7),
            (10, Quality::Maj6),
        ];
        let mut drift: Vec<(i32, Quality)> = (0..8).map(|i| loop4[i % 4]).collect();
        drift[5] = (5, Quality::Maj7); // bar 5 must equal bar 1 (the F); break the cycle
        assert!(
            pop_route_law(&drift, &grammar).is_err(),
            "a drifting loop fails groove-cycle closure"
        );
        let short: Vec<(i32, Quality)> = (0..6).map(|i| loop4[i % 4]).collect(); // 6 % 4 != 0
        assert!(
            pop_route_law(&short, &grammar).is_err(),
            "a non-whole-cycle length is refused"
        );
    }

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
