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
use super::comp::LineNote;
use super::context::HarmonicContext;
use super::harmonic_state::{candidate, voice_of, HarmonicEnsembleState};
use super::ids::{ActionId, ActionStamp};
use super::performance::{AccentGrid, BassMode, PerformancePlan, STEPS};
use super::plan::CompositionPlan;
use super::score::{Note, PitchFunction, Provenance, Role};
use super::sonority::{is_linear, is_suspension, resolution_of, BassFunction};
use super::theory::{note_name, pitch_class, Midi};
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
pub(super) fn fifth_of(ctx: &HarmonicContext) -> i32 {
    let r = ctx.chord.root_pc;
    ctx.chord
        .pitch_classes()
        .into_iter()
        .min_by_key(|&pc| ((pc - r).rem_euclid(12) - 7).abs())
        .unwrap_or((r + 7).rem_euclid(12))
}

/// Realize the bass. `lead` is already placed and the bass listens to it (its density, its gaps);
/// what another player SAID reaches the bass as plan material, not as notes (see
/// [`super::material`]), so `keys` is only a hook for collision listening. This is the R7b
/// control (`EnsembleCoupling::Independent`); the coupled floor is [`realize_bass_coupled`].
pub fn realize_bass(
    perf: &PerformancePlan,
    plan: &CompositionPlan,
    world: &MusicWorld,
    lead: &[Note],
    keys: &[Note],
) -> Vec<Note> {
    realize(perf, plan, world, lead, keys, None, false)
}

/// The same bass rhythm with approaches aimed at the destination's actual root register.
/// The R11 control retains its old source-relative octave choice in [`realize_bass`].
pub fn realize_bass_temporal(
    perf: &PerformancePlan,
    plan: &CompositionPlan,
    world: &MusicWorld,
    lead: &[Note],
    keys: &[Note],
) -> Vec<Note> {
    realize(perf, plan, world, lead, keys, None, true)
}

/// Express source-owned pickups before drums and pad consume this bass. Shared unisons already
/// inherit the final lead rhythm; they are protected here rather than independently rephrased.
pub fn realize_bass_expressive(
    perf: &PerformancePlan,
    plan: &CompositionPlan,
    world: &MusicWorld,
    lead: &[Note],
    keys: &[Note],
) -> (Vec<Note>, Vec<super::expression::ExpressionDecision>) {
    let notes = realize(perf, plan, world, lead, keys, None, true);
    let line = notes
        .into_iter()
        .map(|note| super::expression::ExpressionEvent {
            structural: note.prov.role_note != "approach",
            note,
        })
        .collect();
    let support: Vec<_> = lead.iter().chain(keys).copied().collect();
    let result = super::expression::realize(perf, world, line, &support);
    (
        result.events.into_iter().map(|e| e.note).collect(),
        result.decisions,
    )
}

/// Realize the bass as the FLOOR of the coupled ensemble (Round VIII, `EnsembleCoupling::CoupledR8`
/// only): the same rhythm and the same stage as [`realize_bass`], but every pitch is chosen as
/// what it does UNDER the band. The ensemble unison keeps its rhythm and picks its own pitch per
/// line note (a lead 9th is the lead's colour, not the bass's chord); figures, answers and echoes
/// are anchored on the root and echo on root/fifth; a projected onset that lands in another
/// harmony is re-checked there; every label is read off the note's actual floor position; and
/// each note asks `state` (only the lead is committed yet) for hard hazards before it is emitted.
/// Every decision is recorded in `state`'s log. The caller commits the result.
pub fn realize_bass_coupled(
    perf: &PerformancePlan,
    plan: &CompositionPlan,
    world: &MusicWorld,
    lead: &[Note],
    state: &mut HarmonicEnsembleState<'_>,
) -> Vec<Note> {
    realize(perf, plan, world, lead, &[], Some(state), false)
}

