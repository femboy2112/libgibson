//! The harmonic engine: generate a chord progression over the composition plan's phrase targets,
//! planned at **phrase scope** and steered by each phrase's discourse targets and the world's
//! vocabulary.
//!
//! Round I picked a function from a scalar tension, then rolled a degree, and a "cadence" just
//! forced the final chord to tonic with no preparation — coherent only by coincidence. Round II
//! plans each phrase as a unit: the phrase *ends* on a prepared cadence (…PD → D → T), the
//! interior degrees are chosen to MINIMIZE an inspectable [`MorphismCost`] (voice-leading,
//! tension error, repetition) rather than by dice, and a secondary dominant is a real
//! obligation — if we emit a V/x, the next slot resolves to its target, no exceptions.
//! Borrowed/mixture/mediant colors only *tint* an already-coherent interior slot; they never
//! override a cadence, a cadence-prep, or a resolution.
//!
//! Diatonic chords are still derived from the scale (quality classified from the actual stacked
//! scale thirds, so it is correct in any mode).

use super::backbone::BackbonePlan;
use super::contract::ResolutionPolicy;
use super::discourse::Closure;
use super::form::BEATS_PER_BAR;
use super::intent::{CostWeights, MorphismCost};
use super::rng::Rng;
use super::theory::{Chord, Function, Quality, Scale};
use super::world::MusicWorld;

/// A chord placed in time with its analysis.
#[derive(Debug, Clone, Copy)]
pub struct ChordSpan {
    pub start_beat: f64,
    pub dur_beats: f32,
    pub chord: Chord,
    pub function: Function,
    /// The scale degree (0-based) the chord is built on, or -1 for a borrowed/applied chord.
    pub degree: i32,
    /// A short provenance note (e.g. "V/of", "res", "bVI mix").
    pub note: &'static str,
}

/// Generates progressions for a world.
pub struct HarmonyEngine {
    scale: Scale,
    world_use_sevenths: bool,
    allow_secondary: bool,
    allow_mixture: bool,
    allow_chromatic_mediant: bool,
    rng: Rng,
}

impl HarmonyEngine {
    /// A harmony engine for `world`, seeded by `seed`.
    pub fn new(world: &MusicWorld, seed: u64) -> HarmonyEngine {
        HarmonyEngine {
            scale: Scale::new(world.tonic_pc, world.mode),
            world_use_sevenths: world.use_sevenths,
            allow_secondary: world.allow_secondary_dominant,
            allow_mixture: world.allow_modal_mixture,
            allow_chromatic_mediant: world.allow_chromatic_mediant,
            rng: Rng::new(seed ^ 0xC0FF_EE00),
        }
    }

    /// The scale this engine works in.
    pub fn scale(&self) -> Scale {
        self.scale
    }

    /// Build a diatonic chord on scale `degree` (0-based), a seventh if requested.
    pub fn diatonic_chord(&self, degree: i32, seventh: bool) -> Chord {
        diatonic_chord(&self.scale, degree, seventh)
    }

    /// Generate the full progression from the composition plan's [`super::plan::PhraseTarget`]s,
    /// planned phrase by phrase, realizing each phrase's discourse [`Closure`] under the piece's
    /// [`ResolutionPolicy`].
    ///
    /// The targets tile `[0, total_beats)` contiguously; the phrase's discourse goal is the single
    /// authority for harmonic rhythm (its density target) and functional heat (its tension target).
    /// **Not every phrase ends with a full stop.** Under `Functional` resolution the phrase's
    /// closure decides the cadence — `Strong` = prepared PD→D→T, `Weak` = plagal IV→I, `Half` =
    /// end on the dominant, `Deceptive` = V→vi, `Deferred` = V→IV (evaded), `Open` = no cadence at
    /// all. Under `Loop`/`ModalPedal` there is no functional cadence: the phrase cycles or pedals
    /// home. This is the closure hierarchy that lets long-range expectation accumulate instead of
    /// being cashed out every four bars.
    pub fn generate(
        &mut self,
        targets: &[super::plan::PhraseTarget],
        resolution: ResolutionPolicy,
    ) -> Vec<ChordSpan> {
        self.generate_with_backbone(targets, resolution, None)
    }

