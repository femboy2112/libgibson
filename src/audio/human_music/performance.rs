//! **PerformancePlan** — one shared performance that every instrument realizes a projection of.
//!
//! Through Round VI each part was realized blind from common static inputs: keys saw only the
//! chords, bass saw chords plus kick times, the lead saw chords plus the plan, and nobody saw
//! anybody else's notes. Call and response was a *slot relation* — `ThematicTrajectory::next_for`
//! handed the call to one discourse role and the answer to another, so "who answers" and "when"
//! were fixed by the form. Five composers shared a skeleton; none of them listened.
//!
//! The pipeline is now
//!
//! ```text
//! CompositionPlan (form, discourse, backbone timeline)   — the SONG
//!        │  + MusicWorld (timbre)  + MusicalLanguage (idiom)
//!        ▼
//! PerformancePlan                                         — the shared PERFORMANCE
//!   harmony + HarmonicContext timeline (local pitch palettes, relations, expectations)
//!   ActionPlan (path-lifted morphisms + the spine's own verbs + interaction actions)
//!   AccentGrid (one rhythmic field every player draws different subsets from)
//!   lead statement plan + calls → ResponseWindows → Interactions (with InteractionMemory)
//!   EnsembleBar per bar (who is foreground, each player's mode, the complexity budget,
//!                        the kinetic target)
//!        ▼
//! Score realization — lead, keys, bass, drums, pad each read THE SAME plan, and later players
//!                     read the earlier players' notes (they listen).
//! ```

use super::action::{ActionKind, ActionPlan, Agent};
use super::backbone::{DeflectWitness, HarmonicGesture};
use super::context::{analyze, HarmonicContext};
use super::ensemble::plan_ensemble;
use super::form::BEATS_PER_BAR;
use super::harmony::{ChordSpan, HarmonyEngine};
use super::interaction::plan_interactions;
use super::language::MusicalLanguage;
use super::motif::MotifBank;
use super::plan::CompositionPlan;
use super::region::apply_harmonic_actions;
use super::rng::Rng;
use super::theory::Scale;
use super::timeline::IntentTimeline;
use super::world::MusicWorld;

pub use super::ensemble::{BassMode, DrumsMode, EnsembleBar, KeysMode, PadMode};
pub use super::interaction::{
    Call, Interaction, InteractionMemory, LeadStatement, Response, ResponseMode, Transform,
};
pub use super::region::HarmonicEdit;

/// Sixteenth-note steps per 4/4 bar.
pub const STEPS: usize = 16;
/// Beats per step.
pub const STEP_BEATS: f64 = BEATS_PER_BAR / STEPS as f64;

/// The accent weights of one grid step, each in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StepWeight {
    /// Metric structure (the downbeat, beat 3).
    pub structural: f32,
    /// The backbeat (beats 2 and 4).
    pub backbeat: f32,
    /// An opportunity to syncopate (off-beats, the piece's gesture rhythm cells).
    pub syncopation: f32,
    /// A lead-in onto the next structural event.
    pub pickup: f32,
    /// An anticipation push onto a target.
    pub push: f32,
    /// A planned hole: players should leave it empty.
    pub hole: f32,
    /// An ensemble hit.
    pub hit: f32,
}

/// The shared rhythmic field: one [`StepWeight`] per sixteenth of every bar.
#[derive(Debug, Clone, PartialEq)]
pub struct AccentGrid {
    pub bars: Vec<[StepWeight; STEPS]>,
    /// The rhythm cell (off-beat steps) each gesture uses in this piece — its rhythmic identity.
    pub cells: [Vec<usize>; 4],
}

impl AccentGrid {
    /// The beat of `(bar, step)`.
    pub fn beat_of(bar: u32, step: usize) -> f64 {
        bar as f64 * BEATS_PER_BAR + step as f64 * STEP_BEATS
    }

    /// The `(bar, step)` holding `beat` (floor).
    pub fn step_of(beat: f64) -> (u32, usize) {
        let b = (beat / BEATS_PER_BAR).floor().max(0.0);
        let s = ((beat - b * BEATS_PER_BAR) / STEP_BEATS).round() as usize;
        if s >= STEPS {
            (b as u32 + 1, 0)
        } else {
            (b as u32, s)
        }
    }

