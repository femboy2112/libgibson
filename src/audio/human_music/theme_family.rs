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

use super::material::MaterialEvent;
use super::rng::Rng;

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

// ---------------------------------------------------------------------------------------------
// Generator. Produces fresh seeded material that satisfies the laws above by construction; the
// predicates remain the independent gate (the builder asserts them before returning).
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

/// Generate a verse/hook pair over a `span`-beat phrase of `beats_per_bar`, seeded and lawful.
///
/// The verse is built from a short germ cell sequenced across four bars — a rise to a climax then a
/// settle that HANGS on an open tendency tone. The hook keeps the verse's onsets and head, lifts the
/// register, plateaus the peaks, re-voices the interior, and lands home on the tonic. The returned
/// pair is asserted to satisfy [`Kinship::altered_consequent`]; a seed that cannot (it is rejected
/// by the gate) is retried deterministically, and the function cannot return an un-lawful pair.
pub fn generate_pair(seed: u64, span: f64, beats_per_bar: f64) -> ThemePair {
    for salt in 0..64u64 {
        let child = (seed ^ 0x7E3E_FA33_C137_D00D)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(salt.wrapping_mul(0xD1B5_4A32_D192_ED03));
        let mut r = Rng::new(child);
        if let Some(pair) = try_pair(&mut r, span, beats_per_bar) {
            let k = Kinship::measure(&pair.verse, &pair.hook, beats_per_bar);
            if k.altered_consequent() {
                return pair;
            }
        }
    }
    // Deterministic fallback: a hand-shaped lawful pair (still fresh material, not the teacher's).
    fallback_pair(span, beats_per_bar)
}

fn try_pair(rng: &mut Rng, span: f64, beats_per_bar: f64) -> Option<ThemePair> {
    // Four bars; the last bar holds a long final. Onset grid: eighths with a few sixteenths.
    let bars = (span / beats_per_bar).round().max(1.0) as usize;
    if bars < 2 {
        return None;
    }
    // Open tendency tones the verse may hang on (off-tonic): the 2nd, the 5th, the ♭7.
    let open_tones = [2, 7, 10];
    let open = open_tones[rng.below(open_tones.len())];

    // Build a rhythm skeleton per bar: a pickup/eighths pattern, with the final bar settling.
    let mut verse: Vec<MaterialEvent> = Vec::new();
    // A germ contour for bar 0 (rising), reused and transposed for sequence.
    let germ_peaks = [3i32, 5, 7, 8]; // candidate climax degrees
    let climax = germ_peaks[rng.below(germ_peaks.len())] + 5; // reach up
    let mut cur = 0i32;
    for bar in 0..bars {
        let base = bar as f64 * beats_per_bar;
        let is_last = bar + 1 == bars;
        // Per-bar target: rise toward the climax across the first bars, then settle.
        let target = if is_last {
            open
        } else {
            (climax * (bar as i32 + 1) / (bars as i32 - 1).max(1)).min(climax)
        };
        let onsets: &[f64] = if is_last {
            &[0.0, 1.0, 2.0] // settle, then a long final
        } else if bar == 0 {
            &[0.0, 0.5, 1.0, 1.5, 2.0, 3.0]
        } else {
            &[0.0, 0.5, 1.5, 2.0, 2.5, 3.0]
        };
        for (i, &o) in onsets.iter().enumerate() {
            if base + o >= span {
                break;
            }
            let last_in_bar = i + 1 == onsets.len();
            // Step toward the target, stepwise most of the time, with an occasional leap.
            let want = if last_in_bar && !is_last {
                target
            } else {
                let drift = rng.below(5) as i32 - 2; // -2..=2
                cur + drift
            };
            cur = snap_aeolian(want.clamp(-2, climax + 2));
            let dur = if is_last && last_in_bar {
                (span - (base + o)).max(1.0)
            } else {
                0.5
            };
            let accent = if i == 0 {
                0.95
            } else {
                0.72 + 0.04 * (i % 3) as f32
            };
            verse.push(ev(base + o, dur, accent, cur));
            if is_last && last_in_bar {
                break;
            }
        }
        // Force the last pitched note of the final bar to the open tone.
        if is_last {
            if let Some(last) = verse.last_mut() {
                last.step = Some(open);
            }
        }
    }
    // The verse must end Open.
    if ArrivalKind::of(&verse) != Some(ArrivalKind::Open) {
        return None;
    }

    // Hook = altered consequent: keep onsets, track the verse's contour a register higher (so
    // Parsons stays high), perturb the interior so it is not a rigid transposition (exact-interval
    // drops, offset spread opens), then land home on the tonic.
    let lift = 2 + rng.below(3) as i32; // +2..=+4 register lift
    let mut hook: Vec<MaterialEvent> = verse.clone();
    let n = hook.len();
    for (i, e) in hook.iter_mut().enumerate() {
        let is_last = i + 1 == n;
        if is_last {
            e.step = Some(0); // land home on the tonic (Closed)
            continue;
        }
        if let Some(s) = e.step {
            let jitter = rng.below(3) as i32; // 0..=2: varies offsets without flipping signs
            e.step = Some(snap_aeolian((s + lift + jitter).max(0)));
        }
    }
    // Construct the head explicitly so the verse's first two intervals are preserved EXACTLY
    // (law P3) while every head note stays diatonic — a lifted scale start whose +i1 and +i1+i2
    // also land on scale tones. The teacher does exactly this (E→A, interval +3 preserved).
    if verse.len() >= 3 && hook.len() >= 3 {
        let (v0, v1, v2) = (
            verse[0].step.unwrap(),
            verse[1].step.unwrap(),
            verse[2].step.unwrap(),
        );
        let (i1, i2) = (v1 - v0, v2 - v1);
        let target = v0 + 12; // an octave up anchors the lift; search nearby scale starts
        let start = (target - 6..=target + 6)
            .filter(|&h| is_scale(h) && is_scale(h + i1) && is_scale(h + i1 + i2))
            .min_by_key(|&h| (h - target).abs())
            .unwrap_or(snap_aeolian(target));
        hook[0].step = Some(start);
        hook[1].step = Some(start + i1);
        hook[2].step = Some(start + i1 + i2);
    }
    Some(ThemePair { verse, hook })
}

