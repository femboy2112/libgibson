//! Motif identities and their transformations. A motif is a scale-degree contour plus a
//! rhythm; the same motif heard at the climax must be recognizably the object seeded at the
//! start. Transformations preserve the identity (`id`) while developing the material, so
//! the melody engine can grow one idea across the whole piece instead of inventing a new
//! tune every four bars.

use super::context::HarmonicContext;
use super::form::BEATS_PER_BAR;
use super::harmony::ChordSpan;
use super::language::MusicalLanguage;
use super::rng::Rng;
use super::score::PitchFunction;
use super::theory::{pitch_class, Chord, Midi, Mode, Scale};

/// A motif: parallel scale-degree and rhythm vectors sharing an identity.
#[derive(Debug, Clone, PartialEq)]
pub struct Motif {
    /// Stable identity — survives every transformation.
    pub id: u8,
    /// Scale-degree offsets from a tonal root (the melodic contour).
    pub degrees: Vec<i32>,
    /// Note durations in beats, parallel to `degrees`.
    pub rhythm: Vec<f32>,
}

impl Motif {
    /// The signature seed: a syncopated call that leaps to a guide tone and steps back — a theme
    /// with an interval story, not a triad exercise. The eighth-note pickup and the reach up to the
    /// 6th degree (degree 5) give it a hook the ear can catch.
    pub fn seed_a() -> Motif {
        Motif {
            id: 0,
            degrees: vec![0, 4, 3, 5, 2],
            rhythm: vec![0.5, 0.5, 1.0, 1.0, 1.0],
        }
    }

    /// A darker, more restless seed: a stepwise climb that reaches for the 7th (degree 6) and folds
    /// back down, front-loaded with sixteenth-into-eighth momentum.
    pub fn seed_b() -> Motif {
        Motif {
            id: 1,
            degrees: vec![0, 3, 6, 4, 3, 1],
            rhythm: vec![0.5, 0.5, 0.5, 0.5, 1.0, 1.0],
        }
    }

    /// Number of notes.
    pub fn len(&self) -> usize {
        self.degrees.len()
    }

    /// True if empty.
    pub fn is_empty(&self) -> bool {
        self.degrees.is_empty()
    }

    /// Total duration in beats.
    pub fn total_beats(&self) -> f32 {
        self.rhythm.iter().sum()
    }

    /// Transpose the contour by `by` scale degrees (identity preserved).
    pub fn transpose(&self, by: i32) -> Motif {
        Motif {
            id: self.id,
            degrees: self.degrees.iter().map(|d| d + by).collect(),
            rhythm: self.rhythm.clone(),
        }
    }

    /// Invert the contour about its first degree (intervals negated).
    pub fn invert(&self) -> Motif {
        let pivot = self.degrees.first().copied().unwrap_or(0);
        Motif {
            id: self.id,
            degrees: self.degrees.iter().map(|d| pivot - (d - pivot)).collect(),
            rhythm: self.rhythm.clone(),
        }
    }

    /// Reverse both contour and rhythm.
    pub fn retrograde(&self) -> Motif {
        let mut degrees = self.degrees.clone();
        degrees.reverse();
        let mut rhythm = self.rhythm.clone();
        rhythm.reverse();
        Motif {
            id: self.id,
            degrees,
            rhythm,
        }
    }

    /// Scale the rhythm by `factor` (>1 augments/slows, <1 diminishes/quickens).
    pub fn scale_rhythm(&self, factor: f32) -> Motif {
        let f = factor.max(0.05);
        Motif {
            id: self.id,
            degrees: self.degrees.clone(),
            rhythm: self.rhythm.iter().map(|r| r * f).collect(),
        }
    }

    /// Take the first `take` notes (a fragment) — the *question*: an incomplete gesture.
    pub fn fragment(&self, take: usize) -> Motif {
        let n = take.clamp(1, self.len().max(1));
        Motif {
            id: self.id,
            degrees: self.degrees.iter().take(n).copied().collect(),
            rhythm: self.rhythm.iter().take(n).copied().collect(),
        }
    }

    /// Take the notes *after* the first `skip` (the withheld remainder) — the *answer* that
    /// completes a `fragment(skip)` question. Clamped so at least one note always remains.
    pub fn tail(&self, skip: usize) -> Motif {
        let s = skip.min(self.len().saturating_sub(1));
        Motif {
            id: self.id,
            degrees: self.degrees.iter().skip(s).copied().collect(),
            rhythm: self.rhythm.iter().skip(s).copied().collect(),
        }
    }

    /// Build a sequence: `times` copies, each transposed a further `step` degrees.
    pub fn sequence(&self, step: i32, times: usize) -> Motif {
        let mut degrees = Vec::new();
        let mut rhythm = Vec::new();
        for k in 0..times.max(1) {
            for (i, d) in self.degrees.iter().enumerate() {
                degrees.push(d + step * k as i32);
                rhythm.push(self.rhythm[i]);
            }
        }
        Motif {
            id: self.id,
            degrees,
            rhythm,
        }
    }

    /// Append `other` after this motif (same identity) — a call and its response fused into one
    /// statement, sharing both their DNA.
    pub fn concat(&self, other: &Motif) -> Motif {
        let mut degrees = self.degrees.clone();
        degrees.extend_from_slice(&other.degrees);
        let mut rhythm = self.rhythm.clone();
        rhythm.extend_from_slice(&other.rhythm);
        Motif {
            id: self.id,
            degrees,
            rhythm,
        }
    }