    /// The weights at `(bar, step)` (zero outside the piece).
    pub fn at(&self, bar: u32, step: usize) -> StepWeight {
        self.bars
            .get(bar as usize)
            .map(|b| b[step.min(STEPS - 1)])
            .unwrap_or_default()
    }

    /// The weights at `beat`.
    pub fn at_beat(&self, beat: f64) -> StepWeight {
        let (b, s) = Self::step_of(beat);
        self.at(b, s)
    }

    /// Whether `beat` falls in a planned hole.
    pub fn is_hole(&self, beat: f64) -> bool {
        self.at_beat(beat).hole >= 0.5
    }
}

fn gesture_ix(g: HarmonicGesture) -> usize {
    match g {
        HarmonicGesture::Lift => 0,
        HarmonicGesture::Deflect => 1,
        HarmonicGesture::Open => 2,
        HarmonicGesture::Reset => 3,
    }
}

/// Options that select the calibration probes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerformanceOptions {
    pub language: MusicalLanguage,
    /// Build the action plan (false = the mood-without-action probe).
    pub actions: bool,
    pub responses: ResponseMode,
}

impl Default for PerformanceOptions {
    fn default() -> Self {
        PerformanceOptions {
            language: MusicalLanguage::default(),
            actions: true,
            responses: ResponseMode::Free,
        }
    }
}

/// The whole shared performance.
#[derive(Debug, Clone)]
pub struct PerformancePlan {
    pub language: MusicalLanguage,
    pub region: Scale,
    pub chords: Vec<ChordSpan>,
    pub contexts: Vec<HarmonicContext>,
    pub deflects: Vec<DeflectWitness>,
    pub edits: Vec<HarmonicEdit>,
    pub actions: ActionPlan,
    pub accent: AccentGrid,
    pub statements: Vec<LeadStatement>,
    pub interactions: Vec<Interaction>,
    pub ensemble: Vec<EnsembleBar>,
    pub bank: MotifBank,
    pub response_mode: ResponseMode,
    pub total_beats: f64,
}

impl PerformancePlan {
    /// Build the performance of `plan` under `world`.
    pub fn build(
        timeline: &IntentTimeline,
        plan: &CompositionPlan,
        world: &MusicWorld,
        seed: u64,
        opts: PerformanceOptions,
    ) -> PerformancePlan {
        let lang = opts.language;
        let region = Scale::new(world.tonic_pc, world.mode);
        let total_beats = plan.form.total_bars as f64 * BEATS_PER_BAR;
        let targets = plan.targets();

        // 1. Harmony: the backbone realized in this world and language, or the phrase engine.
        let (mut chords, deflects) = match &plan.backbone {
            Some(tl) => {
                let r = super::backbone::realize(tl, world, &lang, seed);
                (r.spans, r.deflects)
            }
            None => (
                HarmonyEngine::new(world, seed).generate(&targets, plan.contract.resolution),
                Vec::new(),
            ),
        };

        // 2. Actions.
        let mut actions = if opts.actions {
            ActionPlan::build(timeline, plan.backbone.as_ref(), &lang, total_beats, seed)
        } else {
            ActionPlan::none()
        };

        // 2b. An ensemble push/hit needs an ensemble: where the arrangement leaves fewer than two
        //     players on stage (the intro's pad+bass), the verb is carried by the one who is there —
        //     it becomes that player's pickup, not a phantom tutti.
        for a in actions.actions.iter_mut() {
            if !matches!(a.kind, ActionKind::Push | ActionKind::Hit)
                || a.initiator != Agent::Ensemble
            {
                continue;
            }
            let ph = plan.form.phrase_at(a.start_beat.min(total_beats - 1e-6)).ix as usize;
            let arr = plan.arrangement.at(ph);
            let on_stage: Vec<Agent> = [
                (Agent::Bass, arr.bass),
                (Agent::Keys, arr.keys),
                (Agent::Drums, arr.drums),
                (Agent::Lead, arr.lead),
            ]
            .into_iter()
            .filter(|(_, r)| r.is_audible())
            .map(|(g, _)| g)
            .collect();
            if on_stage.len() < 2 {
                if let Some(&solo) = on_stage.first() {
                    a.kind = ActionKind::Pickup;
                    a.initiator = solo;
                    a.target_beat = a.target_beat.or(Some(a.end_beat()));
                }
            }
        }

        // 3. Harmonic actions edit the harmony (so they are heard, not merely labelled).
        let edits = apply_harmonic_actions(&mut chords, &actions, &region);
        let contexts = analyze(&chords, &region);

        // 4. The shared rhythmic field.
        let accent = build_accent_grid(plan, &actions, &lang, seed);

        // 5. Lead statements, calls, responses.
        let bank = MotifBank::generate(&region, seed ^ 0x3E10_D1E5);
        let (statements, interactions) = plan_interactions(
            plan,
            &mut actions,
            &accent,
            &chords,
            &bank,
            &lang,
            opts.responses,
            opts.actions,
            seed,
        );

        // 6. The ensemble per bar.
        let ensemble = plan_ensemble(plan, &actions, &statements, &interactions, &lang);

        PerformancePlan {
            language: lang,
            region,
            chords,
            contexts,
            deflects,
            edits,
            actions,
            accent,
            statements,
            interactions,
            ensemble,
            bank,
            response_mode: opts.responses,
            total_beats,
        }
    }

