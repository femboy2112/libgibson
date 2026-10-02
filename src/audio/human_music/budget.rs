//! **Complexity budget** — one shared allowance of simultaneous information per bar, planned
//! BEFORE realization and consumed by every player (Round VIIb).
//!
//! Round VII had `EnsembleBar.budget`, but only the keys spent it; the bass and drums followed
//! their own density heuristics and the lead reserved nothing, so a dense lead, dense keys, a
//! counter-bass and a fill could all pile up in one bar. Here:
//! - [`spend_of`] is ONE interpretable measure (weighted onsets: a lead note costs more than a
//!   hat; a chord costs more than a single note; a connector, an answer, a figure or a
//!   counterline costs a little more than a plain note; a unison — one idea shared — less);
//! - [`allocate`] plans each bar: the lead's planned statements are RESERVED first (they exist
//!   before anybody plays), then planned answers and figures, then the remainder is shared out
//!   to keys / bass / drums / pad by who is in front. A named ensemble action (a unison, a fill,
//!   a hit or push) licenses a [`Burst`] for its bar;
//! - the realizers consume their allowance (keys stabs, bass extra onsets, drum ghosts);
//! - [`ComplexityReport::measure`] compares planned and realized spend per bar and counts
//!   violations: realized spend over the bar's total (plus any licensed burst).
//!
//! Units: weighted events per bar. Not a quality score.

use super::action::{ActionKind, Agent};
use super::ensemble::{seat_ix, STAGE_AGENTS};
use super::form::BEATS_PER_BAR;
use super::ids::ActionId;
use super::performance::PerformancePlan;
use super::score::{DrumVoice, Role, Score};

/// A licensed burst: a named ensemble action lets the bar exceed its total.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Burst {
    pub action: ActionId,
    pub kind: ActionKind,
    /// Extra units the burst allows on top of the bar's total.
    pub extra: f32,
}

/// One bar's allocation (units per bar, indexed like [`STAGE_AGENTS`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComplexityAllocation {
    pub bar: u32,
    /// The bar's total allowance (before any burst).
    pub total: f32,
    /// What the plan already committed before realization: the lead's statements, the planned
    /// answers and figures.
    pub reserved: [f32; 5],
    /// Each player's allowance (reserved + its share of the remainder).
    pub allowance: [f32; 5],
    pub burst: Option<Burst>,
}

impl ComplexityAllocation {
    /// `agent`'s allowance.
    pub fn of(&self, agent: Agent) -> f32 {
        seat_ix(agent).map(|i| self.allowance[i]).unwrap_or(0.0)
    }
    /// The total including a licensed burst.
    pub fn ceiling(&self) -> f32 {
        self.total + self.burst.map(|b| b.extra).unwrap_or(0.0)
    }
}

/// The weight of one event of `agent` (a pitched note tagged `tag`, or a drum voice).
pub fn spend_of(agent: Agent, tag: &str, drum: Option<DrumVoice>, voices: usize) -> f32 {
    let base = match (agent, drum) {
        (Agent::Drums, Some(DrumVoice::Kick)) | (Agent::Drums, Some(DrumVoice::Snare)) => 0.4,
        (Agent::Drums, Some(DrumVoice::ClosedHat)) => 0.1,
        (Agent::Drums, Some(DrumVoice::OpenHat)) => 0.15,
        (Agent::Drums, _) => 0.2,
        (Agent::Lead, _) => 1.0,
        (Agent::Keys, _) | (Agent::Bass, _) => 0.8,
        (Agent::Pad, _) => 0.3,
        (Agent::Ensemble, _) => 0.0,
    };
    let tag_mult = match tag {
        "ghost" => 0.5,
        "answer" | "figure" | "quote" | "counter" | "fill" | "connector" => 1.25,
        "unison" => 0.5,
        _ => 1.0,
    };
    base * tag_mult * (1.0 + 0.25 * voices.saturating_sub(1) as f32)
}

