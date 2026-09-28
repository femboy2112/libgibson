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

use super::form::{Form, SectionKind, BEATS_PER_BAR};
use super::groove::GrooveEngine;
use super::harmony::{ChordSpan, HarmonyEngine};
use super::intent::{IntentMorphism, MusicIntent};
use super::motif::{motif_similarity, Motif, MotifBank};
use super::plan::{CompositionPlan, Phrase, PhraseObligation, SectionFamily};
use super::rng::Rng;
use super::score::{Note, Provenance, Role, Score, SfxEvent, SfxKind};
use super::semantic::{EventKind, SemanticTrace, Tone};
use super::theory::{Midi, Scale};
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
/// Round II inserts the planning boundary here: the semantic trace becomes a causal
/// [`IntentTimeline`], the timeline yields a [`CompositionPlan`] (contract + form graph +
/// arrangement), the Round-I engines still generate the raw material, and then
/// `apply_arrangement` gates every voice by its per-phrase role — so the band finally has
/// a bandleader deciding who plays, how loud, and when to shut up.
pub fn compose_with_plan(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
) -> (Score, CompositionPlan) {
    let form = Form::from_trace(trace);
    let scale = Scale::new(world.tonic_pc, world.mode);

    // The causal spine and the plan derived from it.
    let timeline = IntentTimeline::walk(trace);
    let plan = CompositionPlan::build(&timeline, form.total_bars);

    let mut harmony = HarmonyEngine::new(world, seed);
    // Harmony is planned at phrase scope: each phrase closes on a prepared cadence.
    let chords = harmony.generate(&form, &plan.form.phrases);

    let mut groove = GrooveEngine::new(world, seed);
    // Fills land at the plan's phrase boundaries, not the old Form's section edges.
    let phrase_end_bars: Vec<u32> = plan.form.phrases.iter().map(|p| p.end_bar()).collect();
    let gr = groove.generate(&form, &phrase_end_bars);

    let total_beats = form.total_bars as f64 * BEATS_PER_BAR;
    let mut score = Score::new(world.tempo_bpm, BEATS_PER_BAR, total_beats);
    score.sections = form.sections.clone();
    score.chords = chords.clone();
    score.drums = gr.hits;

    // --- Comp: sustained pad bed + sparse groove-locked keys arpeggio. ---
    add_comp(&mut score, &chords, world);

    // --- Bass: persistent kick-locked figure. ---
    add_bass(&mut score, &chords, &gr.kick_beats, &form);

    // --- Melody: one developing motif threaded through the plan's phrases. ---
    add_melody(&mut score, &chords, &plan, &scale, seed);

    // --- SFX + intent morphisms from significant semantic events. ---
    add_sfx_and_provenance(&mut score, trace, &form);

    // --- Arrangement: gate every voice by its per-phrase role and stamp real provenance
    //     from the plan (fixing the Round-I hardcoded SectionKind::A). ---
    apply_arrangement(&mut score, &plan);

    (score, plan)
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
        n.prov.obligation = Some(phrase.obligation.label());
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
        true
    });

    // SFX are punctuation tied to semantic beats — keep them, but restamp section/phrase so
    // the whole IR agrees with the plan.
    for e in &mut score.sfx {
        let phrase = *form.phrase_at(e.start_beat);
        e.prov.section = phrase.family.to_section_kind();
        e.prov.phrase = Some(phrase.ix);
        e.prov.family = Some(phrase.family.label());
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
            score.notes.push(Note {
                start_beat: span.start_beat,
                dur_beats: span.dur_beats * 0.98,
                pitch: p,
                velocity: (0.4 * world.base_dynamic).clamp(0.05, 1.0),
                role: Role::Pad,
                prov,
            });
        }
        // Keys: single-voice offbeat pushes arpeggiated through the voicing — one every two
        // beats, starting on the "and of 1". The chord already sounds in the pad, so the keys
        // are pure rhythmic interlock, not another block of triads.
        let voicing = keys_vl.lead(&span.chord, 4, 72);
        if voicing.voices.is_empty() {
            continue;
        }
        let dur = span.dur_beats as f64;
        let mut idx = 0usize;
        let mut off = 0.5f64;
        while off < dur - 1e-6 {
            let p = voicing.voices[idx % voicing.voices.len()];
            score.notes.push(Note {
                start_beat: span.start_beat + off,
                dur_beats: 0.45,
                pitch: p,
                velocity: (0.35 * world.base_dynamic).clamp(0.05, 1.0),
                role: Role::Keys,
                prov,
            });
            idx += 1;
            off += 2.0;
        }
    }
}

