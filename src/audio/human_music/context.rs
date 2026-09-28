//! **Contextual harmony** — harmonic function as a *relation*, not a property of a scale degree.
//!
//! Through Round VI the engine carried two fixed tables: `degree_implied_tension(degree)` (degree 6
//! is "0.90, the leading-tone chord") and `function_of_degree(degree)` (degree 4 is "Dominant").
//! Both silently assume Ionian functional harmony. In A Aeolian, degree 4 is E minor — a natural
//! minor `v` with no G♯ leading tone, so almost none of the classical dominant pull — and degree 6
//! is G major, a modal `♭VII`, not a diminished leading-tone chord. In Mixolydian the seventh degree
//! is likewise modal `♭VII` colour. The tables were wrong for the modes the worlds actually use.
//!
//! This module replaces "degree → meaning" with meaning derived from **pitch content in context**:
//!
//! - [`PullEvidence`] — does a chord concretely pull toward a target root? (a leading tone a
//!   semitone below it, a tritone that resolves inward onto it, a root a fifth above it).
//! - [`HarmonicRelation`] — the relation a chord bears to its tonal region and its neighbours
//!   (prolongation, departure, preparation, dominant-to-a-specific-target, tonicization, a
//!   deflected arrival, modal shift, pedal, chromatic connector, arrival).
//! - [`TensionVector`] — tension as a small inspectable vector (pull, distance, colour, strain,
//!   surprise, openness) instead of one number per degree.
//! - [`PitchPalette`] — the *local* pitch field a melody may use over one harmony: structural chord
//!   tones, guide tones, licensed tensions, contextual colour, expensive tones, and the next
//!   context's targets. Several lawful chord-scales are considered and the one most continuous with
//!   the previous palette is chosen — a palette is not "new chord, run its scale".
//! - [`HarmonicContext`] — all of the above for one [`ChordSpan`], analysed over a whole
//!   progression by [`analyze`].

use super::harmony::ChordSpan;
use super::theory::{Chord, Function, Mode, Quality, Scale};

/// Concrete evidence that a chord pulls toward `target_root`. Pull is a property of pitch content
/// relative to a *specific* target, never of a scale degree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PullEvidence {
    /// The chord contains the pitch class a semitone below the target root (a leading tone).
    pub leading_tone: bool,
    /// The chord contains a tritone whose members resolve by semitone inward/outward onto the
    /// target triad (the 3rd–7th tritone of a dominant seventh).
    pub resolving_tritone: bool,
    /// The chord's root lies a perfect fifth above the target root (root motion down a fifth).
    pub root_fifth_above: bool,
    /// The chord's root lies a semitone above the target root (a tritone-substitute approach).
    pub root_semitone_above: bool,
}

impl PullEvidence {
    /// Measure the pull of `chord` toward `target_root`.
    pub fn of(chord: &Chord, target_root: i32) -> PullEvidence {
        let pcs = chord.pitch_classes();
        let t = target_root.rem_euclid(12);
        let lt = (t - 1).rem_euclid(12);
        let has = |pc: i32| pcs.contains(&pc.rem_euclid(12));
        // A tritone pair (a, a+6) resolves onto the target triad when each member lies a semitone
        // from some target-triad tone (major or minor third: include both 3rds).
        let target_tones = [
            t,
            (t + 3).rem_euclid(12),
            (t + 4).rem_euclid(12),
            (t + 7) % 12,
        ];
        let near_target = |pc: i32| {
            target_tones
                .iter()
                .any(|&tt| (pc - tt).rem_euclid(12) == 1 || (tt - pc).rem_euclid(12) == 1)
        };
        let resolving_tritone = pcs.iter().any(|&a| {
            let b = (a + 6).rem_euclid(12);
            has(b) && near_target(a) && near_target(b)
        });
        PullEvidence {
            leading_tone: has(lt) && !has(t),
            resolving_tritone,
            root_fifth_above: chord.root_pc == (t + 7).rem_euclid(12),
            root_semitone_above: chord.root_pc == (t + 1).rem_euclid(12),
        }
    }

    /// A scalar pull strength in `[0, 1]`. A classical dominant (fifth above + leading tone +
    /// tritone) is ~1; a natural-minor `v` (fifth above only) is weak; a chord with no evidence is 0.
    pub fn strength(&self) -> f32 {
        let mut s = 0.0;
        if self.leading_tone {
            s += 0.40;
        }
        if self.resolving_tritone {
            s += 0.30;
        }
        if self.root_fifth_above {
            s += 0.30;
        }
        if self.root_semitone_above && self.resolving_tritone {
            s += 0.25; // a tritone substitute borrows the tritone, approaching from a semitone above
        }
        f32::min(s, 1.0)
    }

