//! Round XIII temporal mass: how much perceptual responsibility one pitched event carries.
//!
//! A pitch path is parameterized by time, `(p0,t0) → (p1,t1) → (p2,t2)`, and its pitch-function
//! semantics are not invariant under reparameterization. A chromatic intermediate played fast is
//! a member of one gesture; the same pitch dwelt on is heard as its own statement; a colour
//! struck together with the harmony's own attack fuses into the chord that attack declares.
//!
//! [`TemporalMass`] is evidence, never a quality score: a heavy tonic, a heavy suspension and a
//! heavy unowned colour weigh the same. What is judged is mass against ownership
//! ([`MassDiagnostics`]). The overlay reads the unchanged Round XII observer's independently
//! reconstructed support (it never trusts a declared function) and adds time: written seconds at
//! the score's tempo, audible seconds from the world's patch envelope (Round VIII
//! [`audible_end`]), metric accent, onset fusion with the harmonic articulation, and recent
//! recurrence of the same colour in the same role.
//!
//! Mathematical invariants (tested): mass is non-decreasing in audible seconds, recurrence and
//! metric accent; audible seconds are non-decreasing in written seconds; the exposure class is
//! non-decreasing in mass and in fusion. Calibrated heuristics (named below, each with a witness):
//! the two class bands, the recall window, the salience ladder and the fusion tolerance.
//! Velocity is recorded but not weighted: loudness across roles depends on mix gains this model
//! deliberately does not simulate, and the acceptance holds are no louder than their stabs.

use super::context::HarmonicContext;
use super::instrument::Patch;
use super::performance::PerformancePlan;
use super::score::{Note, PitchFunction as F, Role, Score};
use super::sonority::audible_end;
use super::temporal::{ExtensionPath, PitchTrajectoryProof, TemporalPitchDiagnostics};
use super::theory::{note_name, pitch_class};
use super::world::MusicWorld;
use std::fmt::Write;

const EPS: f64 = 1e-6;

/// Salience-weighted seconds below which an event sounds at ornament or run rate (five or more
/// events a second): a member of the gesture that carries it, not a pitch evaluated alone.
/// Calibrated heuristic. Witness: a half-beat weak event is Fleeting at 160 BPM (0.19 s) and
/// Gestural at 118 BPM (0.25 s).
pub const FLEETING_MAX_SECS: f64 = 0.20;

/// Salience-weighted seconds at or above which a pitch is heard as a harmonic statement.
/// Calibrated heuristic: above a weak-beat quarter note at 88 BPM (0.68 s — the walking-bass
/// approach idiom stays gestural at BLACK_ICE's tempo) and below a downbeat quarter at 118 BPM
/// (0.51 s × 1.5 = 0.76 s). Witness: a one-beat mid-bar event is Gestural at 118 BPM (0.64) and
/// Asserted at 88 BPM (0.85).
pub const ASSERTED_MIN_SECS: f64 = 0.75;

// The bands are ordered: nothing is asserted before it has left the gesture.
const _: () = assert!(FLEETING_MAX_SECS < ASSERTED_MIN_SECS);

/// Seconds within which an earlier attack of the same pitch class in the same role is still
/// heard as the same colour (the order of echoic memory). Calibrated heuristic. Witness: four
/// offbeat keys pings of one colour inside the window weigh four times one ping.
pub const RECALL_WINDOW_SECS: f64 = 2.0;

/// Onset asynchrony (seconds) within which a support note is attacked WITH its harmony's
/// articulation and fuses into the chord that attack declares (the order of the asynchrony below
/// which simultaneous attacks are heard as one onset). The engine's onsets are grid-exact.
/// Witness: the SWISS keys D5 at the C6 arrival (beat 32) fuses; the D5 stab at 30.75 does not.
pub const FUSION_ONSET_SECS: f64 = 0.03;

/// Metric salience (dimensionless multiplier on exposure) of a bar downbeat.
pub const DOWNBEAT_SALIENCE: f64 = 1.5;
/// Metric salience of the secondary strong beat in an even meter (beat 3 of 4/4).
pub const MIDBAR_SALIENCE: f64 = 1.25;

/// The metric position of an onset: the standard strong/secondary/weak hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Accent {
    Weak,
    MidBar,
    Downbeat,
}

