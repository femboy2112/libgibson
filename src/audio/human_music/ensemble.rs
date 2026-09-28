//! **Ensemble** — the per-bar plan of the band: who is in front, each player's mode, the
//! complexity budget and the kinetic target (Round VII; split out of [`super::performance`] in
//! Round VIIb).

use super::action::{ActionKind, ActionPlan, Agent, MusicalAction};
use super::backbone::{BackboneTimeline, HarmonicGesture};
use super::form::BEATS_PER_BAR;
use super::ids::ActionId;
use super::interaction::{Interaction, LeadStatement, Transform};
use super::language::MusicalLanguage;
use super::plan::{ArrangementRole, CompositionPlan};

/// Pad behaviour for a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadMode {
    /// Hold the full voicing.
    Sustain,
    /// A partial shell (guide tones only).
    Shell,
    /// Hold only the tones common with the previous harmony.
    CommonToneCarry,
    /// A swelling entry.
    Swell,
    /// A wide upper-structure layer (colour tones high).
    UpperStructure,
    /// Out.
    Silent,
}

/// Keys behaviour for a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeysMode {
    /// Syncopated shell comping on the accent grid, around the lead.
    Comp,
    /// Answering a call in a response window.
    Answer,
    /// Short stabs on the planned hits.
    Stab,
    /// Holding a voicing (a suspension).
    Sustain,
    /// Leaving space.
    Space,
}

/// Bass behaviour for a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BassMode {
    /// Roots/fifths on the structural steps.
    Foundation,
    /// One held pitch under changing harmony.
    Pedal,
    /// A stepwise line into the next harmony.
    Walk,
    /// A melodic counterline in the lead's gaps.
    Counter,
    /// Quoting motif material (a call or an answer).
    Quote,
}

/// Drums behaviour for a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumsMode {
    Pocket,
    HalfTime,
    DoubleTime,
    Break,
    Fill,
}

/// The ensemble's per-bar plan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnsembleBar {
    pub bar: u32,
    pub gesture: Option<HarmonicGesture>,
    pub cycle: u32,
    pub foreground: Agent,
    pub pad: PadMode,
    pub keys: KeysMode,
    pub bass: BassMode,
    pub drums: DrumsMode,
    /// Simultaneous-information budget for the bar (capped per-role onsets per beat, summed).
    pub budget: f32,
    /// The kinetic target in `[0, 1]`: forward motion independent of loudness.
    pub kinetic: f32,
}

/// The players who can sit on the stage (the pitched roles plus the kit).
pub const STAGE_AGENTS: [Agent; 5] = [
    Agent::Lead,
    Agent::Keys,
    Agent::Pad,
    Agent::Bass,
    Agent::Drums,
];

/// The stage slot of `agent`, if it is a single player.
pub fn seat_ix(agent: Agent) -> Option<usize> {
    STAGE_AGENTS.iter().position(|&a| a == agent)
}

/// One player's seat in one bar: whether it plays, at what level, in which arrangement role.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seat {
    pub on: bool,
    pub gain: f32,
    pub role: ArrangementRole,
}

/// A window in which an otherwise off-stage player is ADMITTED to perform one action (a pickup
/// from someone about to enter, a fill, an answer) — the action is the authority for its window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageWindow {
    pub agent: Agent,
    pub start_beat: f64,
    pub end_beat: f64,
    pub action: ActionId,
}

/// **The single orchestration authority** (Round VIIb). The phrase-level
/// [`super::plan::ArrangementPlan`] is only the coarse envelope it is seeded from; actions are
/// reconciled with it BEFORE realization (admitted, recast or rejected — see
/// [`super::performance::Admission`]); every realizer asks the stage who plays and how loud, and
/// nothing is deleted after the performance has been realized.
#[derive(Debug, Clone, PartialEq)]
pub struct Stage {
    /// Per bar, per [`STAGE_AGENTS`] slot.
    pub seats: Vec<[Seat; 5]>,
    pub windows: Vec<StageWindow>,
}