    /// Whether the evidence licenses a claim of *dominant* function toward the target: a
    /// fifth-above (or semitone-above substitute) root together with a leading tone or a resolving
    /// tritone. Root motion alone is not enough — that is exactly the natural-minor `v`.
    pub fn is_dominant(&self) -> bool {
        (self.root_fifth_above && (self.leading_tone || self.resolving_tritone))
            || (self.root_semitone_above && self.resolving_tritone)
            // A rootless dominant (vii°): the leading tone AND the resolving tritone together.
            || (self.leading_tone && self.resolving_tritone)
    }
}

/// The relation a chord bears to its tonal region and its neighbours — the contextual replacement
/// for a universal `Chord -> Function` table. Deliberately small: only what the engine realizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarmonicRelation {
    /// Arrival on the region's tonic.
    Arrival,
    /// Stability kept: shares at least two tones with the previous harmony (or restates it).
    Prolong,
    /// Motion away from the region tonic without concrete pull (a natural-minor `v`, a modal
    /// `♭VII`, a subdominant step).
    Depart,
    /// Prepares a following dominant: its root lies a fifth above that dominant's root.
    Prepare,
    /// Concrete dominant pull toward `target` — and the next harmony really is `target`.
    DominantTo { target: i32 },
    /// An applied dominant that lands on a non-tonic `target`: a local tonicization.
    Tonicize { target: i32 },
    /// The previous harmony pulled toward `expected`, and this one is not it: an evasion. The
    /// relation keeps `common` tones with the expected arrival (so it reads as a miss, not a
    /// non-sequitur).
    Deflected { expected: i32, common: u8 },
    /// Borrows pitch classes outside the region's scale without dominant pull (mixture).
    ModalShift,
    /// The bass/root is held while the upper harmony changes.
    Pedal,
    /// A chromatic passing harmony whose root moves by semitone into the next.
    ChromaticConnector,
}

impl HarmonicRelation {
    /// A short lowercase label for dumps and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            HarmonicRelation::Arrival => "arrival",
            HarmonicRelation::Prolong => "prolong",
            HarmonicRelation::Depart => "depart",
            HarmonicRelation::Prepare => "prepare",
            HarmonicRelation::DominantTo { .. } => "dominant-to",
            HarmonicRelation::Tonicize { .. } => "tonicize",
            HarmonicRelation::Deflected { .. } => "deflected",
            HarmonicRelation::ModalShift => "modal-shift",
            HarmonicRelation::Pedal => "pedal",
            HarmonicRelation::ChromaticConnector => "chromatic",
        }
    }
}

/// Tension as an inspectable vector, not one number per degree. Every component is in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TensionVector {
    /// Concrete pull toward the region tonic (from [`PullEvidence`]).
    pub pull: f32,
    /// Tonal distance: root distance from the tonic on the circle of fifths, plus out-of-region
    /// pitch content.
    pub distance: f32,
    /// Sonority colour/roughness: extensions beyond the triad and semitone/tritone clashes.
    pub color: f32,
    /// Voice-leading strain from the previous harmony (how far its tones must move).
    pub strain: f32,
    /// Deviation from what the previous harmony led the ear to expect.
    pub surprise: f32,
    /// Stability/openness: shares the tonic's triad, no tritone, no pull.
    pub openness: f32,
}

impl TensionVector {
    /// A scalar projection for the few call sites that still need one number (a bounded search
    /// fit). It is a *projection* of the vector, never the source of meaning.
    pub fn scalar(&self) -> f32 {
        (0.34 * self.pull
            + 0.26 * self.distance
            + 0.12 * self.color
            + 0.10 * self.strain
            + 0.18 * self.surprise
            + 0.20 * (1.0 - self.openness)
            - 0.10)
            .clamp(0.0, 1.0)
    }
}

/// The local pitch field over one harmony (pitch classes `0..=11`).
#[derive(Debug, Clone, PartialEq)]
pub struct PitchPalette {
    /// The chord's own tones (root, 3rd, 5th, 7th/6th and any written extension).
    pub chord_tones: Vec<i32>,
    /// The guide tones: the 3rd and the 7th (or 6th) — the pitches that define the sonority.
    pub guide_tones: Vec<i32>,
    /// Tensions licensed over this chord by the chosen chord-scale (9ths/11ths/13ths that do not
    /// clash a semitone above a chord tone).
    pub tensions: Vec<i32>,
    /// Contextual colour: the chosen scale's remaining tones that are neither chord tones nor
    /// licensed tensions nor expensive (usable on weak beats, in lines).
    pub color: Vec<i32>,
    /// Expensive tones: a semitone above a chord tone (the classic "avoid" 11 over a major chord).
    /// Usable only as short weak-beat passing/neighbour motion.
    pub expensive: Vec<i32>,
    /// The next harmony's guide tones — the targets a line should be heading for.
    pub next_targets: Vec<i32>,
    /// The chord-scale this palette was drawn from, as a scale (tonic + mode).
    pub scale: Scale,
}

