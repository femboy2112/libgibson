//! Round XVI: chord identity and a chord's performed voicing are different observations.
//!
//! This observer reads the actual pad notes and the band's envelope-derived audible intervals.
//! It does not call the pad realizer or change a score. Register, inversion, interval contact,
//! attack coincidence and adjacent motion remain separate evidence; none is a quality score.
//! The high-seventh excursion witness is a bounded comparative listening hypothesis, calibrated
//! against within-song controls, not a universal prohibition on high major sevenths.

use super::performance::PerformancePlan;
use super::score::{Note, Role, Score};
use super::sonority::{audible_end, audible_voices, AUDIBLE_FLOOR_DB};
use super::theory::{note_name, pitch_class, Midi};
use super::world::MusicWorld;
use std::fmt::Write;

/// One actual pad/band interval, over the interval in which both voices are audible.
#[derive(Debug, Clone, PartialEq)]
pub struct IntervalContact {
    pub pad_pitch: Midi,
    pub other_pitch: Midi,
    pub other_role: Role,
    /// Absolute register interval, not reduced modulo twelve.
    pub semitones: i32,
    pub overlap_seconds: f64,
    pub onset_separation_seconds: f64,
    /// Geometric co-attack evidence only; this does not certify perceptual fusion.
    pub coincident_attack: bool,
}

/// The actual pitch collection over one constant audible-band interval.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundingSet {
    pub start_beat: f64,
    pub end_beat: f64,
    pub notes: Vec<(Role, Midi)>,
    pub bass: Option<Midi>,
}

/// A decomposed observation of one pad harmony.
#[derive(Debug, Clone, PartialEq)]
pub struct VoicingSurfaceRow {
    pub context: usize,
    pub beat: f64,
    pub seconds: f64,
    pub root_pc: i32,
    pub pad: Vec<Midi>,
    pub root_pitches: Vec<Midi>,
    pub guide_pitches: Vec<Midi>,
    pub doubled_pitch_classes: Vec<i32>,
    /// Pad's lowest sounding member relative to the chart root. This is not the band bass.
    pub pad_inversion: Option<i32>,
    pub register_center: Option<f64>,
    pub span: i32,
    pub previous_pad: Vec<Midi>,
    pub next_pad: Vec<Midi>,
    /// Ordered-voice deltas; empty when voice cardinality changes (no invented ownership).
    pub incoming_motion: Vec<i32>,
    pub outgoing_motion: Vec<i32>,
    pub incoming_common_pitches: Vec<Midi>,
    pub outgoing_common_pitches: Vec<Midi>,
    pub sounding_sets: Vec<SoundingSet>,
    pub contacts: Vec<IntervalContact>,
    /// Top note's whole envelope-derived lifetime, including its release across a boundary.
    pub top_audible_seconds: f64,
    /// Seconds times velocity times patch gain. An exposure proxy, NOT measured loudness,
    /// an envelope integral, spectral roughness, fusion, or a musical quality scalar.
    pub top_weighted_exposure: f64,
    pub top_is_major_seventh: bool,
    pub top_incoming_semitones: Option<i32>,
    pub top_outgoing_semitones: Option<i32>,
    /// A sustained top major seventh rises at least a minor third above BOTH adjacent tops,
    /// while the pad places the root above its bass. This isolates the reported Cmaj7 family
    /// without matching a world, beat, MIDI pitch or chord name.
    pub high_seventh_excursion: bool,
}

/// Actual-score voicing evidence, independent of the identity observer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VoicingSurfaceDiagnostics {
    pub rows: Vec<VoicingSurfaceRow>,
}

fn pad_at(score: &Score, a: f64, b: f64) -> Vec<Midi> {
    let mut pitches: Vec<_> = score
        .role_notes(Role::Pad)
        .filter(|n| n.start_beat >= a - 1e-6 && n.start_beat < b - 1e-6)
        .map(|n| n.pitch)
        .collect();
    pitches.sort_unstable();
    pitches
}

