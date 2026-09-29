//! Round XI: a vector of musical witnesses, independent of the Round X event grammar.
//! F is explicit about the intended regime; μ reads the written song, never its target label.
//! These are compositional affordances, not measured emotions or a model of enjoyment.

use super::meaning::Level;
use super::semantic::SemanticTrace;

/// An explicit composition request. Neither regime is a quality judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhenomenalRegime {
    /// Sustained preparation, recurring denial, and withheld closure: a negative control.
    SuspendedDeflection,
    /// A recurring rhythmic identity over tonic prolongation and confirmed returns.
    StablePropulsion,
}

/// Independent coordinates. No weighted total is defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhenomenalState {
    pub stability: Level,
    pub propulsion: Level,
    pub expectation: Level,
    pub surprise: Level,
    /// Written melodic register span; performance voicing/texture is outside this observer.
    pub openness: Level,
    pub familiarity: Level,
}

/// F(trace, regime), before grammar or notes exist. The explicit request selects the meaning;
/// the trace supplies duration. The legacy pressure-to-deflection interpretation remains R10.
#[derive(Debug, Clone, PartialEq)]
pub struct PhenomenalTarget {
    pub regime: PhenomenalRegime,
    pub total_beats: f64,
    pub state: PhenomenalState,
    /// Whether recurring expected home arrivals are requested as positive payoffs.
    pub confirm: bool,
}

impl PhenomenalTarget {
    /// One factorization per regime; a deflection slot never has to mean confirmation.
    pub fn grammar(&self) -> super::contract::CompositionGrammar {
        match self.regime {
            PhenomenalRegime::StablePropulsion => {
                super::contract::CompositionGrammar::PropulsiveReturn
            }
            PhenomenalRegime::SuspendedDeflection => {
                super::contract::CompositionGrammar::DeflectedLift
            }
        }
    }
    pub fn from_trace(trace: &SemanticTrace, regime: PhenomenalRegime) -> Self {
        use Level::*;
        let state = match regime {
            PhenomenalRegime::StablePropulsion => PhenomenalState {
                stability: High,
                propulsion: High,
                expectation: Low,
                surprise: Low,
                openness: High,
                familiarity: High,
            },
            PhenomenalRegime::SuspendedDeflection => PhenomenalState {
                stability: Low,
                propulsion: High,
                expectation: High,
                surprise: High,
                openness: High,
                familiarity: High,
            },
        };
        Self {
            regime,
            total_beats: if trace.total_beats.is_finite() && trace.total_beats > 0.0 {
                trace.total_beats
            } else {
                4.0
            },
            state,
            confirm: regime == PhenomenalRegime::StablePropulsion,
        }
    }

    /// Preparation derives from the requested expectation coordinate, never from a slot name.
    /// Low expectation permits no forced dominant; high expectation calls for a strong pointer.
    pub fn preparation(&self) -> Level {
        self.state.expectation
    }
}

/// Fulfilled expectation is an event with positive content, including learned plagal returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrival {
    /// No predicted arrival occurs at this point.
    None,
    /// A dominant or a previously learned neighboring motion arrives at home.
    Confirm,
    /// A dominant points home, but the next root withholds it.
    Withheld,
}

use super::backbone::lead_sheet;
use super::context::PullEvidence;
use super::meaning::{Family, ThemeRelation};
use super::song::SongMap;
use super::theory::Scale;

/// Raw evidence at a structural slot. Debt counts denied home arrivals since the last actual
/// home, plus a currently sounding dominant. A return pays that debt; a label cannot pay it.
#[derive(Debug, Clone, PartialEq)]
pub struct PhenomenalWitness {
    pub beats: f64,
    pub tonic_beats: f64,
    pub roots: Vec<i32>,
    pub peak_pull: f32,
    pub pointer_beats: f64,
    pub theme_attacks: usize,
    pub theme_beats: f32,
    pub register_span: i32,
    pub literal_statements: usize,
    pub confirms: usize,
    pub withheld: usize,
    pub unexpected_moves: usize,
    pub debt: usize,
}

/// μ's state at a structural slot, with evidence rather than an affect label.
#[derive(Debug, Clone, PartialEq)]
pub struct PhenomenalSlot {
    pub beat: f64,
    pub state: PhenomenalState,
    pub witness: PhenomenalWitness,
}

/// μ(SongMap), independent of the target, grammar name, world, language and performance.
#[derive(Debug, Clone, PartialEq)]
pub struct PhenomenalTrajectory {
    pub slots: Vec<PhenomenalSlot>,
    pub summary: PhenomenalState,
    pub tonic_fraction: f64,
    pub attacks_per_theme_beat: f32,
    pub pointer_fraction: f64,
    pub literal_statements: usize,
    pub confirms: usize,
    pub withheld: usize,
    pub unresolved_debt: usize,
}

fn band(value: f64, mid: f64, high: f64) -> Level {
    if value >= high {
        Level::High
    } else if value >= mid {
        Level::Mid
    } else {
        Level::Low
    }
}