    /// The ensemble plan for `bar`.
    pub fn bar(&self, bar: u32) -> Option<&EnsembleBar> {
        self.ensemble.get(bar as usize)
    }

    /// The ensemble plan for the bar holding `beat`.
    pub fn bar_at(&self, beat: f64) -> Option<&EnsembleBar> {
        self.bar((beat / BEATS_PER_BAR).floor().max(0.0) as u32)
    }

    /// The harmonic context sounding at `beat`.
    pub fn context_at(&self, beat: f64) -> Option<&HarmonicContext> {
        super::context::context_at(&self.contexts, beat)
    }

    /// The responses assigned to `agent`.
    pub fn responses_for(&self, agent: Agent) -> impl Iterator<Item = (&Call, &Response)> {
        self.interactions.iter().filter_map(move |i| {
            i.response
                .as_ref()
                .filter(|r| r.responder == agent && r.transform != Transform::Silence)
                .map(|r| (&i.call, r))
        })
    }

    /// The calls initiated by `agent` other than lead statements (figures a player states).
    pub fn figure_calls_for(&self, agent: Agent) -> impl Iterator<Item = &Call> {
        self.interactions
            .iter()
            .map(|i| &i.call)
            .filter(move |c| c.initiator == agent && c.statement.is_none())
    }

    /// A compact dump of the performance: statements, interactions, ensemble.
    pub fn dump(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "performance: language={} responses={:?} statements={} interactions={} edits={}",
            self.language.id.label(),
            self.response_mode,
            self.statements.len(),
            self.interactions.len(),
            self.edits.len()
        );
        for i in &self.interactions {
            let c = &i.call;
            match &i.response {
                Some(r) => {
                    let _ = writeln!(
                        s,
                        "  call a{} {:<5} {:>6.2}..{:<6.2} -> {:<5} {:+.2}b {:<8}{}{}",
                        c.action,
                        c.initiator.label(),
                        c.start_beat,
                        c.end_beat,
                        r.responder.label(),
                        r.latency,
                        r.transform.label(),
                        if r.overlap { " overlap" } else { "" },
                        if r.crosses_chord {
                            " crosses-chord"
                        } else {
                            ""
                        }
                    );
                }
                None => {
                    let _ = writeln!(
                        s,
                        "  call a{} {:<5} {:>6.2}..{:<6.2} -> (no room)",
                        c.action,
                        c.initiator.label(),
                        c.start_beat,
                        c.end_beat
                    );
                }
            }
        }
        let _ = writeln!(
            s,
            "  ensemble (bar: gesture fg pad/keys/bass/drums kinetic):"
        );
        for e in &self.ensemble {
            let _ = writeln!(
                s,
                "    {:>2} {:<8} {:<6} {:?}/{:?}/{:?}/{:?} k={:.2}",
                e.bar,
                e.gesture.map(|g| g.label()).unwrap_or("-"),
                e.foreground.label(),
                e.pad,
                e.keys,
                e.bass,
                e.drums,
                e.kinetic
            );
        }
        s
    }
}

