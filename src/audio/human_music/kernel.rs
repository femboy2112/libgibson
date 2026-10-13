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
        if p.anchor_semitones != 0
            || !p.section_gain.is_finite()
            || !(0.0 < p.section_gain && p.section_gain <= 1.0)
            || p.carriers != [Agent::Lead]
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
    // The verse travels pre-dominant → dominant → tonic and hangs open on its own cadence; the
    // first hook lands home on the tonic at bar 7; and the SONG lands home on the tonic at its final
    // bar (the Resolved ending's earned home-coming). Region-relative, reconstructed from the route.
    if argument.route[2] != (7, Quality::Dom7)
        || argument.route[3] != (0, Quality::Min)
        || argument.route[7] != (0, Quality::Min)
        || argument.route[argument.route.len() - 1] != (0, Quality::Min)
    {
        return Err(KernelError::Source(
            "route must travel to its dominant and land the hook home",
        ));
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
        if !matches!(transform, Transform::Silence) {
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
                                n.start_beat >= bridge_ws && n.start_beat < bridge_ws + EPISODE_BEATS
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
                        overlap: false,
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
    for bar in 0..total_bars {
        let at = bar as f64 * 4.0;
        let ctx = perf.context_at(at).unwrap();
        let drive = harmonic_drive(&ctx.chord, world.tonic_pc);
        let energy = drive_energy(drive);
        let tones = ctx.chord.pitch_classes();
        let root = register(ctx.chord.root_pc, 36);
        let fifth = register(tones[2.min(tones.len().saturating_sub(1))], 36);
        let third = register(tones[1.min(tones.len().saturating_sub(1))], 36);
        // Beat four leads by a half-step into the next bar's root; the final bar has no successor,
        // so it settles on the fifth rather than leading away from home.
        let beat_four = match (bar + 1 < total_bars)
            .then(|| register(perf.context_at(at + 4.0).unwrap().chord.root_pc, 36))
        {
            Some(next) if next > 36 => next - 1,
            Some(next) => next + 1,
            None => fifth,
        };
        // At HOME the bass lays back: the root rings and the fifth answers on beat three, leaving the
        // tonic room to breathe (the fifth is clipped short of beat four, clearing any planned seam
        // pickup). Everywhere else it walks the changes. One monophonic line in the bass register —
        // motion that voices the harmony, never a doubled or sub-octave weave.
        let figure: Vec<(f64, i32, f32, f32)> = match drive {
            HarmonicDrive::Home => vec![(0.0, root, 1.8, 0.70), (2.0, fifth, 1.4, 0.52)],
            _ => vec![
                (0.0, root, 0.8, 0.76),
                (1.0, fifth, 0.45, 0.56),
                (2.0, third, 0.45, 0.60),
                (3.0, beat_four.clamp(36, 55), 0.45, 0.54),
            ],
        };
        let swell = groove_swell(bar, bars_per_episode);
        for (offset, pitch, dur, velocity) in figure {
            // The grid position labels the section/phrase; the SHARED pocket leans the attack off it
            // (the downbeat root at `offset==0` stays exactly on `at`, the required anchor), and the
            // velocity rides the harmonic energy and the phrase swell. The bass orbits the SAME
            // pocket as the drums and the lead, so coincident attacks lock — one solar system.
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
                velocity * energy * swell,
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
    for bar in 0..total_bars {
        let bar_start = bar as f64 * 4.0;
        let drive = harmonic_drive(&perf.context_at(bar_start).unwrap().chord, world.tonic_pc);
        let energy = drive_energy(drive);
        // A fill lands because the NEXT bar opens a new phrase (each episode seam, then the final bar).
        let fill_bar = (bar + 1) % bars_per_episode == 0;
        // The whole kit's dynamics ride the harmonic energy AND the within-phrase swell; the `lay`
        // leans a stroke off the grid into the pocket (0 = dead-on, the time reference).
        let swell = groove_swell(bar, bars_per_episode);
        let mut push = |voice: DrumVoice, grid: f64, vel: f32, label: &'static str, lay: f64| {
            score.drums.push(DrumHit {
                start_beat: grid + lay,
                voice,
                velocity: (vel * energy * swell).clamp(0.02, 1.0),
                prov: provenance(&perf, grid, label, section_at(argument, grid)),
            });
        };
        // The steady spine — always present, locking the shared time. The downbeat kick and the hats
        // are the dead-on time reference; the backbeat snare (and the beat-three kick) DRAG behind
        // them by the SAME shared pocket the bass and lead use, so the whole band locks as one body.
        let pocket = super::theme_family::pocket_offset;
        push(DrumVoice::Kick, bar_start, 0.95, "pulse", 0.0);
        push(DrumVoice::Kick, bar_start + 2.0, 0.78, "pulse", pocket(2.0));
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
        // The ride carpet: its SUBDIVISION follows the harmonic drive — eighths at rest, sixteenths in
        // motion — with the down/and/e-a accent hierarchy and the hat opening on the "and of four".
        let sixteenths = !matches!(drive, HarmonicDrive::Home);
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
        // The conversation: comp strokes dropped into the off-beats the lead left open, up to the
        // budget the harmonic drive allows (home lays out; building comps; a dominant drives hardest).
        // A kick leans on the "ands" (a syncopated push with the bass), a ghost snare on the "a" — but
        // ONLY where the lead is not attacking (make room), softer inside a live phrase and fuller
        // where the phrase has yielded. This is the make-room law that turns competing voices into
        // interplay, and it reads the lead only, so it never perturbs the keys-independent pulse.
        let budget = match drive {
            HarmonicDrive::Home => 0,
            HarmonicDrive::Building => 2,
            HarmonicDrive::Driving => 3,
        };
        let mut placed = 0;
        for s in [2u32, 6, 10, 14, 7, 11] {
            if placed >= budget {
                break;
            }
            let at = bar_start + s as f64 * 0.25;
            if !lead_occ.allows_comp_at(at, &lead_notes) {
                continue; // the lead is speaking here — leave the space
            }
            let (voice, base) = if s % 4 == 2 {
                (DrumVoice::Kick, 0.56) // on the "and"
            } else {
                (DrumVoice::Snare, 0.40) // a ghost on the "a"
            };
            let vel = if lead_occ.yields_at(at) {
                base
            } else {
                base * 0.5
            };
            push(voice, at, vel, "comp", pocket(at - bar_start)); // comps ride the shared pocket
            placed += 1;
        }
        // Phrase-end fill: rising sixteenth snares across the last half-bar because the phrase is
        // ending (into the hook, then into the final cadence) — not on a coin flip. Fills are tight
        // (on the grid) so they drive cleanly into the downbeat.
        if fill_bar {
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
    // --- PAD: the RUG. A sustained chordal bed that ties the room together under the band — the
    // committed harmony voiced as a warm MID-register chord (above the bass, under the lead and the
    // keys' accents, never competing), re-voiced each bar as the chord changes and held across it,
    // soft, riding the same shared energy so it breathes with the band (eased at home, leaning in on
    // the bridge). It is the one voice the kernel voices POLYPHONICALLY (the synth's pad pool is six
    // voices); it carries no argument and no continuity line, so it is exempt from the monophonic
    // lane and seam laws. It only sounds where the arrangement seats the pad on stage — the
    // two-episode verse→hook control keeps the pad silent, so its render stays byte-identical.
    let mut pad = Vec::new();
    if perf.stage.on_stage_span(Agent::Pad, 0.0, score.total_beats) {
        for bar in 0..total_bars {
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
            let within_episode = n.start_beat.rem_euclid(EPISODE_BEATS);
            if !(4.0..8.0).contains(&within_episode) {
                return Err(KernelError::Source("violated deliberate keyboard silence"));
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