impl PhenomenalTrajectory {
    /// Read only actual written chord content and audible theme sites. The register coordinate
    /// is scale-degree span, not the eventual instrument octave. Propulsion measures rhythmic
    /// motion in the statements, not tension, BPM, emotion, or rendered groove continuity.
    pub fn observe(song: &SongMap) -> Self {
        let region = Scale::new(0, song.frame);
        let end = song.plan.form.total_beats;
        let sheet = match (&song.plan.backbone, &song.harmonic) {
            (Some(bb), Some(hm)) => lead_sheet(bb, hm, song.frame),
            _ => Vec::new(),
        };
        let mut slots = Vec::new();
        let (mut debt, mut literal, mut seen_home) = (0usize, 0usize, false);
        let mut previous = None;
        let mut learned_returns = Vec::new();
        let (mut total_theme_beats, mut total_attacks, mut max_span) = (0.0f32, 0usize, 0i32);
        if let Some(bb) = &song.plan.backbone {
            for slot in bb.slots.iter().filter(|s| s.start_beat() < end) {
                let start = slot.start_beat();
                let stop = slot.end_beat().min(end);
                let mut w = PhenomenalWitness {
                    beats: stop - start,
                    tonic_beats: 0.0,
                    roots: Vec::new(),
                    peak_pull: 0.0,
                    pointer_beats: 0.0,
                    theme_attacks: 0,
                    theme_beats: 0.0,
                    register_span: 0,
                    literal_statements: 0,
                    confirms: 0,
                    withheld: 0,
                    unexpected_moves: 0,
                    debt: 0,
                };
                let mut pending = false;
                for span in sheet
                    .iter()
                    .filter(|s| s.start_beat >= start && s.start_beat < stop)
                {
                    let chord = span.chord;
                    let home = chord.root_pc == region.tonic_pc;
                    let pull = PullEvidence::of(&chord, region.tonic_pc);
                    let duration = (span.dur_beats as f64).min(stop - span.start_beat);
                    w.roots.push(chord.root_pc);
                    w.peak_pull = w.peak_pull.max(pull.strength());
                    pending = pull.is_dominant();
                    if pending {
                        w.pointer_beats += duration;
                    }
                    if home {
                        w.tonic_beats += duration;
                    }
                    if let Some(from) = previous {
                        let pointed = PullEvidence::of(&from, region.tonic_pc).is_dominant();
                        let changed = from.root_pc != chord.root_pc;
                        // A learned return predicts home from its previous non-home root. This
                        // evidence is learned from the chart itself, never from a Confirm label.
                        let learned = seen_home && learned_returns.contains(&from.root_pc);
                        let arrival = if changed && home && (pointed || learned) {
                            Arrival::Confirm
                        } else if changed && !home && (pointed || learned) {
                            Arrival::Withheld
                        } else {
                            Arrival::None
                        };
                        match arrival {
                            Arrival::Confirm => w.confirms += 1,
                            Arrival::Withheld => {
                                w.withheld += 1;
                                debt += 1;
                            }
                            Arrival::None => {}
                        }
                        if changed && !Family::of(&from, &chord, &region).is_familiar() {
                            w.unexpected_moves += 1;
                        }
                        if changed && home && !learned_returns.contains(&from.root_pc) {
                            learned_returns.push(from.root_pc);
                        }
                    }
                    if home {
                        seen_home = true;
                        debt = 0;
                    }
                    previous = Some(chord);
                }
                for site in &song.thematic.sites {
                    let Some(phrase) = song.plan.form.phrases.iter().find(|p| p.ix == site.phrase)
                    else {
                        continue;
                    };
                    if phrase.start_beat() < start
                        || phrase.start_beat() >= stop
                        || !song
                            .plan
                            .arrangement
                            .at(site.phrase as usize)
                            .lead
                            .is_audible()
                    {
                        continue;
                    }
                    let motif = &site.motif;
                    if ThemeRelation::of(&song.thematic.bank.identity, motif)
                        == ThemeRelation::Literal
                    {
                        literal += 1;
                    }
                    let span = motif.degrees.iter().max().copied().unwrap_or(0)
                        - motif.degrees.iter().min().copied().unwrap_or(0);
                    w.register_span = w.register_span.max(span);
                    w.theme_attacks += motif.len();
                    w.theme_beats += motif.total_beats();
                }
                w.literal_statements = literal;
                w.debt = debt + usize::from(pending);
                total_attacks += w.theme_attacks;
                total_theme_beats += w.theme_beats;
                max_span = max_span.max(w.register_span);
                let state = PhenomenalState {
                    stability: band(w.tonic_beats / w.beats.max(f64::EPSILON), 0.25, 0.6),
                    propulsion: band(
                        (w.theme_attacks as f32 / w.theme_beats.max(1.0)) as f64,
                        0.5,
                        1.0,
                    ),
                    expectation: band(w.peak_pull as f64, 0.5, 0.9),
                    surprise: band((w.withheld + w.unexpected_moves) as f64, 1.0, 2.0),
                    openness: band(w.register_span as f64, 4.0, 7.0),
                    familiarity: band(literal as f64, 1.0, 2.0),
                };
                slots.push(PhenomenalSlot {
                    beat: start,
                    state,
                    witness: w,
                });
            }
        }
        let beats: f64 = slots.iter().map(|s| s.witness.beats).sum();
        let tonic_fraction =
            slots.iter().map(|s| s.witness.tonic_beats).sum::<f64>() / beats.max(1.0);
        let pointer_fraction =
            slots.iter().map(|s| s.witness.pointer_beats).sum::<f64>() / beats.max(1.0);
        let attacks_per_theme_beat = total_attacks as f32 / total_theme_beats.max(1.0);
        let confirms = slots.iter().map(|s| s.witness.confirms).sum();
        let withheld: usize = slots.iter().map(|s| s.witness.withheld).sum();
        let unexpected: usize = slots.iter().map(|s| s.witness.unexpected_moves).sum();
        let summary = PhenomenalState {
            stability: band(tonic_fraction, 0.25, 0.6),
            propulsion: band(attacks_per_theme_beat as f64, 0.5, 1.0),
            expectation: slots
                .iter()
                .map(|s| s.state.expectation)
                .max()
                .unwrap_or(Level::Low),
            surprise: band((withheld + unexpected) as f64, 1.0, 2.0),
            openness: band(max_span as f64, 4.0, 7.0),
            familiarity: band(literal as f64, 1.0, 2.0),
        };
        Self {
            unresolved_debt: slots.last().map_or(0, |s| s.witness.debt),
            slots,
            summary,
            tonic_fraction,
            attacks_per_theme_beat,
            pointer_fraction,
            literal_statements: literal,
            confirms,
            withheld,
        }
    }

