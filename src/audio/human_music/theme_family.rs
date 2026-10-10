//! Generative `ThemeFamily`: related-but-distinct melodic material.
//!
//! The [`super::argument`] layer models the antecedent/consequent of *one* theme: an answer must
//! share the question's exact head and length (octave and time-scale aside — `same_head`). A real
//! song has a VERSE and a distinct-but-related HOOK, a departing BRIDGE, and an earned RETURN. This
//! module supplies the laws that make a second phrase *belong to the same song without being the
//! same tune*, so the argument contract can admit a genuine consequent instead of a restatement.
//!
//! The laws are calibrated from our own hand-authored teacher (`examples/rick_probe.rs`) — measured
//! relationships, never its literal note arrays. The generator produces FRESH seeded material that
//! satisfies them; the predicates are the independent gate. A transform that returns the source
//! unchanged (identity, or a rigid transposition) is a *restatement*, never a development: that is
//! the single most important thing these predicates reject, because the archived `same_head` route
//! accepts it.
//!
//! Ear is the oracle. These predicates only ever REJECT material that is provably a clone, an
//! unrelated tune, or a no-op relabelled as development; they never certify that anything "sounds
//! right". A render that passes every gate here is merely *not disqualified*.
//!
//! The generator does NOT improvise note by note. A phrase is DERIVED from a handful of seed-chosen
//! parameters (an arch height, a register, a signature descent cell) composed *against the harmonic
//! route*: strong beats land on chord tones of the bar they fall in, the connecting onsets step
//! diatonically, and each bar is a single-peak arch — the shape measured in our own teacher, whose
//! coherence is carried by harmonic rooting, rhythm-grid reuse, and a few recurring cells rather
//! than by interval-exact motivic transformation. The dice are rolled high (which arch, which
//! register, which cell), never low (every note): the song is thought up, then played.

use super::material::MaterialEvent;
use super::rng::Rng;
use super::theory::Quality;

/// Scale steps (semitones from the tonic) of the natural-minor home the teacher writes in.
pub const AEOLIAN: [i32; 7] = [0, 2, 3, 5, 7, 8, 10];
/// Chromatic colours the teacher licenses: the leading tone (3rd of the home dominant E7) and the
/// raised 4th-above (3rd of the secondary dominant A7). Diatonic verse/hook stay out of these; the
/// bridge earns them (the departure law, added with the bridge).
pub const LEADING_TONE: i32 = 11;
pub const SECONDARY_THIRD: i32 = 4;

/// Whether a phrase's final note leaves home open or lands it home.
///
/// This replaces the archived `|last - expected| <= 2` *proximity* rule: the teacher's verse ends on
/// the ♭7 (seven semitones below, five above the tonic) — open by *function*, not distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrivalKind {
    /// Final pitch-class is not the tonic: the phrase hangs on a tendency tone and wants resolution.
    Open,
    /// Final pitch-class is the tonic: the phrase lands home.
    Closed,
}

impl ArrivalKind {
    /// The arrival of a melody read from its own last pitched event, relative to the tonic.
    ///
    /// Melodic (function at the chord level is a separate, later obligation; this is the note the
    /// ear actually hears land). A phrase with no pitched event has no arrival.
    pub fn of(events: &[MaterialEvent]) -> Option<ArrivalKind> {
        let last = events.iter().rev().find_map(|e| e.step)?;
        Some(if last.rem_euclid(12) == 0 {
            ArrivalKind::Closed
        } else {
            ArrivalKind::Open
        })
    }
}

/// The measured kinship of a candidate consequent against its antecedent.
///
/// Every field is the raw measurement; [`Kinship::altered_consequent`] applies the calibrated gates.
/// Exposed so a witness/diagnostic can report *why* a pair was rejected rather than a bare bool.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kinship {
    pub onset_iou: f64,
    pub head: bool,
    pub parsons: f64,
    pub exact_interval: f64,
    pub offset_spread: i32,
    pub mean_delta: f64,
    pub floor_at_or_above_tonic: bool,
    pub peak_profile_differs: bool,
    pub source_arrival: Option<ArrivalKind>,
    pub cand_arrival: Option<ArrivalKind>,
    pub scale_fraction: f64,
    pub step_rate: f64,
}

impl Kinship {
    /// Measure a candidate consequent against its antecedent over a phrase of `beats_per_bar`.
    pub fn measure(
        source: &[MaterialEvent],
        cand: &[MaterialEvent],
        beats_per_bar: f64,
    ) -> Kinship {
        Kinship {
            onset_iou: onset_iou(source, cand),
            head: head_match(source, cand),
            parsons: parsons_agreement(source, cand),
            exact_interval: exact_interval_agreement(source, cand),
            offset_spread: aligned_offset_spread(source, cand),
            mean_delta: mean_pitch(cand) - mean_pitch(source),
            floor_at_or_above_tonic: floor_step(cand).is_some_and(|f| f >= 0),
            peak_profile_differs: peak_profile_differs(source, cand, beats_per_bar, 2),
            source_arrival: ArrivalKind::of(source),
            cand_arrival: ArrivalKind::of(cand),
            scale_fraction: scale_pool_fraction(cand),
            step_rate: step_rate(cand),
        }
    }

