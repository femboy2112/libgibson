//! The harmonic **backbone**: the song's Lift → Deflect → Open → Reset spine.
//!
//! This axis is deliberately ORTHOGONAL to the rhetorical [`super::discourse::DiscourseRole`]
//! layer. A phrase can be `DiscourseRole::Answer` (why it exists in the argument) and
//! `HarmonicGesture::Open` (what the harmony does under it) at the same time.
//!
//! Round VII splits the backbone in two, along the world boundary:
//!
//! - [`BackboneTimeline`] is **world-independent** and lives in the canonical
//!   [`super::plan::CompositionPlan`]. It says, for every bar, which gesture is active, in which
//!   cycle, with which variation, and which semantic event it is bound to — so arrangement, the
//!   performance planner and every instrument know the spine before a single chord is chosen.
//! - [`realize`] translates that abstract gesture path into world-specific harmony.
//!
//! Round IX moves the chord journey itself upstream: the [`ChartCell`] (lift, pointer, expected,
//! deflect, open, reset and the satellites as relational [`ChartRoot`]s) is searched ONCE, in the
//! song's reference frame, and lives in the [`super::song::SongMap`]. [`realize`] re-modes and
//! colours it for a room; it no longer searches the room's own mode for a cell.
//!
//! **The two clocks are one clock now.** Round VI tiled the four gestures one per bar (`bar % 4`)
//! while the flagship story placed its semantic lift/deflect/open/reset roughly four bars apart, so
//! one semantic "lift" contained a whole harmonic Lift→Deflect→Open→Reset cycle — and the story's
//! second release (a `Confirmation`) landed on a harmonic *deflect*. The relation between the
//! semantic, gesture, phrase and bar timescales is now declared ([`TimeScales`], [`ClockBinding`]):
//! each gesture-bearing semantic event opens a gesture slot, snapped to the phrase grid, so the
//! semantic reach IS the harmonic Lift. A home-establishing opening slot states the whole cell in
//! miniature (the *thesis*), so the ear learns the identity before the story expands it; later
//! cycles are marked expanded or compressed relative to the first.
//!
//! **Deflect actually deflects.** A miss needs an expectation. Every Lift ends on a *pointer* — a
//! chord with concrete dominant pull ([`super::context::PullEvidence`]) toward home — so the ear
//! predicts the arrival; the Deflect then lands somewhere else that keeps common tones with the
//! expected arrival (an evasion, not a non-sequitur). Each miss is recorded as a
//! [`DeflectWitness`]: expected vs actual, common tones, voice-leading distance, and how long the
//! path takes to rejoin home. The Open is chosen as a *consequence* of that miss (connected to the
//! deflected chord), and the Reset restores home so another attempt is possible. No real
//! progression is transcribed; only this relational geometry is fixed.

use super::context::{
    common_tones, contextual_function, expected_chord, expected_target, pc_motion, PullEvidence,
};
use super::form::BEATS_PER_BAR;
use super::harmony::{diatonic_chord, ChordSpan};
use super::intent::IntentMorphism;
use super::language::MusicalLanguage;
use super::rng::Rng;
use super::semantic::EventKind;
use super::theory::{Chord, Quality, Scale};
use super::timeline::IntentTimeline;
use super::world::MusicWorld;

/// An abstract harmonic gesture — WHAT the harmony does, independent of WHY (the discourse role).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarmonicGesture {
    /// Reach up and away from home, ending on a pointer that makes the ear expect an arrival.
    Lift,
    /// The miss: the expected arrival is withheld; the harmony lands on a related substitute.
    Deflect,
    /// The warm window the miss opens: a coloured, released harmony connected to the deflection.
    Open,
    /// Return home, rounded — ready to reach again.
    Reset,
}

impl HarmonicGesture {
    /// The cell, in canonical order. This relational order is the DeflectedLift identity.
    pub const CELL: [HarmonicGesture; 4] = [
        HarmonicGesture::Lift,
        HarmonicGesture::Deflect,
        HarmonicGesture::Open,
        HarmonicGesture::Reset,
    ];

    /// A short lowercase label for the Score dump and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            HarmonicGesture::Lift => "lift",
            HarmonicGesture::Deflect => "deflect",
            HarmonicGesture::Open => "open",
            HarmonicGesture::Reset => "reset",
        }
    }
}

/// The gesture a semantic event's intent morphisms bind to, by their *effect* (not by event name):
/// a release opens, a relaxation or cadence resets, a suspension or modulation is the miss, an
/// intensification reaches, and a bare preparation establishes home. Pure recolourings
/// (`Reharmonize`, motif work, syncopation) do not open a new gesture — they are actions *inside*
/// the current one.
pub fn gesture_for_morphisms(applied: &[IntentMorphism]) -> Option<HarmonicGesture> {
    use IntentMorphism::*;
    let has = |m: IntentMorphism| applied.contains(&m);
    if has(Resolve) {
        Some(HarmonicGesture::Open)
    } else if has(Relax) || has(Cadence) {
        Some(HarmonicGesture::Reset)
    } else if has(Suspend) || has(Modulate) {
        Some(HarmonicGesture::Deflect)
    } else if has(Intensify) {
        Some(HarmonicGesture::Lift)
    } else if has(Prepare) {
        Some(HarmonicGesture::Reset)
    } else {
        None
    }
}

/// How gesture slots relate to the semantic trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockBinding {
    /// Each gesture-bearing semantic event opens a gesture slot (snapped to the phrase grid): the
    /// semantic phase and the backbone gesture are the same span.
    SemanticPhase,
    /// The trace carries too few gesture-bearing events to bind; the cell is tiled at a declared
    /// fixed rate instead. Stated, not implied.
    FixedTiling { bars_per_gesture: u32 },
}

/// The declared relationship between the piece's timescales.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeScales {
    /// Beats per bar (the groove timescale).
    pub beats_per_bar: f64,
    /// The contract's phrase grid in bars (the phrase timescale).
    pub phrase_bars: u32,
    /// How gesture slots are bound to semantic time.
    pub binding: ClockBinding,
}

/// How a cycle of the cell relates to the first full statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleVariation {
    /// The cell in miniature, inside the home-establishing opening (one short slot per gesture).
    Thesis,
    /// The first full cycle.
    Statement,
    /// Longer than the statement.
    Expanded,
    /// Shorter than the statement — the same identity, more urgent.
    Compressed,
    /// About as long as the statement, recoloured.
    Transformed,
}