impl PitchPalette {
    /// Every pitch class the palette allows at all (chord ∪ tensions ∪ colour ∪ expensive).
    pub fn all(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self
            .chord_tones
            .iter()
            .chain(&self.tensions)
            .chain(&self.color)
            .chain(&self.expensive)
            .copied()
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Whether `pc` is a stable target over this harmony (a chord tone or a licensed tension).
    pub fn is_stable(&self, pc: i32) -> bool {
        let pc = pc.rem_euclid(12);
        self.chord_tones.contains(&pc) || self.tensions.contains(&pc)
    }

    /// Whether `pc` belongs to the palette at all.
    pub fn contains(&self, pc: i32) -> bool {
        let pc = pc.rem_euclid(12);
        self.chord_tones.contains(&pc)
            || self.tensions.contains(&pc)
            || self.color.contains(&pc)
            || self.expensive.contains(&pc)
    }
}

/// Everything the engine knows about one harmony in context.
#[derive(Debug, Clone, PartialEq)]
pub struct HarmonicContext {
    pub start_beat: f64,
    pub dur_beats: f32,
    pub chord: Chord,
    /// The bass pitch class (the root, until inversions are generated).
    pub bass_pc: i32,
    /// The tonal region in force (tonic + mode).
    pub region: Scale,
    /// The relation this harmony bears to its region and neighbours.
    pub relation: HarmonicRelation,
    /// The contextual tension vector.
    pub tension: TensionVector,
    /// The local pitch palette.
    pub palette: PitchPalette,
    /// If this harmony pulls concretely toward some root, which one the ear expects next.
    pub expects: Option<i32>,
}

/// The pitch classes of a scale.
fn scale_pcs(s: &Scale) -> Vec<i32> {
    s.mode
        .intervals()
        .iter()
        .map(|&i| (s.tonic_pc + i).rem_euclid(12))
        .collect()
}

/// The 3rd and 7th/6th (guide tones) of a chord, as pitch classes.
pub fn guide_tones(chord: &Chord) -> Vec<i32> {
    let iv = chord.quality.intervals();
    let mut g = Vec::with_capacity(2);
    // The 3rd (or the suspended 2nd/4th standing in for it).
    if let Some(&third) = iv.iter().find(|&&i| matches!(i, 2..=5)) {
        g.push((chord.root_pc + third).rem_euclid(12));
    }
    // The 7th, else a 6th.
    if let Some(&sev) = iv.iter().find(|&&i| i == 10 || i == 11) {
        g.push((chord.root_pc + sev).rem_euclid(12));
    } else if let Some(&six) = iv.iter().find(|&&i| i == 9) {
        g.push((chord.root_pc + six).rem_euclid(12));
    }
    g
}

/// Circle-of-fifths distance between two pitch classes, `0..=6`.
fn fifths_distance(a: i32, b: i32) -> i32 {
    // Position on the circle of fifths: pc * 7 mod 12.
    let pa = (a * 7).rem_euclid(12);
    let pb = (b * 7).rem_euclid(12);
    let d = (pa - pb).rem_euclid(12);
    d.min(12 - d)
}

/// Common pitch classes between two chords.
pub fn common_tones(a: &Chord, b: &Chord) -> u8 {
    let bp = b.pitch_classes();
    a.pitch_classes().iter().filter(|p| bp.contains(p)).count() as u8
}

/// Minimal total semitone motion to move `a`'s pitch classes onto `b`'s (each tone of `a` to its
/// nearest tone of `b`) — a cheap voice-leading-strain proxy.
pub fn pc_motion(a: &Chord, b: &Chord) -> i32 {
    let bp = b.pitch_classes();
    a.pitch_classes()
        .iter()
        .map(|&p| {
            bp.iter()
                .map(|&q| {
                    let d = (p - q).rem_euclid(12);
                    d.min(12 - d)
                })
                .min()
                .unwrap_or(0)
        })
        .sum()
}

/// Candidate chord-scales for `chord` in `region`, most idiomatic first. Several lawful choices are
/// offered where theory allows them; the palette builder picks by continuity.
fn chord_scale_candidates(chord: &Chord, region: &Scale, next: Option<&Chord>) -> Vec<Scale> {
    let r = chord.root_pc;
    let pcs = chord.pitch_classes();
    let mut out = Vec::new();
    // 1. The region itself, when it contains every chord tone (the diatonic reading).
    if pcs.iter().all(|&p| region.contains_pc(p)) {
        out.push(*region);
    }
    // 2. If the chord is an applied dominant to the next chord, the target's harmonic-minor or
    //    major region (so E7 -> Am reads the G♯ as the local leading tone).
    if let Some(n) = next {
        let ev = PullEvidence::of(chord, n.root_pc);
        if ev.is_dominant() {
            let minor_target = n
                .pitch_classes()
                .contains(&((n.root_pc + 3).rem_euclid(12)));
            out.push(Scale::new(
                n.root_pc,
                if minor_target {
                    Mode::HarmonicMinor
                } else {
                    Mode::Ionian
                },
            ));
        }
    }
    // 3. Quality-idiomatic modes on the chord's own root.
    let iv = chord.quality.intervals();
    let minor3 = iv.contains(&3);
    let has_b7 = iv.contains(&10);
    let has_maj7 = iv.contains(&11);
    let has_b5 = iv.contains(&6);
    let modes: &[Mode] = match (minor3, has_b7, has_maj7, has_b5) {
        (true, _, _, true) => &[Mode::Locrian],
        (true, true, _, _) => &[Mode::Dorian, Mode::Aeolian, Mode::Phrygian],
        (true, _, true, _) => &[Mode::HarmonicMinor],
        (true, _, _, _) => &[Mode::Dorian, Mode::Aeolian],
        (false, true, _, _) => &[Mode::Mixolydian],
        (false, _, true, _) => &[Mode::Ionian, Mode::Lydian],
        _ => &[Mode::Ionian, Mode::Lydian, Mode::Mixolydian],
    };
    for &m in modes {
        let s = Scale::new(r, m);
        if pcs.iter().all(|&p| s.contains_pc(p)) && !out.contains(&s) {
            out.push(s);
        }
    }
    if out.is_empty() {
        out.push(Scale::new(r, Mode::Ionian));
    }
    out
}

/// Build the palette for `chord` from a chosen chord-scale.
fn palette_from(chord: &Chord, scale: Scale, next: Option<&Chord>) -> PitchPalette {
    let chord_tones = chord.pitch_classes();
    let guide = guide_tones(chord);
    let mut tensions = Vec::new();
    let mut color = Vec::new();
    let mut expensive = Vec::new();
    for pc in scale_pcs(&scale) {
        if chord_tones.contains(&pc) {
            continue;
        }
        // A semitone above a chord tone clashes (a minor 9th against it): expensive.
        let clashes = chord_tones.iter().any(|&c| (pc - c).rem_euclid(12) == 1);
        // A whole step above a chord tone (9, 11 over minor, 13) is a licensed tension.
        let tension = chord_tones.iter().any(|&c| (pc - c).rem_euclid(12) == 2);
        if clashes {
            expensive.push(pc);
        } else if tension {
            tensions.push(pc);
        } else {
            color.push(pc);
        }
    }
    PitchPalette {
        chord_tones,
        guide_tones: guide,
        tensions,
        color,
        expensive,
        next_targets: next.map(guide_tones).unwrap_or_default(),
        scale,
    }
}

/// Choose the chord-scale most continuous with the previous palette (fewest changed pitch classes),
/// preferring earlier (more idiomatic) candidates on ties.
fn choose_palette(
    chord: &Chord,
    region: &Scale,
    next: Option<&Chord>,
    prev: Option<&PitchPalette>,
) -> PitchPalette {
    let cands = chord_scale_candidates(chord, region, next);
    let mut best: Option<(i32, PitchPalette)> = None;
    for (rank, s) in cands.into_iter().enumerate() {
        let p = palette_from(chord, s, next);
        let change = prev
            .map(|q| {
                let a = scale_pcs(&p.scale);
                let b = scale_pcs(&q.scale);
                a.iter().filter(|x| !b.contains(x)).count() as i32
            })
            .unwrap_or(0);
        let cost = change * 4 + rank as i32;
        if best.as_ref().is_none_or(|(c, _)| cost < *c) {
            best = Some((cost, p));
        }
    }
    best.map(|(_, p)| p)
        .unwrap_or_else(|| palette_from(chord, *region, next))
}

/// The contextual relation of `chord` given its region and neighbours.
pub fn relate(
    chord: &Chord,
    region: &Scale,
    prev: Option<&Chord>,
    prev_expects: Option<i32>,
    next: Option<&Chord>,
) -> HarmonicRelation {
    let tonic = region.tonic_pc;
    // A deflected arrival: the previous harmony concretely pulled toward something else.
    if let Some(exp) = prev_expects {
        if chord.root_pc != exp.rem_euclid(12) {
            let expected = expected_chord(exp, region);
            return HarmonicRelation::Deflected {
                expected: exp.rem_euclid(12),
                common: common_tones(chord, &expected),
            };
        }
    }
    // Concrete dominant pull toward the actual next harmony.
    if let Some(n) = next {
        let ev = PullEvidence::of(chord, n.root_pc);
        if ev.is_dominant() {
            return if n.root_pc == tonic {
                HarmonicRelation::DominantTo { target: n.root_pc }
            } else {
                HarmonicRelation::Tonicize { target: n.root_pc }
            };
        }
    }
    if chord.root_pc == tonic {
        return match prev {
            Some(p) if p.root_pc == tonic => HarmonicRelation::Prolong,
            _ => HarmonicRelation::Arrival,
        };
    }
    if let Some(p) = prev {
        if p.root_pc == chord.root_pc {
            return HarmonicRelation::Pedal;
        }
    }
    let outside = chord
        .pitch_classes()
        .iter()
        .filter(|&&p| !region.contains_pc(p))
        .count();
    if outside > 0 {
        if let Some(n) = next {
            let step = (n.root_pc - chord.root_pc).rem_euclid(12);
            if step == 1 || step == 11 {
                return HarmonicRelation::ChromaticConnector;
            }
        }
        return HarmonicRelation::ModalShift;
    }
    if let Some(n) = next {
        // Root a fifth above a following chord that itself pulls concretely somewhere: a
        // preparation (the ii of a ii–V).
        if chord.root_pc == (n.root_pc + 7).rem_euclid(12) && expected_target(n, region).is_some() {
            return HarmonicRelation::Prepare;
        }
    }
    if let Some(p) = prev {
        if common_tones(chord, p) >= 2 {
            return HarmonicRelation::Prolong;
        }
    }
    HarmonicRelation::Depart
}

/// The chord the ear expects on `root` in `region`: the region's diatonic triad on that root when
/// the root is in the region, else a major triad.
pub fn expected_chord(root: i32, region: &Scale) -> Chord {
    let root = root.rem_euclid(12);
    let minor = region.contains_pc(root + 3) && !region.contains_pc(root + 4);
    Chord::new(root, if minor { Quality::Min } else { Quality::Maj })
}

/// The root a chord concretely pulls toward, if any: the region tonic first, then the root a fifth
/// below (an applied dominant), then a semitone below (a tritone substitute).
pub fn expected_target(chord: &Chord, region: &Scale) -> Option<i32> {
    let cands = [
        region.tonic_pc,
        (chord.root_pc + 5).rem_euclid(12),
        (chord.root_pc + 11).rem_euclid(12),
    ];
    cands
        .into_iter()
        .find(|&t| PullEvidence::of(chord, t).is_dominant())
}

/// The contextual tension vector of `chord`.
pub fn tension_of(
    chord: &Chord,
    region: &Scale,
    prev: Option<&Chord>,
    relation: HarmonicRelation,
) -> TensionVector {
    let tonic = region.tonic_pc;
    let pull = PullEvidence::of(chord, tonic).strength();
    let outside = chord
        .pitch_classes()
        .iter()
        .filter(|&&p| !region.contains_pc(p))
        .count() as f32;
    let distance =
        (fifths_distance(chord.root_pc, tonic) as f32 / 6.0 * 0.7 + outside * 0.2).min(1.0);
    let iv = chord.quality.intervals();
    let extensions = iv.len().saturating_sub(3) as f32;
    let pcs = chord.pitch_classes();
    let mut clashes = 0.0;
    for (i, &a) in pcs.iter().enumerate() {
        for &b in &pcs[i + 1..] {
            let d = (a - b).rem_euclid(12);
            if d == 1 || d == 11 || d == 6 {
                clashes += 1.0;
            }
        }
    }
    let color = (extensions * 0.25 + clashes * 0.2).min(1.0);
    let strain = prev
        .map(|p| (pc_motion(p, chord) as f32 / 8.0).min(1.0))
        .unwrap_or(0.0);
    let surprise = match relation {
        HarmonicRelation::Deflected { common, .. } => (1.0 - 0.2 * common as f32).clamp(0.3, 1.0),
        HarmonicRelation::ModalShift | HarmonicRelation::ChromaticConnector => 0.35,
        _ => 0.0,
    };
    let tonic_triad = expected_chord(tonic, region);
    let has_tritone = pcs.iter().any(|&a| pcs.contains(&((a + 6).rem_euclid(12))));
    let openness = ((common_tones(chord, &tonic_triad) as f32 / 3.0)
        * if has_tritone { 0.5 } else { 1.0 }
        * (1.0 - pull))
        .clamp(0.0, 1.0);
    TensionVector {
        pull,
        distance,
        color,
        strain,
        surprise,
        openness,
    }
}

/// Analyse a progression in `region` into per-harmony contexts: relation, tension vector, palette
/// (chosen for continuity with the previous one) and expectation.
pub fn analyze(chords: &[ChordSpan], region: &Scale) -> Vec<HarmonicContext> {
    let mut out: Vec<HarmonicContext> = Vec::with_capacity(chords.len());
    for (i, span) in chords.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| &chords[j].chord);
        let next = chords.get(i + 1).map(|c| &c.chord);
        let prev_expects = out.last().and_then(|c| c.expects);
        let relation = relate(&span.chord, region, prev, prev_expects, next);
        let tension = tension_of(&span.chord, region, prev, relation);
        let palette = choose_palette(&span.chord, region, next, out.last().map(|c| &c.palette));
        out.push(HarmonicContext {
            start_beat: span.start_beat,
            dur_beats: span.dur_beats,
            chord: span.chord,
            bass_pc: span.chord.root_pc,
            region: *region,
            relation,
            tension,
            palette,
            expects: expected_target(&span.chord, region),
        });
    }
    out
}