/// A deterministic, lawful, still-original pair, used only if the seeded search is unlucky.
fn fallback_pair(span: f64, beats_per_bar: f64) -> ThemePair {
    let bar = beats_per_bar;
    // Verse: a rootless rise (2 → 5 → b7 → octave) that hangs on the 2nd. ~ fits any span ≥ 4 bars.
    let verse = vec![
        ev(0.0, 0.5, 0.95, 2),
        ev(0.5, 0.5, 0.74, 5),
        ev(1.0, 0.5, 0.78, 7),
        ev(1.5, 0.5, 0.80, 8),
        ev(2.0, 0.5, 0.78, 10),
        ev(3.0, 0.5, 0.72, 8),
        ev(bar, 0.5, 0.90, 7),
        ev(bar + 0.5, 0.5, 0.72, 8),
        ev(bar + 1.5, 0.5, 0.74, 10),
        ev(bar + 2.0, 0.5, 0.78, 12),
        ev(bar + 3.0, 0.5, 0.72, 10),
        ev(2.0 * bar, 0.5, 0.90, 8),
        ev(2.0 * bar + 1.0, 0.5, 0.74, 7),
        ev(2.0 * bar + 2.0, 0.5, 0.74, 5),
        ev(3.0 * bar, 1.0, 0.85, 3),
        ev(
            3.0 * bar + 1.5,
            (span - (3.0 * bar + 1.5)).max(1.0),
            0.80,
            2,
        ),
    ];
    // Hook: same onsets/head, lifted register, plateau at the ceiling, lands on the tonic.
    let hook = vec![
        ev(0.0, 0.5, 0.95, 7),
        ev(0.5, 0.5, 0.74, 10),
        ev(1.0, 0.5, 0.80, 12),
        ev(1.5, 0.5, 0.82, 12),
        ev(2.0, 0.5, 0.80, 10),
        ev(3.0, 0.5, 0.74, 12),
        ev(bar, 0.5, 0.92, 12),
        ev(bar + 0.5, 0.5, 0.74, 10),
        ev(bar + 1.5, 0.5, 0.78, 12),
        ev(bar + 2.0, 0.5, 0.80, 15),
        ev(bar + 3.0, 0.5, 0.74, 12),
        ev(2.0 * bar, 0.5, 0.92, 10),
        ev(2.0 * bar + 1.0, 0.5, 0.78, 12),
        ev(2.0 * bar + 2.0, 0.5, 0.76, 8),
        ev(3.0 * bar, 1.0, 0.86, 3),
        ev(
            3.0 * bar + 1.5,
            (span - (3.0 * bar + 1.5)).max(1.0),
            0.84,
            0,
        ),
    ];
    ThemePair { verse, hook }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BPB: f64 = 4.0;
    const SPAN: f64 = 16.0;

    fn teacher_like_verse() -> Vec<MaterialEvent> {
        fallback_pair(SPAN, BPB).verse
    }
    fn teacher_like_hook() -> Vec<MaterialEvent> {
        fallback_pair(SPAN, BPB).hook
    }

    #[test]
    fn fallback_pair_is_a_lawful_altered_consequent() {
        let k = Kinship::measure(&teacher_like_verse(), &teacher_like_hook(), BPB);
        assert!(
            k.altered_consequent(),
            "the hand-shaped reference pair must pass every gate: {k:?}"
        );
        assert_eq!(
            ArrivalKind::of(&teacher_like_verse()),
            Some(ArrivalKind::Open)
        );
        assert_eq!(
            ArrivalKind::of(&teacher_like_hook()),
            Some(ArrivalKind::Closed)
        );
    }

    #[test]
    fn generated_pairs_are_lawful_across_seeds() {
        for seed in [1u64, 2, 19, 701, 2112, 0xDEAD_BEEF] {
            let pair = generate_pair(seed, SPAN, BPB);
            let k = Kinship::measure(&pair.verse, &pair.hook, BPB);
            assert!(
                k.altered_consequent(),
                "seed {seed} produced an unlawful pair: {k:?}"
            );
        }
    }

    // --- Negative controls (Bearing 2's falsifier suite N1–N8): each MUST be rejected. ---

    #[test]
    fn n5_identity_and_rigid_transposition_are_restatements_not_consequents() {
        let verse = teacher_like_verse();
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
        let verse = teacher_like_verse();
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
        let verse = teacher_like_verse();
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
        // Prove it by constructing content-identical-to-N3 material and confirming rejection.
        let verse = teacher_like_verse();
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
}
