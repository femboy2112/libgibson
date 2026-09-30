//! Target-relative performance fiber (Round XV).
//!
//! Pitch syntax and performed subordination are separate predicates. Seconds describe attacks
//! and envelope exposure; beats describe meter and immutable destinations. The thresholds below
//! are explicit human-calibrated hypotheses, not psychoacoustic laws or a quality score.
//! Source realizers inspect the chart's support obligation and already committed players. A
//! frozen-score audit can additionally inspect the actual pad; it never repairs that score.
use super::instrument::Patch;
use super::motif::{melodic_events, LineStyle, TargetKind};
use super::performance::{AccentGrid, PerformancePlan};
use super::score::{Note, PitchFunction as F, Provenance, Role, Score};
use super::sonority::{audible_end, release_tail_secs, AUDIBLE_FLOOR_DB};
use super::theory::pitch_class;
use super::world::MusicWorld;
use std::fmt::Write;

/// A source-authored structural bit, retained separately from pitch function.
#[derive(Debug, Clone, Copy)]
pub struct ExpressionEvent {
    pub note: Note,
    pub structural: bool,
}

/// q(E(L)) = q(L): exact ordered skeleton, including provenance and harmonic obligation.
/// Duration and velocity of structural notes are protected too (a stronger law than required).
#[derive(Debug, Clone, PartialEq)]
pub struct StructuralNote {
    pub onset: f64,
    pub duration: f32,
    pub velocity: f32,
    pub pitch: i32,
    pub role: Role,
    pub function: Option<F>,
    pub provenance: Provenance,
}

/// Forget only explicitly optional expressive detail; never infer structure from a pitch label.
pub fn project(line: &[ExpressionEvent]) -> Vec<StructuralNote> {
    line.iter()
        .filter(|e| e.structural)
        .map(|e| StructuralNote {
            onset: e.note.start_beat,
            duration: e.note.dur_beats,
            velocity: e.note.velocity,
            pitch: e.note.pitch,
            role: e.note.role,
            function: e.note.function,
            provenance: e.note.prov,
        })
        .collect()
}

/// Physical viability, independent of the correctness of the pitch-function stamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectiveViability {
    AsWritten,
    NeedsCompression,
    NeedsDifferentArticulation,
    NonViable,
}