    /// Like [`HarmonyEngine::generate`], but when a [`BackbonePlan`] is supplied the progression IS
    /// that backbone: the frozen Lift → Deflect → Open → Reset cell, tiled bar-aligned across the
    /// piece and transformed per cycle — a small cyclic harmonic identity the ear can learn. Without
    /// a backbone this is the phrase-scope cadential engine, unchanged.
    pub fn generate_with_backbone(
        &mut self,
        targets: &[super::plan::PhraseTarget],
        resolution: ResolutionPolicy,
        backbone: Option<&BackbonePlan>,
    ) -> Vec<ChordSpan> {
        if targets.is_empty() {
            return Vec::new();
        }
        if let Some(bb) = backbone {
            return generate_backbone_tiled(targets, bb);
        }
        let total_beats = targets.last().map(|t| t.end_beat()).unwrap_or(0.0);
        let functional = matches!(resolution, ResolutionPolicy::Functional);

        let mut spans = Vec::new();
        // Voice-leading memory threads across phrase boundaries — the line is continuous even
        // where the structure isn't. The opening reference is the tonic.
        let mut prev_root: i32 = self.scale.tonic_pc;
        let mut prev_degree: i32 = 0;

        for t in targets {
            let ps = t.start_beat();
            let pe = t.end_beat().min(total_beats);
            if pe <= ps + 1e-9 {
                continue;
            }
            // The phrase's own discourse targets — constant across the phrase — drive both the
            // harmonic rhythm and the functional heat; its closure decides the cadence.
            let tension = t.goal.tension_target;
            let closure = t.goal.closure;
            let slots = carve_slots(t.goal.density_target, ps, pe);
            let n = slots.len();
            // A V/x set here mandates the next interior slot resolve to `target` — an
            // obligation, not a suggestion. Reset at each phrase: cadences don't inherit debts.
            let mut pending_resolve: Option<i32> = None;

            for (i, &(sb, dur)) in slots.iter().enumerate() {
                let is_last = i == n - 1;
                let is_prep = n >= 2 && i == n - 2; // penultimate (cadence approach)
                let is_pd_prep = n >= 3 && i == n - 3;
                let s7 = self.world_use_sevenths;

                // A closure-specific cadence overrides the last one or two slots; every other slot
                // (and every slot under a non-functional policy) is interior. `forced` is `Some`
                // for an overridden slot, `None` for interior.
                let forced: Option<(Chord, Function, i32, &'static str)> = if !functional {
                    // Loop / modal-pedal: the phrase cycles or pedals home — no functional cadence.
                    if is_last {
                        let note = if matches!(resolution, ResolutionPolicy::Loop) {
                            "loop"
                        } else {
                            "pedal"
                        };
                        Some((self.diatonic_chord(0, false), Function::Tonic, 0, note))
                    } else {
                        None
                    }
                } else if is_last {
                    match closure {
                        Closure::Strong => {
                            Some((self.diatonic_chord(0, false), Function::Tonic, 0, ""))
                        }
                        Closure::Weak => {
                            Some((self.diatonic_chord(0, false), Function::Tonic, 0, "plagal"))
                        }
                        Closure::Half => {
                            Some((self.diatonic_chord(4, s7), Function::Dominant, 4, "half"))
                        }
                        // V→vi: prepared by the dominant at n-2, resolves deceptively to the
                        // submediant (tonic-function, but not home).
                        Closure::Deceptive => {
                            Some((self.diatonic_chord(5, s7), Function::Tonic, 5, "dec"))
                        }
                        // The expected resolution is evaded — a V→IV retrogression that refuses
                        // to close, leaving the dominant hanging.
                        Closure::Deferred => Some((
                            self.diatonic_chord(3, false),
                            Function::Predominant,
                            3,
                            "defer",
                        )),
                        Closure::Open => None, // no cadence: interior continuation
                    }
                } else if is_prep {
                    match closure {
                        Closure::Strong | Closure::Deceptive | Closure::Deferred => {
                            Some((self.diatonic_chord(4, s7), Function::Dominant, 4, ""))
                        }
                        // Half and plagal approach the final chord from the subdominant.
                        Closure::Half | Closure::Weak => Some((
                            self.diatonic_chord(3, s7 && tension > 0.4),
                            Function::Predominant,
                            3,
                            "",
                        )),
                        Closure::Open => None,
                    }
                } else if is_pd_prep && matches!(closure, Closure::Strong) {
                    let d = self.choose_interior_degree(
                        Function::Predominant,
                        tension,
                        prev_root,
                        prev_degree,
                    );
                    Some((
                        self.diatonic_chord(d, s7 && tension > 0.4),
                        Function::Predominant,
                        d,
                        "",
                    ))
                } else {
                    None
                };

                let (chord, function, degree, note) = match forced {
                    Some(x) => x,
                    // Interior slot. A pending resolution outranks everything else here.
                    None => {
                        if let Some(target) = pending_resolve.take() {
                            let func = function_of_degree(target);
                            let seventh = s7 && (tension > 0.4 || func == Function::Dominant);
                            (self.diatonic_chord(target, seventh), func, target, "res")
                        } else {
                            let func = function_for_tension(tension);
                            // Interior slots run 0..(n-3); how many are left including this one.
                            let interior_len = n.saturating_sub(3);
                            let remaining_interior = interior_len.saturating_sub(i);

                            if self.allow_secondary
                                && func == Function::Dominant
                                && remaining_interior >= 2
                                && self.rng.chance(0.5)
                            {
                                // A secondary dominant: Dom7 a perfect fifth above the target's
                                // root. We take on the debt now; the next interior slot pays it.
                                let target = self.pick_secondary_target(prev_degree);
                                let target_root = self.scale.degree_pitch(target, 4).rem_euclid(12);
                                let dom_root = (target_root + 7).rem_euclid(12);
                                pending_resolve = Some(target);
                                (
                                    Chord::new(dom_root, Quality::Dom7),
                                    Function::Dominant,
                                    -1,
                                    "V/of",
                                )
                            } else {
                                // Cost-driven diatonic choice, then an optional color tint.
                                let d = self.choose_interior_degree(
                                    func,
                                    tension,
                                    prev_root,
                                    prev_degree,
                                );
                                let seventh = s7 && (tension > 0.4 || func == Function::Dominant);
                                let mut chord = self.diatonic_chord(d, seventh);
                                let mut degree = d;
                                let mut note = "";
                                // Colors tint a coherent path; they never fabricate one. Guarded to
                                // interior diatonic slots only — never a cadence, prep, or resolution.
                                if self.allow_mixture && tension > 0.6 && self.rng.chance(0.2) {
                                    let bvi = (self.scale.tonic_pc + 8).rem_euclid(12);
                                    chord = Chord::new(
                                        bvi,
                                        if s7 { Quality::Maj7 } else { Quality::Maj },
                                    );
                                    degree = -1;
                                    note = "bVI mix";
                                } else if self.allow_chromatic_mediant && self.rng.chance(0.12) {
                                    let cm = (self.scale.tonic_pc + 4).rem_euclid(12);
                                    chord = Chord::new(cm, Quality::Maj);
                                    degree = -1;
                                    note = "chr med";
                                }
                                (chord, func, degree, note)
                            }
                        }
                    }
                };

                spans.push(ChordSpan {
                    start_beat: sb,
                    dur_beats: dur,
                    chord,
                    function,
                    degree,
                    note,
                });
                prev_root = chord.root_pc;
                prev_degree = degree;
            }
        }
        spans
    }

