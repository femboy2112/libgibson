//! Independent bounded temporal pitch audit (Round XII).
//!
//! Local palette membership, instantaneous ensemble syntax and temporal ownership are separate
//! facts. This observer never calls the production pitch classifier. It reconstructs paths from
//! written notes, so a function stamp cannot supply its own evidence. Orphans are listening
//! suspects, not forbidden pitches. The horizon is the current harmony and two structural changes.
//! Structural here means an integer beat, a duration of at least one beat, or a material endpoint;
//! it is an explicitly documented proxy, not a recovered composer's intention.

use super::action::ActionKind;
use super::context::HarmonicContext;
use super::ids::MaterialId;
use super::performance::PerformancePlan;
use super::score::{Note, PitchFunction as F, Role, Score};
use super::sonority::core_pcs;
use super::theory::{note_name, pitch_class};
use std::fmt::Write;

const EPS: f64 = 1e-6;
const RESOLUTION_GAP: f64 = 1.0;

/// A path verdict, independently of the declared local function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStatus {
    Stable,
    Resolved,
    Pending,
    Orphaned,
    Broken,
}

/// Why an available extension has a place in the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionPath {
    RestingColor,
    CommonTone,
    ForwardLeading,
    OwnedTendency,
    OrphanColor,
}

/// One note's inspectable proof. Indices refer to the audited score and performance.
#[derive(Debug, Clone, PartialEq)]
pub struct PitchTrajectoryProof {
    pub note_index: usize,
    pub supported: Vec<F>,
    pub status: PathStatus,
    pub extension: Option<ExtensionPath>,
    pub structural: bool,
    pub previous_note: Option<usize>,
    pub next_note: Option<usize>,
    pub context: Option<usize>,
    pub previous_context: Option<usize>,
    pub next_context: Option<usize>,
    pub destination: Option<usize>,
    pub reasons: Vec<String>,
    pub suspect: bool,
    /// Ambiguous polyphony cannot certify a melodic resolution.
    pub ambiguous_voice: bool,
    /// Complete contextual receipt, including the declared function and L2 scope.
    pub detail: String,
}

/// Counts are different kinds of evidence, never one musical quality score.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TemporalPitchDiagnostics {
    pub false_function_claims: usize,
    pub false_suspensions: usize,
    pub broken_anticipations: usize,
    pub orphan_extensions: usize,
    pub orphan_structural_extensions: usize,
    pub unresolved_tendencies: usize,
    pub bad_arrivals: usize,
    pub guide_path_misses: usize,
    pub path_breaks_at_harmony_changes: usize,
    pub locally_legal_globally_unexplained: usize,
    pub ambiguous_voices: usize,
    pub rows: Vec<PitchTrajectoryProof>,
}

fn end(n: &Note) -> f64 {
    n.start_beat + f64::from(n.dur_beats)
}
fn core(c: &HarmonicContext, p: i32) -> bool {
    core_pcs(&c.chord).contains(&pitch_class(p))
}
fn chord(c: &HarmonicContext, p: i32) -> bool {
    c.chord.contains_pc(pitch_class(p))
}
fn step(a: i32, b: i32) -> bool {
    (1..=2).contains(&(a - b).abs())
}
fn context_index(perf: &PerformancePlan, beat: f64) -> Option<usize> {
    perf.contexts.iter().position(|c| {
        beat >= c.start_beat - EPS && beat < c.start_beat + f64::from(c.dur_beats) - EPS
    })
}
fn same_structure(a: &HarmonicContext, b: &HarmonicContext) -> bool {
    // Seventh/sixth/ninth colour changes remain immediate contexts, but do not consume
    // the bounded I-IV-I / ii-V-I structural horizon. Altered triads do consume it.
    let triad = |c: &HarmonicContext| {
        c.chord
            .quality
            .intervals()
            .iter()
            .take(3)
            .copied()
            .collect::<Vec<_>>()
    };
    a.chord.root_pc == b.chord.root_pc && triad(a) == triad(b)
}
fn next_structure(perf: &PerformancePlan, i: usize) -> Option<usize> {
    (i + 1..perf.contexts.len()).find(|&j| !same_structure(&perf.contexts[i], &perf.contexts[j]))
}

// Material identity partitions independent figures. Ordinary mono playing remains one voice;
// its next event, not an arbitrary later convenient event, must supply a claimed resolution.
fn same_material(a: &Note, b: &Note) -> bool {
    a.role == b.role && a.prov.material == b.prov.material
}

