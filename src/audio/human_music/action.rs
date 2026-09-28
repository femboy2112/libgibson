//! **Musical actions** — the verbs the music performs, as first-class objects.
//!
//! Through Round VI the semantic verbs evaporated. `IntentMorphism` names real actions
//! (`Syncopate`, `Reharmonize`, `ThickenTexture`, `Suspend`, …) and every
//! [`super::timeline::IntentTransition`] records which ones an event applied — but nothing
//! downstream ever read `applied`; composition consumed only the resulting scalar
//! energy/tension/density/register. "Syncopate" became "energy went up", which is exactly how a
//! piece ends up with mood shifts and no action.
//!
//! This module path-lifts the morphisms. [`ActionPlan::build`] walks the intent timeline and the
//! backbone timeline and emits concrete [`MusicalAction`]s — each with a stable id, a cause
//! (the morphism or backbone gesture that produced it), an initiator, a time window, an effect,
//! a target, possible responders and the action it pays. Actions are *constraints on the
//! performance*, not canned MIDI patterns: the performance planner and the instrument realizers
//! read them and decide how to witness them. Every non-identity morphism an event applies either
//! yields at least one action carrying its provenance, or is recorded as a [`Deferral`] with a
//! reason; the diagnostics count unwitnessed morphisms against that ledger.

use super::backbone::{BackboneTimeline, HarmonicGesture};
use super::form::BEATS_PER_BAR;
use super::ids::ActionId;
use super::intent::IntentMorphism;
use super::language::MusicalLanguage;
use super::rng::Rng;
use super::timeline::IntentTimeline;

/// Who performs an action. Drums are an agent here even though they are not a pitched `Role`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Agent {
    Lead,
    Keys,
    Bass,
    Drums,
    Pad,
    /// Several players at once (an ensemble hit, a break, a unison figure).
    Ensemble,
}

impl Agent {
    /// A short lowercase label for dumps and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Agent::Lead => "lead",
            Agent::Keys => "keys",
            Agent::Bass => "bass",
            Agent::Drums => "drums",
            Agent::Pad => "pad",
            Agent::Ensemble => "ensemble",
        }
    }
}

/// What an action does. The minimal basis the realizers witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActionKind {
    /// A lead-in onto a structural downbeat.
    Pickup,
    /// An anticipation: the ensemble lands just before the target downbeat.
    Push,
    /// The surface decelerates (fewer onsets, a half-time feel).
    Pullback,
    /// The surface accelerates (a finer subdivision, double-time feel).
    Accelerate,
    /// An ensemble accent on a structural moment.
    Hit,
    /// The band stops for a moment: a planned hole.
    Break,
    /// A layer returns (or the band re-enters after a break).
    ReEntry,
    /// A sustained suspension carried across the barline.
    Hold,
    /// A harmony is recoloured or substituted.
    Reharmonize,
    /// A local tonal centre is tonicized by an applied dominant (a real region change).
    Tonicize,
    /// The backbone's miss: the expected arrival is withheld.
    Deflect,
    /// Arrival on an expected target.
    Resolve,
    /// Metric displacement: accents move off the grid's strong positions.
    Displace,
    /// Thematic fragmentation.
    Fragment,
    /// Thematic sequence.
    Sequence,
    /// A layer is added.
    Thicken,
    /// A layer is subtracted.
    Thin,
    /// A drum/bass fill leading into the next structural event.
    Fill,
    /// A statement that opens a response window.
    Call,
    /// A response to a call.
    Answer,
    /// Two or more players sound the same figure together.
    Unison,
}

impl ActionKind {
    /// A short lowercase label for dumps and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            ActionKind::Pickup => "pickup",
            ActionKind::Push => "push",
            ActionKind::Pullback => "pullback",
            ActionKind::Accelerate => "accelerate",
            ActionKind::Hit => "hit",
            ActionKind::Break => "break",
            ActionKind::ReEntry => "re-entry",
            ActionKind::Hold => "hold",
            ActionKind::Reharmonize => "reharmonize",
            ActionKind::Tonicize => "tonicize",
            ActionKind::Deflect => "deflect",
            ActionKind::Resolve => "resolve",
            ActionKind::Displace => "displace",
            ActionKind::Fragment => "fragment",
            ActionKind::Sequence => "sequence",
            ActionKind::Thicken => "thicken",
            ActionKind::Thin => "thin",
            ActionKind::Fill => "fill",
            ActionKind::Call => "call",
            ActionKind::Answer => "answer",
            ActionKind::Unison => "unison",
        }
    }
}

