// Shared by the `field_cinema` example (`--live`) and by the Gate C tests via `#[path]`; std-only so
// it stays independently includable like the wire_instrument sibling modules.
#![allow(dead_code)]
//! The six-beat cinematic arc and its typed EFFECT CONTRACT (RXIII Checkpoint B / Gate C;
//! repaired in RXIV Gate A).
//!
//! The live film moves through six beats:
//!
//! ```text
//!   ESTABLISH → DISCOVER → INTERVENE → AWAIT → TRANSFORM → RESOLVE
//! ```
//!
//! # What TRANSFORM means — and what it does not
//!
//! RXIII gated TRANSFORM on `observed_peak >= 0.8 · target`. RXIV Gate A1 refuted that rule with an
//! executed counterexample: baseline 2.0, target 2.0, ACK, observed 2.0 — the source never changed,
//! yet the arc transformed (and on the real producer a no-op `set-amplitude 1.0` did the same). A
//! threshold on the *target* alone cannot tell "the edit landed" from "the value was already there".
//!
//! The repair separates five things that are not logically equivalent:
//!
//! 1. **a producer acknowledgment** — [`Verdict::Acknowledged`]: the producer *says* it will act;
//! 2. **a later observation** — an [`ObsSummary`] whose cursor is strictly newer than the baseline,
//!    in the same producer epoch ([`Evidence::Stale`] / [`Evidence::EpochBreak`] otherwise);
//! 3. **a difference from the baseline** — `|m − baseline| > tol` ([`Evidence::Unchanged`] otherwise);
//! 4. **consistency with the requested effect** — `|m − target| ≤ tol` ([`Evidence::Inconsistent`]
//!    otherwise);
//! 5. **causal attribution to the command** — NOT established by the arc.
//!
//! TRANSFORM is entered only on [`Evidence::Consistent`] = (1)∧(2)∧(3)∧(4). The arc never claims (5):
//! temporal order plus a consistent value is *evidence compatible with* the command having caused the
//! change, not a proof of mechanism. Attribution is established only where a matched counterfactual
//! exists — the deterministic reference producer, run as no-edit and edited histories at identical
//! model ticks (`tests/field_cinema_effect_contract.rs`).
//!
//! The tolerance is a **declared source property** ([`EffectContract`]): the largest change natural
//! evolution is declared to produce in the measured quantity between observations. The arc does not
//! take the declaration on faith: before an intervention it must have observed natural variation at
//! least once ([`Evidence::Uncalibrated`]), and if that observed variation already exceeds the
//! tolerance the quantity is not stationary enough to attribute anything ([`Evidence::NonStationary`]).
//! An observed calibration can refute stationarity; it cannot prove it.
//!
//! A request the contract cannot distinguish from no change — `|target − baseline| ≤ 2·tol`, e.g.
//! setting the amplitude to the value it already has — is [`Evidence::Indiscriminable`] and can never
//! transform, whatever the producer says.

/// Which beat the film is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Beat {
    /// The first authoritative field is on screen; the baseline is set.
    Establish,
    /// The viewer is inspecting/probing the field (and natural variation is being calibrated).
    Discover,
    /// A validated intervention has been issued.
    Intervene,
    /// The intervention was acknowledged; the film awaits authoritative evidence it landed.
    Await,
    /// An observation consistent with the requested effect has been evidenced.
    Transform,
    /// The film has resolved (terminal).
    Resolve,
}

/// A presentation-free summary of one observed field — the only field state the arc judges on.
/// `epoch`/`seq` are the producer's wire cursor (its own observation chronology); `peak_value` is the
/// measured quantity a set-amplitude intervention moves (`None` = no finite unmasked sample).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObsSummary {
    pub epoch: u32,
    pub seq: u64,
    pub tick: u64,
    pub peak_value: Option<f64>,
}

impl ObsSummary {
    pub fn new(epoch: u32, seq: u64, tick: u64, peak_value: Option<f64>) -> Self {
        Self {
            epoch,
            seq,
            tick,
            peak_value,
        }
    }

    /// The finite measurement, if any.
    pub fn measured(&self) -> Option<f64> {
        self.peak_value.filter(|v| v.is_finite())
    }
}

/// The producer's authoritative verdict on an intervention — exactly what the producer sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Acknowledged,
    Refused,
}

/// The declared measurement contract for an intervention on the peak quantity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectContract {
    /// Declared natural-variation bound, relative to the larger of |baseline| and |target|.
    pub rel_tol: f64,
    /// Absolute floor on the tolerance (so a near-zero scale still has a nonzero band).
    pub abs_tol: f64,
    /// In-session observations required before an intervention (>= 2: natural variation measured
    /// at least once).
    pub min_calibration: usize,
    /// Newer observations an acknowledged effect may take to be evidenced before the cycle closes as
    /// unevidenced (the film cannot AWAIT forever).
    pub evidence_window: u32,
}