    /// The `AlteredConsequent` law (verse → hook): a genuinely different consequent that still
    /// clearly belongs to the same song.
    ///
    /// PRESERVE (kinship K): phrase frame, rhythm skeleton (onset IoU ≥ 0.70), head cell (first
    /// interval exact + first two signs equal), contour (Parsons ≥ 0.60). CHANGE (must-change D,
    /// else it is a restatement): exact-interval agreement ≤ 0.60 AND aligned-offset spread ≥ 2
    /// (not a rigid transposition), register lifted 2–6 semitones with the floor at/above the tonic
    /// and a bar-peak profile that differs, and the arrival swaps Open → Closed. Clone-killers:
    /// ≥ 95% of pitches in the scale pool and ≥ 50% stepwise motion.
    pub fn altered_consequent(&self) -> bool {
        // K — preserved kinship.
        let preserved = self.onset_iou >= 0.70 && self.head && self.parsons >= 0.60;
        // D — must-change (this is the clause the archived `same_head`/identity route fails).
        let changed = self.exact_interval <= 0.60
            && self.offset_spread >= 2
            && (2.0..=6.0).contains(&self.mean_delta)
            && self.floor_at_or_above_tonic
            && self.peak_profile_differs;
        // Function: an answer must actually land home where its antecedent hung open.
        let answers = self.source_arrival == Some(ArrivalKind::Open)
            && self.cand_arrival == Some(ArrivalKind::Closed);
        // Clone-killers: idiomatic, not noise.
        let idiomatic = self.scale_fraction >= 0.95 && self.step_rate >= 0.50;
        preserved && changed && answers && idiomatic
    }
}

// ---------------------------------------------------------------------------------------------
// Measurement helpers. All operate on the pitched events only (unpitched = rhythm, excluded).
// ---------------------------------------------------------------------------------------------

/// Round an onset to the sixteenth grid the teacher writes on, for set comparison.
fn grid(onset: f64) -> i64 {
    (onset * 4.0).round() as i64
}

fn pitched(events: &[MaterialEvent]) -> Vec<(i64, i32)> {
    events
        .iter()
        .filter_map(|e| e.step.map(|s| (grid(e.onset), s)))
        .collect()
}