/// Realized spend per bar per player: `[bar][STAGE_AGENTS slot]`.
pub fn realized(score: &Score, bars: usize) -> Vec<[f32; 5]> {
    let mut v = vec![[0f32; 5]; bars];
    let bar_of =
        |t: f64| ((t / BEATS_PER_BAR).floor().max(0.0) as usize).min(bars.saturating_sub(1));
    // Pitched: group simultaneous onsets of one role into one event with N voices.
    let mut groups: Vec<(Role, i64, &'static str, usize, bool)> = Vec::new();
    for n in &score.notes {
        let key = (n.start_beat * 64.0).round() as i64;
        let connector = n.role == Role::Lead && n.function.is_some_and(|f| !f.is_consonant());
        match groups.iter_mut().find(|g| g.0 == n.role && g.1 == key) {
            Some(g) => g.3 += 1,
            None => groups.push((n.role, key, n.prov.role_note, 1, connector)),
        }
    }
    for (role, key, tag, voices, connector) in groups {
        let agent = match role {
            Role::Lead => Agent::Lead,
            Role::Keys => Agent::Keys,
            Role::Pad => Agent::Pad,
            Role::Bass => Agent::Bass,
        };
        let tag = if connector { "connector" } else { tag };
        let Some(i) = seat_ix(agent) else { continue };
        v[bar_of(key as f64 / 64.0)][i] += spend_of(agent, tag, None, voices);
    }
    let drum_ix = seat_ix(Agent::Drums).expect("drums have a seat");
    for d in &score.drums {
        let tag = d.prov.groove_variation.unwrap_or("");
        v[bar_of(d.start_beat + 0.01)][drum_ix] += spend_of(Agent::Drums, tag, Some(d.voice), 1);
    }
    v
}

/// Plan every bar's allocation for `perf` (the performance's lead statements, answers and
/// figures are the reservations; `lang.complexity_budget` and the bar's kinetic target set the
/// total).
pub fn allocate(perf: &PerformancePlan) -> Vec<ComplexityAllocation> {
    let bars = perf.ensemble.len();
    let mut reserved = vec![[0f32; 5]; bars];
    let bar_of =
        |t: f64| ((t / BEATS_PER_BAR).floor().max(0.0) as usize).min(bars.saturating_sub(1));
    let lead = seat_ix(Agent::Lead).expect("lead seat");
    for st in &perf.statements {
        let m = perf.material(st.material);
        for e in &m.events {
            reserved[bar_of(m.start_beat + e.onset)][lead] +=
                spend_of(Agent::Lead, "melody", None, 1);
        }
    }
    for i in &perf.interactions {
        let Some(r) = i.response else { continue };
        let (Some(mid), Some(ix)) = (r.material, seat_ix(r.responder)) else {
            continue;
        };
        let m = perf.material(mid);
        for e in &m.events {
            reserved[bar_of(r.start_beat + e.onset)][ix] +=
                spend_of(r.responder, "answer", None, 1);
        }
    }
    for m in perf
        .materials
        .iter()
        .filter(|m| matches!(m.source, super::material::MaterialSource::Figure { .. }))
    {
        let Some(ix) = seat_ix(m.owner) else { continue };
        for e in &m.events {
            reserved[bar_of(m.start_beat + e.onset)][ix] += spend_of(m.owner, "figure", None, 1);
        }
    }
    perf.ensemble
        .iter()
        .enumerate()
        .map(|(b, eb)| {
            let (s, e) = (b as f64 * BEATS_PER_BAR, (b + 1) as f64 * BEATS_PER_BAR);
            // The bar's total: the language's appetite scaled by the kinetic target, and grown
            // with the backbone cycle (the compressed later cycle is planned to be more urgent).
            let total = perf.language.complexity_budget
                * 1.6
                * (0.7 + 0.5 * eb.kinetic)
                * (1.0 + 0.12 * eb.cycle as f32);
            let burst = perf
                .actions
                .actions
                .iter()
                .filter(|a| a.start_beat < e - 1e-6 && a.end_beat() > s + 1e-6)
                .filter_map(|a| {
                    let extra = match a.kind {
                        ActionKind::Unison | ActionKind::Fill => 0.6 * total,
                        ActionKind::Hit | ActionKind::Push => 0.25 * total,
                        _ => return None,
                    };
                    Some(Burst {
                        action: a.id,
                        kind: a.kind,
                        extra,
                    })
                })
                .max_by(|a, b| a.extra.total_cmp(&b.extra));
            let res = reserved[b];
            let committed: f32 = res.iter().sum();
            let remainder = (total - committed).max(0.0);
            // The remainder goes to the accompanists, the one in front first.
            let weights: [f32; 5] = std::array::from_fn(|i| {
                let a = STAGE_AGENTS[i];
                if a == Agent::Lead || !perf.on_stage(a, s) {
                    0.0
                } else if a == eb.foreground {
                    1.5
                } else {
                    match a {
                        Agent::Drums => 1.2,
                        Agent::Pad => 0.35,
                        _ => 1.0,
                    }
                }
            });
            let wsum: f32 = weights.iter().sum::<f32>().max(1e-6);
            let allowance: [f32; 5] =
                std::array::from_fn(|i| res[i] + remainder * weights[i] / wsum);
            ComplexityAllocation {
                bar: b as u32,
                total,
                reserved: res,
                allowance,
                burst,
            }
        })
        .collect()
}

/// Planned vs realized spend, bar by bar.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexityReport {
    pub allocations: Vec<ComplexityAllocation>,
    pub realized: Vec<[f32; 5]>,
    /// Bars whose realized total exceeds the allocation's ceiling (total + licensed burst).
    pub violations: Vec<u32>,
    /// Bars that needed their licensed burst (realized over total, within the ceiling).
    pub licensed: Vec<u32>,
    /// The largest `realized - ceiling` (≤ 0 when nothing overspent).
    pub max_overspend: f32,
}