    /// Realize the motif to `(start_beat, dur_beats, pitch)` notes on `scale`, with each
    /// degree offset from `root_degree` at `octave`, beginning at `start_beat`.
    pub fn render(
        &self,
        scale: &Scale,
        root_degree: i32,
        octave: i32,
        start_beat: f64,
    ) -> Vec<(f64, f32, Midi)> {
        let mut out = Vec::with_capacity(self.len());
        let mut t = start_beat;
        for (i, &deg) in self.degrees.iter().enumerate() {
            let dur = self.rhythm[i];
            let pitch = scale.degree_pitch(root_degree + deg, octave);
            out.push((t, dur, pitch));
            t += dur as f64;
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Round II motif toolkit.
//
// Round I performed the melodic equivalent of amputating the patient and stitching
// on a fresh limb every section: it re-seeded a pristine motif per span, then snapped
// each note to a chord tone one at a time, severing the joint contour. The tools below
// keep the limb attached. `MotifIdentity` is a fingerprint that survives transposition
// and tempo; `motif_similarity` measures whether two statements are the same idea;
// `MotifBank` is a coherent roster grown from one germ; `realize_phrase` sets a whole
// statement against harmony at once, preserving its shape instead of dicing it note by
// note. Diagnose before you cut — the same discipline, in code.
// ---------------------------------------------------------------------------

/// A transposition- and tempo-invariant structural fingerprint of a motif.
///
/// Strip away absolute pitch and absolute duration and what's left is the *idea*: the
/// sequence of leaps, their directions, and the proportion of the rhythm. Two statements
/// that share this fingerprint are heard as the same object, however far apart in the
/// register or the clock.
#[derive(Debug, Clone, PartialEq)]
pub struct MotifIdentity {
    /// Successive degree differences — the melodic shape, invariant under transposition.
    pub interval_contour: Vec<i32>,
    /// Rhythm normalized to sum = 1 — invariant under augmentation/diminution.
    pub rhythmic_profile: Vec<f32>,
    /// Sign of each interval: -1 down, 0 same, +1 up — the bare gesture.
    pub direction_signature: Vec<i8>,
}

impl Motif {
    /// Extract this motif's [`MotifIdentity`] — its fingerprint stripped of key and tempo.
    pub fn identity(&self) -> MotifIdentity {
        let interval_contour: Vec<i32> = self.degrees.windows(2).map(|w| w[1] - w[0]).collect();
        let direction_signature: Vec<i8> =
            interval_contour.iter().map(|&i| i.signum() as i8).collect();
        let total: f32 = self.rhythm.iter().sum();
        // Guard the degenerate all-zero (or empty) rhythm: an even split beats a NaN.
        let rhythmic_profile: Vec<f32> = if total.abs() > f32::EPSILON {
            self.rhythm.iter().map(|r| r / total).collect()
        } else if self.rhythm.is_empty() {
            Vec::new()
        } else {
            let even = 1.0 / self.rhythm.len() as f32;
            self.rhythm.iter().map(|_| even).collect()
        };
        MotifIdentity {
            interval_contour,
            rhythmic_profile,
            direction_signature,
        }
    }
}

impl MotifIdentity {
    /// Build a fingerprint directly from a REALIZED pitch + rhythm sequence — for measuring what
    /// the Score actually sounds, independent of any planned [`Motif`]. Mirrors [`Motif::identity`]
    /// but over concrete pitches (semitone intervals) rather than scale degrees.
    pub fn from_sequence(pitches: &[Midi], rhythms: &[f32]) -> MotifIdentity {
        let interval_contour: Vec<i32> = pitches.windows(2).map(|w| w[1] - w[0]).collect();
        let direction_signature: Vec<i8> =
            interval_contour.iter().map(|&i| i.signum() as i8).collect();
        let total: f32 = rhythms.iter().sum();
        let rhythmic_profile: Vec<f32> = if total.abs() > f32::EPSILON {
            rhythms.iter().map(|r| r / total).collect()
        } else if rhythms.is_empty() {
            Vec::new()
        } else {
            let even = 1.0 / rhythms.len() as f32;
            rhythms.iter().map(|_| even).collect()
        };
        MotifIdentity {
            interval_contour,
            rhythmic_profile,
            direction_signature,
        }
    }
}

/// Bounded `[0, 1]` perceptual relatedness of two motif identities: `1.0` = the same idea,
/// `0.0` = unrelated.
///
/// Three components, each mapped into `[0, 1]` and blended under a **declared weighting**:
/// - **interval contour** (weight `0.55`) — per-step degree-difference agreement, each step
///   scored `1 - |Δ| / INTERVAL_CAP` (capped at `INTERVAL_CAP` degrees); the melodic shape
///   carries the most weight because it *is* the tune.
/// - **direction signature** (weight `0.25`) — fraction of up/down/same gestures that agree.
/// - **rhythmic profile** (weight `0.20`) — `1 - L1_distance / 2` over the normalized rhythm.
///
/// Different-length motifs are compared over the shorter length, then the whole score is
/// scaled by a length-mismatch penalty of `min_len / max_len` so a fragment of a phrase reads
/// as *related but not identical*. Self-identity returns exactly `1.0` (the weighted mean of
/// three perfect components, times a unit penalty).
pub fn motif_similarity(a: &MotifIdentity, b: &MotifIdentity) -> f32 {
    const W_INTERVAL: f32 = 0.55;
    const W_DIRECTION: f32 = 0.25;
    const W_RHYTHM: f32 = 0.20;

    let interval_sim = contour_agreement(&a.interval_contour, &b.interval_contour);
    let direction_sim = direction_agreement(&a.direction_signature, &b.direction_signature);
    let rhythm_sim = profile_agreement(&a.rhythmic_profile, &b.rhythmic_profile);

    let numer = W_INTERVAL * interval_sim + W_DIRECTION * direction_sim + W_RHYTHM * rhythm_sim;
    let denom = W_INTERVAL + W_DIRECTION + W_RHYTHM;
    let blended = numer / denom;

    // Length-mismatch penalty over the rhythmic profile (one entry per note, unlike the
    // interval/direction vectors which lose the last note). Equal lengths => unit penalty,
    // so self-identity stays exactly 1.0.
    let la = a.rhythmic_profile.len();
    let lb = b.rhythmic_profile.len();
    let penalty = if la == 0 || lb == 0 {
        1.0
    } else {
        la.min(lb) as f32 / la.max(lb) as f32
    };

    (blended * penalty).clamp(0.0, 1.0)
}

/// Maximum degree-difference before two contour steps count as fully unrelated.
const INTERVAL_CAP: f32 = 5.0;

/// Mean per-step interval agreement over the shorter contour; `1.0` when nothing to compare.
fn contour_agreement(a: &[i32], b: &[i32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 1.0;
    }
    let sum: f32 = a
        .iter()
        .zip(b.iter())
        .take(n)
        .map(|(&x, &y)| {
            let d = (x - y).abs() as f32;
            (1.0 - d / INTERVAL_CAP).max(0.0)
        })
        .sum();
    sum / n as f32
}

/// Fraction of matching direction gestures over the shorter signature; `1.0` when empty.
fn direction_agreement(a: &[i8], b: &[i8]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 1.0;
    }
    let matches = a
        .iter()
        .zip(b.iter())
        .take(n)
        .filter(|(x, y)| x == y)
        .count();
    matches as f32 / n as f32
}

/// Rhythmic-profile agreement as `1 - L1/2` over the shorter profile; `1.0` when empty.
fn profile_agreement(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 1.0;
    }
    let l1: f32 = a
        .iter()
        .zip(b.iter())
        .take(n)
        .map(|(&x, &y)| (x - y).abs())
        .sum();
    (1.0 - l1 / 2.0).clamp(0.0, 1.0)
}

/// A small fixed roster of related motifs, grown ONCE per composition from a single germ.
///
/// The whole point of the bank is coherence: `hook` and `bass_cell` share DNA with
/// `identity` (they're fragments/transforms of it), `rhythmic_cell` is a short rhythm-forward
/// diminution, and the optional `countermotif` is a contrary-motion partner. One idea, several
/// costumes — so the piece sounds composed, not assembled.
#[derive(Debug, Clone, PartialEq)]
pub struct MotifBank {
    /// The primary idea — the germ every other member descends from.
    pub identity: Motif,
    /// A compact, front-loaded fragment of the identity for the ear to latch onto.
    pub hook: Motif,
    /// A short, quick rhythm-forward figure (a diminution of the germ's head).
    pub rhythmic_cell: Motif,
    /// The identity's opening dropped a register — a bass answer, same blood.
    pub bass_cell: Motif,
    /// An optional inverted partner for contrary motion.
    pub countermotif: Option<Motif>,
}

impl MotifBank {
    /// Generate the roster deterministically from `seed`, in the song's reference `frame`.
    ///
    /// Determinism lives entirely in `seed` (a seeded [`Rng`]); `frame` — the SONG's reference
    /// mode (Round IX: [`super::song::SongMap::frame`]), never a room's — only tilts which germ is
    /// chosen (a bright frame gets the open rising call, a darker one the restless germ), so
    /// `(frame, seed)` fully determines the bank. Every member is a scale-degree contour: a room
    /// realizes it in its own mode without choosing it. Every melodic member is non-empty by
    /// construction.
    pub fn generate(frame: Mode, seed: u64) -> MotifBank {
        let mut rng = Rng::new(seed);

        // A bright frame gets the open rising call; a darker one the more restless germ.
        let bright = matches!(frame, Mode::Ionian | Mode::Lydian | Mode::Mixolydian);
        let base = if bright {
            Motif::seed_a()
        } else {
            Motif::seed_b()
        };
        // A small tonal shift so distinct seeds land in distinct colors.
        let shift = rng.below(5) as i32 - 2; // -2..=2
        let identity = base.transpose(shift);

        // Hook: a compact head of the identity, optionally sequenced up a step.
        let span = identity.len().saturating_sub(1).max(1);
        let frag_len = 2 + rng.below(span);
        let mut hook = identity.fragment(frag_len);
        if rng.chance(0.5) {
            hook = hook.sequence(1, 2);
        }

        // Rhythmic cell: the germ's head, diminished — short and rhythm-forward.
        let rc_len = 2 + rng.below(2); // 2 or 3 notes
        let rhythmic_cell = identity.fragment(rc_len).scale_rhythm(0.5);

        // Bass cell: the opening two notes, dropped an octave in degree space.
        let bass_cell = identity.fragment(2).transpose(-7);

        // Countermotif: sometimes present — the inversion, a contrary-motion partner.
        let countermotif = if rng.chance(0.6) {
            Some(identity.invert())
        } else {
            None
        };

        MotifBank {
            identity,
            hook,
            rhythmic_cell,
            bass_cell,
            countermotif,
        }
    }
}

/// A typed **handoff** — how one lead statement connects to the next, so phrases flow as one song
/// rather than being spliced. Reported per lead-bearing boundary by the diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handoff {
    /// The thesis restated — the song comes home to M0.
    Restatement,
    /// A development of the previous statement under a named transform.
    Develop,
    /// A call posed (a fragment carried forward, awaiting its answer).
    Call,
    /// The answer completing a carried call (call/response).
    Response,
    /// The hook — the call+response DNA, the payoff.
    Hook,
    /// A closing evaporation.
    Dissolve,
    /// The thesis's consequent (Round X): its head and rhythm, coming to rest — an answer the song
    /// states AS WRITTEN (its landing is the listener event), so no performance fragments it.
    Consequent,
}

impl Handoff {
    /// A short label for provenance and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Handoff::Restatement => "restate",
            Handoff::Develop => "develop",
            Handoff::Call => "call",
            Handoff::Response => "response",
            Handoff::Hook => "hook",
            Handoff::Dissolve => "dissolve",
            // An answer, to every diagnostic that counts answers.
            Handoff::Consequent => "response",
        }
    }
}

/// The whole-piece thematic line, developed with MEMORY.
///
/// Round IV re-read a fixed germ every phrase and applied an unrelated one-shot transform per role,
/// so consecutive statements were different distortions of the same seed that did not flow into one
/// another — the "kaleidoscope of coherent fragments spliced together." The trajectory instead
/// carries the material it last produced and develops THAT (`M_{n+1} = develop(M_n)`), returns
/// explicitly to the thesis `M0` on every Restate/Return (the song comes home), and derives its
/// hook from the call+response DNA. Each step reports the [`Handoff`] connecting it to the previous
/// statement, so no boundary is an accidental teleport.
pub struct ThematicTrajectory {
    /// `M0` — the stable call; every Return comes home to it.
    thesis: Motif,
    /// The hook — the call's head fused with the response's tail (call+response DNA).
    hook: Motif,
    /// The material last produced — the memory each development grows from.
    current: Motif,
    /// A fragment split point carried from a call to its answering phrase.
    carry: Option<usize>,
}

impl ThematicTrajectory {
    /// Build the trajectory from the germ `bank`. The call is the germ; the response is the call
    /// transposed a step (same contour, its answering phrase); the hook fuses the call's head with
    /// the response's tail — one compact idea sharing both their DNA.
    pub fn new(bank: &MotifBank) -> ThematicTrajectory {
        let thesis = bank.identity.clone();
        let response = thesis.transpose(1);
        let head = (thesis.len() / 2).max(1);
        let hook = thesis.fragment(head).concat(&response.tail(head));
        ThematicTrajectory {
            current: thesis.clone(),
            thesis,
            hook,
            carry: None,
        }
    }