/// The compatibility projection of a chord's contextual behaviour onto the three-valued
/// [`Function`]: `Dominant` only with concrete pull evidence toward the region tonic, `Tonic` for
/// the tonic and chords sharing two tones with the tonic triad (its substitutes), `Predominant`
/// (departure) otherwise. A natural-minor `v` and a modal `♭VII` are therefore departures, not
/// dominants. `Function` survives for simple diagnostics; meaning lives in [`HarmonicRelation`].
pub fn contextual_function(chord: &Chord, region: &Scale) -> Function {
    let tonic = region.tonic_pc;
    if PullEvidence::of(chord, tonic).is_dominant() {
        Function::Dominant
    } else if chord.root_pc == tonic || common_tones(chord, &expected_chord(tonic, region)) >= 2 {
        Function::Tonic
    } else {
        Function::Predominant
    }
}

/// The contextual heat of the diatonic triad on `degree` of `region`, in `[0, 1]` — the
/// mode-safe replacement for the old fixed degree table, derived from the chord's
/// [`TensionVector`] (pull, distance, colour, openness) rather than from its index.
pub fn degree_tension(region: &Scale, degree: i32) -> f32 {
    let chord = super::harmony::diatonic_chord(region, degree, false);
    let rel = relate(&chord, region, None, None, None);
    (tension_of(&chord, region, None, rel).scalar() * 1.7).min(1.0)
}

