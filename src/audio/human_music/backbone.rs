//! The harmonic **backbone**: an abstract Lift → Deflect → Open → Reset gesture cell that recurs
//! as the song's spine.
//!
//! This axis is deliberately ORTHOGONAL to the rhetorical [`super::discourse::DiscourseRole`]
//! layer. A phrase can be `DiscourseRole::Answer` (why it exists in the argument) and
//! `HarmonicGesture::Open` (what the harmony does under it) at the same time. Round V conflated the
//! two — it encoded the harmonic contour by reusing discourse roles, in a muddled order — which is
//! exactly why the "Swing & A Miss" spine was inaudible. Round VI separates them: `DiscoursePlan`
//! says *why*, `BackbonePlan` says *what the harmony does*.
//!
//! The four-slot cell is generated ONCE per composition by a bounded, deterministic constraint
//! search over diatonic candidates. No real progression is transcribed; only the relational
//! geometry is fixed — a reach up, a soft miss, a warm window, a rounded reset. The cell then tiles
//! bar-aligned across the piece so the ear can learn it, and later cycles TRANSFORM the frozen cell
//! (a warm color deepens) so recurrence is recognizable without being static.

use super::contract::CompositionGrammar;
use super::harmony::{degree_implied_tension, diatonic_chord};
use super::rng::Rng;
use super::theory::{Chord, Function, Quality, Scale};
use super::world::MusicWorld;

/// An abstract harmonic gesture — WHAT the harmony does, independent of WHY (the discourse role).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarmonicGesture {
    /// Reach up and away from home — increased brightness / forward pull.
    Lift,
    /// The soft miss: a deceptive sidestep that refuses the obvious continuation ("almost… nope").
    Deflect,
    /// The warm window: a wide, colored opening that releases the pressure.
    Open,
    /// Return near enough to home to restart the bounce — without a giant terminal cadence.
    Reset,
}

impl HarmonicGesture {
    /// The cell, in canonical order: reach up, soft miss, warm opening, rounded reset. This exact
    /// relational order is the DeflectedLift identity — permuting it is a different song.
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

    /// The relational tension `[0,1]` this gesture reaches for — the target the cell search fits
    /// each slot's chord against. Lift reaches highest, Reset settles home.
    fn target_tension(self) -> f32 {
        match self {
            HarmonicGesture::Lift => 0.55,
            HarmonicGesture::Deflect => 0.45,
            HarmonicGesture::Open => 0.25,
            HarmonicGesture::Reset => 0.10,
        }
    }
}

/// One realized slot of the frozen cell.
#[derive(Debug, Clone, Copy)]
pub struct CellSlot {
    pub gesture: HarmonicGesture,
    /// The scale degree (0-based) the slot is built on.
    pub degree: i32,
    /// The realized chord (base color; per-cycle transforms are applied at tiling time).
    pub chord: Chord,
    /// The chord's harmonic function.
    pub function: Function,
}

/// A frozen four-slot harmonic identity — the song's spine. Later cycles transform it; the
/// relational four-slot identity (roots + gesture order) stays audible.
#[derive(Debug, Clone)]
pub struct HarmonicCell {
    pub slots: [CellSlot; 4],
}

impl HarmonicCell {
    /// The four root pitch-classes — the cell's audible signature.
    pub fn signature(&self) -> [i32; 4] {
        std::array::from_fn(|i| self.slots[i].chord.root_pc)
    }
}

/// The whole-piece backbone: the cell plus how many bars it spans before repeating.
#[derive(Debug, Clone)]
pub struct BackbonePlan {
    pub cell: HarmonicCell,
    /// The recurrence period in bars (from the contract). The cell's four slots are spread evenly
    /// across it, so a 4-bar recurrence is one chord per bar; an 8-bar recurrence is two per slot.
    pub recurrence_bars: u32,
}

impl BackbonePlan {
    fn bars_per_slot(&self) -> u32 {
        (self.recurrence_bars / 4).max(1)
    }

    /// The chord sounding in absolute `bar`: the cell slot for that bar, transformed for its cycle.
    pub fn chord_at_bar(&self, bar: u32) -> (Chord, Function, HarmonicGesture, u32) {
        let bps = self.bars_per_slot();
        let slot_ix = ((bar / bps) % 4) as usize;
        let cycle = bar / (bps * 4);
        let slot = self.cell.slots[slot_ix];
        (
            transform_for_cycle(slot.chord, slot.gesture, cycle),
            slot.function,
            slot.gesture,
            cycle,
        )
    }

    /// The scale degree at absolute `bar` (for ChordSpan provenance).
    pub fn degree_at_bar(&self, bar: u32) -> i32 {
        let bps = self.bars_per_slot();
        self.cell.slots[((bar / bps) % 4) as usize].degree
    }
}

