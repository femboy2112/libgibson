//! Motif identities and their transformations. A motif is a scale-degree contour plus a
//! rhythm; the same motif heard at the climax must be recognizably the object seeded at the
//! start. Transformations preserve the identity (`id`) while developing the material, so
//! the melody engine can grow one idea across the whole piece instead of inventing a new
//! tune every four bars.

use super::harmony::ChordSpan;
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
    /// The signature seed: a rising-then-turning four-note call.
    pub fn seed_a() -> Motif {
        Motif {
            id: 0,
            degrees: vec![0, 2, 4, 3],
            rhythm: vec![1.0, 1.0, 1.0, 1.0],
        }
    }

    /// A contrasting, more active seed.
    pub fn seed_b() -> Motif {
        Motif {
            id: 1,
            degrees: vec![4, 3, 1, 0, 1],
            rhythm: vec![0.5, 0.5, 1.0, 0.5, 1.5],
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
    /// Generate the roster deterministically from `seed`, colored by `scale`.
    ///
    /// Determinism lives entirely in `seed` (a seeded [`Rng`]); `scale` only tilts which
    /// germ is chosen (bright modes get the open rising call, darker modes the restless one),
    /// so `(scale, seed)` fully determines the bank. Every melodic member is non-empty by
    /// construction.
    pub fn generate(scale: &Scale, seed: u64) -> MotifBank {
        let mut rng = Rng::new(seed);

        // Bright rooms get the open rising call; darker modes get the more restless germ.
        let bright = matches!(scale.mode, Mode::Ionian | Mode::Lydian | Mode::Mixolydian);
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

/// Realize a WHOLE motif statement JOINTLY against the harmony over its span.
///
/// This is the cure for Round I's per-note snapping. Instead of dragging each note to the
/// nearest chord tone in isolation — which shreds the contour — we run a bounded DP (a beam of
/// at most `max_candidates` pitch options per note) that scores the *whole* path at once:
/// chord-tone fit on strong beats, small voice-leading motion, and — decisively — agreement
/// with the original motif's contour direction between successive notes. The shape survives;
/// the harmony is satisfied; nothing is diced.
///
/// Output mirrors [`Motif::render`]'s timing and pitch and adds a per-note [`PitchFunction`]
/// classification (the jazz principle: chord tone, or the justification a non-chord tone carries,
/// or `None` for an unjustified note). Durations come from `motif.rhythm`, start times accumulate
/// from `start_beat`. Deterministic — no RNG, and ties resolve to the candidate nearest the
/// intended pitch (candidate lists are closest-first).
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
) -> Vec<(f64, f32, Midi, Option<PitchFunction>)> {
    let n = motif.len();
    if n == 0 {
        return Vec::new();
    }
    let cap = max_candidates.max(1);

    // Timings — identical to Motif::render (durations from rhythm, cumulative from start).
    let mut starts = Vec::with_capacity(n);
    let mut t = start_beat;
    for i in 0..n {
        starts.push(t);
        t += motif.rhythm[i] as f64;
    }

    // Per-note anchor (the intended contour pitch), strong-beat flag, and chord in force.
    let mut anchors = Vec::with_capacity(n);
    let mut strong = Vec::with_capacity(n);
    let mut chord_here = Vec::with_capacity(n);
    let mut cands: Vec<Vec<Midi>> = Vec::with_capacity(n);
    for (&start, &deg) in starts.iter().zip(motif.degrees.iter()) {
        let anchor = scale.degree_pitch(root_degree + deg, octave);
        let is_strong = is_strong_beat(start);
        let chord = chord_at(chords, start);
        cands.push(candidate_pitches(anchor, is_strong, chord, scale, cap));
        anchors.push(anchor);
        strong.push(is_strong);
        chord_here.push(chord);
    }

    // Forward DP: cost[i][c] = least cost to reach candidate c of note i.
    let inf = f32::INFINITY;
    let mut cost: Vec<Vec<f32>> = Vec::with_capacity(n);
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(n);
    for i in 0..n {
        let m = cands[i].len();
        let mut ci = vec![inf; m];
        let mut bi = vec![0usize; m];
        for (c, &p) in cands[i].iter().enumerate() {
            let node = node_cost(p, anchors[i], strong[i], chord_here[i]);
            if i == 0 {
                // Continuity: seed the first note toward the previous statement's exit pitch, so
                // consecutive phrases connect in register instead of teleporting between them.
                ci[c] = node + prev_pitch.map_or(0.0, |pp| VL_W * (p - pp).abs() as f32);
            } else {
                let dir_orig = (motif.degrees[i] - motif.degrees[i - 1]).signum();
                let mut best = inf;
                let mut bidx = 0;
                for (pi, &pp) in cands[i - 1].iter().enumerate() {
                    let tot = cost[i - 1][pi] + transition_cost(pp, p, dir_orig);
                    if tot < best {
                        best = tot;
                        bidx = pi;
                    }
                }
                ci[c] = best + node;
                bi[c] = bidx;
            }
        }
        cost.push(ci);
        back.push(bi);
    }

    // Backtrack the minimal path; strict `<` keeps the first (closest) candidate on ties.
    let mut idx = 0;
    let mut best = inf;
    for (c, &v) in cost[n - 1].iter().enumerate() {
        if v < best {
            best = v;
            idx = c;
        }
    }
    let mut chosen = vec![0usize; n];
    chosen[n - 1] = idx;
    for i in (1..n).rev() {
        chosen[i - 1] = back[i][chosen[i]];
    }

    let mut pitches: Vec<Midi> = (0..n).map(|i| cands[i][chosen[i]]).collect();

    // Classify note `i` against its actual harmonic context and its realized neighbours in time.
    let classify_at = |pitches: &[Midi], i: usize| {
        let prev = if i > 0 { Some(pitches[i - 1]) } else { None };
        let next = if i + 1 < n {
            Some(pitches[i + 1])
        } else {
            None
        };
        let prev_chord = if i > 0 { chord_here[i - 1] } else { None };
        let next_chord = if i + 1 < n { chord_here[i + 1] } else { None };
        super::pitch::classify(
            pitches[i],
            prev,
            next,
            prev_chord,
            chord_here[i],
            next_chord,
            scale,
            strong[i],
        )
    };

    // Justify-or-snap repair (the jazz principle's negative side): a note that classifies to `None`
    // is an unjustified "wrong note" — not a chord tone, and no stepwise path or held/borrowed tone
    // explains it. Replace ONLY those with the nearest tone of the chord sounding under them; a note
    // that already carries a reason (approach / passing / neighbour / suspension / anticipation /
    // appoggiatura) is never touched — we remove unexplained tension, never tension itself. Iterated
    // to a fixed point, since repairing one note can change a neighbour's classification.
    for _ in 0..4 {
        let mut changed = false;
        for i in 0..n {
            if classify_at(&pitches, i).is_none() {
                let fixed = nearest_chord_tone(pitches[i], chord_here[i], scale);
                if fixed != pitches[i] {
                    pitches[i] = fixed;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Emit with each note's final PitchFunction for the Score IR.
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push((
            starts[i],
            motif.rhythm[i],
            pitches[i],
            classify_at(&pitches, i),
        ));
    }
    out
}

/// True when `beat` sits on an integer-beat onset (a strong beat).
fn is_strong_beat(beat: f64) -> bool {
    (beat - beat.round()).abs() < 1e-6
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

/// Up to `cap` pitch candidates near `anchor`, closest-first: chord tones on strong beats,
/// scale tones on weak ones (chord tones fall out as a subset). Never empty.
fn candidate_pitches(
    anchor: Midi,
    strong: bool,
    chord: Option<Chord>,
    scale: &Scale,
    cap: usize,
) -> Vec<Midi> {
    let mut v: Vec<Midi> = Vec::new();
    for d in 0..=12 {
        for &m in &[anchor - d, anchor + d] {
            let pc = pitch_class(m);
            let ok = if strong {
                match chord {
                    Some(ch) => ch.contains_pc(pc),
                    None => scale.contains_pc(pc),
                }
            } else {
                scale.contains_pc(pc)
            };
            if ok && !v.contains(&m) {
                v.push(m);
                if v.len() >= cap {
                    break;
                }
            }
        }
        if v.len() >= cap {
            break;
        }
    }
    if v.is_empty() {
        v.push(scale.nearest_scale_pitch(anchor));
    }
    v
}

/// Weight on a note's distance from its intended (anchor) pitch.
const ANCHOR_W: f32 = 0.5;
/// Weight on voice-leading motion between successive realized notes.
const VL_W: f32 = 0.1;
/// Penalty for contradicting the original motif's contour direction — dominant on purpose.
const CONTOUR_PEN: f32 = 10.0;
/// Penalty for a weak-beat note that isn't a chord tone (passing/neighbor tolerance).
const WEAK_NONCHORD: f32 = 0.3;

/// Standalone cost of placing pitch `p` at a note (register fit + weak-beat non-chord tax).
fn node_cost(p: Midi, anchor: Midi, strong: bool, chord: Option<Chord>) -> f32 {
    let mut c = ANCHOR_W * (p - anchor).abs() as f32;
    if !strong {
        let is_chord_tone = chord.is_some_and(|ch| ch.contains_pc(pitch_class(p)));
        if !is_chord_tone {
            c += WEAK_NONCHORD;
        }
    }
    c
}

/// Cost of moving from `pp` to `p` given the motif's intended direction `dir_orig`.
fn transition_cost(pp: Midi, p: Midi, dir_orig: i32) -> f32 {
    let mut c = VL_W * (p - pp).abs() as f32;
    if (p - pp).signum() != dir_orig {
        c += CONTOUR_PEN;
    }
    c
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
        assert_eq!(t.degrees, vec![2, 4, 6, 5]);
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
        assert_eq!(notes.len(), 4);
        assert_eq!(notes[0].0, 8.0); // first at start
        assert_eq!(notes[0].2, 60); // degree 0 -> C4
        assert_eq!(notes[1].0, 9.0); // after 1 beat
        assert_eq!(notes[1].2, 64); // degree 2 -> E4
        assert_eq!(notes[2].2, 67); // degree 4 -> G4 (0-based scale degree 4 = 5th tone)
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
        let s = Scale::new(0, Mode::Ionian);
        let a = MotifBank::generate(&s, 7);
        let b = MotifBank::generate(&s, 7);
        assert_eq!(a, b); // deterministic in (scale, seed)
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

        assert_eq!(out.len(), 4);
        // Timings mirror Motif::render exactly.
        for (i, (start, dur, _, _)) in out.iter().enumerate() {
            assert_eq!(*dur, m.rhythm[i]);
            assert_eq!(*start, i as f64); // 0, 1, 2, 3
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
}