/// Why an action exists.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActionCause {
    /// A semantic event applied this morphism (`transition` indexes the intent timeline).
    Morphism {
        transition: usize,
        morphism: IntentMorphism,
    },
    /// A backbone gesture slot demands it (`slot` indexes the backbone timeline).
    Gesture {
        slot: usize,
        gesture: HarmonicGesture,
    },
    /// An interaction: a response to the call with action id `call`.
    Interaction { call: ActionId },
    /// A planned lead statement in phrase `phrase` (the thematic line itself: a call others may
    /// answer).
    Statement { phrase: u32 },
}

/// How much an action does — a small continuous effect vector derived from the semantic state
/// delta that caused it (Round VIIb). A faint, spacious Impact and a strong, compact, dangerous
/// Impact are the same *kind* of verb; they must not be the same *size* of verb.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectVector {
    /// Overall force in `[0, 1]`: half the arrival state's weight (dynamic + pressure), half the
    /// size of the change (so a Danger→Danger impact still lands hard, and a big swing reads big).
    pub strength: f32,
    /// Change in semantic pressure (`next - prev`).
    pub d_pressure: f32,
    /// Change in dynamic weight.
    pub d_dynamic: f32,
    /// Change in density: Spacious = -1, Normal = 0, Compact = +1.
    pub d_density: i8,
    /// Change in elevation: Flat = 0, Raised = 1, Overlay = 2.
    pub d_elevation: i8,
    /// How compact the arrival is, in `[0, 1]` (Spacious 0, Normal 0.5, Compact 1).
    pub compactness: f32,
    /// How new the arrival is, in `[0, 1]` (a tone change is fully new).
    pub novelty: f32,
}

impl EffectVector {
    /// The neutral effect (actions the semantics did not size: gestures without a bound event,
    /// interaction actions).
    pub const NEUTRAL: EffectVector = EffectVector {
        strength: 0.5,
        d_pressure: 0.0,
        d_dynamic: 0.0,
        d_density: 0,
        d_elevation: 0,
        compactness: 0.5,
        novelty: 0.0,
    };

    /// The effect of moving from semantic state `prev` to `next`.
    pub fn between(
        prev: &super::semantic::SemanticState,
        next: &super::semantic::SemanticState,
    ) -> EffectVector {
        use super::semantic::{Density, Elevation};
        let dens = |d: Density| match d {
            Density::Spacious => -1i8,
            Density::Normal => 0,
            Density::Compact => 1,
        };
        let elev = |e: Elevation| match e {
            Elevation::Flat => 0i8,
            Elevation::Raised => 1,
            Elevation::Overlay => 2,
        };
        let d_pressure = next.pressure() - prev.pressure();
        let d_dynamic = next.dynamic() - prev.dynamic();
        let d_density = dens(next.density) - dens(prev.density);
        let d_elevation = elev(next.elevation) - elev(prev.elevation);
        let arrival = 0.5 * next.dynamic() + 0.5 * next.pressure();
        let delta =
            (d_pressure.abs() + d_dynamic.abs() + 0.25 * d_density.unsigned_abs() as f32).min(1.0);
        let novelty = if next.tone != prev.tone { 1.0 } else { delta };
        EffectVector {
            strength: (0.5 * arrival + 0.5 * delta).clamp(0.0, 1.0),
            d_pressure,
            d_dynamic,
            d_density,
            d_elevation,
            compactness: (dens(next.density) as f32 + 1.0) / 2.0,
            novelty,
        }
    }
}