/// Bass = a persistent, deterministic figure locked to the kick and the harmony. Round I
/// re-rolled root/fifth/approach with a coin at every kick, so the line never settled into a
/// figure. Here the role of each kick is fixed by its position: the root states the chord on
/// the span's first kick, a fifth drives the offbeat kicks at high energy, and the last kick
/// before a chord change steps chromatically into the next root — a repeatable shape, not a
/// dice roll, still onset-locked to the groove.
fn add_bass(score: &mut Score, chords: &[ChordSpan], kick_beats: &[f64], form: &Form) {
    let bass_center = 40; // ~E2
    for (ci, span) in chords.iter().enumerate() {
        let span_end = span.start_beat + span.dur_beats as f64;
        let root_pc = span.chord.root_pc;
        let root = pitch_near(root_pc, bass_center);
        let fifth = pitch_near((root_pc + 7).rem_euclid(12), bass_center);
        let next_root_pc = chords
            .get(ci + 1)
            .map(|c| c.chord.root_pc)
            .unwrap_or(root_pc);
        let energy = form.energy_at(span.start_beat / BEATS_PER_BAR);

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
            );
            continue;
        }
        let n = kicks.len();
        for (i, &k) in kicks.iter().enumerate() {
            let is_last = i + 1 == n;
            let approaching_change = is_last && span_end < score.total_beats - 1e-6;
            let (pitch, note) = if approaching_change {
                // Step chromatically into the next chord's root.
                let next_root = pitch_near(next_root_pc, bass_center);
                let dir = (next_root - root).signum();
                (next_root - dir.clamp(-1, 1), "approach")
            } else if i == 0 {
                (root, "root")
            } else if energy > 0.55 && i % 2 == 1 {
                // Fifth on the offbeat kicks when there's drive.
                (fifth, "fifth")
            } else {
                (root, "root")
            };
            let dur = kicks
                .get(i + 1)
                .map(|&nx| (nx - k) as f32)
                .unwrap_or((span_end - k) as f32)
                .clamp(0.1, 2.0);
            push_bass(score, k, dur * 0.9, pitch, energy, note);
        }
    }
}

fn push_bass(score: &mut Score, at: f64, dur: f32, pitch: Midi, energy: f32, note: &'static str) {
    score.notes.push(Note {
        start_beat: at,
        dur_beats: dur.max(0.1),
        pitch,
        velocity: (0.6 + 0.35 * energy).clamp(0.1, 1.0),
        role: Role::Bass,
        prov: Provenance {
            role_note: note,
            ..Provenance::new(SectionKind::A)
        },
    });
}