impl CycleVariation {
    /// A short lowercase label for dumps.
    pub fn label(self) -> &'static str {
        match self {
            CycleVariation::Thesis => "thesis",
            CycleVariation::Statement => "statement",
            CycleVariation::Expanded => "expanded",
            CycleVariation::Compressed => "compressed",
            CycleVariation::Transformed => "transformed",
        }
    }
}

/// A semantic event bound to a gesture.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticBinding {
    /// Index of the [`super::timeline::IntentTransition`] that fired it.
    pub transition: usize,
    pub at_beat: f64,
    /// The bar its gesture slot starts on (snapped to the phrase grid).
    pub bar: u32,
    pub event: EventKind,
    pub gesture: HarmonicGesture,
    pub morphisms: Vec<IntentMorphism>,
}

/// One gesture slot of the world-independent timeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GestureSlot {
    pub gesture: HarmonicGesture,
    /// Which pass through the cell (0 = the thesis when present).
    pub cycle: u32,
    pub start_bar: u32,
    pub bars: u32,
    /// The index into [`BackboneTimeline::bindings`] this slot realizes, if bound.
    pub binding: Option<usize>,
    pub variation: CycleVariation,
}

impl GestureSlot {
    /// One past the last bar.
    pub fn end_bar(&self) -> u32 {
        self.start_bar + self.bars
    }
    /// First beat.
    pub fn start_beat(&self) -> f64 {
        self.start_bar as f64 * BEATS_PER_BAR
    }
    /// One past the last beat.
    pub fn end_beat(&self) -> f64 {
        self.end_bar() as f64 * BEATS_PER_BAR
    }
}

/// The world-independent backbone: every bar's gesture, cycle, variation and semantic binding.
#[derive(Debug, Clone, PartialEq)]
pub struct BackboneTimeline {
    pub scales: TimeScales,
    /// Contiguous slots tiling `[0, total_bars)`.
    pub slots: Vec<GestureSlot>,
    pub bindings: Vec<SemanticBinding>,
    pub total_bars: u32,
}

/// Snap `bar_f` to the nearest phrase boundary within one bar, else to the nearest bar.
fn snap_bar(bar_f: f64, phrase_starts: &[u32], total_bars: u32) -> u32 {
    let near = phrase_starts
        .iter()
        .copied()
        .filter(|&b| (b as f64 - bar_f).abs() <= 1.0 + 1e-9)
        .min_by(|a, b| {
            (*a as f64 - bar_f)
                .abs()
                .total_cmp(&(*b as f64 - bar_f).abs())
        });
    near.unwrap_or(bar_f.round() as u32)
        .min(total_bars.saturating_sub(1))
}

impl BackboneTimeline {
    /// PropulsiveReturn: home → repeat → neighbor → home, two bars per phase.
    /// Open/Reset are existing performance gestures, used literally. No Lift or Deflect is
    /// smuggled in under another label. The final partial cell closes at home.
    pub fn propulsive_return(total_bars: u32, phrase_bars: u32) -> Self {
        let total_bars = total_bars.max(1);
        let mut slots = Vec::new();
        for bar in (0..total_bars).step_by(2) {
            let phase = (bar / 2) % 4;
            let neighbor = phase == 2 && bar + 2 < total_bars;
            slots.push(GestureSlot {
                gesture: if neighbor {
                    HarmonicGesture::Open
                } else {
                    HarmonicGesture::Reset
                },
                cycle: bar / 8,
                start_bar: bar,
                bars: 2.min(total_bars - bar),
                binding: None,
                variation: if bar < 8 {
                    CycleVariation::Statement
                } else {
                    CycleVariation::Transformed
                },
            });
        }
        Self {
            scales: TimeScales {
                beats_per_bar: BEATS_PER_BAR,
                phrase_bars,
                binding: ClockBinding::FixedTiling {
                    bars_per_gesture: 2,
                },
            },
            slots,
            bindings: Vec::new(),
            total_bars,
        }
    }
    /// Build the timeline from the intent timeline and the plan's phrase grid.
    ///
    /// `phrase_starts` are the form's phrase start bars (for snapping); `recurrence_bars` is only
    /// used by the declared fixed-tiling fallback.
    pub fn build(
        timeline: &IntentTimeline,
        total_bars: u32,
        phrase_starts: &[u32],
        phrase_bars: u32,
        recurrence_bars: u32,
    ) -> BackboneTimeline {
        let total_bars = total_bars.max(1);
        let mut bindings: Vec<SemanticBinding> = Vec::new();
        for (i, t) in timeline.transitions.iter().enumerate() {
            let Some(g) = gesture_for_morphisms(&t.applied) else {
                continue;
            };
            let bar = snap_bar(t.at_beat / BEATS_PER_BAR, phrase_starts, total_bars);
            bindings.push(SemanticBinding {
                transition: i,
                at_beat: t.at_beat,
                bar,
                event: t.event_kind,
                gesture: g,
                morphisms: t.applied.clone(),
            });
        }
        let distinct = {
            let mut g: Vec<HarmonicGesture> = Vec::new();
            for b in &bindings {
                if !g.contains(&b.gesture) {
                    g.push(b.gesture);
                }
            }
            g.len()
        };
        if distinct < 3 {
            return Self::fixed_tiling(total_bars, phrase_bars, recurrence_bars, bindings);
        }

        // Raw segments: (gesture, start_bar, binding). A later event at the same bar wins; a repeat
        // of the running gesture extends it rather than restarting it.
        let mut segs: Vec<(HarmonicGesture, u32, Option<usize>)> = Vec::new();
        for (bi, b) in bindings.iter().enumerate() {
            if let Some(last) = segs.last_mut() {
                if last.1 == b.bar {
                    *last = (b.gesture, b.bar, Some(bi));
                    continue;
                }
                if last.0 == b.gesture {
                    continue;
                }
            }
            segs.push((b.gesture, b.bar, Some(bi)));
        }
        // Collapse any repeat created by same-bar replacement.
        segs.dedup_by(|b, a| a.0 == b.0);
        if segs.first().is_none_or(|s| s.1 > 0) {
            // Nothing bound at the downbeat: the opening is home, unbound.
            segs.insert(0, (HarmonicGesture::Reset, 0, None));
        }

        let mut slots: Vec<GestureSlot> = Vec::new();
        let mut cycle = 0u32;
        for (k, &(g, start, bi)) in segs.iter().enumerate() {
            let end = segs.get(k + 1).map(|s| s.1).unwrap_or(total_bars);
            if end <= start {
                continue;
            }
            let bars = end - start;
            // A home-establishing opening at least a cell long states the cell in miniature: the
            // thesis the rest of the piece develops.
            if k == 0 && g == HarmonicGesture::Reset && bars >= 4 {
                let each = bars / 4;
                let mut at = start;
                for (j, &cg) in HarmonicGesture::CELL.iter().enumerate() {
                    let len = if j == 3 { end - at } else { each };
                    slots.push(GestureSlot {
                        gesture: cg,
                        cycle: 0,
                        start_bar: at,
                        bars: len,
                        binding: bi,
                        variation: CycleVariation::Thesis,
                    });
                    at += len;
                }
                continue;
            }
            if g == HarmonicGesture::Lift && !slots.is_empty() {
                cycle += 1;
            }
            slots.push(GestureSlot {
                gesture: g,
                cycle,
                start_bar: start,
                bars,
                binding: bi,
                variation: CycleVariation::Statement,
            });
        }
        Self::mark_variations(&mut slots);
        BackboneTimeline {
            scales: TimeScales {
                beats_per_bar: BEATS_PER_BAR,
                phrase_bars,
                binding: ClockBinding::SemanticPhase,
            },
            slots,
            bindings,
            total_bars,
        }
    }