impl Accent {
    /// The accent of `beat` in a bar of `beats_per_bar`.
    pub fn of(beat: f64, beats_per_bar: f64) -> Accent {
        let bpb = beats_per_bar.max(1.0);
        let pos = beat.rem_euclid(bpb);
        let at = |x: f64| (pos - x).abs() < EPS || (pos - bpb - x).abs() < EPS;
        if at(0.0) {
            Accent::Downbeat
        } else if (bpb.round() as i64) % 2 == 0 && bpb >= 4.0 && at(bpb / 2.0) {
            Accent::MidBar
        } else {
            Accent::Weak
        }
    }

    /// The accent's exposure multiplier.
    pub fn salience(self) -> f64 {
        match self {
            Accent::Weak => 1.0,
            Accent::MidBar => MIDBAR_SALIENCE,
            Accent::Downbeat => DOWNBEAT_SALIENCE,
        }
    }
}

/// How much explanation an event owes, coarsely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExposureClass {
    /// Heard as part of a moving gesture: path membership may be sufficient.
    Fleeting,
    /// Heard as a pitch, inside a line: requires a clear local destination or relation.
    Gestural,
    /// Heard as a harmonic statement: requires explicit harmonic or phrase-level ownership.
    Asserted,
}

impl ExposureClass {
    /// The class of `mass` (weighted seconds); onset fusion asserts a support colour into the
    /// chord it is attacked with, whatever its dwell.
    pub fn of(mass: f64, fused: bool) -> ExposureClass {
        if fused || mass >= ASSERTED_MIN_SECS {
            ExposureClass::Asserted
        } else if mass >= FLEETING_MAX_SECS {
            ExposureClass::Gestural
        } else {
            ExposureClass::Fleeting
        }
    }
}

/// One pitched event's perceptual exposure. Units: beats, seconds, weighted seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemporalMass {
    pub role: Role,
    pub written_beats: f64,
    pub written_secs: f64,
    /// Seconds from onset until the patch envelope falls 30 dB below its peak, or until the same
    /// role's next attack at or after the written end masks the tail.
    pub audible_secs: f64,
    /// Audible seconds of earlier same-role, same-pitch-class attacks inside
    /// [`RECALL_WINDOW_SECS`] before the onset.
    pub recurrence_secs: f64,
    pub accent: Accent,
    /// A support voice (pad, keys, bass) attacked with its harmony's articulation.
    pub fused: bool,
    pub velocity: f32,
    /// `audible_secs + recurrence_secs`.
    pub exposure_secs: f64,
    /// The accent's salience.
    pub salience: f64,
    /// `exposure_secs × salience` (weighted seconds).
    pub mass: f64,
    pub class: ExposureClass,
}

impl TemporalMass {
    /// Compose the mass from its evidence (pure).
    #[allow(clippy::too_many_arguments)]
    pub fn from_evidence(
        role: Role,
        written_beats: f64,
        written_secs: f64,
        audible_secs: f64,
        recurrence_secs: f64,
        accent: Accent,
        fused: bool,
        velocity: f32,
    ) -> TemporalMass {
        let exposure_secs = audible_secs.max(0.0) + recurrence_secs.max(0.0);
        let salience = accent.salience();
        let mass = exposure_secs * salience;
        TemporalMass {
            role,
            written_beats,
            written_secs,
            audible_secs,
            recurrence_secs,
            accent,
            fused,
            velocity,
            exposure_secs,
            salience,
            mass,
            class: ExposureClass::of(mass, fused),
        }
    }

