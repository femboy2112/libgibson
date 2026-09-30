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
                    .map_or(true, |x| x.2),
                Role::Bass if n.prov.role_note == "approach" => false,
                Role::Bass if n.prov.role_note == "unison" => slots
                    .iter()
                    .find(|(_, b, _)| (*b - n.start_beat).abs() < 1e-6)
                    .map_or(true, |x| x.2),
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
            prev.is_some_and(|p| p.pitch == t.pitch) && (1..=2).contains(&delta.abs())
        }
        Some(F::ChromaticPassing | F::DiatonicPassing) => prev.is_some_and(|p| {
            let into = n.pitch - p.pitch;
            (1..=2).contains(&into.abs())
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
    let n = &event.note;
    let spb = 60.0 / f64::from(world.tempo_bpm.max(1.0));
    let p = patch(world, n.role);
    let end = audible_end(n.start_beat, f64::from(n.dur_beats), p, world.tempo_bpm);
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
                && audible_end(
                    s.start_beat,
                    f64::from(s.dur_beats),
                    patch(world, s.role),
                    world.tempo_bpm,
                ) > n.start_beat
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
    let latency_limit = if fleeting { 0.55 } else { 0.28 };
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
            let line = annotate(perf, &notes);
            for (i, e) in line
                .iter()
                .enumerate()
                .filter(|(_, e)| connective(e.note.function))
            {
                rows.push(observe(
                    perf,
                    world,
                    e,
                    i.checked_sub(1).map(|j| &notes[j]),
                    notes.get(i + 1),
                    &score.notes,
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
