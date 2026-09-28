//! **Ensemble** — the per-bar plan of the band: who is in front, each player's mode, the
//! complexity budget and the kinetic target (Round VII; split out of [`super::performance`] in
//! Round VIIb).

use super::action::{ActionKind, ActionPlan, Agent, MusicalAction};
use super::backbone::{BackboneTimeline, HarmonicGesture};
use super::form::BEATS_PER_BAR;
use super::interaction::{Interaction, LeadStatement, Transform};
use super::language::MusicalLanguage;
use super::plan::CompositionPlan;

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

/// Plan each bar's ensemble: foreground, player modes, budget and kinetic target.
pub(super) fn plan_ensemble(
    plan: &CompositionPlan,
    actions: &ActionPlan,
    statements: &[LeadStatement],
    interactions: &[Interaction],
    lang: &MusicalLanguage,
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
        let foreground = if lead_busy >= 2.0 {
            Agent::Lead
        } else if let Some(r) = responder {
            r
        } else {
            Agent::Keys
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