    /// The declared fallback: tile the cell at a fixed rate.
    fn fixed_tiling(
        total_bars: u32,
        phrase_bars: u32,
        recurrence_bars: u32,
        bindings: Vec<SemanticBinding>,
    ) -> BackboneTimeline {
        let bpg = (recurrence_bars / 4).max(1);
        let mut slots = Vec::new();
        let mut bar = 0;
        let mut i = 0usize;
        while bar < total_bars {
            let bars = bpg.min(total_bars - bar);
            slots.push(GestureSlot {
                gesture: HarmonicGesture::CELL[i % 4],
                cycle: (i / 4) as u32,
                start_bar: bar,
                bars,
                binding: None,
                variation: if i < 4 {
                    CycleVariation::Statement
                } else {
                    CycleVariation::Transformed
                },
            });
            bar += bars;
            i += 1;
        }
        BackboneTimeline {
            scales: TimeScales {
                beats_per_bar: BEATS_PER_BAR,
                phrase_bars,
                binding: ClockBinding::FixedTiling {
                    bars_per_gesture: bpg,
                },
            },
            slots,
            bindings,
            total_bars,
        }
    }

    /// Mark each post-thesis cycle relative to the first full statement's length.
    fn mark_variations(slots: &mut [GestureSlot]) {
        let cycle_len = |slots: &[GestureSlot], c: u32| -> u32 {
            slots
                .iter()
                .filter(|s| s.cycle == c && s.variation != CycleVariation::Thesis)
                .map(|s| s.bars)
                .sum()
        };
        let first = slots
            .iter()
            .find(|s| s.variation != CycleVariation::Thesis)
            .map(|s| s.cycle);
        let Some(first) = first else {
            return;
        };
        let base = cycle_len(slots, first).max(1) as f32;
        let max_cycle = slots.iter().map(|s| s.cycle).max().unwrap_or(0);
        for c in (first + 1)..=max_cycle {
            let len = cycle_len(slots, c) as f32;
            let v = if len < base * 0.85 {
                CycleVariation::Compressed
            } else if len > base * 1.15 {
                CycleVariation::Expanded
            } else {
                CycleVariation::Transformed
            };
            for s in slots.iter_mut().filter(|s| s.cycle == c) {
                if s.variation != CycleVariation::Thesis {
                    s.variation = v;
                }
            }
        }
    }

    /// The slot holding `bar`.
    pub fn slot_at_bar(&self, bar: u32) -> Option<&GestureSlot> {
        self.slots
            .iter()
            .find(|s| bar >= s.start_bar && bar < s.end_bar())
    }

    /// The slot sounding at `beat`.
    pub fn slot_at_beat(&self, beat: f64) -> Option<&GestureSlot> {
        self.slot_at_bar((beat / BEATS_PER_BAR).floor().max(0.0) as u32)
    }

    /// The index of the slot sounding at `beat`.
    pub fn slot_index_at_beat(&self, beat: f64) -> Option<usize> {
        let bar = (beat / BEATS_PER_BAR).floor().max(0.0) as u32;
        self.slots
            .iter()
            .position(|s| bar >= s.start_bar && bar < s.end_bar())
    }

    /// The gesture active at `beat`.
    pub fn gesture_at_beat(&self, beat: f64) -> Option<HarmonicGesture> {
        self.slot_at_beat(beat).map(|s| s.gesture)
    }

    /// How many passes through the cell the timeline makes (including the thesis).
    pub fn cycles(&self) -> u32 {
        self.slots
            .iter()
            .map(|s| s.cycle)
            .max()
            .map_or(0, |c| c + 1)
    }

    /// A compact dump: one line per slot.
    pub fn dump(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let binding = match self.scales.binding {
            ClockBinding::SemanticPhase => "semantic-phase".to_string(),
            ClockBinding::FixedTiling { bars_per_gesture } => {
                format!("fixed-tiling({bars_per_gesture} bar/gesture)")
            }
        };
        let _ = writeln!(
            s,
            "backbone timeline: binding={binding} phrase={}bar cycles={}",
            self.scales.phrase_bars,
            self.cycles()
        );
        for sl in &self.slots {
            let bound = sl
                .binding
                .and_then(|b| self.bindings.get(b))
                .map(|b| format!("{:?}@{:.0}", b.event, b.at_beat))
                .unwrap_or_else(|| "-".into());
            let _ = writeln!(
                s,
                "  bars {:>2}..{:<2} {:<8} cycle {} {:<11} <- {}",
                sl.start_bar,
                sl.end_bar(),
                sl.gesture.label(),
                sl.cycle,
                sl.variation.label(),
                bound
            );
        }
        s
    }
}

/// The world-specific anchor chords of the cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicCell {
    /// The Lift's reach (a departure that prepares the pointer).
    pub lift: Chord,
    /// A second reach for longer Lift slots (a sequence step).
    pub lift_alt: Chord,
    /// The chord that ends every Lift with concrete pull toward home — the expectation.
    pub pointer: Chord,
    /// What the pointer makes the ear expect.
    pub expected: Chord,
    /// The miss: the related substitute actually heard.
    pub deflect: Chord,
    /// The window the miss opens.
    pub open: Chord,
    /// Home, rounded.
    pub reset: Chord,
    /// Common-tone satellites of the Deflect, Open and Reset anchors: a longer slot alternates its
    /// anchor with its satellite, so the gesture MOVES while keeping its function (a prolongation),
    /// instead of sitting on one chord for four bars.
    pub satellites: [Chord; 3],
}