    /// The next statement for `role`, developed from the CURRENT material (not a fresh germ), and
    /// the [`Handoff`] describing how it connects to the previous statement.
    pub fn next_for(&mut self, role: super::discourse::DiscourseRole) -> (Motif, Handoff) {
        use super::discourse::DiscourseRole as R;
        let half = |m: &Motif| (m.len() / 2).max(1);
        match role {
            // The song comes home to the thesis — memory reset to M0.
            R::Establish | R::Restate | R::Return => {
                self.current = self.thesis.clone();
                (self.current.clone(), Handoff::Restatement)
            }
            // Develop the CURRENT material a step further — the next phrase grows from the last.
            R::Depart => {
                self.current = self.current.transpose(2);
                (self.current.clone(), Handoff::Develop)
            }
            R::Intensify => {
                self.current = self.current.scale_rhythm(0.75);
                (self.current.clone(), Handoff::Develop)
            }
            // Pose a call: a fragment of the current material, remembering where it broke off.
            R::Question => {
                let k = half(&self.current);
                self.carry = Some(k);
                (self.current.fragment(k), Handoff::Call)
            }
            R::Withhold => {
                let k = half(&self.current);
                (self.current.fragment(k), Handoff::Call)
            }
            // The payoff — the call+response hook.
            R::Culminate => (self.hook.clone(), Handoff::Hook),
            // Answer the carried call by completing exactly its withheld remainder.
            R::Answer => {
                let s = self.carry.take().unwrap_or_else(|| half(&self.current));
                (self.current.tail(s), Handoff::Response)
            }
            R::Dissolve => {
                let k = 2.min(self.current.len().max(1));
                (self.current.fragment(k), Handoff::Dissolve)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Round VII line engine: targets first, then connectors, justification inside the search.
//
// Through Round VI the realizer ran one DP over "any scale tone near the anchor" and only
// classified each note AFTER backtracking; whatever the classifier could not explain was snapped
// to a chord tone and counted as a repair. That is justification as an afterthought. Here the
// statement is first split into MelodicEvents — structural targets (strong beats, long notes, the
// first and last note, the contour peak), connective notes between them, and articulated internal
// rests. The targets are chosen jointly from each onset harmony's STABLE palette (chord tones and
// licensed tensions), preferring guide tones and colour over roots, by a small DP that passes
// through chord changes as one thought. Between each pair of candidate targets the connectors are
// searched with the classifier INSIDE the search: a connector is only admissible when
// `pitch::classify` justifies it against the exact neighbours it will actually have (passing,
// neighbour, chromatic approach, enclosure, anticipation, suspension, appoggiatura, or a chord
// tone/tension). A target pair whose gap no justified connector path can bridge is simply not a
// path. The snap-repair pass remains only as a counted last resort.
// ---------------------------------------------------------------------------

/// A realized note: `(start_beat, dur_beats, pitch, pitch-function)`.
pub type RealizedNote = (f64, f32, Midi, Option<PitchFunction>);

/// Whether a melodic event carries the line's structure or connects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    /// A structural target: strong-beat onset, long note, first/last note or the contour peak.
    /// Its pitch is chosen from the stable palette of its onset harmony.
    Structural,
    /// A connective note between targets: its pitch must be justified by the targets around it.
    Connective,
}

/// One event of a statement's melodic surface, derived from a [`Motif`] note.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MelodicEvent {
    /// Onset in beats from the statement start (identical to the motif's cumulative rhythm).
    pub onset: f64,
    /// Sounding gate in beats (shorter than the rhythmic slot when the note breathes).
    pub dur: f32,
    /// A first-class internal rest: the slot is kept in time but nothing sounds.
    pub rest: bool,
    /// Structural target or connective note.
    pub target: TargetKind,
    /// Relative accent `[0, 1]` (structural events lean, connectives recede).
    pub accent: f32,
}

/// How a line is spoken: the parts of the [`MusicalLanguage`] the line engine consumes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineStyle {
    /// Structural targets may land on licensed tensions (9/11/13), not only chord tones; the
    /// classifier is told which tensions the local palette licenses.
    pub tension_targets: bool,
    /// Appetite for chromatic connectives (approach / chromatic passing / enclosure), `[0, 1]`.
    pub chromatic_connectives: f32,
    /// Fraction of a statement's interior gaps articulated as rests, `[0, 1]`.
    pub internal_rest: f32,
    /// Gate as a fraction of the rhythmic slot.
    pub gate: f32,
    /// `true`: a structural strong beat is an even beat of the bar (downbeat / mid-bar);
    /// `false`: every integer beat is structural (the plain, pre-Round-VII grid).
    pub bar_strong_beats: bool,
}

impl LineStyle {
    /// The plain style behind [`realize_phrase`]: chord-tone targets on every integer beat, full
    /// gates, no articulated rests, chromatic connectives allowed but expensive.
    pub fn plain() -> LineStyle {
        LineStyle {
            tension_targets: false,
            chromatic_connectives: 0.0,
            internal_rest: 0.0,
            gate: 1.0,
            bar_strong_beats: false,
        }
    }

    /// The style a [`MusicalLanguage`] speaks: tension targets from colour depth, chromatic
    /// appetite and internal-rest rate from the language's melodic policy.
    pub fn for_language(lang: &MusicalLanguage) -> LineStyle {
        LineStyle {
            tension_targets: lang.color_depth > 0,
            chromatic_connectives: lang.chromatic_connectives.clamp(0.0, 1.0),
            internal_rest: lang.internal_rest.clamp(0.0, 1.0),
            gate: 0.9,
            bar_strong_beats: true,
        }
    }
}

/// True when `beat` sits on an integer-beat onset (a strong beat for the classifier).
fn is_strong_beat(beat: f64) -> bool {
    (beat - beat.round()).abs() < 1e-6
}

/// True when `beat` is an even integer beat of a 4/4 bar (downbeat or mid-bar).
fn is_bar_strong(beat: f64) -> bool {
    let b = beat.rem_euclid(BEATS_PER_BAR);
    let r = b.round();
    (b - r).abs() < 1e-6 && (r as i64).rem_euclid(2) == 0
}

/// Derive a statement's [`MelodicEvent`]s from `motif` starting at absolute `start_beat`.
///
/// Onsets are exactly the motif's (so its rhythm and identity are kept). Structural events are
/// the first and last note, strong-beat onsets, long notes (>= 1.5 beats) and the contour peak;
/// the rest connect. Internal rests are articulated deterministically at a rate of
/// `style.internal_rest` of the interior gaps: first by letting a long interior note breathe (its
/// gate halves, its onset stays), then — for statements long enough to afford it — by resting in
/// place of a short interior connective. Never the first or last event, never a structural target,
/// never two rests side by side. The choice depends only on the motif, the onset grid and the
/// style, so a restated thesis rests where the thesis rested.
pub fn melodic_events(motif: &Motif, start_beat: f64, style: &LineStyle) -> Vec<MelodicEvent> {
    let n = motif.len();
    let mut out = Vec::with_capacity(n);
    if n == 0 {
        return out;
    }
    let peak = (0..n)
        .max_by(|&a, &b| motif.degrees[a].cmp(&motif.degrees[b]).then(b.cmp(&a)))
        .unwrap_or(0);
    let mut t = 0.0f64;
    for i in 0..n {
        let r = motif.rhythm[i];
        let abs = start_beat + t;
        let strong = if style.bar_strong_beats {
            is_bar_strong(abs)
        } else {
            is_strong_beat(abs)
        };
        let structural = i == 0 || i + 1 == n || strong || r >= 1.5 || i == peak;
        out.push(MelodicEvent {
            onset: t,
            dur: (r * style.gate).max(0.1),
            rest: false,
            target: if structural {
                TargetKind::Structural
            } else {
                TargetKind::Connective
            },
            accent: if structural { 1.0 } else { 0.8 },
        });
        t += r as f64;
    }

    let budget = (style.internal_rest * n.saturating_sub(1) as f32).round() as usize;
    if n < 3 || budget == 0 {
        return out;
    }
    let mut used = 0usize;
    // 1. Breaths: long interior notes keep their onset and give back half their slot.
    let mut long: Vec<usize> = (1..n - 1).filter(|&i| motif.rhythm[i] >= 1.0).collect();
    long.sort_by(|&a, &b| motif.rhythm[b].total_cmp(&motif.rhythm[a]).then(a.cmp(&b)));
    for i in long.into_iter().take(budget) {
        out[i].dur = (motif.rhythm[i] * 0.5).max(0.5);
        used += 1;
    }
    // 2. Rests in place of a short interior connective (long statements only).
    if used < budget && n >= 6 {
        for i in 1..n - 1 {
            if used >= budget {
                break;
            }
            let ok = out[i].target == TargetKind::Connective
                && motif.rhythm[i] <= 0.5
                && !out[i - 1].rest
                && !out[i + 1].rest;
            if ok {
                out[i].rest = true;
                used += 1;
            }
        }
    }
    out
}

/// One realized note of a line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineNote {
    pub start: f64,
    pub dur: f32,
    pub pitch: Midi,
    pub function: Option<PitchFunction>,
    pub accent: f32,
    pub structural: bool,
}

/// A realized line and how many notes the last-resort snap pass had to repair (target 0).
#[derive(Debug, Clone, PartialEq)]
pub struct LineRealization {
    pub notes: Vec<LineNote>,
    pub repairs: usize,
}