impl EffectContract {
    /// The contract for the reference producer's amplitude-scaled bump. Its natural evolution moves
    /// the peak's POSITION; the peak SAMPLE varies only by grid discretization (measured ≤ 0.1% of
    /// amplitude between ticks — `tests/field_cinema_effect_contract.rs`), far inside 5%.
    pub const REFERENCE: Self = Self {
        rel_tol: 0.05,
        abs_tol: 1e-3,
        min_calibration: 2,
        evidence_window: 3,
    };

    /// The tolerance band for a change between `a` and `b`.
    pub fn tolerance(&self, a: f64, b: f64) -> f64 {
        self.abs_tol.max(self.rel_tol * a.abs().max(b.abs()))
    }
}

/// An armed expectation: the pre-command baseline (cursor + value), the requested target, and the
/// tolerance band derived from the contract.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpectedEffect {
    pub baseline: ObsSummary,
    pub baseline_value: f64,
    pub target: f64,
    pub tolerance: f64,
}

/// What the arc can say about an intervention. Only [`Evidence::Consistent`] admits TRANSFORM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    // --- decided at intervention: the edit cannot be evidenced at all ---
    /// The baseline measurement is missing or non-finite — no reference to measure change from.
    UnmeasurableBaseline,
    /// Fewer than `min_calibration` in-session observations before the intervention: natural
    /// variation was never measured.
    Uncalibrated,
    /// Observed pre-intervention variation already exceeded the declared tolerance.
    NonStationary,
    /// `|target − baseline| ≤ 2·tol` (or a non-finite target): no change could be distinguished.
    Indiscriminable,
    // --- per-observation classification of an evidenceable effect ---
    /// No newer observation has been classified yet.
    Pending,
    /// The observation is not strictly newer than the baseline (stale or repeated cursor).
    Stale,
    /// The observation comes from a different producer epoch (a restart voids the baseline).
    EpochBreak,
    /// The observation's measurement is missing or non-finite.
    Unmeasurable,
    /// `|m − baseline| ≤ tol`: no change observed.
    Unchanged,
    /// Changed beyond tolerance, but not to within tolerance of the target.
    Inconsistent,
    /// Changed beyond tolerance AND within tolerance of the target.
    Consistent,
}

impl Evidence {
    /// The only class that admits TRANSFORM.
    pub fn admits_transform(self) -> bool {
        matches!(self, Evidence::Consistent)
    }

    /// The edit was not evidenceable from the start (decided at intervention).
    pub fn is_unevidenceable(self) -> bool {
        matches!(
            self,
            Evidence::UnmeasurableBaseline
                | Evidence::Uncalibrated
                | Evidence::NonStationary
                | Evidence::Indiscriminable
        )
    }

    /// A short human label for readouts.
    pub fn label(self) -> &'static str {
        match self {
            Evidence::UnmeasurableBaseline => "baseline unmeasurable",
            Evidence::Uncalibrated => "uncalibrated (observe longer)",
            Evidence::NonStationary => "source not stationary",
            Evidence::Indiscriminable => "no-op: indistinguishable",
            Evidence::Pending => "awaiting observation",
            Evidence::Stale => "stale observation",
            Evidence::EpochBreak => "producer restarted",
            Evidence::Unmeasurable => "measurement missing",
            Evidence::Unchanged => "unchanged",
            Evidence::Inconsistent => "changed, not as requested",
            Evidence::Consistent => "consistent with request",
        }
    }
}

/// Classify one observation against an armed effect. Pure.
pub fn classify(effect: &ExpectedEffect, obs: &ObsSummary) -> Evidence {
    if obs.epoch != effect.baseline.epoch {
        return Evidence::EpochBreak;
    }
    if obs.seq <= effect.baseline.seq {
        return Evidence::Stale;
    }
    let Some(m) = obs.measured() else {
        return Evidence::Unmeasurable;
    };
    if (m - effect.baseline_value).abs() <= effect.tolerance {
        return Evidence::Unchanged;
    }
    if (m - effect.target).abs() <= effect.tolerance {
        Evidence::Consistent
    } else {
        Evidence::Inconsistent
    }
}