fn motion(a: &[Midi], b: &[Midi]) -> Vec<i32> {
    if a.len() != b.len() {
        Vec::new()
    } else {
        a.iter().zip(b).map(|(a, b)| b - a).collect()
    }
}

fn common(a: &[Midi], b: &[Midi]) -> Vec<Midi> {
    a.iter().copied().filter(|p| b.contains(p)).collect()
}

impl VoicingSurfaceDiagnostics {
    /// Observe each context in the performed score. Envelope-derived contacts include pitched
    /// SFX; source register/motion describe only notes actually emitted by the pad.
    pub fn measure(perf: &PerformancePlan, score: &Score, world: &MusicWorld) -> Self {
        let spb = 60.0 / f64::from(score.tempo_bpm.max(1.0));
        let voices = audible_voices(score, &perf.contexts, world, AUDIBLE_FLOOR_DB);
        let pads: Vec<_> = perf
            .contexts
            .iter()
            .map(|c| pad_at(score, c.start_beat, c.start_beat + f64::from(c.dur_beats)))
            .collect();
        let mut rows = Vec::new();
        for (ci, ctx) in perf.contexts.iter().enumerate() {
            let pad = &pads[ci];
            if pad.is_empty() {
                continue;
            }
            let a = ctx.start_beat;
            let b = a + f64::from(ctx.dur_beats);
            let previous_pad = ci
                .checked_sub(1)
                .map(|i| pads[i].clone())
                .unwrap_or_default();
            let next_pad = pads.get(ci + 1).cloned().unwrap_or_default();
            let top = *pad.last().expect("nonempty pad");
            let top_notes: Vec<&Note> = score
                .role_notes(Role::Pad)
                .filter(|n| n.pitch == top && n.start_beat >= a - 1e-6 && n.start_beat < b - 1e-6)
                .collect();
            let lifetime = |n: &Note| {
                (audible_end(
                    n.start_beat,
                    f64::from(n.dur_beats),
                    &world.pad,
                    score.tempo_bpm,
                ) - n.start_beat)
                    * spb
            };
            let top_audible_seconds = top_notes.iter().map(|n| lifetime(n)).sum::<f64>();
            let top_weighted_exposure = top_notes
                .iter()
                .map(|n| lifetime(n) * f64::from(n.velocity * world.pad.gain))
                .sum();
            let root_pc = ctx.chord.root_pc.rem_euclid(12);
            let root_pitches: Vec<_> = pad
                .iter()
                .copied()
                .filter(|&p| pitch_class(p) == root_pc)
                .collect();
            let guide_pitches = pad
                .iter()
                .copied()
                .filter(|&p| ctx.palette.guide_tones.contains(&pitch_class(p)))
                .collect();
            let doubled_pitch_classes = (0..12)
                .filter(|&pc| pad.iter().filter(|&&p| pitch_class(p) == pc).count() > 1)
                .collect();
            let mut edges = vec![a, b];
            for v in voices.iter().filter(|v| v.start < b && v.end > a) {
                edges.push(v.start.max(a));
                edges.push(v.end.min(b));
            }
            edges.sort_by(f64::total_cmp);
            edges.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
            let sounding_sets = edges
                .windows(2)
                .map(|w| {
                    let notes: Vec<_> = voices
                        .iter()
                        .filter(|v| v.start < w[1] - 1e-9 && v.end > w[0] + 1e-9)
                        .map(|v| (v.role, v.pitch))
                        .collect();
                    let bass = notes.iter().map(|(_, p)| *p).min();
                    SoundingSet {
                        start_beat: w[0],
                        end_beat: w[1],
                        notes,
                        bass,
                    }
                })
                .collect();
            let mut contacts = Vec::new();
            for (i, p) in voices.iter().enumerate().filter(|(_, v)| {
                !v.sfx && v.role == Role::Pad && v.start >= a - 1e-6 && v.start < b - 1e-6
            }) {
                for (j, q) in voices.iter().enumerate() {
                    if i == j || (q.role == Role::Pad && !q.sfx && j < i && q.start >= a - 1e-6) {
                        continue;
                    }
                    let overlap =
                        (p.end.min(q.end).min(b) - p.start.max(q.start).max(a)).max(0.0) * spb;
                    if overlap <= 1e-9 {
                        continue;
                    }
                    let onset_separation_seconds = (p.start - q.start).abs() * spb;
                    contacts.push(IntervalContact {
                        pad_pitch: p.pitch,
                        other_pitch: q.pitch,
                        other_role: q.role,
                        semitones: (p.pitch - q.pitch).abs(),
                        overlap_seconds: overlap,
                        onset_separation_seconds,
                        coincident_attack: onset_separation_seconds <= 0.01,
                    });
                }
            }
            let top_is_major_seventh = (pitch_class(top) - root_pc).rem_euclid(12) == 11
                && ctx.chord.contains_pc(pitch_class(top));
            let top_incoming_semitones = previous_pad.last().map(|p| top - p);
            let top_outgoing_semitones = next_pad.last().map(|p| p - top);
            let pad_inversion = pad
                .first()
                .map(|p| (pitch_class(*p) - root_pc).rem_euclid(12));
            let high_seventh_excursion = top_is_major_seventh
                && top_audible_seconds >= 1.0
                && pad_inversion.is_some_and(|i| i != 0)
                && !root_pitches.is_empty()
                && top_incoming_semitones.is_some_and(|d| d >= 3)
                && top_outgoing_semitones.is_some_and(|d| d <= -3);
            rows.push(VoicingSurfaceRow {
                context: ci,
                beat: a,
                seconds: a * spb,
                root_pc,
                pad: pad.clone(),
                root_pitches,
                guide_pitches,
                doubled_pitch_classes,
                pad_inversion,
                register_center: Some(
                    pad.iter().map(|&p| f64::from(p)).sum::<f64>() / pad.len() as f64,
                ),
                span: top - pad[0],
                incoming_motion: motion(&previous_pad, pad),
                outgoing_motion: motion(pad, &next_pad),
                incoming_common_pitches: common(&previous_pad, pad),
                outgoing_common_pitches: common(pad, &next_pad),
                previous_pad,
                next_pad,
                sounding_sets,
                contacts,
                top_audible_seconds,
                top_weighted_exposure,
                top_is_major_seventh,
                top_incoming_semitones,
                top_outgoing_semitones,
                high_seventh_excursion,
            });
        }
        Self { rows }
    }