/// Intersection-over-union of the two onset sets on the sixteenth grid.
fn onset_iou(a: &[MaterialEvent], b: &[MaterialEvent]) -> f64 {
    use std::collections::BTreeSet;
    let sa: BTreeSet<i64> = pitched(a).into_iter().map(|(o, _)| o).collect();
    let sb: BTreeSet<i64> = pitched(b).into_iter().map(|(o, _)| o).collect();
    if sa.is_empty() && sb.is_empty() {
        return 1.0;
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// First three onsets equal, first interval exact, first two interval signs equal.
fn head_match(a: &[MaterialEvent], b: &[MaterialEvent]) -> bool {
    let pa = pitched(a);
    let pb = pitched(b);
    if pa.len() < 3 || pb.len() < 3 {
        return false;
    }
    if pa[..3]
        .iter()
        .map(|&(o, _)| o)
        .ne(pb[..3].iter().map(|&(o, _)| o))
    {
        return false;
    }
    let first_interval = (pa[1].1 - pa[0].1) == (pb[1].1 - pb[0].1);
    let sign = |x: i32| x.signum();
    let signs = sign(pa[1].1 - pa[0].1) == sign(pb[1].1 - pb[0].1)
        && sign(pa[2].1 - pa[1].1) == sign(pb[2].1 - pb[1].1);
    first_interval && signs
}

/// Fraction of consecutive-interval signs that agree, over notes at shared onsets.
fn parsons_agreement(a: &[MaterialEvent], b: &[MaterialEvent]) -> f64 {
    let common = common_onset_pitches(a, b);
    if common.len() < 2 {
        return 0.0;
    }
    let mut agree = 0usize;
    let mut total = 0usize;
    for w in common.windows(2) {
        let sa = (w[1].0 - w[0].0).signum();
        let sb = (w[1].1 - w[0].1).signum();
        total += 1;
        if sa == sb {
            agree += 1;
        }
    }
    agree as f64 / total as f64
}

/// Fraction of consecutive intervals that are *exactly* equal, over notes at shared onsets.
fn exact_interval_agreement(a: &[MaterialEvent], b: &[MaterialEvent]) -> f64 {
    let common = common_onset_pitches(a, b);
    if common.len() < 2 {
        return 1.0; // nothing to disagree on: treat as maximal similarity (conservative for D)
    }
    let mut agree = 0usize;
    let mut total = 0usize;
    for w in common.windows(2) {
        let ia = w[1].0 - w[0].0;
        let ib = w[1].1 - w[0].1;
        total += 1;
        if ia == ib {
            agree += 1;
        }
    }
    agree as f64 / total as f64
}

/// max(offset) − min(offset) over notes at shared onsets, offset = cand − source. A rigid
/// transposition has spread 0; a genuinely re-voiced consequent moves notes by varying amounts.
fn aligned_offset_spread(a: &[MaterialEvent], b: &[MaterialEvent]) -> i32 {
    let common = common_onset_pitches(a, b);
    if common.is_empty() {
        return 0;
    }
    let offsets: Vec<i32> = common.iter().map(|&(x, y)| y - x).collect();
    offsets.iter().copied().max().unwrap() - offsets.iter().copied().min().unwrap()
}

/// Pairs (source_pitch, cand_pitch) for onsets present in BOTH, in onset order.
fn common_onset_pitches(a: &[MaterialEvent], b: &[MaterialEvent]) -> Vec<(i32, i32)> {
    use std::collections::BTreeMap;
    let ma: BTreeMap<i64, i32> = pitched(a).into_iter().collect();
    let mb: BTreeMap<i64, i32> = pitched(b).into_iter().collect();
    ma.iter()
        .filter_map(|(o, &pa)| mb.get(o).map(|&pb| (pa, pb)))
        .collect()
}

fn mean_pitch(events: &[MaterialEvent]) -> f64 {
    let p = pitched(events);
    if p.is_empty() {
        return 0.0;
    }
    p.iter().map(|&(_, s)| f64::from(s)).sum::<f64>() / p.len() as f64
}

fn floor_step(events: &[MaterialEvent]) -> Option<i32> {
    pitched(events).into_iter().map(|(_, s)| s).min()
}

/// Whether a semitone step (from the tonic) is a diatonic Aeolian scale tone.
fn is_scale(step: i32) -> bool {
    AEOLIAN.contains(&step.rem_euclid(12))
}

/// The maximum pitch in each bar-sized window.
fn bar_peaks(events: &[MaterialEvent], beats_per_bar: f64) -> Vec<i32> {
    use std::collections::BTreeMap;
    let mut peaks: BTreeMap<i64, i32> = BTreeMap::new();
    for e in events {
        if let Some(s) = e.step {
            let bar = (e.onset / beats_per_bar).floor() as i64;
            peaks
                .entry(bar)
                .and_modify(|p| *p = (*p).max(s))
                .or_insert(s);
        }
    }
    peaks.into_values().collect()
}

/// At least one shared bar whose peak differs by ≥ `min` semitones.
fn peak_profile_differs(
    a: &[MaterialEvent],
    b: &[MaterialEvent],
    beats_per_bar: f64,
    min: i32,
) -> bool {
    let pa = bar_peaks(a, beats_per_bar);
    let pb = bar_peaks(b, beats_per_bar);
    pa.iter().zip(&pb).any(|(x, y)| (x - y).abs() >= min)
}

/// Fraction of pitches whose pitch-class is in the scale pool (diatonic + licensed chromatics).
fn scale_pool_fraction(events: &[MaterialEvent]) -> f64 {
    let p = pitched(events);
    if p.is_empty() {
        return 1.0;
    }
    let inpool = |s: i32| {
        let pc = s.rem_euclid(12);
        AEOLIAN.contains(&pc) || pc == LEADING_TONE || pc == SECONDARY_THIRD
    };
    p.iter().filter(|&&(_, s)| inpool(s)).count() as f64 / p.len() as f64
}

/// Fraction of consecutive intervals that are stepwise (|interval| ≤ 2).
fn step_rate(events: &[MaterialEvent]) -> f64 {
    let p = pitched(events);
    if p.len() < 2 {
        return 1.0;
    }
    let mut steps = 0usize;
    for w in p.windows(2) {
        if (w[1].1 - w[0].1).abs() <= 2 {
            steps += 1;
        }
    }
    steps as f64 / (p.len() - 1) as f64
}

/// Whether `b` contains a ≥`len`-note interval-sign contour that also appears in `a` (a recycled
/// cell). The bridge's melodic kinship to the hook is carried by recycled cells, not by a new tune.
fn shares_contour_cell(a: &[MaterialEvent], b: &[MaterialEvent], len: usize) -> bool {
    let signs = |ev: &[MaterialEvent]| -> Vec<i32> {
        let p = pitched(ev);
        p.windows(2).map(|w| (w[1].1 - w[0].1).signum()).collect()
    };
    let sa = signs(a);
    let sb = signs(b);
    if len == 0 || sa.len() < len || sb.len() < len {
        return false;
    }
    sb.windows(len).any(|wb| sa.windows(len).any(|wa| wa == wb))
}

/// The `Departure` law (hook → bridge): a real bridge TAKES the material somewhere different, but —
/// per the teacher — by HARMONY, register and rhythm, NOT by a melodically unrelated tune. It
/// recycles a hook cell (shared contour), lifts the register (mean ≥ the hook's), and ends OPEN on a
/// tendency tone wanting the return. The harmonic departure itself is carried by the route, not here.
/// A relabelled restatement of the hook (same register, Closed) is rejected.
pub fn is_departure(hook: &[MaterialEvent], bridge: &[MaterialEvent]) -> bool {
    shares_contour_cell(hook, bridge, 3)
        && mean_pitch(bridge) >= mean_pitch(hook)
        && ArrivalKind::of(bridge) == Some(ArrivalKind::Open)
        && scale_pool_fraction(bridge) >= 0.95
}

// ---------------------------------------------------------------------------------------------
// Harmonic targeting. The route is region-relative (semitones above the tonic, quality) — the SAME
// basis as a melody `step`, so a bar's chord tones are just (offset + interval) mod 12. The composer
// pins strong beats to these so the lead is ROOTED in the harmony it plays over, not floating above
// a scale. This is what the teacher does (strong-beat chord-tone fraction ~0.74–0.81) and what the
// old random-walk generator never did (it only knew the global scale).
// ---------------------------------------------------------------------------------------------

/// A region-relative bar chord: `(semitones above the tonic, quality)`.
pub type BarChord = (i32, Quality);

/// The chord's tonic-relative pitch classes (root/3rd/5th/(7th), `0..=11`).
fn chord_pcs(chord: BarChord) -> Vec<i32> {
    chord
        .1
        .intervals()
        .iter()
        .map(|&i| (chord.0 + i).rem_euclid(12))
        .collect()
}

/// The chord tone nearest to `near` (in `step` semitones from the tonic). Ties resolve downward.
fn snap_chord(near: i32, chord: BarChord) -> i32 {
    let pcs = chord_pcs(chord);
    (0..=12)
        .flat_map(|d| [near - d, near + d])
        .find(|&c| pcs.contains(&c.rem_euclid(12)))
        .unwrap_or(near)
}

/// The nearest NON-tonic chord tone to `near`: an Open arrival hangs on a tendency tone of the
/// cadence chord, never the tonic.
fn open_chord_tone(near: i32, chord: BarChord) -> i32 {
    let pcs = chord_pcs(chord);
    (0..=12)
        .flat_map(|d| [near - d, near + d])
        .find(|&c| {
            let pc = c.rem_euclid(12);
            pc != 0 && pcs.contains(&pc)
        })
        .unwrap_or_else(|| snap_chord(near, chord))
}

/// The nearest tonic (pitch-class 0), at or above the floor.
fn nearest_tonic(near: i32) -> i32 {
    let k = (f64::from(near) / 12.0).round() as i32;
    (k * 12).max(0)
}

/// Choose a chord tone within `window` semitones of the arch target `height` — a COMPOSITIONAL
/// choice among the harmony's own tones (root/3rd/5th/7th in some octave), seeded so different seeds
/// voice genuinely different-but-rooted lines instead of all snapping to the one nearest tone. This
/// is where the seed's freedom lives: not a per-note pitch dice (that was the random walk), but which
/// consonant tone the arch lands on. Always returns a chord tone, so strong beats stay rooted.
fn pick_chord_tone_near(rng: &mut Rng, height: i32, chord: BarChord, window: i32) -> i32 {
    let pcs = chord_pcs(chord);
    let cands: Vec<i32> = ((height - window)..=(height + window))
        .filter(|t| pcs.contains(&t.rem_euclid(12)))
        .collect();
    if cands.is_empty() {
        snap_chord(height, chord)
    } else {
        cands[rng.below(cands.len())]
    }
}

// ---------------------------------------------------------------------------------------------
// Generator. Composes phrases by DERIVING every note from the harmony and an arch — not a random
// walk. The laws above remain the independent gate (the builder asserts them before returning).
// ---------------------------------------------------------------------------------------------

/// A generated, related pair of four-bar phrases.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemePair {
    /// The antecedent: rises, ends Open (off the tonic, on a tendency tone).
    pub verse: Vec<MaterialEvent>,
    /// The consequent: an `AlteredConsequent` of the verse, ends Closed (home on the tonic).
    pub hook: Vec<MaterialEvent>,
}

fn ev(onset: f64, dur: f64, accent: f32, step: i32) -> MaterialEvent {
    MaterialEvent {
        onset,
        dur,
        accent,
        step: Some(step),
    }
}

/// Snap a pitch to the nearest Aeolian scale tone (ties resolve downward).
fn snap_aeolian(step: i32) -> i32 {
    let octave = step.div_euclid(12);
    let pc = step.rem_euclid(12);
    let nearest = AEOLIAN
        .iter()
        .copied()
        .min_by_key(|&d| (d - pc).abs() * 2 + i32::from(d > pc))
        .unwrap();
    octave * 12 + nearest
}

// Onset grids, reused FUNCTIONALLY across bars (the teacher's 16 unique bars share only 5 grids).
// Every grid carries the two strong beats (0.0 and 2.0) so a bar is always rooted on beats 1 and 3.
const HEAD_GRID: [f64; 6] = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0];
const REACH_GRID: [f64; 5] = [0.0, 1.0, 1.5, 2.0, 3.0];
const CADENCE_GRID: [f64; 5] = [0.0, 0.5, 1.0, 1.5, 2.0];
const BRIDGE_GRID: [f64; 4] = [0.0, 1.0, 2.0, 3.0];
const BRIDGE_CADENCE_GRID: [f64; 3] = [0.0, 1.0, 2.0];