/// Everything the line engine needs to realize one statement.
pub struct LineRequest<'a> {
    pub motif: &'a Motif,
    pub chords: &'a [ChordSpan],
    /// The per-chord harmonic contexts (palettes) — parallel to `chords`.
    pub contexts: &'a [HarmonicContext],
    /// The tonal region the motif's degrees are read in.
    pub scale: &'a Scale,
    pub root_degree: i32,
    pub octave: i32,
    pub start_beat: f64,
    /// The previous statement's exit pitch, for register continuity.
    pub prev_pitch: Option<Midi>,
    pub style: LineStyle,
    /// Target candidates considered per structural event.
    pub max_candidates: usize,
    /// A beat the line must ARRIVE at: the first sounding event within `[t - 0.25, t + 1)` may
    /// only be a chord tone of its onset harmony (a lead Resolve the plan handed this statement).
    /// A feasibility filter over the unconstrained candidates, not a cost: a line that already
    /// arrives is unchanged. One arrival per statement (the first Resolve whose window it sounds
    /// in); a second would go unconstrained — the witness audit reports it if it ever fails.
    pub arrival: Option<f64>,
}

/// One sounding event with its full harmonic situation, precomputed once.
struct Slot {
    start: f64,
    gate: f64,
    anchor: Midi,
    degree: i32,
    structural: bool,
    accent: f32,
    /// Integer-beat onset (what the classifier calls strong).
    strong: bool,
    /// Even beat of the bar (where a guide tone / colour matters most).
    bar_strong: bool,
    cur: Option<Chord>,
    prev_chord: Option<Chord>,
    next_chord: Option<Chord>,
    next_boundary: Option<f64>,
    /// Licensed tensions over `cur` (a pitch-class mask; 0 when the style licenses none).
    licensed: u16,
    /// Pitch classes a connector may use here (palette ∪ chord ∪ imminent next chord).
    allowed: u16,
    /// Guide tones (3rd, 7th/6th) of the onset harmony.
    guide: u16,
    /// Upcoming guide tones supplied by the shared harmonic palette.
    next_targets: u16,
    /// The line's required arrival ([`LineRequest::arrival`]): chord tones of `cur` only, filtered
    /// from the unconstrained candidates.
    arrive: bool,
}

fn has_pc(mask: u16, p: Midi) -> bool {
    mask & (1u16 << pitch_class(p)) != 0
}

fn chord_mask(c: Option<Chord>) -> u16 {
    c.map_or(0, |c| super::pitch::pc_mask(&c.pitch_classes()))
}

const INF: f32 = f32::INFINITY;
/// Target register fit (distance from the motif's anchor pitch).
const T_ANCHOR_W: f32 = 0.3;
/// Continuity from the previous statement's exit into the first target.
const FIRST_VL_W: f32 = 0.3;
/// Contradicting the motif's direction between two targets.
const T_CONTOUR_PEN: f32 = 6.0;
/// Deviation of a target-to-target interval from the motif's own interval.
const T_FID_W: f32 = 0.1;
/// Connector register fit.
const C_ANCHOR_W: f32 = 0.15;
/// Contradicting the motif's direction on one event-to-event step.
const C_CONTOUR_PEN: f32 = 3.0;
/// Deviation of one step from the motif's own interval.
const C_FID_W: f32 = 0.08;
/// Connector candidates per event.
const CONN_CAP: usize = 12;

/// A soft cost for structural colour without a nearby core-tone destination.
/// Callers supply the actual adjacent events; a scale or function label is not proof.
pub(super) fn extension_path_cost(
    source: Chord,
    pitch: Midi,
    destination: Chord,
    next_pitch: Midi,
    elapsed: f64,
) -> f32 {
    if source.contains_pc(pitch_class(pitch)) {
        return 0.0;
    }
    let owned = elapsed <= 2.0 + 1e-6
        && (next_pitch - pitch).abs() <= 2
        && destination.contains_pc(pitch_class(next_pitch));
    if owned {
        0.0
    } else {
        1.4
    }
}

impl<'a> LineRequest<'a> {
    fn slots(&self, events: &[MelodicEvent]) -> Vec<Slot> {
        let mut out = Vec::new();
        let mut arrival = self.arrival;
        for (i, ev) in events.iter().enumerate() {
            if ev.rest {
                continue;
            }
            let start = self.start_beat + ev.onset;
            let arrive = arrival.is_some_and(|t| start >= t - 0.25 && start < t + 1.0);
            if arrive {
                arrival = None;
            }
            let deg = self.motif.degrees[i];
            let cur = chord_at(self.chords, start);
            let next_boundary = next_boundary_after(self.chords, start);
            let next_chord = next_boundary.and_then(|b| chord_at(self.chords, b));
            let prev_chord = cur_span_start(self.chords, start)
                .filter(|&cs| cs > 1e-9)
                .and_then(|cs| chord_at(self.chords, cs - 1e-3));
            let ctx = super::context::context_at(self.contexts, start).or(self.contexts.first());
            let cur_mask = chord_mask(cur);
            let licensed = if self.style.tension_targets {
                ctx.map_or(0, |c| super::pitch::pc_mask(&c.palette.tensions)) & !cur_mask
            } else {
                0
            };
            let palette_mask = ctx.map_or_else(
                || {
                    super::pitch::pc_mask(
                        &(0..12)
                            .filter(|&pc| self.scale.contains_pc(pc))
                            .collect::<Vec<_>>(),
                    )
                },
                |c| super::pitch::pc_mask(&c.palette.all()),
            );
            let imminent = match next_boundary {
                Some(b) if b - start <= super::pitch::ANTICIPATION_WINDOW + 1e-6 => {
                    chord_mask(next_chord)
                }
                _ => 0,
            };
            let guide = cur.map_or(0, |c| {
                super::pitch::pc_mask(&super::context::guide_tones(&c))
            });
            out.push(Slot {
                start,
                gate: ev.dur as f64,
                anchor: self.scale.degree_pitch(self.root_degree + deg, self.octave),
                degree: deg,
                structural: ev.target == TargetKind::Structural,
                accent: ev.accent,
                strong: is_strong_beat(start),
                bar_strong: is_bar_strong(start),
                cur,
                prev_chord,
                next_chord,
                next_boundary,
                licensed,
                allowed: palette_mask | cur_mask | imminent,
                guide,
                next_targets: ctx.map_or(0, |c| super::pitch::pc_mask(&c.palette.next_targets)),
                arrive,
            });
        }
        out
    }
}

/// The line engine over precomputed slots.
struct Engine<'s> {
    slots: &'s [Slot],
    scale: Scale,
    style: LineStyle,
    prev_pitch: Option<Midi>,
    /// The R11 rhythm/register scaffold. Presence enables temporal pitch selection.
    legacy: Option<&'s [LineNote]>,
}

