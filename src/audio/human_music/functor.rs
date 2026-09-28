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
use super::motif::Motif;
use super::rng::Rng;
use super::score::{Note, Provenance, Role, Score, SfxEvent, SfxKind};
use super::semantic::{EventKind, SemanticTrace, Tone};
use super::theory::{Chord, Midi, Scale};
use super::voicing::VoiceLeader;
use super::world::MusicWorld;

/// Compose a full score for `trace` under `world`, deterministic in `seed`.
pub fn compose(trace: &SemanticTrace, world: &MusicWorld, seed: u64) -> Score {
    let form = Form::from_trace(trace);
    let scale = Scale::new(world.tonic_pc, world.mode);

    let mut harmony = HarmonyEngine::new(world, seed);
    let chords = harmony.generate(&form);

    let mut groove = GrooveEngine::new(world, seed);
    let gr = groove.generate(&form);

    let total_beats = form.total_bars as f64 * BEATS_PER_BAR;
    let mut score = Score::new(world.tempo_bpm, BEATS_PER_BAR, total_beats);
    score.sections = form.sections.clone();
    score.chords = chords.clone();
    score.drums = gr.hits;

    // --- Comp (pad sustains + keys stabs) via voice leading. ---
    add_comp(&mut score, &chords, world, seed);

    // --- Bass locked to the kick and the harmony. ---
    add_bass(&mut score, &chords, &gr.kick_beats, &form, seed);

    // --- Melody: motif development across sections, harmony-aware. ---
    add_melody(&mut score, &chords, &form, &scale, seed);

    // --- SFX + intent morphisms from significant semantic events. ---
    add_sfx_and_provenance(&mut score, trace, &form);

    score
}

/// The chord sounding at `beat`.
fn chord_at(chords: &[ChordSpan], beat: f64) -> Chord {
    chords
        .iter()
        .rev()
        .find(|c| beat + 1e-6 >= c.start_beat)
        .map(|c| c.chord)
        .unwrap_or_else(|| {
            chords
                .first()
                .map(|c| c.chord)
                .unwrap_or(Chord::new(0, super::theory::Quality::Maj))
        })
}

/// The pitch with class `pc` nearest to `center`.
fn pitch_near(pc: i32, center: Midi) -> Midi {
    let base = (center / 12) * 12 + pc.rem_euclid(12);
    [base - 12, base, base + 12]
        .into_iter()
        .min_by_key(|p| (p - center).abs())
        .unwrap()
}

fn add_comp(score: &mut Score, chords: &[ChordSpan], world: &MusicWorld, seed: u64) {
    let mut pad_vl = VoiceLeader::new(52, 79, world.voicing_spread);
    let mut keys_vl = VoiceLeader::new(58, 84, world.voicing_spread);
    let mut rng = Rng::new(seed ^ 0xC0AB_11ED);
    let keys_stab_vapor = matches!(world.id, super::world::WorldId::Vapor95);

    for span in chords {
        let sec_kind = SectionKind::A; // refined by provenance pass; comp is structural
        let prov = Provenance {
            role_note: "comp",
            ..Provenance::new(sec_kind)
        };
        // Pad: hold the whole voicing for the chord's duration.
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
        // Keys: rhythmic stabs/arpeggio across the chord span.
        let voicing = keys_vl.lead(&span.chord, 4, 72);
        let stab_positions: Vec<f64> = if keys_stab_vapor {
            // Off-beat stabs (laid-back).
            (0..(span.dur_beats as usize))
                .map(|b| b as f64 + 0.5)
                .collect()
        } else {
            // On-beat comps.
            (0..(span.dur_beats.ceil() as usize))
                .map(|b| b as f64)
                .collect()
        };
        for (i, off) in stab_positions.iter().enumerate() {
            if *off >= span.dur_beats as f64 {
                break;
            }
            // Arpeggiate for variety: rotate which voice leads.
            let arp = i % voicing.voices.len();
            let pitches = if rng.chance(0.5) {
                vec![voicing.voices[arp]]
            } else {
                voicing.voices.clone()
            };
            for &p in &pitches {
                score.notes.push(Note {
                    start_beat: span.start_beat + off,
                    dur_beats: 0.45,
                    pitch: p,
                    velocity: (0.35 * world.base_dynamic).clamp(0.05, 1.0),
                    role: Role::Keys,
                    prov,
                });
            }
        }
    }
}

