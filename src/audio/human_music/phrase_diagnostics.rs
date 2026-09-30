//! Read-only phrase-surface evidence. Local connective viability is necessary but cannot
//! distinguish a phrase from repeated instances of the same individually legal ornament.
//! Time bins below expose near-repetition; they are declared measurements, not quality scores.

use super::expression::{annotate, connective, patch};
use super::performance::{PerformancePlan, StepWeight};
use super::plan::CompositionPlan;
use super::score::{Note, Role, Score};
use super::sonority::audible_end;
use super::world::MusicWorld;
use std::collections::BTreeMap;
use std::fmt::Write;

/// A declared musical subdivision, with tight tolerance so seconds-derived near-sixteenths
/// do not silently become intentional grid positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceGrid {
    Beat,
    Eighth,
    Sixteenth,
    ThirtySecond,
    Triplet,
    OffGrid,
}

pub fn grid_at(beat: f64) -> SurfaceGrid {
    for (denom, grid) in [
        (1.0, SurfaceGrid::Beat),
        (2.0, SurfaceGrid::Eighth),
        (4.0, SurfaceGrid::Sixteenth),
        (8.0, SurfaceGrid::ThirtySecond),
        (3.0, SurfaceGrid::Triplet),
        (6.0, SurfaceGrid::Triplet),
    ] {
        if (beat * denom - (beat * denom).round()).abs() < 1e-6 {
            return grid;
        }
    }
    SurfaceGrid::OffGrid
}

#[derive(Debug, Clone)]
pub struct SurfaceNote {
    /// Contains pitch, written gate, velocity and complete source provenance.
    pub note: Note,
    pub source_onset: f64,
    pub structural: bool,
    pub grid: SurfaceGrid,
    pub next_ioi_beats: Option<f64>,
    pub next_ioi_secs: Option<f64>,
    pub audible_secs: f64,
    /// Positive acoustic silence before the next same-role attack.
    pub silence_to_next_secs: Option<f64>,
    pub silence_before_secs: f64,
    pub accent: StepWeight,
    pub kinetic: f32,
}

/// Coarse temporal morphology deliberately forgets pitch and absolute onset. Both dimensions
/// use 20-ms bins, preventing tiny floating-point/kinetic changes from faking rhythmic variety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConnectiveShape {
    pub target_latency_20ms: i64,
    pub gate_20ms: i64,
}

/// A calibrated morphology witness, not a universal judgment about rests or grace notes.
pub fn isolated_grace(silence_before_secs: f64, gate_secs: f64, target_latency_secs: f64) -> bool {
    silence_before_secs >= 0.25 && gate_secs <= 0.065 && target_latency_secs <= 0.20
}

pub fn temporal_shape(latency_secs: f64, gate_secs: f64) -> ConnectiveShape {
    ConnectiveShape {
        target_latency_20ms: (latency_secs / 0.020).round() as i64,
        gate_20ms: (gate_secs / 0.020).round() as i64,
    }
}