fn same_line(a: &Note, b: &Note) -> bool {
    a.role == b.role && (matches!(a.role, Role::Lead | Role::Bass) || same_material(a, b))
}

/// Neighbour edges from onset bundles. Equal-cardinality polyphonic bundles pair in register
/// order, one to one; changing cardinality is explicitly ambiguous and supplies no path proof.
fn neighbors(score: &Score) -> (Vec<Option<usize>>, Vec<Option<usize>>, Vec<bool>) {
    let len = score.notes.len();
    let mut prev = vec![None; len];
    let mut next = vec![None; len];
    let mut ambiguous = vec![false; len];
    let mut handled = vec![false; len];
    for i in 0..len {
        if handled[i] {
            continue;
        }
        let mut line: Vec<usize> = (0..len)
            .filter(|&j| same_line(&score.notes[i], &score.notes[j]))
            .collect();
        line.sort_by(|&a, &b| {
            score.notes[a]
                .start_beat
                .total_cmp(&score.notes[b].start_beat)
                .then(score.notes[a].pitch.cmp(&score.notes[b].pitch))
        });
        let mut bundles: Vec<Vec<usize>> = Vec::new();
        for j in line {
            handled[j] = true;
            if bundles.last().is_some_and(|b| {
                (score.notes[b[0]].start_beat - score.notes[j].start_beat).abs() < EPS
            }) {
                bundles.last_mut().unwrap().push(j);
            } else {
                bundles.push(vec![j]);
            }
        }
        for pair in bundles.windows(2) {
            if pair[0].len() != pair[1].len() {
                for &j in pair.iter().flatten() {
                    ambiguous[j] = true;
                }
                continue;
            }
            for (&a, &b) in pair[0].iter().zip(&pair[1]) {
                if score.notes[b].start_beat - end(&score.notes[a]) <= RESOLUTION_GAP + EPS {
                    next[a] = Some(b);
                    prev[b] = Some(a);
                }
            }
        }
    }
    (prev, next, ambiguous)
}

impl TemporalPitchDiagnostics {
    /// Audit every written pitched note. SFX already have separately audited action ownership;
    /// release tails remain the audible sonority diagnostic's responsibility.
    pub fn measure(perf: &PerformancePlan, score: &Score) -> Self {
        Self::measure_with_selected_colors(perf, score, &[])
    }

