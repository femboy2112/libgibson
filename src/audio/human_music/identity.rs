//! Round XIV: does the band still sound the chord the chart says?
//!
//! Every earlier pitch audit asked whether each NOTE could be justified. None of them asked what
//! CHORD the ensemble sounds. Round XIIIb shows the gap. It removed C5 from SWISS's Cmaj7 pad
//! (beat 28), and every surviving note was a legal member of Cmaj7. But the bass then walks
//! E2 G2 B2 under a pad holding E4 B4 G5, so for 0.77 s the heard object is E minor over its own
//! bass while the chart says Cmaj7.
//!
//! This audit slices the heard band ([`heard_windows`]: each note's audible lifetime, cut at its
//! role's next attack) wherever anything starts or stops. For every slice it reports explicit
//! evidence, not a score:
//!
//! - the declared chord;
//! - what sounds, and the bass (the lowest heard pitch);
//! - whether the chart's root is heard, and when it was last heard (the identity memory);
//! - the chart's guide tones;
//! - a rival: a complete major or minor triad on another root that contains the bass.
//!
//! The chart's identity FLIPS when a rival is heard over a support voice (pad or keys), with no
//! trace of the chart's root, for at least [`IDENTITY_HOLD_SECS`]. An inversion keeps its root in
//! the band (Cmaj7/E passes). A rootless voicing over the root in the bass passes. A walk away
//! from the root is a passing ambiguity until it holds.
//!
//! The Round VIII law ([`super::sonority`]) cannot hear the Round XIIIb flip. Its root memory
//! lasts two beats past the root's audible end and is clamped to the harmony's start, so a
//! downbeat root certifies the whole bar. Here the root's own audible tail is its memory, and
//! the hold is measured in seconds.
use super::context::{guide_tones, HarmonicContext};
use super::score::{Note, Role, Score};
use super::tension::heard_windows;
use super::theory::{note_name, pitch_class, Chord, Quality};
use super::world::MusicWorld;
use std::fmt::Write;

const EPS: f64 = 1e-6;

/// How long a rival triad must hold the ear before the band is heard playing it instead of the
/// chart: seconds. The rival must contain the bass and sound over a support voice, and no voice
/// may sound the chart's root. The anchors:
///
/// - The maintainer's two reported wrong chords are rival holds of 0.77 s (Round XIIIb SWISS
///   16 s, Em for Cmaj7) and 1.03 s (Round XII SWISS 43 s, C for Fmaj9, inside the 43–45 s
///   defect of the Round XII listen). Both flip.
/// - A walking bass's single eighth through a rival at 118 BPM (0.25 s) stays a passing
///   ambiguity.
pub const IDENTITY_HOLD_SECS: f64 = 0.5;

/// What a heard slice says about the chart's chord. Ordered worst first, so "no worse" is `>=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdentityStatus {
    /// The band is heard playing a rival chord: a rival held for at least [`IDENTITY_HOLD_SECS`].
    Flipped,
    /// A rival, not yet held long enough to replace the chart's chord.
    Passing,
    /// The chart's root is silent and nothing complete competes: what sounds implies the chord.
    Implied,
    /// The chart's root sounds.
    Rooted,
}

impl IdentityStatus {
    pub fn label(self) -> &'static str {
        match self {
            IdentityStatus::Flipped => "FLIPPED",
            IdentityStatus::Passing => "passing",
            IdentityStatus::Implied => "implied",
            IdentityStatus::Rooted => "rooted",
        }
    }
}

/// One stretch of constant heard band under one chart harmony.
#[derive(Debug, Clone, PartialEq)]
pub struct IdentitySlice {
    pub start_beat: f64,
    pub end_beat: f64,
    /// Index into the contexts.
    pub context: usize,
    /// The heard notes (indices into the notes), low to high.
    pub heard: Vec<usize>,
    /// The lowest heard note.
    pub bass: usize,
    /// Whether any heard note sounds the chart's root pitch class.
    pub root_heard: bool,
    /// When the chart's root was last heard, in beats (the end of its latest heard window), if
    /// ever before this slice.
    pub root_last_heard: Option<f64>,
    /// Whether the chart's guide tones are heard: 3rd, then 7th or 6th (if the chord has one).
    pub guide_heard: Vec<(i32, bool)>,
    /// Whether a pad or keys note sounds.
    pub support: bool,
    /// A complete major or minor triad on another root, containing the bass (only while the
    /// chart's root is silent and a support voice sounds).
    pub rival: Option<Chord>,
    pub status: IdentityStatus,
}