/// The DeflectedLift backbone for a `world`, deterministic in `seed`. `None` for grammars whose
/// coherence does not rest on a recurring harmonic cell (they use the phrase-scope harmony engine).
pub fn plan_for(
    grammar: CompositionGrammar,
    world: &MusicWorld,
    seed: u64,
    recurrence_bars: u32,
) -> Option<BackbonePlan> {
    match grammar {
        CompositionGrammar::DeflectedLift => Some(BackbonePlan {
            cell: generate_deflected_lift_cell(world, seed),
            recurrence_bars: recurrence_bars.max(4),
        }),
        _ => None,
    }
}

/// The functional role a diatonic degree carries.
fn function_of_degree(degree: i32) -> Function {
    match degree.rem_euclid(7) {
        0 | 5 | 2 => Function::Tonic,
        3 | 1 => Function::Predominant,
        _ => Function::Dominant,
    }
}

/// Whether a chord's third is major (used to pick major vs minor warm colors).
fn is_major_third(q: Quality) -> bool {
    matches!(
        q,
        Quality::Maj
            | Quality::Maj7
            | Quality::Dom7
            | Quality::Aug
            | Quality::Maj6
            | Quality::Maj9
            | Quality::Add9
            | Quality::Dom9
            | Quality::Sus4
            | Quality::Sus2
    )
}

/// Apply each gesture's characteristic color to a diatonic base chord. Lift/Deflect keep their
/// diatonic color (a 7th where the world uses them — a forward pull, a deceptive minor). Open opens
/// a warm add9 window; Reset rings as a soft 6 that never slams the door — both draw on the extended
/// `Quality` vocabulary that used to sit dead in the type.
fn warm_color(base: Chord, gesture: HarmonicGesture) -> Chord {
    let major = is_major_third(base.quality);
    match gesture {
        HarmonicGesture::Open => Chord::new(
            base.root_pc,
            if major { Quality::Add9 } else { Quality::Min9 },
        ),
        HarmonicGesture::Reset => Chord::new(
            base.root_pc,
            if major { Quality::Maj6 } else { Quality::Min6 },
        ),
        HarmonicGesture::Lift | HarmonicGesture::Deflect => base,
    }
}

/// A bounded per-cycle transformation. Roots, functions and gesture order stay fixed (the cell's
/// identity); on odd cycles a warm color deepens — the Open window gains its maj7 shimmer, the Reset
/// rings as a 7th instead of a 6 — so a returning cycle is recognizably the same cell in a slightly
/// different light. Even cycles are exact recurrences.
fn transform_for_cycle(chord: Chord, gesture: HarmonicGesture, cycle: u32) -> Chord {
    if cycle % 2 == 0 {
        return chord;
    }
    match gesture {
        HarmonicGesture::Open => Chord::new(
            chord.root_pc,
            match chord.quality {
                Quality::Add9 => Quality::Maj9,
                other => other,
            },
        ),
        HarmonicGesture::Reset => Chord::new(
            chord.root_pc,
            match chord.quality {
                Quality::Maj6 => Quality::Maj7,
                Quality::Min6 => Quality::Min7,
                other => other,
            },
        ),
        _ => chord,
    }
}

/// The total cost of a candidate degree assignment `[lift, deflect, open, reset]` — lower is
/// better. Three inspectable terms: gesture fit (each slot's implied tension vs the gesture's
/// target), distinctness (four different roots — never a trivial I-I-I-I), an interior-tonic
/// penalty (only Reset should be home), plus a light preference for a smooth cyclic loop.
fn cell_cost(scale: &Scale, degs: &[i32; 4]) -> f32 {
    let roots: [i32; 4] = std::array::from_fn(|i| scale.degree_pitch(degs[i], 4).rem_euclid(12));
    let mut fit = 0.0;
    let mut interior_home = 0.0;
    for (i, &d) in degs.iter().enumerate() {
        fit += (degree_implied_tension(d) - HarmonicGesture::CELL[i].target_tension()).abs();
        if i < 3 && d.rem_euclid(7) == 0 {
            interior_home += 1.5; // a Lift/Deflect/Open that is really home defeats the bounce
        }
    }
    let mut dup = 0.0;
    for i in 0..4 {
        for j in (i + 1)..4 {
            if roots[i] == roots[j] {
                dup += 2.0;
            }
        }
    }
    let mut motion = 0.0;
    for i in 0..4 {
        let raw = (roots[(i + 1) % 4] - roots[i]).rem_euclid(12);
        motion += raw.min(12 - raw) as f32;
    }
    fit + dup + interior_home + 0.08 * motion
}