/// Build the accent grid: meter, the piece's per-gesture rhythm cells (varied per bar by
/// rotation and omission), and the actions' pushes, hits, holes, pickups and displacements.
fn build_accent_grid(
    plan: &CompositionPlan,
    actions: &ActionPlan,
    lang: &MusicalLanguage,
    seed: u64,
) -> AccentGrid {
    let total_bars = plan.form.total_bars as usize;
    let mut rng = Rng::new(seed ^ 0x6A1D_0C3E);
    let sync = lang.syncopation;
    let sixteenths = lang.surface_subdivision >= 4;

    // Per-gesture rhythm cells: a small set of off-beat steps chosen once per piece — the
    // gesture's rhythmic identity (Lift pushes most, Open least).
    let offbeats: Vec<usize> = (0..STEPS)
        .filter(|s| s % 4 != 0 && (sixteenths || s % 2 == 0))
        .collect();
    let sizes = [4usize, 3, 2, 3];
    let cells: [Vec<usize>; 4] = std::array::from_fn(|g| {
        let mut pool = offbeats.clone();
        let mut cell = Vec::new();
        for _ in 0..sizes[g].min(pool.len()) {
            // Prefer eighth off-beats (stronger syncopation), sixteenths as colour.
            let weights: Vec<f32> = pool
                .iter()
                .map(|s| if s % 2 == 0 { 1.0 } else { 0.55 })
                .collect();
            let total: f32 = weights.iter().sum();
            let mut r = rng.range_f32(0.0, total);
            let mut k = 0;
            for (i, w) in weights.iter().enumerate() {
                if r < *w {
                    k = i;
                    break;
                }
                r -= w;
            }
            cell.push(pool.remove(k));
        }
        cell.sort_unstable();
        cell
    });

    let mut bars = vec![[StepWeight::default(); STEPS]; total_bars];
    for (bar, w) in bars.iter_mut().enumerate() {
        for (s, sw) in w.iter_mut().enumerate() {
            sw.structural = match s {
                0 => 1.0,
                8 => 0.8,
                4 | 12 => 0.35,
                _ => 0.0,
            };
            sw.backbeat = if s == 4 || s == 12 { 1.0 } else { 0.0 };
            sw.syncopation = if s % 4 == 2 {
                0.4 * sync
            } else if s % 2 == 1 && sixteenths {
                0.2 * sync
            } else {
                0.0
            };
            if s >= 14 {
                sw.pickup = 0.3;
            }
        }
        // The gesture's cell, varied per bar: rotated by an eighth on odd bars of the slot,
        // thinned on the slot's last bar (a breath), with a pickup into the next slot.
        if let Some(bb) = &plan.backbone {
            if let Some(slot) = bb.slot_at_bar(bar as u32) {
                let in_slot = bar as u32 - slot.start_bar;
                let last = bar as u32 + 1 == slot.end_bar();
                let cell = &cells[gesture_ix(slot.gesture)];
                let rot = if in_slot % 2 == 1 { 2 } else { 0 };
                for (k, &c) in cell.iter().enumerate() {
                    if last && k + 1 == cell.len() && cell.len() > 1 {
                        continue;
                    }
                    let st = (c + rot) % STEPS;
                    if st % 4 == 0 {
                        continue;
                    }
                    w[st].syncopation = w[st].syncopation.max(0.35 + 0.6 * sync);
                }
                if last {
                    w[14].pickup = 0.8;
                    w[15].pickup = 0.7;
                }
            }
        }
    }

    // Actions mutate the grid.
    for a in &actions.actions {
        let (b0, s0) = AccentGrid::step_of(a.start_beat);
        let steps = ((a.dur_beats / STEP_BEATS).round() as usize).max(1);
        let mut each = |f: &mut dyn FnMut(&mut StepWeight, usize)| {
            for k in 0..steps {
                let abs = b0 as usize * STEPS + s0 + k;
                let (bi, si) = (abs / STEPS, abs % STEPS);
                if let Some(bw) = bars.get_mut(bi) {
                    f(&mut bw[si], si);
                }
            }
        };
        match a.kind {
            ActionKind::Push => each(&mut |w, _| {
                w.push = 1.0;
                w.hit = w.hit.max(0.7);
            }),
            ActionKind::Hit => {
                let (b, s) = AccentGrid::step_of(a.start_beat);
                if let Some(bw) = bars.get_mut(b as usize) {
                    bw[s].hit = 1.0;
                }
            }
            ActionKind::Break => each(&mut |w, _| w.hole = 1.0),
            ActionKind::Displace => each(&mut |w, s| {
                if s % 4 != 0 {
                    w.syncopation = (w.syncopation * 1.5 + 0.1).min(1.0);
                } else if s == 8 {
                    w.structural *= 0.5;
                }
            }),
            ActionKind::Pullback => each(&mut |w, s| {
                w.syncopation *= 0.5;
                if s % 2 == 1 {
                    w.hole = w.hole.max(0.5);
                }
            }),
            ActionKind::Accelerate => each(&mut |w, s| {
                if s % 2 == 1 {
                    w.syncopation = (w.syncopation + 0.3).min(1.0);
                }
            }),
            ActionKind::Fill | ActionKind::Pickup => each(&mut |w, _| {
                w.pickup = w.pickup.max(0.9);
            }),
            _ => {}
        }
    }
    AccentGrid { bars, cells }
}