/// The context sounding at `beat` (the latest one starting at or before it).
pub fn context_at(contexts: &[HarmonicContext], beat: f64) -> Option<&HarmonicContext> {
    contexts.iter().rev().find(|c| c.start_beat <= beat + 1e-6)
}

#[cfg(test)]
mod tests {
    use super::super::harmony::diatonic_chord;
    use super::*;

    fn a_aeolian() -> Scale {
        Scale::new(9, Mode::Aeolian)
    }

    /// The Round-VI degree-only function table, preserved verbatim as a failing witness.
    fn legacy_function_of_degree(degree: i32) -> Function {
        match degree.rem_euclid(7) {
            0 | 5 | 2 => Function::Tonic,
            3 | 1 => Function::Predominant,
            _ => Function::Dominant,
        }
    }

    /// The Round-VI degree-only tension table, preserved verbatim as a failing witness.
    fn legacy_degree_implied_tension(degree: i32) -> f32 {
        match degree.rem_euclid(7) {
            0 => 0.10,
            5 => 0.25,
            2 => 0.30,
            3 => 0.45,
            1 => 0.50,
            4 => 0.80,
            _ => 0.90,
        }
    }

    #[test]
    fn a_natural_minor_v_is_not_a_classical_dominant() {
        // A Aeolian degree 4 = E minor (E G B): root a fifth above A, but G natural, not G♯ — no
        // leading tone and no tritone. The legacy table calls it Dominant; the evidence does not.
        let s = a_aeolian();
        let v = diatonic_chord(&s, 4, false);
        assert_eq!(v, Chord::new(4, Quality::Min));
        assert_eq!(legacy_function_of_degree(4), Function::Dominant); // the old claim
        let ev = PullEvidence::of(&v, 9);
        assert!(ev.root_fifth_above && !ev.leading_tone && !ev.resolving_tritone);
        assert!(!ev.is_dominant(), "natural-minor v claimed dominant pull");
        // The harmonic-minor V (E G♯ B) does have it.
        let big_v = Chord::new(4, Quality::Maj);
        assert!(PullEvidence::of(&big_v, 9).is_dominant());
        assert!(PullEvidence::of(&big_v, 9).strength() > 2.0 * ev.strength());
    }