impl ComplexityReport {
    /// Measure `score` against `perf`'s allocation.
    pub fn measure(perf: &PerformancePlan, score: &Score) -> ComplexityReport {
        let allocations = allocate(perf);
        Self::against(allocations, score)
    }

    /// Measure `score` against explicit allocations (the mutation probes use this).
    pub fn against(allocations: Vec<ComplexityAllocation>, score: &Score) -> ComplexityReport {
        let realized = realized(score, allocations.len());
        let mut violations = Vec::new();
        let mut licensed = Vec::new();
        let mut max_overspend = f32::NEG_INFINITY;
        for (a, r) in allocations.iter().zip(&realized) {
            let spent: f32 = r.iter().sum();
            max_overspend = max_overspend.max(spent - a.ceiling());
            if spent > a.ceiling() + 1e-3 {
                violations.push(a.bar);
            } else if spent > a.total + 1e-3 {
                licensed.push(a.bar);
            }
        }
        ComplexityReport {
            allocations,
            realized,
            violations,
            licensed,
            max_overspend,
        }
    }

    /// A compact report: totals, violations, and per bar `planned total / realized`.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "complexity budget (weighted onsets per bar — NOT a quality score):"
        );
        let _ = writeln!(
            s,
            "  violations={} {:?} licensed_bursts={} max_overspend={:+.1}",
            self.violations.len(),
            self.violations,
            self.licensed.len(),
            self.max_overspend
        );
        let mut line = String::from("  bar total/realized:");
        for (a, r) in self.allocations.iter().zip(&self.realized) {
            let _ = write!(
                line,
                " {}:{:.0}/{:.0}",
                a.bar,
                a.total,
                r.iter().sum::<f32>()
            );
        }
        let _ = writeln!(s, "{line}");
        s
    }
}