/// One musical action.
#[derive(Debug, Clone, PartialEq)]
pub struct MusicalAction {
    /// The action's id: always its index in [`ActionPlan::actions`]. The base plan
    /// ([`ActionPlan::build`]) is sorted by time before ids are assigned; actions the performance
    /// planner appends later ([`ActionPlan::push`]: calls, answers) keep creation order, so the
    /// vector is NOT chronological — iterate [`ActionPlan::chronological`] for time order.
    pub id: ActionId,
    pub cause: ActionCause,
    pub initiator: Agent,
    pub start_beat: f64,
    pub dur_beats: f64,
    pub kind: ActionKind,
    /// The beat the action points at (a push's downbeat, a pickup's arrival), if any.
    pub target_beat: Option<f64>,
    /// Who may respond to it.
    pub responders: Vec<Agent>,
    /// The semantic binding (index into the backbone timeline's bindings) it realizes, if any.
    pub binding: Option<usize>,
    /// The action this one answers, pays or resolves, if any.
    pub pays: Option<ActionId>,
    /// How much the action does (its semantic size).
    pub effect: EffectVector,
}

impl MusicalAction {
    /// One past the last beat.
    pub fn end_beat(&self) -> f64 {
        self.start_beat + self.dur_beats
    }
    /// Whether the action's window covers `beat`.
    pub fn covers(&self, beat: f64) -> bool {
        beat >= self.start_beat - 1e-6 && beat < self.end_beat() - 1e-6
    }
}

/// A morphism an event applied that has no action witness, with the reason.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Deferral {
    pub transition: usize,
    pub morphism: IntentMorphism,
    pub reason: &'static str,
}

/// A span deliberately left without action (declared stasis), so the diagnostics can tell an
/// intended still point from accidental inactivity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StasisSpan {
    pub start_beat: f64,
    pub end_beat: f64,
    pub reason: &'static str,
}

/// Which agents a piece assigns to each backbone gesture's action family — chosen ONCE per piece
/// (seeded), so a recurring gesture has a recognizable consequence every time it returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionFamilies {
    /// Who leads the band into a Lift (a pickup).
    pub lift_pickup: Agent,
    /// Who carries the fragment/reach inside a Lift.
    pub lift_reach: Agent,
    /// Who fills out of a Reset into the next attempt.
    pub reset_fill: Agent,
    /// Who re-enters first after the Deflect's break (the Open).
    pub open_reentry: Agent,
}

impl ActionFamilies {
    /// Choose the families for a piece.
    pub fn choose(lang: &MusicalLanguage, seed: u64) -> ActionFamilies {
        let mut rng = Rng::new(seed ^ 0xAC71_0F4A);
        if !lang.distributed_agency {
            return ActionFamilies {
                lift_pickup: Agent::Drums,
                lift_reach: Agent::Lead,
                reset_fill: Agent::Drums,
                open_reentry: Agent::Pad,
            };
        }
        let pick = |rng: &mut Rng, xs: &[Agent]| *rng.pick(xs).unwrap_or(&xs[0]);
        ActionFamilies {
            lift_pickup: pick(&mut rng, &[Agent::Bass, Agent::Drums, Agent::Keys]),
            lift_reach: pick(&mut rng, &[Agent::Lead, Agent::Bass, Agent::Keys]),
            reset_fill: pick(&mut rng, &[Agent::Drums, Agent::Bass]),
            open_reentry: pick(&mut rng, &[Agent::Keys, Agent::Pad, Agent::Lead]),
        }
    }
}

/// The whole-piece action plan.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ActionPlan {
    pub actions: Vec<MusicalAction>,
    pub deferred: Vec<Deferral>,
    pub stasis: Vec<StasisSpan>,
    /// The piece's gesture → player families (None for the null plan).
    pub families: Option<ActionFamilies>,
}

/// A lifted morphism's action: (kind, initiator, start, duration, target, responders).
type ActionSpec = (ActionKind, Agent, f64, f64, Option<f64>, Vec<Agent>);

/// Beats of lookahead a transition's surface-shaping action spans (two bars).
const SURFACE_WINDOW: f64 = 2.0 * BEATS_PER_BAR;

impl ActionPlan {
    /// An empty plan — the "actions disabled" calibration probe. The mood (scalar intent) is
    /// unchanged; only the verbs are gone.
    pub fn none() -> ActionPlan {
        ActionPlan::default()
    }