    #[test]
    fn a_modal_flat_seven_is_not_a_leading_tone_chord() {
        // A Aeolian degree 6 = G major (♭VII). The legacy tension table rates degree 6 the hottest
        // chord in the key (0.90, "vii°"); in context it has no pull to A at all.
        let s = a_aeolian();
        let bvii = diatonic_chord(&s, 6, false);
        assert_eq!(bvii, Chord::new(7, Quality::Maj));
        assert!((legacy_degree_implied_tension(6) - 0.90).abs() < 1e-6); // the old claim
        let rel = relate(&bvii, &s, Some(&Chord::new(0, Quality::Maj)), None, None);
        assert!(!matches!(
            rel,
            HarmonicRelation::DominantTo { .. } | HarmonicRelation::Tonicize { .. }
        ));
        let tv = tension_of(&bvii, &s, None, rel);
        assert_eq!(
            tv.pull, 0.0,
            "a modal bVII has no concrete pull to the tonic"
        );
        assert!(tv.scalar() < 0.6, "bVII still rated as the hottest chord");
        // Mixolydian: G Mixolydian's degree 6 is F major — modal bVII colour again, no pull to G.
        let mixo = Scale::new(7, Mode::Mixolydian);
        let f = diatonic_chord(&mixo, 6, false);
        assert_eq!(f, Chord::new(5, Quality::Maj));
        assert_eq!(PullEvidence::of(&f, 7).strength(), 0.0);
        // Ionian degree 6 really is the leading-tone chord (B dim in C): it does pull.
        let ion = Scale::new(0, Mode::Ionian);
        let vii = diatonic_chord(&ion, 6, false);
        assert!(PullEvidence::of(&vii, 0).leading_tone);
        assert_eq!(contextual_function(&vii, &ion), Function::Dominant);
        // The contextual heat ranks the Aeolian bVII well below the Ionian vii°, where the legacy
        // table rated them identically (0.90).
        assert!(degree_tension(&s, 6) + 0.3 < degree_tension(&ion, 6));
        // ...and the natural-minor v is a departure, not a dominant, in the compatibility view.
        assert_eq!(
            contextual_function(&diatonic_chord(&s, 4, false), &s),
            Function::Predominant
        );
    }