    /// Choose an interior degree from `func`'s pool by minimizing the weighted [`MorphismCost`]
    /// against the previous chord and the form's target tension. The cost vector drives the
    /// choice; the RNG only splits an exact tie. This is the whole point of Round II harmony —
    /// the progression is *optimized*, not rolled.
    pub(crate) fn choose_interior_degree(
        &mut self,
        func: Function,
        target_tension: f32,
        prev_root: i32,
        prev_degree: i32,
    ) -> i32 {
        let pool: &[i32] = match func {
            Function::Tonic => &[0, 5, 2],
            Function::Predominant => &[3, 1],
            Function::Dominant => &[4, 6],
        };
        let w = CostWeights::default();
        let mut best_score = f32::INFINITY;
        let mut best: Vec<i32> = Vec::new();
        for &d in pool {
            let root = self.scale.degree_pitch(d, 4).rem_euclid(12);
            let score = degree_cost(root, d, target_tension, prev_root, prev_degree).weighted(&w);
            if score < best_score - 1e-6 {
                best_score = score;
                best.clear();
                best.push(d);
            } else if (score - best_score).abs() <= 1e-6 {
                best.push(d);
            }
        }
        if best.len() == 1 {
            best[0]
        } else {
            *self.rng.pick(&best).unwrap_or(&0)
        }
    }