impl Stage {
    /// Seed the stage from the arrangement envelope: each bar takes its phrase's roles.
    pub fn from_arrangement(plan: &CompositionPlan) -> Stage {
        let seats = (0..plan.form.total_bars)
            .map(|bar| {
                let beat = bar as f64 * BEATS_PER_BAR;
                let arr = plan.arrangement.at(plan.form.phrase_at(beat).ix as usize);
                let roles = [arr.lead, arr.keys, arr.pad, arr.bass, arr.drums];
                roles.map(|role| Seat {
                    on: role.is_audible(),
                    gain: role.gain(),
                    role,
                })
            })
            .collect();
        Stage {
            seats,
            windows: Vec::new(),
        }
    }

    fn bar_of(&self, beat: f64) -> usize {
        ((beat / BEATS_PER_BAR).floor().max(0.0) as usize).min(self.seats.len().saturating_sub(1))
    }

    /// `agent`'s seat in the bar holding `beat` (an ensemble "seat" is never on).
    pub fn seat(&self, agent: Agent, beat: f64) -> Seat {
        match (seat_ix(agent), self.seats.get(self.bar_of(beat))) {
            (Some(i), Some(bar)) => bar[i],
            _ => Seat {
                on: false,
                gain: 0.0,
                role: ArrangementRole::Silent,
            },
        }
    }

    /// The admitted window covering `agent` at `beat`, if any.
    pub fn window(&self, agent: Agent, beat: f64) -> Option<&StageWindow> {
        self.windows
            .iter()
            .find(|w| w.agent == agent && beat >= w.start_beat - 1e-6 && beat < w.end_beat - 1e-6)
    }

    /// Whether `agent` plays at `beat` (its seat is on, or an action admitted it).
    pub fn on_stage(&self, agent: Agent, beat: f64) -> bool {
        self.seat(agent, beat).on || self.window(agent, beat).is_some()
    }

    /// Whether `agent` plays throughout `[a, b)`.
    pub fn on_stage_span(&self, agent: Agent, a: f64, b: f64) -> bool {
        let mut t = a;
        while t < b - 1e-6 {
            if !self.on_stage(agent, t) {
                return false;
            }
            t += 0.25;
        }
        true
    }

    /// The level `agent` plays at `beat`: its seat's gain, the punctuation level inside an admitted
    /// window, silence otherwise.
    pub fn level(&self, agent: Agent, beat: f64) -> f32 {
        let seat = self.seat(agent, beat);
        if seat.on {
            seat.gain
        } else if self.window(agent, beat).is_some() {
            ArrangementRole::Punctuation.gain()
        } else {
            0.0
        }
    }

    /// Admit `agent` for `[start, end)` to perform `action`.
    pub fn admit(&mut self, agent: Agent, start_beat: f64, end_beat: f64, action: ActionId) {
        if seat_ix(agent).is_some() && end_beat > start_beat + 1e-6 {
            self.windows.push(StageWindow {
                agent,
                start_beat,
                end_beat,
                action,
            });
        }
    }

    /// The players on stage throughout `[a, b)` among `among`.
    pub fn present(&self, among: &[Agent], a: f64, b: f64) -> Vec<Agent> {
        among
            .iter()
            .copied()
            .filter(|&g| self.on_stage_span(g, a, b.max(a + 0.25)))
            .collect()
    }
}