impl HarmonicCell {
    /// The four gesture anchor roots — the cell's audible signature.
    pub fn signature(&self) -> [i32; 4] {
        [
            self.lift.root_pc,
            self.deflect.root_pc,
            self.open.root_pc,
            self.reset.root_pc,
        ]
    }
}

/// The measured miss at one Deflect slot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeflectWitness {
    pub at_beat: f64,
    /// The chord sounding just before the deflection (the pointer, when the Lift set one up).
    pub pointer: Chord,
    /// Whether that chord concretely pulled toward the expected arrival.
    pub prepared: bool,
    /// The arrival the pointer made the ear expect.
    pub expected: Chord,
    /// What was actually heard.
    pub actual: Chord,
    /// Common tones between expected and actual (an evasion keeps some).
    pub common_tones: u8,
    /// Root distance expected→actual in semitones (`0..=6`).
    pub root_distance: i32,
    /// Voice-leading distance pointer→actual (pitch-class motion).
    pub voice_motion: i32,
    /// Beats from the miss until the path is home again (the next Reset slot).
    pub rejoin_beats: f64,
}

/// A realized backbone: the cell, the chord spans, and the witnessed misses.
#[derive(Debug, Clone)]
pub struct BackboneRealization {
    pub cell: HarmonicCell,
    pub spans: Vec<ChordSpan>,
    pub deflects: Vec<DeflectWitness>,
}

fn has_major_third(c: &Chord) -> bool {
    c.quality.intervals().contains(&4)
}

/// Recolour `base` for `gesture` at `depth` (0 plain, 1 warm, 2 deep); `alt` picks the second
/// colour of a pair so a repeated chord inside a slot still moves.
fn colour(base: Chord, gesture: HarmonicGesture, depth: u8, alt: bool, s7: bool) -> Chord {
    let major = has_major_third(&base);
    let minor = base.quality.intervals().contains(&3) && !major;
    let dim = base.quality.intervals().contains(&6);
    let q = match gesture {
        HarmonicGesture::Lift | HarmonicGesture::Deflect => {
            if dim {
                Quality::Min7b5
            } else if depth == 0 && !s7 {
                base.quality
            } else if depth >= 2 && alt {
                if major {
                    Quality::Maj9
                } else {
                    Quality::Min9
                }
            } else if major {
                Quality::Maj7
            } else if minor {
                Quality::Min7
            } else {
                base.quality
            }
        }
        HarmonicGesture::Open => match (depth, alt, major) {
            (0, _, true) => Quality::Maj,
            (0, _, false) => Quality::Min,
            (1, _, true) | (_, false, true) => Quality::Add9,
            (1, _, false) | (_, false, false) => Quality::Min9,
            (_, true, true) => Quality::Maj9,
            (_, true, false) => Quality::Min7,
        },
        HarmonicGesture::Reset => match (depth, alt, major) {
            (0, _, true) => Quality::Maj,
            (0, _, false) => Quality::Min,
            (1, _, true) | (_, false, true) => Quality::Maj6,
            (1, _, false) | (_, false, false) => Quality::Min6,
            (_, true, true) => Quality::Maj7,
            (_, true, false) => Quality::Min9,
        },
    };
    if dim {
        Chord::new(
            base.root_pc,
            if s7 { Quality::Min7b5 } else { Quality::Dim },
        )
    } else {
        Chord::new(base.root_pc, q)
    }
}