    /// Register and path witnesses, followed by every interval contact and audible set.
    pub fn report(&self) -> String {
        let mut out = String::new();
        for r in &self.rows {
            let names = r
                .pad
                .iter()
                .map(|&p| note_name(p))
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(out, "beat={:.3} seconds={:.3} pad=[{}] root={:?} guides={:?} doubled_pc={:?} inversion={:?} center={:?} span={} previous={:?} next={:?} in_motion={:?} out_motion={:?} common_in={:?} common_out={:?} top_life_s={:.6} top_exposure_proxy={:.6} top_major_seventh={} top_in={:?} top_out={:?} high_seventh_excursion={}", r.beat,r.seconds,names,r.root_pitches,r.guide_pitches,r.doubled_pitch_classes,r.pad_inversion,r.register_center,r.span,r.previous_pad,r.next_pad,r.incoming_motion,r.outgoing_motion,r.incoming_common_pitches,r.outgoing_common_pitches,r.top_audible_seconds,r.top_weighted_exposure,r.top_is_major_seventh,r.top_incoming_semitones,r.top_outgoing_semitones,r.high_seventh_excursion);
            for s in &r.sounding_sets {
                let _ = writeln!(
                    out,
                    "  sounding {:.6}..{:.6} bass={:?} notes={:?}",
                    s.start_beat, s.end_beat, s.bass, s.notes
                );
            }
            for c in &r.contacts {
                let _ = writeln!(out, "  contact {}:{} {}:{} interval={} overlap_s={:.6} onset_separation_s={:.6} coattack={}", Role::Pad.label(),note_name(c.pad_pitch),c.other_role.label(),note_name(c.other_pitch),c.semitones,c.overlap_seconds,c.onset_separation_seconds,c.coincident_attack);
            }
        }
        out
    }
}