/// Plan each bar's ensemble: foreground, player modes, budget and kinetic target.
pub(super) fn plan_ensemble(
    plan: &CompositionPlan,
    actions: &ActionPlan,
    statements: &[LeadStatement],
    interactions: &[Interaction],
    lang: &MusicalLanguage,
    stage: &Stage,
) -> Vec<EnsembleBar> {
    let bb: Option<&BackboneTimeline> = plan.backbone.as_ref();
    let total_bars = plan.form.total_bars;
    let mut out = Vec::with_capacity(total_bars as usize);
    for bar in 0..total_bars {
        let s = bar as f64 * BEATS_PER_BAR;
        let e = s + BEATS_PER_BAR;
        let in_bar = |a: &MusicalAction| a.start_beat < e - 1e-6 && a.end_beat() > s + 1e-6;
        let has = |k: ActionKind| actions.actions.iter().any(|a| a.kind == k && in_bar(a));
        let slot = bb.and_then(|b| b.slot_at_bar(bar));
        let gesture = slot.map(|s| s.gesture);
        let cycle = slot.map(|s| s.cycle).unwrap_or(0);
        let lead_busy: f64 = statements
            .iter()
            .map(|st| (st.end_beat().min(e) - st.start_beat.max(s)).max(0.0))
            .sum();
        let responder = interactions.iter().find_map(|i| {
            i.response.as_ref().and_then(|r| {
                (r.transform != Transform::Silence
                    && r.start_beat < e - 1e-6
                    && r.start_beat + r.dur_beats > s + 1e-6)
                    .then_some(r.responder)
            })
        });
        // The foreground is somebody who is actually on stage (Round VII defaulted to the keys even
        // in bars where the arrangement had the keys out).
        let on = |g: Agent| stage.on_stage(g, s);
        let foreground = if lead_busy >= 2.0 && on(Agent::Lead) {
            Agent::Lead
        } else if let Some(r) = responder.filter(|&r| {
            stage.on_stage_span(r, s, e)
                || stage
                    .windows
                    .iter()
                    .any(|w| w.agent == r && w.start_beat < e && w.end_beat > s)
        }) {
            r
        } else {
            [
                Agent::Keys,
                Agent::Bass,
                Agent::Pad,
                Agent::Drums,
                Agent::Lead,
            ]
            .into_iter()
            .find(|&g| on(g))
            .unwrap_or(Agent::Keys)
        };
        let keys = if responder == Some(Agent::Keys) {
            KeysMode::Answer
        } else if has(ActionKind::Hold) {
            KeysMode::Sustain
        } else if has(ActionKind::Hit) || has(ActionKind::Push) {
            KeysMode::Stab
        } else if lead_busy >= 3.0 && lang.complexity_budget < 8.0 {
            KeysMode::Space
        } else {
            KeysMode::Comp
        };
        let pad = if has(ActionKind::Thin) {
            PadMode::Silent
        } else if has(ActionKind::Thicken) {
            PadMode::UpperStructure
        } else {
            match gesture {
                Some(HarmonicGesture::Lift) => PadMode::Shell,
                Some(HarmonicGesture::Deflect) => PadMode::CommonToneCarry,
                Some(HarmonicGesture::Open) => {
                    if has(ActionKind::ReEntry) {
                        PadMode::Swell
                    } else {
                        PadMode::UpperStructure
                    }
                }
                Some(HarmonicGesture::Reset) => PadMode::Sustain,
                None => PadMode::Sustain,
            }
        };
        let bass = if responder == Some(Agent::Bass)
            || interactions.iter().any(|i| {
                i.call.initiator == Agent::Bass && i.call.start_beat < e && i.call.end_beat > s
            }) {
            BassMode::Quote
        } else {
            match gesture {
                Some(HarmonicGesture::Lift) => BassMode::Walk,
                Some(HarmonicGesture::Deflect) => BassMode::Pedal,
                Some(HarmonicGesture::Open) if lead_busy < 2.0 => BassMode::Counter,
                _ => BassMode::Foundation,
            }
        };
        let drums = if has(ActionKind::Break) {
            DrumsMode::Break
        } else if actions
            .actions
            .iter()
            .any(|a| a.kind == ActionKind::Fill && a.start_beat >= s && a.start_beat < e)
        {
            DrumsMode::Fill
        } else if has(ActionKind::Pullback) {
            DrumsMode::HalfTime
        } else if has(ActionKind::Accelerate) {
            DrumsMode::DoubleTime
        } else {
            DrumsMode::Pocket
        };
        let base = match gesture {
            Some(HarmonicGesture::Lift) => 0.6,
            Some(HarmonicGesture::Deflect) => 0.75,
            Some(HarmonicGesture::Open) => 0.45,
            Some(HarmonicGesture::Reset) => 0.5,
            None => 0.5,
        };
        let acts = actions.actions.iter().filter(|a| in_bar(a)).count() as f32;
        let kinetic = (base * (1.0 + 0.2 * cycle as f32) + 0.04 * acts).clamp(0.0, 1.0);
        out.push(EnsembleBar {
            bar,
            gesture,
            cycle,
            foreground,
            pad,
            keys,
            bass,
            drums,
            budget: lang.complexity_budget * (0.7 + 0.5 * kinetic),
            kinetic,
        });
    }
    out
}