#[cfg(test)]
mod tests {
    use super::super::semantic::deflected_lift_trace;
    use super::*;

    fn flagship(opts: PerformanceOptions) -> PerformancePlan {
        let trace = deflected_lift_trace(120.0);
        let tl = IntentTimeline::walk(&trace);
        let plan = CompositionPlan::build_with_contract(
            &tl,
            30,
            super::super::contract::CoherenceContract::for_grammar(
                super::super::contract::CompositionGrammar::DeflectedLift,
            ),
        );
        PerformancePlan::build(&tl, &plan, &MusicWorld::black_ice(), 2112, opts)
    }

    #[test]
    fn free_responses_vary_responder_and_latency() {
        let p = flagship(PerformanceOptions::default());
        let rs: Vec<&Response> = p
            .interactions
            .iter()
            .filter_map(|i| i.response.as_ref())
            .filter(|r| r.transform != Transform::Silence)
            .collect();
        assert!(rs.len() >= 3, "too few responses: {}", rs.len());
        let responders: std::collections::BTreeSet<Agent> =
            rs.iter().map(|r| r.responder).collect();
        assert!(responders.len() >= 2, "one responder only: {responders:?}");
        let lat: std::collections::BTreeSet<i64> = rs
            .iter()
            .map(|r| (r.latency * 2.0).round() as i64)
            .collect();
        assert!(lat.len() >= 3, "latencies too uniform: {lat:?}");
    }

    #[test]
    fn clockwork_responses_are_rigid_by_construction() {
        let p = flagship(PerformanceOptions {
            responses: ResponseMode::Clockwork,
            ..PerformanceOptions::default()
        });
        for r in p.interactions.iter().filter_map(|i| i.response.as_ref()) {
            assert_eq!(r.responder, Agent::Keys);
            assert_eq!(AccentGrid::step_of(r.start_beat).1, 10);
        }
    }

    #[test]
    fn the_accent_grid_varies_across_bars_of_a_slot() {
        let p = flagship(PerformanceOptions::default());
        let sig = |bar: u32| -> Vec<usize> {
            (0..STEPS)
                .filter(|&s| p.accent.at(bar, s).syncopation > 0.5)
                .collect()
        };
        // Bars 4..8 are one Lift slot: the rotation makes consecutive bars differ.
        assert_ne!(sig(4), sig(5));
    }

    #[test]
    fn disabling_actions_keeps_the_harmony_and_removes_the_verbs() {
        let on = flagship(PerformanceOptions::default());
        let off = flagship(PerformanceOptions {
            actions: false,
            ..PerformanceOptions::default()
        });
        assert!(off.actions.actions.is_empty());
        assert!(off.interactions.is_empty());
        assert_eq!(on.statements.len(), off.statements.len());
        assert!(on.actions.actions.len() > off.actions.actions.len());
        assert_eq!(on.chords.len(), off.chords.len());
    }
}
