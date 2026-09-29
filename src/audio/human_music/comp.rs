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
use super::harmonic_state::{voice_of, HarmonicEnsembleState, Hazard, VerticalDecision};
use super::ids::{ActionId, ActionStamp};
use super::performance::{AccentGrid, Call, KeysMode, PadMode, PerformancePlan, Response, STEPS};
use super::plan::CompositionPlan;
use super::rng::Rng;
use super::score::{Note, PitchFunction, Provenance, Role};
use super::theory::{note_name, pitch_class, Midi};
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
    release_with_overhang(notes, chords, 0.5)
}

/// [`release_at_harmony_change`] with an explicit tolerated overhang (beats): a note may carry at
/// most `overhang` into a harmony it does not belong to. The coupled keys use a near-zero overhang —
/// a stab's written tail a quarter beat into the next chord is a real vertical collision there.
pub fn release_with_overhang(
    notes: &mut [Note],
    chords: &[super::harmony::ChordSpan],
    overhang: f64,
) -> usize {
    let mut released = 0;
    for n in notes.iter_mut() {
        let end = n.start_beat + n.dur_beats as f64;
        let Some(next) = chords
            .iter()
            .find(|c| c.start_beat > n.start_beat + 1e-6 && c.start_beat < end - overhang)
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

/// The keys' base velocity in `world`.
pub fn keys_velocity(world: &MusicWorld) -> f32 {
    (0.35 * world.base_dynamic).clamp(0.05, 1.0)
}

/// How many voices the keys' path carries in `perf`'s language (3 for shell voicings, else 4).
pub fn keys_shell_n(perf: &PerformancePlan) -> usize {
    if perf.language.shell_voicings {
        3
    } else {
        4
    }
}

/// Realize the keys — the R7b control: one global voice path over every harmony the keys play in,
/// listening to the lead, and the material lines (answers, figures, unison) projected alone.
pub fn realize_keys(
    perf: &PerformancePlan,
    _plan: &CompositionPlan,
    world: &MusicWorld,
    lead: &[Note],
    seed: u64,
) -> Vec<Note> {
    let kp = keys_path(perf, world.voicing_spread, lead, keys_shell_n(perf));
    let mut out = keys_comp(perf, world, lead, seed, &kp);
    out.extend(keys_lines(perf, lead, keys_velocity(world)));
    finish_keys(out, perf)
}

/// The keys' final pass: onset order (stable), then lift off at harmony changes.
pub fn finish_keys(mut out: Vec<Note>, perf: &PerformancePlan) -> Vec<Note> {
    out.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    release_at_harmony_change(&mut out, &perf.chords);
    out
}

/// The coupled keys' final pass: as [`finish_keys`], but a note lifts off AT the next harmony when
/// it does not belong there (no half-beat of tolerated overhang), and a comping stab or hold — a
/// voicing chosen FOR its harmony — never rings into the next one at all, whatever its pitch class
/// (the lines — answers, figures, unison — may cross a change; they are lines).
pub fn finish_keys_coupled(mut out: Vec<Note>, perf: &PerformancePlan) -> Vec<Note> {
    out.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    release_with_overhang(&mut out, &perf.chords, 0.02);
    // A comping stab stops at the change; a HOLD is a suspension — its common tones ring across
    // (the release above already lifted the rest), which is what its witness hears.
    for n in out.iter_mut().filter(|n| n.prov.role_note == "comp") {
        let end = n.start_beat + n.dur_beats as f64;
        if let Some(next) = perf
            .chords
            .iter()
            .find(|c| c.start_beat > n.start_beat + 1e-6 && c.start_beat < end - 1e-6)
        {
            n.dur_beats = ((next.start_beat - n.start_beat) as f32 * 0.97).max(0.1);
        }
    }
    out
}

/// The keys' comping and holds on the voice path `kp`: stabs on the shared grid around the lead,
/// planned pushes/hits, sustained bars and the planned suspensions (not the material lines).
pub fn keys_comp(
    perf: &PerformancePlan,
    world: &MusicWorld,
    lead: &[Note],
    seed: u64,
    kp: &super::voicing::RolePath,
) -> Vec<Note> {
    let mut out = Vec::new();
    let mut rng = Rng::new(seed ^ 0x6E75_C0A9);
    let vel = keys_velocity(world);
    let shell_n = keys_shell_n(perf);
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
        // Listen to the lead: never stab on top of its onsets. And ask the stage per stab.
        steps.retain(|&(_, s)| {
            let b = AccentGrid::beat_of(bar, s);
            !lead_onset_near(lead, b) && perf.on_stage(Agent::Keys, b)
        });
        steps.sort_by(|a, b| b.0.total_cmp(&a.0));
        let max_stabs = match eb.keys {
            KeysMode::Stab => 2,
            KeysMode::Comp | KeysMode::Answer => {
                // The shared complexity budget: the keys spend only their own allowance (the lead's
                // statements and every planned answer and figure were reserved first). A stab
                // costs about one weighted onset.
                let free = perf.free_allowance(Agent::Keys, bar);
                ((free / 1.1).floor() as usize).min(if perf.language.shell_voicings {
                    3
                } else {
                    2
                })
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
                    let mut v_mult = if accent.hit >= 0.9 { 1.1 } else { 1.0 };
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
                    // The accent's semantic size: a faint arrival is left to bass and drums, a
                    // big one gets a harder stab.
                    if let Some(force) = perf.force_of(st) {
                        if force < 0.35 {
                            continue;
                        }
                        v_mult *= 0.9 + 0.3 * force;
                    }
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
    out
}

/// The keys' material lines, each projected alone (the R7b control): answers to calls, the keys'
/// own figures, and the planned ensemble unison.
pub fn keys_lines(perf: &PerformancePlan, lead: &[Note], vel: f32) -> Vec<Note> {
    let mut out = Vec::new();
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
    out.extend(keys_unison_notes(perf, lead, vel));
    out
}

/// The lowest / highest pitch a re-placed keys material line may sound.
const KEYS_LINE_RANGE: (Midi, Midi) = (55, 88);

/// The whole-line octave moves a coupled keys line may take, in tie-break order (identity first,
/// then smaller moves, then down before up).
const KEYS_LINE_SHIFTS: [Midi; 5] = [0, -12, 12, -24, 24];

/// The keys' material lines placed against the ONE harmonic state (Round VIII, coupled only). The
/// lines are the same projections [`keys_lines`] makes — onsets, durations, velocities and every
/// provenance stamp untouched — but each answer and each keys figure is heard against what already
/// sounds (the lead and the bass in `state`) before it is committed: the whole line takes the octave
/// with the fewest hard hazards, then the least counterpoint friction against the overlapping lead,
/// then the register nearest the keys' home. An answer's CONNECTOR that still collides may step to
/// the nearest stable pitch that keeps its contour; targets and arrivals move only with the whole
/// line, never on their own, and no event is ever dropped (the witness needs every one). Figures move by whole octaves only: a lead
/// answering a keys figure continues from `line_of`'s last pitch class. The unison is untouched.
/// Returns the notes and the explained decisions (the ledger is read-only here).
pub fn keys_lines_coupled(
    perf: &PerformancePlan,
    lead: &[Note],
    vel: f32,
    state: &HarmonicEnsembleState,
) -> (Vec<Note>, Vec<VerticalDecision>) {
    let mut out = Vec::new();
    let mut log = Vec::new();
    for (call, r) in perf.responses_for(Agent::Keys) {
        let mut line = answer_notes(perf, call, r, Agent::Keys, Role::Keys, vel * 1.05);
        place_line(perf, lead, state, &mut line, true, &mut log);
        out.extend(line);
    }
    // `figure_notes` emits each figure's notes contiguously; each run of one material is one line.
    let figures = figure_notes(perf, Agent::Keys, Role::Keys, vel * 1.05);
    for chunk in figures.chunk_by(|a, b| a.prov.material == b.prov.material) {
        let mut line = chunk.to_vec();
        place_line(perf, lead, state, &mut line, false, &mut log);
        out.extend(line);
    }
    out.extend(keys_unison_notes(perf, lead, vel));
    (out, log)
}

/// The counterpoint friction (in tenths) of `line` moved by `k` against the lead notes sounding at
/// each of its onsets: 3rds, 6ths, unisons and octaves are free, 4ths/5ths (and the tritone) cost 3,
/// 2nds/7ths 8; two consecutive onsets moving in parallel at the same minor 2nd / 9th cost 10 more.
/// (The per-onset classes are octave-invariant by construction; the parallel term is not.)
fn counterpoint_cost(lead: &[Note], line: &[Note], k: Midi) -> u32 {
    let mut cost = 0;
    let mut prev: Option<(Midi, Midi)> = None;
    for n in line {
        let t = n.start_beat;
        let p = n.pitch + k;
        let mut rel = None;
        for l in lead
            .iter()
            .filter(|l| l.start_beat <= t + 1e-6 && l.start_beat + l.dur_beats as f64 > t + 1e-6)
        {
            let d = p - l.pitch;
            cost += match d.abs() % 12 {
                0 | 3 | 4 | 8 | 9 => 0,
                1 | 2 | 10 | 11 => 8,
                _ => 3,
            };
            if matches!(d.abs(), 1 | 13) {
                rel = Some((d, p));
            }
        }
        if let (Some((d0, p0)), Some((d1, p1))) = (prev, rel) {
            if d0 == d1 && p0 != p1 {
                cost += 10;
            }
        }
        prev = rel;
    }
    cost
}

/// The hard hazards `n`, sounding at `pitch` for its (released) length, would create in `state`.
fn line_hazards(state: &HarmonicEnsembleState, n: &Note, pitch: Midi) -> Vec<Hazard> {
    let mut v = voice_of(n);
    v.pitch = pitch;
    state.hard_hazards(&v)
}

/// Place one keys material line against `state` (see [`keys_lines_coupled`]); `answer` enables
/// the connector substitution. Every move is appended to `log`.
fn place_line(
    perf: &PerformancePlan,
    lead: &[Note],
    state: &HarmonicEnsembleState,
    line: &mut [Note],
    answer: bool,
    log: &mut Vec<VerticalDecision>,
) {
    if line.is_empty() {
        return;
    }
    let kind = if answer { "answer" } else { "figure" };
    // Judge each note for as long as it will actually sound: the coupled keys' final pass lifts a
    // line note off at a change it does not belong to (pitch-class based, so octave-invariant).
    let mut sounding = line.to_vec();
    release_with_overhang(&mut sounding, &perf.chords, 0.02);
    let hazards_at = |k: Midi| -> usize {
        sounding
            .iter()
            .map(|n| line_hazards(state, n, n.pitch + k).len())
            .sum()
    };
    let mean = line.iter().map(|n| n.pitch as f64).sum::<f64>() / line.len() as f64;
    let home = super::material::home_register(Agent::Keys) as f64;
    let in_range = |k: Midi| {
        line.iter()
            .all(|n| (KEYS_LINE_RANGE.0..=KEYS_LINE_RANGE.1).contains(&(n.pitch + k)))
    };
    let judged: Vec<(Midi, usize, u32, f64)> = KEYS_LINE_SHIFTS
        .iter()
        .filter(|&&k| k == 0 || in_range(k))
        .map(|&k| {
            (
                k,
                hazards_at(k),
                counterpoint_cost(lead, &sounding, k),
                (mean + k as f64 - home).abs(),
            )
        })
        .collect();
    // Lexicographic (hazards, friction, register); the scan order is the tie-break. The identity
    // projection keeps every tie on the first two: register alone never moves a clean line — the
    // material's own placement is not a defect.
    let identity = judged[0];
    let best = judged
        .iter()
        .copied()
        .reduce(|b, c| {
            let key = |x: &(Midi, usize, u32, f64)| (x.1, x.2);
            match key(&c).cmp(&key(&b)).then(c.3.total_cmp(&b.3)) {
                std::cmp::Ordering::Less => c,
                _ => b,
            }
        })
        .filter(|b| (b.1, b.2) < (identity.1, identity.2))
        .unwrap_or(identity);
    let k = best.0;
    if k != 0 {
        let names = |k: Midi| {
            line.iter()
                .map(|n| note_name(n.pitch + k))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let why = sounding
            .iter()
            .flat_map(|n| {
                line_hazards(state, n, n.pitch)
                    .into_iter()
                    .map(|h| h.describe(n.pitch))
            })
            .collect::<Vec<_>>();
        log.push(VerticalDecision {
            beat: line[0].start_beat,
            role: Role::Keys,
            what: "octave",
            reason: format!(
                "keys {kind} [{}] -> [{}] ({k:+}): hard hazards {} -> {}, friction {} -> {} ({})",
                names(0),
                names(k),
                identity.1,
                best.1,
                identity.2,
                best.2,
                if why.is_empty() {
                    "parallel semitones against the lead".to_string()
                } else {
                    why.join("; ")
                }
            ),
        });
        for n in line.iter_mut() {
            n.pitch += k;
        }
    }
    // What each event MEANS where it sounds, read on the line as projected (octave-invariant).
    let roles = super::material::resolve_roles(
        &line
            .iter()
            .map(|n| (n.start_beat, Some(n.pitch), n.function))
            .collect::<Vec<_>>(),
        perf,
    );
    let accented = |n: &Note| {
        n.prov.actions.iter().any(|a| {
            perf.actions
                .get(a)
                .is_some_and(|x| matches!(x.kind, ActionKind::Push | ActionKind::Hit))
        })
    };
    // Per event: only a connector may be re-pitched on its own; targets, arrivals and accented
    // landings are load-bearing and keep their pitch. A residue nobody may touch is written down
    // as `kept` rather than sutured over.
    for j in 0..line.len() {
        let p = line[j].pitch;
        let hz = line_hazards(state, &sounding[j], p);
        if hz.is_empty() {
            continue;
        }
        let at = line[j].start_beat;
        let inner = j > 0 && j + 1 < line.len();
        let sub = (answer
            && inner
            && roles[j] == super::material::MaterialRole::Connector
            && !accented(&line[j]))
        .then(|| {
            let (prev, next) = (line[j - 1].pitch, line[j + 1].pitch);
            let ctx = perf.context_at(at)?;
            [p - 1, p + 1, p - 2, p + 2]
                .into_iter()
                .find(|&q| {
                    (KEYS_LINE_RANGE.0..=KEYS_LINE_RANGE.1).contains(&q)
                        && super::material::stable_for(Agent::Keys, ctx, pitch_class(q))
                        && (q - prev).signum() == (p - prev).signum()
                        && (next - q).signum() == (next - p).signum()
                        && line_hazards(state, &sounding[j], q).is_empty()
                })
                .map(|q| (q, function_over(ctx, q)))
        })
        .flatten();
        let why = hz
            .iter()
            .map(|h| h.describe(p))
            .collect::<Vec<_>>()
            .join("; ");
        match sub {
            Some((q, f)) => {
                log.push(VerticalDecision {
                    beat: at,
                    role: Role::Keys,
                    what: "substituted",
                    reason: format!(
                        "keys {kind} connector {} -> {}: {why}",
                        note_name(p),
                        note_name(q)
                    ),
                });
                line[j].pitch = q;
                line[j].function = f;
            }
            None => log.push(VerticalDecision {
                beat: at,
                role: Role::Keys,
                what: "kept",
                reason: format!(
                    "keys {kind} {} ({}) kept, no clean alternative: {why}",
                    note_name(p),
                    roles[j].label()
                ),
            }),
        }
    }
}

/// The keys' part of the planned ensemble unison figures: the shared line, at the keys' octave.
fn keys_unison_notes(perf: &PerformancePlan, lead: &[Note], vel: f32) -> Vec<Note> {
    let mut out = Vec::new();
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
            let p = nearest_stable(ctx, perf.region_at(at).degree_pitch(deg, 4));
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
pub fn land_on_accents(perf: &PerformancePlan, at: f64) -> (f64, ActionStamp) {
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

/// [`land_on_accents`], at most once per accent step: a second event near an already-landed step
/// keeps its own onset (a line's rhythm is never collapsed onto one beat).
pub fn land_once(perf: &PerformancePlan, at: f64, landed: &mut Vec<f64>) -> (f64, ActionStamp) {
    match land_on_accents(perf, at) {
        (t, st) if !st.is_empty() && !landed.iter().any(|&l| (l - t).abs() < 1e-6) => {
            landed.push(t);
            (t, st)
        }
        (_, st) if !st.is_empty() => (at, ActionStamp::NONE),
        other => other,
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
    let mut landed: Vec<f64> = Vec::new();
    super::material::line_of(m, agent, r.start_beat, r.start_beat + r.dur_beats, perf)
        .into_iter()
        .filter_map(|(at, d, p, accent)| {
            let p = p?;
            let ctx = perf.context_at(at)?;
            if !perf.on_stage(agent, at) {
                return None;
            }
            let (at, accents) = land_once(perf, at, &mut landed);
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
        let mut landed: Vec<f64> = Vec::new();
        for (at, d, p, accent) in super::material::line_of(m, agent, m.start_beat, until, perf) {
            let (Some(p), Some(ctx)) = (p, perf.context_at(at)) else {
                continue;
            };
            if !perf.on_stage(agent, at) {
                continue;
            }
            // Land on a planned accent — once: a second event near the same step keeps its own
            // onset, so the figure's rhythm is never collapsed.
            let (at, accents) = land_once(perf, at, &mut landed);
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

/// Realize the pad — the R7b control: its own voice path, hearing nobody.
pub fn realize_pad(
    perf: &PerformancePlan,
    _plan: &CompositionPlan,
    world: &MusicWorld,
) -> Vec<Note> {
    // One global voice path over the sounding harmonies; each bar's mode picks its shape family.
    realize_pad_on(perf, world, &pad_path(perf, world.voicing_spread), None)
}

/// Whether a pad pitch's release tail over `[a, b)` would meet ANOTHER player a minor 2nd / 9th
/// away (the coupled realization supplies it; the pad's own next voicing is a legato crossfade).
pub type TailGuard<'g> = &'g dyn Fn(f64, f64, Midi) -> bool;

/// Realize the pad on the voice path `pp` (its own, or the joint support path). With a `guard`
/// (the coupled realization), a voice whose release tail would clash with another player in the
/// next harmony starts its release early by that tail.
pub fn realize_pad_on(
    perf: &PerformancePlan,
    world: &MusicWorld,
    pp: &super::voicing::RolePath,
    guard: Option<TailGuard>,
) -> Vec<Note> {
    let mut out = Vec::new();
    let vel = (0.4 * world.base_dynamic).clamp(0.05, 1.0);
    for (ci, ctx) in perf.contexts.iter().enumerate() {
        let Some(eb) = perf.bar_at(ctx.start_beat) else {
            continue;
        };
        let dur = ctx.dur_beats * 0.98;
        let v = pp.voicing(ci, ctx);
        // Round VIII (coupled): a pad voice whose release tail would ring a minor 2nd / 9th against
        // ANOTHER player at the next harmony — or a minor 9th against the pad's OWN next voicing (a
        // semitone step into it is a legato crossfade; a 9th away is a smear) — starts its release
        // early by that tail (to the masking floor), so it has faded under the next chord's attack.
        // Held common tones keep their full length.
        let next = perf
            .contexts
            .get(ci + 1)
            .filter(|n| (n.start_beat - (ctx.start_beat + ctx.dur_beats as f64)).abs() < 1e-6);
        let next_voicing = next.and_then(|_| pp.at(ci + 1)).map(|v| v.voices);
        let tail =
            (super::sonority::release_tail_secs(&world.pad, super::sonority::MASKING_FLOOR_DB)
                * world.tempo_bpm.max(1.0) as f64
                / 60.0) as f32;
        let pad_dur = |p: Midi, d: f32| -> f32 {
            let (Some(n), Some(guard)) = (next, guard) else {
                return d;
            };
            let held = next_voicing.as_ref().is_some_and(|nv| nv.contains(&p));
            let smear = next_voicing
                .as_ref()
                .is_some_and(|nv| nv.iter().any(|&q| (q - p).abs() == 13));
            if held || !(smear || guard(n.start_beat, n.start_beat + tail as f64, p)) {
                return d;
            }
            (d - tail).max(0.5 * d)
        };
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
                pad_dur(p, d).max(0.1),
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

#[cfg(test)]
mod tests {
    use super::super::contract::CompositionGrammar;
    use super::super::functor::{compose_full, realize_performance, Composition};
    use super::super::ids::{InteractionId, MaterialId};
    use super::super::interaction::Transform;
    use super::super::material::{
        resolve_roles, role_counts, InteractionMaterial, MaterialEvent, MaterialRole,
        MaterialSource, RoleEvent,
    };
    use super::super::melody::realize_lead;
    use super::super::performance::{EnsembleCoupling, PerformanceOptions};
    use super::super::score::Score;
    use super::super::semantic::deflected_lift_trace;
    use super::super::sonority::{
        classify_clash, plan_sonority, voices_of, Clash, ColorPolicy, EnsembleSonorityDiagnostics,
        VerticalClass, Voice, MIN_OVERLAP_BEATS,
    };
    use super::super::witness::{audit, interaction_receipts};
    use super::*;

    const SEED: u64 = 2112;

    fn flagship(world: &MusicWorld, coupling: EnsembleCoupling) -> Composition {
        compose_full(
            &deflected_lift_trace(120.0),
            world,
            SEED,
            Some(CompositionGrammar::DeflectedLift),
            PerformanceOptions {
                coupling,
                ..PerformanceOptions::default()
            },
        )
    }

    fn measure(
        score: &Score,
        perf: &PerformancePlan,
        world: &MusicWorld,
    ) -> EnsembleSonorityDiagnostics {
        EnsembleSonorityDiagnostics::measure(
            score,
            &perf.contexts,
            &ColorPolicy::for_world(world.id, &perf.language),
            &[],
        )
    }

    fn pair(d: &EnsembleSonorityDiagnostics, key: &str) -> usize {
        d.role_pairs.get(key).copied().unwrap_or(0)
    }

    fn onsets(line: &[Note]) -> Vec<f64> {
        line.iter().map(|n| n.start_beat).collect()
    }

    fn signs(line: &[Note]) -> Vec<i32> {
        line.windows(2)
            .map(|w| (w[1].pitch - w[0].pitch).signum())
            .collect()
    }

    fn roles_of(line: &[Note], perf: &PerformancePlan) -> Vec<MaterialRole> {
        let ev: Vec<RoleEvent> = line
            .iter()
            .map(|n| (n.start_beat, Some(n.pitch), n.function))
            .collect();
        resolve_roles(&ev, perf)
    }

    /// The ledger exactly as `realize_coupled` holds it when the keys' lines are placed: the lead,
    /// then the coupled bass.
    fn ledger<'a>(
        world: &MusicWorld,
        plan: &CompositionPlan,
        perf: &'a PerformancePlan,
        lead: &[Note],
    ) -> HarmonicEnsembleState<'a> {
        let policy = ColorPolicy::for_world(world.id, &perf.language);
        let mut st = HarmonicEnsembleState::new(
            &perf.contexts,
            plan_sonority(&perf.contexts, lead, &policy),
            policy,
        );
        st.commit(lead);
        let bass = super::super::bass::realize_bass_coupled(perf, plan, world, lead, &mut st);
        st.commit(&bass);
        st
    }

    /// The coupled flagship at 5db3296, when the keys' material lines were still projected alone:
    /// `(world, hard_total, the informative receipts' interactions, their min margin)`. Every world
    /// measured keys/lead = keys/pad = bass/keys = 0 unowned, 54/54 witnessed, melody_repairs = 0.
    const BEFORE: [(&str, usize, &[u32], f32); 3] = [
        ("BLACK_ICE", 0, &[2, 3, 4, 6, 7, 8, 10], 0.167),
        ("VAPOR95", 3, &[2, 3, 6, 7, 8, 10], 0.500),
        ("SWISS_SIGNAL", 0, &[2, 3, 6, 7, 8, 10], 0.300),
    ];

    /// Rooms whose flagship song changed in Round IX (the SongMap owns the theme): the coupled
    /// control's exact `(keys/lead unowned, hard_total)` on the new song.
    const R9_SONG_CHANGED: [(&str, (usize, usize)); 1] = [("BLACK_ICE", (1, 1))];

    /// The flagship, coupled, every world: the keys' answers and figures are placed against the band
    /// and nothing the witness reads is lost — no unowned keys/lead collision, no more hard problems
    /// than before, every action witnessed, every informative receipt still about its caller, the
    /// lead unrepaired; each answer keeps its onsets, its contour signs and the meaning of every
    /// non-connector event, and each figure keeps `line_of`'s pitch classes.
    #[test]
    fn the_coupled_keys_lines_are_placed_against_the_band_without_losing_their_meaning() {
        for world in MusicWorld::all() {
            let &(_, hard_before, informative_before, min_before) = BEFORE
                .iter()
                .find(|b| b.0 == world.name)
                .expect("a flagship world");
            let c = flagship(&world, EnsembleCoupling::CoupledR8);
            let p = &c.perf;
            let d = measure(&c.score, p, &world);
            let lead: Vec<Note> = c
                .score
                .notes
                .iter()
                .filter(|n| n.role == Role::Lead)
                .copied()
                .collect();
            let realized = |mid: MaterialId, tag: &str| -> Vec<Note> {
                let mut v: Vec<Note> = c
                    .score
                    .notes
                    .iter()
                    .filter(|n| {
                        n.role == Role::Keys
                            && n.prov.material == Some(mid)
                            && n.prov.role_note == tag
                    })
                    .copied()
                    .collect();
                v.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
                v
            };
            let overlaps = |line: &[Note], of: &dyn Fn(&Note) -> bool| {
                line.iter().any(|k| {
                    lead.iter().any(|l| {
                        of(l)
                            && l.start_beat < k.start_beat + k.dur_beats as f64 - 1e-6
                            && l.start_beat + l.dur_beats as f64 > k.start_beat + 1e-6
                    })
                })
            };
            let (mut answers, mut over_caller, mut over_lead) = (0, 0, 0);
            for (call, r) in p.responses_for(Agent::Keys) {
                let Some(mid) = r.material else { continue };
                let alone = answer_notes(p, call, r, Agent::Keys, Role::Keys, 1.0);
                let got = realized(mid, "answer");
                if alone.is_empty() {
                    continue;
                }
                answers += 1;
                over_caller += overlaps(&got, &|l| l.prov.material == Some(call.material)) as usize;
                over_lead += overlaps(&got, &|_| true) as usize;
                assert_eq!(onsets(&alone), onsets(&got), "{}", world.name);
                assert_eq!(signs(&alone), signs(&got), "{}", world.name);
                for (a, g) in roles_of(&alone, p).iter().zip(roles_of(&got, p)) {
                    if *a != MaterialRole::Connector {
                        assert_eq!(*a, g, "{}: an answer target changed meaning", world.name);
                    }
                }
            }
            let figures = figure_notes(p, Agent::Keys, Role::Keys, 1.0);
            let mut n_figures = 0;
            for alone in figures.chunk_by(|a, b| a.prov.material == b.prov.material) {
                let got = realized(alone[0].prov.material.expect("stamped"), "figure");
                n_figures += 1;
                assert_eq!(onsets(alone), onsets(&got), "{}", world.name);
                assert_eq!(
                    alone
                        .iter()
                        .map(|n| pitch_class(n.pitch))
                        .collect::<Vec<_>>(),
                    got.iter().map(|n| pitch_class(n.pitch)).collect::<Vec<_>>(),
                    "{}: a keys figure left line_of's pitch classes",
                    world.name
                );
            }
            let decided = |what: &str| {
                c.score
                    .vertical_decisions
                    .iter()
                    .filter(|v| v.role == Role::Keys && v.what == what)
                    .count()
            };
            let r = audit(p, &c.score);
            let rec = interaction_receipts(p, &c.score);
            let min_margin = rec
                .iter()
                .filter(|x| x.informative())
                .map(|x| x.margin())
                .fold(f32::INFINITY, f32::min);
            eprintln!(
                "{}: keys answers={answers} (overlap caller {over_caller}, lead {over_lead}) \
                 figures={n_figures} | octave={} substituted={} kept={} | keys/lead={} keys/pad={} \
                 bass/keys={} hard_total={} (before {hard_before}) | witnessed {}/{} | min informative \
                 margin {min_margin:.3} (before {min_before:.3}) | repairs={} | roles {:?}",
                world.name,
                decided("octave"),
                decided("substituted"),
                decided("kept"),
                pair(&d, "keys/lead"),
                pair(&d, "keys/pad"),
                pair(&d, "bass/keys"),
                d.hard_total(),
                r.witnessed(),
                r.total(),
                c.score.melody_repairs,
                role_counts(p, &c.score.notes, Role::Keys),
            );
            assert_eq!(r.witnessed(), r.total(), "{}: {}", world.name, r.report());
            assert_eq!(c.score.melody_repairs, 0, "{}", world.name);
            if let Some(&(keys_lead, hard)) = R9_SONG_CHANGED
                .iter()
                .find(|x| x.0 == world.name)
                .map(|x| &x.1)
            {
                // Round IX: this room now plays the SONG's own material, so the 5db3296 history is
                // not its history. The frozen R8 solver (the rejected control, not revisited) is
                // held to its exact measurements on the new song, and every informative receipt
                // must still be about its caller.
                assert_eq!(
                    (pair(&d, "keys/lead"), d.hard_total()),
                    (keys_lead, hard),
                    "{}",
                    world.name
                );
                assert!(
                    rec.iter()
                        .filter(|x| x.informative())
                        .all(|x| x.margin() > 0.0),
                    "{}",
                    world.name
                );
                continue;
            }
            assert_eq!(pair(&d, "keys/lead"), 0, "{}", world.name);
            assert!(d.hard_total() <= hard_before, "{}", world.name);
            for &ix in informative_before {
                let x = rec
                    .iter()
                    .find(|x| x.interaction == InteractionId(ix))
                    .unwrap_or_else(|| panic!("{}: receipt {ix} vanished", world.name));
                assert!(
                    x.informative() && x.margin() > 0.0,
                    "{}: receipt {ix} = {x:?}",
                    world.name
                );
            }
            assert!(min_margin >= min_before - 0.05, "{}", world.name);
        }
    }

    /// The keys notes of material `mid` in `score`, in onset order.
    fn injected(score: &Score, mid: MaterialId) -> Vec<Note> {
        let mut v: Vec<Note> = score
            .notes
            .iter()
            .filter(|n| n.role == Role::Keys && n.prov.material == Some(mid))
            .copied()
            .collect();
        v.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
        v
    }

    /// Unowned minor 2nds / 9ths between the keys notes of material `mid` and the lead in `score`,
    /// judged exactly as the audit judges them (the realized voices, [`classify_clash`] over the
    /// harmony where they meet, at least [`MIN_OVERLAP_BEATS`] of overlap).
    fn unowned_against_lead(score: &Score, perf: &PerformancePlan, mid: MaterialId) -> usize {
        let voices = voices_of(score, &perf.contexts);
        let mine = injected(score, mid);
        let is_mine = |v: &Voice| {
            v.role == Role::Keys
                && mine
                    .iter()
                    .any(|n| (n.start_beat - v.start).abs() < 1e-9 && n.pitch == v.pitch)
        };
        let mut n = 0;
        for a in voices.iter().filter(|v| is_mine(v)) {
            for b in voices.iter().filter(|v| v.role == Role::Lead) {
                let (s, e) = (a.start.max(b.start), a.end.min(b.end));
                if e - s < MIN_OVERLAP_BEATS - 1e-9 || Clash::of(a.pitch, b.pitch).is_none() {
                    continue;
                }
                let Some(ctx) = perf.context_at(s) else {
                    continue;
                };
                n += (classify_clash(ctx, a, b, None) == VerticalClass::UnownedCollision) as usize;
            }
        }
        n
    }

    /// Re-time the keys answer of interaction `ix` to `[at, at + 0.5·len)` with a fresh `steps`
    /// material (a quote-shaped line on eighths), derived from the same call. Returns its id.
    fn inject_answer(p: &mut PerformancePlan, ix: usize, at: f64, steps: &[i32]) -> MaterialId {
        let (call_mid, call_end) = {
            let call = &p.interactions[ix].call;
            (call.material, call.end_beat)
        };
        let mid = MaterialId(p.materials.len() as u32);
        p.materials.push(InteractionMaterial {
            id: mid,
            owner: Agent::Keys,
            source: MaterialSource::Derived {
                from: call_mid,
                transform: Transform::Quote,
            },
            start_beat: at,
            events: steps
                .iter()
                .enumerate()
                .map(|(j, &s)| MaterialEvent {
                    onset: j as f64 * 0.5,
                    dur: 0.45,
                    accent: 0.7,
                    step: Some(s),
                })
                .collect(),
        });
        let r = p.interactions[ix].response.as_mut().expect("answered");
        r.start_beat = at;
        r.dur_beats = steps.len() as f64 * 0.5;
        r.latency = at - call_end;
        r.overlap = r.latency < 0.0;
        r.transform = Transform::Quote;
        r.material = Some(mid);
        mid
    }

    /// DISCRIMINATING: a keys answer re-timed to overlap its caller's held lead note. Projected alone
    /// at the keys' home register (the independent control — `keys_lines`) it lands an unowned minor
    /// 2nd / 9th on that note; placed against the ledger it does not — and it keeps its onsets, its
    /// contour signs, its durations, velocities and every stamp (the transform relation stands). The
    /// whole-score audit agrees in both arms.
    #[test]
    fn a_keys_answer_over_its_callers_held_note_is_placed_not_summed() {
        const PATTERNS: [&[i32]; 6] = [
            &[0, 1, 2],
            &[0, -1, -2],
            &[0, 2, 1],
            &[0, -2, -1],
            &[0, 1, 3],
            &[0, -1, -3],
        ];
        let mut exercised = 0;
        for world in MusicWorld::all() {
            let c = flagship(&world, EnsembleCoupling::CoupledR8);
            let lead = realize_lead(&c.perf, &c.song.plan).notes;
            let state = ledger(&world, &c.song.plan, &c.perf, &lead);
            let vel = keys_velocity(&world);
            let clash_on = |n: &Note, held: Midi| {
                state.hard_hazards(&voice_of(n)).iter().any(|h| {
                    matches!(h, Hazard::Clash { with: Role::Lead, pitch, .. } if *pitch == held)
                })
            };
            let mut found = None;
            'search: for (ix, i) in c.perf.interactions.iter().enumerate() {
                let Some(r) = i.response else { continue };
                if r.responder != Agent::Keys
                    || r.material.is_none()
                    || i.call.initiator != Agent::Lead
                {
                    continue;
                }
                for l in lead
                    .iter()
                    .filter(|l| l.prov.material == Some(i.call.material) && l.dur_beats >= 0.75)
                {
                    for off in [0.0, 0.25, 0.5] {
                        for steps in PATTERNS {
                            let mut q = c.perf.clone();
                            let mid = inject_answer(&mut q, ix, l.start_beat + off, steps);
                            let alone: Vec<Note> = keys_lines(&q, &lead, vel)
                                .into_iter()
                                .filter(|n| n.prov.material == Some(mid))
                                .collect();
                            if alone.len() == steps.len()
                                && alone.iter().any(|n| clash_on(n, l.pitch))
                            {
                                found = Some((q, mid, *l, alone));
                                break 'search;
                            }
                        }
                    }
                }
            }
            let Some((mut q, mid, held, alone)) = found else {
                eprintln!("{}: no caller-overlapping window for the probe", world.name);
                continue;
            };
            // The lead and the ledger do not depend on the keys' answer.
            assert_eq!(
                onsets(&realize_lead(&q, &c.song.plan).notes),
                onsets(&lead),
                "{}",
                world.name
            );
            let (placed, log) = keys_lines_coupled(&q, &lead, vel, &state);
            let placed: Vec<Note> = placed
                .into_iter()
                .filter(|n| n.prov.material == Some(mid))
                .collect();
            let name = |v: &[Note]| v.iter().map(|n| note_name(n.pitch)).collect::<Vec<_>>();
            eprintln!(
                "{}: answer over held lead {} at {:.2}: alone {:?} -> placed {:?}; decisions {:?}",
                world.name,
                note_name(held.pitch),
                held.start_beat,
                name(&alone),
                name(&placed),
                log.iter()
                    .map(|d| format!("{} {}", d.what, d.reason))
                    .collect::<Vec<_>>()
            );
            // The control collides (the test discriminates)...
            assert!(alone.iter().any(|n| clash_on(n, held.pitch)));
            // ...the placed line does not, against the lead at all...
            assert!(
                placed.iter().all(
                    |n| !state.hard_hazards(&voice_of(n)).iter().any(|h| matches!(
                        h,
                        Hazard::Clash {
                            with: Role::Lead,
                            ..
                        }
                    ))
                ),
                "{}",
                world.name
            );
            // ...and it is the same answer: onsets, contour, rhythm, dynamics, every stamp.
            assert_eq!(onsets(&placed), onsets(&alone), "{}", world.name);
            assert_eq!(signs(&placed), signs(&alone), "{}", world.name);
            for (a, b) in alone.iter().zip(&placed) {
                assert_eq!((a.dur_beats, a.velocity), (b.dur_beats, b.velocity));
                assert_eq!(format!("{:?}", a.prov), format!("{:?}", b.prov));
            }
            assert!(
                log.iter()
                    .any(|d| matches!(d.what, "octave" | "substituted") && !d.reason.is_empty()),
                "{}: the move is explained",
                world.name
            );
            // The whole realized score, both arms: the audit's own theory on the injected notes.
            let score_c = realize_performance(&c.song, &world, &q);
            let realized: Vec<Midi> = injected(&score_c, mid).iter().map(|n| n.pitch).collect();
            assert_eq!(
                realized,
                placed.iter().map(|n| n.pitch).collect::<Vec<_>>(),
                "{}: the realized coupled answer is the placed one",
                world.name
            );
            let coupled_pairs = (
                pair(&measure(&c.score, &c.perf, &world), "keys/lead"),
                pair(&measure(&score_c, &q, &world), "keys/lead"),
            );
            q.coupling = EnsembleCoupling::Independent;
            let score_i = realize_performance(&c.song, &world, &q);
            let (on_i, on_c) = (
                unowned_against_lead(&score_i, &q, mid),
                unowned_against_lead(&score_c, &q, mid),
            );
            eprintln!(
                "{}: injected answer's unowned m2/m9 against the lead: independent {on_i}, \
                 coupled {on_c}; coupled keys/lead unowned base -> injected {coupled_pairs:?}",
                world.name
            );
            assert!(on_i >= 1, "{}: the control no longer collides", world.name);
            assert_eq!(on_c, 0, "{}", world.name);
            assert_eq!(coupled_pairs.1, coupled_pairs.0, "{}", world.name);
            exercised += 1;
        }
        assert!(exercised >= 1, "the probe found no window in any world");
    }

    /// A connector no octave can save (a hand-built ledger holds lead tones a semitone either side
    /// of it, so every whole-line move still collides) steps to a stable pitch that keeps its
    /// contour with both sides; the answer's first and last events — its targets — are never
    /// re-pitched relative to each other, and its onsets stand. (A PASSING connector between two
    /// scale neighbours has only chromatic sign-keeping alternatives within ±2, which are rarely
    /// stable; a NEIGHBOUR-shaped one has room — so the probe uses neighbour figures.)
    #[test]
    fn a_cornered_connector_steps_aside_and_keeps_the_contour() {
        let world = &MusicWorld::all()[0];
        let c = flagship(world, EnsembleCoupling::CoupledR8);
        let vel = keys_velocity(world);
        let ix = c
            .perf
            .interactions
            .iter()
            .position(|i| {
                i.response
                    .is_some_and(|r| r.responder == Agent::Keys && r.material.is_some())
            })
            .expect("a keys answer");
        let mut tried = 0;
        for at in (8..c.perf.contexts.len().min(100) * 2).map(|b| b as f64) {
            for steps in [&[0, 1, 0][..], &[0, -1, 0][..]] {
                let mut q = c.perf.clone();
                let mid = inject_answer(&mut q, ix, at, steps);
                let alone: Vec<Note> = keys_lines(&q, &[], vel)
                    .into_iter()
                    .filter(|n| n.prov.material == Some(mid))
                    .collect();
                if alone.len() != 3 || roles_of(&alone, &q)[1] != MaterialRole::Connector {
                    continue;
                }
                tried += 1;
                let m = alone[1];
                let held = |p: Midi| {
                    let mut n = Note::new(
                        m.start_beat,
                        m.dur_beats,
                        p,
                        0.8,
                        Role::Lead,
                        Provenance::new(super::super::form::SectionKind::A),
                    );
                    n.function = Some(PitchFunction::ChordTone);
                    n
                };
                let lead = [held(m.pitch - 1), held(m.pitch + 1)];
                let mut state =
                    HarmonicEnsembleState::new(&q.contexts, Vec::new(), ColorPolicy::lenient());
                state.commit(&lead);
                let (placed, log) = keys_lines_coupled(&q, &lead, vel, &state);
                let placed: Vec<Note> = placed
                    .into_iter()
                    .filter(|n| n.prov.material == Some(mid))
                    .collect();
                let Some(sub) = log.iter().find(|d| d.what == "substituted") else {
                    continue;
                };
                eprintln!(
                    "at {at}: {:?} -> {:?}: {}",
                    alone.iter().map(|n| note_name(n.pitch)).collect::<Vec<_>>(),
                    placed
                        .iter()
                        .map(|n| note_name(n.pitch))
                        .collect::<Vec<_>>(),
                    sub.reason
                );
                let k = placed[0].pitch - alone[0].pitch;
                assert_eq!(k % 12, 0, "the line moves by whole octaves");
                assert_eq!(
                    placed[2].pitch - alone[2].pitch,
                    k,
                    "the arrival is not re-pitched"
                );
                assert!((1..=2).contains(&(placed[1].pitch - (m.pitch + k)).abs()));
                assert_eq!(onsets(&placed), onsets(&alone));
                assert_eq!(signs(&placed), signs(&alone));
                assert!(state.hard_hazards(&voice_of(&placed[1])).is_empty());
                assert!(super::super::material::stable_for(
                    Agent::Keys,
                    q.context_at(m.start_beat).expect("a harmony"),
                    pitch_class(placed[1].pitch)
                ));
                return;
            }
        }
        panic!("no connector could step aside ({tried} cornered connectors tried)");
    }

    /// Each event of a line is labelled by what it IS where it sounds, in precedence order.
    #[test]
    fn material_roles_name_what_each_event_is_harmonically() {
        let c = flagship(&MusicWorld::all()[0], EnsembleCoupling::CoupledR8);
        let p = &c.perf;
        let t0 = p
            .contexts
            .iter()
            .find(|x| {
                !x.palette.tensions.is_empty()
                    && !x.palette.guide_tones.is_empty()
                    && x.dur_beats >= 1.0
            })
            .map(|x| x.start_beat.ceil())
            .expect("a context with tensions");
        let at = |b: f64| p.context_at(b).expect("a harmony");
        let (g, r, t) = (
            at(t0).palette.guide_tones[0],
            at(t0 + 1.0).chord.root_pc,
            at(t0 + 2.0).palette.tensions[0],
        );
        let ev: Vec<RoleEvent> = vec![
            (t0, Some(60 + g), None),
            (t0 + 1.0, Some(72 + r), None),
            (t0 + 2.0, Some(84 + t), None),
            (t0 + 3.0, Some(50), Some(PitchFunction::DiatonicPassing)),
            (t0 + 4.0, Some(60), Some(PitchFunction::Suspension)),
            (t0 + 5.0, Some(67), None),
        ];
        assert_eq!(
            resolve_roles(&ev, p),
            vec![
                MaterialRole::GuideTarget,
                MaterialRole::StructuralTarget,
                MaterialRole::ColorTarget,
                MaterialRole::Connector,
                MaterialRole::Tendency,
                MaterialRole::Arrival,
            ]
        );
        // An off-beat root reached and left by step is a connector, whatever it is in the chord.
        let root = 72 + at(t0 + 0.5).chord.root_pc;
        let passing: Vec<RoleEvent> = vec![
            (t0, Some(root - 2), None),
            (t0 + 0.5, Some(root), None),
            (t0 + 1.0, Some(root + 2), None),
        ];
        assert_eq!(resolve_roles(&passing, p)[1], MaterialRole::Connector);
    }
}

#[cfg(test)]
mod r12_landing_witness {
    #[test]
    #[ignore = "R12 pre-intervention falsifier: chord landing uses color-inclusive helper"]
    fn chord_landing_must_choose_a_chord_member() {
        use crate::audio::human_music::{context, harmony::ChordSpan, theory::*};
        let chord = Chord::new(0, Quality::Maj7);
        let contexts = context::analyze(
            &[ChordSpan::test(0.0, 4.0, chord)],
            &Scale::new(0, Mode::Ionian),
        );
        let ctx = &contexts[0];
        let p = 60 + ctx.palette.tensions[0];
        assert!(chord.contains_pc(pitch_class(super::nearest_stable(ctx, p))));
    }
}