#[derive(Debug, Clone)]
pub struct ShapeOccurrences {
    pub shape: ConnectiveShape,
    pub source_onsets: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct RoleSurface {
    pub phrase: u32,
    pub role: Role,
    pub start_beat: f64,
    pub end_beat: f64,
    pub discourse: &'static str,
    pub notes: Vec<SurfaceNote>,
    /// All optional connective shapes, not merely repeated or changed notes.
    pub shapes: Vec<ShapeOccurrences>,
    pub attacks_per_beat: f64,
    pub audible_occupied_beats: f64,
    /// Consecutive IOI pairs with a ratio of at least four. A witness, not a defect verdict.
    pub abrupt_ioi_changes: Vec<(f64, f64, f64)>,
    /// Source slots abandoned by retiming, paired with final onset.
    pub vacated_slots: Vec<(f64, f64)>,
    /// Long acoustic space followed by a tiny attacked connector and close destination.
    pub isolated_grace_onsets: Vec<f64>,
    /// Consecutive optional connectors with the same coarse physical shape.
    pub repeated_shape_runs: Vec<Vec<f64>>,
    /// Sliding one-beat windows containing at least four distinct attacks.
    pub density_spikes: Vec<(f64, usize)>,
    /// Adjacent velocity differences of at least 0.20, retaining both values.
    pub velocity_discontinuities: Vec<(f64, f32, f32)>,
}

#[derive(Debug, Clone)]
pub struct RoleContact {
    pub phrase: u32,
    pub roles: (Role, Role),
    /// Pairs of distinct onset times within 30 ms. Polyphonic voicing does not multiply hits.
    pub synchronized_onsets: Vec<(f64, f64)>,
    pub synchronized_connective_shapes: Vec<(f64, f64, ConnectiveShape)>,
}

#[derive(Debug, Clone, Default)]
pub struct PhraseSurfaceDiagnostics {
    pub roles: Vec<RoleSurface>,
    pub contacts: Vec<RoleContact>,
}

/// Length of the union of clipped audible intervals. Polyphony cannot inflate occupation.
pub fn occupied_beats(intervals: &[(f64, f64)], start: f64, end: f64) -> f64 {
    let mut spans: Vec<_> = intervals
        .iter()
        .map(|&(a, b)| (a.max(start), b.min(end)))
        .filter(|&(a, b)| b > a)
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut total = 0.0;
    let mut until = start;
    for (a, b) in spans {
        total += (b - a.max(until)).max(0.0);
        until = until.max(b);
    }
    total
}

impl PhraseSurfaceDiagnostics {
    pub fn measure(
        plan: &CompositionPlan,
        perf: &PerformancePlan,
        score: &Score,
        world: &MusicWorld,
    ) -> Self {
        let mut out = Self::default();
        let spb = 60.0 / f64::from(score.tempo_bpm.max(1.0));
        for phrase in &plan.form.phrases {
            let start = phrase.start_beat();
            let end = phrase.end_beat();
            let base = out.roles.len();
            for role in [Role::Lead, Role::Bass, Role::Keys] {
                let mut all: Vec<_> = score.role_notes(role).copied().collect();
                all.sort_by(|a, b| {
                    a.start_beat
                        .total_cmp(&b.start_beat)
                        .then(a.pitch.cmp(&b.pitch))
                });
                let authored = annotate(perf, &all);
                let mut notes = Vec::new();
                let mut shapes: BTreeMap<ConnectiveShape, Vec<f64>> = BTreeMap::new();
                for (i, n) in all
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.start_beat >= start && n.start_beat < end)
                {
                    let decision = score.expression_decisions.iter().find(|d| {
                        d.after.is_some_and(|a| {
                            (a.start_beat - n.start_beat).abs() < 1e-6
                                && ((a.role == role && a.pitch == n.pitch)
                                    || n.prov.role_note == "unison" && a.role == Role::Lead)
                        })
                    });
                    let source_onset = decision.map_or(n.start_beat, |d| d.before.note.start_beat);
                    let structural =
                        decision.map_or(authored[i].structural, |d| !d.before.optional);
                    let next = all
                        .iter()
                        .skip(i + 1)
                        .find(|x| x.start_beat > n.start_beat + 1e-6);
                    let ioi = next.map(|x| x.start_beat - n.start_beat);
                    let audible_end = audible_end(
                        n.start_beat,
                        f64::from(n.dur_beats),
                        patch(world, role),
                        score.tempo_bpm,
                    );
                    if !structural && connective(n.function) {
                        if let Some(latency) = ioi {
                            shapes
                                .entry(temporal_shape(latency * spb, f64::from(n.dur_beats) * spb))
                                .or_default()
                                .push(source_onset);
                        }
                    }
                    let previous_end = all
                        .iter()
                        .take(i)
                        .map(|p| audible_end_fn(p, world, score.tempo_bpm))
                        .fold(0.0, f64::max);
                    notes.push(SurfaceNote {
                        note: *n,
                        source_onset,
                        structural,
                        grid: grid_at(n.start_beat),
                        next_ioi_beats: ioi,
                        next_ioi_secs: ioi.map(|x| x * spb),
                        audible_secs: (audible_end - n.start_beat) * spb,
                        silence_before_secs: ((n.start_beat - previous_end) * spb).max(0.0),
                        silence_to_next_secs: next
                            .map(|x| ((x.start_beat - audible_end) * spb).max(0.0)),
                        accent: perf.accent.at_beat(n.start_beat),
                        kinetic: perf
                            .ensemble
                            .iter()
                            .find(|b| b.bar == (n.start_beat / 4.0).floor() as u32)
                            .map_or(0.5, |b| b.kinetic),
                    });
                }
                let intervals: Vec<_> = all
                    .iter()
                    .map(|n| {
                        (
                            n.start_beat,
                            audible_end(
                                n.start_beat,
                                f64::from(n.dur_beats),
                                patch(world, role),
                                score.tempo_bpm,
                            ),
                        )
                    })
                    .collect();
                let abrupt_ioi_changes = notes
                    .windows(2)
                    .filter_map(|w| {
                        let a = w[0].next_ioi_beats?;
                        let b = w[1].next_ioi_beats?;
                        (a.min(b) > 0.0 && a.max(b) / a.min(b) >= 4.0).then_some((
                            w[0].note.start_beat,
                            a,
                            b,
                        ))
                    })
                    .collect();
                let vacated_slots = notes
                    .iter()
                    .filter(|n| (n.source_onset - n.note.start_beat).abs() > 1e-6)
                    .map(|n| (n.source_onset, n.note.start_beat))
                    .collect();
                let isolated_grace_onsets = notes
                    .iter()
                    .filter(|n| {
                        !n.structural
                            && connective(n.note.function)
                            && n.next_ioi_secs.is_some_and(|latency| {
                                isolated_grace(
                                    n.silence_before_secs,
                                    f64::from(n.note.dur_beats) * spb,
                                    latency,
                                )
                            })
                    })
                    .map(|n| n.note.start_beat)
                    .collect();
                let mut repeated_shape_runs = Vec::new();
                let mut run = Vec::new();
                let mut previous_shape = None;
                for n in notes
                    .iter()
                    .filter(|n| !n.structural && connective(n.note.function))
                {
                    let shape = n
                        .next_ioi_secs
                        .map(|ioi| temporal_shape(ioi, f64::from(n.note.dur_beats) * spb));
                    if shape != previous_shape {
                        if run.len() >= 2 {
                            repeated_shape_runs.push(run.clone());
                        }
                        run.clear();
                    }
                    if shape.is_some() {
                        run.push(n.note.start_beat);
                    }
                    previous_shape = shape;
                }
                if run.len() >= 2 {
                    repeated_shape_runs.push(run);
                }
                let onsets = distinct_onsets(&notes);
                let density_spikes = onsets
                    .iter()
                    .filter_map(|&at| {
                        let count = onsets.iter().filter(|&&x| x >= at && x < at + 1.0).count();
                        (count >= 4).then_some((at, count))
                    })
                    .collect();
                let velocity_discontinuities = notes
                    .windows(2)
                    .filter(|w| (w[1].note.velocity - w[0].note.velocity).abs() >= 0.20)
                    .map(|w| (w[1].note.start_beat, w[0].note.velocity, w[1].note.velocity))
                    .collect();
                let attacks = onsets.len();
                out.roles.push(RoleSurface {
                    phrase: phrase.ix,
                    role,
                    start_beat: start,
                    end_beat: end,
                    discourse: plan.discourse.goal(phrase.ix as usize).role.label(),
                    notes,
                    shapes: shapes
                        .into_iter()
                        .map(|(shape, source_onsets)| ShapeOccurrences {
                            shape,
                            source_onsets,
                        })
                        .collect(),
                    attacks_per_beat: attacks as f64 / (end - start).max(f64::EPSILON),
                    audible_occupied_beats: occupied_beats(&intervals, start, end),
                    abrupt_ioi_changes,
                    vacated_slots,
                    isolated_grace_onsets,
                    repeated_shape_runs,
                    density_spikes,
                    velocity_discontinuities,
                });
            }
            for (a, b) in [(0, 1), (0, 2), (1, 2)] {
                let left = &out.roles[base + a];
                let right = &out.roles[base + b];
                let mut synchronized_onsets = Vec::new();
                for x in distinct_onsets(&left.notes) {
                    for y in distinct_onsets(&right.notes) {
                        if (x - y).abs() * spb <= 0.030 {
                            synchronized_onsets.push((x, y));
                        }
                    }
                }
                let mut synchronized_connective_shapes = Vec::new();
                for x in left
                    .notes
                    .iter()
                    .filter(|n| !n.structural && connective(n.note.function))
                {
                    for y in right
                        .notes
                        .iter()
                        .filter(|n| !n.structural && connective(n.note.function))
                    {
                        if let (Some(a), Some(b)) = (x.next_ioi_secs, y.next_ioi_secs) {
                            let shape = temporal_shape(a, f64::from(x.note.dur_beats) * spb);
                            if (x.note.start_beat - y.note.start_beat).abs() * spb <= 0.030
                                && shape == temporal_shape(b, f64::from(y.note.dur_beats) * spb)
                            {
                                synchronized_connective_shapes.push((
                                    x.note.start_beat,
                                    y.note.start_beat,
                                    shape,
                                ));
                            }
                        }
                    }
                }
                out.contacts.push(RoleContact {
                    phrase: phrase.ix,
                    roles: (left.role, right.role),
                    synchronized_onsets,
                    synchronized_connective_shapes,
                });
            }
        }
        out
    }