/// The lead voice: one developing melody threaded through the plan's phrases.
///
/// Round I re-seeded a pristine motif every section, inserted random rests, and snapped each
/// note to a chord tone in isolation — preserving a motif `id` while destroying the phrase
/// identity. This threads the *developed* motif through the form (restating the germ at
/// A-sections and whenever it has drifted past the contract's transform budget, developing
/// the current object otherwise), enters statements grid-aligned (no random offset), reads
/// register from the phrase's now-live elevation intent, and realizes each whole statement
/// jointly against the harmony via [`super::motif::realize_phrase`] instead of per-note
/// snapping. The lead plays only where the arrangement gives it a voice — so it breathes.
fn add_melody(
    score: &mut Score,
    chords: &[ChordSpan],
    plan: &CompositionPlan,
    scale: &Scale,
    seed: u64,
) {
    let mut rng = Rng::new(seed ^ 0x3E10_D1E5);
    let bank = MotifBank::generate(scale, seed ^ 0x3E10_D1E5);
    let germ = bank.identity.identity();
    // Restate once the developed object has drifted more than the contract permits.
    let floor = (1.0 - plan.contract.max_transform).clamp(0.0, 1.0);
    let two_bar = 2.0 * BEATS_PER_BAR;

    let mut current = bank.identity.clone();
    for phrase in &plan.form.phrases {
        // The melody breathes: it sounds only where the arrangement gives the lead a voice.
        if !plan.arrangement.at(phrase.ix as usize).lead.is_audible() {
            continue;
        }

        // Restate the germ (or land the hook at the climax), else develop the CURRENT object.
        let (next, morph) = if matches!(phrase.family, SectionFamily::Climax) {
            (bank.hook.clone(), "hook")
        } else if matches!(phrase.family, SectionFamily::A)
            || motif_similarity(&current.identity(), &germ) < floor
        {
            (bank.identity.clone(), "statement")
        } else {
            develop_current(&current, phrase, &mut rng)
        };
        current = next;

        // Register from the phrase's intent — elevation is finally live (see IntentTimeline).
        let octave = if matches!(phrase.family, SectionFamily::Climax) {
            5
        } else {
            4 + (phrase.intent.register > 0.65) as i32
        };

        let stmt_beats = current.total_beats() as f64;
        if stmt_beats < 1e-6 {
            continue;
        }
        let phrase_end = phrase.end_beat();
        let mut t = phrase.start_beat();
        let mut guard = 0;
        while t + stmt_beats <= phrase_end + 1e-6 && guard < 32 {
            for (nb, dur, pitch) in
                super::motif::realize_phrase(&current, chords, scale, 0, octave, t, 4)
            {
                score.notes.push(Note {
                    start_beat: nb,
                    dur_beats: (dur * 0.9).max(0.1),
                    pitch,
                    velocity: (0.55 + 0.4 * phrase.intent.energy).clamp(0.1, 1.0),
                    role: Role::Lead,
                    prov: Provenance {
                        motif_id: Some(current.id),
                        motif_xform: Some(morph),
                        anchor: Some("motif"),
                        role_note: "melody",
                        ..Provenance::new(phrase.family.to_section_kind())
                    },
                });
            }
            // Breathe to the next 2-bar boundary — grid-aligned rest, not a random gap.
            let after = t + stmt_beats;
            let boundary = (after / two_bar).ceil() * two_bar;
            t = if boundary > after + 1e-6 {
                boundary
            } else {
                after
            };
            guard += 1;
        }
    }
}

/// Transform the CURRENT developed motif by the phrase's obligation — bounded,
/// length-preserving moves so the statement keeps fitting the phrase and stays recognizable
/// as the same idea (the restate-on-drift check in `add_melody` bounds cumulative drift).
fn develop_current(m: &Motif, phrase: &Phrase, rng: &mut Rng) -> (Motif, &'static str) {
    match phrase.obligation {
        PhraseObligation::Lift => (m.transpose(2), "sequence+2"),
        PhraseObligation::Release => (m.scale_rhythm(0.75), "diminish"),
        PhraseObligation::Continuation => (m.transpose(-1), "sequence-1"),
        _ => {
            if rng.chance(0.5) {
                (m.transpose(1), "sequence+1")
            } else {
                (m.scale_rhythm(1.25), "augment")
            }
        }
    }
}

fn add_sfx_and_provenance(score: &mut Score, trace: &SemanticTrace, form: &Form) {
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
        let sec = form.section_at_bar((ev.at_beat / BEATS_PER_BAR) as u32);
        score.sfx.push(SfxEvent {
            start_beat: ev.at_beat,
            kind,
            velocity: ev.state.dynamic(),
            prov: Provenance {
                section: sec.kind,
                role_note: "sfx",
                ..Provenance::new(sec.kind)
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
}