/// How one intervention cycle closed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CycleOutcome {
    /// A consistent observation was evidenced (the earned transformation).
    Transformed { observed: f64 },
    /// The producer refused; nothing was armed.
    Refused,
    /// Acknowledged, but no consistent observation arrived within the window (suppression, an
    /// inconsistent change, or an unevidenceable request); `last` is the final classification.
    Unevidenced { last: Evidence },
    /// The baseline was voided (producer restart) before evidence arrived.
    Void { reason: Evidence },
}

/// Natural-variation calibration over consecutive in-session observations taken while no effect was
/// pending.
#[derive(Debug, Clone, Copy)]
struct Calibration {
    count: usize,
    last: Option<ObsSummary>,
    drift_high_water: f64,
}

impl Calibration {
    fn empty() -> Self {
        Self {
            count: 0,
            last: None,
            drift_high_water: 0.0,
        }
    }

    fn seeded(o: ObsSummary) -> Self {
        Self {
            count: 1,
            last: Some(o),
            drift_high_water: 0.0,
        }
    }

    /// Fold an observation in; a different epoch restarts calibration, a stale cursor is ignored.
    fn fold(&mut self, o: ObsSummary) {
        match self.last {
            Some(prev) if prev.epoch != o.epoch => *self = Self::seeded(o),
            Some(prev) if o.seq <= prev.seq => {}
            Some(prev) => {
                if let (Some(a), Some(b)) = (prev.measured(), o.measured()) {
                    self.drift_high_water = self.drift_high_water.max((b - a).abs());
                }
                self.count += 1;
                self.last = Some(o);
            }
            None => *self = Self::seeded(o),
        }
    }
}

/// The cinematic arc: a deterministic state machine over the session's causal events.
#[derive(Debug, Clone)]
pub struct Arc {
    contract: EffectContract,
    beat: Beat,
    established: Option<ObsSummary>,
    calib: Calibration,
    /// The intervention's armed effect, or why it could not be armed.
    effect: Option<Result<ExpectedEffect, Evidence>>,
    /// The latest classification for the current (or last) cycle.
    evidence: Option<Evidence>,
    /// Newer observations classified since the ACK.
    window_used: u32,
    transforms: u32,
    last_outcome: Option<CycleOutcome>,
}

impl Default for Arc {
    fn default() -> Self {
        Self::new()
    }
}

impl Arc {
    pub fn new() -> Self {
        Self::with_contract(EffectContract::REFERENCE)
    }

    pub fn with_contract(contract: EffectContract) -> Self {
        Self {
            contract,
            beat: Beat::Establish,
            established: None,
            calib: Calibration::empty(),
            effect: None,
            evidence: None,
            window_used: 0,
            transforms: 0,
            last_outcome: None,
        }
    }

    pub fn beat(&self) -> Beat {
        self.beat
    }

    pub fn contract(&self) -> EffectContract {
        self.contract
    }

    /// Whether an earned transformation ever occurred (latched; survives RESOLVE).
    pub fn transformed(&self) -> bool {
        self.transforms > 0
    }

    /// How many cycles earned a transformation.
    pub fn transforms(&self) -> u32 {
        self.transforms
    }

    /// The ESTABLISH baseline peak (the first observation's measurement).
    pub fn baseline_peak(&self) -> Option<f64> {
        self.established.and_then(|o| o.peak_value)
    }

    /// Whether an acknowledged, evidenceable effect is still awaiting a consistent observation.
    pub fn awaiting(&self) -> bool {
        matches!(self.beat, Beat::Await) && matches!(self.effect, Some(Ok(_)))
    }

    /// The armed effect of the current cycle, if it could be armed.
    pub fn effect(&self) -> Option<ExpectedEffect> {
        self.effect.and_then(|e| e.ok())
    }

    /// The latest classification for the current (or last) cycle.
    pub fn evidence(&self) -> Option<Evidence> {
        self.evidence
    }

    /// How the last intervention cycle closed.
    pub fn last_outcome(&self) -> Option<CycleOutcome> {
        self.last_outcome
    }

    /// Observed natural variation (high-water) and the calibration sample count.
    pub fn calibration(&self) -> (usize, f64) {
        (self.calib.count, self.calib.drift_high_water)
    }

    /// Observe an authoritative field and return its classification if an effect is awaited.
    ///
    /// The first observation ESTABLISHes. While no intervention is pending the observation feeds the
    /// natural-variation calibration. While AWAITing an acknowledged effect it is classified; only
    /// [`Evidence::Consistent`] enters TRANSFORM.
    pub fn observe(&mut self, o: ObsSummary) -> Option<Evidence> {
        if self.established.is_none() {
            self.established = Some(o);
            self.calib = Calibration::seeded(o);
            self.beat = Beat::Establish;
            return None;
        }
        match self.beat {
            Beat::Establish | Beat::Discover | Beat::Transform => {
                self.calib.fold(o);
                None
            }
            // Between INTERVENE and the verdict the observation may or may not reflect the edit: it
            // neither calibrates nor evidences.
            Beat::Intervene | Beat::Resolve => None,
            Beat::Await => Some(self.judge(o)),
        }
    }