    /// Build the plan from the intent timeline (morphism lifting) and, when present, the backbone
    /// timeline (gesture actions). `total_beats` bounds every window.
    pub fn build(
        timeline: &IntentTimeline,
        backbone: Option<&BackboneTimeline>,
        lang: &MusicalLanguage,
        total_beats: f64,
        seed: u64,
    ) -> ActionPlan {
        let fam = ActionFamilies::choose(lang, seed);
        let mut plan = ActionPlan {
            families: Some(fam),
            ..ActionPlan::default()
        };
        let clamp = |b: f64| b.clamp(0.0, total_beats);

        // --- 1. Path-lift every applied morphism. ---
        for (ti, t) in timeline.transitions.iter().enumerate() {
            let at = clamp(t.at_beat);
            if at >= total_beats - 1e-6 {
                continue;
            }
            let binding =
                backbone.and_then(|bb| bb.bindings.iter().position(|b| b.transition == ti));
            let window_end = timeline
                .transitions
                .get(ti + 1)
                .map(|n| n.at_beat)
                .unwrap_or(total_beats)
                .min(at + SURFACE_WINDOW)
                .max(at + 1.0)
                .min(total_beats);
            let has_resolve = t.applied.contains(&IntentMorphism::Resolve);
            for &m in &t.applied {
                let cause = ActionCause::Morphism {
                    transition: ti,
                    morphism: m,
                };
                use IntentMorphism::*;
                let spec: Option<ActionSpec> = match m {
                    Prolong => None, // identity: nothing to witness, by definition
                    Prepare => Some((
                        ActionKind::Pickup,
                        fam.lift_pickup,
                        (at - 1.0).max(0.0),
                        1.0,
                        Some(at),
                        vec![Agent::Lead],
                    )),
                    Intensify => Some((
                        ActionKind::Push,
                        Agent::Ensemble,
                        (at - 0.5).max(0.0),
                        0.5,
                        Some(at),
                        vec![],
                    )),
                    Relax | Augment => Some((
                        ActionKind::Pullback,
                        Agent::Drums,
                        at,
                        window_end - at,
                        None,
                        vec![],
                    )),
                    Diminish => Some((
                        ActionKind::Accelerate,
                        Agent::Drums,
                        at,
                        window_end - at,
                        None,
                        vec![],
                    )),
                    Suspend => Some((
                        ActionKind::Hold,
                        Agent::Keys,
                        at,
                        2.0_f64.min(total_beats - at),
                        None,
                        vec![Agent::Lead],
                    )),
                    Pivot | Modulate => Some((
                        ActionKind::Tonicize,
                        Agent::Keys,
                        at,
                        BEATS_PER_BAR.min(total_beats - at),
                        Some(at),
                        vec![],
                    )),
                    Resolve => Some((
                        ActionKind::Resolve,
                        Agent::Lead,
                        at,
                        BEATS_PER_BAR.min(total_beats - at),
                        Some(at),
                        vec![Agent::Keys, Agent::Bass],
                    )),
                    Cadence => Some((ActionKind::Hit, Agent::Ensemble, at, 0.5, Some(at), vec![])),
                    Reharmonize => Some((
                        ActionKind::Reharmonize,
                        Agent::Keys,
                        at,
                        BEATS_PER_BAR.min(total_beats - at),
                        None,
                        vec![],
                    )),
                    FragmentMotif => Some((
                        ActionKind::Fragment,
                        fam.lift_reach,
                        at,
                        window_end - at,
                        None,
                        vec![Agent::Lead, Agent::Keys, Agent::Bass],
                    )),
                    SequenceMotif => {
                        // Honest deferral: no realizer states a sequence yet (Round VII emitted
                        // the action and "witnessed" it from the plan alone).
                        plan.deferred.push(Deferral {
                            transition: ti,
                            morphism: m,
                            reason: "no realizer sequences motif material yet",
                        });
                        None
                    }
                    Syncopate => Some((
                        ActionKind::Displace,
                        Agent::Ensemble,
                        at,
                        window_end - at,
                        None,
                        vec![],
                    )),
                    ThinTexture => Some((
                        ActionKind::Thin,
                        Agent::Pad,
                        at,
                        window_end - at,
                        None,
                        vec![],
                    )),
                    ThickenTexture => Some((
                        ActionKind::Thicken,
                        Agent::Pad,
                        at,
                        window_end - at,
                        None,
                        vec![],
                    )),
                };
                let Some((kind, initiator, start, dur, target, responders)) = spec else {
                    continue;
                };
                if kind == ActionKind::Pickup && target.is_some_and(|t| t < 1.0 - 1e-6) {
                    plan.deferred.push(Deferral {
                        transition: ti,
                        morphism: m,
                        reason: "prepares the very first downbeat: there is no time before it for a pickup",
                    });
                    continue;
                }
                if dur <= 1e-6 {
                    plan.deferred.push(Deferral {
                        transition: ti,
                        morphism: m,
                        reason: "fires at the very end of the piece: no time left to witness it",
                    });
                    continue;
                }
                let id = ActionId(plan.actions.len() as u32);
                // A cadence hit that coincides with a resolution pays that resolution.
                let pays = if kind == ActionKind::Hit && has_resolve {
                    plan.actions
                        .iter()
                        .rev()
                        .find(|a| {
                            a.kind == ActionKind::Resolve && (a.start_beat - start).abs() < 1e-6
                        })
                        .map(|a| a.id)
                } else {
                    None
                };
                plan.actions.push(MusicalAction {
                    id,
                    cause,
                    initiator,
                    start_beat: start,
                    dur_beats: dur,
                    kind,
                    target_beat: target,
                    responders,
                    binding,
                    pays,
                    effect: EffectVector::NEUTRAL,
                });
            }
        }

        // --- 2. Gesture actions: the spine's own verbs, the same family every cycle. ---
        if let Some(bb) = backbone {
            for (si, slot) in bb.slots.iter().enumerate() {
                let s = slot.start_beat();
                let e = slot.end_beat().min(total_beats);
                if e <= s + 1e-6 {
                    continue;
                }
                let cause = ActionCause::Gesture {
                    slot: si,
                    gesture: slot.gesture,
                };
                let mut push = |kind: ActionKind,
                                initiator: Agent,
                                start: f64,
                                dur: f64,
                                target: Option<f64>,
                                responders: Vec<Agent>| {
                    let id = ActionId(plan.actions.len() as u32);
                    plan.actions.push(MusicalAction {
                        id,
                        cause,
                        initiator,
                        start_beat: start.max(0.0),
                        dur_beats: dur,
                        kind,
                        target_beat: target,
                        responders,
                        binding: slot.binding,
                        pays: None,
                        effect: EffectVector::NEUTRAL,
                    });
                };
                let long = slot.bars >= 2;
                match slot.gesture {
                    HarmonicGesture::Lift => {
                        // Lead the band into the reach, then push into the miss's downbeat.
                        if s > 0.0 {
                            push(
                                ActionKind::Pickup,
                                fam.lift_pickup,
                                s - 1.0,
                                1.0,
                                Some(s),
                                vec![Agent::Lead, Agent::Keys],
                            );
                        }
                        if e < total_beats - 1e-6 {
                            push(
                                ActionKind::Push,
                                Agent::Ensemble,
                                e - 0.5,
                                0.5,
                                Some(e),
                                vec![],
                            );
                        }
                    }
                    HarmonicGesture::Deflect => {
                        // Everyone hits the deceptive arrival together; in the conversational
                        // language the band then drops out for a beat — the miss has a consequence.
                        push(ActionKind::Deflect, Agent::Ensemble, s, e - s, None, vec![]);
                        push(ActionKind::Hit, Agent::Ensemble, s, 0.5, Some(s), vec![]);
                        if lang.unison_figures && long {
                            push(
                                ActionKind::Break,
                                Agent::Ensemble,
                                s + 1.0,
                                1.0,
                                None,
                                vec![Agent::Lead, Agent::Bass],
                            );
                        }
                    }
                    HarmonicGesture::Open => {
                        push(
                            ActionKind::ReEntry,
                            fam.open_reentry,
                            s,
                            BEATS_PER_BAR.min(e - s),
                            Some(s),
                            vec![Agent::Lead],
                        );
                        if lang.unison_figures && long && slot.cycle > 0 {
                            // A short ensemble figure in the released window.
                            push(
                                ActionKind::Unison,
                                Agent::Ensemble,
                                e - BEATS_PER_BAR,
                                2.0,
                                None,
                                vec![],
                            );
                        }
                    }
                    HarmonicGesture::Reset => {
                        if e < total_beats - 1e-6 {
                            let fill_len = if long { 2.0 } else { 1.0 };
                            push(
                                ActionKind::Fill,
                                fam.reset_fill,
                                e - fill_len,
                                fill_len,
                                Some(e),
                                vec![Agent::Drums],
                            );
                        }
                    }
                }
            }
        }
        plan.actions.sort_by(|a, b| {
            a.start_beat
                .total_cmp(&b.start_beat)
                .then(a.kind.cmp(&b.kind))
        });
        // Re-number after sorting so the BASE plan's ids follow time; remap `pays` references.
        let old_ids: Vec<ActionId> = plan.actions.iter().map(|a| a.id).collect();
        for (i, a) in plan.actions.iter_mut().enumerate() {
            a.id = ActionId(i as u32);
        }
        for a in &mut plan.actions {
            if let Some(p) = a.pays {
                a.pays = old_ids
                    .iter()
                    .position(|&o| o == p)
                    .map(|i| ActionId(i as u32));
            }
        }
        plan
    }

