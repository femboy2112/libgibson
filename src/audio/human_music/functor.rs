//! `F_skin : 𝒮 → 𝓜` — the (lax-monoidal) functor that turns a semantic trace into a
//! [`Score`], parameterized by a [`MusicWorld`]. This is where simultaneous semantic layers
//! (tone + emphasis + density + elevation) become simultaneous musical voices (harmony +
//! bass + drums + melody) via a shared coherence map (the form's energy/tension curve and
//! the one progression), rather than independent sums.
//!
//! Walking the trace, each semantic morphism maps to a sequence of intent morphisms that
//! evolve a running [`MusicIntent`]; those choices drive motif development, SFX placement
//! and local density, and their labels become event provenance. Skins are natural
//! transformations: swap the world and the form/motif/resolutions stay; the dialect changes.

use super::contract::{CoherenceContract, CompositionGrammar};
use super::discourse::DiscourseRole;
use super::form::{Section, SectionKind, BEATS_PER_BAR};
use super::groove::GrooveEngine;
use super::harmony::{ChordSpan, HarmonyEngine};
use super::intent::{IntentMorphism, MusicIntent};
use super::motif::{MotifBank, ThematicTrajectory};
use super::plan::CompositionPlan;
use super::score::{Note, PitchFunction, Provenance, Role, Score, SfxEvent, SfxKind};
use super::semantic::{EventKind, SemanticTrace, Tone};
use super::theory::{pitch_class, Midi, Scale};
use super::timeline::IntentTimeline;
use super::voicing::VoiceLeader;
use super::world::MusicWorld;

/// Compose a full score for `trace` under `world`, deterministic in `seed`.
pub fn compose(trace: &SemanticTrace, world: &MusicWorld, seed: u64) -> Score {
    compose_with_plan(trace, world, seed).0
}

/// Like [`compose`], but also returns the [`CompositionPlan`] the score was realized from —
/// for structural dumps (`plan.dump()`) and coherence diagnostics.
///
/// Round III makes the [`CompositionPlan`] the **sole** compositional authority. The semantic
/// trace becomes a causal [`IntentTimeline`]; the timeline yields one plan (contract, form graph,
/// discourse and arrangement); and every realizer reads its per-phrase
/// [`super::plan::PhraseTarget`] from that plan — harmony, groove and bass no longer consult a
/// parallel `Form`. The legacy [`Section`] list is *projected* from the plan for the Score IR, so
/// the summary and the per-event provenance finally describe the same decomposition;
/// `apply_arrangement` then gates every voice by its per-phrase role.
pub fn compose_with_plan(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
) -> (Score, CompositionPlan) {
    let total_bars = ((trace.total_beats / BEATS_PER_BAR).round() as u32).max(1);
    let timeline = IntentTimeline::walk(trace);
    let plan = CompositionPlan::build(&timeline, total_bars);
    let score = realize(trace, world, seed, &plan);
    (score, plan)
}

/// Like [`compose_with_plan`], but the grammar is **chosen**, not inferred — the calibration path.
/// The plan is built under [`CoherenceContract::for_grammar`], so the piece exercises that grammar's
/// `ResolutionPolicy` (Functional cadences vs a Loop's cyclic return vs a modal pedal), budgets and
/// anchors, whatever the trace shape would otherwise infer.
pub fn compose_with_grammar(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
    grammar: CompositionGrammar,
) -> (Score, CompositionPlan) {
    let total_bars = ((trace.total_beats / BEATS_PER_BAR).round() as u32).max(1);
    let timeline = IntentTimeline::walk(trace);
    let plan = CompositionPlan::build_with_contract(
        &timeline,
        total_bars,
        CoherenceContract::for_grammar(grammar),
    );
    let score = realize(trace, world, seed, &plan);
    (score, plan)
}