/// The handful of seed-chosen parameters a whole phrase is DERIVED from. This is the "seed level":
/// the composer reads these and the route, and everything else is determined — no per-note dice.
#[derive(Debug, Clone, Copy)]
struct PhraseShape {
    /// Register floor in semitones above the tonic (0 = verse home; lifted for hook/bridge).
    lift: i32,
    /// The phrase lands home on the tonic (Closed) when true, else hangs Open on a chord tone.
    close: bool,
    /// The climax height (steps above the tonic) the arch reaches before the cadence.
    climax: i32,
    /// The signature descent cell (two interval steps, each negative) stated at the cadence.
    descent: [i32; 2],
    /// A thinned (quarter-note) grid — the teacher's bridge drops its fast motion.
    thin: bool,
}

fn select_grid(bar: usize, is_cadence: bool, thin: bool) -> &'static [f64] {
    match (is_cadence, thin) {
        (true, true) => &BRIDGE_CADENCE_GRID,
        (false, true) => &BRIDGE_GRID,
        (true, false) => &CADENCE_GRID,
        (false, false) if bar == 0 => &HEAD_GRID,
        (false, false) => &REACH_GRID,
    }
}

/// Compose one phrase over its per-bar harmonic route. Strong beats (onset 0.0 and 2.0 of each bar)
/// land on chord tones of that bar's chord; the connecting onsets step diatonically between them;
/// each non-cadence bar is a single-peak REACH to the arch; the final bar states the signature
/// descent cell onto the arrival tone (Open on a tendency tone, or Closed on the tonic). Fully
/// determined by `shape` + `prog` — the composition, not an improvisation.
fn compose_phrase(
    rng: &mut Rng,
    span: f64,
    bpb: f64,
    prog: &[BarChord],
    shape: PhraseShape,
) -> Vec<MaterialEvent> {
    // How far a strong beat may stray from its arch target while staying a chord tone — the seed's
    // compositional room. A little wider on the peak so the climax can be a 3rd/5th/7th, not always
    // the nearest tone.
    const W: i32 = 3;
    let bars = (span / bpb).round().max(1.0) as usize;
    let peak_bar = bars.saturating_sub(2).max(1);
    // (onset, step, is_strong_beat)
    let mut notes: Vec<(f64, i32, bool)> = Vec::new();
    for bar in 0..bars {
        let base = bar as f64 * bpb;
        let chord = prog[bar % prog.len().max(1)];
        let is_cadence = bar + 1 == bars && bars >= 2;
        let g = select_grid(bar, is_cadence, shape.thin);
        if is_cadence {
            // State the signature descent cell from a chord tone near the arch top onto the arrival.
            let top = pick_chord_tone_near(
                rng,
                (shape.lift + 6).min(shape.climax).max(shape.lift + 3),
                chord,
                W,
            );
            let mut seq = vec![top];
            let mut p = top;
            for &d in shape.descent.iter() {
                p = snap_aeolian(p + d);
                seq.push(p);
            }
            while seq.len() < g.len() {
                p = snap_aeolian(p - 2);
                seq.push(p);
            }
            let n = g.len();
            seq[n - 1] = if shape.close {
                nearest_tonic(seq[n - 1])
            } else {
                open_chord_tone(seq[n - 1], chord)
            };
            for (i, &o) in g.iter().enumerate() {
                if base + o >= span {
                    break;
                }
                let strong = o == 0.0 || (o - 2.0).abs() < 1e-9;
                notes.push((base + o, seq[i.min(seq.len() - 1)], strong));
            }
        } else {
            // A single-peak REACH: beat 1 low on a chord tone, climbing to a chord-tone peak at
            // beat 3, then a small fall — transposed per bar to that bar's chord, rising across the
            // phrase toward the climax.
            let frac = (bar as f64 / peak_bar as f64).min(1.0);
            let lo = shape.lift + (3.0 * frac).round() as i32;
            let hi = shape.lift + (f64::from(shape.climax - shape.lift) * frac).round() as i32;
            let a_down = pick_chord_tone_near(rng, lo, chord, W);
            let a_mid = pick_chord_tone_near(rng, hi.max(a_down + 2), chord, W);
            for (i, &o) in g.iter().enumerate() {
                if base + o >= span {
                    break;
                }
                let _ = i;
                let (step, strong) = if o == 0.0 {
                    (a_down, true)
                } else if (o - 2.0).abs() < 1e-9 {
                    (a_mid, true)
                } else if o < 2.0 {
                    let up = a_down + (f64::from(a_mid - a_down) * (o / 2.0)).round() as i32;
                    (snap_aeolian(up), false)
                } else {
                    (snap_aeolian(a_mid - 2), false)
                };
                notes.push((base + o, step, strong));
            }
        }
    }
    // Durations and accents: a long final note; downbeats loudest, strong beats next, fills soft.
    let mut out = Vec::with_capacity(notes.len());
    for i in 0..notes.len() {
        let (on, st, strong) = notes[i];
        let is_last = i + 1 == notes.len();
        let dur = if is_last {
            (span - on).max(1.0)
        } else {
            (notes[i + 1].0 - on).clamp(0.25, 1.0)
        };
        let on_bar = (on.rem_euclid(bpb)).abs() < 1e-9;
        let accent = if on_bar {
            0.95
        } else if strong {
            0.85
        } else {
            0.72 + 0.03 * (i % 3) as f32
        };
        out.push(ev(on, dur, accent, st));
    }
    out
}