    /// Aggregate raw repetition counts across phrases for a role, retaining every source slot.
    pub fn shapes_for(&self, role: Role) -> Vec<ShapeOccurrences> {
        let mut shapes: BTreeMap<ConnectiveShape, Vec<f64>> = BTreeMap::new();
        for row in self.roles.iter().filter(|r| r.role == role) {
            for s in &row.shapes {
                shapes.entry(s.shape).or_default().extend(&s.source_onsets);
            }
        }
        shapes
            .into_iter()
            .map(|(shape, source_onsets)| ShapeOccurrences {
                shape,
                source_onsets,
            })
            .collect()
    }

    pub fn report(&self) -> String {
        let mut out = String::from("phrase surface: raw evidence, no quality scalar; shape bins=20ms, synchronization=30ms\n");
        for row in &self.roles {
            let _ = writeln!(out, "phrase={} role={} discourse={} span={:.3}..{:.3} attacks/beat={:.3} occupied={:.3}b shapes={:?} abrupt_iois={:?}", row.phrase, row.role.label(), row.discourse, row.start_beat, row.end_beat, row.attacks_per_beat, row.audible_occupied_beats, row.shapes, row.abrupt_ioi_changes);
            let _ = writeln!(out, "  vacated_slots={:?} isolated_graces={:?} repeated_shape_runs={:?} density_spikes={:?} velocity_discontinuities={:?}", row.vacated_slots, row.isolated_grace_onsets, row.repeated_shape_runs, row.density_spikes, row.velocity_discontinuities);
            for n in &row.notes {
                let _ = writeln!(out, "  silence_before_s={:.6}", n.silence_before_secs);
                let _ = writeln!(out, "  onset={:.6} source={:.6} pitch={} gate={:.6} velocity={:.6} structural={} grid={:?} ioi_b={:?} ioi_s={:?} audible_s={:.6} silence_s={:?} kinetic={:.3} accent={:?} provenance={:?}", n.note.start_beat, n.source_onset, n.note.pitch, n.note.dur_beats, n.note.velocity, n.structural, n.grid, n.next_ioi_beats, n.next_ioi_secs, n.audible_secs, n.silence_to_next_secs, n.kinetic, n.accent, n.note.prov);
            }
        }
        for c in &self.contacts {
            let _ = writeln!(
                out,
                "  synchronized_shapes={:?}",
                c.synchronized_connective_shapes
            );
            let _ = writeln!(
                out,
                "phrase={} contacts={:?} synchronized_onsets={:?}",
                c.phrase, c.roles, c.synchronized_onsets
            );
        }
        out
    }
}

fn distinct_onsets(notes: &[SurfaceNote]) -> Vec<f64> {
    let mut onsets: Vec<_> = notes.iter().map(|n| n.note.start_beat).collect();
    onsets.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    onsets
}

fn audible_end_fn(note: &Note, world: &MusicWorld, tempo: f32) -> f64 {
    audible_end(
        note.start_beat,
        f64::from(note.dur_beats),
        patch(world, note.role),
        tempo,
    )
}