/// A maximal stretch of rival slices with the same chart root.
#[derive(Debug, Clone, PartialEq)]
pub struct IdentityRun {
    pub start_beat: f64,
    pub end_beat: f64,
    pub secs: f64,
    /// The chart chord at the run's start.
    pub chart: Chord,
    /// The rival at the run's start.
    pub rival: Chord,
    /// Indices of the slices in the run.
    pub slices: Vec<usize>,
    pub flipped: bool,
}

/// The heard-identity audit of one band.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IdentityDiagnostics {
    pub tempo_bpm: f32,
    pub slices: Vec<IdentitySlice>,
    pub runs: Vec<IdentityRun>,
}

/// The first complete major or minor triad on a root other than `root` whose pitch classes all
/// sound in `pcs` and which contains `bass`. A triad rooted on the bass is preferred.
pub fn rival_triad(pcs: &[i32], bass: i32, root: i32) -> Option<Chord> {
    let has = |pc: i32| pcs.contains(&pc.rem_euclid(12));
    let mut roots: Vec<i32> = (0..12).map(|k| (root + k) % 12).collect();
    roots.sort_by_key(|&r| r != bass.rem_euclid(12));
    roots
        .into_iter()
        .filter(|&r| r != root.rem_euclid(12))
        .flat_map(|r| [Chord::new(r, Quality::Maj), Chord::new(r, Quality::Min)])
        .find(|c| {
            let pc = c.pitch_classes();
            pc.iter().all(|&p| has(p)) && pc.contains(&bass.rem_euclid(12))
        })
}

impl IdentityDiagnostics {
    /// Audit `notes` against `contexts` as heard on `world`'s patches at `tempo_bpm`.
    pub fn measure(
        notes: &[Note],
        contexts: &[HarmonicContext],
        world: &MusicWorld,
        tempo_bpm: f32,
    ) -> IdentityDiagnostics {
        let win = heard_windows(notes, world, tempo_bpm);
        Self::measure_windows(notes, contexts, tempo_bpm, &win)
    }

    /// Audit with the score's explicit pocket continuity and its own tempo.
    pub fn measure_score(score: &Score, contexts: &[HarmonicContext], world: &MusicWorld) -> Self {
        let win = super::tension::heard_windows_score(score, world);
        Self::measure_windows(&score.notes, contexts, score.tempo_bpm, &win)
    }