    /// The evidence for every note of `notes` at `tempo_bpm` under `world`'s patches. Every note
    /// of a role takes part in that role's masking and recurrence, whatever its material.
    pub fn of_notes(
        notes: &[Note],
        contexts: &[HarmonicContext],
        tempo_bpm: f32,
        beats_per_bar: f64,
        world: &MusicWorld,
    ) -> Vec<TemporalMass> {
        let spb = 60.0 / f64::from(tempo_bpm.max(1.0));
        let dwell_end: Vec<f64> = notes
            .iter()
            .map(|n| {
                let written_end = n.start_beat + f64::from(n.dur_beats);
                let heard = audible_end(
                    n.start_beat,
                    f64::from(n.dur_beats),
                    patch(world, n.role),
                    tempo_bpm,
                );
                notes
                    .iter()
                    .filter(|m| {
                        m.role == n.role
                            && m.start_beat >= written_end - EPS
                            && m.start_beat > n.start_beat + EPS
                    })
                    .map(|m| m.start_beat)
                    .fold(heard, f64::min)
                    .max(n.start_beat)
            })
            .collect();
        let window = RECALL_WINDOW_SECS / spb;
        notes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let recurrence = notes
                    .iter()
                    .enumerate()
                    .filter(|&(j, m)| {
                        j != i
                            && m.role == n.role
                            && pitch_class(m.pitch) == pitch_class(n.pitch)
                            && m.start_beat < n.start_beat - EPS
                    })
                    .map(|(j, m)| {
                        (dwell_end[j].min(n.start_beat) - m.start_beat.max(n.start_beat - window))
                            .max(0.0)
                    })
                    .sum::<f64>()
                    * spb;
                // Never a signed zero in a receipt.
                let recurrence = if recurrence > 0.0 { recurrence } else { 0.0 };
                let fused = n.role != Role::Lead
                    && contexts
                        .iter()
                        .any(|c| (c.start_beat - n.start_beat).abs() * spb <= FUSION_ONSET_SECS);
                TemporalMass::from_evidence(
                    n.role,
                    f64::from(n.dur_beats),
                    f64::from(n.dur_beats) * spb,
                    (dwell_end[i] - n.start_beat) * spb,
                    recurrence,
                    Accent::of(n.start_beat, beats_per_bar),
                    fused,
                    n.velocity,
                )
            })
            .collect()
    }

    /// One line of inspectable evidence.
    pub fn evidence(&self) -> String {
        format!(
            "written {:.3} beats = {:.3} s; audible {:.3} s; recurrence {:.3} s; accent {:?} (x{:.2}); fused {}; velocity {:.3}; mass {:.3} weighted-s -> {:?}",
            self.written_beats,
            self.written_secs,
            self.audible_secs,
            self.recurrence_secs,
            self.accent,
            self.salience,
            if self.fused { "yes" } else { "no" },
            self.velocity,
            self.mass,
            self.class
        )
    }
}

/// The world patch that sounds `role`.
pub fn patch(world: &MusicWorld, role: Role) -> &Patch {
    match role {
        Role::Pad => &world.pad,
        Role::Keys => &world.keys,
        Role::Bass => &world.bass,
        Role::Lead => &world.lead,
    }
}

/// The strongest reason the independent observer found for a pitch, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ownership {
    /// No reconstructed local function.
    None,
    /// Only local palette licence: an available tension with no observed path.
    Licensed,
    /// Gesture membership: a slide link, or a connector into a non-structural destination.
    Path,
    /// A local destination: a connector into a written chord tone, or a colour retained as a
    /// common tone / resolving by step into a core tone.
    Local,
    /// Explicit harmonic or phrase-level ownership: written in the sounding harmony, a physical
    /// suspension/retardation, an appoggiatura, an anticipation, a pedal, or a selected colour.
    Harmonic,
}

/// Mass judged against ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MassVerdict {
    /// The event's reason is as strong as its exposure demands.
    Carried,
    /// A gestural orphan colour: Round XII's soft suspect, not asserted, not a defect.
    SoftOrphan,
    /// An asserted colour the sounding harmony does not write and no contract owns. In a support
    /// role it is an accidental reharmonization of the song.
    UnownedAssertedColor,
    /// A non-harmonic tone heard longer than its gesture or connector membership can carry.
    OverdrawnGesture,
    /// No reconstructed function at all (Round XII's own receipts cover these).
    Unsupported,
}

/// The ownership of an independently reconstructed row.
pub fn ownership_of(
    row: &PitchTrajectoryProof,
    score: &Score,
    perf: &PerformancePlan,
) -> Ownership {
    let s = &row.supported;
    let harmonic = s.iter().any(|f| {
        matches!(
            f,
            F::ChordTone
                | F::Suspension
                | F::Retardation
                | F::Appoggiatura
                | F::Anticipation
                | F::PedalTone
        )
    }) || row.extension == Some(ExtensionPath::RestingColor);
    if harmonic {
        return Ownership::Harmonic;
    }
    let connector = s.iter().any(|f| {
        matches!(
            f,
            F::ChromaticApproach
                | F::DiatonicPassing
                | F::ChromaticPassing
                | F::Neighbor
                | F::Enclosure
        )
    });
    let structural_destination = row.next_note.is_some_and(|j| {
        let x = &score.notes[j];
        perf.context_at(x.start_beat)
            .is_some_and(|c| c.chord.contains_pc(pitch_class(x.pitch)))
    });
    if matches!(
        row.extension,
        Some(
            ExtensionPath::CommonTone
                | ExtensionPath::ForwardLeading
                | ExtensionPath::OwnedTendency
        )
    ) || (connector && structural_destination)
    {
        Ownership::Local
    } else if connector || s.contains(&F::SlidePath) {
        Ownership::Path
    } else if s.contains(&F::LicensedExtension) {
        Ownership::Licensed
    } else {
        Ownership::None
    }
}

