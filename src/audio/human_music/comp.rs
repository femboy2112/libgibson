//! **Keys and pad** — two projections of the [`PerformancePlan`] that *listen*.
//!
//! Round VI's keys stabbed at fixed offsets (+0.5 beats, then every 2 beats) whatever anyone else
//! played, and the pad held a full voicing on every chord. Here the keys read the shared
//! [`super::performance::AccentGrid`] and the lead's actual onsets: they comp on the grid's
//! syncopation opportunities *around* the lead, answer calls in their planned response windows
//! with the call's own material (quoted, echoed, inverted, compressed or completed), stab the
//! planned hits and pushes, hold suspensions, and leave space when the lead is busy. The pad picks
//! a per-bar mode (sustain, guide-tone shell, common-tone carry, swell, upper structure, silence).

use super::action::Agent;
use super::context::HarmonicContext;
use super::performance::{AccentGrid, KeysMode, PadMode, PerformancePlan, Transform, STEPS};
use super::plan::CompositionPlan;
use super::rng::Rng;
use super::score::{Note, PitchFunction, Provenance, Role};
use super::theory::{pitch_class, Midi};
use super::voicing::VoiceLeader;
use super::world::MusicWorld;

/// The function of `pitch` over `ctx`: a chord tone, a licensed tension, or `None`.
fn function_over(ctx: &HarmonicContext, pitch: Midi) -> Option<PitchFunction> {
    let pc = pitch_class(pitch);
    if ctx.chord.contains_pc(pc) {
        Some(PitchFunction::ChordTone)
    } else if ctx.palette.tensions.contains(&pc) {
        Some(PitchFunction::LicensedExtension)
    } else {
        None
    }
}

/// The nearest pitch to `target` that is stable over `ctx` (a chord tone or licensed tension).
fn nearest_stable(ctx: &HarmonicContext, target: Midi) -> Midi {
    (0..=12)
        .flat_map(|d| [target - d, target + d])
        .find(|&p| ctx.palette.is_stable(pitch_class(p)))
        .unwrap_or(target)
}

fn prov(role_note: &'static str, xform: Option<&'static str>) -> Provenance {
    Provenance {
        role_note,
        motif_xform: xform,
        ..Provenance::new(super::form::SectionKind::A)
    }
}

/// Whether a lead onset lands within one sixteenth of `beat`.
fn lead_onset_near(lead: &[Note], beat: f64) -> bool {
    lead.iter()
        .any(|n| (n.start_beat - beat).abs() < super::performance::STEP_BEATS * 0.99)
}

/// Lead notes sounding in `[a, b)`.
fn lead_in(lead: &[Note], a: f64, b: f64) -> Vec<Note> {
    lead.iter()
        .filter(|n| n.start_beat >= a - 1e-6 && n.start_beat < b - 1e-6)
        .copied()
        .collect()
}

/// Release any note whose sustain would carry it more than half a beat into a harmony it does not
/// belong to: it ends at the harmony change instead (a player lifts off when the chord moves). A
/// pedal is re-struck by its realizer rather than smeared. Returns how many notes were released.
pub fn release_at_harmony_change(
    notes: &mut [Note],
    chords: &[super::harmony::ChordSpan],
) -> usize {
    let mut released = 0;
    for n in notes.iter_mut() {
        let end = n.start_beat + n.dur_beats as f64;
        let Some(next) = chords
            .iter()
            .find(|c| c.start_beat > n.start_beat + 1e-6 && c.start_beat < end - 0.5)
        else {
            continue;
        };
        if !next.chord.contains_pc(pitch_class(n.pitch)) {
            n.dur_beats = ((next.start_beat - n.start_beat) as f32 * 0.97).max(0.1);
            released += 1;
        }
    }
    released
}