/// Search the cell in `region`: bounded, deterministic, the RNG only breaking exact ties. Run ONCE,
/// by the song, in its reference frame ([`ChartCell::chart`]) — never by a room (Round IX: a room
/// that searched its own mode discovered its own journey).
fn search_cell(region: &Scale, seed: u64) -> HarmonicCell {
    let region = *region;
    let tonic = region.tonic_pc;
    let mut rng = Rng::new(seed ^ 0xBACC_B0E1);
    let triad = |d: i32| diatonic_chord(&region, d, false);

    // The pointer: the cadential dominant with CONCRETE pull toward home. In a mode whose diatonic
    // degree-4 chord has no leading tone, borrow the raised third — the expectation must be real.
    // A dominant seventh on degree 4 is the diatonic V7 in Ionian and the borrowed harmonic-minor
    // V7 elsewhere; either way it carries the leading tone and the resolving tritone.
    let pointer = Chord::new(triad(4).root_pc, Quality::Dom7);
    debug_assert!(PullEvidence::of(&pointer, tonic).is_dominant());
    let expected = expected_chord(expected_target(&pointer, &region).unwrap_or(tonic), &region);

    // Pick the minimum-cost candidate, collecting exact ties for the seeded tie-break.
    fn argmin(cands: &[(Chord, f32)], rng: &mut Rng) -> Chord {
        let best = cands.iter().map(|c| c.1).fold(f32::INFINITY, f32::min);
        let ties: Vec<Chord> = cands
            .iter()
            .filter(|c| (c.1 - best).abs() <= 1e-6)
            .map(|c| c.0)
            .collect();
        *rng.pick(&ties).unwrap_or(&cands[0].0)
    }

    // Lift: reach away from home toward the pointer — prefer the chord a fifth above the pointer
    // (a preparation, the ii of a ii–V) or a step below it; never home, never the pointer itself.
    let lift_cands: Vec<(Chord, f32)> = (1..7)
        .map(&triad)
        .filter(|c| c.root_pc != tonic && c.root_pc != pointer.root_pc)
        .map(|c| {
            let prepares = c.root_pc == (pointer.root_pc + 7).rem_euclid(12);
            let mut cost = 0.18 * pc_motion(&c, &pointer) as f32;
            if prepares {
                cost -= 0.8;
            }
            cost += 0.6 * PullEvidence::of(&c, tonic).strength();
            (c, cost)
        })
        .collect();
    let lift = argmin(&lift_cands, &mut rng);

    // Deflect: land somewhere other than the expected arrival, keeping common tones with it (an
    // evasion), approached smoothly from the pointer, ideally with the bass stepping up (the
    // deceptive geometry: the leading tone resolves while the root refuses home).
    let mut defl_cands: Vec<Chord> = (1..7).map(&triad).collect();
    let bvi = Chord::new((tonic + 8).rem_euclid(12), Quality::Maj);
    if !defl_cands.contains(&bvi) {
        defl_cands.push(bvi);
    }
    let defl_scored: Vec<(Chord, f32)> = defl_cands
        .into_iter()
        .filter(|c| c.root_pc != expected.root_pc && c.root_pc != pointer.root_pc)
        .map(|c| {
            let common = common_tones(&c, &expected) as f32;
            let mut cost = -common + 0.15 * pc_motion(&pointer, &c) as f32;
            if common == 0.0 {
                cost += 3.0; // a non-sequitur, not a miss
            }
            let step = (c.root_pc - pointer.root_pc).rem_euclid(12);
            if step == 1 || step == 2 {
                cost -= 0.6;
            }
            if c.root_pc == lift.root_pc {
                cost += 0.5;
            }
            (c, cost)
        })
        .collect();
    let deflect = argmin(&defl_scored, &mut rng);

    // Open: the window the miss opens — connected to the deflection, released (no pull home), a
    // plagal/relative step away from it, bright where possible.
    let open_scored: Vec<(Chord, f32)> = (1..7)
        .map(&triad)
        .filter(|c| {
            c.root_pc != tonic
                && c.root_pc != deflect.root_pc
                && c.root_pc != lift.root_pc
                && c.root_pc != pointer.root_pc
        })
        .map(|c| {
            let mut cost = -0.6 * common_tones(&c, &deflect) as f32
                + 0.8 * PullEvidence::of(&c, tonic).strength();
            let step = (c.root_pc - deflect.root_pc).rem_euclid(12);
            if step == 5 || step == 7 {
                cost -= 0.4;
            }
            if has_major_third(&c) {
                cost -= 0.3;
            }
            (c, cost)
        })
        .collect();
    let open = if open_scored.is_empty() {
        triad(3)
    } else {
        argmin(&open_scored, &mut rng)
    };

    // The second reach must not pre-empt a later gesture: never the deflection or the opening.
    let lift_alt = {
        let rest: Vec<(Chord, f32)> = lift_cands
            .iter()
            .copied()
            .filter(|c| {
                c.0.root_pc != lift.root_pc
                    && c.0.root_pc != deflect.root_pc
                    && c.0.root_pc != open.root_pc
            })
            .collect();
        if rest.is_empty() {
            lift
        } else {
            argmin(&rest, &mut rng)
        }
    };
    let reset = triad(0);
    let anchors = [lift, lift_alt, pointer, deflect, open, reset];
    // A satellite shares two tones with its anchor and is not another gesture's anchor (so it
    // prolongs rather than pre-empting the next gesture); home's satellite is its plagal neighbour.
    let satellite = |anchor: Chord| -> Chord {
        // Two common tones and no real pull home; if no such neighbour is free, the satellite is the
        // anchor itself — the slot then moves by colour over a held root (a pedal), never by
        // borrowing a chord that points home inside a release.
        (1..7)
            .map(&triad)
            .filter(|c| {
                c.root_pc != anchor.root_pc
                    && !anchors.contains(c)
                    && c.root_pc != tonic
                    && common_tones(c, &anchor) >= 2
                    && PullEvidence::of(c, tonic).strength() < 0.35
            })
            .min_by_key(|c| pc_motion(&anchor, c))
            .unwrap_or(anchor)
    };
    let plagal = triad(3);
    HarmonicCell {
        lift,
        lift_alt,
        pointer,
        expected,
        deflect,
        open,
        reset,
        satellites: [satellite(deflect), satellite(open), plagal],
    }
}

/// One root of the song's chart, RELATIVE to whatever home it is realized in: a scale degree (the
/// room's mode gives it its quality — ii in major is ii° in minor, the same function) or a fixed
/// chromatic interval above the tonic with its own quality (a borrowed chord stays borrowed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartRoot {
    /// A 0-based scale degree of the home.
    Degree(i32),
    /// `semitones` above the tonic, sounding `quality` in every room.
    Chromatic { semitones: i32, quality: Quality },
}

impl ChartRoot {
    /// The chart root of `chord` in `region`: its degree when the chord is that degree's diatonic
    /// triad, otherwise its exact interval and quality above the tonic.
    fn of(chord: Chord, region: &Scale) -> ChartRoot {
        (0..7)
            .find(|&d| diatonic_chord(region, d, false) == chord)
            .map(ChartRoot::Degree)
            .unwrap_or(ChartRoot::Chromatic {
                semitones: (chord.root_pc - region.tonic_pc).rem_euclid(12),
                quality: chord.quality,
            })
    }

    /// The root pitch class this chart root names in `region`.
    pub fn root_pc(self, region: &Scale) -> i32 {
        match self {
            ChartRoot::Degree(d) => region.degree_pitch(d, 4).rem_euclid(12),
            ChartRoot::Chromatic { semitones, .. } => (region.tonic_pc + semitones).rem_euclid(12),
        }
    }

    /// The chord this chart root is in `region` (a degree takes the region's diatonic triad).
    pub fn chord(self, region: &Scale) -> Chord {
        match self {
            ChartRoot::Degree(d) => diatonic_chord(region, d, false),
            ChartRoot::Chromatic { quality, .. } => Chord::new(self.root_pc(region), quality),
        }
    }

    /// The scale degree whose root is pitch class `pc` in `region`, if `pc` is diatonic there.
    pub fn degree_of_pc(pc: i32, region: &Scale) -> Option<i32> {
        (0..7).find(|&d| region.degree_pitch(d, 4).rem_euclid(12) == pc.rem_euclid(12))
    }
}

/// The song's harmonic chart for the DeflectedLift journey — Lift, the pointer and what it makes
/// the ear expect, the Deflect that misses it, the Open the miss opens, the Reset home, and each
/// anchor's prolonging satellite — as RELATIONAL roots ([`ChartRoot`]). Searched once, in the
/// song's reference frame; every room realizes the same journey in its own mode
/// ([`ChartCell::realize`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChartCell {
    pub lift: ChartRoot,
    pub lift_alt: ChartRoot,
    /// Always realized as a dominant seventh on its root: the pointer must carry the leading tone.
    pub pointer: ChartRoot,
    /// What the pointer makes the ear expect (home).
    pub expected: ChartRoot,
    pub deflect: ChartRoot,
    pub open: ChartRoot,
    pub reset: ChartRoot,
    /// The Deflect, Open and Reset anchors' satellites.
    pub satellites: [ChartRoot; 3],
}