    fn judge(&mut self, o: ObsSummary) -> Evidence {
        let ev = match self.effect {
            Some(Ok(effect)) => classify(&effect, &o),
            Some(Err(why)) => why,
            None => Evidence::Pending,
        };
        // Stale observations do not consume the window (they carry no new information).
        if ev != Evidence::Stale {
            self.window_used += 1;
        }
        self.evidence = Some(ev);
        if ev.admits_transform() {
            self.transforms += 1;
            self.last_outcome = Some(CycleOutcome::Transformed {
                observed: o.measured().unwrap_or(f64::NAN),
            });
            self.effect = None;
            self.beat = Beat::Transform;
            // The post-transform state is the new natural baseline.
            self.calib = Calibration::seeded(o);
        } else if ev == Evidence::EpochBreak {
            self.last_outcome = Some(CycleOutcome::Void { reason: ev });
            self.effect = None;
            self.beat = Beat::Discover;
            self.calib = Calibration::seeded(o);
        } else if self.window_used >= self.contract.evidence_window || ev.is_unevidenceable() {
            self.last_outcome = Some(CycleOutcome::Unevidenced { last: ev });
            self.effect = None;
            self.beat = Beat::Discover;
            self.calib.fold(o);
        }
        ev
    }

    /// The viewer inspects/probes — DISCOVER, available before an intervention.
    pub fn discover(&mut self) {
        if matches!(self.beat, Beat::Establish | Beat::Transform) {
            self.beat = Beat::Discover;
        }
    }

    /// Issue a validated intervention requesting `target` — INTERVENE. Snapshots the latest
    /// calibrated observation as the baseline and arms the expected effect, returning
    /// [`Evidence::Pending`] when the effect is evidenceable, or the reason it is not. The beat moves
    /// to INTERVENE either way (the presenter may still send the command; an unevidenceable effect can
    /// simply never TRANSFORM).
    pub fn intervene(&mut self, target: f64) -> Evidence {
        if !matches!(
            self.beat,
            Beat::Establish | Beat::Discover | Beat::Transform
        ) {
            return self.evidence.unwrap_or(Evidence::Pending);
        }
        let armed = self.arm(target);
        let ev = match armed {
            Ok(_) => Evidence::Pending,
            Err(why) => why,
        };
        self.effect = Some(armed);
        self.evidence = Some(ev);
        self.window_used = 0;
        self.beat = Beat::Intervene;
        ev
    }

    fn arm(&self, target: f64) -> Result<ExpectedEffect, Evidence> {
        let Some(base) = self.calib.last else {
            return Err(Evidence::UnmeasurableBaseline);
        };
        let Some(b) = base.measured() else {
            return Err(Evidence::UnmeasurableBaseline);
        };
        if self.calib.count < self.contract.min_calibration {
            return Err(Evidence::Uncalibrated);
        }
        if !target.is_finite() {
            return Err(Evidence::Indiscriminable);
        }
        let tol = self.contract.tolerance(b, target);
        if self.calib.drift_high_water > tol {
            return Err(Evidence::NonStationary);
        }
        if (target - b).abs() <= 2.0 * tol {
            return Err(Evidence::Indiscriminable);
        }
        Ok(ExpectedEffect {
            baseline: base,
            baseline_value: b,
            target,
            tolerance: tol,
        })
    }

    /// A producer verdict lands. ACK moves to AWAIT (where only a consistent observation can
    /// TRANSFORM); REFUSED disarms and returns to DISCOVER.
    pub fn verdict(&mut self, v: Verdict) {
        if !matches!(self.beat, Beat::Intervene) {
            return;
        }
        match v {
            Verdict::Acknowledged => self.beat = Beat::Await,
            Verdict::Refused => {
                self.effect = None;
                self.last_outcome = Some(CycleOutcome::Refused);
                self.beat = Beat::Discover;
            }
        }
    }

    /// Close the film. RESOLVE is terminal; an effect still awaited closes as unevidenced, and the
    /// transform latch is preserved.
    pub fn resolve(&mut self) {
        if matches!(self.beat, Beat::Await | Beat::Intervene) {
            self.last_outcome = Some(CycleOutcome::Unevidenced {
                last: self.evidence.unwrap_or(Evidence::Pending),
            });
            self.effect = None;
        }
        self.beat = Beat::Resolve;
    }
}