/// Every inspected relationship, including unchanged and structurally protected events.
#[derive(Debug, Clone)]
pub struct ExpressionObservation {
    pub note: Note,
    pub target: Option<Note>,
    pub optional: bool,
    pub pitch_valid: bool,
    pub target_latency_secs: f64,
    pub audible_duration_secs: f64,
    pub written_duration_secs: f64,
    /// All current Notes physically retrigger the synth; grace is still an attack.
    pub fresh_attack: bool,
    pub metric_strength: f32,
    pub pickup: f32,
    pub kinetic: f32,
    /// Prospective harmony, not a claim that a later player was already heard.
    pub chart_dissonance: bool,
    /// Actual semitone-class contacts among the supplied, already realized support.
    pub contacts: Vec<(Role, i32)>,
    pub verdict: ConnectiveViability,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct ExpressionDiagnostics {
    pub rows: Vec<ExpressionObservation>,
}

pub fn connective(f: Option<F>) -> bool {
    matches!(
        f,
        Some(
            F::ChromaticApproach
                | F::ChromaticPassing
                | F::DiatonicPassing
                | F::Neighbor
                | F::Enclosure
        )
    )
}

pub fn patch(world: &MusicWorld, role: Role) -> &Patch {
    match role {
        Role::Lead => &world.lead,
        Role::Bass => &world.bass,
        Role::Keys => &world.keys,
        Role::Pad => &world.pad,
    }
}

/// Reconstruct authored lead slots for auditing frozen scores. Production retains the bit
/// directly. Unknown events are protected, as are all bass notes except source-owned pickups.
/// A unison doubles a lead slot, so it inherits that slot's optionality, not its function label.
pub fn annotate(perf: &PerformancePlan, notes: &[Note]) -> Vec<ExpressionEvent> {
    let slots: Vec<_> = perf
        .statements
        .iter()
        .flat_map(|s| {
            melodic_events(
                &s.motif,
                s.start_beat,
                &LineStyle::for_language(&perf.language),
            )
            .into_iter()
            .filter(|e| !e.rest)
            .map(move |e| {
                (
                    s.material,
                    s.start_beat + e.onset,
                    e.target == TargetKind::Structural,
                )
            })
        })
        .collect();
    notes
        .iter()
        .map(|n| {
            let structural = match n.role {
                Role::Lead => slots
                    .iter()
                    .find(|(m, b, _)| {
                        Some(*m) == n.prov.material && (*b - n.start_beat).abs() < 1e-6
                    })
                    .is_none_or(|x| x.2),
                Role::Bass if n.prov.role_note == "approach" => false,
                Role::Bass if n.prov.role_note == "unison" => slots
                    .iter()
                    .find(|(_, b, _)| (*b - n.start_beat).abs() < 1e-6)
                    .is_none_or(|x| x.2),
                _ => true,
            };
            ExpressionEvent {
                note: *n,
                structural,
            }
        })
        .collect()
}

/// Validate the named local geometry from actual neighbors and the destination harmony.
/// Unlike the local classifier's priority order, several pitch functions may be true at once.
/// No attacked sequence can obtain SlidePath here.
pub fn valid_function(
    perf: &PerformancePlan,
    prev: Option<&Note>,
    n: &Note,
    next: Option<&Note>,
) -> bool {
    let current = perf.context_at(n.start_beat);
    if n.function == Some(F::ChordTone) {
        return current.is_some_and(|c| c.chord.contains_pc(pitch_class(n.pitch)));
    }
    if n.function == Some(F::LicensedExtension) {
        return current.is_some_and(|c| c.palette.tensions.contains(&pitch_class(n.pitch)));
    }
    let prev =
        prev.filter(|p| n.start_beat - (p.start_beat + f64::from(p.dur_beats)) <= 1.0 + 1e-6);
    let Some(t) = next else {
        return false;
    };
    if t.start_beat <= n.start_beat
        || t.start_beat - (n.start_beat + f64::from(n.dur_beats)) > 1.0 + 1e-6
    {
        return false;
    }
    let stable = perf.context_at(t.start_beat).is_some_and(|c| {
        c.chord.contains_pc(pitch_class(t.pitch))
            || c.palette.tensions.contains(&pitch_class(t.pitch))
    });
    let delta = t.pitch - n.pitch;
    match n.function {
        Some(F::ChromaticApproach) => stable && delta.abs() == 1,
        Some(F::Neighbor) => {
            stable && prev.is_some_and(|p| p.pitch == t.pitch) && (1..=2).contains(&delta.abs())
        }
        Some(F::ChromaticPassing | F::DiatonicPassing) => prev.is_some_and(|p| {
            let into = n.pitch - p.pitch;
            let diatonic =
                current.is_some_and(|c| c.palette.scale.contains_pc(pitch_class(n.pitch)));
            stable
                && perf
                    .context_at(p.start_beat)
                    .is_some_and(|c| c.palette.is_stable(pitch_class(p.pitch)))
                && (n.function == Some(F::DiatonicPassing)) == diatonic
                && (1..=2).contains(&into.abs())
                && (1..=2).contains(&delta.abs())
                && into.signum() == delta.signum()
        }),
        Some(F::Enclosure) => {
            stable
                && prev.is_some_and(|p| {
                    (1..=2).contains(&(p.pitch - t.pitch).abs())
                        && (1..=2).contains(&delta.abs())
                        && (p.pitch - t.pitch).signum() == delta.signum()
                })
        }
        _ => false,
    }
}

// Reconstruct function after retiming using actual adjacent events and current chord-scale.
// Preserve the original label when still supported; the classifier's priority is not evidence
// that a simultaneously true neighbor relationship vanished.
fn reclassify(
    perf: &PerformancePlan,
    prev: Option<&Note>,
    n: &Note,
    next: Option<&Note>,
) -> Option<F> {
    if valid_function(perf, prev, n, next) {
        return n.function;
    }
    let ci = perf.contexts.iter().position(|c| {
        n.start_beat >= c.start_beat && n.start_beat < c.start_beat + f64::from(c.dur_beats)
    })?;
    let c = &perf.contexts[ci];
    let ctx = super::pitch::PitchContext {
        pitch: n.pitch,
        onset: n.start_beat,
        duration: f64::from(n.dur_beats),
        prev: prev.map(|p| p.pitch),
        next: next.map(|p| p.pitch),
        next_onset: next.map(|p| p.start_beat),
        prev_chord: ci.checked_sub(1).map(|i| perf.contexts[i].chord),
        cur: Some(c.chord),
        next_chord: perf.contexts.get(ci + 1).map(|c| c.chord),
        next_boundary: Some(c.start_beat + f64::from(c.dur_beats)),
        is_strong: (n.start_beat - n.start_beat.round()).abs() < 1e-6,
        licensed: super::pitch::pc_mask(&c.palette.tensions),
    };
    let f = super::pitch::classify(&ctx, &c.palette.scale)?;
    let mut proposed = *n;
    proposed.function = Some(f);
    valid_function(perf, prev, &proposed, next).then_some(f)
}

/// Inspect one attacked connective in both time coordinate systems. Release tails use the
/// existing sonority envelope model, never a second DSP simulator.
pub fn observe(
    perf: &PerformancePlan,
    world: &MusicWorld,
    event: &ExpressionEvent,
    prev: Option<&Note>,
    target: Option<&Note>,
    support: &[Note],
) -> ExpressionObservation {
    observe_with_voice_contract(perf, world, event, prev, target, support, &[])
}

/// Inspect with explicit source-owned voice continuity. Only a linked successor bounds
/// the current direct voice at that attack plus the synth's short choke ramp.
/// This is direct-voice exposure: shared downstream reverb can outlive it. The frozen Round XV
/// observer is exactly the empty-links path; all acceptance thresholds are unchanged.
pub fn observe_with_voice_contract(
    perf: &PerformancePlan,
    world: &MusicWorld,
    event: &ExpressionEvent,
    prev: Option<&Note>,
    target: Option<&Note>,
    support: &[Note],
    links: &[super::voice::VoiceContinuation],
) -> ExpressionObservation {
    let n = &event.note;
    let spb = 60.0 / f64::from(world.tempo_bpm.max(1.0));
    let p = patch(world, n.role);
    let end = super::voice::effective_audible_end(n, world, links);
    let audible = (end - n.start_beat) * spb;
    let latency = target.map_or(f64::INFINITY, |t| (t.start_beat - n.start_beat) * spb);
    let chart_dissonance = perf
        .context_at(n.start_beat)
        .is_some_and(|c| !c.chord.contains_pc(pitch_class(n.pitch)));
    let contacts: Vec<_> = support
        .iter()
        .filter(|s| {
            s.role != n.role
                && matches!((s.pitch - n.pitch).rem_euclid(12), 1 | 11)
                && s.start_beat < end
                && super::voice::effective_audible_end(s, world, links) > n.start_beat
        })
        .map(|s| (s.role, s.pitch))
        .collect();
    let (bar, step) = AccentGrid::step_of(n.start_beat);
    let accent = perf.accent.at(bar, step);
    let kinetic = perf
        .ensemble
        .iter()
        .find(|e| e.bar == bar)
        .map_or(0.5, |e| e.kinetic);
    let pitch_valid = valid_function(perf, prev, n, target);
    // A short decaying lead can carry a half-second written connection without sustaining a
    // separate proposition. A bass has a stricter IOI budget even on a short patch.
    let fleeting = n.role != Role::Bass && audible <= 0.115;
    let latency_limit = if fleeting {
        0.55
    } else if n.role == Role::Bass && audible <= 0.115 {
        0.36
    } else {
        0.28
    };
    let exposure_limit = if n.role == Role::Bass {
        0.33
    } else if chart_dissonance || !contacts.is_empty() {
        0.20
    } else {
        0.24
    };
    let min_gate = 0.025_f64.max(f64::from(p.adsr.0));
    let min_audible = (audible_end(0.0, min_gate / spb, p, world.tempo_bpm)) * spb;
    let (verdict, reason) = if !pitch_valid {
        (
            ConnectiveViability::NonViable,
            "no valid local destination path",
        )
    } else if latency <= latency_limit && audible <= exposure_limit {
        (
            ConnectiveViability::AsWritten,
            "attack and audible lifetime remain subordinate to destination",
        )
    } else if event.structural {
        (
            ConnectiveViability::NeedsDifferentArticulation,
            "structural onset protected; connective retiming is not authorized",
        )
    } else if min_audible > exposure_limit || release_tail_secs(p, AUDIBLE_FLOOR_DB) > 0.18 {
        (
            ConnectiveViability::NonViable,
            "even minimum attacked gate leaves an exposed release tail",
        )
    } else {
        (
            ConnectiveViability::NeedsCompression,
            "separate attack is too exposed or too far from its destination",
        )
    };
    ExpressionObservation {
        note: *n,
        target: target.copied(),
        optional: !event.structural,
        pitch_valid,
        target_latency_secs: latency,
        audible_duration_secs: audible,
        written_duration_secs: f64::from(n.dur_beats) * spb,
        fresh_attack: true,
        metric_strength: accent.push,
        pickup: accent.pickup,
        kinetic,
        chart_dissonance,
        contacts,
        verdict,
        reason,
    }
}

impl ExpressionDiagnostics {
    /// Frozen-score observation only. No notes are changed.
    pub fn measure(perf: &PerformancePlan, score: &Score, world: &MusicWorld) -> Self {
        let mut rows = Vec::new();
        for role in [Role::Lead, Role::Bass] {
            let mut notes: Vec<_> = score.role_notes(role).copied().collect();
            notes.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
            let mut line = annotate(perf, &notes);
            // A performed grace no longer sits on its authored motif slot. Recover its source
            // structural bit from the decision receipt, including downstream shared unisons.
            for e in &mut line {
                if let Some(d) = score.expression_decisions.iter().find(|d| {
                    d.after.is_some_and(|n| {
                        (n.start_beat - e.note.start_beat).abs() < 1e-6
                            && ((n.role == role && n.pitch == e.note.pitch)
                                || e.note.prov.role_note == "unison" && n.role == Role::Lead)
                    })
                }) {
                    e.structural = !d.before.optional;
                }
            }
            for (i, e) in line
                .iter()
                .enumerate()
                .filter(|(_, e)| connective(e.note.function))
            {
                rows.push(observe_with_voice_contract(
                    perf,
                    world,
                    e,
                    i.checked_sub(1).map(|j| &notes[j]),
                    notes.get(i + 1),
                    &score.notes,
                    &score.voice_continuity,
                ));
            }
        }
        Self { rows }
    }