/// Realize a score from a finished plan: every realizer reads the plan's per-phrase targets under
/// `world`, then `apply_arrangement` gates the voices. Shared by the inferred and grammar-forced
/// compose paths so both go through exactly the same realization.
fn realize(trace: &SemanticTrace, world: &MusicWorld, seed: u64, plan: &CompositionPlan) -> Score {
    let scale = Scale::new(world.tonic_pc, world.mode);
    let targets = plan.targets();

    let mut harmony = HarmonyEngine::new(world, seed);
    // Under DeflectedLift the harmony IS a recurring Lift->Deflect->Open->Reset cell (the audible
    // spine); other grammars use the phrase-scope cadential engine (backbone = None).
    let backbone = super::backbone::plan_for(
        plan.contract.grammar,
        world,
        seed,
        plan.contract.recurrence_bars,
    );
    let chords =
        harmony.generate_with_backbone(&targets, plan.contract.resolution, backbone.as_ref());

    let mut groove = GrooveEngine::new(world, seed);
    let gr = groove.generate(&targets);

    let total_beats = plan.form.total_bars as f64 * BEATS_PER_BAR;
    let mut score = Score::new(world.tempo_bpm, BEATS_PER_BAR, total_beats);
    score.sections = sections_from_plan(plan);
    score.chords = chords.clone();
    score.drums = gr.hits;

    // --- Comp: sustained pad bed + sparse groove-locked keys arpeggio. ---
    add_comp(&mut score, &chords, world);
    // --- Bass: persistent kick-locked figure. ---
    add_bass(&mut score, &chords, &gr.kick_beats, plan, &scale);
    // --- Melody: the thesis motif transformed by each phrase's discourse role. ---
    add_melody(&mut score, &chords, plan, &scale, seed);
    // --- SFX + intent morphisms from significant semantic events. ---
    add_sfx_and_provenance(&mut score, trace, plan);
    // --- Arrangement: gate every voice by its per-phrase role and stamp real provenance. ---
    apply_arrangement(&mut score, plan);
    score
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

/// Realize the [`ArrangementPlan`] onto a generated score: drop voices that are silent in
/// their phrase, scale surviving events by their arrangement role's dynamic, and stamp each
/// event's real phrase/family/role/obligation provenance from the plan. This is the pass
/// that turns "everyone plays all the time" into an arrangement with foreground, support and
/// negative space.
fn apply_arrangement(score: &mut Score, plan: &CompositionPlan) {
    let form = &plan.form;
    let arr = &plan.arrangement;

    score.notes.retain_mut(|n| {
        let phrase = *form.phrase_at(n.start_beat);
        let role = arr.at(phrase.ix as usize).role_for(n.role);
        if !role.is_audible() {
            return false;
        }
        n.velocity = (n.velocity * role.gain()).clamp(0.02, 1.0);
        n.prov.section = phrase.family.to_section_kind();
        n.prov.phrase = Some(phrase.ix);
        n.prov.family = Some(phrase.family.label());
        n.prov.role_kind = Some(role.label());
        let goal = plan.discourse.goal(phrase.ix as usize);
        n.prov.role = Some(goal.role.label());
        n.prov.closure = Some(goal.closure.label());
        true
    });

    score.drums.retain_mut(|d| {
        let phrase = *form.phrase_at(d.start_beat);
        let role = arr.at(phrase.ix as usize).drums;
        if !role.is_audible() {
            return false;
        }
        d.velocity = (d.velocity * role.gain()).clamp(0.02, 1.0);
        d.prov.section = phrase.family.to_section_kind();
        d.prov.phrase = Some(phrase.ix);
        d.prov.family = Some(phrase.family.label());
        d.prov.role_kind = Some(role.label());
        let goal = plan.discourse.goal(phrase.ix as usize);
        d.prov.role = Some(goal.role.label());
        d.prov.closure = Some(goal.closure.label());
        true
    });

    // SFX are punctuation tied to semantic beats — keep them, but restamp section/phrase so
    // the whole IR agrees with the plan.
    for e in &mut score.sfx {
        let phrase = *form.phrase_at(e.start_beat);
        e.prov.section = phrase.family.to_section_kind();
        e.prov.phrase = Some(phrase.ix);
        e.prov.family = Some(phrase.family.label());
        let goal = plan.discourse.goal(phrase.ix as usize);
        e.prov.role = Some(goal.role.label());
        e.prov.closure = Some(goal.closure.label());
    }
}

/// The pitch with class `pc` nearest to `center`.
fn pitch_near(pc: i32, center: Midi) -> Midi {
    let base = (center / 12) * 12 + pc.rem_euclid(12);
    [base - 12, base, base + 12]
        .into_iter()
        .min_by_key(|p| (p - center).abs())
        .unwrap()
}

/// Comp = a sustained harmonic bed (pad) plus a SPARSE, groove-locked rhythmic arpeggio
/// (keys). Round I stabbed the keys on *every* beat and flipped a coin for single-vs-chord,
/// so keys alone were ~300 events fighting everything else. Here the pad holds the chord and
/// the keys play single voices on the offbeats ("and of 1", "and of 3") — an interlocking
/// pattern derived from the bar grid, deterministic, roughly two hits a bar. The arrangement
/// pass gates and dynamically scales all of it afterward.
fn add_comp(score: &mut Score, chords: &[ChordSpan], world: &MusicWorld) {
    let mut pad_vl = VoiceLeader::new(52, 79, world.voicing_spread);
    let mut keys_vl = VoiceLeader::new(58, 84, world.voicing_spread);

    for span in chords {
        let prov = Provenance {
            role_note: "comp",
            ..Provenance::new(SectionKind::A)
        };
        // Pad: hold the whole voicing for the chord's duration (the harmonic bed).
        let pad = pad_vl.lead(&span.chord, 4, 67);
        for &p in &pad.voices {
            let mut note = Note::new(
                span.start_beat,
                span.dur_beats * 0.98,
                p,
                (0.4 * world.base_dynamic).clamp(0.05, 1.0),
                Role::Pad,
                prov,
            );
            // Pad voices come strictly from the chord (the voice leader draws from its pitch
            // classes) — verify rather than assume, so a future non-chord tone would surface as
            // unjustified instead of hiding as "background".
            note.function = span
                .chord
                .contains_pc(pitch_class(p))
                .then_some(PitchFunction::ChordTone);
            score.notes.push(note);
        }
        // Keys: a comping VOICE, not a single-note arp. Each stab is a 2-3 note upper-structure
        // shell drawn from the top of the voicing — so it carries the chord's guide tones and any
        // licensed color (the 7ths/9ths/6ths the backbone now supplies) rather than one bare pitch.
        // Stabs land syncopated (the "and" of 1 and 3), alternating a fuller and a thinner voicing
        // for rhythmic life, with an anticipation push into the next chord on longer spans. The pad
        // holds the bed underneath; the arrangement pass gives keys the foreground in the B phrases
        // where the lead rests, so the two converse rather than pile up.
        let voicing = keys_vl.lead(&span.chord, 4, 72);
        if voicing.voices.is_empty() {
            continue;
        }
        let dur = span.dur_beats as f64;
        // The top three voices, high-to-low — an upper-structure shell.
        let shell: Vec<Midi> = voicing.voices.iter().rev().take(3).copied().collect();
        let mut positions: Vec<f64> = Vec::new();
        let mut off = 0.5f64;
        while off < dur - 1e-6 {
            positions.push(off);
            off += 2.0;
        }
        // A syncopated push just before the chord change (an anticipation) on longer spans.
        if dur >= 3.0 {
            positions.push(dur - 0.5);
        }
        for (si, &pos) in positions.iter().enumerate() {
            // Alternate a full shell and a thinner two-note stab so the comp breathes.
            let take = if si % 2 == 0 {
                shell.len()
            } else {
                2.min(shell.len())
            };
            for &p in shell.iter().take(take) {
                let mut note = Note::new(
                    span.start_beat + pos,
                    0.4,
                    p,
                    (0.35 * world.base_dynamic).clamp(0.05, 1.0),
                    Role::Keys,
                    prov,
                );
                note.function = span
                    .chord
                    .contains_pc(pitch_class(p))
                    .then_some(PitchFunction::ChordTone);
                score.notes.push(note);
            }
        }
    }
}

/// Bass = a persistent, deterministic figure locked to the kick and the harmony. Round I
/// re-rolled root/fifth/approach with a coin at every kick, so the line never settled into a
/// figure. Here the role of each kick is fixed by its position: the root states the chord on
/// the span's first kick, a fifth drives the offbeat kicks at high energy, and the last kick
/// before a chord change steps chromatically into the next root — a repeatable shape, not a
/// dice roll, still onset-locked to the groove.
/// Max beats a single chromatic approach may sound before its target: beyond this it stops reading
/// as a pickup and becomes a sustained chromatic tone under the old chord (the R4 defect).
const BASS_APPROACH_MAX: f64 = 1.0;
/// Room (beats) before a chord change at which the bass WALKS into it (a stepwise SlidePath line)
/// rather than stating a single short approach.
const BASS_WALK_MIN: f64 = 1.5;
/// The step of a walking bass note (beats).
const BASS_WALK_STEP: f64 = 0.5;

fn add_bass(
    score: &mut Score,
    chords: &[ChordSpan],
    kick_beats: &[f64],
    plan: &CompositionPlan,
    scale: &Scale,
) {
    let bass_center = 40; // ~E2
    for (ci, span) in chords.iter().enumerate() {
        let span_end = span.start_beat + span.dur_beats as f64;
        let root_pc = span.chord.root_pc;
        let root = pitch_near(root_pc, bass_center);
        // The chord's ACTUAL fifth: the chord tone nearest a perfect fifth above the root — 7
        // semitones for major/minor, but 6 for a diminished chord. `root_pc + 7` played a
        // NON-chord tone on vii°/dim (a wrong note the diagnostics never saw); take the real one.
        let fifth_pc = span
            .chord
            .pitch_classes()
            .into_iter()
            .min_by_key(|&pc| ((pc - root_pc).rem_euclid(12) - 7).abs())
            .unwrap_or((root_pc + 7).rem_euclid(12));
        let fifth = pitch_near(fifth_pc, bass_center);
        let next_root_pc = chords
            .get(ci + 1)
            .map(|c| c.chord.root_pc)
            .unwrap_or(root_pc);
        let next_root = pitch_near(next_root_pc, bass_center);
        let energy = {
            let ph = plan.form.phrase_at(span.start_beat);
            plan.discourse.goal(ph.ix as usize).energy_target
        };

        // Kick-locked bass: a bass note on each kick within the span.
        let kicks: Vec<f64> = kick_beats
            .iter()
            .copied()
            .filter(|&k| k >= span.start_beat - 1e-6 && k < span_end - 1e-6)
            .collect();
        if kicks.is_empty() {
            // Fallback: at least the downbeat root.
            push_bass(
                score,
                span.start_beat,
                span.dur_beats.min(2.0),
                root,
                energy,
                "root",
                PitchFunction::ChordTone,
            );
            continue;
        }
        let n = kicks.len();
        for (i, &k) in kicks.iter().enumerate() {
            let is_last = i + 1 == n;
            let room = span_end - k; // beats from this kick to the chord change
            let approaching = is_last && span_end < score.total_beats - 1e-6;

            // Enough room to WALK into the next chord: a stepwise scale line resolving on a
            // chromatic approach to the next root. This replaces the old single tone that could
            // sustain most of a bar as static chromatic dissonance.
            if approaching && room >= BASS_WALK_MIN {
                push_bass_walk(score, scale, root, next_root, k, span_end, energy);
                continue;
            }

            let default_dur = kicks
                .get(i + 1)
                .map(|&nx| (nx - k) as f32)
                .unwrap_or((span_end - k) as f32)
                .clamp(0.1, 2.0);

            let (pitch, note, func, dur) = if approaching && room <= BASS_APPROACH_MAX {
                // A short chromatic pickup into the next root — bounded so it reads as an approach.
                let dir = (next_root - root).signum().clamp(-1, 1);
                let dir = if dir == 0 { -1 } else { dir };
                (
                    next_root - dir,
                    "approach",
                    PitchFunction::ChromaticApproach,
                    room as f32,
                )
            } else if i == 0 {
                (root, "root", PitchFunction::ChordTone, default_dur)
            } else if energy > 0.55 && i % 2 == 1 {
                // Fifth on the offbeat kicks when there's drive.
                (fifth, "fifth", PitchFunction::ChordTone, default_dur)
            } else {
                (root, "root", PitchFunction::ChordTone, default_dur)
            };
            push_bass(score, k, dur * 0.9, pitch, energy, note, func);
        }
    }
}

/// A stepwise walking bass from `from` toward `next_root`, filling `[start, end)`: scale-tone
/// intermediates ([`PitchFunction::SlidePath`] — their justification is the path, not the local
/// chord) resolving onto a final chromatic approach a semitone from the next root
/// ([`PitchFunction::ChromaticApproach`]). Contiguous and bounded — an idiomatic lead-in, not a
/// held dissonance.
fn push_bass_walk(
    score: &mut Score,
    scale: &Scale,
    from: Midi,
    next_root: Midi,
    start: f64,
    end: f64,
    energy: f32,
) {
    let dir = (next_root - from).signum();
    let dir = if dir == 0 { -1 } else { dir };
    let target = next_root - dir; // the chromatic approach pitch (a semitone off the next root)
    let steps = (((end - start) / BASS_WALK_STEP).floor() as usize).max(2);
    for s in 0..steps {
        let t0 = start + s as f64 * BASS_WALK_STEP;
        if t0 >= end - 1e-6 {
            break;
        }
        let t1 = (t0 + BASS_WALK_STEP).min(end);
        let last = s + 1 == steps;
        let (pitch, note, func) = if last {
            (target, "approach", PitchFunction::ChromaticApproach)
        } else {
            let frac = (s + 1) as f64 / steps as f64;
            let interp = from as f64 + (target - from) as f64 * frac;
            (
                scale.nearest_scale_pitch(interp.round() as i32),
                "walk",
                PitchFunction::SlidePath,
            )
        };
        push_bass(score, t0, (t1 - t0) as f32 * 0.9, pitch, energy, note, func);
    }
}

fn push_bass(
    score: &mut Score,
    at: f64,
    dur: f32,
    pitch: Midi,
    energy: f32,
    note: &'static str,
    func: PitchFunction,
) {
    let mut n = Note::new(
        at,
        dur.max(0.1),
        pitch,
        (0.6 + 0.35 * energy).clamp(0.1, 1.0),
        Role::Bass,
        Provenance {
            role_note: note,
            ..Provenance::new(SectionKind::A)
        },
    );
    n.function = Some(func);
    score.notes.push(n);
}

/// The lead voice, developed along a whole-piece [`ThematicTrajectory`].
///
/// Each phrase's material is DEVELOPED from the previous statement rather than re-derived from a
/// fixed germ, so consecutive phrases flow as one song instead of splicing (Round IV's
/// "kaleidoscope"). Establish/Restate/Return come home to the thesis M0; Depart/Intensify develop
/// the current material a step further; **Question** poses a call (a fragment, remembering where it
/// broke off) and the matching **Answer** completes exactly that remainder; Culminate sounds the
/// call+response hook; Dissolve evaporates. Every transition carries a typed [`super::motif::Handoff`].
/// Statements enter grid-aligned and are realized jointly against the harmony via
/// [`super::motif::realize_phrase`]. The lead plays only where the arrangement gives it a voice — so
/// it breathes.
fn add_melody(
    score: &mut Score,
    chords: &[ChordSpan],
    plan: &CompositionPlan,
    scale: &Scale,
    seed: u64,
) {
    let bank = MotifBank::generate(scale, seed ^ 0x3E10_D1E5);
    // The whole-piece thematic line: each phrase's material is DEVELOPED from the previous
    // statement (with explicit returns to the thesis), not re-derived afresh from a fixed germ — so
    // consecutive phrases flow as one song instead of splicing. The trajectory owns the call/answer
    // coupling internally and reports a typed handoff per statement.
    let mut traj = ThematicTrajectory::new(&bank);
    let two_bar = 2.0 * BEATS_PER_BAR;
    // The previous statement's exit pitch — carried across statements and phrases so each new
    // statement connects to where the last one ended (continuity, not teleportation).
    let mut prev_exit: Option<Midi> = None;

    for t in plan.targets() {
        let phrase = t.phrase;
        // The melody breathes: it sounds only where the arrangement gives the lead a voice, and the
        // thematic trajectory develops along that audible line (consecutive audible statements are
        // adjacent development steps, which is what keeps the line coherent rather than drifting).
        if !plan.arrangement.at(phrase.ix as usize).lead.is_audible() {
            continue;
        }
        let (motif, handoff) = traj.next_for(t.goal.role);
        let morph = handoff.label();

        // Register realizes the role: the culmination and intensification climb, the dissolve
        // settles low, everything else follows the phrase's elevation target. For ordinary roles we
        // then nudge the octave to CONNECT to the previous statement's exit (flow between phrases);
        // the culmination and any licensed rupture keep their dramatic leap (Depart/Intensify/
        // Question/Answer/Return must not sound like edits between unrelated songs — Culminate may).
        let base_octave = match t.goal.role {
            DiscourseRole::Culminate | DiscourseRole::Intensify => 5,
            DiscourseRole::Dissolve => 3,
            _ => 4 + (t.goal.register_target > 0.65) as i32,
        };
        let octave = match prev_exit {
            Some(pe) if !phrase.is_rupture && t.goal.role != DiscourseRole::Culminate => {
                let first_deg = motif.degrees.first().copied().unwrap_or(0);
                [base_octave - 1, base_octave, base_octave + 1]
                    .into_iter()
                    .min_by_key(|&o| (scale.degree_pitch(first_deg, o) - pe).abs())
                    .unwrap_or(base_octave)
            }
            _ => base_octave,
        };

        let stmt_beats = motif.total_beats() as f64;
        if stmt_beats < 1e-6 {
            continue;
        }
        let phrase_end = phrase.end_beat();
        let mut at = phrase.start_beat();
        let mut guard = 0;
        while at + stmt_beats <= phrase_end + 1e-6 && guard < 32 {
            let (notes, reps) = super::motif::realize_phrase_reporting(
                &motif, chords, scale, 0, octave, at, prev_exit, 4,
            );
            score.melody_repairs += reps;
            if let Some(&(_, _, last_pitch, _)) = notes.last() {
                prev_exit = Some(last_pitch);
            }
            for (nb, dur, pitch, function) in notes {
                let mut note = Note::new(
                    nb,
                    (dur * 0.9).max(0.1),
                    pitch,
                    (0.55 + 0.4 * t.goal.energy_target).clamp(0.1, 1.0),
                    Role::Lead,
                    Provenance {
                        motif_id: Some(motif.id),
                        motif_xform: Some(morph),
                        anchor: Some("motif"),
                        role_note: "melody",
                        ..Provenance::new(phrase.family.to_section_kind())
                    },
                );
                note.function = function;
                score.notes.push(note);
            }
            // Breathe to the next 2-bar boundary — grid-aligned rest, not a random gap.
            let after = at + stmt_beats;
            let boundary = (after / two_bar).ceil() * two_bar;
            at = if boundary > after + 1e-6 {
                boundary
            } else {
                after
            };
            guard += 1;
        }
    }
}

fn add_sfx_and_provenance(score: &mut Score, trace: &SemanticTrace, plan: &CompositionPlan) {
    let mut prev = trace.events.first().map(|e| e.state);
    for ev in &trace.events {
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
        let sec_kind = plan.form.phrase_at(ev.at_beat).family.to_section_kind();
        score.sfx.push(SfxEvent {
            start_beat: ev.at_beat,
            kind,
            velocity: ev.state.dynamic(),
            prov: Provenance {
                section: sec_kind,
                role_note: "sfx",
                ..Provenance::new(sec_kind)
            },
        });
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
    use super::super::semantic::demo_trace;
    use super::*;

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