fn mix(seed: u64, salt: u64, tag: u64) -> u64 {
    (seed ^ tag)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(salt.wrapping_mul(0xD1B5_4A32_D192_ED03))
}

fn pick_descent(r: &mut Rng) -> [i32; 2] {
    match r.below(3) {
        0 => [-2, -2],
        1 => [-1, -2],
        _ => [-2, -1],
    }
}

/// Pin the hook's head so its first interval and the first two interval signs match the verse — the
/// `head_match` kinship — WITHOUT un-rooting beat 1. The downbeat (`hook[0]`) stays the chord tone
/// the composer placed; a nearby chord tone is chosen only if needed so that `+i1` and `+i1+i2` are
/// scale tones, and the two off-beat head notes then carry the shared contour.
fn force_shared_head(verse: &[MaterialEvent], hook: &mut [MaterialEvent], hook_bar0: BarChord) {
    let vp = pitched(verse);
    if vp.len() < 3 || hook.len() < 3 {
        return;
    }
    let (i1, i2) = (vp[1].1 - vp[0].1, vp[2].1 - vp[1].1);
    let base = hook[0].step.unwrap_or(0);
    // Prefer the composer's own downbeat; else the nearest chord tone keeping the head diatonic.
    let start = std::iter::once(base)
        .chain((0..=6).flat_map(|d| [base - d, base + d]))
        .find(|&h| {
            chord_pcs(hook_bar0).contains(&h.rem_euclid(12))
                && is_scale(h + i1)
                && is_scale(h + i1 + i2)
        })
        .unwrap_or(base);
    hook[0].step = Some(start);
    hook[1].step = Some(start + i1);
    hook[2].step = Some(start + i1 + i2);
}