fn add_bass(score: &mut Score, chords: &[ChordSpan], kick_beats: &[f64], form: &Form, seed: u64) {
    let mut rng = Rng::new(seed ^ 0xBA55_0001);
    let bass_center = 40; // ~E2
    for (ci, span) in chords.iter().enumerate() {
        let span_end = span.start_beat + span.dur_beats as f64;
        let root_pc = span.chord.root_pc;
        let root = pitch_near(root_pc, bass_center);
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
        for (i, &k) in kicks.iter().enumerate() {
            let is_last = i + 1 == kicks.len();
            let approaching_change = is_last && span_end < score.total_beats - 1e-6;
            let (pitch, note) = if approaching_change && rng.chance(0.6) {
                // Chromatic/scale approach to the next root.
                let next_root = pitch_near(next_root_pc, bass_center);
                let dir = (next_root - root).signum();
                (next_root - dir.clamp(-1, 1), "approach")
            } else if energy > 0.6 && rng.chance(0.3) {
                // Fifth or octave displacement for drive.
                (
                    pitch_near((root_pc + 7).rem_euclid(12), bass_center),
                    "fifth",
                )
            } else {
                (root, "root")
            };
            let dur = kicks
                .get(i + 1)
                .map(|&n| (n - k) as f32)
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

fn add_melody(score: &mut Score, chords: &[ChordSpan], form: &Form, scale: &Scale, seed: u64) {
    let mut rng = Rng::new(seed ^ 0x3E10_D1E5);
    let seed_motif = Motif::seed_a();

    for sec in &form.sections {
        let sec_start = sec.start_bar as f64 * BEATS_PER_BAR;
        let sec_beats = sec.bars as f64 * BEATS_PER_BAR;
        let energy = sec.energy;

        // Intro/coda: mostly rest — let the pad breathe.
        if matches!(sec.kind, SectionKind::Intro) && energy < 0.3 {
            continue;
        }

        // Develop the motif per section kind (identity preserved throughout).
        let (motif, morph) = develop_motif(&seed_motif, sec.kind, &mut rng);
        // Register from elevation/energy: climax rides higher.
        let octave = match sec.kind {
            SectionKind::Climax => 5,
            SectionKind::Coda => 4,
            _ => 4 + (energy > 0.65) as i32,
        };
        let root_degree = 0; // relative to scale tonic; motif degrees offset from here

        // Place statements across the section with breath (rests) between.
        let phrase_beats = motif.total_beats() as f64;
        let mut t = sec_start;
        let mut guard = 0;
        while t + phrase_beats <= sec_start + sec_beats && guard < 64 {
            let notes = motif.render(scale, root_degree, octave, t);
            for (nb, dur, raw_pitch) in notes {
                // Harmony-aware target tones: snap on-beat notes to chord tones; leave
                // short off-beat notes as scale/passing tones.
                let chord = chord_at(chords, nb);
                let on_beat = (nb.fract()).abs() < 1e-6;
                let pitch = if on_beat || dur >= 1.0 {
                    chord.nearest_chord_tone(scale.nearest_scale_pitch(raw_pitch))
                } else {
                    scale.nearest_scale_pitch(raw_pitch)
                };
                score.notes.push(Note {
                    start_beat: nb,
                    dur_beats: (dur * 0.9).max(0.1),
                    pitch,
                    velocity: (0.55 + 0.4 * energy).clamp(0.1, 1.0),
                    role: Role::Lead,
                    prov: Provenance {
                        section: sec.kind,
                        motif_id: Some(motif.id),
                        morphism: Some(morph),
                        role_note: "melody",
                    },
                });
            }
            // Breath between statements: a beat or two of rest.
            t += phrase_beats + rng.range_f32(1.0, 2.0) as f64;
            guard += 1;
        }
    }
}

/// Choose a motif development for a section (returns the developed motif and a label).
fn develop_motif(seed: &Motif, kind: SectionKind, rng: &mut Rng) -> (Motif, &'static str) {
    match kind {
        SectionKind::Intro => (seed.fragment(2), "fragment"),
        SectionKind::A => (seed.clone(), "statement"),
        SectionKind::Development => {
            if rng.chance(0.5) {
                (seed.sequence(1, 2), "sequence")
            } else {
                (seed.invert(), "invert")
            }
        }
        SectionKind::Climax => (seed.transpose(2).scale_rhythm(0.75), "augment+up"),
        SectionKind::Contrast => (seed.retrograde(), "retrograde"),
        SectionKind::Coda => (seed.fragment(2).scale_rhythm(1.5), "diminish-frag"),
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

/// Apply a trace's events as morphisms to an initial intent (for law tests / provenance).
pub fn walk_intent(trace: &SemanticTrace) -> MusicIntent {
    let mut intent = MusicIntent::default();
    for ev in &trace.events {
        for m in event_to_morphisms(ev.kind, ev.state.tone) {
            intent = m.apply(intent).0;
        }
    }
    intent
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