/// The verdict: greater mass demands stronger ownership. A colour is an available tension the
/// sounding harmony does not write.
pub fn verdict(class: ExposureClass, own: Ownership, is_color: bool) -> MassVerdict {
    use MassVerdict::*;
    if own == Ownership::Harmonic {
        return Carried;
    }
    match class {
        ExposureClass::Fleeting if own >= Ownership::Licensed => Carried,
        ExposureClass::Gestural if own >= Ownership::Local => Carried,
        ExposureClass::Gestural if is_color && own >= Ownership::Licensed => SoftOrphan,
        ExposureClass::Asserted if is_color => UnownedAssertedColor,
        ExposureClass::Gestural | ExposureClass::Asserted if own >= Ownership::Path => {
            OverdrawnGesture
        }
        _ => Unsupported,
    }
}

/// One note's exposure judged against its independently reconstructed ownership.
#[derive(Debug, Clone, PartialEq)]
pub struct MassRow {
    pub note_index: usize,
    pub mass: TemporalMass,
    /// An available tension the sounding harmony does not write (Round XII's extension flag).
    pub is_color: bool,
    pub ownership: Ownership,
    pub verdict: MassVerdict,
    pub detail: String,
}

/// Counts are different kinds of evidence, never one quality score.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MassDiagnostics {
    /// Asserted colours without harmonic ownership, every role.
    pub unowned_asserted_colors: usize,
    /// The support-role (pad, keys, bass) subset: the performance reharmonized the song.
    pub accidental_reharmonizations: usize,
    /// Non-harmonic tones exposed beyond what their gesture or connector can carry.
    pub overdrawn_gestures: usize,
    /// Gestural orphan colours kept as soft colour.
    pub soft_orphans: usize,
    /// Events with no reconstructed function.
    pub unsupported: usize,
    /// Every asserted event, owned or not (evidence, not a defect count).
    pub asserted_events: usize,
    pub rows: Vec<MassRow>,
    /// The unchanged Round XII audit this overlays.
    pub base: TemporalPitchDiagnostics,
}