    #[test]
    fn one_degree_behaves_differently_across_modes() {
        // Degree 4 on the same tonic: Ionian V (pulls), Mixolydian v (no leading tone), Aeolian v.
        let pulls: Vec<bool> = [Mode::Ionian, Mode::Mixolydian, Mode::Aeolian]
            .into_iter()
            .map(|m| {
                let s = Scale::new(0, m);
                let c = diatonic_chord(&s, 4, true);
                PullEvidence::of(&c, 0).is_dominant()
            })
            .collect();
        assert_eq!(pulls, vec![true, false, false]);
    }

    #[test]
    fn a_dominant_claim_needs_evidence_and_an_applied_dominant_tonicizes() {
        // In C major, E7 -> Am: a concrete applied dominant (G♯ leading tone to A).
        let c = Scale::new(0, Mode::Ionian);
        let e7 = Chord::new(4, Quality::Dom7);
        let am = Chord::new(9, Quality::Min);
        assert_eq!(
            relate(&e7, &c, None, None, Some(&am)),
            HarmonicRelation::Tonicize { target: 9 }
        );
        // ...and the melody's local palette over E7 carries the G♯ and reads A harmonic minor.
        let ctx = analyze(
            &[ChordSpan::test(0.0, 4.0, e7), ChordSpan::test(4.0, 4.0, am)],
            &c,
        );
        assert!(ctx[0].palette.chord_tones.contains(&8));
        assert_eq!(ctx[0].palette.scale, Scale::new(9, Mode::HarmonicMinor));
        assert_eq!(ctx[0].palette.next_targets, guide_tones(&am));
        // Em -> Am in C has root motion but no evidence: no dominant claim.
        let em = Chord::new(4, Quality::Min);
        assert!(!matches!(
            relate(&em, &c, None, None, Some(&am)),
            HarmonicRelation::Tonicize { .. } | HarmonicRelation::DominantTo { .. }
        ));
    }