impl Engine<'_> {
    fn same_gate(&self, s: usize, p: Midi) -> bool {
        self.legacy
            .is_none_or(|line| (self.gate_for(s, p) as f32 - line[s].dur).abs() < 1e-6)
    }

    fn edit_cost(&self, s: usize, p: Midi) -> f32 {
        self.legacy
            .map_or(0.0, |line| if line[s].pitch == p { 0.0 } else { 0.2 })
    }

    /// The sounding gate of pitch `p` at slot `s`: the event's gate, released at the next harmony
    /// change when `p` does not belong to that harmony (a note lifts off rather than smearing).
    fn gate_for(&self, s: usize, p: Midi) -> f64 {
        let sl = &self.slots[s];
        match (sl.next_boundary, sl.next_chord) {
            (Some(b), Some(nc))
                if sl.start + sl.gate > b + 1e-6 && !has_pc(chord_mask(Some(nc)), p) =>
            {
                ((b - sl.start) * 0.97).max(0.1)
            }
            _ => sl.gate,
        }
    }

    /// Classify pitch `p` at slot `s` with the exact neighbours it will have.
    fn classify(
        &self,
        s: usize,
        p: Midi,
        prev: Option<Midi>,
        next: Option<Midi>,
    ) -> Option<PitchFunction> {
        let sl = &self.slots[s];
        let ctx = super::pitch::PitchContext {
            pitch: p,
            onset: sl.start,
            duration: self.gate_for(s, p),
            prev,
            next,
            next_onset: self.slots.get(s + 1).map(|slot| slot.start),
            prev_chord: sl.prev_chord,
            cur: sl.cur,
            next_chord: sl.next_chord,
            next_boundary: sl.next_boundary,
            is_strong: sl.strong,
            licensed: sl.licensed,
        };
        if self.legacy.is_some() {
            super::pitch::classify(&ctx, &self.scale)
        } else {
            super::pitch::classify_r11(&ctx, &self.scale)
        }
    }

    fn is_stable(&self, s: usize, p: Midi) -> bool {
        let sl = &self.slots[s];
        has_pc(chord_mask(sl.cur), p) || has_pc(sl.licensed, p)
    }

    /// Up to `cap` stable target candidates near the anchor, closest first. Never empty. At the
    /// line's required arrival (`Slot::arrive`) only the chord tones AMONG those candidates stay:
    /// a filter over the unconstrained set, so a line whose optimum already arrives is unchanged
    /// (its choice survives the filter and nothing new enters). Only when no candidate is a chord
    /// tone does the arrival fall back to its harmony's nearest chord tone.
    fn target_cands(&self, s: usize, cap: usize) -> Vec<Midi> {
        let a = self.slots[s].anchor;
        let mut v = Vec::new();
        'outer: for d in 0..=7 {
            for m in [a - d, a + d] {
                if !v.contains(&m)
                    && self.is_stable(s, m)
                    && self.classify(s, m, None, None).is_some()
                    && self.same_gate(s, m)
                {
                    v.push(m);
                    if v.len() >= cap {
                        break 'outer;
                    }
                }
            }
        }
        let sl = &self.slots[s];
        if sl.arrive {
            v.retain(|&m| has_pc(chord_mask(sl.cur), m));
        }
        if v.is_empty() {
            let fallback = if self.legacy.is_none() {
                nearest_chord_tone(a, sl.cur, &self.scale)
            } else {
                (0..=24)
                    .flat_map(|d| [a - d, a + d])
                    .find(|&p| has_pc(chord_mask(sl.cur), p) && self.same_gate(s, p))
                    .unwrap_or_else(|| nearest_chord_tone(a, sl.cur, &self.scale))
            };
            v.push(fallback);
        }
        v
    }

    /// Which chord member / tension a target is, as a preference cost (lower = preferred): guide
    /// tones and colour over the plain root and fifth, most strongly on the bar's strong beats.
    fn tone_pref(&self, s: usize, p: Midi, is_last: bool) -> f32 {
        let sl = &self.slots[s];
        let Some(ch) = sl.cur else {
            return 0.0;
        };
        let rel = (pitch_class(p) - ch.root_pc).rem_euclid(12);
        let base = if rel == 0 {
            if is_last {
                0.3
            } else {
                0.7
            }
        } else if has_pc(sl.guide, p) {
            0.0
        } else if has_pc(chord_mask(sl.cur), p) {
            if rel == 7 {
                0.4
            } else {
                0.1
            }
        } else {
            0.12
        };
        base * if sl.bar_strong { 1.0 } else { 0.5 }
    }

    fn target_node(&self, s: usize, p: Midi, is_first: bool, is_last: bool) -> f32 {
        let sl = &self.slots[s];
        let mut c = T_ANCHOR_W * (p - sl.anchor).abs() as f32 + self.tone_pref(s, p, is_last);
        if self.legacy.is_some() {
            c += self.edit_cost(s, p);
            if is_last && !has_pc(chord_mask(sl.cur), p) {
                c += 1.1;
            }
            if sl.next_boundary.is_some_and(|b| b - sl.start <= 2.0 + 1e-6) && sl.next_targets != 0
            {
                let distance = (0..=6)
                    .find(|&d| has_pc(sl.next_targets, p - d) || has_pc(sl.next_targets, p + d))
                    .unwrap_or(6);
                c += 0.18 * distance as f32;
            }
        }
        if is_first {
            if let Some(pp) = self.prev_pitch {
                c += FIRST_VL_W * (p - pp).abs() as f32;
            }
        }
        c
    }

    fn target_trans(&self, a: usize, pa: Midi, b: usize, pb: Midi) -> f32 {
        let (sa, sb) = (&self.slots[a], &self.slots[b]);
        let dir = (sb.degree - sa.degree).signum();
        let mv = pb - pa;
        let mut c = 0.0;
        if dir == 0 {
            if mv != 0 {
                c += 1.0 + 0.1 * mv.abs() as f32;
            }
        } else if mv.signum() != dir {
            c += T_CONTOUR_PEN;
        }
        c += T_FID_W * (mv - (sb.anchor - sa.anchor)).abs() as f32;
        if mv.abs() > 9 {
            c += 0.3 * (mv.abs() - 9) as f32;
        }
        if self.legacy.is_some() {
            // With connectors, their actual adjacent steps carry the ownership cost instead.
            if b == a + 1 {
                c += self.path_step(a, pa, b, pb);
            }
            if sa.cur != sb.cur && mv.abs() > 5 {
                c += 0.12 * (mv.abs() - 5) as f32;
            }
        }
        c
    }

    fn path_step(&self, a: usize, pa: Midi, b: usize, pb: Midi) -> f32 {
        let (sa, sb) = (&self.slots[a], &self.slots[b]);
        if !sa.structural || has_pc(chord_mask(sa.cur), pa) {
            return 0.0;
        }
        match (sa.cur, sb.cur) {
            (Some(source), Some(destination)) => {
                extension_path_cost(source, pa, destination, pb, sb.start - sa.start)
            }
            _ => 0.0,
        }
    }

    /// Connector register fit, loosened by the language's connective appetite: a line that
    /// decorates its approaches may stray further from the motif's literal pitch between targets
    /// (the direction of every step is still held by the contour penalty).
    fn c_anchor_w(&self) -> f32 {
        C_ANCHOR_W * (1.0 - self.style.chromatic_connectives)
    }

    /// Connector interval fidelity, loosened the same way.
    fn c_fid_w(&self) -> f32 {
        C_FID_W * (1.0 - self.style.chromatic_connectives)
    }

    /// Cost of one event-to-event step `x -> y` against the motif's own step.
    fn step(&self, x: usize, px: Midi, y: usize, py: Midi) -> f32 {
        let (sx, sy) = (&self.slots[x], &self.slots[y]);
        let dir = (sy.degree - sx.degree).signum();
        let mv = py - px;
        let mut c = 0.0;
        if dir == 0 {
            if mv != 0 {
                c += 0.6 + 0.05 * mv.abs() as f32;
            }
        } else if mv.signum() != dir {
            c += C_CONTOUR_PEN;
        }
        c + self.c_fid_w() * (mv - (sy.anchor - sx.anchor)).abs() as f32
            + if self.legacy.is_some() {
                self.path_step(x, px, y, py)
            } else {
                0.0
            }
    }

    /// The cost of a connector's justification. A connector's job is to connect: motion that
    /// does (passing, neighbour, approach, enclosure) is preferred to re-arpeggiating a stable
    /// tone, and the more connective the language, the stronger that preference; chromatic
    /// motion is priced by the language's appetite for it.
    fn func_cost(&self, f: PitchFunction) -> f32 {
        let c = self.style.chromatic_connectives;
        let arpeggiate = 0.4 + 0.8 * c;
        let chroma = 0.55 - 0.6 * c;
        match f {
            PitchFunction::ChordTone => arpeggiate,
            PitchFunction::LicensedExtension => arpeggiate - 0.1,
            PitchFunction::DiatonicPassing => 0.0,
            PitchFunction::Neighbor => 0.1,
            PitchFunction::Suspension => 0.15,
            PitchFunction::Anticipation => 0.2,
            PitchFunction::Appoggiatura => 0.35,
            PitchFunction::ChromaticPassing
            | PitchFunction::ChromaticApproach
            | PitchFunction::Enclosure => chroma,
            _ => 0.4,
        }
    }

    /// Connector candidates at slot `s` between targets `pa` and `pb`: palette tones in the
    /// segment's window, plus the chromatic semitones next to either target.
    fn connector_cands(&self, s: usize, pa: Midi, pb: Midi) -> Vec<Midi> {
        let sl = &self.slots[s];
        let lo = (pa.min(pb) - 3).min(sl.anchor - 2);
        let hi = (pa.max(pb) + 3).max(sl.anchor + 2);
        let mut v: Vec<Midi> = (lo..=hi)
            .filter(|&p| has_pc(sl.allowed, p) || (p - pb).abs() == 1 || (p - pa).abs() == 1)
            .collect();
        v.sort_by_key(|&p| ((p - sl.anchor).abs(), p));
        v.truncate(CONN_CAP);
        // The required arrival keeps only the chord tones among the (capped) candidates — a filter
        // over the unconstrained set, never a door for candidates the cap excluded. Only when none
        // of them is a chord tone does it reach for the harmony's chord tones in the window.
        if sl.arrive {
            let chord = chord_mask(sl.cur);
            v.retain(|&p| has_pc(chord, p));
            if v.is_empty() {
                v = (lo..=hi).filter(|&p| has_pc(chord, p)).collect();
                v.sort_by_key(|&p| ((p - sl.anchor).abs(), p));
                v.truncate(CONN_CAP);
            }
        }
        v.retain(|&p| self.same_gate(s, p));
        v
    }

    /// The cheapest justified connector path between target `pa` at slot `ia` and `pb` at slot
    /// `ib` (every connector classified against its real neighbours), or `None` if none exists.
    fn connect(
        &self,
        ia: usize,
        pa: Midi,
        ib: usize,
        pb: Midi,
    ) -> Option<(f32, Vec<(Midi, PitchFunction)>)> {
        let k = ib - ia - 1;
        if k == 0 {
            return Some((0.0, Vec::new()));
        }
        let mut cands: Vec<Vec<Midi>> = Vec::with_capacity(k + 2);
        cands.push(vec![pa]);
        for s in ia + 1..ib {
            cands.push(self.connector_cands(s, pa, pb));
        }
        cands.push(vec![pb]);
        let slot = |pos: usize| ia + pos;

        // dp[pos][i * |cands[pos]| + j]: least cost with cands[pos-1][i] then cands[pos][j].
        let mut dp: Vec<Vec<f32>> = vec![Vec::new(); k + 2];
        let mut bp: Vec<Vec<usize>> = vec![Vec::new(); k + 2];
        dp[1] = cands[1]
            .iter()
            .map(|&p| {
                self.c_anchor_w() * (p - self.slots[slot(1)].anchor).abs() as f32
                    + self.edit_cost(slot(1), p)
                    + self.step(slot(0), pa, slot(1), p)
            })
            .collect();
        bp[1] = vec![0; cands[1].len()];
        for pos in 2..=k + 1 {
            let (np, nc, nn) = (cands[pos - 2].len(), cands[pos - 1].len(), cands[pos].len());
            let mut d = vec![INF; nc * nn];
            let mut b = vec![0usize; nc * nn];
            for j in 0..nc {
                let pj = cands[pos - 1][j];
                for l in 0..nn {
                    let pl = cands[pos][l];
                    let mut best = INF;
                    let mut bi = 0;
                    for i in 0..np {
                        let prev_cost = dp[pos - 1][i * nc + j];
                        if !prev_cost.is_finite() || prev_cost >= best {
                            continue;
                        }
                        let pi = cands[pos - 2][i];
                        let Some(f) = self.classify(slot(pos - 1), pj, Some(pi), Some(pl)) else {
                            continue;
                        };
                        let tot = prev_cost + self.func_cost(f);
                        if tot < best {
                            best = tot;
                            bi = i;
                        }
                    }
                    if best.is_finite() {
                        let node = if pos == k + 1 {
                            0.0
                        } else {
                            self.c_anchor_w() * (pl - self.slots[slot(pos)].anchor).abs() as f32
                                + self.edit_cost(slot(pos), pl)
                        };
                        d[j * nn + l] = best + node + self.step(slot(pos - 1), pj, slot(pos), pl);
                        b[j * nn + l] = bi;
                    }
                }
            }
            dp[pos] = d;
            bp[pos] = b;
        }

        // Best final pair (connector k, target b).
        let last = k + 1;
        let (mut j, mut best) = (0usize, INF);
        for (jj, &v) in dp[last].iter().enumerate() {
            if v < best {
                best = v;
                j = jj;
            }
        }
        if !best.is_finite() {
            return None;
        }
        // Backtrack indices: idx[pos] for pos in 0..=last.
        let mut idx = vec![0usize; last + 1];
        idx[last] = 0;
        idx[last - 1] = j;
        for pos in (2..=last).rev() {
            let nn = cands[pos].len();
            idx[pos - 2] = bp[pos][idx[pos - 1] * nn + idx[pos]];
        }
        let pitches: Vec<Midi> = (0..=last).map(|pos| cands[pos][idx[pos]]).collect();
        let mut path = Vec::with_capacity(k);
        for pos in 1..=k {
            let f = self.classify(
                slot(pos),
                pitches[pos],
                Some(pitches[pos - 1]),
                Some(pitches[pos + 1]),
            )?;
            path.push((pitches[pos], f));
        }
        Some((best, path))
    }
}