/// Realize the keys.
pub fn realize_keys(
    perf: &PerformancePlan,
    _plan: &CompositionPlan,
    world: &MusicWorld,
    lead: &[Note],
    seed: u64,
) -> Vec<Note> {
    let mut out = Vec::new();
    let mut vl = VoiceLeader::new(58, 84, world.voicing_spread);
    let mut rng = Rng::new(seed ^ 0x6E75_C0A9);
    let vel = (0.35 * world.base_dynamic).clamp(0.05, 1.0);
    let shell_n = if perf.language.shell_voicings { 3 } else { 4 };
    let unisons: Vec<(f64, f64)> = perf
        .actions
        .of_kind(super::action::ActionKind::Unison)
        .map(|a| (a.start_beat, a.end_beat()))
        .collect();
    let in_unison = |b: f64| unisons.iter().any(|&(s, e)| b >= s - 1e-6 && b < e - 1e-6);
    let answered: Vec<(f64, f64)> = perf
        .responses_for(Agent::Keys)
        .map(|(_, r)| (r.start_beat, r.start_beat + r.dur_beats))
        .collect();

    for eb in &perf.ensemble {
        let bar = eb.bar;
        let bar_start = AccentGrid::beat_of(bar, 0);
        let in_answer = |b: f64| answered.iter().any(|&(s, e)| b >= s - 1e-6 && b < e - 1e-6);
        // Candidate stab steps from the shared grid.
        let mut steps: Vec<(f32, usize)> = (0..STEPS)
            .filter_map(|s| {
                let w = perf.accent.at(bar, s);
                let beat = AccentGrid::beat_of(bar, s);
                if w.hole >= 0.5 || in_answer(beat) {
                    return None;
                }
                if in_unison(beat) {
                    return None;
                }
                let score = match eb.keys {
                    KeysMode::Stab => w.hit + w.push,
                    // Answer bars still comp outside the answer window itself.
                    KeysMode::Comp | KeysMode::Answer => {
                        w.syncopation + 0.6 * w.push + 0.5 * w.hit + 0.15 * w.pickup
                    }
                    _ => 0.0,
                };
                (score > 0.2).then_some((score + rng.range_f32(0.0, 0.05), s))
            })
            .collect();
        // Listen to the lead: never stab on top of its onsets.
        steps.retain(|&(_, s)| !lead_onset_near(lead, AccentGrid::beat_of(bar, s)));
        steps.sort_by(|a, b| b.0.total_cmp(&a.0));
        let max_stabs = match eb.keys {
            KeysMode::Stab => 2,
            KeysMode::Comp | KeysMode::Answer => {
                // The complexity budget: fewer stabs when the lead is dense in this bar.
                let lead_notes = lead_in(lead, bar_start, bar_start + 4.0).len();
                // When the bass is quoting/countering (in front), the keys leave it room too.
                let bass_front = matches!(
                    eb.bass,
                    super::performance::BassMode::Quote | super::performance::BassMode::Counter
                );
                let room =
                    (eb.budget - lead_notes as f32 - if bass_front { 2.5 } else { 0.0 }).max(0.0);
                ((room / 2.5).round() as usize)
                    .clamp(1, if perf.language.shell_voicings { 3 } else { 2 })
            }
            _ => 0,
        };
        let mut chosen: Vec<usize> = steps.iter().take(max_stabs).map(|x| x.1).collect();
        chosen.sort_unstable();
        match eb.keys {
            KeysMode::Sustain => {
                if let Some(ctx) = perf.context_at(bar_start) {
                    let v = vl.lead(&ctx.chord, shell_n, 72);
                    for &p in v.voices.iter().rev().take(shell_n) {
                        let mut n =
                            Note::new(bar_start, 3.9, p, vel * 0.9, Role::Keys, prov("hold", None));
                        n.function = function_over(ctx, p);
                        out.push(n);
                    }
                }
            }
            KeysMode::Space => {}
            KeysMode::Comp | KeysMode::Stab | KeysMode::Answer => {
                for (k, &s) in chosen.iter().enumerate() {
                    let beat = AccentGrid::beat_of(bar, s);
                    let Some(ctx) = perf.context_at(beat) else {
                        continue;
                    };
                    let v = vl.lead(&ctx.chord, shell_n.max(3), 72);
                    let take = if k % 2 == 0 { 3 } else { 2 }.min(v.voices.len());
                    let next = chosen.get(k + 1).map(|&n| AccentGrid::beat_of(bar, n));
                    let dur = next
                        .map(|n| (n - beat) * 0.8)
                        .unwrap_or(0.45)
                        .clamp(0.2, 0.9) as f32;
                    let accent = perf.accent.at(bar, s);
                    let v_mult = if accent.hit >= 0.9 { 1.1 } else { 1.0 };
                    for &p in v.voices.iter().rev().take(take) {
                        let mut n = Note::new(
                            beat,
                            dur,
                            p,
                            (vel * v_mult).min(1.0),
                            Role::Keys,
                            prov("comp", None),
                        );
                        n.function = function_over(ctx, p);
                        out.push(n);
                    }
                }
            }
        }
    }

    // Answers: the call's own material, transformed, in the keys' register.
    for (call, r) in perf.responses_for(Agent::Keys) {
        let src = lead_in(lead, call.start_beat, call.end_beat);
        out.extend(answer_notes(
            perf,
            &src,
            r.start_beat,
            r.dur_beats,
            r.transform,
            72,
            Role::Keys,
            vel * 1.05,
        ));
    }
    // Planned ensemble unison figures: the keys sound the shared line.
    for line in unison_lines(perf, lead) {
        for &(at, d, p, f) in &line {
            let mut n = Note::new(
                at,
                d,
                octave_near(p, 74),
                vel * 1.1,
                Role::Keys,
                prov("unison", Some("unison")),
            );
            n.function = f;
            out.push(n);
        }
    }
    out.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    release_at_harmony_change(&mut out, &perf.chords);
    out
}