    /// Pick a diatonic target for a secondary dominant, avoiding an immediate repeat of the
    /// previous degree. The idiomatic targets: V (V/V), ii (V/ii), vi (V/vi).
    fn pick_secondary_target(&mut self, prev: i32) -> i32 {
        let pool = [4, 1, 5];
        let filtered: Vec<i32> = pool.iter().copied().filter(|&d| d != prev).collect();
        let chosen: &[i32] = if filtered.is_empty() {
            &pool
        } else {
            &filtered
        };
        *self.rng.pick(chosen).unwrap_or(&4)
    }
}

/// Build a diatonic chord on scale `degree` (0-based), classified from the actual stacked scale
/// thirds so the quality is correct in any mode; a seventh is added when `use_seventh` is set.
/// Shared by [`HarmonyEngine::diatonic_chord`] and the harmonic backbone cell generator.
pub(crate) fn diatonic_chord(scale: &Scale, degree: i32, use_seventh: bool) -> Chord {
    let root = scale.degree_pitch(degree, 4);
    let third = scale.degree_pitch(degree + 2, 4) - root;
    let fifth = scale.degree_pitch(degree + 4, 4) - root;
    let seventh = scale.degree_pitch(degree + 6, 4) - root;
    Chord::new(
        root.rem_euclid(12),
        classify(third, fifth, seventh, use_seventh),
    )
}

/// Tile a [`BackbonePlan`]'s frozen cell bar-aligned across the targets' span — one chord per bar,
/// each the cell's gesture chord for that bar (transformed per cycle). This makes the DeflectedLift
/// harmony a recurring, learnable cyclic identity instead of per-slot degree roulette.
fn generate_backbone_tiled(
    targets: &[super::plan::PhraseTarget],
    bb: &BackbonePlan,
) -> Vec<ChordSpan> {
    let total_beats = targets.last().map(|t| t.end_beat()).unwrap_or(0.0);
    let n_bars = (total_beats / BEATS_PER_BAR).round() as u32;
    let mut spans = Vec::with_capacity(n_bars as usize);
    for bar in 0..n_bars {
        let sb = bar as f64 * BEATS_PER_BAR;
        let dur = (total_beats - sb).min(BEATS_PER_BAR) as f32;
        if dur <= 1e-6 {
            break;
        }
        let (chord, function, gesture, _cycle) = bb.chord_at_bar(bar);
        spans.push(ChordSpan {
            start_beat: sb,
            dur_beats: dur,
            chord,
            function,
            degree: bb.degree_at_bar(bar),
            note: gesture.label(),
        });
    }
    spans
}

/// The rough functional heat of a scale degree — tonic-ish degrees are calm, the leading-tone
/// dominant is hot. This is what `tension_error` in [`degree_cost`] measures against the form.
pub(crate) fn degree_implied_tension(degree: i32) -> f32 {
    match degree.rem_euclid(7) {
        0 => 0.10, // I
        5 => 0.25, // vi
        2 => 0.30, // iii
        3 => 0.45, // IV
        1 => 0.50, // ii
        4 => 0.80, // V
        _ => 0.90, // vii°
    }
}