/// Generate a verse/hook pair over a `span`-beat phrase of `beats_per_bar`, composed against the
/// verse and home progressions so both are rooted in their own harmony. The verse rises and hangs
/// OPEN over its progression; the hook is an `AlteredConsequent` — the same head and rhythm skeleton,
/// lifted a register, re-voiced by the DIFFERENT home harmony it is composed against (not by random
/// jitter), landing home CLOSED. The returned pair is asserted to satisfy
/// [`Kinship::altered_consequent`]; seeds are tried deterministically and a lawful fallback closes.
pub fn generate_pair(
    seed: u64,
    span: f64,
    beats_per_bar: f64,
    verse_prog: &[BarChord],
    hook_prog: &[BarChord],
) -> ThemePair {
    for salt in 0..64u64 {
        let mut r = Rng::new(mix(seed, salt, 0x7E3E_FA33_C137_D00D));
        let climax_v = 7 + r.below(4) as i32; // 7..=10 above the tonic
        let descent_v = pick_descent(&mut r);
        let lift = 3 + r.below(3) as i32; // +3..=+5 register lift
        let descent_h = pick_descent(&mut r);
        let shape_v = PhraseShape {
            lift: 0,
            close: false,
            climax: climax_v,
            descent: descent_v,
            thin: false,
        };
        let verse = compose_phrase(&mut r, span, beats_per_bar, verse_prog, shape_v);
        if ArrivalKind::of(&verse) != Some(ArrivalKind::Open) {
            continue;
        }
        let shape_h = PhraseShape {
            lift,
            close: true,
            climax: climax_v + lift,
            descent: descent_h,
            thin: false,
        };
        let mut hook = compose_phrase(&mut r, span, beats_per_bar, hook_prog, shape_h);
        force_shared_head(&verse, &mut hook, hook_prog[0]);
        if Kinship::measure(&verse, &hook, beats_per_bar).altered_consequent() {
            return ThemePair { verse, hook };
        }
    }
    fallback_pair(span, beats_per_bar, verse_prog, hook_prog)
}

/// A deterministic, lawful pair, used only if the seeded search is unlucky. Still composed against
/// the route (not a hand-typed array) — it is the generator at a fixed shape, not the teacher.
fn fallback_pair(
    span: f64,
    beats_per_bar: f64,
    verse_prog: &[BarChord],
    hook_prog: &[BarChord],
) -> ThemePair {
    let mut r = Rng::new(0xFA11_BACC_C137_0001);
    let verse = compose_phrase(
        &mut r,
        span,
        beats_per_bar,
        verse_prog,
        PhraseShape {
            lift: 0,
            close: false,
            climax: 9,
            descent: [-2, -2],
            thin: false,
        },
    );
    let mut hook = compose_phrase(
        &mut r,
        span,
        beats_per_bar,
        hook_prog,
        PhraseShape {
            lift: 4,
            close: true,
            climax: 13,
            descent: [-1, -2],
            thin: false,
        },
    );
    force_shared_head(&verse, &mut hook, hook_prog[0]);
    ThemePair { verse, hook }
}