/// The one bass realizer. `floor` is `None` for the R7b control (byte-identical to Round VIIb) and
/// the coupled ledger otherwise; every Coupled-only step is gated on it.
fn realize(
    perf: &PerformancePlan,
    _plan: &CompositionPlan,
    world: &MusicWorld,
    lead: &[Note],
    _keys: &[Note],
    mut floor: Option<&mut HarmonicEnsembleState<'_>>,
    temporal: bool,
) -> Vec<Note> {
    let coupled = floor.is_some();
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
    let unisons = if temporal {
        super::comp::unison_lines_temporal(perf, lead)
    } else {
        super::comp::unison_lines(perf, lead)
    };
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
                    let nr = near(
                        next_ctx.unwrap().chord.root_pc,
                        if temporal { CENTER } else { root },
                    );
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
                    if coupled {
                        // Coupled: near G2 (43), not B2 (47) — a counterline that sits on the pad's
                        // floor (52) is a second pad, not a bass. Still a chord tone, or the root.
                        let pc = if ctx.chord.contains_pc(pc) {
                            pc
                        } else {
                            ctx.chord.root_pc
                        };
                        (near(pc, CENTER + 3), PitchFunction::ChordTone, "counter")
                    } else {
                        (near(pc, CENTER + 7), PitchFunction::ChordTone, "counter")
                    }
                }
                _ if k > 0 && last_in_bar && s >= 14 && next_ctx.is_some() => {
                    let nr = near(
                        next_ctx.unwrap().chord.root_pc,
                        if temporal { CENTER } else { root },
                    );
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
    if let Some(state) = floor.as_deref_mut() {
        // Coupled: a projected onset that land_once moved onto an accent may now sit in another
        // harmony than the one its pitch was chosen for. Judge it where it actually sounds.
        for n in out
            .iter_mut()
            .filter(|n| matches!(n.prov.role_note, "quote" | "answer"))
        {
            reland(perf, n, state);
        }
        // Coupled: the unison keeps the band's rhythm; the bass chooses the floor under it.
        for (id, line) in &unisons {
            out.extend(unison_floor(perf, *id, line, base_vel * 1.05, state));
        }
    }
    // The ensemble unison: the bass doubles the shared line two octaves down.
    for (id, line) in unisons.iter().filter(|_| !coupled) {
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
    if let Some(state) = floor {
        // Coupled, on the notes exactly as they will sound: labels by floor position, then the
        // ledger's hard-hazard gate.
        for n in &mut out {
            label_by_floor(perf, n);
        }
        gate_hazards(perf, &unisons, &mut out, state);
    }
    out
}

/// Coupled: a bass figure/answer note judged where it actually sounds. Its pitch, over the harmony
/// at its FINAL onset: a chord tone stays (and is labelled one); anything else is re-pitched to the
/// nearest chord tone — the root on the harmony's first beat — and the decision logged. Its end:
/// it does not ring into a following harmony it does not root.
fn reland(perf: &PerformancePlan, n: &mut Note, state: &mut HarmonicEnsembleState<'_>) {
    let Some(ctx) = perf.context_at(n.start_beat) else {
        return;
    };
    let root_pc = ctx.chord.root_pc;
    if !ctx.chord.contains_pc(pitch_class(n.pitch)) {
        let q = if n.start_beat < ctx.start_beat + 1.0 - 1e-6 {
            near(root_pc, n.pitch)
        } else {
            // Nearest chord tone; ties go to the root, then downward (explicit, deterministic).
            (1..=6)
                .flat_map(|d| [n.pitch - d, n.pitch + d])
                .filter(|&p| ctx.chord.contains_pc(pitch_class(p)))
                .min_by_key(|&p| ((p - n.pitch).abs(), pitch_class(p) != root_pc, p > n.pitch))
                .unwrap_or_else(|| near(root_pc, n.pitch))
        };
        state.record(
            n.start_beat,
            Role::Bass,
            "revoiced",
            format!(
                "re-pitched at landing: bass {} {} is {} over {}; {} instead",
                n.prov.role_note,
                note_name(n.pitch),
                BassFunction::of(&ctx.chord, n.pitch).label(),
                ctx.chord.label(),
                note_name(q)
            ),
        );
        n.pitch = q;
    }
    n.function = Some(PitchFunction::ChordTone);
    // A material note that rings into the NEXT harmony lifts off there unless it is that
    // harmony's root: a 3rd or 5th carried under a new chord nobody else roots is an inversion
    // the phrase never asked for (the root-anchored contour can arrive on the fifth). Judged on
    // the pitch actually emitted, so after any re-pitch above.
    let end = n.start_beat + n.dur_beats as f64;
    if let Some(c) = perf.chords.iter().find(|c| {
        c.start_beat > n.start_beat + 1e-6
            && c.start_beat < end - 1e-6
            && c.chord.root_pc != pitch_class(n.pitch)
    }) {
        n.dur_beats = ((c.start_beat - n.start_beat) as f32 * 0.97).max(0.1);
    }
}

/// Coupled: the bass's own part of the ensemble unison `id` over the shared `line`. The rhythm is
/// the band's (onsets, durations, the action's stamp); the pitch is the floor's. A resting line
/// note that is the chord's root or fifth is doubled; a resting 3rd/7th/6th or any tension puts
/// the ROOT underneath instead; a linear or suspended line note is not re-attacked — the previous
/// bass note holds through it. One onset must still share its pitch class with the keys (who
/// double the line itself): the first root/fifth doubling does that; failing one, the first
/// chord-tone line note is played as a short inversion the unison action owns.
fn unison_floor(
    perf: &PerformancePlan,
    id: ActionId,
    line: &[LineNote],
    vel: f32,
    state: &mut HarmonicEnsembleState<'_>,
) -> Vec<Note> {
    let resting = |f: Option<PitchFunction>| !f.is_some_and(|f| is_linear(f) || is_suspension(f));
    let chord_tone = |at: f64, p: Midi| {
        perf.context_at(at)
            .is_some_and(|c| c.chord.contains_pc(pitch_class(p)))
    };
    let foundation = |at: f64, p: Midi| {
        chord_tone(at, p)
            && perf
                .context_at(at)
                .is_some_and(|c| BassFunction::of(&c.chord, p).is_foundation())
    };
    // The witness onset, when no resting root/fifth doubling supplies one by itself.
    let natural = line
        .iter()
        .any(|&(at, _, p, f)| resting(f) && foundation(at, p));
    let owned = if natural {
        None
    } else {
        line.iter().position(|&(at, _, p, _)| chord_tone(at, p))
    };
    if !natural && owned.is_none() {
        if let Some(&(at, ..)) = line.first() {
            state.record(
                at,
                Role::Bass,
                "substituted",
                "unison line has no chord tone at all: the bass keeps the roots, and no bass onset \
                 shares a pitch class with the keys' line"
                    .into(),
            );
        }
    }
    let mut out: Vec<Note> = Vec::new();
    // Whether the last emitted note is the owned inversion (never stretched past its beat).
    let mut last_owned = false;
    for (i, &(at, d, p, f)) in line.iter().enumerate() {
        let Some(ctx) = perf.context_at(at) else {
            continue;
        };
        let root = near(ctx.chord.root_pc, CENTER);
        let lead = note_name(p);
        let (pitch, dur) = if owned == Some(i) {
            state.record(
                at,
                Role::Bass,
                "owned",
                format!(
                    "inversion owned by the unison action: bass doubles lead {lead} ({}) for at most a beat",
                    BassFunction::of(&ctx.chord, p).label()
                ),
            );
            (super::comp::octave_near(p, CENTER + 4), d.min(0.9))
        } else if resting(f) && foundation(at, p) {
            (super::comp::octave_near(p, CENTER + 4), d)
        } else if !resting(f) {
            // A moving line note: hold what is already sounding when it still belongs to this
            // harmony; otherwise (nothing to hold, or the chord moved under it) state the root.
            if let Some(prev) = out
                .last_mut()
                .filter(|q| !last_owned && chord_tone(at, q.pitch))
            {
                prev.dur_beats = ((at + d as f64 - prev.start_beat) as f32).max(prev.dur_beats);
                state.record(
                    at,
                    Role::Bass,
                    "held",
                    format!(
                        "lead {lead} is {} (linear): the bass holds {} through it, no new attack",
                        f.map(|x| x.label()).unwrap_or("NONE"),
                        note_name(prev.pitch)
                    ),
                );
                continue;
            }
            state.record(
                at,
                Role::Bass,
                "substituted",
                format!(
                    "lead {lead} is {} (linear) with no bass note to hold through it: root {} instead",
                    f.map(|x| x.label()).unwrap_or("NONE"),
                    note_name(root)
                ),
            );
            (root, d)
        } else {
            state.record(
                at,
                Role::Bass,
                "substituted",
                format!(
                    "lead {lead} would be the {} under {}: root {} instead",
                    BassFunction::of(&ctx.chord, p).label(),
                    ctx.chord.label(),
                    note_name(root)
                ),
            );
            (root, d)
        };
        let mut n = note(
            at,
            dur as f64,
            pitch,
            vel,
            PitchFunction::ChordTone,
            "unison",
        );
        n.prov.motif_xform = Some("unison");
        n.prov = n.prov.realizing(id);
        out.push(n);
        last_owned = owned == Some(i);
    }
    out
}

/// Coupled: a bass note's `function` is what its pitch IS over the harmony at its onset — a chord
/// tone is a `ChordTone`; an approach stays an approach and a pedal a pedal (they own their
/// dissonance by motion and by structure). The bass never carries a `LicensedExtension`: a tension
/// in the floor is a different chord, not a colour.
fn label_by_floor(perf: &PerformancePlan, n: &mut Note) {
    if matches!(
        n.function,
        Some(PitchFunction::ChromaticApproach) | Some(PitchFunction::PedalTone)
    ) {
        return;
    }
    let Some(ctx) = perf.context_at(n.start_beat) else {
        return;
    };
    if ctx.chord.contains_pc(pitch_class(n.pitch)) {
        n.function = Some(PitchFunction::ChordTone);
    } else if n.function == Some(PitchFunction::LicensedExtension) {
        n.function = None;
    }
}

/// Coupled: every bass note asks the ledger (the lead committed) for HARD hazards before it is
/// emitted. On a hazard: an unstamped approach is dropped (space beats garbage); otherwise the
/// root, the fifth, then the same pitch class an octave down/up (within 28..=52) are tried in
/// that order. A note stamped with a Deflect, or a unison onset sharing the keys' pitch class (the
/// witnesses need that exact class), may only move by octaves. A note stamped with any action is
/// never dropped: with no clean alternative it stays, and says so. Every decision is logged with
/// the hazards' own descriptions.
fn gate_hazards(
    perf: &PerformancePlan,
    unisons: &[(ActionId, Vec<LineNote>)],
    out: &mut Vec<Note>,
    state: &mut HarmonicEnsembleState<'_>,
) {
    const LO: Midi = 28;
    const HI: Midi = 52;
    let mut i = 0;
    while i < out.len() {
        let n = out[i];
        // The ledger's question, with the resolution the audit will observe on the bass line as
        // it stands (a linear note only owns its dissonance by resolving).
        let judge = |out: &[Note], pitch: Midi, function: Option<PitchFunction>| {
            let mut c = candidate(
                Role::Bass,
                pitch,
                n.start_beat,
                n.start_beat + n.dur_beats as f64,
                function,
                n.prov.role_note,
            );
            if function.is_some_and(|f| is_linear(f) || is_suspension(f)) {
                let line: Vec<_> = out.iter().map(voice_of).collect();
                c.resolves = resolution_of(&line, &c, state.contexts()).is_some();
            }
            state.hard_hazards(&c)
        };
        let hz = judge(out, n.pitch, n.function);
        if hz.is_empty() {
            i += 1;
            continue;
        }
        let why = hz
            .iter()
            .map(|h| h.describe(n.pitch))
            .collect::<Vec<_>>()
            .join("; ");
        let stamped = !n.prov.actions.is_empty();
        if n.function == Some(PitchFunction::ChromaticApproach) && !stamped {
            state.record(
                n.start_beat,
                Role::Bass,
                "refused",
                format!("approach dropped: {why}"),
            );
            out.remove(i);
            continue;
        }
        let deflect = n.prov.actions.iter().any(|a| {
            perf.actions
                .get(a)
                .is_some_and(|x| x.kind == ActionKind::Deflect)
        });
        let unison_witness = n.prov.role_note == "unison"
            && unisons.iter().any(|(id, l)| {
                n.prov.actions.iter().any(|a| a == *id)
                    && l.iter().any(|x| {
                        (x.0 - n.start_beat).abs() < 1e-6
                            && pitch_class(x.2) == pitch_class(n.pitch)
                    })
            });
        let octaves = [n.pitch - 12, n.pitch + 12];
        let mut alts: Vec<Midi> = Vec::new();
        if !(deflect || unison_witness) {
            if let Some(ctx) = perf.context_at(n.start_beat) {
                alts.push(near(ctx.chord.root_pc, n.pitch));
                alts.push(near(fifth_of(ctx), n.pitch));
            }
        }
        alts.extend(octaves);
        // Into range by octaves (root/fifth), then only real, distinct alternatives.
        let alts: Vec<Midi> = alts
            .into_iter()
            .map(|p| match p {
                p if p < LO && p + 12 <= HI => p + 12,
                p if p > HI && p - 12 >= LO => p - 12,
                p => p,
            })
            .filter(|&p| (LO..=HI).contains(&p) && p != n.pitch)
            .fold(Vec::new(), |mut v, p| {
                if !v.contains(&p) {
                    v.push(p);
                }
                v
            });
        let fixed = alts.into_iter().find_map(|q| {
            // Same class keeps its label; a new class is a chord tone (root or fifth).
            let f = if pitch_class(q) == pitch_class(n.pitch) {
                n.function
            } else {
                Some(PitchFunction::ChordTone)
            };
            judge(out, q, f).is_empty().then_some((q, f))
        });
        match fixed {
            Some((q, f)) => {
                let what = if pitch_class(q) == pitch_class(n.pitch) {
                    "octave"
                } else {
                    "revoiced"
                };
                state.record(
                    n.start_beat,
                    Role::Bass,
                    what,
                    format!("{why}: {} instead", note_name(q)),
                );
                out[i].pitch = q;
                out[i].function = f;
                i += 1;
            }
            None if stamped => {
                state.record(
                    n.start_beat,
                    Role::Bass,
                    "kept",
                    format!("{why}: no clean alternative, and an action is stamped on it"),
                );
                i += 1;
            }
            None => {
                state.record(
                    n.start_beat,
                    Role::Bass,
                    "refused",
                    format!("{why}: no clean alternative; dropped"),
                );
                out.remove(i);
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::super::action::{ActionCause, EffectVector, MusicalAction};
    use super::super::contract::CompositionGrammar;
    use super::super::functor::{compose_full, realize_performance, Composition};
    use super::super::performance::{EnsembleCoupling, PerformanceOptions};
    use super::super::semantic::deflected_lift_trace;
    use super::super::sonority::{ColorPolicy, EnsembleSonorityDiagnostics};
    use super::super::witness::audit;
    use super::super::world::MusicWorld;
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

    /// ADVERSARIAL: an ensemble unison injected over a lead line whose first note is a resting
    /// licensed extension (a 9th/11th/13th the lead owns as colour). The independent control
    /// octave-copies it into the bass — the tension becomes the floor, which is the R8 listen's
    /// "out of tune despite individually justified notes" — and the coupled bass must not.
    #[test]
    fn a_unison_over_a_lead_tension_does_not_put_the_tension_in_the_bass() {
        let mut exercised = 0;
        for world in MusicWorld::all() {
            let c = flagship(&world, EnsembleCoupling::Independent);
            let p = &c.perf;
            let lead: Vec<&Note> = c
                .score
                .notes
                .iter()
                .filter(|n| n.role == Role::Lead)
                .collect();
            // Candidates: a resting lead extension that would be a TENSION as the floor, followed
            // within two beats by another lead onset, keys and bass both on stage, and no planned
            // unison or bass material anywhere near (so the injected action is the only cause).
            let busy = |a: f64, b: f64| {
                p.actions
                    .actions
                    .iter()
                    .any(|x| x.kind == ActionKind::Unison && x.start_beat < b && x.end_beat() > a)
                    || p.figures_for(Agent::Bass)
                        .any(|m| m.start_beat < b && m.start_beat + m.length() > a)
                    || p.responses_for(Agent::Bass)
                        .any(|(_, r)| r.start_beat < b && r.start_beat + r.dur_beats > a)
            };
            let found = lead.iter().enumerate().find_map(|(i, n)| {
                let ctx = p.context_at(n.start_beat)?;
                let next = lead.get(i + 1)?;
                let t = n.start_beat;
                let end = next.start_beat + 0.05;
                (n.function == Some(PitchFunction::LicensedExtension)
                    && n.dur_beats >= 0.25
                    && matches!(
                        BassFunction::of(&ctx.chord, n.pitch),
                        BassFunction::Tension(_)
                    )
                    && next.start_beat - t <= 2.0
                    && [Agent::Keys, Agent::Bass]
                        .iter()
                        .all(|&g| p.stage.on_stage_span(g, t, end))
                    && !busy(t - 1.0, end + 1.0))
                .then_some((t, end, n.pitch))
            });
            let Some((t, end, ext)) = found else {
                eprintln!("{}: no lead extension window for the probe", world.name);
                continue;
            };
            let mut injected = c.perf.clone();
            let id = injected.actions.push(MusicalAction {
                id: ActionId(0),
                cause: ActionCause::Statement { phrase: 0 },
                initiator: Agent::Ensemble,
                start_beat: t,
                dur_beats: end - t,
                kind: ActionKind::Unison,
                target_beat: None,
                responders: vec![],
                binding: None,
                pays: None,
                effect: EffectVector::NEUTRAL,
            });
            let mut floors = Vec::new();
            for coupling in [EnsembleCoupling::Independent, EnsembleCoupling::CoupledR8] {
                injected.coupling = coupling;
                let score = realize_performance(&c.song, &world, &injected);
                // The same lead extension is there to be doubled, in both arms.
                if !score.notes.iter().any(|n| {
                    n.role == Role::Lead
                        && (n.start_beat - t).abs() < 1e-6
                        && n.pitch == ext
                        && n.function == Some(PitchFunction::LicensedExtension)
                }) {
                    break;
                }
                let Some(b) = score.notes.iter().find(|n| {
                    n.role == Role::Bass
                        && (n.start_beat - t).abs() < 1e-6
                        && n.prov.actions.iter().any(|a| a == id)
                }) else {
                    break;
                };
                let chord = &injected.context_at(t).unwrap().chord;
                floors.push((coupling, *b, BassFunction::of(chord, b.pitch)));
            }
            let [(_, old, old_f), (_, new, new_f)] = floors.as_slice() else {
                eprintln!(
                    "{}: the injected unison did not sound in both arms",
                    world.name
                );
                continue;
            };
            eprintln!(
                "{}: unison over lead {} at {t}: independent bass {} = {} ({:?}); coupled bass {} = {} ({:?})",
                world.name,
                note_name(ext),
                note_name(old.pitch),
                old_f.label(),
                old.function,
                note_name(new.pitch),
                new_f.label(),
                new.function,
            );
            // The control reproduces the bug (the test discriminates)...
            assert_eq!(pitch_class(old.pitch), pitch_class(ext), "{}", world.name);
            assert!(
                matches!(old_f, BassFunction::Tension(_)),
                "{}: the control no longer octave-copies the lead's tension",
                world.name
            );
            // ...and the coupled floor does not.
            assert!(
                new_f.is_foundation()
                    || (matches!(new_f, BassFunction::Third | BassFunction::Seventh)
                        && new.dur_beats < 1.0),
                "{}: coupled bass under a lead tension is {} ({:?})",
                world.name,
                new_f.label(),
                new.function
            );
            assert_ne!(
                new.function,
                Some(PitchFunction::LicensedExtension),
                "{}",
                world.name
            );
            exercised += 1;
        }
        assert!(exercised >= 1, "the probe found no window in any world");
    }

    /// The flagship under the coupled floor, every world: no bass note names a tension or a
    /// non-chord tone nobody owns, no bass note is labelled a licensed extension, no held identity
    /// flip, every planned action (the Unison and every Deflect included) still audibly witnessed,
    /// and the lead needed no repairs.
    #[test]
    fn the_coupled_flagship_bass_owns_the_floor() {
        for world in MusicWorld::all() {
            let c = flagship(&world, EnsembleCoupling::CoupledR8);
            let d = EnsembleSonorityDiagnostics::measure(
                &c.score,
                &c.perf.contexts,
                &ColorPolicy::for_world(world.id, &c.perf.language),
                &[],
            );
            let r = audit(&c.perf, &c.score);
            let unison: Vec<bool> = r
                .rows
                .iter()
                .filter(|w| w.kind == ActionKind::Unison)
                .map(|w| w.witnessed)
                .collect();
            let decided = |what: &str| {
                c.score
                    .vertical_decisions
                    .iter()
                    .filter(|v| v.role == Role::Bass && v.what == what)
                    .count()
            };
            eprintln!(
                "{}: bass_function_violations={} identity_flips={} (held {}) unowned m2={} m9={} \
                 unison substituted={} held={} owned={} | gate revoiced={} octave={} refused={} kept={} \
                 | unison witnessed {unison:?} | witnessed {}/{} | melody_repairs={}",
                world.name,
                d.bass_function_violations,
                d.identity_flips,
                d.identity_flips_held,
                d.unowned_m2,
                d.unowned_m9,
                decided("substituted"),
                decided("held"),
                decided("owned"),
                decided("revoiced"),
                decided("octave"),
                decided("refused"),
                decided("kept"),
                r.witnessed(),
                r.total(),
                c.score.melody_repairs,
            );
            for v in &c.score.vertical_decisions {
                eprintln!(
                    "  {:>7.2} {} {}: {}",
                    v.beat,
                    v.role.label(),
                    v.what,
                    v.reason
                );
            }
            assert_eq!(d.bass_function_violations, 0, "{}", world.name);
            assert_eq!(d.identity_flips_held, 0, "{}", world.name);
            assert!(
                !c.score.notes.iter().any(|n| n.role == Role::Bass
                    && n.function == Some(PitchFunction::LicensedExtension)),
                "{}: a coupled bass note is labelled a licensed extension",
                world.name
            );
            assert!(
                !unison.is_empty(),
                "{}: the flagship plans no unison",
                world.name
            );
            assert_eq!(r.witnessed(), r.total(), "{}: {}", world.name, r.report());
            assert_eq!(c.score.melody_repairs, 0, "{}", world.name);
        }
    }
}