    #[test]
    fn a_deflection_differs_from_its_expected_arrival_but_stays_related() {
        // G7 in C pulls to C; landing on Am instead is a deceptive/deflected arrival that keeps two
        // common tones (C, E) with the expected C major — an evasion, not a non-sequitur.
        let c = Scale::new(0, Mode::Ionian);
        let g7 = Chord::new(7, Quality::Dom7);
        assert_eq!(expected_target(&g7, &c), Some(0));
        let ctx = analyze(
            &[
                ChordSpan::test(0.0, 4.0, g7),
                ChordSpan::test(4.0, 4.0, Chord::new(9, Quality::Min)),
            ],
            &c,
        );
        match ctx[1].relation {
            HarmonicRelation::Deflected { expected, common } => {
                assert_eq!(expected, 0);
                assert!(common >= 2);
            }
            other => panic!("expected a deflection, got {other:?}"),
        }
        assert!(ctx[1].tension.surprise > 0.0);
        // A true arrival (G7 -> C) is not a deflection and carries no surprise.
        let ok = analyze(
            &[
                ChordSpan::test(0.0, 4.0, g7),
                ChordSpan::test(4.0, 4.0, Chord::new(0, Quality::Maj)),
            ],
            &c,
        );
        assert!(matches!(
            ok[0].relation,
            HarmonicRelation::DominantTo { target: 0 }
        ));
        assert_eq!(ok[1].tension.surprise, 0.0);
    }

    #[test]
    fn a_modal_pedal_is_a_pedal_not_a_cadence() {
        // D Dorian vamp: Dm7 -> G (IV of Dorian) over a held D would be a pedal on the same root;
        // same root, new colour: Dm7 -> Dm9.
        let d = Scale::new(2, Mode::Dorian);
        let a = Chord::new(2, Quality::Min7);
        let b = Chord::new(2, Quality::Min9);
        let ctx = analyze(
            &[ChordSpan::test(0.0, 4.0, a), ChordSpan::test(4.0, 4.0, b)],
            &d,
        );
        assert_eq!(ctx[0].relation, HarmonicRelation::Arrival);
        assert_eq!(ctx[1].relation, HarmonicRelation::Prolong);
        // A non-tonic pedal: Em7 -> Em9 in D Dorian.
        let e1 = Chord::new(4, Quality::Min7);
        let e2 = Chord::new(4, Quality::Min9);
        let ctx = analyze(
            &[ChordSpan::test(0.0, 4.0, e1), ChordSpan::test(4.0, 4.0, e2)],
            &d,
        );
        assert_eq!(ctx[1].relation, HarmonicRelation::Pedal);
    }

    #[test]
    fn palettes_stay_continuous_across_consecutive_contexts() {
        // A diatonic ii-V-I in C: every palette should read the region itself (zero pitch change),
        // not three different modes on three roots.
        let c = Scale::new(0, Mode::Ionian);
        let prog = [
            ChordSpan::test(0.0, 4.0, Chord::new(2, Quality::Min7)),
            ChordSpan::test(4.0, 4.0, Chord::new(7, Quality::Dom7)),
            ChordSpan::test(8.0, 4.0, Chord::new(0, Quality::Maj7)),
        ];
        let ctx = analyze(&prog, &c);
        for x in &ctx {
            assert_eq!(scale_pcs(&x.palette.scale).len(), 7);
            assert!(scale_pcs(&x.palette.scale)
                .iter()
                .all(|&p| c.contains_pc(p)));
        }
        // Guide tones are real: Dm7 -> F, C; G7 -> B, F; Cmaj7 -> E, B.
        assert_eq!(ctx[0].palette.guide_tones, vec![5, 0]);
        assert_eq!(ctx[1].palette.guide_tones, vec![11, 5]);
        assert_eq!(ctx[2].palette.guide_tones, vec![4, 11]);
        // Over Cmaj7 the F (a semitone above E) is expensive; D and A are licensed tensions.
        assert!(ctx[2].palette.expensive.contains(&5));
        assert!(ctx[2].palette.tensions.contains(&2));
        assert!(ctx[2].palette.tensions.contains(&9));
    }
}
