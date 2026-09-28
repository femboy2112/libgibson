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

/// Realize the bass. `lead` and `keys` are already placed: the bass listens to both.
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
        .figure_calls_for(Agent::Bass)
        .map(|c| (c.start_beat, c.end_beat))
        .chain(
            perf.responses_for(Agent::Bass)
                .map(|(_, r)| (r.start_beat, r.start_beat + r.dur_beats)),
        )
        .collect();
    let in_quote = |b: f64| quoted.iter().any(|&(s, e)| b >= s - 1e-6 && b < e - 1e-6);

    for eb in &perf.ensemble {
        let bar = eb.bar;
        let bs = AccentGrid::beat_of(bar, 0);
        let be = bs + 4.0;
        let vel = base_vel * (0.85 + 0.25 * eb.kinetic);
        // Onset steps this bar, by mode.
        let mut onsets: Vec<usize> = vec![0];
        match eb.bass {
            BassMode::Pedal => {}
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
                let n = if eb.kinetic > 0.6 { 3 } else { 2 };
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
            !perf.accent.is_hole(b) && !in_quote(b)
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
            out.push(note(
                at,
                dur,
                pitch,
                vel * if s == 0 { 1.0 } else { 0.85 },
                f,
                tag,
            ));
        }
    }

    // Figures the bass states (calls) and answers: motif material in the bass register.
    for c in perf.figure_calls_for(Agent::Bass) {
        let cell = &perf.bank.bass_cell;
        let head = perf.bank.identity.fragment(3.min(perf.bank.identity.len()));
        let figure = if cell.len() >= 2 { head } else { cell.clone() };
        let mut at = c.start_beat;
        for (i, &deg) in figure.degrees.iter().enumerate() {
            if at >= c.end_beat - 1e-6 {
                break;
            }
            let Some(ctx) = perf.context_at(at) else {
                break;
            };
            let raw = perf.region.degree_pitch(deg, 2);
            let p = nearest_stable_bass(ctx, raw);
            let d = (figure.rhythm.get(i).copied().unwrap_or(0.5) as f64).min(c.end_beat - at);
            let mut n = note(
                at,
                d * 0.9,
                p,
                base_vel * 1.05,
                PitchFunction::ChordTone,
                "quote",
            );
            n.prov.motif_xform = Some("figure");
            n.function = stable_function(ctx, p);
            out.push(n);
            at += d.max(0.25);
        }
    }
    for (call, r) in perf.responses_for(Agent::Bass) {
        let src: Vec<Note> = lead
            .iter()
            .filter(|n| {
                n.start_beat >= call.start_beat - 1e-6 && n.start_beat < call.end_beat - 1e-6
            })
            .copied()
            .collect();
        out.extend(super::comp::answer_notes(
            perf,
            &src,
            r.start_beat,
            r.dur_beats,
            r.transform,
            CENTER + 5,
            Role::Bass,
            base_vel * 1.05,
        ));
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

fn nearest_stable_bass(ctx: &HarmonicContext, target: Midi) -> Midi {
    (0..=12)
        .flat_map(|d| [target - d, target + d])
        .find(|&p| ctx.chord.contains_pc(pitch_class(p)))
        .unwrap_or(target)
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