/// Generate the frozen four-slot DeflectedLift cell for `world`, deterministic in `seed`. A bounded
/// constraint search over per-gesture diatonic candidate pools; the RNG only breaks an exact tie.
pub fn generate_deflected_lift_cell(world: &MusicWorld, seed: u64) -> HarmonicCell {
    let scale = Scale::new(world.tonic_pc, world.mode);
    let s7 = world.use_sevenths;
    let mut rng = Rng::new(seed ^ 0xBACC_B0E1);

    // Per-gesture diatonic degree pools (0-based). Lift pulls forward (ii/IV/V), Deflect sidesteps
    // deceptively (vi/iii — the relative color), Open opens a warm subdominant/submediant window,
    // Reset is home.
    let lift_pool = [1, 3, 4];
    let deflect_pool = [5, 2];
    let open_pool = [3, 5];
    let reset = 0;

    let mut best_cost = f32::INFINITY;
    let mut ties: Vec<[i32; 4]> = Vec::new();
    for &lf in &lift_pool {
        for &df in &deflect_pool {
            for &op in &open_pool {
                let degs = [lf, df, op, reset];
                let cost = cell_cost(&scale, &degs);
                if cost < best_cost - 1e-6 {
                    best_cost = cost;
                    ties.clear();
                    ties.push(degs);
                } else if (cost - best_cost).abs() <= 1e-6 {
                    ties.push(degs);
                }
            }
        }
    }
    let degs = *rng.pick(&ties).unwrap_or(&[1, 5, 3, 0]);

    let slots = std::array::from_fn(|i| {
        let gesture = HarmonicGesture::CELL[i];
        let degree = degs[i];
        // Lift/Deflect take a 7th where the world uses them; Open/Reset are recolored warm below.
        let seventh = s7 && matches!(gesture, HarmonicGesture::Lift | HarmonicGesture::Deflect);
        let base = diatonic_chord(&scale, degree, seventh);
        CellSlot {
            gesture,
            degree,
            chord: warm_color(base, gesture),
            function: function_of_degree(degree),
        }
    });
    HarmonicCell { slots }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cell_is_deterministic_for_a_seed_and_world() {
        let a = generate_deflected_lift_cell(&MusicWorld::black_ice(), 2112);
        let b = generate_deflected_lift_cell(&MusicWorld::black_ice(), 2112);
        assert_eq!(a.signature(), b.signature());
    }

    #[test]
    fn the_cell_reaches_up_and_settles_home_in_order() {
        // Gestures are in canonical order, and the contour is honored: the Lift reaches for more
        // tension than the Reset settles to, and the Open window is calmer than the Lift.
        for world in MusicWorld::all() {
            let cell = generate_deflected_lift_cell(&world, 2112);
            let g: Vec<HarmonicGesture> = cell.slots.iter().map(|s| s.gesture).collect();
            assert_eq!(g, HarmonicGesture::CELL.to_vec(), "{}", world.name);
            let ten = |i: usize| degree_implied_tension(cell.slots[i].degree);
            assert!(
                ten(0) > ten(3),
                "{}: lift not tenser than reset",
                world.name
            );
            assert!(ten(2) < ten(0), "{}: open not calmer than lift", world.name);
            // Reset is home; the interior gestures are not.
            assert_eq!(cell.slots[3].degree.rem_euclid(7), 0, "{}", world.name);
            assert!(
                cell.slots[..3].iter().all(|s| s.degree.rem_euclid(7) != 0),
                "{}: an interior slot collapsed to home",
                world.name
            );
        }
    }

    #[test]
    fn the_cell_is_not_a_single_repeated_chord() {
        for world in MusicWorld::all() {
            let sig = generate_deflected_lift_cell(&world, 2112).signature();
            let distinct: std::collections::BTreeSet<i32> = sig.iter().copied().collect();
            assert!(
                distinct.len() >= 3,
                "{}: cell is nearly one chord ({sig:?})",
                world.name
            );
        }
    }

    #[test]
    fn the_open_and_reset_windows_use_extended_color() {
        // The whole point of finding the dead Quality vocabulary: Open opens a 9th, Reset a 6th.
        let cell = generate_deflected_lift_cell(&MusicWorld::swiss_signal(), 2112);
        let open = cell.slots[2].chord.quality;
        let reset = cell.slots[3].chord.quality;
        assert!(
            matches!(open, Quality::Add9 | Quality::Min9),
            "open is not a colored window: {open:?}"
        );
        assert!(
            matches!(reset, Quality::Maj6 | Quality::Min6),
            "reset is not a soft 6: {reset:?}"
        );
    }

    #[test]
    fn the_cell_recurs_exactly_on_even_cycles_and_transforms_on_odd() {
        let cell = generate_deflected_lift_cell(&MusicWorld::black_ice(), 2112);
        let bb = BackbonePlan {
            cell,
            recurrence_bars: 4,
        };
        // Bar 0 (Lift, cycle 0) recurs exactly at bar 8 (Lift, cycle 2).
        assert_eq!(bb.chord_at_bar(0).0, bb.chord_at_bar(8).0);
        // The Reset at bar 3 (cycle 0) and bar 7 (cycle 1) share a root but differ in color.
        let (r0, _, g0, _) = bb.chord_at_bar(3);
        let (r1, _, g1, _) = bb.chord_at_bar(7);
        assert_eq!(g0, HarmonicGesture::Reset);
        assert_eq!(g1, HarmonicGesture::Reset);
        assert_eq!(r0.root_pc, r1.root_pc);
        assert_ne!(
            r0.quality, r1.quality,
            "odd cycle did not transform the reset"
        );
    }
}