    fn measure_windows(
        notes: &[Note],
        contexts: &[HarmonicContext],
        tempo_bpm: f32,
        win: &[(f64, f64)],
    ) -> Self {
        let mut cuts: Vec<f64> = win.iter().flat_map(|&(a, b)| [a, b]).collect();
        cuts.extend(contexts.iter().map(|c| c.start_beat));
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < EPS);
        let mut slices = Vec::new();
        for w in cuts.windows(2) {
            let (t0, t1) = (w[0], w[1]);
            if t1 - t0 < EPS {
                continue;
            }
            let Some(ci) = contexts.iter().rposition(|c| c.start_beat <= t0 + EPS) else {
                continue;
            };
            let chord = contexts[ci].chord;
            let mut heard: Vec<usize> = (0..notes.len())
                .filter(|&i| win[i].0 <= t0 + EPS && win[i].1 > t0 + EPS)
                .collect();
            if heard.is_empty() {
                continue;
            }
            heard.sort_by_key(|&i| (notes[i].pitch, notes[i].role.label()));
            let bass = heard[0];
            let pcs: Vec<i32> = heard.iter().map(|&i| pitch_class(notes[i].pitch)).collect();
            let root = chord.root_pc.rem_euclid(12);
            let root_heard = pcs.contains(&root);
            let root_last_heard = (0..notes.len())
                .filter(|&i| pitch_class(notes[i].pitch) == root && win[i].0 < t0 + EPS)
                .map(|i| win[i].1.min(t0))
                .max_by(f64::total_cmp);
            let guide_heard = guide_tones(&chord)
                .into_iter()
                .map(|g| (g, pcs.contains(&g)))
                .collect();
            let support = heard
                .iter()
                .any(|&i| matches!(notes[i].role, Role::Pad | Role::Keys));
            let rival = (!root_heard && support)
                .then(|| rival_triad(&pcs, pitch_class(notes[bass].pitch), root))
                .flatten();
            let status = if root_heard {
                IdentityStatus::Rooted
            } else if rival.is_some() {
                IdentityStatus::Passing
            } else {
                IdentityStatus::Implied
            };
            slices.push(IdentitySlice {
                start_beat: t0,
                end_beat: t1,
                context: ci,
                heard,
                bass,
                root_heard,
                root_last_heard,
                guide_heard,
                support,
                rival,
                status,
            });
        }
        let secs = |beats: f64| beats * 60.0 / f64::from(tempo_bpm.max(1.0));
        let mut runs: Vec<IdentityRun> = Vec::new();
        for (k, s) in slices.iter().enumerate() {
            let Some(rival) = s.rival else {
                continue;
            };
            let root = contexts[s.context].chord.root_pc;
            match runs.last_mut() {
                Some(r)
                    if (r.end_beat - s.start_beat).abs() < EPS
                        && r.chart.root_pc == root
                        && r.slices.last() == Some(&(k - 1)) =>
                {
                    r.end_beat = s.end_beat;
                    r.slices.push(k);
                }
                _ => runs.push(IdentityRun {
                    start_beat: s.start_beat,
                    end_beat: s.end_beat,
                    secs: 0.0,
                    chart: contexts[s.context].chord,
                    rival,
                    slices: vec![k],
                    flipped: false,
                }),
            }
        }
        for r in &mut runs {
            r.secs = secs(r.end_beat - r.start_beat);
            r.flipped = r.secs >= IDENTITY_HOLD_SECS - EPS;
            if r.flipped {
                for &k in &r.slices {
                    slices[k].status = IdentityStatus::Flipped;
                }
            }
        }
        IdentityDiagnostics {
            tempo_bpm,
            slices,
            runs,
        }
    }

    /// Beats to seconds at this audit's tempo.
    pub fn secs(&self, beats: f64) -> f64 {
        beats * 60.0 / f64::from(self.tempo_bpm.max(1.0))
    }

    /// The runs that flip the chart's identity.
    pub fn flips(&self) -> impl Iterator<Item = &IdentityRun> {
        self.runs.iter().filter(|r| r.flipped)
    }

    /// Total seconds of flipped identity.
    pub fn flipped_secs(&self) -> f64 {
        self.flips().map(|r| r.secs).sum()
    }

    /// The slice sounding at `beat`, if anything sounds.
    pub fn at_beat(&self, beat: f64) -> Option<&IdentitySlice> {
        self.slices
            .iter()
            .find(|s| s.start_beat <= beat + EPS && beat + EPS < s.end_beat)
    }

    /// The status at `beat` ([`IdentityStatus::Implied`] where nothing sounds: there is no chord
    /// to lose).
    pub fn status_at(&self, beat: f64) -> IdentityStatus {
        self.at_beat(beat)
            .map_or(IdentityStatus::Implied, |s| s.status)
    }

    /// One line of evidence for slice `s`.
    pub fn line(&self, notes: &[Note], contexts: &[HarmonicContext], s: &IdentitySlice) -> String {
        let chord = contexts[s.context].chord;
        let mut by_role = String::new();
        for role in [Role::Bass, Role::Pad, Role::Keys, Role::Lead] {
            let v: Vec<String> = s
                .heard
                .iter()
                .filter(|&&i| notes[i].role == role)
                .map(|&i| note_name(notes[i].pitch))
                .collect();
            if !v.is_empty() {
                let _ = write!(by_role, " {} {}", role.label(), v.join(" "));
            }
        }
        let memory = if s.root_heard {
            "heard".to_string()
        } else {
            s.root_last_heard.map_or("never heard".to_string(), |t| {
                format!("last heard {:.3} s ago", self.secs(s.start_beat - t))
            })
        };
        let guide: Vec<String> = s
            .guide_heard
            .iter()
            .map(|&(g, h)| format!("{}{}", pc_name(g), if h { "+" } else { "-" }))
            .collect();
        let rival = s.rival.map_or("none".to_string(), |r| r.label());
        format!(
            "{:>7.3}-{:<7.3} s  beats {:>7.3}-{:<7.3}  chart {:<6} bass {:<4}|{}  | root {}: {}  guide [{}]  rival {}  -> {}",
            self.secs(s.start_beat),
            self.secs(s.end_beat),
            s.start_beat,
            s.end_beat,
            chord.label(),
            note_name(notes[s.bass].pitch),
            by_role,
            pc_name(chord.root_pc),
            memory,
            guide.join(" "),
            rival,
            s.status.label()
        )
    }

    /// Counts, every run, and every flipped slice spelled out.
    pub fn report(&self, notes: &[Note], contexts: &[HarmonicContext]) -> String {
        let mut out = String::new();
        let flips: Vec<&IdentityRun> = self.flips().collect();
        let _ = writeln!(
            out,
            "identity: {} slices, {} rival runs, {} flipped ({:.3} s), hold {IDENTITY_HOLD_SECS} s",
            self.slices.len(),
            self.runs.len(),
            flips.len(),
            self.flipped_secs()
        );
        for r in &self.runs {
            let _ = writeln!(
                out,
                "run {:>7.3}-{:<7.3} s  beats {:>7.3}-{:<7.3}  chart {:<6} rival {:<4} {:.3} s  {}",
                self.secs(r.start_beat),
                self.secs(r.end_beat),
                r.start_beat,
                r.end_beat,
                r.chart.label(),
                r.rival.label(),
                r.secs,
                if r.flipped { "FLIPPED" } else { "passing" }
            );
            if r.flipped {
                for &k in &r.slices {
                    let _ = writeln!(out, "    {}", self.line(notes, contexts, &self.slices[k]));
                }
            }
        }
        out
    }

    /// Every slice within `radius_secs` of `center_secs`.
    pub fn around_seconds(
        &self,
        notes: &[Note],
        contexts: &[HarmonicContext],
        center_secs: f64,
        radius_secs: f64,
    ) -> String {
        let mut out = String::new();
        for s in &self.slices {
            let (a, b) = (self.secs(s.start_beat), self.secs(s.end_beat));
            if b > center_secs - radius_secs && a < center_secs + radius_secs {
                let _ = writeln!(out, "{}", self.line(notes, contexts, s));
            }
        }
        out
    }
}

/// Whether `after` keeps the chart's identity at least as well as `before` at every instant of
/// `[a, b)` beats: the hard safety property of any support edit.
pub fn keeps_identity(
    before: &IdentityDiagnostics,
    after: &IdentityDiagnostics,
    a: f64,
    b: f64,
) -> bool {
    let mut cuts: Vec<f64> = before
        .slices
        .iter()
        .chain(&after.slices)
        .flat_map(|s| [s.start_beat, s.end_beat])
        .filter(|&t| t > a + EPS && t < b - EPS)
        .collect();
    cuts.push(a);
    cuts.push(b);
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|x, y| (*x - *y).abs() < EPS);
    cuts.windows(2).all(|w| {
        let mid = 0.5 * (w[0] + w[1]);
        after.status_at(mid) >= before.status_at(mid)
    })
}

fn pc_name(pc: i32) -> &'static str {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    NAMES[pc.rem_euclid(12) as usize]
}