impl ChartCell {
    /// I–I–IV–I tonic prolongation (i–i–iv–i when re-moded to minor). The unused Lift and
    /// Deflect coordinates remain for the existing chart representation; the new timeline
    /// never schedules them. Satellites hold roots, leaving motion to the written theme/bass.
    pub fn propulsive_return() -> Self {
        let home = ChartRoot::Degree(0);
        let neighbor = ChartRoot::Degree(3);
        Self {
            lift: home,
            lift_alt: neighbor,
            pointer: ChartRoot::Degree(4),
            expected: home,
            deflect: home,
            open: neighbor,
            reset: home,
            satellites: [home, neighbor, home],
        }
    }
    /// Chart the cell: the search run once in `frame` (tonic C — the chart is transposition-free),
    /// every chord converted to its relative root.
    pub fn chart(frame: super::theory::Mode, seed: u64) -> ChartCell {
        let region = Scale::new(0, frame);
        let c = search_cell(&region, seed);
        let r = |ch: Chord| ChartRoot::of(ch, &region);
        ChartCell {
            lift: r(c.lift),
            lift_alt: r(c.lift_alt),
            pointer: ChartRoot::degree_of_pc(c.pointer.root_pc, &region)
                .map(ChartRoot::Degree)
                .unwrap_or(ChartRoot::Chromatic {
                    semitones: c.pointer.root_pc.rem_euclid(12),
                    quality: Quality::Dom7,
                }),
            expected: r(c.expected),
            deflect: r(c.deflect),
            open: r(c.open),
            reset: r(c.reset),
            satellites: c.satellites.map(r),
        }
    }

    /// The chart realized in `region`: every degree takes the region's diatonic triad, the pointer
    /// its dominant seventh, and the expectation is re-derived from the pointer IN the region (so
    /// the deflection is measured against what this room actually expects).
    pub fn realize(&self, region: &Scale) -> HarmonicCell {
        let pointer = Chord::new(self.pointer.root_pc(region), Quality::Dom7);
        let expected = expected_chord(
            expected_target(&pointer, region).unwrap_or(region.tonic_pc),
            region,
        );
        HarmonicCell {
            lift: self.lift.chord(region),
            lift_alt: self.lift_alt.chord(region),
            pointer,
            expected,
            deflect: self.deflect.chord(region),
            open: self.open.chord(region),
            reset: self.reset.chord(region),
            satellites: self.satellites.map(|s| s.chord(region)),
        }
    }
}

/// The chord path of one slot: `(beats offset, beats length, chord)`.
fn slot_path(
    slot: &GestureSlot,
    cell: &HarmonicCell,
    lang: &MusicalLanguage,
    chart_bars: u32,
    s7: bool,
) -> Vec<(f64, f64, Chord)> {
    let total = slot.bars as f64 * BEATS_PER_BAR;
    let hr = lang.harmonic_rhythm.bars_per_chord(chart_bars) as f64 * BEATS_PER_BAR;
    // Deeper colour on transformed/compressed returns; the thesis and statement keep the base.
    let depth = match slot.variation {
        CycleVariation::Thesis | CycleVariation::Statement => lang.color_depth.min(1),
        _ => lang.color_depth,
    };
    let n = ((total / hr).floor() as usize).max(1);
    let unit = total / n as f64;
    let mut out: Vec<(f64, f64, Chord)> = Vec::new();
    match slot.gesture {
        HarmonicGesture::Lift => {
            // Reach, (sequence), then the POINTER — always last, so the ear expects an arrival.
            let pointer = cell.pointer;
            let lift = colour(cell.lift, HarmonicGesture::Lift, depth, false, s7);
            if n == 1 {
                let half = total / 2.0;
                out.push((0.0, half, lift));
                out.push((half, total - half, pointer));
            } else {
                // In the full-colour language a statement-length reach climbs through the pointer's
                // own applied dominant (V/V -> V7): a tonicization on the way up (the region stays).
                let applied = Chord::new((pointer.root_pc + 7).rem_euclid(12), Quality::Dom7);
                for i in 0..n - 1 {
                    let c = if lang.color_depth >= 2 && n >= 4 && i + 2 == n {
                        applied
                    } else if i % 2 == 1 {
                        colour(cell.lift_alt, HarmonicGesture::Lift, depth, true, s7)
                    } else {
                        colour(cell.lift, HarmonicGesture::Lift, depth, i > 0, s7)
                    };
                    out.push((i as f64 * unit, unit, c));
                }
                out.push((
                    (n - 1) as f64 * unit,
                    total - (n - 1) as f64 * unit,
                    pointer,
                ));
            }
        }
        g => {
            let (base, sat) = match g {
                HarmonicGesture::Deflect => (cell.deflect, cell.satellites[0]),
                HarmonicGesture::Open => (cell.open, cell.satellites[1]),
                _ => (cell.reset, cell.satellites[2]),
            };
            for i in 0..n {
                // Anchor on the slot's downbeat and every other unit; the satellite between. A Reset
                // of three or more units closes on its anchor, so the cycle ends at home.
                let closes_home = g == HarmonicGesture::Reset && n >= 3 && i + 1 == n;
                let use_sat = i % 2 == 1 && !closes_home;
                let c = if use_sat && sat == base {
                    // No free neighbour: move by colour over the held root (a pedal).
                    colour(base, g, (depth + 1).min(2), true, s7)
                } else if use_sat {
                    colour(sat, g, depth, false, s7)
                } else {
                    colour(base, g, depth, i >= 2, s7)
                };
                let len = if i + 1 == n {
                    total - i as f64 * unit
                } else {
                    unit
                };
                out.push((i as f64 * unit, len, c));
            }
        }
    }
    out
}

/// Realize the song's `timeline` and `chart` as harmony in `world`'s room under `lang`. The room
/// re-modes the chart and colours it; the idiom applies its declared rhythm transform to the
/// chart's canonical rhythm; neither chooses a root or a change point of its own.
pub fn realize(
    timeline: &BackboneTimeline,
    chart: &super::song::HarmonicMap,
    world: &MusicWorld,
    lang: &MusicalLanguage,
) -> BackboneRealization {
    let region = Scale::new(world.tonic_pc, world.mode);
    let cell = chart.cell.realize(&region);
    let s7 = world.use_sevenths;
    let mut spans: Vec<ChordSpan> = Vec::new();
    for slot in &timeline.slots {
        for (off, len, chord) in slot_path(slot, &cell, lang, chart.bars_per_chord, s7) {
            let degree = (0..7)
                .find(|&d| region.degree_pitch(d, 4).rem_euclid(12) == chord.root_pc)
                .unwrap_or(-1);
            spans.push(ChordSpan {
                start_beat: slot.start_beat() + off,
                dur_beats: len as f32,
                chord,
                function: contextual_function(&chord, &region),
                degree,
                note: slot.gesture.label(),
            });
        }
    }

    let deflects = deflect_witnesses(timeline, &spans, &|_| region, cell.reset);
    BackboneRealization {
        cell,
        spans,
        deflects,
    }
}