    pub fn report(&self, tempo_bpm: f32) -> String {
        let mut out = String::from("pitch-valid and performance-viable are separate; every note is a fresh attack; support is chart obligation plus supplied audible contacts\n");
        let spb = 60.0 / f64::from(tempo_bpm);
        for r in &self.rows {
            let n = r.note;
            let target = r.target.map_or("none".to_string(), |t| {
                format!(
                    "{}@{:.6}b/{:.6}s {:?}",
                    super::theory::note_name(t.pitch),
                    t.start_beat,
                    t.start_beat * spb,
                    t.function
                )
            });
            let _ = writeln!(out, "{} {} {:?} onset={:.6}b/{:.6}s dur={:.6}b/{:.6}s audible={:.6}s vel={:.6} target={} latency={:.6}s optional={} pitch_valid={} fresh_attack={} metric_push={:.3} pickup={:.3} kinetic={:.3} chart_dissonance={} contacts={:?} {:?}: {}",
                n.role.label(), super::theory::note_name(n.pitch), n.function, n.start_beat, n.start_beat*spb, n.dur_beats, r.written_duration_secs, r.audible_duration_secs, n.velocity, target, r.target_latency_secs, r.optional, r.pitch_valid, r.fresh_attack, r.metric_strength, r.pickup, r.kinetic, r.chart_dissonance, r.contacts, r.verdict, r.reason);
        }
        out
    }
}

/// The physical strategy actually emitted. No legato/glide variant exists without DSP support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionStrategy {
    /// Short attacked pickup at a metrically obligated push/hit onset.
    ShortPickup,
    Grace,
    Burst,
    Substituted,
    Omitted,
}

