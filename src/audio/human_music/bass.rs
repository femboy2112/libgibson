//! **Bass** — a second melodic and rhythmic agent, one projection of the [`PerformancePlan`].
//!
//! Round VI's bass put a note on every kick (the groove was generated first and the bass locked to
//! its onsets), stepping chromatically into chord changes. Here the bass reads the shared accent
//! grid and its per-bar [`BassMode`]: a foundation that takes its own subset of the gesture's
//! rhythm cell (not the kick's), a pedal under the Deflect, a walk up through the Lift into the
//! pointer, a counterline in the lead's gaps under the Open, and motif quotes when the bass calls
//! or answers. It plays the planned ensemble hits with everyone else and leaves the planned holes
//! empty. Drums are realized *after* the bass and interlock with it — the kick follows the bass,
//! not the other way round.

use super::action::{ActionKind, Agent};
use super::context::HarmonicContext;
use super::ids::ActionStamp;
use super::performance::{AccentGrid, BassMode, PerformancePlan, STEPS};
use super::plan::CompositionPlan;
use super::score::{Note, PitchFunction, Provenance, Role};
use super::theory::{pitch_class, Midi};
use super::world::MusicWorld;

const CENTER: Midi = 40; // ~E2

fn near(pc: i32, center: Midi) -> Midi {
    let base = (center / 12) * 12 + pc.rem_euclid(12);
    [base - 12, base, base + 12]
        .into_iter()
        .min_by_key(|p| (p - center).abs())
        .unwrap()
}

fn prov(role_note: &'static str, xform: Option<&'static str>) -> Provenance {
    Provenance {
        role_note,
        motif_xform: xform,
        ..Provenance::new(super::form::SectionKind::A)
    }
}

fn note(at: f64, dur: f64, pitch: Midi, vel: f32, f: PitchFunction, tag: &'static str) -> Note {
    let mut n = Note::new(
        at,
        (dur as f32).max(0.1),
        pitch,
        vel.clamp(0.1, 1.0),
        Role::Bass,
        prov(tag, None),
    );
    n.function = Some(f);
    n
}

/// The chord's real fifth (the chord tone nearest a perfect fifth above the root).
fn fifth_of(ctx: &HarmonicContext) -> i32 {
    let r = ctx.chord.root_pc;
    ctx.chord
        .pitch_classes()
        .into_iter()
        .min_by_key(|&pc| ((pc - r).rem_euclid(12) - 7).abs())
        .unwrap_or((r + 7).rem_euclid(12))
}