/// The song's **lead sheet**: the chart as charted — every slot's chord path in the song's own
/// reference `frame` (tonic C), at the chart's canonical rhythm, with no room's mode, no idiom's
/// colour and no rhythm transform. What the listener model reads (Round X,
/// [`super::meaning::MeaningPlan::observe`]); no performance plays it verbatim.
pub fn lead_sheet(
    timeline: &BackboneTimeline,
    chart: &super::song::HarmonicMap,
    frame: super::theory::Mode,
) -> Vec<ChordSpan> {
    let region = Scale::new(0, frame);
    let cell = chart.cell.realize(&region);
    let plain = MusicalLanguage::simple();
    let mut spans = Vec::new();
    for slot in &timeline.slots {
        for (off, len, chord) in slot_path(slot, &cell, &plain, chart.bars_per_chord, false) {
            spans.push(ChordSpan {
                start_beat: slot.start_beat() + off,
                dur_beats: len as f32,
                chord,
                function: contextual_function(&chord, &region),
                degree: ChartRoot::degree_of_pc(chord.root_pc, &region).unwrap_or(-1),
                note: slot.gesture.label(),
            });
        }
    }
    spans
}

/// Measure the miss at every Deflect slot of `timeline` over the harmony `spans`, each in the
/// region `region_at` its slot start (`fallback_pointer` when the slot opens the piece). Called on
/// the backbone's own spans by [`realize`], and again by the performance AFTER the harmonic
/// actions edited them (Round VIIb), so a tonicized or modulated pointer is measured as it sounds.
pub(super) fn deflect_witnesses(
    timeline: &BackboneTimeline,
    spans: &[ChordSpan],
    region_at: &dyn Fn(f64) -> Scale,
    fallback_pointer: Chord,
) -> Vec<DeflectWitness> {
    let mut deflects = Vec::new();
    for (si, slot) in timeline.slots.iter().enumerate() {
        if slot.gesture != HarmonicGesture::Deflect {
            continue;
        }
        let at = slot.start_beat();
        let region = region_at(at);
        let Some(ix) = spans.iter().position(|s| (s.start_beat - at).abs() < 1e-6) else {
            continue;
        };
        let actual = spans[ix].chord;
        let pointer = ix
            .checked_sub(1)
            .map(|j| spans[j].chord)
            .unwrap_or(fallback_pointer);
        let target = expected_target(&pointer, &region);
        let expected = expected_chord(target.unwrap_or(region.tonic_pc), &region);
        let raw = (actual.root_pc - expected.root_pc).rem_euclid(12);
        let rejoin = timeline.slots[si..]
            .iter()
            .find(|s| s.gesture == HarmonicGesture::Reset)
            .map(|s| s.start_beat() - at)
            .unwrap_or(timeline.total_bars as f64 * BEATS_PER_BAR - at);
        deflects.push(DeflectWitness {
            at_beat: at,
            pointer,
            prepared: target.is_some(),
            expected,
            actual,
            common_tones: common_tones(&actual, &expected),
            root_distance: raw.min(12 - raw),
            voice_motion: pc_motion(&pointer, &actual),
            rejoin_beats: rejoin,
        });
    }
    deflects
}

#[cfg(test)]
mod tests {
    use super::super::semantic::{calm_loop, deflected_lift_trace};
    use super::*;

    fn flagship() -> BackboneTimeline {
        let trace = deflected_lift_trace(120.0);
        let tl = IntentTimeline::walk(&trace);
        // The flagship form's phrase starts (verified against the plan in the plan tests).
        BackboneTimeline::build(&tl, 30, &[0, 4, 8, 12, 16, 20, 22, 24, 28], 4, 4)
    }

    /// A chart (charted in the reference frame at `seed`, the canonical rhythm).
    fn chart_at(seed: u64) -> super::super::song::HarmonicMap {
        super::super::song::HarmonicMap {
            cell: ChartCell::chart(super::super::song::REFERENCE_FRAME, seed),
            bars_per_chord: super::super::song::CHART_BARS_PER_CHORD,
        }
    }

    /// The flagship chart (seed 2112).
    fn chart() -> super::super::song::HarmonicMap {
        chart_at(2112)
    }

    #[test]
    fn morphism_effects_bind_to_gestures() {
        use IntentMorphism::*;
        assert_eq!(
            gesture_for_morphisms(&[Intensify, FragmentMotif]),
            Some(HarmonicGesture::Lift)
        );
        assert_eq!(
            gesture_for_morphisms(&[Suspend, ThickenTexture]),
            Some(HarmonicGesture::Deflect)
        );
        assert_eq!(
            gesture_for_morphisms(&[Resolve, Cadence]),
            Some(HarmonicGesture::Open)
        );
        assert_eq!(
            gesture_for_morphisms(&[Relax, Cadence]),
            Some(HarmonicGesture::Reset)
        );
        assert_eq!(gesture_for_morphisms(&[Reharmonize]), None);
    }

    #[test]
    fn the_two_clocks_are_one_clock() {
        // Every semantic phase IS its backbone gesture: the gesture active at each bound event's
        // (snapped) bar is exactly the gesture that event binds to. Round VI failed this — its
        // bar-25 Confirmation (a release) sounded over a harmonic deflect.
        let tl = flagship();
        assert_eq!(tl.scales.binding, ClockBinding::SemanticPhase);
        for b in &tl.bindings {
            if b.bar == 0 {
                continue; // the opening binds the thesis (home, stated as the cell in miniature)
            }
            let g = tl.slot_at_bar(b.bar).unwrap().gesture;
            assert_eq!(
                g, b.gesture,
                "{:?} at bar {} bound to {:?} but the spine says {:?}",
                b.event, b.bar, b.gesture, g
            );
        }
        let release = tl
            .bindings
            .iter()
            .rfind(|b| b.event == EventKind::Confirmation)
            .unwrap();
        assert_eq!(
            tl.slot_at_bar(release.bar).unwrap().gesture,
            HarmonicGesture::Open
        );
    }