/// One source decision, including original identity when space replaces an optional note.
#[derive(Debug, Clone)]
pub struct ExpressionDecision {
    pub before: ExpressionObservation,
    pub after: Option<Note>,
    pub after_observation: Option<ExpressionObservation>,
    pub strategy: ExpressionStrategy,
    pub reason: &'static str,
}

/// The source result. Downstream players receive `events`, never the unexpressed candidates.
pub struct ExpressedLine {
    pub events: Vec<ExpressionEvent>,
    pub decisions: Vec<ExpressionDecision>,
}

fn identities_survive(before: &[ExpressionEvent], after: &[ExpressionEvent]) -> bool {
    before.iter().all(|e| {
        e.note
            .prov
            .actions
            .iter()
            .all(|a| after.iter().any(|x| x.note.prov.actions.has(a)))
            && e.note
                .prov
                .material
                .is_none_or(|m| after.iter().any(|x| x.note.prov.material == Some(m)))
    })
}

fn anchored_onset(perf: &PerformancePlan, n: &Note) -> bool {
    perf.actions.actions.iter().any(|a| {
        n.prov.actions.has(a.id)
            && matches!(
                a.kind,
                super::action::ActionKind::Push | super::action::ActionKind::Hit
            )
            && (a.start_beat - n.start_beat).abs() < 1e-6
    })
}