    /// Actions whose window covers `beat`.
    pub fn at(&self, beat: f64) -> impl Iterator<Item = &MusicalAction> {
        self.actions.iter().filter(move |a| a.covers(beat))
    }

    /// Actions of `kind`.
    pub fn of_kind(&self, kind: ActionKind) -> impl Iterator<Item = &MusicalAction> {
        self.actions.iter().filter(move |a| a.kind == kind)
    }

    /// Whether `morphism` applied by transition `ti` has an action witness in this plan.
    pub fn witnesses(&self, ti: usize, morphism: IntentMorphism) -> bool {
        self.actions.iter().any(|a| {
            matches!(a.cause, ActionCause::Morphism { transition, morphism: m }
                if transition == ti && m == morphism)
        })
    }

    /// Append an action (the performance planner adds interaction actions), returning its id.
    /// Ids are creation ids (`id == index`); appended actions are not in time order.
    pub fn push(&mut self, mut a: MusicalAction) -> ActionId {
        a.id = ActionId(self.actions.len() as u32);
        let id = a.id;
        self.actions.push(a);
        id
    }

    /// Remove the actions `gone` (before anything outside the plan references ids), re-number
    /// the rest densely in their existing order, and remap `pays` (a reference to a removed action
    /// becomes `None`). Returns the old-id → new-id map.
    pub fn remove(&mut self, gone: &[ActionId]) -> impl Fn(ActionId) -> Option<ActionId> {
        let mut map: Vec<Option<ActionId>> = Vec::with_capacity(self.actions.len());
        let mut next = 0u32;
        for a in &self.actions {
            if gone.contains(&a.id) {
                map.push(None);
            } else {
                map.push(Some(ActionId(next)));
                next += 1;
            }
        }
        self.actions.retain(|a| !gone.contains(&a.id));
        let lookup = move |id: ActionId| map.get(id.index()).copied().flatten();
        for a in &mut self.actions {
            a.id = lookup(a.id).expect("kept actions have new ids");
            a.pays = a.pays.and_then(&lookup);
            if let ActionCause::Interaction { call } = a.cause {
                if let Some(c) = lookup(call) {
                    a.cause = ActionCause::Interaction { call: c };
                }
            }
        }
        lookup
    }