/// The enriched cost of choosing `candidate_root` / `degree` after `prev_root` / `prev_degree`,
/// aiming at `target_tension`. Three live dimensions: voice-leading (nearest semitone root
/// motion), tension error (functional heat vs. the form's target), and repetition. An
/// inspectable vector — exactly what `choose_interior_degree` minimizes.
pub(crate) fn degree_cost(
    candidate_root: i32,
    degree: i32,
    target_tension: f32,
    prev_root: i32,
    prev_degree: i32,
) -> MorphismCost {
    let raw = (candidate_root - prev_root).rem_euclid(12);
    let motion = raw.min(12 - raw) as f32; // nearest semitone root motion, 0..=6
    let implied = degree_implied_tension(degree);
    MorphismCost {
        voice_leading: motion,
        tension_error: (implied - target_tension).abs(),
        repetition: if degree == prev_degree { 1.0 } else { 0.0 },
        ..MorphismCost::default()
    }
}

/// The harmonic function a diatonic degree carries (used when resolving a secondary dominant).
fn function_of_degree(degree: i32) -> Function {
    match degree.rem_euclid(7) {
        0 | 5 | 2 => Function::Tonic,
        3 | 1 => Function::Predominant,
        _ => Function::Dominant, // 4, 6
    }
}

/// Carve `[ps, pe)` into contiguous slots, each sized by `chord_dur(density)` and clamped so the
/// last one lands exactly on `pe`. `chord_dur` is bounded below, so this terminates.
fn carve_slots(density: f32, ps: f64, pe: f64) -> Vec<(f64, f32)> {
    let mut slots = Vec::new();
    let mut beat = ps;
    while beat < pe - 1e-6 {
        let dur = chord_dur(density).min((pe - beat) as f32);
        slots.push((beat, dur));
        beat += dur as f64;
    }
    slots
}

/// Chord duration in beats from a density target.
fn chord_dur(density: f32) -> f32 {
    if density < 0.4 {
        (BEATS_PER_BAR * 2.0) as f32 // one chord every two bars
    } else if density < 0.72 {
        BEATS_PER_BAR as f32 // one per bar
    } else {
        (BEATS_PER_BAR / 2.0) as f32 // two per bar
    }
}

/// Harmonic function targeted by a tension level.
fn function_for_tension(t: f32) -> Function {
    if t < 0.35 {
        Function::Tonic
    } else if t < 0.62 {
        Function::Predominant
    } else {
        Function::Dominant
    }
}

/// Classify a chord quality from stacked-third semitone intervals.
fn classify(third: i32, fifth: i32, seventh: i32, use_seventh: bool) -> Quality {
    let t = third.rem_euclid(12);
    let f = fifth.rem_euclid(12);
    let s = seventh.rem_euclid(12);
    if !use_seventh {
        return match (t, f) {
            (4, 7) => Quality::Maj,
            (3, 7) => Quality::Min,
            (3, 6) => Quality::Dim,
            (4, 8) => Quality::Aug,
            _ => Quality::Maj,
        };
    }
    match (t, f, s) {
        (4, 7, 11) => Quality::Maj7,
        (4, 7, 10) => Quality::Dom7,
        (3, 7, 10) => Quality::Min7,
        (3, 7, 11) => Quality::MinMaj7,
        (3, 6, 10) => Quality::Min7b5,
        (3, 6, 9) => Quality::Dim7,
        (4, 8, _) => Quality::Aug,
        _ => Quality::Maj7,
    }
}

#[cfg(test)]
mod tests {
    use super::super::plan::CompositionPlan;
    use super::super::timeline::IntentTimeline;
    use super::*;
    use crate::audio::human_music::semantic::demo_trace;

    #[test]
    fn c_major_diatonic_chords_are_correct() {
        let w = MusicWorld::swiss_signal(); // C Ionian, triads
        let h = HarmonyEngine::new(&w, 1);
        // I = C major, ii = D minor, V = G major, vii° = B diminished.
        assert_eq!(h.diatonic_chord(0, false), Chord::new(0, Quality::Maj));
        assert_eq!(h.diatonic_chord(1, false), Chord::new(2, Quality::Min));
        assert_eq!(h.diatonic_chord(4, false), Chord::new(7, Quality::Maj));
        assert_eq!(h.diatonic_chord(6, false), Chord::new(11, Quality::Dim));
    }

