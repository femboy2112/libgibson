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
use super::mass::{ExposureClass, TemporalMass};
use super::performance::{AccentGrid, Call, KeysMode, PadMode, PerformancePlan, Response, STEPS};
use super::plan::CompositionPlan;
use super::rng::Rng;
use super::score::{Note, PitchFunction, Provenance, Role};
use super::theory::{note_name, pitch_class, Midi};
use super::voicing::{keys_path, pad_path, stab_voices};
use super::world::MusicWorld;

/// The function of `pitch` over `ctx`: a chord tone, a licensed tension, or `None`.
fn function_over(ctx: &HarmonicContext, pitch: Midi) -> Option<PitchFunction> {
    super::pitch::source_stable_function(ctx.chord, &ctx.palette.tensions, pitch)
}

/// The nearest pitch to `target` that is stable over `ctx` (a chord tone or licensed tension).
fn nearest_stable(ctx: &HarmonicContext, target: Midi) -> Midi {
    (0..=12)
        .flat_map(|d| [target - d, target + d])
        .find(|&p| ctx.palette.is_stable(pitch_class(p)))
        .unwrap_or(target)
}

/// The nearest actual chord member; available colour does not satisfy this contract.
fn nearest_chord_tone(ctx: &HarmonicContext, target: Midi) -> Midi {
    ctx.chord.nearest_chord_tone(target)
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

/// The support release the plan's [`super::policy::FunctionPolicy`] declares, for the bass and
/// the keys (neither declares a suspension). Archived: [`release_at_harmony_change`], a tail may
/// ring half a beat into a harmony it does not belong to. Earned: a note lifts off at the first
/// harmony change it does not belong to, however short its tail.
pub fn release_support(notes: &mut [Note], perf: &PerformancePlan) {
    match perf.functions {
        super::policy::FunctionPolicy::Archived => {
            release_at_harmony_change(notes, &perf.chords);
        }
        super::policy::FunctionPolicy::Earned => {
            for n in notes.iter_mut() {
                let end = n.start_beat + n.dur_beats as f64;
                if let Some(foreign) = perf.chords.iter().find(|c| {
                    c.start_beat > n.start_beat + 1e-6
                        && c.start_beat < end - 1e-9
                        && !c.chord.contains_pc(pitch_class(n.pitch))
                }) {
                    n.dur_beats = (foreign.start_beat - n.start_beat) as f32 * 0.97;
                }
            }
        }
    }
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

/// Round XII keys: unchanged voicing machinery, path-aware material, and boundary-safe stabs.
pub fn realize_keys_temporal(
    perf: &PerformancePlan,
    _plan: &CompositionPlan,
    world: &MusicWorld,
    lead: &[Note],
    seed: u64,
) -> Vec<Note> {
    let kp = keys_path(perf, world.voicing_spread, lead, keys_shell_n(perf));
    realize_keys_temporal_on(perf, world, lead, seed, &kp)
}

/// [`realize_keys_temporal`] on the voice path `kp`.
pub(crate) fn realize_keys_temporal_on(
    perf: &PerformancePlan,
    world: &MusicWorld,
    lead: &[Note],
    seed: u64,
    kp: &super::voicing::RolePath,
) -> Vec<Note> {
    let mut out = keys_comp_impl(perf, world, lead, seed, kp, true, None);
    out.extend(keys_lines_impl(perf, lead, keys_velocity(world), true));
    finish_keys(out, perf)
}

/// Source-level comping with authored lead ownership and the final acoustic lead kept separate.
/// Retiming a grace does not donate its rhythmic reservation to a new keys attack. Shared
/// material lines still inherit the final lead's actual gesture.
pub fn realize_keys_owned(
    perf: &PerformancePlan,
    _plan: &CompositionPlan,
    world: &MusicWorld,
    lead: &[Note],
    ownership: &super::occupancy::AuthoredOccupancy,
    seed: u64,
) -> Vec<Note> {
    let kp = keys_path(perf, world.voicing_spread, lead, keys_shell_n(perf));
    realize_keys_owned_on(perf, world, lead, ownership, seed, &kp)
}

/// [`realize_keys_owned`] on the voice path `kp`.
pub(crate) fn realize_keys_owned_on(
    perf: &PerformancePlan,
    world: &MusicWorld,
    lead: &[Note],
    ownership: &super::occupancy::AuthoredOccupancy,
    seed: u64,
    kp: &super::voicing::RolePath,
) -> Vec<Note> {
    let mut out = keys_comp_impl(perf, world, lead, seed, kp, true, Some(ownership));
    out.extend(keys_lines_impl(perf, lead, keys_velocity(world), true));
    finish_keys(out, perf)
}

/// Choose a pitch for the complete written gate. Only stabs whose original pitch would
/// cross into foreign harmony change; select the nearest available common tone before emission.
fn gate_pitch(perf: &PerformancePlan, beat: f64, dur: f32, pitch: Midi) -> Midi {
    let Some(ctx) = perf.context_at(beat) else {
        return pitch;
    };
    let legal = |p: Midi| {
        ctx.palette.is_stable(pitch_class(p))
            && perf
                .contexts
                .iter()
                .filter(|c| {
                    c.start_beat > beat + 1e-6 && c.start_beat < beat + f64::from(dur) - 1e-6
                })
                .all(|c| c.chord.contains_pc(pitch_class(p)))
    };
    if legal(pitch) {
        return pitch;
    }
    (1..=12)
        .flat_map(|d| [pitch - d, pitch + d])
        .find(|&p| legal(p))
        .unwrap_or(pitch)
}

/// Round XIII: the support voicings' temporal-mass contract (the opt-in `perform_mass` arm).
///
/// A pad or keys voicing colour — an available tension the sounding harmony does not write —
/// whose [`TemporalMass`] asserts it into the chord (struck with the harmony's own attack, or
/// dwelt on long enough) has become part of the perceived chord identity, so the song must have
/// authored it. When it has not, the smallest intervention that pays the rent wins, in order:
/// shorten it (in grid steps, never below one, only if that alone ends the assertion); omit it
/// (when the voicing still sounds a guide tone of the harmony and has not already lost a voice
/// here: a second omission would revoice the bed, Round VIII's failure); or move it to the
/// nearest written chord tone the voicing does not already sound. Written extensions, lines (answers, figures,
/// unison) and non-asserted colours are untouched. This is the realizer's own rule — pitch-class
/// authorship — not the audit's reconstruction, which grades the result independently.
///
/// Returns the gate's own ledger, one entry per decision, in decision order.
pub fn gate_support_mass(
    perf: &PerformancePlan,
    world: &MusicWorld,
    pad: &mut Vec<Note>,
    keys: &mut Vec<Note>,
) -> Vec<SupportMassEdit> {
    let mut ledger = gate_role_mass(perf, world, pad);
    ledger.extend(gate_role_mass(perf, world, keys));
    ledger
}

/// What the support mass gate did to one asserted, unwritten voicing colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SupportMassAction {
    /// Released early, at this written length (beats).
    Shortened(f32),
    Omitted,
    /// Moved to this written chord tone.
    Moved(Midi),
}

/// One support mass gate decision: the note as realized, its mass there, and the action.
#[derive(Debug, Clone, Copy)]
pub struct SupportMassEdit {
    pub note: Note,
    pub mass: TemporalMass,
    pub action: SupportMassAction,
}

fn gate_role_mass(
    perf: &PerformancePlan,
    world: &MusicWorld,
    notes: &mut Vec<Note>,
) -> Vec<SupportMassEdit> {
    let mut ledger = Vec::new();
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by(|&a, &b| notes[a].start_beat.total_cmp(&notes[b].start_beat));
    let mut alive = vec![true; notes.len()];
    // Onsets of voicings that already lost a voice to this gate.
    let mut thinned: Vec<f64> = Vec::new();
    // The class of note `i` in the current (partly gated) line.
    let class_of = |notes: &[Note], alive: &[bool], i: usize| -> TemporalMass {
        let idx: Vec<usize> = (0..notes.len()).filter(|&j| alive[j]).collect();
        let line: Vec<Note> = idx.iter().map(|&j| notes[j]).collect();
        let at = idx
            .iter()
            .position(|&j| j == i)
            .expect("gated note is alive");
        TemporalMass::of_notes(
            &line,
            &perf.contexts,
            world.tempo_bpm,
            super::form::BEATS_PER_BAR,
            world,
        )[at]
    };
    for &i in &order {
        let n = notes[i];
        if !alive[i] || !matches!(n.prov.role_note, "pad" | "comp" | "hold") {
            continue;
        }
        let Some(ctx) = perf.context_at(n.start_beat) else {
            continue;
        };
        let pc = pitch_class(n.pitch);
        if ctx.chord.contains_pc(pc) || !ctx.palette.tensions.contains(&pc) {
            continue;
        }
        let mass = class_of(notes, &alive, i);
        if mass.class < ExposureClass::Asserted {
            continue;
        }
        let edit = |action| SupportMassEdit {
            note: n,
            mass,
            action,
        };
        // 1. Shorten: the longest grid-step length that ends the assertion.
        let step = super::performance::STEP_BEATS as f32;
        let mut len = ((n.dur_beats / step).ceil() - 1.0) * step;
        let mut shortened = false;
        while len >= step - 1e-6 {
            notes[i].dur_beats = len;
            if class_of(notes, &alive, i).class < ExposureClass::Asserted {
                shortened = true;
                ledger.push(edit(SupportMassAction::Shortened(len)));
                break;
            }
            len -= step;
        }
        if shortened {
            continue;
        }
        notes[i].dur_beats = n.dur_beats;
        // 2. Omit, once per voicing, when it still states the harmony's guide tones without it.
        let bundle: Vec<usize> = (0..notes.len())
            .filter(|&j| j != i && alive[j] && (notes[j].start_beat - n.start_beat).abs() < 1e-6)
            .collect();
        let first = !thinned.iter().any(|&t| (t - n.start_beat).abs() < 1e-6);
        if first
            && bundle.iter().any(|&j| {
                ctx.palette
                    .guide_tones
                    .contains(&pitch_class(notes[j].pitch))
            })
        {
            alive[i] = false;
            thinned.push(n.start_beat);
            ledger.push(edit(SupportMassAction::Omitted));
            continue;
        }
        // 3. The nearest written chord tone the voicing does not already sound.
        if let Some(q) = (1..=12)
            .flat_map(|d| [n.pitch - d, n.pitch + d])
            .find(|&q| {
                ctx.chord.contains_pc(pitch_class(q)) && bundle.iter().all(|&j| notes[j].pitch != q)
            })
        {
            notes[i].pitch = q;
            notes[i].function = function_over(ctx, q);
            ledger.push(edit(SupportMassAction::Moved(q)));
        }
    }
    let mut k = 0;
    notes.retain(|_| {
        k += 1;
        alive[k - 1]
    });
    ledger
}

/// The keys' final pass: onset order (stable), then lift off at harmony changes. A hold is a
/// voicing chosen FOR its harmony: a held voice that is not a member of the next harmony lifts
/// off AT the change (the coupled keys' rule), while its member voices sustain the hold. Other
/// notes keep the plan's support release ([`release_support`]: a half-beat tail archived, none
/// when earned).
pub fn finish_keys(mut out: Vec<Note>, perf: &PerformancePlan) -> Vec<Note> {
    out.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    release_support(&mut out, perf);
    for n in out.iter_mut().filter(|n| n.prov.role_note == "hold") {
        release_with_overhang(std::slice::from_mut(n), &perf.chords, 0.0);
    }
    // Earned: a projected material onset that `land_once` moved onto an accent may now sit in a
    // harmony other than the one its pitch was chosen for (the bass's `reland` law, applied to the
    // keys' material lines). Judge it where it actually sounds: keep the pitch if it is stable
    // there, otherwise take the nearest stable pitch, and declare the function it earns.
    if perf.functions == super::policy::FunctionPolicy::Earned {
        for n in out
            .iter_mut()
            .filter(|n| matches!(n.prov.role_note, "answer" | "figure" | "quote"))
        {
            let Some(ctx) = perf.context_at(n.start_beat) else {
                continue;
            };
            if function_over(ctx, n.pitch).is_none() {
                n.pitch = nearest_stable(ctx, n.pitch);
            }
            n.function = function_over(ctx, n.pitch);
        }
    }
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
    keys_comp_impl(perf, world, lead, seed, kp, false, None)
}

fn keys_comp_impl(
    perf: &PerformancePlan,
    world: &MusicWorld,
    lead: &[Note],
    seed: u64,
    kp: &super::voicing::RolePath,
    temporal: bool,
    ownership: Option<&super::occupancy::AuthoredOccupancy>,
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
        // The final acoustic lead supplies collision information. Its source's authored
        // reservations separately decide whether an apparent gap is an accompaniment opening.
        steps.retain(|&(_, s)| {
            let b = AccentGrid::beat_of(bar, s);
            ownership.map_or_else(
                || !lead_onset_near(lead, b),
                |intent| intent.allows_comp_at(b, lead),
            ) && perf.on_stage(Agent::Keys, b)
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
                        let p = if temporal {
                            gate_pitch(perf, beat, dur, p)
                        } else {
                            p
                        };
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
    keys_lines_impl(perf, lead, vel, false)
}

fn keys_lines_impl(perf: &PerformancePlan, lead: &[Note], vel: f32, temporal: bool) -> Vec<Note> {
    let mut out = Vec::new();
    // Answers: the CALL's material, transformed (derived in the plan), in the keys' register —
    // whoever called. Round VII transformed whatever the lead happened to play in the window.
    for (call, r) in perf.responses_for(Agent::Keys) {
        out.extend(answer_notes_impl(
            perf,
            call,
            r,
            Agent::Keys,
            Role::Keys,
            vel * 1.05,
            temporal,
        ));
    }
    // The figures the keys state (a keys-led pickup / fragment / re-entry): their own material.
    out.extend(figure_notes_impl(
        perf,
        Agent::Keys,
        Role::Keys,
        vel * 1.05,
        temporal,
    ));
    out.extend(keys_unison_notes_impl(perf, lead, vel, temporal));
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
    keys_unison_notes_impl(perf, lead, vel, false)
}

fn keys_unison_notes_impl(
    perf: &PerformancePlan,
    lead: &[Note],
    vel: f32,
    temporal: bool,
) -> Vec<Note> {
    let mut out = Vec::new();
    // Planned ensemble unison figures: the keys sound the shared line.
    for (id, line) in if temporal {
        unison_lines_temporal(perf, lead)
    } else {
        unison_lines(perf, lead)
    } {
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
    unison_lines_inner(perf, lead, false)
}

/// Realize the same figure with its literal chord-member landing contract enforced.
pub fn unison_lines_temporal(
    perf: &PerformancePlan,
    lead: &[Note],
) -> Vec<(ActionId, Vec<LineNote>)> {
    unison_lines_inner(perf, lead, true)
}

fn unison_lines_inner(
    perf: &PerformancePlan,
    lead: &[Note],
    temporal: bool,
) -> Vec<(ActionId, Vec<LineNote>)> {
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
                let target = line.last().map(|l| l.2).unwrap_or(67);
                let p = if temporal {
                    nearest_chord_tone(ctx, target)
                } else {
                    nearest_stable(ctx, target)
                };
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
    answer_notes_impl(perf, call, r, agent, role, velocity, false)
}

fn answer_notes_impl(
    perf: &PerformancePlan,
    call: &Call,
    r: &Response,
    agent: Agent,
    role: Role,
    velocity: f32,
    temporal: bool,
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
    (if temporal {
        super::material::line_of_temporal(m, agent, r.start_beat, r.start_beat + r.dur_beats, perf)
    } else {
        super::material::line_of(m, agent, r.start_beat, r.start_beat + r.dur_beats, perf)
    })
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
    figure_notes_impl(perf, agent, role, velocity, false)
}

fn figure_notes_impl(
    perf: &PerformancePlan,
    agent: Agent,
    role: Role,
    velocity: f32,
    temporal: bool,
) -> Vec<Note> {
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
        for (at, d, p, accent) in if temporal {
            super::material::line_of_temporal(m, agent, m.start_beat, until, perf)
        } else {
            super::material::line_of(m, agent, m.start_beat, until, perf)
        } {
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

/// The pad's full register: [`super::voicing::VoiceRange::pad`]'s floor to
/// [`super::voicing::VoiceRange::pad_upper`]'s ceiling.
const PAD_SPAN: (Midi, Midi) = (52, 91);

/// Why the Round XIV pad changed a voicing of its Round XII path.
#[derive(Debug, Clone, PartialEq)]
pub enum PadVoicingReason {
    /// Two held members a minor 2nd or minor 9th apart: `from` moved by whole octaves to `to`
    /// (same pitch class, so the chord the pad sounds is unchanged).
    Spacing { from: Midi, to: Midi },
    /// The band heard over this harmony was `rival`, with no chart root in it, for at least
    /// [`super::identity::IDENTITY_HOLD_SECS`]. The pad now sounds the chart's `root`, in place of
    /// `replaced` (an unwritten colour or the 5th), or added when nothing could give way.
    Rooting {
        rival: super::theory::Chord,
        replaced: Option<Midi>,
        root: Midi,
    },
    /// Under earned functions: the band heard over this harmony was `rival`, completed by the
    /// pad's own consonant release tails from the harmony before; the pad lets them go at this
    /// harmony's start.
    Release { rival: super::theory::Chord },
}

/// One Round XIV pad voicing change: harmony `context` (starting at `start_beat`), the voicing
/// before and after.
#[derive(Debug, Clone, PartialEq)]
pub struct PadVoicingEdit {
    pub context: usize,
    pub start_beat: f64,
    pub before: Vec<Midi>,
    pub after: Vec<Midi>,
    pub reason: PadVoicingReason,
}

/// How structural pitch class `pc` is over `ctx`: its place in the written chord (root 0,
/// 3rd 1, 5th 2, 7th/6th 3, written extensions after), then 8 for a scale tone, 9 chromatic.
fn degree_rank(ctx: &HarmonicContext, pc: i32) -> u8 {
    match ctx.chord.pitch_classes().iter().position(|&c| c == pc) {
        Some(i) => i as u8,
        None if ctx.region.contains_pc(pc) => 8,
        None => 9,
    }
}

/// The first pair of `v`'s members a minor 2nd or minor 9th apart (low, high).
fn cluster(v: &[Midi]) -> Option<(Midi, Midi)> {
    v.iter().enumerate().find_map(|(i, &a)| {
        v[i + 1..]
            .iter()
            .find(|&&b| super::sonority::Clash::of(a, b).is_some())
            .map(|&b| (a.min(b), a.max(b)))
    })
}

/// Whether `q` could join `others` in the pad: in its register, not already there, and no minor
/// 2nd or minor 9th against any of them.
fn fits(q: Midi, others: &[Midi]) -> bool {
    fits_in(q, others, PAD_SPAN)
}

/// [`fits`] in the register `span` of the player voicing it.
fn fits_in(q: Midi, others: &[Midi], span: (Midi, Midi)) -> bool {
    (span.0..=span.1).contains(&q)
        && !others.contains(&q)
        && others
            .iter()
            .all(|&x| super::sonority::Clash::of(x, q).is_none())
}

/// Whether `after` keeps the upper layer `before` had: an upper-structure voicing's top voice at
/// or above [`super::voicing::VoiceRange::pad_upper`]'s `min_top` (the audible witness of a pad
/// thickening).
fn keeps_upper_layer(before: &[Midi], after: &[Midi]) -> bool {
    let top = super::voicing::VoiceRange::pad_upper(0.0)
        .min_top
        .unwrap_or(Midi::MAX);
    before.iter().all(|&p| p < top) || after.iter().any(|&p| p >= top)
}

/// Space one voicing: while two members sit a minor 2nd or 9th apart, move the less structural
/// of the two (then the other) by the smallest whole-octave step that leaves it clear of every
/// other member. Returns the edits made.
fn space_voicing(ctx: &HarmonicContext, ci: usize, v: &mut [Midi]) -> Vec<PadVoicingEdit> {
    let mut edits = Vec::new();
    while let Some((lo, hi)) = cluster(v) {
        let (rl, rh) = (
            degree_rank(ctx, pitch_class(lo)),
            degree_rank(ctx, pitch_class(hi)),
        );
        let order = if rh >= rl { [hi, lo] } else { [lo, hi] };
        let moved = order.iter().find_map(|&p| {
            let others: Vec<Midi> = v.iter().copied().filter(|&x| x != p).collect();
            [12, -12, 24, -24]
                .iter()
                .map(|d| p + d)
                .find(|&q| {
                    let mut after = others.clone();
                    after.push(q);
                    fits(q, &others) && keeps_upper_layer(v, &after)
                })
                .map(|q| (p, q))
        });
        let Some((from, to)) = moved else {
            break;
        };
        let before = v.to_vec();
        for x in v.iter_mut() {
            if *x == from {
                *x = to;
            }
        }
        v.sort_unstable();
        edits.push(PadVoicingEdit {
            context: ci,
            start_beat: ctx.start_beat,
            before,
            after: v.to_vec(),
            reason: PadVoicingReason::Spacing { from, to },
        });
    }
    edits
}

/// The ways the pad could sound the chart's root in voicing `v`, in order of preference: in
/// place of an unwritten colour (least structural first), then of the 5th, then added. Each
/// places the root in the octave nearest the note it replaces (the voicing's middle when added)
/// where it clashes with no remaining member. Written chord tones other than the 5th, and
/// written extensions, are never given up, and an upper-structure voicing keeps its upper layer.
fn rooting_candidates(ctx: &HarmonicContext, v: &[Midi]) -> Vec<(Option<Midi>, Midi, Vec<Midi>)> {
    rooting_candidates_in(ctx, v, PAD_SPAN, true)
}

/// [`rooting_candidates`] for the support player whose register is `span`; `upper`: whether the
/// player's voicings carry an upper-structure layer that must survive (the pad's).
fn rooting_candidates_in(
    ctx: &HarmonicContext,
    v: &[Midi],
    span: (Midi, Midi),
    upper: bool,
) -> Vec<(Option<Midi>, Midi, Vec<Midi>)> {
    let root = ctx.chord.root_pc.rem_euclid(12);
    let fifth = (root + 7) % 12;
    let mut give: Vec<Midi> = v
        .iter()
        .copied()
        .filter(|&p| degree_rank(ctx, pitch_class(p)) >= 8)
        .collect();
    give.sort_by_key(|&p| (std::cmp::Reverse(degree_rank(ctx, pitch_class(p))), -p));
    give.extend(v.iter().copied().filter(|&p| {
        pitch_class(p) == fifth && ctx.chord.contains_pc(fifth) && degree_rank(ctx, fifth) < 8
    }));
    let middle = v.iter().sum::<Midi>() / v.len().max(1) as Midi;
    give.into_iter()
        .map(Some)
        .chain(std::iter::once(None))
        .filter_map(|out| {
            let rest: Vec<Midi> = v.iter().copied().filter(|&p| Some(p) != out).collect();
            let near = out.unwrap_or(middle);
            (span.0..=span.1)
                .filter(|&q| pitch_class(q) == root && fits_in(q, &rest, span))
                .map(|q| {
                    let mut after = rest.clone();
                    after.push(q);
                    after.sort_unstable();
                    (out, q, after)
                })
                .filter(|(_, _, after)| !upper || keeps_upper_layer(v, after))
                .min_by_key(|&(_, q, _)| ((q - near).abs(), q))
        })
        .collect()
}

/// Round XIV: the pad as a player who hears the band (`band`: the lead, keys and bass already
/// realized, which never read the pad). It plays Round XII's voice path, voicing by voicing,
/// with two changes made at the source, before any note is emitted:
///
/// 1. Spacing. Two held members a minor 2nd or 9th apart (SWISS's Cmaj7 E4 B4 C5 G5, held
///    2.3 s) are spread by octaves. The pitch classes are unchanged, so the chord it sounds is
///    unchanged.
/// 2. Rooting. Where the heard band flips the chart's identity over a harmony the pad voices
///    ([`super::identity`]), the pad sounds the chart's root. An edit is kept only when it removes
///    flipped time over that harmony and leaves the band's identity no worse anywhere
///    ([`super::identity::keeps_identity`]).
///
/// Nothing is ever removed on the grounds that another player covers it.
pub fn realize_pad_heard(
    perf: &PerformancePlan,
    _plan: &CompositionPlan,
    world: &MusicWorld,
    band: &[Note],
) -> (Vec<Note>, Vec<PadVoicingEdit>) {
    let (_, pad, edits) = heard_pad_path(perf, world, band);
    (pad, edits)
}

/// Heard harmony under canonical direct-voice physics. Empty links mean the full envelope;
/// another note of the same role does not implicitly mask a released voice.
pub fn realize_pad_heard_with_continuity(
    perf: &PerformancePlan,
    world: &MusicWorld,
    band: &[Note],
    links: &[super::voice::VoiceContinuation],
) -> (Vec<Note>, Vec<PadVoicingEdit>) {
    let (_, pad, edits) = heard_pad_path_impl(perf, world, band, Some(links));
    (pad, edits)
}

/// Round XVI's pad: retain the Round XIV harmony solution, then select a same-pitch-class
/// voicing within the source-owned path when the audible register/path witness asks for one.
/// Trial realizations supply evidence; the selected path is emitted once as the final pad.
pub fn realize_pad_phrased(
    perf: &PerformancePlan,
    _plan: &CompositionPlan,
    world: &MusicWorld,
    band: &[Note],
) -> (
    Vec<Note>,
    Vec<PadVoicingEdit>,
    Vec<super::support_voicing::VoicingPathDecision>,
) {
    let (path, pad, edits) = heard_pad_path(perf, world, band);
    let (path, decisions) = super::support_voicing::select(perf, world, band, path, &pad);
    (realize_pad_on(perf, world, &path, None), edits, decisions)
}

/// Round XVII uses the same source support selection with the explicit heard-lifetime law.
/// This does not promote the localization-pending support-top-voice hypothesis.
pub fn realize_pad_pocketed(
    perf: &PerformancePlan,
    world: &MusicWorld,
    band: &[Note],
    links: &[super::voice::VoiceContinuation],
) -> (
    Vec<Note>,
    Vec<PadVoicingEdit>,
    Vec<super::support_voicing::VoicingPathDecision>,
) {
    let (path, pad, edits) = heard_pad_path_impl(perf, world, band, Some(links));
    let (path, decisions) =
        super::support_voicing::select_with_continuity(perf, world, band, path, &pad, links);
    (realize_pad_on(perf, world, &path, None), edits, decisions)
}

/// The exact Round XIV algorithm, with its selected source path retained for later source
/// articulation. The public Round XIV arm returns the same notes and edits as before.
fn heard_pad_path(
    perf: &PerformancePlan,
    world: &MusicWorld,
    band: &[Note],
) -> (super::voicing::RolePath, Vec<Note>, Vec<PadVoicingEdit>) {
    heard_pad_path_impl(perf, world, band, None)
}

fn heard_pad_path_impl(
    perf: &PerformancePlan,
    world: &MusicWorld,
    band: &[Note],
    continuity: Option<&[super::voice::VoiceContinuation]>,
) -> (super::voicing::RolePath, Vec<Note>, Vec<PadVoicingEdit>) {
    use super::identity::{keeps_held_identity, keeps_identity, IdentityDiagnostics};
    let mut pp = pad_path(perf, world.voicing_spread);
    let mut edits = Vec::new();
    for t in 0..pp.path.voicings.len() {
        let ci = pp.context_ix[t];
        let mut v = pp.path.voicings[t].voices.clone();
        edits.extend(space_voicing(&perf.contexts[ci], ci, &mut v));
        pp.path.voicings[t].voices = v;
    }
    let heard = |pp: &super::voicing::RolePath| -> (Vec<Note>, IdentityDiagnostics) {
        let pad = realize_pad_on(perf, world, pp, None);
        let mut all = band.to_vec();
        all.extend(pad.iter().copied());
        let id = if let Some(links) = continuity {
            let heard = super::voice::HeardWindows::explicit(&all, world, world.tempo_bpm, links);
            IdentityDiagnostics::measure_heard_windows(&heard, &perf.contexts)
        } else {
            IdentityDiagnostics::measure(&all, &perf.contexts, world, world.tempo_bpm)
        };
        (pad, id)
    };
    let (mut pad, mut id) = heard(&pp);
    let mut flipped: Vec<(usize, super::theory::Chord)> = id
        .flips()
        .flat_map(|r| r.slices.iter().map(move |&k| (k, r.rival)))
        .map(|(k, rival)| (id.slices[k].context, rival))
        .collect();
    flipped.dedup_by_key(|x| x.0);
    let end = perf
        .chords
        .last()
        .map_or(0.0, |c| c.start_beat + f64::from(c.dur_beats))
        + 64.0;
    // Earned: the rooting law reaches a common-tone carry too (archived, the carry renders only
    // its held voices and new guide tones, so a root it was asked for never sounds there).
    let earned = perf.functions == super::policy::FunctionPolicy::Earned;
    // Earned: the pad answers until it has nothing left to answer (a root vetoed only because a
    // neighbouring harmony was still flipped is judged again once that neighbour is answered);
    // archived, one sweep.
    let passes = if earned { PAD_ANSWER_PASSES } else { 1 };
    for pass in 0..passes {
        if pass > 0 {
            flipped = id
                .flips()
                .flat_map(|r| r.slices.iter().map(move |&k| (k, r.rival)))
                .map(|(k, rival)| (id.slices[k].context, rival))
                .collect();
            flipped.dedup_by_key(|x| x.0);
        }
        let answered = edits.len();
        for (ci, rival) in flipped.iter().copied() {
            let before = flipped_over(&id, ci);
            if before <= 0.0 {
                continue;
            }
            let voiced = pp.context_ix.binary_search(&ci).ok();
            if let Some(t) = voiced {
                let voices = pp.path.voicings[t].voices.clone();
                let carry = earned
                    && perf
                        .bar_at(perf.contexts[ci].start_beat)
                        .is_some_and(|b| b.pad == PadMode::CommonToneCarry);
                // A carry first sounds the root its own voicing already holds; then the usual edits.
                let own_root = voices
                    .iter()
                    .copied()
                    .find(|&p| pitch_class(p) == perf.contexts[ci].chord.root_pc.rem_euclid(12))
                    .filter(|_| carry)
                    .map(|q| (None, q, voices.clone()));
                // Earned: the edit is judged by the hold law the receipt judges (a sub-hold
                // overlap where the chord was only implied is lawful), and a candidate that
                // REMOVES the flip wins over one that only shortens it ([`choose_rooting`]).
                // Archived: the first candidate that shortens the flip under the strict
                // pointwise rule.
                let chosen = choose_rooting(
                    own_root
                        .into_iter()
                        .chain(rooting_candidates(&perf.contexts[ci], &voices)),
                    ci,
                    before,
                    earned,
                    |after| {
                        let mut trial = pp.clone();
                        trial.path.voicings[t].voices = after.to_vec();
                        if earned {
                            trial.rooted.push(ci);
                        }
                        let (p2, id2) = heard(&trial);
                        // Earned: the root answers for THIS harmony. Where its tail would complete
                        // another chord in the next one, the next harmony lets the pad's tails go.
                        let next = ci + 1;
                        if earned
                            && next < perf.contexts.len()
                            && !trial.released.contains(&next)
                            && flipped_over(&id2, next) > flipped_over(&id, next) + 1e-9
                        {
                            trial.released.push(next);
                            let (p3, id3) = heard(&trial);
                            return Some(((trial, p3), id3));
                        }
                        Some(((trial, p2), id2))
                    },
                    |id2| {
                        if earned {
                            keeps_held_identity(&id, id2, 0.0, end)
                        } else {
                            keeps_identity(&id, id2, 0.0, end)
                        }
                    },
                );
                if let Some((replaced, root, after, (trial, p2), id2)) = chosen {
                    edits.push(PadVoicingEdit {
                        context: ci,
                        start_beat: perf.contexts[ci].start_beat,
                        before: voices,
                        after,
                        reason: PadVoicingReason::Rooting {
                            rival,
                            replaced,
                            root,
                        },
                    });
                    (pp, pad, id) = (trial, p2, id2);
                    continue;
                }
            }
            // Earned: where the rival is completed by the pad's own consonant tails from the
            // harmony before (a harmony it may not voice at all), the pad lets them go at this
            // harmony's start — judged exactly like a rooting.
            if earned && !pp.released.contains(&ci) {
                let mut trial = pp.clone();
                trial.released.push(ci);
                let (p2, id2) = heard(&trial);
                if flipped_over(&id2, ci) < before - 1e-9
                    && keeps_held_identity(&id, &id2, 0.0, end)
                {
                    let voices = voiced
                        .map(|t| pp.path.voicings[t].voices.clone())
                        .unwrap_or_default();
                    edits.push(PadVoicingEdit {
                        context: ci,
                        start_beat: perf.contexts[ci].start_beat,
                        before: voices.clone(),
                        after: voices,
                        reason: PadVoicingReason::Release { rival },
                    });
                    (pp, pad, id) = (trial, p2, id2);
                }
            }
        }
        if edits.len() == answered {
            break;
        }
    }
    (pp, pad, edits)
}

/// Under earned functions: how many sweeps the pad's identity law may make over the harmonies the
/// band still flips (it stops as soon as a sweep answers nothing). A bound, not a tuning.
const PAD_ANSWER_PASSES: usize = 4;

/// One rooting decision over harmony `ci`, shared by every support player that answers for the
/// chart's identity (the pad, the keys). `candidates` come in preference order; `trial` realizes
/// and hears the band with a candidate voicing (`None`: the candidate is not a rooting as
/// realized); `keeps` is the player's identity acceptance. A candidate is admissible when it
/// shortens the flip over `ci` (from `before` beats) and `keeps` accepts it. Under earned
/// functions the first admissible candidate that REMOVES the flip wins over one that only shortens
/// it; archived, the first admissible candidate wins.
#[allow(clippy::type_complexity)]
fn choose_rooting<T>(
    candidates: impl IntoIterator<Item = (Option<Midi>, Midi, Vec<Midi>)>,
    ci: usize,
    before: f64,
    earned: bool,
    mut trial: impl FnMut(&[Midi]) -> Option<(T, super::identity::IdentityDiagnostics)>,
    keeps: impl Fn(&super::identity::IdentityDiagnostics) -> bool,
) -> Option<(
    Option<Midi>,
    Midi,
    Vec<Midi>,
    T,
    super::identity::IdentityDiagnostics,
)> {
    let mut chosen = None;
    for (replaced, root, after) in candidates {
        let Some((t, id2)) = trial(&after) else {
            continue;
        };
        let left = flipped_over(&id2, ci);
        if left < before - 1e-9 && keeps(&id2) {
            let removes = left <= 1e-9;
            if chosen.is_none() || removes {
                chosen = Some((replaced, root, after, t, id2));
            }
            if !earned || removes {
                break;
            }
        }
    }
    chosen
}

/// Beats of held (flipped) identity the band leaves over harmony `ci`.
fn flipped_over(id: &super::identity::IdentityDiagnostics, ci: usize) -> f64 {
    id.flips()
        .flat_map(|r| r.slices.iter())
        .map(|&k| &id.slices[k])
        .filter(|s| s.context == ci)
        .map(|s| s.end_beat - s.start_beat)
        .sum()
}

/// Under earned functions ([`super::policy::FunctionPolicy::Earned`]) harmonic identity is the
/// responsibility of the support that sounds the harmony. The pad answers first, for every harmony
/// it voices (its rooting law, [`realize_pad_pocketed`]). Whatever the band still holds as another
/// chord once the pad has answered — a harmony the pad leaves silent (a Thin bar) or cannot root
/// in time (a swell whose root enters late) — the keys answer for under the same law: `hear`
/// measures the band as it will sound (the lead, these keys, the bass, and the pad as it answers
/// them, inside the piece); where it flips the chart's chord over a harmony the keys voice, the
/// keys sound its root, in place of a colour, then of the 5th, else added ([`rooting_candidates`],
/// in the keys' register). The first candidate that removes the flip is kept, else the first that
/// shortens it — only when it keeps the identity as the hold law judges it
/// ([`super::identity::keeps_held_identity`]). A rootless voicing nobody hears as another chord is
/// never touched; nothing is decided after the keys are emitted (`render` realizes a trial path at
/// the source). Returns the keys' path and the edits made.
pub(crate) fn keys_answer_for_identity(
    perf: &PerformancePlan,
    world: &MusicWorld,
    kp: &super::voicing::RolePath,
    render: &dyn Fn(&super::voicing::RolePath) -> Vec<Note>,
    hear: &dyn Fn(&[Note]) -> super::identity::IdentityDiagnostics,
) -> (super::voicing::RolePath, Vec<PadVoicingEdit>) {
    use super::identity::keeps_held_identity;
    let range = super::voicing::VoiceRange::keys(keys_shell_n(perf), world.voicing_spread);
    let end = perf
        .chords
        .last()
        .map_or(0.0, |c| c.start_beat + f64::from(c.dur_beats))
        + 64.0;
    let mut kp = kp.clone();
    let mut id = hear(&render(&kp));
    let mut flipped: Vec<(usize, super::theory::Chord)> = id
        .flips()
        .flat_map(|r| r.slices.iter().map(move |&k| (k, r.rival)))
        .map(|(k, rival)| (id.slices[k].context, rival))
        .collect();
    flipped.dedup_by_key(|x| x.0);
    let mut edits = Vec::new();
    for (ci, rival) in flipped {
        let Ok(t) = kp.context_ix.binary_search(&ci) else {
            continue; // the keys do not voice this harmony
        };
        let before = flipped_over(&id, ci);
        if before <= 0.0 {
            continue;
        }
        let voices = kp.path.voicings[t].voices.clone();
        let ctx = &perf.contexts[ci];
        let (a, b) = (ctx.start_beat, ctx.start_beat + f64::from(ctx.dur_beats));
        let root_pc = ctx.chord.root_pc.rem_euclid(12);
        let chosen = choose_rooting(
            rooting_candidates_in(ctx, &voices, (range.low, range.high), false),
            ci,
            before,
            true,
            |after| {
                let mut trial = kp.clone();
                trial.path.voicings[t].voices = after.to_vec();
                let keys = render(&trial);
                // An edit is a rooting only if the keys, as rendered, sound the root here (a stab
                // voices a subset of its voicing).
                let sounds_root = keys.iter().any(|n| {
                    pitch_class(n.pitch) == root_pc
                        && n.start_beat < b - 1e-6
                        && n.start_beat + f64::from(n.dur_beats) > a + 1e-6
                });
                sounds_root.then(|| {
                    let id2 = hear(&keys);
                    (trial, id2)
                })
            },
            |id2| keeps_held_identity(&id, id2, 0.0, end),
        );
        if let Some((replaced, root, after, trial, id2)) = chosen {
            edits.push(PadVoicingEdit {
                context: ci,
                start_beat: perf.contexts[ci].start_beat,
                before: voices,
                after,
                reason: PadVoicingReason::Rooting {
                    rival,
                    replaced,
                    root,
                },
            });
            (kp, id) = (trial, id2);
        }
    }
    (kp, edits)
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
                // guide tones - and the chart root where the rooting law asked for it.
                let prev = pp.previous(ci).map(|p| p.voices).unwrap_or_default();
                let rooted = pp.rooted.contains(&ci);
                for &p in &v.voices {
                    if prev.contains(&p)
                        || ctx.palette.guide_tones.contains(&pitch_class(p))
                        || (rooted && pitch_class(p) == ctx.chord.root_pc.rem_euclid(12))
                    {
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
    // BAND support lifetime (`FunctionPolicy::Earned`): a pad note's acoustic tail is justified in
    // the next harmony only where that harmony admits the pitch (a chord tone or licensed tension)
    // — the bass's and keys' own release law ([`release_support`]'s membership test), extended to
    // the pad. A tail into a harmony that EXCLUDES the pitch is a smear (a false function and, with
    // the rest of the band, a contradictory heard identity); it fades under the next harmony's
    // attack. A consonant tail is kept: it can be load-bearing (the pad's own root ringing under a
    // bar the pad does not re-voice). (Archived arms keep their byte-exact half-beat behaviour;
    // this is a source law, not an observer change.)
    //
    // The pad's identity law (`heard_pad_path_impl`) reaches the same source: a harmony it
    // RELEASED — where, with the band, the pad's consonant tails complete another chord — lets
    // every incoming tail go at its start.
    if perf.functions == super::policy::FunctionPolicy::Earned {
        let spb = 60.0 / f64::from(world.tempo_bpm.max(1.0));
        let tail =
            super::voice::release_tail_secs(&world.pad, super::voice::AUDIBLE_FLOOR_DB) / spb;
        // The harmony a note sounds in and the one its tail rings into, looked up exactly as the
        // archived law does (`context_at`: past the piece's end it answers the final harmony
        // itself, whose consonant tail therefore rings out).
        let index_of = |c: &HarmonicContext| perf.contexts.iter().position(|x| std::ptr::eq(x, c));
        for n in out.iter_mut() {
            let Some(ctx) = perf.context_at(n.start_beat) else {
                continue;
            };
            let ci = index_of(ctx);
            let boundary = ctx.start_beat + f64::from(ctx.dur_beats);
            if n.start_beat + f64::from(n.dur_beats) + tail <= boundary + 1e-9 {
                continue;
            }
            let next = perf.context_at(boundary + 1e-6);
            let admitted = next.is_some_and(|next| function_over(next, n.pitch).is_some());
            // A release concerns another harmony the tail rings into, never the piece's end.
            let released = next
                .and_then(index_of)
                .is_some_and(|j| Some(j) != ci && pp.released.contains(&j));
            if admitted && !released {
                continue;
            }
            let short = ((boundary - n.start_beat) - tail - 1e-4) as f32;
            // A structural context shorter than the tail has no room even for the release:
            // leave the note valid rather than emitting a non-positive duration.
            if short >= 0.02 {
                n.dur_beats = short;
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
            pitch_basis: crate::audio::human_music::material::PitchBasis::ScaleSteps,
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
            (t0 + 5.0, Some(60 + at(t0 + 5.0).chord.root_pc), None),
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
    fn chord_landing_must_choose_a_chord_member() {
        use crate::audio::human_music::{context, harmony::ChordSpan, theory::*};
        let chord = Chord::new(0, Quality::Maj7);
        let contexts = context::analyze(
            &[ChordSpan::test(0.0, 4.0, chord)],
            &Scale::new(0, Mode::Ionian),
        );
        let ctx = &contexts[0];
        let p = 60 + ctx.palette.tensions[0];
        assert!(chord.contains_pc(pitch_class(super::nearest_chord_tone(ctx, p))));
    }
}

#[cfg(test)]
mod keys_identity_responsibility {
    use crate::audio::human_music::{
        composer::Composer,
        contract::CompositionGrammar,
        functor::perform_with_profile,
        identity::IdentityDiagnostics,
        language::MusicalLanguage,
        performance::PerformanceOptions,
        policy::PerformanceProfile,
        score::{Note, Role},
        semantic::deflected_lift_trace,
        voicing::{keys_path, RolePath},
        SongMap,
    };

    /// The keys' decision on fresh seed 96_860_001 (Dm7 at 48, the pad silent, the keys holding
    /// C-F-G over a bass that leaves the root after half a beat), A/B on the bass alone.
    fn decide(bass_holds_root: bool) -> Vec<super::PadVoicingEdit> {
        let song = SongMap::compose(
            &deflected_lift_trace(64.0),
            96_860_001,
            Some(CompositionGrammar::DeflectedLift),
            Composer::StructuralR9,
        );
        // The closeout falsifier was derived under the archival VAPOR95 v1 world.
        let world = super::super::world::vapor95_v1();
        let opts = PerformanceOptions {
            language: MusicalLanguage::fusion_conversation(),
            ..PerformanceOptions::default()
        };
        let c = perform_with_profile(&song, &world, opts, PerformanceProfile::BAND)
            .expect("lawful candidate");
        let lead: Vec<Note> = c.score.role_notes(Role::Lead).copied().collect();
        let pad: Vec<Note> = c.score.role_notes(Role::Pad).copied().collect();
        let mut bass: Vec<Note> = c.score.role_notes(Role::Bass).copied().collect();
        if bass_holds_root {
            // The bass supplies the discriminating evidence: D2 held through the harmony.
            let mut d = bass[0];
            (d.start_beat, d.dur_beats, d.pitch) = (48.0, 3.9, 38);
            bass.retain(|n| n.start_beat < 48.0 - 1e-6 || n.start_beat >= 52.0 - 1e-6);
            bass.push(d);
        }
        let kp = keys_path(
            &c.perf,
            world.voicing_spread,
            &lead,
            super::keys_shell_n(&c.perf),
        );
        let render = |kp: &RolePath| super::realize_keys_temporal_on(&c.perf, &world, &lead, 0, kp);
        let hear = |keys: &[Note]| {
            let band: Vec<Note> = lead
                .iter()
                .chain(keys)
                .chain(&bass)
                .chain(&pad)
                .copied()
                .collect();
            IdentityDiagnostics::measure(&band, &c.perf.contexts, &world, world.tempo_bpm)
        };
        super::keys_answer_for_identity(&c.perf, &world, &kp, &render, &hear).1
    }

    #[test]
    fn the_keys_answer_where_the_band_would_hold_another_chord() {
        let edits = decide(false);
        assert!(
            edits.iter().any(|e| (e.start_beat - 48.0).abs() < 1e-6),
            "{edits:?}"
        );
    }

    #[test]
    fn a_bass_that_holds_the_root_leaves_the_keys_rootless() {
        let edits = decide(true);
        assert!(
            edits.iter().all(|e| (e.start_beat - 48.0).abs() > 1e-6),
            "the bass states the root, so nothing asks the keys to: {edits:?}"
        );
    }
}