    /// The action with id `id`.
    pub fn get(&self, id: ActionId) -> Option<&MusicalAction> {
        self.actions.get(id.index()).filter(|a| a.id == id)
    }

    /// Every action in time order (start beat, then kind, then id) — the order a listener meets
    /// them, whatever order they were created in.
    pub fn chronological(&self) -> Vec<&MusicalAction> {
        let mut v: Vec<&MusicalAction> = self.actions.iter().collect();
        v.sort_by(|a, b| {
            a.start_beat
                .total_cmp(&b.start_beat)
                .then(a.kind.cmp(&b.kind))
                .then(a.id.cmp(&b.id))
        });
        v
    }

    /// A compact dump.
    pub fn dump(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "actions ({}), deferred ({}), declared stasis ({}):",
            self.actions.len(),
            self.deferred.len(),
            self.stasis.len()
        );
        if let Some(f) = &self.families {
            let _ = writeln!(
                s,
                "  semantic binding plan (fixed for the piece): reach→Lift = pickup by {} + reach by {}; \
                 miss→Deflect = ensemble hit + break; opening→Open = re-entry by {}; \
                 reset→Reset = fill by {} into the next attempt",
                f.lift_pickup.label(),
                f.lift_reach.label(),
                f.open_reentry.label(),
                f.reset_fill.label()
            );
        }
        for a in self.chronological() {
            let cause = match a.cause {
                ActionCause::Morphism {
                    transition,
                    morphism,
                } => format!("t{transition}:{}", morphism.label()),
                ActionCause::Gesture { slot, gesture } => format!("slot{slot}:{}", gesture.label()),
                ActionCause::Interaction { call } => format!("answers {call}"),
                ActionCause::Statement { phrase } => format!("statement@phrase{phrase}"),
            };
            let resp: Vec<&str> = a.responders.iter().map(|r| r.label()).collect();
            let _ = writeln!(
                s,
                "  a{:<3} {:>6.2}+{:<4.2} {:<11} by {:<8} <- {:<20} resp=[{}]{}",
                a.id.0,
                a.start_beat,
                a.dur_beats,
                a.kind.label(),
                a.initiator.label(),
                cause,
                resp.join(","),
                a.pays.map(|p| format!(" pays {p}")).unwrap_or_default()
            );
        }
        for d in &self.deferred {
            let _ = writeln!(
                s,
                "  deferred t{}:{} — {}",
                d.transition,
                d.morphism.label(),
                d.reason
            );
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::super::semantic::{deflected_lift_trace, demo_trace};
    use super::*;

    fn flagship() -> (IntentTimeline, BackboneTimeline) {
        let tl = IntentTimeline::walk(&deflected_lift_trace(120.0));
        let bb = BackboneTimeline::build(&tl, 30, &[0, 4, 8, 12, 16, 20, 22, 24, 28], 4, 4);
        (tl, bb)
    }

    #[test]
    fn every_live_morphism_is_witnessed_or_deferred() {
        for trace in [deflected_lift_trace(120.0), demo_trace(120.0)] {
            let tl = IntentTimeline::walk(&trace);
            let plan = ActionPlan::build(&tl, None, &MusicalLanguage::default(), 120.0, 3);
            for (ti, t) in tl.transitions.iter().enumerate() {
                for &m in &t.applied {
                    if m == IntentMorphism::Prolong {
                        continue;
                    }
                    let deferred = plan
                        .deferred
                        .iter()
                        .any(|d| d.transition == ti && d.morphism == m);
                    assert!(
                        plan.witnesses(ti, m) || deferred,
                        "t{ti} {} has neither a witness nor a deferral",
                        m.label()
                    );
                }
            }
        }
    }

    #[test]
    fn the_spine_generates_its_own_verbs_every_cycle() {
        let (tl, bb) = flagship();
        let plan = ActionPlan::build(&tl, Some(&bb), &MusicalLanguage::default(), 120.0, 3);
        let deflect_slots = bb
            .slots
            .iter()
            .filter(|s| s.gesture == HarmonicGesture::Deflect)
            .count();
        let hits_on_deflects = plan
            .of_kind(ActionKind::Hit)
            .filter(|a| {
                matches!(
                    a.cause,
                    ActionCause::Gesture {
                        gesture: HarmonicGesture::Deflect,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(hits_on_deflects, deflect_slots);
        assert!(plan.of_kind(ActionKind::Pickup).count() >= 2);
        assert!(plan.of_kind(ActionKind::Fill).count() >= 2);
        // The base plan's ids follow time and are dense.
        for (i, a) in plan.actions.iter().enumerate() {
            assert_eq!(a.id.index(), i);
        }
        for w in plan.actions.windows(2) {
            assert!(w[0].start_beat <= w[1].start_beat + 1e-9);
        }
    }

    #[test]
    fn families_are_stable_per_piece_and_the_null_plan_is_empty() {
        let lang = MusicalLanguage::default();
        assert_eq!(
            ActionFamilies::choose(&lang, 9),
            ActionFamilies::choose(&lang, 9)
        );
        assert!(ActionPlan::none().actions.is_empty());
    }
}