    #[test]
    fn slots_tile_the_piece_and_the_thesis_states_the_cell() {
        let tl = flagship();
        let mut bar = 0;
        for s in &tl.slots {
            assert_eq!(s.start_bar, bar, "gap/overlap at bar {bar}");
            assert!(s.bars >= 1);
            bar = s.end_bar();
        }
        assert_eq!(bar, 30);
        let thesis: Vec<HarmonicGesture> = tl
            .slots
            .iter()
            .filter(|s| s.variation == CycleVariation::Thesis)
            .map(|s| s.gesture)
            .collect();
        assert_eq!(thesis, HarmonicGesture::CELL.to_vec());
        // Two story cycles after the thesis; the second is compressed (more urgent).
        assert_eq!(tl.cycles(), 3);
        assert!(tl
            .slots
            .iter()
            .any(|s| s.cycle == 2 && s.variation == CycleVariation::Compressed));
    }

    #[test]
    fn a_trace_without_gesture_events_declares_a_fixed_tiling() {
        let tl = IntentTimeline::walk(&calm_loop(96.0));
        let bt = BackboneTimeline::build(&tl, 24, &[0, 4, 8, 12, 16, 20], 4, 4);
        if let ClockBinding::FixedTiling { bars_per_gesture } = bt.scales.binding {
            assert_eq!(bars_per_gesture, 1);
        }
        // Either way the slots tile the piece.
        assert_eq!(bt.slots.iter().map(|s| s.bars).sum::<u32>(), 24);
    }

    #[test]
    fn every_deflect_is_a_prepared_miss() {
        // The pointer concretely pulls toward the expected arrival; the actual arrival is not it but
        // keeps common tones with it — in every world, at every Deflect slot.
        let tl = flagship();
        for world in MusicWorld::all() {
            let r = realize(&tl, &chart(), &world, &MusicalLanguage::default());
            assert!(r.deflects.len() >= 3, "{}: too few misses", world.name);
            for w in &r.deflects {
                assert!(
                    w.prepared,
                    "{}: unprepared miss at {}",
                    world.name, w.at_beat
                );
                assert_ne!(w.actual.root_pc, w.expected.root_pc, "{}", world.name);
                assert!(w.common_tones >= 1, "{}: a non-sequitur", world.name);
                assert!(w.rejoin_beats > 0.0);
            }
        }
    }

    #[test]
    fn the_open_follows_from_the_miss_and_reset_is_home() {
        for world in MusicWorld::all() {
            let c = chart()
                .cell
                .realize(&Scale::new(world.tonic_pc, world.mode));
            assert!(
                common_tones(&c.open, &c.deflect) >= 1,
                "{}: open unrelated to the deflection",
                world.name
            );
            assert_eq!(c.reset.root_pc, world.tonic_pc.rem_euclid(12));
            let distinct: std::collections::BTreeSet<i32> = c.signature().into_iter().collect();
            assert_eq!(distinct.len(), 4, "{}: {:?}", world.name, c.signature());
        }
    }

    #[test]
    fn realization_is_deterministic_and_world_specific() {
        let tl = flagship();
        let lang = MusicalLanguage::default();
        let chart = chart_at(7);
        let a = realize(&tl, &chart, &MusicWorld::black_ice(), &lang);
        let b = realize(&tl, &chart, &MusicWorld::black_ice(), &lang);
        assert_eq!(a.cell, b.cell);
        assert_eq!(a.spans.len(), b.spans.len());
        // The same chart sounds in another home (absolute roots move with the room)...
        let v = realize(&tl, &chart, &MusicWorld::vapor95(), &lang);
        assert_ne!(a.cell.signature(), v.cell.signature());
        // The spans cover the piece and carry the gesture of their slot.
        let end = a
            .spans
            .last()
            .map(|s| s.start_beat + s.dur_beats as f64)
            .unwrap();
        assert!((end - 120.0).abs() < 1e-6);
        for s in &a.spans {
            assert_eq!(s.note, tl.gesture_at_beat(s.start_beat).unwrap().label());
        }
    }

    #[test]
    fn the_simple_language_moves_slower_but_is_the_same_spine() {
        let tl = flagship();
        let chart = chart_at(1);
        let f = realize(
            &tl,
            &chart,
            &MusicWorld::black_ice(),
            &MusicalLanguage::fusion_conversation(),
        );
        let s = realize(
            &tl,
            &chart,
            &MusicWorld::black_ice(),
            &MusicalLanguage::simple(),
        );
        assert!(s.spans.len() < f.spans.len());
        assert_eq!(s.cell.signature(), f.cell.signature());
        assert_eq!(s.deflects.len(), f.deflects.len());
    }

    fn permutations_of_four() -> Vec<[usize; 4]> {
        let mut out = Vec::new();
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    for d in 0..4 {
                        let p = [a, b, c, d];
                        let mut q = p;
                        q.sort_unstable();
                        if q == [0, 1, 2, 3] {
                            out.push(p);
                        }
                    }
                }
            }
        }
        out
    }

    #[test]
    fn the_spine_order_beats_a_scrambled_gesture_assignment() {
        // Broken-DeflectedLift adversarial: relabel the gestures of every slot by a permutation and
        // realize. The composed order makes every Deflect a prepared miss; a scramble mostly does
        // not (the pointer no longer precedes the miss). The order is load-bearing.
        let tl = flagship();
        let world = MusicWorld::black_ice();
        let lang = MusicalLanguage::default();
        let prepared_misses = |t: &BackboneTimeline| {
            realize(t, &chart(), &world, &lang)
                .deflects
                .iter()
                .filter(|w| w.prepared && w.actual.root_pc != w.expected.root_pc)
                .count()
        };
        let identity = prepared_misses(&tl);
        let perms: Vec<[usize; 4]> = permutations_of_four()
            .into_iter()
            .filter(|p| *p != [0, 1, 2, 3])
            .collect();
        let mut total = 0usize;
        for p in &perms {
            let mut t = tl.clone();
            for s in &mut t.slots {
                let i = HarmonicGesture::CELL
                    .iter()
                    .position(|g| *g == s.gesture)
                    .unwrap();
                s.gesture = HarmonicGesture::CELL[p[i]];
            }
            total += prepared_misses(&t);
        }
        let mean = total as f32 / perms.len() as f32;
        assert!(
            identity as f32 > mean,
            "composed spine ({identity} prepared misses) no better than scrambles ({mean})"
        );
    }
}
