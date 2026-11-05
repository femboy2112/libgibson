// Shared by the `field_cinema` example (`--live`) and by the Gate C tests via `#[path]`; std-only so
// it stays independently includable like the wire_instrument sibling modules.
#![allow(dead_code)]
//! The six-beat cinematic arc and its causal-honesty gate (RXIII Checkpoint B / Gate C2–C3).
//!
//! The live film moves through six beats:
//!
//! ```text
//!   ESTABLISH → DISCOVER → INTERVENE → AWAIT → TRANSFORM → RESOLVE
//! ```
//!
//! The arc advances on SEMANTIC events — an observed authoritative field, a probe, a validated
//! intervention, a producer verdict, the close. The load-bearing rule is the **TRANSFORM gate**: the
//! film enters TRANSFORM *only* when an authoritative field change is actually OBSERVED that matches
//! an accepted intervention. Never on an ACK alone; never on ordinary model motion. A producer
//! verdict does not prove a source-state change (Gate C3) — so an acknowledged-but-suppressed edit,
//! whose model never moves, must stay in AWAIT and never reach TRANSFORM, even though it was acked.
//!
//! This encodes the causal grammar deterministically and presentation-free, so the four-history
//! oracle can drive it headlessly and the `--live` executable can drive it from the real producer.

/// Which beat the film is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Beat {
    /// The first authoritative field is on screen; the baseline is set.
    Establish,
    /// The viewer is inspecting/probing the field.
    Discover,
    /// A validated intervention has been issued.
    Intervene,
    /// The intervention was acknowledged; the film awaits authoritative evidence it landed.
    Await,
    /// Authoritative evidence of the accepted change has been observed — the earned transformation.
    Transform,
    /// The film has resolved (terminal).
    Resolve,
}

/// A presentation-free summary of one observed field — the only field state the arc judges on,
/// derived from the cinema `FieldObservation`. The peak VALUE (amplitude) is what a set-amplitude
/// intervention moves; ordinary model travel moves the peak's position, not its value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObsSummary {
    pub tick: u64,
    pub peak_value: Option<f64>,
}

impl ObsSummary {
    pub fn new(tick: u64, peak_value: Option<f64>) -> Self {
        Self { tick, peak_value }
    }
}

/// The producer's authoritative verdict on an intervention — exactly what the producer sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Acknowledged,
    Refused,
}

/// Fraction of an accepted intervention's target peak an observation must reach to count as the
/// authoritative transformation landing. Below it, the change has not (yet) been evidenced.
pub const TRANSFORM_EVIDENCE_FRACTION: f64 = 0.8;

/// The cinematic arc: a deterministic state machine over the session's causal events.
#[derive(Debug, Clone)]
pub struct Arc {
    beat: Beat,
    established: bool,
    baseline_peak: Option<f64>,
    /// An accepted intervention's target peak, armed on ACK and disarmed when evidenced or refused.
    pending_target: Option<f64>,
    /// Latched true the moment TRANSFORM is first entered, so it survives RESOLVE.
    transformed: bool,
}

impl Default for Arc {
    fn default() -> Self {
        Self::new()
    }
}

impl Arc {
    pub fn new() -> Self {
        Self {
            beat: Beat::Establish,
            established: false,
            baseline_peak: None,
            pending_target: None,
            transformed: false,
        }
    }

    pub fn beat(&self) -> Beat {
        self.beat
    }

    /// Whether the earned transformation ever occurred (latched; survives RESOLVE).
    pub fn transformed(&self) -> bool {
        self.transformed
    }

    pub fn baseline_peak(&self) -> Option<f64> {
        self.baseline_peak
    }

    /// Whether an accepted intervention is still awaiting authoritative evidence.
    pub fn awaiting(&self) -> bool {
        self.pending_target.is_some()
    }

    /// Observe an authoritative field. The first observation ESTABLISHes and records the baseline.
    /// A later observation whose peak reaches a pending accepted target is the TRANSFORM — the ONLY
    /// path into that beat. An ack with no observed change never reaches it (suppression stays in
    /// AWAIT), and ordinary motion with no pending target never reaches it either.
    pub fn observe(&mut self, o: ObsSummary) {
        if !self.established {
            self.established = true;
            self.baseline_peak = o.peak_value;
            self.beat = Beat::Establish;
            return;
        }
        if let (Some(target), Some(pv)) = (self.pending_target, o.peak_value) {
            if pv >= TRANSFORM_EVIDENCE_FRACTION * target {
                self.pending_target = None;
                self.transformed = true;
                self.beat = Beat::Transform;
            }
            // else: the authoritative change has not been evidenced yet — remain in AWAIT.
        }
    }

    /// The viewer inspects/probes — DISCOVER, available before an intervention.
    pub fn discover(&mut self) {
        if matches!(self.beat, Beat::Establish) {
            self.beat = Beat::Discover;
        }
    }

    /// Issue a validated intervention — INTERVENE (only from an establishing/discovering beat).
    pub fn intervene(&mut self) {
        if matches!(self.beat, Beat::Establish | Beat::Discover) {
            self.beat = Beat::Intervene;
        }
    }

    /// A producer verdict lands. ACK arms the transform target and AWAITs the authoritative change;
    /// REFUSED arms nothing and returns to DISCOVER (no transformation is possible). Suppression
    /// needs no special case: ACK arms the target, but if the model never moves, `observe` never
    /// sees the evidence and the arc never leaves AWAIT.
    pub fn verdict(&mut self, v: Verdict, target: f64) {
        if !matches!(self.beat, Beat::Intervene) {
            return;
        }
        match v {
            Verdict::Acknowledged => {
                self.pending_target = Some(target);
                self.beat = Beat::Await;
            }
            Verdict::Refused => {
                self.pending_target = None;
                self.beat = Beat::Discover;
            }
        }
    }

    /// Close the film. RESOLVE is terminal; the `transformed` latch is preserved.
    pub fn resolve(&mut self) {
        self.beat = Beat::Resolve;
    }
}