    /// Classify only measured evidence. Neither a requested label nor a grammar name votes.
    pub fn regime(&self) -> Option<PhenomenalRegime> {
        use Level::*;
        let s = self.summary;
        if !self.slots.is_empty()
            && s.stability == High
            && s.propulsion == High
            && s.familiarity == High
            && s.expectation <= Mid
            && s.surprise == Low
            && self.unresolved_debt == 0
            && self.confirms > 0
        {
            Some(PhenomenalRegime::StablePropulsion)
        } else if s.stability == Low
            && s.expectation == High
            && self.withheld >= 2
            && self.unresolved_debt > 0
        {
            Some(PhenomenalRegime::SuspendedDeflection)
        } else {
            None
        }
    }

    /// Approximate commutation = independently checked coordinate bounds, debt and positive
    /// payoff. A short fragment may fail recognition/confirmation; it is never silently passed.
    pub fn divergences(&self, target: &PhenomenalTarget) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.slots.is_empty() {
            out.push("no observed chart");
        }
        let beats: f64 = self.slots.iter().map(|s| s.witness.beats).sum();
        if (beats - target.total_beats).abs() > 1e-6 {
            out.push("duration");
        }
        let s = self.summary;
        for (name, heard, wanted) in [
            ("stability", s.stability, target.state.stability),
            ("propulsion", s.propulsion, target.state.propulsion),
            ("expectation", s.expectation, target.state.expectation),
            ("surprise", s.surprise, target.state.surprise),
            ("openness", s.openness, target.state.openness),
            ("familiarity", s.familiarity, target.state.familiarity),
        ] {
            if heard != wanted {
                out.push(name);
            }
        }
        if target.confirm && self.confirms == 0 {
            out.push("no confirmed expected return");
        }
        if target.confirm && self.unresolved_debt != 0 {
            out.push("unresolved harmonic debt");
        }
        if !target.confirm && (self.withheld < 2 || self.unresolved_debt == 0) {
            out.push("no recurring withheld closure");
        }
        out
    }

    /// Compact, inspectable vector and the raw witnesses at every structural slot.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut out = format!("phenomenal {:?}; tonic {:.3}; attacks/theme-beat {:.3}; pointer exposure {:.3}; literal {}; confirms {}; withheld {}; debt {}; classification {:?}\n",
            self.summary, self.tonic_fraction, self.attacks_per_theme_beat, self.pointer_fraction,
            self.literal_statements, self.confirms, self.withheld, self.unresolved_debt, self.regime());
        out.push_str(
            "beat stability propulsion expectation surprise openness familiarity debt witnesses\n",
        );
        for s in &self.slots {
            let x = s.state;
            let _ = writeln!(
                out,
                "{:.2} {:?} {:?} {:?} {:?} {:?} {:?} {} {:?}",
                s.beat,
                x.stability,
                x.propulsion,
                x.expectation,
                x.surprise,
                x.openness,
                x.familiarity,
                s.witness.debt,
                s.witness
            );
        }
        out
    }
}