/// Realize the bass. `lead` is already placed and the bass listens to it (its density, its gaps);
/// what another player SAID reaches the bass as plan material, not as notes (see
/// [`super::material`]), so `_keys` is only a hook for collision listening.
pub fn realize_bass(
    perf: &PerformancePlan,
    _plan: &CompositionPlan,
    world: &MusicWorld,
    lead: &[Note],
    _keys: &[Note],
) -> Vec<Note> {
    let mut out: Vec<Note> = Vec::new();
    let base_vel = (0.62 * world.base_dynamic.max(0.6)).clamp(0.2, 1.0);
    let lead_active = |a: f64, b: f64| {
        lead.iter()
            .any(|n| n.start_beat < b - 1e-6 && n.start_beat + n.dur_beats as f64 > a + 1e-6)
    };
    let quoted: Vec<(f64, f64)> = perf
        .figures_for(Agent::Bass)
        .map(|m| (m.start_beat, m.start_beat + m.length()))
        .chain(
            perf.responses_for(Agent::Bass)
                .map(|(_, r)| (r.start_beat, r.start_beat + r.dur_beats)),
        )
        .collect();
    let unisons = super::comp::unison_lines(perf, lead);
    let in_quote = |b: f64| {
        quoted.iter().any(|&(s, e)| b >= s - 1e-6 && b < e - 1e-6)
            || unisons.iter().any(|(_, l)| {
                l.first().is_some_and(|f| b >= f.0 - 1e-6)
                    && l.last().is_some_and(|x| b < x.0 + x.1 as f64 + 0.25)
            })
    };

    for eb in &perf.ensemble {
        let bar = eb.bar;
        let bs = AccentGrid::beat_of(bar, 0);
        let be = bs + 4.0;
        // The stage decides whether the bass plays this bar, and how loud.
        if !perf.on_stage(Agent::Bass, bs) {
            continue;
        }
        let vel = base_vel * (0.85 + 0.25 * eb.kinetic) * perf.level(Agent::Bass, bs);
        // Onset steps this bar, by mode.
        let mut onsets: Vec<usize> = vec![0];
        match eb.bass {
            BassMode::Pedal => {}
            // A walk costs four onsets; when the allowance cannot pay for them it walks in halves.
            BassMode::Walk if perf.free_allowance(Agent::Bass, bar) < 3.2 => onsets = vec![0, 8],
            BassMode::Walk => onsets = vec![0, 4, 8, 12],
            BassMode::Foundation | BassMode::Counter | BassMode::Quote => {
                // The bass's own subset of the grid: strong eighth off-beats of the gesture cell
                // (the ones the kick does NOT take: the kick keeps the downbeat and beat 3).
                let mut extra: Vec<(f32, usize)> = (1..STEPS)
                    .filter(|&s| s != 8)
                    .map(|s| (perf.accent.at(bar, s), s))
                    .filter(|(w, _)| w.hole < 0.5 && (w.syncopation >= 0.45 || w.pickup >= 0.8))
                    .map(|(w, s)| (w.syncopation + 0.5 * w.pickup + 0.3 * w.push, s))
                    .collect();
                extra.sort_by(|a, b| b.0.total_cmp(&a.0));
                // The complexity budget: a busy lead bar leaves the bass one extra onset fewer,
                // unless the bass is the one in front.
                let lead_notes = lead
                    .iter()
                    .filter(|n| n.start_beat >= bs - 1e-6 && n.start_beat < be - 1e-6)
                    .count();
                let mut n = if eb.kinetic > 0.6 { 3 } else { 2 };
                if lead_notes >= 5 && eb.foreground != Agent::Bass {
                    n -= 1;
                }
                // The shared budget: the downbeat is the floor; extra onsets are what the bass's
                // allowance still pays for.
                let free = perf.free_allowance(Agent::Bass, bar);
                let affordable = ((free - 0.8) / 0.8).floor().max(0.0) as usize;
                n = n.min(affordable);
                onsets.extend(extra.iter().take(n).map(|x| x.1));
                if eb.bass == BassMode::Foundation && eb.kinetic < 0.55 {
                    onsets.push(8);
                }
            }
        }
        // Planned ensemble hits: the bass is in on them.
        for a in perf.actions.actions.iter().filter(|a| {
            matches!(a.kind, ActionKind::Hit | ActionKind::Push)
                && a.start_beat >= bs
                && a.start_beat < be
        }) {
            onsets.push(AccentGrid::step_of(a.start_beat).1);
        }
        onsets.sort_unstable();
        onsets.dedup();
        onsets.retain(|&s| {
            let b = AccentGrid::beat_of(bar, s);
            // The stage per onset: an admitted window (the bass entering to state a missed root)
            // does not open the rest of the bar.
            !perf.accent.is_hole(b) && !in_quote(b) && perf.on_stage(Agent::Bass, b)
        });

        for (k, &s) in onsets.iter().enumerate() {
            let at = AccentGrid::beat_of(bar, s);
            let Some(ctx) = perf.context_at(at) else {
                continue;
            };
            let next_at = onsets
                .get(k + 1)
                .map(|&n| AccentGrid::beat_of(bar, n))
                .unwrap_or(be);
            let dur = ((next_at - at) * 0.9).clamp(0.2, 3.9);
            // The harmony arriving after this bar (for approaches).
            let next_ctx = perf.context_at(be).filter(|c| c.start_beat >= be - 1e-6);
            let last_in_bar = k + 1 == onsets.len();
            let root = near(ctx.chord.root_pc, CENTER);
            let (pitch, f, tag) = match eb.bass {
                BassMode::Pedal => {
                    // Hold the slot's first root under the changing harmony.
                    // The root at the start of this gesture slot, held underneath.
                    let mut sb = bar;
                    while sb > 0
                        && perf.bar(sb - 1).is_some_and(|p| {
                            p.gesture == eb.gesture
                                && p.cycle == eb.cycle
                                && p.bass == BassMode::Pedal
                        })
                    {
                        sb -= 1;
                    }
                    let pedal = perf
                        .context_at(AccentGrid::beat_of(sb, 0))
                        .map(|c| c.chord.root_pc)
                        .unwrap_or(ctx.chord.root_pc);
                    let p = near(pedal, CENTER);
                    let f = if ctx.chord.contains_pc(pedal) {
                        PitchFunction::ChordTone
                    } else {
                        PitchFunction::PedalTone
                    };
                    (p, f, "pedal")
                }
                BassMode::Walk if last_in_bar && next_ctx.is_some() => {
                    // Step into the next root by a semitone: a bounded chromatic approach.
                    let nr = near(next_ctx.unwrap().chord.root_pc, root);
                    let dir = if nr >= root { 1 } else { -1 };
                    (nr - dir, PitchFunction::ChromaticApproach, "approach")
                }
                BassMode::Walk => {
                    // Chord tones climbing through the bar: root, 3rd, 5th (a lift).
                    let tones = ctx.chord.pitch_classes();
                    let pc = tones[k.min(tones.len() - 1)];
                    let mut p = near(pc, root + 4);
                    if p < root {
                        p += 12;
                    }
                    (p, PitchFunction::ChordTone, "walk")
                }
                BassMode::Counter if k > 0 && !lead_active(at, at + 0.5) => {
                    // A melodic counterline in the lead's gap: the chord's guide tones.
                    let g = &ctx.palette.guide_tones;
                    let pc = g
                        .get(k % g.len().max(1))
                        .copied()
                        .unwrap_or(ctx.chord.root_pc);
                    (near(pc, CENTER + 7), PitchFunction::ChordTone, "counter")
                }
                _ if k > 0 && last_in_bar && s >= 14 && next_ctx.is_some() => {
                    let nr = near(next_ctx.unwrap().chord.root_pc, root);
                    let dir = if nr >= root { 1 } else { -1 };
                    (nr - dir, PitchFunction::ChromaticApproach, "approach")
                }
                _ if k % 2 == 1 => (
                    near(fifth_of(ctx), CENTER),
                    PitchFunction::ChordTone,
                    "fifth",
                ),
                _ => (root, PitchFunction::ChordTone, "root"),
            };
            let dur = if f == PitchFunction::ChromaticApproach {
                dur.min(1.0)
            } else {
                dur
            };
            let mut n = note(
                at,
                dur,
                pitch,
                vel * if s == 0 { 1.0 } else { 0.85 },
                f,
                tag,
            );
            // Exact provenance: the planned accents on this step, the bass's own pickups and
            // resolutions, the Deflect it makes concrete (the missed root under the pointer's
            // target), the displacement it sits in off the beat.
            let tol = super::performance::STEP_BEATS * 0.5;
            let mut st = perf
                .actions_starting(&[ActionKind::Push, ActionKind::Hit], at, tol, None)
                .chain(perf.actions_starting(
                    &[ActionKind::Resolve, ActionKind::ReEntry],
                    at,
                    tol,
                    Some(Agent::Bass),
                ))
                .fold(ActionStamp::NONE, ActionStamp::with);
            if s == 0 || tag == "pedal" {
                st = perf
                    .actions_starting(&[ActionKind::Deflect], at, tol, None)
                    .fold(st, ActionStamp::with);
            }
            if s % 4 != 0 {
                st = perf
                    .actions_covering(&[ActionKind::Displace], at, None)
                    .fold(st, ActionStamp::with);
            }
            n.prov = super::comp::stamped(n.prov, st);
            out.push(n);
        }
    }

    // Figures the bass states (its pickups, fragments, fills — calls or not) and its answers:
    // projections of plan material, in the bass register, on chord tones.
    out.extend(super::comp::figure_notes(
        perf,
        Agent::Bass,
        Role::Bass,
        base_vel * 1.05,
    ));
    for n in out.iter_mut().filter(|n| n.prov.role_note == "figure") {
        n.prov.role_note = "quote";
    }
    for (call, r) in perf.responses_for(Agent::Bass) {
        let mut ans =
            super::comp::answer_notes(perf, call, r, Agent::Bass, Role::Bass, base_vel * 1.05);
        // Bass answers sit on chord tones (the projection already chose them); type them.
        for n in &mut ans {
            if let Some(ctx) = perf.context_at(n.start_beat) {
                n.function = stable_function(ctx, n.pitch);
            }
        }
        out.extend(ans);
    }
    // The ensemble unison: the bass doubles the shared line two octaves down.
    for (id, line) in &unisons {
        for &(at, d, p, f) in line {
            let mut n = note(
                at,
                d as f64,
                super::comp::octave_near(p, CENTER + 4),
                base_vel * 1.05,
                PitchFunction::ChordTone,
                "unison",
            );
            n.prov.motif_xform = Some("unison");
            n.prov = n.prov.realizing(*id);
            n.function = f;
            out.push(n);
        }
    }
    out.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    // One bass voice: trim any note that runs into the next onset.
    for i in 0..out.len().saturating_sub(1) {
        let next = out[i + 1].start_beat;
        let n = &mut out[i];
        if n.start_beat + n.dur_beats as f64 > next - 0.02 {
            n.dur_beats = ((next - n.start_beat) as f32 * 0.95).max(0.1);
        }
    }
    // A pedal is re-struck at each harmony change rather than smeared across it; anything else
    // lifts off when the chord moves.
    let mut restruck = Vec::new();
    for n in &out {
        if n.function != Some(PitchFunction::PedalTone) && n.prov.role_note != "pedal" {
            continue;
        }
        let end = n.start_beat + n.dur_beats as f64;
        for c in perf
            .chords
            .iter()
            .filter(|c| c.start_beat > n.start_beat + 1e-6 && c.start_beat < end - 0.25)
        {
            let seg_end = perf
                .chords
                .iter()
                .map(|x| x.start_beat)
                .find(|&b| b > c.start_beat + 1e-6)
                .unwrap_or(end)
                .min(end);
            let mut r = *n;
            r.start_beat = c.start_beat;
            r.dur_beats = ((seg_end - c.start_beat) as f32 * 0.95).max(0.1);
            r.velocity *= 0.8;
            r.function = Some(if c.chord.contains_pc(pitch_class(n.pitch)) {
                PitchFunction::ChordTone
            } else {
                PitchFunction::PedalTone
            });
            restruck.push(r);
        }
    }
    for n in out.iter_mut().filter(|n| n.prov.role_note == "pedal") {
        if let Some(c) = perf.chords.iter().find(|c| {
            c.start_beat > n.start_beat + 1e-6
                && c.start_beat < n.start_beat + n.dur_beats as f64 - 0.25
        }) {
            n.dur_beats = ((c.start_beat - n.start_beat) as f32 * 0.95).max(0.1);
        }
    }
    out.extend(restruck);
    out.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    super::comp::release_at_harmony_change(&mut out, &perf.chords);
    out
}

fn stable_function(ctx: &HarmonicContext, p: Midi) -> Option<PitchFunction> {
    if ctx.chord.contains_pc(pitch_class(p)) {
        Some(PitchFunction::ChordTone)
    } else if ctx.palette.tensions.contains(&pitch_class(p)) {
        Some(PitchFunction::LicensedExtension)
    } else {
        None
    }
}
