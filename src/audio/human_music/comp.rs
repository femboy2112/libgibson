//! **Keys and pad** — two projections of the [`PerformancePlan`] that *listen*.
//!
//! Round VI's keys stabbed at fixed offsets (+0.5 beats, then every 2 beats) whatever anyone else
//! played, and the pad held a full voicing on every chord. Here the keys read the shared
//! [`super::performance::AccentGrid`] and the lead's actual onsets: they comp on the grid's
//! syncopation opportunities *around* the lead, answer calls in their planned response windows
//! with the call's own material (quoted, echoed, inverted, compressed or completed), stab the
//! planned hits and pushes, hold suspensions, and leave space when the lead is busy. The pad picks
//! a per-bar mode (sustain, guide-tone shell, common-tone carry, swell, upper structure, silence).

use super::action::{ActionKind, Agent};
use super::context::HarmonicContext;
use super::ids::{ActionId, ActionStamp};
use super::performance::{AccentGrid, Call, KeysMode, PadMode, PerformancePlan, Response, STEPS};
use super::plan::CompositionPlan;
use super::rng::Rng;
use super::score::{Note, PitchFunction, Provenance, Role};
use super::theory::{pitch_class, Midi};
use super::voicing::{keys_path, pad_path, stab_voices};
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
    let mut rng = Rng::new(seed ^ 0x6E75_C0A9);
    let vel = (0.35 * world.base_dynamic).clamp(0.05, 1.0);
    let shell_n = if perf.language.shell_voicings { 3 } else { 4 };
    // One global voice path over every harmony the keys play in, listening to the lead.
    let kp = keys_path(perf, world.voicing_spread, lead, shell_n);
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
    // The keys' planned suspensions: realized for their own window whatever else the bar does
    // (Round VII only held when the whole bar was in Sustain mode, and an answer elsewhere in the
    // bar silently cancelled the hold).
    let holds: Vec<(f64, f64, ActionId)> = perf
        .actions
        .actions
        .iter()
        .filter(|a| a.kind == ActionKind::Hold && a.initiator == Agent::Keys)
        .map(|a| (a.start_beat, a.end_beat(), a.id))
        .collect();
    let in_hold = |b: f64| holds.iter().any(|&(s, e, _)| b >= s - 1e-6 && b < e - 1e-6);
    let mut held_by_bar: Vec<ActionId> = Vec::new();

    for eb in &perf.ensemble {
        let bar = eb.bar;
        let bar_start = AccentGrid::beat_of(bar, 0);
        // The stage is the single orchestration authority: out means out (Round VII realized the
        // keys everywhere and let a later pass delete them).
        if !perf.on_stage(Agent::Keys, bar_start) {
            continue;
        }
        let level = perf.level(Agent::Keys, bar_start);
        let in_answer = |b: f64| answered.iter().any(|&(s, e)| b >= s - 1e-6 && b < e - 1e-6);
        // Candidate stab steps from the shared grid.
        let mut steps: Vec<(f32, usize)> = (0..STEPS)
            .filter_map(|s| {
                let w = perf.accent.at(bar, s);
                let beat = AccentGrid::beat_of(bar, s);
                if w.hole >= 0.5 || in_answer(beat) || in_hold(beat) {
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
                    let v = kp.voicing_at(perf, bar_start);
                    // The suspension(s) this bar holds.
                    let hold_stamp = perf
                        .actions_covering(&[ActionKind::Hold], bar_start + 0.5, Some(Agent::Keys))
                        .chain(perf.actions_starting(
                            &[ActionKind::Hold],
                            bar_start,
                            3.9,
                            Some(Agent::Keys),
                        ))
                        .fold(ActionStamp::NONE, ActionStamp::with);
                    held_by_bar.extend(
                        holds
                            .iter()
                            .filter(|h| h.0 < bar_start + 4.0 - 1e-6 && h.1 > bar_start + 1e-6)
                            .map(|h| h.2),
                    );
                    for &p in v.voices.iter().rev().take(shell_n) {
                        let mut n = Note::new(
                            bar_start,
                            3.9,
                            p,
                            vel * 0.9 * level,
                            Role::Keys,
                            stamped(prov("hold", None), hold_stamp),
                        );
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
                    let v = kp.voicing_at(perf, beat);
                    let take = if k % 2 == 0 { 3 } else { 2 }.min(v.voices.len());
                    let next = chosen.get(k + 1).map(|&n| AccentGrid::beat_of(bar, n));
                    let dur = next
                        .map(|n| (n - beat) * 0.8)
                        .unwrap_or(0.45)
                        .clamp(0.2, 0.9) as f32;
                    let accent = perf.accent.at(bar, s);
                    let v_mult = if accent.hit >= 0.9 { 1.1 } else { 1.0 };
                    // The planned accents this stab realizes: a push or hit on this step, the
                    // keys' own re-entry, a displacement window it sits in off the beat.
                    let mut st = perf
                        .actions_starting(
                            &[ActionKind::Push, ActionKind::Hit],
                            beat,
                            super::performance::STEP_BEATS * 0.5,
                            None,
                        )
                        .chain(perf.actions_covering(
                            &[ActionKind::ReEntry, ActionKind::Pickup],
                            beat,
                            Some(Agent::Keys),
                        ))
                        .fold(ActionStamp::NONE, ActionStamp::with);
                    if s % 4 != 0 {
                        st = perf
                            .actions_covering(&[ActionKind::Displace], beat, None)
                            .fold(st, ActionStamp::with);
                    }
                    for p in stab_voices(&v, ctx, take) {
                        let mut n = Note::new(
                            beat,
                            dur,
                            p,
                            (vel * v_mult * level).min(1.0),
                            Role::Keys,
                            stamped(prov("comp", None), st),
                        );
                        n.function = function_over(ctx, p);
                        out.push(n);
                    }
                }
            }
        }
    }

    // Suspensions the bar's mode did not already sustain: held for the action's own window.
    for &(hs, he, id) in holds.iter().filter(|h| !held_by_bar.contains(&h.2)) {
        if !perf.on_stage(Agent::Keys, hs) {
            continue;
        }
        let Some(ctx) = perf.context_at(hs) else {
            continue;
        };
        let v = kp.voicing_at(perf, hs);
        let level = perf.level(Agent::Keys, hs);
        for &p in v.voices.iter().rev().take(shell_n) {
            let mut n = Note::new(
                hs,
                ((he - hs) * 0.97).max(1.5) as f32,
                p,
                vel * 0.9 * level,
                Role::Keys,
                stamped(prov("hold", None), ActionStamp::of(id)),
            );
            n.function = function_over(ctx, p);
            out.push(n);
        }
    }
    // Answers: the CALL's material, transformed (derived in the plan), in the keys' register —
    // whoever called. Round VII transformed whatever the lead happened to play in the window.
    for (call, r) in perf.responses_for(Agent::Keys) {
        out.extend(answer_notes(
            perf,
            call,
            r,
            Agent::Keys,
            Role::Keys,
            vel * 1.05,
        ));
    }
    // The figures the keys state (a keys-led pickup / fragment / re-entry): their own material.
    out.extend(figure_notes(perf, Agent::Keys, Role::Keys, vel * 1.05));
    // Planned ensemble unison figures: the keys sound the shared line.
    for (id, line) in unison_lines(perf, lead) {
        for &(at, d, p, f) in &line {
            let mut n = Note::new(
                at,
                d,
                octave_near(p, 74),
                vel * 1.1 * perf.level(Agent::Keys, at).max(0.85),
                Role::Keys,
                stamped(prov("unison", Some("unison")), ActionStamp::of(id)),
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
pub fn unison_lines(perf: &PerformancePlan, lead: &[Note]) -> Vec<(ActionId, Vec<LineNote>)> {
    let mut out = Vec::new();
    for a in perf.actions.of_kind(super::action::ActionKind::Unison) {
        let src = lead_in(lead, a.start_beat, a.end_beat());
        if src.len() >= 2 {
            out.push((
                a.id,
                src.iter()
                    .map(|n| (n.start_beat, n.dur_beats, n.pitch, n.function))
                    .collect(),
            ));
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
            out.push((a.id, line));
        }
    }
    out
}

/// A projected onset within a sixteenth of a planned push/hit step moves ONTO it and carries that
/// accent's id: the ensemble verb changes the line (the figure or answer lands with the band),
/// rather than a coincidence being counted as a witness afterwards.
fn land_on_accents(perf: &PerformancePlan, at: f64) -> (f64, ActionStamp) {
    let near: Vec<(f64, ActionId)> = perf
        .actions
        .actions
        .iter()
        .filter(|a| matches!(a.kind, ActionKind::Push | ActionKind::Hit))
        .filter(|a| (a.start_beat - at).abs() <= super::performance::STEP_BEATS + 1e-6)
        .map(|a| (a.start_beat, a.id))
        .collect();
    match near.first() {
        Some(&(t, _)) => (
            t,
            near.iter()
                .filter(|x| (x.0 - t).abs() < 1e-6)
                .fold(ActionStamp::NONE, |s, x| s.with(x.1)),
        ),
        None => (at, ActionStamp::NONE),
    }
}

/// `prov` additionally realizing every action in `stamp`.
pub fn stamped(mut prov: Provenance, stamp: ActionStamp) -> Provenance {
    for id in stamp.iter() {
        prov = prov.realizing(id);
    }
    prov
}

/// The notes of `agent`'s answer `r` to `call`: a projection of the response's OWN material —
/// derived in the plan from the CALL's material by the planned transform — in the responder's
/// register, every pitch stable over the harmony it lands on (an answer may cross a chord
/// boundary and still belong to the new chord). Stamped with the answer action, the interaction
/// and the material, so a witness can follow the exact causal line call → response.
pub fn answer_notes(
    perf: &PerformancePlan,
    call: &Call,
    r: &Response,
    agent: Agent,
    role: Role,
    velocity: f32,
) -> Vec<Note> {
    let Some(mid) = r.material else {
        return Vec::new();
    };
    let m = perf.material(mid);
    let interaction = perf
        .interactions
        .iter()
        .find(|i| i.call.action == call.action)
        .map(|i| i.id);
    super::material::line_of(m, agent, r.start_beat, r.start_beat + r.dur_beats, perf)
        .into_iter()
        .filter_map(|(at, d, p, accent)| {
            let p = p?;
            let ctx = perf.context_at(at)?;
            if !perf.on_stage(agent, at) {
                return None;
            }
            let (at, accents) = land_on_accents(perf, at);
            let mut prov = stamped(
                prov("answer", Some(r.transform.label()))
                    .realizing_opt(r.action)
                    .realizing_opt(r.realizes),
                accents,
            );
            prov.interaction = interaction;
            prov.material = Some(mid);
            let mut note = Note::new(
                at,
                d as f32,
                p,
                (velocity * (0.9 + 0.15 * accent) * perf.level(agent, at).max(0.85)).min(1.0),
                role,
                prov,
            );
            note.function = function_over(ctx, p);
            Some(note)
        })
        .collect()
}

/// The figures `agent` states (its pickups, fragments, re-entries and fills), each a projection of
/// the figure material the plan generated for the action — stamped with that action, its material
/// and (when somebody answers it) its interaction.
pub fn figure_notes(perf: &PerformancePlan, agent: Agent, role: Role, velocity: f32) -> Vec<Note> {
    let mut out = Vec::new();
    for m in perf.figures_for(agent) {
        let super::material::MaterialSource::Figure { action, .. } = m.source else {
            continue;
        };
        let Some(a) = perf.actions.get(action) else {
            continue;
        };
        let until = a.end_beat().max(a.start_beat + m.length());
        let interaction = perf.interaction_of(action);
        for (at, d, p, accent) in super::material::line_of(m, agent, m.start_beat, until, perf) {
            let (Some(p), Some(ctx)) = (p, perf.context_at(at)) else {
                continue;
            };
            if !perf.on_stage(agent, at) {
                continue;
            }
            let (at, accents) = land_on_accents(perf, at);
            // The figure also performs any other figure verb of the same player it covers (the
            // Lift pickup inside the Reset fill), and the ensemble accents it lands on.
            let covered = perf
                .actions_covering(
                    &[
                        ActionKind::Pickup,
                        ActionKind::Fill,
                        ActionKind::Fragment,
                        ActionKind::ReEntry,
                    ],
                    at,
                    Some(agent),
                )
                .fold(accents, ActionStamp::with);
            let mut prov = stamped(
                prov("figure", Some(a.kind.label())).realizing(action),
                covered,
            );
            prov.interaction = interaction;
            prov.material = Some(m.id);
            let mut n = Note::new(
                at,
                d as f32,
                p,
                (velocity * (0.9 + 0.15 * accent) * perf.level(agent, at)).clamp(0.05, 1.0),
                role,
                prov,
            );
            n.function = if agent == Agent::Bass {
                ctx.chord
                    .contains_pc(pitch_class(p))
                    .then_some(PitchFunction::ChordTone)
            } else {
                function_over(ctx, p)
            };
            out.push(n);
        }
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
    // One global voice path over the sounding harmonies; each bar's mode picks its shape family.
    let pp = pad_path(perf, world.voicing_spread);
    let vel = (0.4 * world.base_dynamic).clamp(0.05, 1.0);
    for (ci, ctx) in perf.contexts.iter().enumerate() {
        let Some(eb) = perf.bar_at(ctx.start_beat) else {
            continue;
        };
        let dur = ctx.dur_beats * 0.98;
        let v = pp.voicing(ci, ctx);
        // The stage decides whether the pad sounds this harmony, and how loud; the pad's own
        // re-entries and thickenings are stamped on the notes that perform them.
        let on = perf.on_stage(Agent::Pad, ctx.start_beat);
        let level = perf.level(Agent::Pad, ctx.start_beat);
        let st = perf
            .actions_covering(
                &[ActionKind::ReEntry, ActionKind::Thicken],
                ctx.start_beat + 1e-3,
                Some(Agent::Pad),
            )
            .fold(ActionStamp::NONE, ActionStamp::with);
        let push = |p: Midi, at: f64, d: f32, vm: f32, out: &mut Vec<Note>| {
            if !on {
                return;
            }
            let mut n = Note::new(
                at,
                d.max(0.1),
                p,
                (vel * vm * level).clamp(0.02, 1.0),
                Role::Pad,
                stamped(prov("pad", None), st),
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
                // The path's guide-tone shell (3–7 / 7–3 + at most one licensed extension; over a
                // single-guide chord the 3rd + 9th + 5th).
                for &p in &v.voices {
                    push(p, ctx.start_beat, dur, 1.0, &mut out);
                }
            }
            PadMode::CommonToneCarry => {
                // Hold, at the exact same pitch, what the previous voicing shares; add the new
                // guide tones.
                let prev = pp.previous(ci).map(|p| p.voices).unwrap_or_default();
                for &p in &v.voices {
                    if prev.contains(&p) || ctx.palette.guide_tones.contains(&pitch_class(p)) {
                        push(p, ctx.start_beat, dur, 0.9, &mut out);
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
                // Wide: the path's upper-window voicing (a voice at MIDI 79 or above), its
                // licensed tensions a little softer.
                for &p in &v.voices {
                    let vm = if ctx.palette.tensions.contains(&pitch_class(p)) {
                        0.6
                    } else {
                        0.75
                    };
                    push(p, ctx.start_beat, dur, vm, &mut out);
                }
            }
        }
    }
    out
}