    /// Explicit authored open endpoint selections, keyed by material and pitch class. This
    /// evidence must originate in a phrase contract, never in a scan of the realized notes.
    pub fn measure_with_selected_colors(
        perf: &PerformancePlan,
        score: &Score,
        selected: &[(MaterialId, i32)],
    ) -> Self {
        let mut out = Self::default();
        let (previous, following, ambiguous) = neighbors(score);
        for (i, n) in score.notes.iter().enumerate() {
            let ci = context_index(perf, n.start_beat);
            let pi = previous[i];
            let ni = following[i];
            let p = pi.map(|j| &score.notes[j]);
            let q = ni.map(|j| &score.notes[j]);
            let mut row = PitchTrajectoryProof {
                note_index: i,
                supported: Vec::new(),
                status: PathStatus::Broken,
                extension: None,
                structural: (n.start_beat - n.start_beat.round()).abs() < EPS || n.dur_beats >= 1.0,
                previous_note: pi,
                next_note: ni,
                context: ci,
                previous_context: None,
                next_context: None,
                destination: None,
                reasons: Vec::new(),
                suspect: false,
                ambiguous_voice: ambiguous[i],
                detail: String::new(),
            };
            let material_last = n.prov.material.is_some()
                && !score
                    .notes
                    .iter()
                    .any(|x| same_material(n, x) && x.start_beat > n.start_beat + EPS);
            row.structural |= material_last;
            if let Some(ci) = ci {
                let c = &perf.contexts[ci];
                row.previous_context = (0..ci)
                    .rev()
                    .find(|&j| !same_structure(c, &perf.contexts[j]));
                row.next_context = next_structure(perf, ci);
                row.destination = row.next_context.and_then(|j| next_structure(perf, j));
                let immediate = perf.contexts.get(ci + 1);
                let qc = q
                    .and_then(|x| context_index(perf, x.start_beat))
                    .map(|j| &perf.contexts[j]);
                let target = q.zip(qc).is_some_and(|(x, cx)| core(cx, x.pitch));
                let step_target = q.is_some_and(|x| step(n.pitch, x.pitch)) && target;
                let stable_target = q.zip(qc).is_some_and(|(x, cx)| {
                    chord(cx, x.pitch) || cx.palette.tensions.contains(&pitch_class(x.pitch))
                });
                let is_open = matches!(
                    n.prov.closure,
                    Some("open" | "half" | "deferred" | "evaporate")
                );
                let selected_open = is_open
                    && n.prov.material.is_some_and(|m| {
                        selected
                            .iter()
                            .any(|&(id, pc)| id == m && pc.rem_euclid(12) == pitch_class(n.pitch))
                    });
                let explicit_closed = (n.prov.role_note == "answer" && !is_open)
                    || perf
                        .actions
                        .actions
                        .iter()
                        .any(|a| a.kind == ActionKind::Resolve && n.prov.actions.has(a.id));
                let actual = score.chords.iter().find(|s| {
                    n.start_beat >= s.start_beat - EPS
                        && n.start_beat < s.start_beat + f64::from(s.dur_beats) - EPS
                });
                let context_true = actual.is_some_and(|s| s.chord == c.chord);
                // A held event can own a suspension even though its onset was consonant.
                let held = immediate.is_some_and(|nc| {
                    n.start_beat < nc.start_beat - EPS
                        && end(n) > nc.start_beat + EPS
                        && chord(c, n.pitch)
                        && !chord(nc, n.pitch)
                        && q.is_some_and(|x| {
                            x.start_beat >= nc.start_beat - EPS
                                && x.start_beat >= end(n) - EPS
                                && chord(nc, x.pitch)
                                && step(n.pitch, x.pitch)
                        })
                });
                let suspension = held && q.is_some_and(|x| x.pitch < n.pitch);
                let retardation = held && q.is_some_and(|x| x.pitch > n.pitch);
                let illegal_sustain = perf.contexts.iter().any(|cx| {
                    cx.start_beat > n.start_beat + EPS
                        && cx.start_beat < end(n) - EPS
                        && !chord(cx, n.pitch)
                }) && !held;
                if chord(c, n.pitch) && !illegal_sustain {
                    row.supported.push(F::ChordTone);
                }
                if c.palette.tensions.contains(&pitch_class(n.pitch)) && !illegal_sustain {
                    row.supported.push(F::LicensedExtension);
                }
                if suspension {
                    row.supported.push(F::Suspension);
                }
                if retardation {
                    row.supported.push(F::Retardation);
                }
                let anticipation = immediate.is_some_and(|nc| {
                    let close = nc.start_beat - n.start_beat;
                    let arrives = score
                        .chords
                        .iter()
                        .any(|s| (s.start_beat - nc.start_beat).abs() < EPS && s.chord == nc.chord);
                    let connected = end(n) >= nc.start_beat - EPS
                        || q.is_some_and(|x| {
                            x.start_beat >= nc.start_beat - EPS
                                && x.start_beat <= nc.start_beat + 1.0
                                && (x.pitch == n.pitch || step(x.pitch, n.pitch))
                                && chord(nc, x.pitch)
                        });
                    close > EPS && close <= 1.0 + EPS && chord(nc, n.pitch) && arrives && connected
                });
                if anticipation {
                    row.supported.push(F::Anticipation);
                }
                if let Some(q) = q {
                    if (q.pitch - n.pitch).abs() == 1 && stable_target {
                        row.supported.push(F::ChromaticApproach);
                    }
                    if let Some(p) = p {
                        let a = n.pitch - p.pitch;
                        let b = q.pitch - n.pitch;
                        if p.pitch == q.pitch && step(p.pitch, n.pitch) && stable_target {
                            row.supported.push(F::Neighbor);
                        }
                        let pc = context_index(perf, p.start_beat).map(|j| &perf.contexts[j]);
                        if step(p.pitch, n.pitch)
                            && step(n.pitch, q.pitch)
                            && a.signum() == b.signum()
                            && stable_target
                            && pc.is_some_and(|x| {
                                chord(x, p.pitch)
                                    || x.palette.tensions.contains(&pitch_class(p.pitch))
                            })
                        {
                            row.supported.push(
                                if c.palette.scale.contains_pc(pitch_class(n.pitch)) {
                                    F::DiatonicPassing
                                } else {
                                    F::ChromaticPassing
                                },
                            );
                        }
                        if (n.start_beat - n.start_beat.round()).abs() < EPS
                            && a.abs() > 2
                            && step_target
                        {
                            row.supported.push(F::Appoggiatura);
                        }
                        if (p.pitch - q.pitch).signum() == -(n.pitch - q.pitch).signum()
                            && step(p.pitch, q.pitch)
                            && step(n.pitch, q.pitch)
                            && stable_target
                        {
                            row.supported.push(F::Enclosure);
                        }
                    }
                }
                // Pedal requires an actual repeated/held pitch established as a prior root.
                if n.role == Role::Bass
                    && (chord(c, n.pitch)
                        || p.is_some_and(|x| {
                            x.pitch == n.pitch
                                && context_index(perf, x.start_beat).is_some_and(|j| {
                                    perf.contexts[j].chord.root_pc == pitch_class(x.pitch)
                                })
                        }))
                {
                    row.supported.push(F::PedalTone);
                }
                // Slide is an observed monotonic chromatic chain ending in a written chord tone.
                let mut at = i;
                let mut direction = 0;
                let mut links = 0;
                let mut slide = false;
                while let Some(j) = following[at] {
                    let a = &score.notes[at];
                    let b = &score.notes[j];
                    let d = b.pitch - a.pitch;
                    if d.abs() > 2
                        || d == 0
                        || (direction != 0 && direction != d.signum())
                        || b.start_beat > n.start_beat + 2.0
                    {
                        break;
                    }
                    direction = d.signum();
                    links += 1;
                    if context_index(perf, b.start_beat)
                        .is_some_and(|k| chord(&perf.contexts[k], b.pitch))
                    {
                        slide = true;
                        break;
                    }
                    at = j;
                }
                if slide && links > 0 {
                    row.supported.push(F::SlidePath);
                }
                if !context_true {
                    row.supported.clear();
                    row.reasons
                        .push("score harmony disagrees with performance context".into());
                }
                let extension =
                    c.palette.tensions.contains(&pitch_class(n.pitch)) && !chord(c, n.pitch);
                if extension {
                    let common = immediate.is_some_and(|nc| {
                        chord(nc, n.pitch)
                            && (end(n) > nc.start_beat + EPS
                                || q.is_some_and(|x| {
                                    x.pitch == n.pitch
                                        && x.start_beat >= nc.start_beat - EPS
                                        && x.start_beat <= nc.start_beat + 1.0
                                }))
                    });
                    let forward = step_target
                        && q.is_some_and(|x| context_index(perf, x.start_beat) != Some(ci));
                    row.extension = Some(if common {
                        ExtensionPath::CommonTone
                    } else if forward {
                        ExtensionPath::ForwardLeading
                    } else if step_target {
                        ExtensionPath::OwnedTendency
                    } else if selected_open && material_last {
                        ExtensionPath::RestingColor
                    } else {
                        ExtensionPath::OrphanColor
                    });
                    if row.extension == Some(ExtensionPath::OrphanColor) {
                        out.orphan_extensions += 1;
                        out.orphan_structural_extensions += usize::from(row.structural);
                        row.reasons.push("available extension has no retained common tone, bounded core resolution, or explicit open endpoint".into());
                    }
                }
                if material_last && explicit_closed && !chord(c, n.pitch) {
                    out.bad_arrivals += 1;
                    row.reasons.push(
                        "closed material endpoint does not earn an arrival: no written chord destination"
                            .into(),
                    );
                }
                if illegal_sustain {
                    out.path_breaks_at_harmony_changes += 1;
                    row.reasons.push(
                        "held note crosses into a nonmember harmony without a held resolution"
                            .into(),
                    );
                }
                let boundary_jump = q.zip(qc).is_some_and(|(x, cx)| {
                    !same_structure(c, cx)
                        && (x.pitch - n.pitch).abs() > 5
                        && !chord(c, n.pitch)
                        && !step_target
                });
                if boundary_jump {
                    out.path_breaks_at_harmony_changes += 1;
                    row.reasons.push("extension jumps across a structural harmony without common-tone or step continuation".into());
                }
                if row.structural
                    && extension
                    && row.extension == Some(ExtensionPath::OrphanColor)
                    && immediate.is_some_and(|nc| nc.start_beat - n.start_beat <= 2.0 + EPS)
                {
                    out.guide_path_misses += 1;
                    row.reasons.push("structural color approaches harmony change without observed guide/core path".into());
                }
                let mandatory = matches!(
                    n.function,
                    Some(
                        F::Suspension
                            | F::Retardation
                            | F::Appoggiatura
                            | F::Anticipation
                            | F::ChromaticApproach
                            | F::DiatonicPassing
                            | F::ChromaticPassing
                            | F::Neighbor
                            | F::Enclosure
                            | F::SlidePath
                    )
                );
                let claimed = n.function.is_some_and(|f| row.supported.contains(&f));
                if mandatory && !claimed {
                    out.unresolved_tendencies += 1;
                    row.reasons
                        .push("declared connective/tendency has no complete observed path".into());
                }
                row.status = if !claimed || illegal_sustain {
                    PathStatus::Broken
                } else if row.extension == Some(ExtensionPath::OrphanColor) || boundary_jump {
                    PathStatus::Orphaned
                } else if mandatory
                    || matches!(
                        row.extension,
                        Some(
                            ExtensionPath::OwnedTendency
                                | ExtensionPath::ForwardLeading
                                | ExtensionPath::CommonTone
                        )
                    )
                {
                    PathStatus::Resolved
                } else {
                    PathStatus::Stable
                };
                if row.supported.is_empty() {
                    row.reasons.push("no reconstructed local function".into());
                }
            } else {
                row.reasons.push("no harmonic context at onset".into());
            }
            let false_claim = n.function.is_some_and(|f| !row.supported.contains(&f));
            out.false_function_claims += usize::from(false_claim);
            out.false_suspensions += usize::from(
                matches!(n.function, Some(F::Suspension | F::Retardation)) && false_claim,
            );
            out.broken_anticipations +=
                usize::from(n.function == Some(F::Anticipation) && false_claim);
            if false_claim {
                row.reasons.push(format!(
                    "declared {:?} lacks independently reconstructed evidence",
                    n.function
                ));
            }
            row.suspect = !row.reasons.is_empty();
            out.locally_legal_globally_unexplained += usize::from(
                !row.supported.is_empty()
                    && matches!(row.status, PathStatus::Orphaned | PathStatus::Broken),
            );
            out.ambiguous_voices += usize::from(row.ambiguous_voice);
            let harmony = |j: Option<usize>| {
                j.map(|j| {
                    format!(
                        "{} @ {:.2}",
                        perf.contexts[j].chord.label(),
                        perf.contexts[j].start_beat
                    )
                })
                .unwrap_or_else(|| "none".into())
            };
            let event = |j: Option<usize>| {
                j.map(|j| {
                    format!(
                        "{} @ {:.2} dur {:.2}",
                        note_name(score.notes[j].pitch),
                        score.notes[j].start_beat,
                        score.notes[j].dur_beats
                    )
                })
                .unwrap_or_else(|| "none".into())
            };
            let immediate = ci
                .and_then(|j| perf.contexts.get(j + 1))
                .map(|c| format!("{} @ {:.2}", c.chord.label(), c.start_beat))
                .unwrap_or_else(|| "none".into());
            row.detail=format!("beat {:.4} role {} pitch {} ({}) duration {:.4}\n  current {} previous {} next {} next_structural {} destination {}\n  declared {:?}; local {:?}; vertical: separate sonority layer (not inferred from path); path {:?} {:?}\n  previous note {}; next note {}; structural_proxy={} ambiguous_voice={}\n  reason: {}\n",n.start_beat,n.role.label(),note_name(n.pitch),n.pitch,n.dur_beats,harmony(ci),harmony(row.previous_context),immediate,harmony(row.next_context),harmony(row.destination),n.function,row.supported,row.status,row.extension,event(pi),event(ni),row.structural,row.ambiguous_voice,if row.reasons.is_empty(){"observed path supports the claim".into()}else{row.reasons.join("; ")});
            if let Some(ci) = ci {
                if let Some(next) = perf.contexts.get(ci + 1) {
                    let _ = writeln!(
                        row.detail,
                        "  next guide targets: {:?}",
                        next.palette.guide_tones
                    );
                }
            }
            out.rows.push(row);
        }
        // A connector may truthfully approach an available colour while that colour's path
        // is unexplained. Preserve the local claim and propagate the soft temporal suspicion.
        let dependent: Vec<usize> = out
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let local_connector = matches!(
                    score.notes[i].function,
                    Some(
                        F::ChromaticApproach
                            | F::DiatonicPassing
                            | F::ChromaticPassing
                            | F::Neighbor
                            | F::Enclosure
                    )
                );
                (local_connector
                    && matches!(r.status, PathStatus::Stable | PathStatus::Resolved)
                    && r.next_note
                        .is_some_and(|j| out.rows[j].extension == Some(ExtensionPath::OrphanColor)))
                .then_some(i)
            })
            .collect();
        for i in dependent {
            let r = &mut out.rows[i];
            r.detail = r
                .detail
                .replace(&format!("path {:?}", r.status), "path Orphaned");
            r.status = PathStatus::Orphaned;
            r.suspect = true;
            r.reasons.push(
                "locally valid connector reaches an extension whose own path is orphaned".into(),
            );
            r.detail = r.detail.replace(
                "observed path supports the claim",
                "local claim is supported; destination extension has no owned path",
            );
            out.locally_legal_globally_unexplained += 1;
        }
        // Level 2 uses the existing ensemble theory with independently reconstructed labels
        // and this observer's voice edges. No lead-derived sonority colour plan is consulted.
        let mut voices: Vec<super::sonority::Voice> = score
            .notes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let proof = &out.rows[i];
                let f = n
                    .function
                    .filter(|f| proof.supported.contains(f))
                    .or_else(|| proof.supported.first().copied());
                let to = proof
                    .next_note
                    .map(|j| score.notes[j].pitch)
                    .filter(|&p| step(n.pitch, p));
                super::sonority::Voice {
                    role: n.role,
                    pitch: n.pitch,
                    start: n.start_beat,
                    end: end(n),
                    function: f,
                    tag: n.prov.role_note,
                    resolves: matches!(proof.status, PathStatus::Stable | PathStatus::Resolved),
                    resolves_to: to,
                    unison: n.prov.role_note == "unison" || n.prov.motif_xform == Some("unison"),
                    written_end: end(n),
                    sfx: false,
                    owned: false,
                }
            })
            .collect();
        let bps = f64::from(score.tempo_bpm.max(1.0)) / 60.0;
        for e in score.sfx.iter().filter(|e| e.is_pitched()) {
            let (a, d, _, _) = e.kind.envelope();
            let stop = e.start_beat + f64::from(a + d + e.kind.hold_secs()) * bps;
            for k in 0..2 {
                voices.push(super::sonority::sfx_voice(
                    e.pitches[k],
                    e.function[k],
                    e.start_beat,
                    stop,
                    e.owned_by.is_some(),
                ));
            }
        }
        let slices = super::sonority::slices(
            &voices,
            &perf.contexts,
            &super::sonority::ColorPolicy::lenient(),
            &[],
        );
        for row in &mut out.rows {
            let classes: std::collections::BTreeSet<&str> = slices
                .iter()
                .filter(|s| s.sounding.contains(&row.note_index))
                .map(|s| s.class.label())
                .collect();
            row.detail = row.detail.replace(
                "separate sonority layer (not inferred from path)",
                &format!(
                    "{:?} (written ensemble slices; world-free lenient color budget)",
                    classes
                ),
            );
        }
        out
    }

    /// Counts followed by every suspicious note's independently reconstructed evidence.
    pub fn report(&self) -> String {
        let mut s = String::new();
        for (name, value) in [
            ("false_function_claims", self.false_function_claims),
            ("false_suspensions", self.false_suspensions),
            ("broken_anticipations", self.broken_anticipations),
            ("orphan_extensions", self.orphan_extensions),
            (
                "orphan_structural_extensions",
                self.orphan_structural_extensions,
            ),
            ("unresolved_tendencies", self.unresolved_tendencies),
            ("bad_arrivals", self.bad_arrivals),
            ("guide_path_misses", self.guide_path_misses),
            (
                "path_breaks_at_harmony_changes",
                self.path_breaks_at_harmony_changes,
            ),
            (
                "locally_legal_globally_unexplained",
                self.locally_legal_globally_unexplained,
            ),
            ("ambiguous_voices", self.ambiguous_voices),
        ] {
            let _ = writeln!(s, "{name}={value}");
        }
        for row in self.rows.iter().filter(|r| r.suspect) {
            s.push_str(&row.detail);
        }
        s
    }
}