/// Realize optional connective groups inside one responsible player. Stable local destinations
/// are anchors even when they are not structural motif targets. Retiming is always later, never
/// across a harmonic boundary or stage hole, and never moves the destination to save a grace.
/// Candidate paths and their physical viability are checked before emission.
pub fn realize(
    perf: &PerformancePlan,
    world: &MusicWorld,
    line: Vec<ExpressionEvent>,
    support: &[Note],
) -> ExpressedLine {
    let skeleton = project(&line);
    let mut working = line.clone();
    let mut omit = vec![false; line.len()];
    let mut decisions = Vec::new();
    let spb = 60.0 / f64::from(world.tempo_bpm.max(1.0));
    let mut i = 0;
    while i < working.len() {
        if working[i].structural || !connective(working[i].note.function) {
            i += 1;
            continue;
        }
        let start = i;
        while i < working.len()
            && !working[i].structural
            && connective(working[i].note.function)
            && working[i].note.prov.material == working[start].note.prov.material
        {
            i += 1;
        }
        let stop = i;
        let Some(destination) = working.get(stop).map(|e| e.note) else {
            continue;
        };
        let observations: Vec<_> = (start..stop)
            .map(|k| {
                observe(
                    perf,
                    world,
                    &working[k],
                    k.checked_sub(1).map(|j| &working[j].note),
                    working.get(k + 1).map(|e| &e.note),
                    support,
                )
            })
            .collect();
        if observations
            .iter()
            .all(|r| r.verdict == ConnectiveViability::AsWritten)
        {
            continue;
        }
        let kinetic = observations[0].kinetic;
        let p = patch(world, working[start].note.role);
        let min_gate = 0.025_f64.max(f64::from(p.adsr.0));
        let tail = release_tail_secs(p, AUDIBLE_FLOOR_DB);
        // More motion gives a tighter run; pickup accents give it a little more attack. Contact
        // shortens the gate. All of these act in seconds, with conversion only at the IR boundary.
        let contact = observations
            .iter()
            .any(|r| r.chart_dissonance || !r.contacts.is_empty());
        let gate =
            (0.045 + 0.018 * f64::from(kinetic) - if contact { 0.01 } else { 0.0 }).max(min_gate);
        let step = (gate + tail + 0.035 - 0.02 * f64::from(kinetic)).clamp(0.09, 0.22);
        let mut candidate = working.clone();
        for k in start..stop {
            let n = &mut candidate[k].note;
            n.start_beat = destination.start_beat - (stop - k) as f64 * step / spb;
            n.dur_beats = (gate / spb) as f32;
            if anchored_onset(perf, &working[k].note) {
                n.start_beat = working[k].note.start_beat;
                n.dur_beats = (min_gate / spb) as f32;
            }
            let rise = (k - start + 1) as f32 / (stop - start) as f32;
            n.velocity *=
                (0.67 + 0.10 * kinetic + 0.06 * rise + 0.03 * observations[k - start].pickup)
                    .clamp(0.6, 0.88);
        }
        for k in start..stop {
            candidate[k].note.function = reclassify(
                perf,
                k.checked_sub(1).map(|j| &candidate[j].note),
                &candidate[k].note,
                candidate.get(k + 1).map(|e| &e.note),
            );
        }
        let allowed = kinetic >= 0.18
            && (start..stop).all(|k| {
                let old = working[k].note;
                let n = candidate[k].note;
                let agent = if n.role == Role::Bass {
                    super::action::Agent::Bass
                } else {
                    super::action::Agent::Lead
                };
                n.start_beat >= old.start_beat
                    && perf.on_stage(agent, n.start_beat)
                    && !perf.accent.is_hole(n.start_beat)
                    && perf.context_at(old.start_beat).map(|c| c.start_beat)
                        == perf.context_at(n.start_beat).map(|c| c.start_beat)
                    && (k == 0 || candidate[k - 1].note.start_beat < n.start_beat)
                    && observe(
                        perf,
                        world,
                        &candidate[k],
                        k.checked_sub(1).map(|j| &candidate[j].note),
                        candidate.get(k + 1).map(|e| &e.note),
                        support,
                    )
                    .verdict
                        == ConnectiveViability::AsWritten
            });
        if allowed {
            for k in start..stop {
                let after = observe(
                    perf,
                    world,
                    &candidate[k],
                    k.checked_sub(1).map(|j| &candidate[j].note),
                    candidate.get(k + 1).map(|e| &e.note),
                    support,
                );
                decisions.push(ExpressionDecision { before: observations[k-start].clone(), after: Some(candidate[k].note),
                    after_observation: Some(after), strategy: if anchored_onset(perf, &working[k].note) { ExpressionStrategy::ShortPickup } else if stop-start > 1 { ExpressionStrategy::Burst } else { ExpressionStrategy::Grace },
                    reason: "target-relative attack, shorter gate and subordinate accent; destination unchanged" });
                working[k] = candidate[k];
            }
            continue;
        }
        // A failed physical candidate does not force chromatic survival. Prefer a nearby common
        // chord tone strictly inside the original direction, then space. No early duplicate of
        // the destination masquerades as a substitute.
        for k in start..stop {
            if observations[k - start].verdict == ConnectiveViability::AsWritten {
                continue;
            }
            let old = working[k].note;
            let prev = k.checked_sub(1).map(|j| working[j].note);
            let replacement = (-2..=2)
                .filter(|d| *d != 0)
                .map(|d| old.pitch + d)
                .find(|&pitch| {
                    kinetic >= 0.18
                        && pitch != destination.pitch
                        && prev.is_some_and(|prev| {
                            pitch > prev.pitch.min(destination.pitch)
                                && pitch < prev.pitch.max(destination.pitch)
                        })
                        && perf
                            .context_at(old.start_beat)
                            .is_some_and(|c| c.palette.is_stable(pitch_class(pitch)))
                        && perf
                            .context_at(old.start_beat + f64::from(old.dur_beats))
                            .is_some_and(|c| c.palette.is_stable(pitch_class(pitch)))
                        && support.iter().all(|s| {
                            !matches!((s.pitch - pitch).abs(), 1 | 13)
                                || s.start_beat >= old.start_beat + f64::from(old.dur_beats)
                                || audible_end(
                                    s.start_beat,
                                    f64::from(s.dur_beats),
                                    patch(world, s.role),
                                    world.tempo_bpm,
                                ) <= old.start_beat
                        })
                });
            if let Some(pitch) = replacement {
                working[k].note.pitch = pitch;
                working[k].note.function = Some(
                    if perf
                        .context_at(old.start_beat)
                        .is_some_and(|c| c.chord.contains_pc(pitch_class(pitch)))
                    {
                        F::ChordTone
                    } else {
                        F::LicensedExtension
                    },
                );
                decisions.push(ExpressionDecision { before: observations[k-start].clone(), after: Some(working[k].note),
                    after_observation: None, strategy: ExpressionStrategy::Substituted, reason: "nearby stable chord/guide/available tone preserves direction; attacked chromatic candidate failed" });
            } else {
                let remaining: Vec<_> = working
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != k && !omit[*j])
                    .map(|(_, e)| *e)
                    .collect();
                if !anchored_onset(perf, &old) && identities_survive(&line, &remaining) {
                    omit[k] = true;
                    decisions.push(ExpressionDecision {
                        before: observations[k - start].clone(),
                        after: None,
                        after_observation: None,
                        strategy: ExpressionStrategy::Omitted,
                        reason: "optional space: no viable subordinate attack within this phrase",
                    });
                }
            }
        }
    }
    let events: Vec<_> = working
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !omit[*i])
        .map(|(_, e)| e)
        .collect();
    assert_eq!(
        skeleton,
        project(&events),
        "expression moved a structural event"
    );
    assert!(
        identities_survive(&line, &events),
        "expression erased material/action identity"
    );
    ExpressedLine { events, decisions }
}