/// One note of a shared line: `(beat, dur, pitch, function)`.
pub type LineNote = (f64, f32, Midi, Option<PitchFunction>);

/// The pitch with `p`'s class nearest `center`.
pub fn octave_near(p: Midi, center: Midi) -> Midi {
    let pc = pitch_class(p);
    let base = (center / 12) * 12 + pc;
    [base - 12, base, base + 12]
        .into_iter()
        .min_by_key(|q| (q - center).abs())
        .unwrap_or(p)
}

/// One realized line of a planned ensemble unison figure: `(beat, dur, pitch, function)`, in a
/// neutral register (each player transposes it by octaves into its own range). If the lead is
/// sounding in the window, the unison DOUBLES THE LEAD'S ACTUAL LINE (so keys, bass and lead play
/// one figure together); otherwise the band states the piece's rhythmic cell on stable tones.
pub fn unison_lines(perf: &PerformancePlan, lead: &[Note]) -> Vec<Vec<LineNote>> {
    let mut out = Vec::new();
    for a in perf.actions.of_kind(super::action::ActionKind::Unison) {
        let src = lead_in(lead, a.start_beat, a.end_beat());
        if src.len() >= 2 {
            out.push(
                src.iter()
                    .map(|n| (n.start_beat, n.dur_beats, n.pitch, n.function))
                    .collect(),
            );
            continue;
        }
        // The band's own figure: the bank's rhythmic cell (a diminished head of the identity).
        let cell = &perf.bank.rhythmic_cell;
        let mut line = Vec::new();
        let mut at = a.start_beat;
        for (i, &deg) in cell.degrees.iter().enumerate() {
            let d = cell.rhythm.get(i).copied().unwrap_or(0.5) as f64;
            if at >= a.end_beat() - 1e-6 {
                break;
            }
            let Some(ctx) = perf.context_at(at) else {
                break;
            };
            let p = nearest_stable(ctx, perf.region.degree_pitch(deg, 4));
            line.push((at, (d * 0.85).max(0.15) as f32, p, function_over(ctx, p)));
            at += d.max(0.25);
        }
        // Land on a chord tone of the harmony the figure arrives in.
        if let Some(ctx) = perf.context_at(at.min(a.end_beat() - 0.25)) {
            if at < a.end_beat() - 0.2 {
                let p = nearest_stable(ctx, line.last().map(|l| l.2).unwrap_or(67));
                line.push((at, 0.5, p, function_over(ctx, p)));
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    out
}

/// Build an answer from `src` (the call's notes) in the responder's register: the transform
/// decides the relation; every pitch is made stable over the harmony it lands on, so an answer can
/// cross a chord boundary and still belong to the new chord.
#[allow(clippy::too_many_arguments)]
pub fn answer_notes(
    perf: &PerformancePlan,
    src: &[Note],
    start: f64,
    dur: f64,
    transform: Transform,
    center: Midi,
    role: Role,
    velocity: f32,
) -> Vec<Note> {
    if src.is_empty() || dur <= 0.0 {
        return Vec::new();
    }
    let tail: Vec<Note> = src.iter().rev().take(4).rev().copied().collect();
    let head: Vec<Note> = src.iter().take(3).copied().collect();
    let (material, time_scale): (Vec<Note>, f64) = match transform {
        Transform::Quote | Transform::Echo | Transform::Invert => (tail, 1.0),
        Transform::Compress => (head, 0.5),
        Transform::Complete => (tail, 1.0),
        Transform::Silence => return Vec::new(),
    };
    let t0 = material[0].start_beat;
    let p0 = material[0].pitch;
    // Transpose the material's centre into the responder's register.
    let mean = material.iter().map(|n| n.pitch).sum::<Midi>() / material.len() as Midi;
    let shift = ((center - mean) as f64 / 12.0).round() as Midi * 12;
    let dir = material
        .last()
        .map(|l| (l.pitch - p0).signum())
        .unwrap_or(1);
    let mut out = Vec::new();
    for (i, n) in material.iter().enumerate() {
        let at = start + (n.start_beat - t0) * time_scale;
        if at >= start + dur - 1e-6 {
            break;
        }
        let Some(ctx) = perf.context_at(at) else {
            continue;
        };
        let raw = match transform {
            Transform::Invert => p0 - (n.pitch - p0) + shift,
            Transform::Echo => {
                // Same rhythm, the responder's own pitches: the palette's guide tones.
                let g = &ctx.palette.guide_tones;
                let pc = g
                    .get(i % g.len().max(1))
                    .copied()
                    .unwrap_or(ctx.chord.root_pc);
                let base = (center / 12) * 12 + pc;
                if base > center + 6 {
                    base - 12
                } else {
                    base
                }
            }
            Transform::Complete => {
                // Continue past the call's end in its final direction.
                let last = material.last().map(|l| l.pitch).unwrap_or(p0);
                last + shift + dir * 2 * (i as Midi + 1)
            }
            _ => n.pitch + shift,
        };
        let p = nearest_stable(ctx, raw);
        let d = ((n.dur_beats as f64) * time_scale)
            .min(start + dur - at)
            .max(0.15) as f32;
        let mut note = Note::new(
            at,
            d,
            p,
            velocity.min(1.0),
            role,
            prov("answer", Some(transform.label())),
        );
        note.function = function_over(ctx, p);
        out.push(note);
    }
    out
}

/// Realize the pad.
pub fn realize_pad(
    perf: &PerformancePlan,
    _plan: &CompositionPlan,
    world: &MusicWorld,
) -> Vec<Note> {
    let mut out = Vec::new();
    let mut vl = VoiceLeader::new(52, 79, world.voicing_spread);
    let vel = (0.4 * world.base_dynamic).clamp(0.05, 1.0);
    let mut prev_voices: Vec<Midi> = Vec::new();
    for (ci, ctx) in perf.contexts.iter().enumerate() {
        let Some(eb) = perf.bar_at(ctx.start_beat) else {
            continue;
        };
        let dur = ctx.dur_beats * 0.98;
        let v = vl.lead(&ctx.chord, 4, 67);
        let push = |p: Midi, at: f64, d: f32, vm: f32, out: &mut Vec<Note>| {
            let mut n = Note::new(
                at,
                d.max(0.1),
                p,
                (vel * vm).clamp(0.02, 1.0),
                Role::Pad,
                prov("pad", None),
            );
            n.function = function_over(ctx, p);
            out.push(n);
        };
        match eb.pad {
            PadMode::Silent => {}
            PadMode::Sustain => {
                for &p in &v.voices {
                    push(p, ctx.start_beat, dur, 1.0, &mut out);
                }
            }
            PadMode::Shell => {
                // Guide tones only (the 3rd and 7th/6th), voiced from the leader's choice.
                for &p in v
                    .voices
                    .iter()
                    .filter(|&&p| ctx.palette.guide_tones.contains(&pitch_class(p)))
                {
                    push(p, ctx.start_beat, dur, 1.0, &mut out);
                }
            }
            PadMode::CommonToneCarry => {
                // Hold what the previous harmony shares; add the new guide tones.
                let prev_pcs: Vec<i32> = if ci > 0 {
                    perf.contexts[ci - 1].chord.pitch_classes()
                } else {
                    Vec::new()
                };
                for &p in &v.voices {
                    let pc = pitch_class(p);
                    if prev_pcs.contains(&pc) || ctx.palette.guide_tones.contains(&pc) {
                        let carried = prev_voices
                            .iter()
                            .copied()
                            .find(|&q| pitch_class(q) == pc)
                            .unwrap_or(p);
                        push(carried, ctx.start_beat, dur, 0.9, &mut out);
                    }
                }
            }
            PadMode::Swell => {
                // Low voices first, the upper voices entering a beat later: a swell.
                for (i, &p) in v.voices.iter().enumerate() {
                    let delay = if i >= 2 {
                        1.0f64.min(ctx.dur_beats as f64 * 0.5)
                    } else {
                        0.0
                    };
                    push(
                        p,
                        ctx.start_beat + delay,
                        (dur - delay as f32).max(0.2),
                        0.7 + 0.1 * i as f32,
                        &mut out,
                    );
                }
            }
            PadMode::UpperStructure => {
                // Wide: the voicing's upper three voices lifted an octave, plus a licensed tension.
                for &p in v.voices.iter().rev().take(3) {
                    push(p + 12, ctx.start_beat, dur, 0.75, &mut out);
                }
                if let Some(&t) = ctx.palette.tensions.first() {
                    let top = v.voices.last().copied().unwrap_or(72) + 12;
                    let p = (top / 12) * 12 + t;
                    let p = if p < top { p + 12 } else { p };
                    if p <= 96 {
                        push(p, ctx.start_beat, dur, 0.6, &mut out);
                    }
                }
            }
        }
        prev_voices = v.voices.clone();
    }
    out
}