impl MassDiagnostics {
    /// Audit every pitched note of `score` with its temporal mass under `world`'s patches at the
    /// score's own tempo (the score is the clock).
    pub fn measure(perf: &PerformancePlan, score: &Score, world: &MusicWorld) -> Self {
        let base = TemporalPitchDiagnostics::measure(perf, score);
        let masses = TemporalMass::of_notes(
            &score.notes,
            &perf.contexts,
            score.tempo_bpm,
            score.beats_per_bar,
            world,
        );
        let spb = 60.0 / f64::from(score.tempo_bpm.max(1.0));
        let mut out = MassDiagnostics::default();
        for (i, n) in score.notes.iter().enumerate() {
            let r = &base.rows[i];
            let mass = masses[i];
            let is_color = r.extension.is_some();
            let ownership = ownership_of(r, score, perf);
            let verdict = verdict(mass.class, ownership, is_color);
            let support = n.role != Role::Lead;
            out.asserted_events += usize::from(mass.class == ExposureClass::Asserted);
            match verdict {
                MassVerdict::UnownedAssertedColor => {
                    out.unowned_asserted_colors += 1;
                    out.accidental_reharmonizations += usize::from(support);
                }
                MassVerdict::OverdrawnGesture => out.overdrawn_gestures += 1,
                MassVerdict::SoftOrphan => out.soft_orphans += 1,
                MassVerdict::Unsupported => out.unsupported += 1,
                MassVerdict::Carried => {}
            }
            let harmony = |j: Option<usize>| {
                j.and_then(|j| perf.contexts.get(j))
                    .map(|c| format!("{} @ {:.2}", c.chord.label(), c.start_beat))
                    .unwrap_or_else(|| "none".into())
            };
            let why = match verdict {
                MassVerdict::Carried => "ownership meets the exposure's demand",
                MassVerdict::SoftOrphan => "gestural orphan colour: a soft suspect, not asserted",
                MassVerdict::UnownedAssertedColor if support => {
                    "asserted colour the sounding harmony does not write and no suspension, appoggiatura, anticipation or selection owns: accidental reharmonization"
                }
                MassVerdict::UnownedAssertedColor => {
                    "asserted colour the sounding harmony does not write and no suspension, appoggiatura, anticipation or selection owns"
                }
                MassVerdict::OverdrawnGesture => {
                    "heard for longer than its gesture or connector can carry: it owes its own justification"
                }
                MassVerdict::Unsupported => "no reconstructed function",
            };
            let detail = format!(
                "beat {:.4} ({:.3} s) role {} pitch {} ({})\n  {}\n  current {} next {}\n  declared {:?}; Round XII path {:?} {:?}; supported {:?}\n  ownership {:?}; mass verdict {:?}: {}\n",
                n.start_beat,
                n.start_beat * spb,
                n.role.label(),
                note_name(n.pitch),
                n.pitch,
                mass.evidence(),
                harmony(r.context),
                harmony(r.context.map(|c| c + 1)),
                n.function,
                r.status,
                r.extension,
                r.supported,
                ownership,
                verdict,
                why
            );
            out.rows.push(MassRow {
                note_index: i,
                mass,
                is_color,
                ownership,
                verdict,
                detail,
            });
        }
        out.base = base;
        out
    }

    /// Counts, then every temporal suspect: each note the Round XII observer suspected and each
    /// note whose mass verdict is not `Carried`.
    pub fn report(&self) -> String {
        let mut s = String::new();
        for (name, value) in [
            ("unowned_asserted_colors", self.unowned_asserted_colors),
            (
                "accidental_reharmonizations",
                self.accidental_reharmonizations,
            ),
            ("overdrawn_gestures", self.overdrawn_gestures),
            ("soft_orphans", self.soft_orphans),
            ("unsupported", self.unsupported),
            ("asserted_events", self.asserted_events),
            ("r12_false_function_claims", self.base.false_function_claims),
            ("r12_false_suspensions", self.base.false_suspensions),
            ("r12_broken_anticipations", self.base.broken_anticipations),
            ("r12_bad_arrivals", self.base.bad_arrivals),
            ("r12_unresolved_tendencies", self.base.unresolved_tendencies),
            ("r12_orphan_extensions", self.base.orphan_extensions),
        ] {
            let _ = writeln!(s, "{name}={value}");
        }
        for row in &self.rows {
            if row.verdict != MassVerdict::Carried || self.base.rows[row.note_index].suspect {
                s.push_str(&row.detail);
            }
        }
        s
    }

    /// Every note sounding within `radius_secs` of `center_secs` (a human timestamp), in onset
    /// order, with its harmony, evidence and verdict. Text only.
    pub fn around_seconds(&self, score: &Score, center_secs: f64, radius_secs: f64) -> String {
        let spb = 60.0 / f64::from(score.tempo_bpm.max(1.0));
        let (lo, hi) = (center_secs - radius_secs, center_secs + radius_secs);
        let mut idx: Vec<usize> = (0..score.notes.len())
            .filter(|&i| {
                let n = &score.notes[i];
                let a = n.start_beat * spb;
                a <= hi
                    && a + self.rows[i]
                        .mass
                        .audible_secs
                        .max(self.rows[i].mass.written_secs)
                        >= lo
            })
            .collect();
        idx.sort_by(|&a, &b| {
            score.notes[a]
                .start_beat
                .total_cmp(&score.notes[b].start_beat)
                .then(score.notes[a].role.label().cmp(score.notes[b].role.label()))
                .then(score.notes[a].pitch.cmp(&score.notes[b].pitch))
        });
        let mut s = format!(
            "window {lo:.2}..{hi:.2} s (beats {:.2}..{:.2}) at {:.0} BPM\n",
            lo / spb,
            hi / spb,
            score.tempo_bpm
        );
        for i in idx {
            s.push_str(&self.rows[i].detail);
        }
        s
    }
}