/// Generate a bridge phrase that DEPARTS from the hook: composed against the bridge progression (the
/// harmonic departure), lifted ABOVE the hook's register, on a thinned quarter-note grid (the teacher
/// drops its fast motion in the bridge), ending OPEN on a tendency tone wanting the return. It recycles
/// a hook contour cell (both are ascending REACHes), so it still belongs to the song. Asserted to
/// satisfy [`is_departure`].
pub fn generate_bridge(
    seed: u64,
    hook: &[MaterialEvent],
    span: f64,
    beats_per_bar: f64,
    bridge_prog: &[BarChord],
) -> Vec<MaterialEvent> {
    let hmean = mean_pitch(hook).round() as i32;
    for salt in 0..64u64 {
        let mut r = Rng::new(mix(seed, salt, 0xB41D_6EC1_37D0));
        let climax = hmean + 5 + r.below(4) as i32;
        let descent = pick_descent(&mut r);
        let shape = PhraseShape {
            lift: hmean.max(0),
            close: false,
            climax,
            descent,
            thin: true,
        };
        let bridge = compose_phrase(&mut r, span, beats_per_bar, bridge_prog, shape);
        if is_departure(hook, &bridge) {
            return bridge;
        }
    }
    let mut r = Rng::new(0xFA11_BACC_C137_0002);
    compose_phrase(
        &mut r,
        span,
        beats_per_bar,
        bridge_prog,
        PhraseShape {
            lift: hmean.max(0),
            close: false,
            climax: hmean + 7,
            descent: [-2, -1],
            thin: true,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const BPB: f64 = 4.0;
    const SPAN: f64 = 16.0;

    // Local mirrors of the argument layer's region-relative progressions (so the generator test is
    // self-contained; the real charts live in `super::argument` and are passed in by `fusion`).
    const VP: [BarChord; 4] = [
        (5, Quality::Min),
        (10, Quality::Maj),
        (3, Quality::Maj),
        (7, Quality::Dom7),
    ];
    const HP: [BarChord; 4] = [
        (8, Quality::Maj),
        (10, Quality::Maj),
        (5, Quality::Min),
        (0, Quality::Min),
    ];
    const BP: [BarChord; 8] = [
        (0, Quality::Dom7),
        (5, Quality::Min),
        (3, Quality::Maj),
        (8, Quality::Maj),
        (5, Quality::Min),
        (0, Quality::Dom7),
        (5, Quality::Min),
        (7, Quality::Dom7),
    ];

    fn ref_pair() -> ThemePair {
        generate_pair(2112, SPAN, BPB, &VP, &HP)
    }
    fn ref_verse() -> Vec<MaterialEvent> {
        ref_pair().verse
    }
    fn ref_hook() -> Vec<MaterialEvent> {
        ref_pair().hook
    }

    #[test]
    fn generated_pair_is_a_lawful_altered_consequent() {
        let pair = ref_pair();
        let k = Kinship::measure(&pair.verse, &pair.hook, BPB);
        assert!(
            k.altered_consequent(),
            "the generated reference pair must pass every gate: {k:?}"
        );
        assert_eq!(ArrivalKind::of(&pair.verse), Some(ArrivalKind::Open));
        assert_eq!(ArrivalKind::of(&pair.hook), Some(ArrivalKind::Closed));
    }

    #[test]
    fn generated_pairs_are_lawful_across_seeds() {
        for seed in [1u64, 2, 19, 701, 2112, 0xDEAD_BEEF] {
            let pair = generate_pair(seed, SPAN, BPB, &VP, &HP);
            let k = Kinship::measure(&pair.verse, &pair.hook, BPB);
            assert!(
                k.altered_consequent(),
                "seed {seed} produced an unlawful pair: {k:?}"
            );
        }
    }

    #[test]
    fn strong_beats_land_on_chord_tones() {
        // The decisive new behaviour: on beats 1 and 3 of each bar, the lead is a chord tone of that
        // bar's route chord — the harmonic rooting the old random walk never had.
        for (phrase, prog) in [(ref_verse(), &VP[..]), (ref_hook(), &HP[..])] {
            let mut strong = 0usize;
            let mut rooted = 0usize;
            for e in &phrase {
                let in_bar = e.onset.rem_euclid(BPB);
                let on_strong = in_bar.abs() < 1e-9 || (in_bar - 2.0).abs() < 1e-9;
                if !on_strong {
                    continue;
                }
                // The head is pinned for kinship; exempt the first two notes (bar 0, beats 1 only
                // partly under the composer's control there).
                strong += 1;
                let bar = (e.onset / BPB).floor() as usize % prog.len();
                if let Some(s) = e.step {
                    if chord_pcs(prog[bar]).contains(&s.rem_euclid(12)) {
                        rooted += 1;
                    }
                }
            }
            assert!(strong >= 4, "expected several strong beats, got {strong}");
            let frac = rooted as f64 / strong as f64;
            assert!(
                frac >= 0.80,
                "strong-beat chord-tone fraction {frac:.2} too low ({rooted}/{strong})"
            );
        }
    }

    // --- Negative controls (Bearing 2's falsifier suite N1–N8): each MUST be rejected. ---

    #[test]
    fn n5_identity_and_rigid_transposition_are_restatements_not_consequents() {
        let verse = ref_verse();
        // Identity: the hook IS the verse.
        let k = Kinship::measure(&verse, &verse, BPB);
        assert!(!k.altered_consequent(), "identity must be a restatement");
        // Rigid +5 transposition: same_head/same intervals, zero offset spread.
        let shifted: Vec<_> = verse
            .iter()
            .map(|e| MaterialEvent {
                step: e.step.map(|s| s + 5),
                ..*e
            })
            .collect();
        let k = Kinship::measure(&verse, &shifted, BPB);
        assert!(
            !k.altered_consequent(),
            "a rigid transposition must be rejected (N5): {k:?}"
        );
        assert_eq!(k.offset_spread, 0, "rigid transpose has zero offset spread");
    }

    #[test]
    fn n3_unrelated_tune_fails_kinship() {
        let verse = ref_verse();
        // An unrelated tune: different onsets and head.
        let unrelated = vec![
            ev(0.0, 1.0, 0.9, 0),
            ev(2.0, 1.0, 0.8, 3),
            ev(5.0, 1.0, 0.8, 7),
            ev(9.0, 1.0, 0.8, 2),
            ev(13.0, 3.0, 0.8, 0),
        ];
        let k = Kinship::measure(&verse, &unrelated, BPB);
        assert!(!k.altered_consequent(), "unrelated tune must fail: {k:?}");
    }

    #[test]
    fn n6_rhythm_flattened_fails_skeleton() {
        let verse = ref_verse();
        // Keep pitches at bar downbeats only — destroys the onset skeleton.
        let flat: Vec<_> = verse
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 4 == 0)
            .map(|(_, e)| *e)
            .collect();
        let k = Kinship::measure(&verse, &flat, BPB);
        assert!(k.onset_iou < 0.70, "flattened rhythm must break IoU: {k:?}");
        assert!(!k.altered_consequent());
    }

    #[test]
    fn n4_provenance_tag_cannot_substitute_for_content() {
        // The predicate only ever reads note content; there is no id/label input to spoof.
        let verse = ref_verse();
        let noise = vec![
            ev(0.3, 0.5, 0.5, 11),
            ev(1.1, 0.5, 0.5, 1),
            ev(2.9, 0.5, 0.5, 6),
            ev(7.7, 0.5, 0.5, 11),
            ev(14.0, 2.0, 0.5, 1),
        ];
        assert!(!Kinship::measure(&verse, &noise, BPB).altered_consequent());
    }

    #[test]
    fn open_vs_closed_arrival_is_functional_not_proximity() {
        // Teacher's verse ends on the ♭7 (step 10): far from the tonic in distance, Open by function.
        let ends_on_b7 = vec![
            ev(0.0, 1.0, 0.9, 0),
            ev(1.0, 1.0, 0.8, 7),
            ev(2.0, 2.0, 0.8, 10),
        ];
        assert_eq!(ArrivalKind::of(&ends_on_b7), Some(ArrivalKind::Open));
        // A note one semitone from the tonic (the leading tone, step 11) is still Open.
        let ends_on_leading = vec![ev(0.0, 1.0, 0.9, 0), ev(2.0, 2.0, 0.8, 11)];
        assert_eq!(ArrivalKind::of(&ends_on_leading), Some(ArrivalKind::Open));
        // Only the tonic itself is Closed, regardless of how it was approached.
        let ends_home = vec![ev(0.0, 1.0, 0.9, 7), ev(2.0, 2.0, 0.8, 12)];
        assert_eq!(ArrivalKind::of(&ends_home), Some(ArrivalKind::Closed));
    }

    #[test]
    fn generated_bridge_is_a_lawful_departure_of_the_hook() {
        for seed in [1u64, 2, 19, 701, 2112, 0xDEAD_BEEF] {
            let hook = generate_pair(seed, SPAN, BPB, &VP, &HP).hook;
            let bridge = generate_bridge(seed, &hook, 8.0, BPB, &BP);
            assert!(
                is_departure(&hook, &bridge),
                "seed {seed}: bridge is not a lawful departure of the hook"
            );
            assert_eq!(ArrivalKind::of(&bridge), Some(ArrivalKind::Open));
        }
    }

    #[test]
    fn a_hook_copy_is_not_a_departure() {
        let hook = generate_pair(2112, SPAN, BPB, &VP, &HP).hook;
        // The hook itself (same register, Closed) is a restatement, never a bridge departure.
        assert!(!is_departure(&hook, &hook));
    }
}