    #[test]
    fn c_major_sevenths() {
        let mut w = MusicWorld::swiss_signal();
        w.use_sevenths = true;
        let h = HarmonyEngine::new(&w, 1);
        // Imaj7, ii m7, V7.
        assert_eq!(h.diatonic_chord(0, true), Chord::new(0, Quality::Maj7));
        assert_eq!(h.diatonic_chord(1, true), Chord::new(2, Quality::Min7));
        assert_eq!(h.diatonic_chord(4, true), Chord::new(7, Quality::Dom7));
    }

    fn demo_plan() -> CompositionPlan {
        let trace = demo_trace(120.0);
        let tl = IntentTimeline::walk(&trace);
        let total_bars = (trace.total_beats / BEATS_PER_BAR).round() as u32;
        CompositionPlan::build(&tl, total_bars)
    }

    #[test]
    fn progression_covers_the_plan_and_ends_on_tonic() {
        let plan = demo_plan();
        let mut h = HarmonyEngine::new(&MusicWorld::black_ice(), 42);
        let prog = h.generate(&plan.targets(), ResolutionPolicy::Functional);
        assert!(!prog.is_empty());
        // Contiguous in time (across phrase boundaries too).
        for w in prog.windows(2) {
            assert!((w[1].start_beat - (w[0].start_beat + w[0].dur_beats as f64)).abs() < 1e-3);
        }
        // The whole piece still ends home: the final phrase (dissolve) is a strong tonic cadence.
        let last = prog.last().unwrap();
        assert_eq!(last.function, Function::Tonic);
        assert_eq!(last.chord.root_pc, 9); // A minor tonic
    }

    #[test]
    fn deterministic_progression_for_seed() {
        let plan = demo_plan();
        let targets = plan.targets();
        let mut a = HarmonyEngine::new(&MusicWorld::vapor95(), 7);
        let mut b = HarmonyEngine::new(&MusicWorld::vapor95(), 7);
        let pa = a.generate(&targets, ResolutionPolicy::Functional);
        let pb = b.generate(&targets, ResolutionPolicy::Functional);
        assert_eq!(pa.len(), pb.len());
        for (x, y) in pa.iter().zip(pb.iter()) {
            assert_eq!(x.chord, y.chord);
            assert_eq!(x.start_beat, y.start_beat);
        }
    }

    #[test]
    fn closure_hierarchy_is_realized_not_a_period_every_phrase() {
        // Round III: the cadence realizes each phrase's discourse closure. Strong phrases still
        // close …D→T; but Half phrases end on the dominant and Deferred phrases refuse to resolve,
        // so NOT every phrase ends on a full stop — the whole point of the closure hierarchy.
        let plan = demo_plan();
        let targets = plan.targets();
        let mut h = HarmonyEngine::new(&MusicWorld::black_ice(), 42);
        let prog = h.generate(&targets, ResolutionPolicy::Functional);

        let mut strong_checked = 0;
        let mut non_tonic_endings = 0;
        for t in &targets {
            let in_phrase: Vec<&ChordSpan> = prog
                .iter()
                .filter(|s| {
                    s.start_beat >= t.start_beat() - 1e-6 && s.start_beat < t.end_beat() - 1e-6
                })
                .collect();
            if in_phrase.len() < 2 {
                continue;
            }
            let last = in_phrase.last().unwrap();
            match t.goal.closure {
                Closure::Strong => {
                    assert_eq!(
                        last.function,
                        Function::Tonic,
                        "strong phrase {} not tonic",
                        t.ix()
                    );
                    let prep = in_phrase[in_phrase.len() - 2];
                    assert_eq!(
                        prep.function,
                        Function::Dominant,
                        "strong phrase {} not dominant-prepared",
                        t.ix()
                    );
                    strong_checked += 1;
                }
                Closure::Half => {
                    assert_eq!(
                        last.function,
                        Function::Dominant,
                        "half-closed phrase {} should end on the dominant",
                        t.ix()
                    );
                    non_tonic_endings += 1;
                }
                Closure::Deferred => {
                    assert_ne!(
                        last.function,
                        Function::Tonic,
                        "deferred phrase {} should not resolve to tonic",
                        t.ix()
                    );
                    non_tonic_endings += 1;
                }
                _ => {}
            }
        }
        assert!(strong_checked > 0, "no strong cadence anywhere in the arc");
        assert!(
            non_tonic_endings > 0,
            "every phrase still ends resolved — the closure hierarchy is not realized"
        );
    }