/// Realize one statement: targets first, then justified connectors. Deterministic — no RNG; ties
/// resolve to the candidate nearest the motif's anchor.
pub fn realize_line(req: &LineRequest) -> LineRealization {
    realize_line_impl(req, None)
}

/// Realize a statement with bounded harmonic direction and extension ownership costs.
/// The R11 realization supplies exactly the same events and sounding gates; only pitch
/// candidates with those gates participate, and all melodic contour costs remain active.
pub fn realize_line_temporal(req: &LineRequest) -> LineRealization {
    let legacy = realize_line(req);
    realize_line_impl(req, Some(&legacy.notes))
}

fn realize_line_impl(req: &LineRequest, legacy: Option<&[LineNote]>) -> LineRealization {
    let events = melodic_events(req.motif, req.start_beat, &req.style);
    let slots = req.slots(&events);
    let n = slots.len();
    if n == 0 {
        return LineRealization {
            notes: Vec::new(),
            repairs: 0,
        };
    }
    let eng = Engine {
        slots: &slots,
        scale: *req.scale,
        style: req.style,
        prev_pitch: req.prev_pitch,
        legacy,
    };
    let cap = req.max_candidates.max(1);

    // The structural targets (the first and last sounding events always are).
    let mut tslots: Vec<usize> = (0..n).filter(|&s| slots[s].structural).collect();
    if tslots.first() != Some(&0) {
        tslots.insert(0, 0);
    }
    if tslots.last() != Some(&(n - 1)) {
        tslots.push(n - 1);
    }
    let m = tslots.len();
    let tc: Vec<Vec<Midi>> = tslots.iter().map(|&s| eng.target_cands(s, cap)).collect();

    // Outer DP over targets; each transition carries its cheapest justified connector path.
    type Conn = Vec<(Midi, PitchFunction)>;
    let mut cost: Vec<Vec<f32>> = Vec::with_capacity(m);
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(m);
    let mut conns: Vec<Vec<Conn>> = Vec::with_capacity(m);
    for t in 0..m {
        let s = tslots[t];
        let is_last = t + 1 == m;
        let mut ct = vec![INF; tc[t].len()];
        let mut bt = vec![0usize; tc[t].len()];
        let mut cn: Vec<Conn> = vec![Vec::new(); tc[t].len()];
        for (c, &p) in tc[t].iter().enumerate() {
            let node = eng.target_node(s, p, t == 0, is_last);
            if t == 0 {
                ct[c] = node;
                continue;
            }
            let ps = tslots[t - 1];
            for (pc, &pp) in tc[t - 1].iter().enumerate() {
                let base = cost[t - 1][pc];
                if !base.is_finite() {
                    continue;
                }
                let tr = base + eng.target_trans(ps, pp, s, p);
                if tr >= ct[c] {
                    continue;
                }
                if let Some((cc, path)) = eng.connect(ps, pp, s, p) {
                    let tot = tr + cc + node;
                    if tot < ct[c] {
                        ct[c] = tot;
                        bt[c] = pc;
                        cn[c] = path;
                    }
                }
            }
        }
        cost.push(ct);
        back.push(bt);
        conns.push(cn);
    }

    let mut pitches: Vec<Midi> = slots.iter().map(|s| s.anchor).collect();
    let mut idx = 0usize;
    let mut best = INF;
    for (c, &v) in cost[m - 1].iter().enumerate() {
        if v < best {
            best = v;
            idx = c;
        }
    }
    if best.is_finite() {
        let mut c = idx;
        for t in (0..m).rev() {
            pitches[tslots[t]] = tc[t][c];
            if t > 0 {
                for (off, &(p, _)) in conns[t][c].iter().enumerate() {
                    pitches[tslots[t - 1] + 1 + off] = p;
                }
                c = back[t][c];
            }
        }
    } else {
        // No justified path at all (should not happen: chord tones always bridge). Fall back to
        // the nearest stable tone per event — every such note was placed outside the search, so
        // each one counts as a repair.
        for (s, p) in pitches.iter_mut().enumerate() {
            *p = eng.target_cands(s, 1)[0];
        }
    }
    let fallback = if best.is_finite() { 0 } else { n };
    let searched = pitches.clone();

    // Verify every note against its actual neighbours — the same classifier, the same context.
    let judge = |pitches: &[Midi], s: usize| {
        eng.classify(
            s,
            pitches[s],
            s.checked_sub(1).map(|j| pitches[j]),
            pitches.get(s + 1).copied(),
        )
    };
    // Last resort only: snap an unjustified note to the nearest tone of its chord (counted).
    for _ in 0..4 {
        let mut changed = false;
        for s in 0..n {
            if judge(&pitches, s).is_none() {
                let fixed = nearest_chord_tone(pitches[s], slots[s].cur, req.scale);
                if fixed != pitches[s] {
                    pitches[s] = fixed;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let repairs = fallback.max((0..n).filter(|&s| pitches[s] != searched[s]).count());
    let notes = (0..n)
        .map(|s| LineNote {
            start: slots[s].start,
            dur: eng.gate_for(s, pitches[s]) as f32,
            pitch: pitches[s],
            function: judge(&pitches, s),
            accent: slots[s].accent,
            structural: slots[s].structural,
        })
        .collect();
    LineRealization { notes, repairs }
}

/// Realize a WHOLE motif statement JOINTLY against the harmony over its span, in the plain
/// [`LineStyle`]: chord-tone targets on every integer beat, justified connectors between them.
///
/// Output mirrors [`Motif::render`]'s timing and pitch and adds a per-note [`PitchFunction`]
/// classification. Durations come from `motif.rhythm` (released at a harmony change the pitch
/// does not belong to), start times accumulate from `start_beat`. `max_candidates` bounds the
/// target candidates per structural note. Deterministic — no RNG.
#[allow(clippy::too_many_arguments)]
pub fn realize_phrase(
    motif: &Motif,
    chords: &[ChordSpan],
    scale: &Scale,
    root_degree: i32,
    octave: i32,
    start_beat: f64,
    prev_pitch: Option<Midi>,
    max_candidates: usize,
) -> Vec<RealizedNote> {
    realize_phrase_reporting(
        motif,
        chords,
        scale,
        root_degree,
        octave,
        start_beat,
        prev_pitch,
        max_candidates,
    )
    .0
}

/// Like [`realize_phrase`], additionally returning `repairs_performed`: how many notes the
/// last-resort snap pass had to fix because no justified path was found. Target: zero — the
/// search chooses justified tension instead of manufacturing a wrong note and repairing it.
#[allow(clippy::too_many_arguments)]
pub fn realize_phrase_reporting(
    motif: &Motif,
    chords: &[ChordSpan],
    scale: &Scale,
    root_degree: i32,
    octave: i32,
    start_beat: f64,
    prev_pitch: Option<Midi>,
    max_candidates: usize,
) -> (Vec<RealizedNote>, usize) {
    let contexts = super::context::analyze(chords, scale);
    let r = realize_line(&LineRequest {
        arrival: None,
        motif,
        chords,
        contexts: &contexts,
        scale,
        root_degree,
        octave,
        start_beat,
        prev_pitch,
        style: LineStyle::plain(),
        max_candidates,
    });
    (
        r.notes
            .iter()
            .map(|n| (n.start, n.dur, n.pitch, n.function))
            .collect(),
        r.repairs,
    )
}

/// The start beat of the first chord span that begins strictly after `beat` — the next harmonic
/// boundary — if any.
fn next_boundary_after(chords: &[ChordSpan], beat: f64) -> Option<f64> {
    chords
        .iter()
        .map(|s| s.start_beat)
        .filter(|&b| b > beat + 1e-6)
        .min_by(|a, b| a.total_cmp(b))
}

/// The start beat of the span sounding at `beat` (the last span starting at or before it).
fn cur_span_start(chords: &[ChordSpan], beat: f64) -> Option<f64> {
    chords
        .iter()
        .map(|s| s.start_beat)
        .filter(|&b| b <= beat + 1e-9)
        .max_by(|a, b| a.total_cmp(b))
}

/// The chord sounding at `beat`: the last span whose `start_beat <= beat`, else the first.
fn chord_at(chords: &[ChordSpan], beat: f64) -> Option<Chord> {
    let mut best: Option<&ChordSpan> = None;
    for s in chords {
        if s.start_beat <= beat + 1e-9 && best.is_none_or(|b| s.start_beat >= b.start_beat) {
            best = Some(s);
        }
    }
    best.or_else(|| chords.first()).map(|s| s.chord)
}

/// The pitch nearest `p` whose pitch-class belongs to `chord` (falling back to the nearest scale
/// pitch when there is no chord). Used to repair an unjustified note to a consonant chord tone.
fn nearest_chord_tone(p: Midi, chord: Option<Chord>, scale: &Scale) -> Midi {
    match chord {
        Some(c) => {
            let pcs = c.pitch_classes();
            (p - 6..=p + 6)
                .filter(|m| pcs.contains(&pitch_class(*m)))
                .min_by_key(|m| (m - p).abs())
                .unwrap_or_else(|| scale.nearest_scale_pitch(p))
        }
        None => scale.nearest_scale_pitch(p),
    }
}

#[cfg(test)]
mod tests {
    use super::super::theory::{Function, Quality};
    use super::*;

    #[test]
    fn transformations_preserve_identity() {
        let m = Motif::seed_a();
        assert_eq!(m.transpose(3).id, m.id);
        assert_eq!(m.invert().id, m.id);
        assert_eq!(m.retrograde().id, m.id);
        assert_eq!(m.scale_rhythm(2.0).id, m.id);
        assert_eq!(m.fragment(2).id, m.id);
        assert_eq!(m.sequence(2, 3).id, m.id);
    }

    #[test]
    fn transpose_shifts_all_degrees() {
        let m = Motif::seed_a();
        let t = m.transpose(2);
        assert_eq!(t.degrees, vec![2, 6, 5, 7, 4]);
        assert_eq!(t.rhythm, m.rhythm); // rhythm unchanged
    }

    #[test]
    fn invert_negates_intervals_about_first() {
        let m = Motif {
            id: 9,
            degrees: vec![0, 2, 4],
            rhythm: vec![1.0, 1.0, 1.0],
        };
        // pivot 0: 0-> 0, 2-> -2, 4-> -4.
        assert_eq!(m.invert().degrees, vec![0, -2, -4]);
    }

    #[test]
    fn fragment_then_tail_reconstructs_the_motif() {
        // The question (fragment) and its answer (tail) partition the motif: a Question phrase
        // states the first k notes, and the later Answer completes the withheld remainder.
        let m = Motif::seed_a();
        let k = 2;
        let q = m.fragment(k);
        let a = m.tail(k);
        assert_eq!(q.degrees.len(), k);
        assert_eq!([q.degrees.clone(), a.degrees.clone()].concat(), m.degrees);
        assert_eq!([q.rhythm.clone(), a.rhythm.clone()].concat(), m.rhythm);
        // tail always leaves at least one note, even for an over-long skip.
        assert!(!m.tail(999).degrees.is_empty());
    }

    #[test]
    fn augment_doubles_duration_diminish_halves() {
        let m = Motif::seed_a();
        assert!((m.scale_rhythm(2.0).total_beats() - m.total_beats() * 2.0).abs() < 1e-5);
        assert!((m.scale_rhythm(0.5).total_beats() - m.total_beats() * 0.5).abs() < 1e-5);
    }

    #[test]
    fn sequence_length_and_transposition() {
        let m = Motif::seed_a();
        let s = m.sequence(2, 3);
        assert_eq!(s.len(), m.len() * 3);
        // Third copy's first degree is original + 2*2.
        assert_eq!(s.degrees[m.len() * 2], m.degrees[0] + 4);
    }

    #[test]
    fn render_places_notes_in_time_and_pitch() {
        let m = Motif::seed_a();
        let s = Scale::new(0, Mode::Ionian);
        let notes = m.render(&s, 0, 4, 8.0);
        assert_eq!(notes.len(), 5);
        assert_eq!(notes[0].0, 8.0); // first at start
        assert_eq!(notes[0].2, 60); // degree 0 -> C4
        assert_eq!(notes[1].0, 8.5); // after an eighth-note pickup
        assert_eq!(notes[1].2, 67); // degree 4 -> G4 (0-based scale degree 4 = 5th tone)
        assert_eq!(notes[2].2, 65); // degree 3 -> F4
    }

    // --- Round II motif toolkit ---------------------------------------------

    #[test]
    fn similarity_is_perfect_with_self() {
        let id = Motif::seed_a().identity();
        assert_eq!(motif_similarity(&id, &id), 1.0);
    }

    #[test]
    fn similarity_is_transposition_invariant() {
        let a = Motif::seed_a().identity();
        let b = Motif::seed_a().transpose(3).identity();
        assert!(motif_similarity(&a, &b) >= 0.9);
    }

    #[test]
    fn similarity_is_tempo_invariant() {
        let a = Motif::seed_a().identity();
        let b = Motif::seed_a().scale_rhythm(0.5).identity();
        assert!(motif_similarity(&a, &b) >= 0.9);
    }

    #[test]
    fn similarity_separates_a_different_contour() {
        let a = Motif::seed_a().identity();
        let other = Motif {
            id: 0,
            degrees: vec![0, -3, -1, -6],
            rhythm: vec![1.0, 0.5, 0.5, 2.0],
        }
        .identity();
        assert!(motif_similarity(&a, &other) < 0.6);
    }

    #[test]
    fn motif_bank_is_deterministic_and_populated() {
        let a = MotifBank::generate(Mode::Ionian, 7);
        let b = MotifBank::generate(Mode::Ionian, 7);
        assert_eq!(a, b); // deterministic in (frame, seed)
        assert!(!a.identity.is_empty());
        assert!(!a.hook.is_empty());
        assert!(!a.rhythmic_cell.is_empty());
        assert!(!a.bass_cell.is_empty());
        if let Some(cm) = &a.countermotif {
            assert!(!cm.is_empty());
        }
    }

    // A single C-major span covering seed_a's whole four-beat statement.
    fn c_major_span() -> ChordSpan {
        ChordSpan {
            start_beat: 0.0,
            dur_beats: 4.0,
            chord: Chord::new(0, Quality::Maj),
            function: Function::Tonic,
            degree: 0,
            note: "I",
        }
    }

    #[test]
    fn realize_phrase_puts_chord_tones_on_strong_beats() {
        let m = Motif::seed_a();
        let s = Scale::new(0, Mode::Ionian);
        let chords = [c_major_span()];
        let out = realize_phrase(&m, &chords, &s, 0, 4, 0.0, None, 4);

        assert_eq!(out.len(), 5);
        // Timings mirror Motif::render exactly (accumulated from the syncopated rhythm).
        let mut expect = 0.0;
        for (i, (start, dur, _, _)) in out.iter().enumerate() {
            assert_eq!(*dur, m.rhythm[i]);
            assert!((*start - expect).abs() < 1e-9);
            expect += m.rhythm[i] as f64;
        }
        // Every integer-beat onset lands on a C-major tone {0,4,7}.
        for (start, _, pitch, _) in &out {
            if (start - start.round()).abs() < 1e-6 {
                let pc = pitch.rem_euclid(12);
                assert!(pc == 0 || pc == 4 || pc == 7, "pc {pc} off the chord");
            }
        }
    }

    #[test]
    fn realize_phrase_preserves_contour_direction() {
        let m = Motif::seed_a(); // degrees 0,2,4,3 -> directions +, +, -
        let s = Scale::new(0, Mode::Ionian);
        let chords = [c_major_span()];
        let out = realize_phrase(&m, &chords, &s, 0, 4, 0.0, None, 4);

        let want: Vec<i32> = m
            .degrees
            .windows(2)
            .map(|w| (w[1] - w[0]).signum())
            .collect();
        let got: Vec<i32> = out.windows(2).map(|w| (w[1].2 - w[0].2).signum()).collect();
        let agree = want.iter().zip(got.iter()).filter(|(a, b)| a == b).count();
        assert!(agree >= 2, "contour agreement {agree}/3 too low");
    }

    #[test]
    fn realize_phrase_is_deterministic() {
        let m = Motif::seed_a();
        let s = Scale::new(0, Mode::Ionian);
        let chords = [c_major_span()];
        let a = realize_phrase(&m, &chords, &s, 0, 4, 0.0, None, 4);
        let b = realize_phrase(&m, &chords, &s, 0, 4, 0.0, None, 4);
        assert_eq!(a, b);
    }

    // --- Round VII line engine ----------------------------------------------

    fn span(start: f64, dur: f32, chord: Chord) -> ChordSpan {
        ChordSpan {
            start_beat: start,
            dur_beats: dur,
            chord,
            function: Function::Tonic,
            degree: 0,
            note: "",
        }
    }

    /// A ii–V–I in C, one chord per two beats, as spans + contexts.
    fn two_five_one() -> (Vec<ChordSpan>, Vec<HarmonicContext>, Scale) {
        let s = Scale::new(0, Mode::Ionian);
        let chords = vec![
            span(0.0, 2.0, Chord::new(2, Quality::Min7)),
            span(2.0, 2.0, Chord::new(7, Quality::Dom7)),
            span(4.0, 4.0, Chord::new(0, Quality::Maj7)),
        ];
        let ctx = super::super::context::analyze(&chords, &s);
        (chords, ctx, s)
    }

    fn fusion_request<'a>(
        motif: &'a Motif,
        chords: &'a [ChordSpan],
        contexts: &'a [HarmonicContext],
        scale: &'a Scale,
    ) -> LineRequest<'a> {
        LineRequest {
            arrival: None,
            motif,
            chords,
            contexts,
            scale,
            root_degree: 0,
            octave: 4,
            start_beat: 0.0,
            prev_pitch: None,
            style: LineStyle::for_language(&MusicalLanguage::fusion_conversation()),
            max_candidates: 6,
        }
    }

    #[test]
    fn events_keep_the_motif_rhythm_and_rest_only_in_the_interior() {
        let m = Motif::seed_b(); // six notes: 0.5 x4, 1.0 x2
        let style = LineStyle::for_language(&MusicalLanguage::fusion_conversation());
        let ev = melodic_events(&m, 0.0, &style);
        assert_eq!(ev.len(), m.len());
        let mut t = 0.0;
        for (e, r) in ev.iter().zip(&m.rhythm) {
            assert!((e.onset - t).abs() < 1e-9, "onsets are the motif's");
            assert!(e.dur <= *r + 1e-6);
            t += *r as f64;
        }
        assert_eq!(ev[0].target, TargetKind::Structural);
        assert_eq!(ev[ev.len() - 1].target, TargetKind::Structural);
        assert!(!ev[0].rest && !ev[ev.len() - 1].rest);
        for e in &ev {
            assert!(!(e.rest && e.target == TargetKind::Structural));
        }
        // At the fusion rate at least one interior gap is articulated (a breath or a rest).
        let articulated = ev
            .iter()
            .zip(&m.rhythm)
            .filter(|(e, r)| e.rest || (**r - e.dur) >= 0.25)
            .count();
        assert!(articulated >= 1, "no internal rest articulated: {ev:?}");
        // The plain style articulates none and keeps full gates.
        let plain = melodic_events(&m, 0.0, &LineStyle::plain());
        assert!(plain
            .iter()
            .zip(&m.rhythm)
            .all(|(e, r)| !e.rest && (e.dur - r).abs() < 1e-6));
    }

    #[test]
    fn a_line_through_changes_needs_no_repairs_and_every_note_is_justified() {
        let (chords, ctx, s) = two_five_one();
        for motif in [
            Motif::seed_a(),
            Motif::seed_b(),
            Motif::seed_b().transpose(2),
        ] {
            let motif = motif.sequence(1, 2);
            let r = realize_line(&fusion_request(&motif, &chords, &ctx, &s));
            assert_eq!(r.repairs, 0, "the search had to repair: {:?}", r.notes);
            assert!(r.notes.iter().all(|n| n.function.is_some()));
            // Structural targets are stable over their onset harmony.
            for n in r.notes.iter().filter(|n| n.structural) {
                let f = n.function.unwrap();
                assert!(f.is_consonant(), "target {n:?} is not stable");
            }
            // No note sustains more than half a beat into a chord it does not belong to.
            for n in &r.notes {
                let end = n.start + n.dur as f64;
                if let Some(next) = chords
                    .iter()
                    .find(|c| c.start_beat > n.start + 1e-6 && c.start_beat < end - 0.5)
                {
                    assert!(next.chord.contains_pc(pitch_class(n.pitch)), "{n:?} smears");
                }
            }
        }
    }

    #[test]
    fn the_fusion_line_uses_connective_motion_and_stays_deterministic() {
        let (chords, ctx, s) = two_five_one();
        let mut connective = 0usize;
        for motif in [Motif::seed_a(), Motif::seed_b()] {
            let motif = motif.sequence(1, 2);
            let a = realize_line(&fusion_request(&motif, &chords, &ctx, &s));
            let b = realize_line(&fusion_request(&motif, &chords, &ctx, &s));
            assert_eq!(a, b);
            connective += a
                .notes
                .iter()
                .filter(|n| n.function.is_some_and(|f| !f.is_consonant()))
                .count();
        }
        assert!(
            connective > 0,
            "no passing/approach/neighbour motion at all"
        );
    }

    #[test]
    fn temporal_next_targets_change_an_ambiguous_structural_choice() {
        let scale = Scale::new(0, Mode::Ionian);
        let chords = vec![
            span(0.0, 2.0, Chord::new(0, Quality::Maj)),
            span(2.0, 2.0, Chord::new(5, Quality::Maj)),
        ];
        let mut contexts = super::super::context::analyze(&chords, &scale);
        let motif = Motif {
            id: 0,
            degrees: vec![1],
            rhythm: vec![0.5],
        };
        // A controlled field mutation proves this information reaches selection. Other
        // palette fields and the actual harmony are frozen, so this is a consumer test.
        contexts[0].palette.next_targets = vec![0];
        let mut request = fusion_request(&motif, &chords, &contexts, &scale);
        request.start_beat = 1.0;
        let legacy = realize_line(&request);
        let toward_c = realize_line_temporal(&request);
        contexts[0].palette.next_targets = vec![4];
        let mut request = fusion_request(&motif, &chords, &contexts, &scale);
        request.start_beat = 1.0;
        assert_eq!(
            realize_line(&request),
            legacy,
            "R11 does not consume future targets"
        );
        let toward_e = realize_line_temporal(&request);
        assert_eq!(pitch_class(toward_c.notes[0].pitch), 0);
        assert_eq!(pitch_class(toward_e.notes[0].pitch), 4);
    }

    #[test]
    fn temporal_search_preserves_gates_and_connective_motion() {
        let (chords, contexts, scale) = two_five_one();
        let mut chromatic = 0;
        for seed in [Motif::seed_a(), Motif::seed_b()] {
            for shift in 0..7 {
                let motif = seed.transpose(shift).sequence(1, 2);
                let request = fusion_request(&motif, &chords, &contexts, &scale);
                let old = realize_line(&request);
                let new = realize_line_temporal(&request);
                assert_eq!(new.repairs, 0, "{new:?}");
                assert_eq!(old.notes.len(), new.notes.len());
                for (a, b) in old.notes.iter().zip(&new.notes) {
                    assert_eq!((a.start, a.dur, a.accent), (b.start, b.dur, b.accent));
                    assert!(b.function.is_some(), "{b:?}");
                    chromatic += usize::from(matches!(
                        b.function,
                        Some(
                            PitchFunction::ChromaticApproach
                                | PitchFunction::ChromaticPassing
                                | PitchFunction::Enclosure
                        )
                    ));
                }
            }
        }
        assert!(
            chromatic > 0,
            "the temporal search must preserve chromaticism"
        );
        let chords = vec![span(0.0, 4.0, Chord::new(0, Quality::Maj))];
        let contexts = super::super::context::analyze(&chords, &scale);
        let motif = Motif {
            id: 0,
            degrees: vec![2, 3, 4],
            rhythm: vec![0.5, 0.5, 1.0],
        };
        let line = realize_line_temporal(&fusion_request(&motif, &chords, &contexts, &scale));
        assert_eq!(
            line.notes[1].function,
            Some(PitchFunction::DiatonicPassing),
            "{line:?}"
        );
    }

    #[test]
    fn temporal_extension_cost_distinguishes_owned_color_from_a_leap() {
        let c = Chord::new(0, Quality::Maj7);
        let f = Chord::new(5, Quality::Maj7);
        assert_eq!(extension_path_cost(c, 69, f, 69, 1.0), 0.0);
        assert_eq!(extension_path_cost(c, 69, c, 67, 1.0), 0.0);
        assert!(extension_path_cost(c, 69, f, 76, 1.0) > 0.0);
    }

    #[test]
    fn temporal_search_keeps_an_extension_that_becomes_a_common_core_tone() {
        let scale = Scale::new(0, Mode::Ionian);
        let chords = vec![
            span(0.0, 2.0, Chord::new(0, Quality::Maj7)),
            span(2.0, 2.0, Chord::new(5, Quality::Maj7)),
        ];
        let contexts = super::super::context::analyze(&chords, &scale);
        let motif = Motif {
            id: 0,
            degrees: vec![5, 5],
            rhythm: vec![1.0, 1.0],
        };
        let mut request = fusion_request(&motif, &chords, &contexts, &scale);
        request.start_beat = 1.0;
        let line = realize_line_temporal(&request);
        assert_eq!(
            line.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
            vec![69, 69]
        );
        assert_eq!(
            line.notes[0].function,
            Some(PitchFunction::LicensedExtension)
        );
        assert_eq!(line.notes[1].function, Some(PitchFunction::ChordTone));
    }
}