    #[test]
    fn loop_policy_cycles_home_without_functional_cadences() {
        use super::super::plan::PhraseTarget;
        // Under Loop the phrase cycles home (a tonic tagged "loop"), never a functional cadence.
        let targets: Vec<PhraseTarget> = (0..4)
            .map(|i| PhraseTarget::test_flat(i * 4, 4, 0.5, 0.5))
            .collect();
        let mut h = HarmonyEngine::new(&MusicWorld::black_ice(), 3);
        let prog = h.generate(&targets, ResolutionPolicy::Loop);
        let loops = prog.iter().filter(|s| s.note == "loop").count();
        assert!(
            loops >= 4,
            "loop policy did not cycle each phrase home: {loops}"
        );
        assert!(
            prog.iter()
                .all(|s| !matches!(s.note, "half" | "dec" | "defer" | "plagal")),
            "loop policy emitted a functional cadence tag"
        );
    }

    #[test]
    fn secondary_dominants_resolve() {
        use super::super::plan::PhraseTarget;
        // A V/x is a real obligation — the next slot resolves to its target. Force many interior
        // Dominant slots with a single long, hot, dense phrase target so a V/x appears within a
        // short seed sweep. target_root = (dom_root - 7) mod 12.
        let targets = [PhraseTarget::test_flat(0, 16, 0.9, 0.9)];
        let mut found = false;
        for seed in 0..64u64 {
            let mut h = HarmonyEngine::new(&MusicWorld::black_ice(), seed);
            let prog = h.generate(&targets, ResolutionPolicy::Functional);
            for w in prog.windows(2) {
                if w[0].note.starts_with("V/") {
                    found = true;
                    let dom_root = w[0].chord.root_pc;
                    let target_root = (dom_root - 7).rem_euclid(12);
                    assert_eq!(
                        w[1].chord.root_pc, target_root,
                        "secondary dominant at seed {seed} does not resolve to its target"
                    );
                }
            }
        }
        assert!(
            found,
            "no secondary dominant produced across the seed sweep — test vacuous"
        );
    }

    #[test]
    fn interior_degree_minimizes_the_cost_vector() {
        // T3: the scorer must rank a smoother root motion cheaper than a jumpier one, and
        // generate's interior selector must actually return the argmin of that vector.
        let w = CostWeights::default();
        let smooth = degree_cost(7, 4, 0.8, 5, 3); // root motion 2
        let jumpy = degree_cost(11, 6, 0.8, 5, 3); // root motion 6
        assert!(
            smooth.weighted(&w) < jumpy.weighted(&w),
            "smoother candidate did not score cheaper"
        );

        // The live selector returns the hand-computed argmin (no tie here, so RNG is inert).
        let mut h = HarmonyEngine::new(&MusicWorld::black_ice(), 3);
        let (func, target_tension, prev_root, prev_degree) = (Function::Dominant, 0.8, 7, 0);
        let mut expect = 4;
        let mut best = f32::INFINITY;
        for d in [4, 6] {
            let root = h.scale().degree_pitch(d, 4).rem_euclid(12);
            let s = degree_cost(root, d, target_tension, prev_root, prev_degree).weighted(&w);
            if s < best {
                best = s;
                expect = d;
            }
        }
        let got = h.choose_interior_degree(func, target_tension, prev_root, prev_degree);
        assert_eq!(got, expect, "interior selection did not pick the argmin");
    }
}
