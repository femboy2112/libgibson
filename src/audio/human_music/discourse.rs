//! The **discourse layer** — where the piece is *going*, and what each phrase owes the future.
//!
//! Round II gave the music identity coherence: a [`super::contract::CoherenceContract`] declares
//! what must remain recognizable (motif, groove, harmonic contour). That is necessary but not
//! sufficient — a piece can satisfy every identity/coherence check and still be *directionless*,
//! a sequence of locally correct musical sentences that never add up to an argument. That was the
//! Round II listening result: "a grammatically correct song whose meaning has been scrambled."
//!
//! The discourse layer answers the questions the contract cannot: *what did this piece establish?
//! what is currently unresolved? what is this phrase preparing? what does the final return resolve
//! that earlier arrivals did not?* It sits between the causal [`super::timeline::IntentTimeline`]
//! and the realizers:
//!
//! - [`MusicalThesis`] — the piece-level home the rest of the music departs from and returns to.
//! - [`DiscourseRole`] — a phrase's rhetorical job (Establish … Culminate … Answer … Dissolve),
//!   a *consequence* of the intent trajectory, not a parity/index label.
//! - [`Closure`] — how strongly a phrase is allowed to close, so not every sentence ends with a
//!   full stop (the Round II over-cadencing defect).
//! - [`ObligationLedger`] — cross-phrase debts (a departure awaiting return, a withheld cadence, a
//!   motif question awaiting its answer): the generalization of Round II's local V/x obligation to
//!   the whole form.
//!
//! Crucially this layer **separates the culmination from its discharge**. The point of maximum
//! pressure is not automatically the release — often it is the event whose consequences *require* a
//! later release. So [`DiscourseRole::Culminate`] carries maximum commitment and stays unresolved;
//! a later [`DiscourseRole::Answer`] pays the debt it opened.

use super::action::{ActionCause, ActionKind, ActionPlan, Agent, MusicalAction};
use super::contract::{CoherenceAnchor, CoherenceContract, CompositionGrammar};
use super::ids::{ActionId, ObligationId};
use super::plan::{FormGraph, SectionFamily};
use super::timeline::{IntentSpan, IntentTimeline};

/// A phrase's rhetorical job in the piece-level argument. Derived from the intent trajectory and
/// the piece's salient anchors, not from a positional label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscourseRole {
    /// State the thesis: clear identity, the home the piece will depart from.
    Establish,
    /// Near-identity return of established material.
    Restate,
    /// Controlled move away from home (transposition / reharmonization / rhythmic alteration).
    Depart,
    /// Increase pressure toward a goal (register, subdivision, sequence).
    Intensify,
    /// Create a genuine outstanding obligation — pose, do not answer.
    Question,
    /// Explicitly refuse an expected closure.
    Withhold,
    /// Maximum pressure / commitment — usually NOT resolved; the peak, not the discharge.
    Culminate,
    /// Pay a named earlier obligation — the discharge the culmination made necessary.
    Answer,
    /// Recognizably restore home material after the answer.
    Return,
    /// Terminal closure or intentional evaporation.
    Dissolve,
}

impl DiscourseRole {
    /// A short provenance/dump label.
    pub fn label(self) -> &'static str {
        match self {
            DiscourseRole::Establish => "establish",
            DiscourseRole::Restate => "restate",
            DiscourseRole::Depart => "depart",
            DiscourseRole::Intensify => "intensify",
            DiscourseRole::Question => "question",
            DiscourseRole::Withhold => "withhold",
            DiscourseRole::Culminate => "culminate",
            DiscourseRole::Answer => "answer",
            DiscourseRole::Return => "return",
            DiscourseRole::Dissolve => "dissolve",
        }
    }

    /// The tension direction this role implies: `+1` rising/holding-high, `-1` releasing, `0` at
    /// rest. Diagnostics compare this against the realized trajectory to catch sign contradictions.
    pub fn tension_direction(self) -> i8 {
        match self {
            DiscourseRole::Establish | DiscourseRole::Restate => 0,
            DiscourseRole::Depart
            | DiscourseRole::Intensify
            | DiscourseRole::Question
            | DiscourseRole::Withhold
            | DiscourseRole::Culminate => 1,
            DiscourseRole::Answer | DiscourseRole::Return | DiscourseRole::Dissolve => -1,
        }
    }

    /// Whether this role refers back to earlier material (and so must carry a `refers_to`).
    pub fn is_referential(self) -> bool {
        matches!(
            self,
            DiscourseRole::Restate | DiscourseRole::Return | DiscourseRole::Answer
        )
    }
}

/// How strongly a phrase is permitted to close — the closure hierarchy that replaces "every phrase
/// ends on a prepared tonic cadence".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closure {
    /// No cadence: end on a continuation chord, the line stays open.
    Open,
    /// Half cadence: end on the dominant, maximum unresolved pull.
    Half,
    /// The expected resolution is evaded (a suspension left hanging).
    Deferred,
    /// Deceptive resolution (V→vi rather than V→I).
    Deceptive,
    /// A weak arrival: tonic via plagal motion or inversion, no strong dominant preparation.
    Weak,
    /// A full prepared cadence (…predominant → dominant → tonic): decisive arrival.
    Strong,
}

impl Closure {
    /// A short provenance/dump label.
    pub fn label(self) -> &'static str {
        match self {
            Closure::Open => "open",
            Closure::Half => "half",
            Closure::Deferred => "deferred",
            Closure::Deceptive => "deceptive",
            Closure::Weak => "weak",
            Closure::Strong => "strong",
        }
    }

    /// Whether this closure is a decisive terminal arrival (a full stop). Used to catch a strong
    /// closure landing while a major obligation is still open (premature resolution).
    pub fn is_terminal(self) -> bool {
        matches!(self, Closure::Strong)
    }
}

/// The piece-level home the discourse departs from and returns to. Not "the first four bars
/// forever" — the referent later material states, distorts, suspends, recovers, or dissolves. The
/// concrete motif identity lives in the realizer's motif bank; this is the abstract home the
/// planner reasons about.
#[derive(Debug, Clone)]
pub struct MusicalThesis {
    /// Home energy `[0,1]`.
    pub home_energy: f32,
    /// Home tension `[0,1]` (the settled, at-rest instability).
    pub home_tension: f32,
    /// Home register bias `[0,1]`.
    pub home_register: f32,
    /// Home density `[0,1]`.
    pub home_density: f32,
    /// The phrase index that first states the thesis.
    pub established_by: u32,
    /// The load-bearing identity axes carried over from the contract.
    pub anchors: Vec<CoherenceAnchor>,
}

/// A kind of cross-phrase debt the discourse opens and must (or intentionally may not) settle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObligationKind {
    /// The harmony left home and owes a return.
    HarmonicDeparture,
    /// A cadence was set up and withheld; a real arrival is owed.
    SuspendedCadence,
    /// A motif was posed incompletely (a question) and owes its completion.
    MotifQuestion,
    /// The register ascended and owes a descent / homecoming.
    RegisterAscent,
    /// The groove was stripped back (the kit reduced to a bare backbone) and owes a restoration.
    /// Opened where `groove.rs` really strips the kit: the first phrase of a Withhold/Question run.
    GrooveDestabilization,
}

impl ObligationKind {
    /// A short provenance/dump label.
    pub fn label(self) -> &'static str {
        match self {
            ObligationKind::HarmonicDeparture => "harmonic-departure",
            ObligationKind::SuspendedCadence => "suspended-cadence",
            ObligationKind::MotifQuestion => "motif-question",
            ObligationKind::RegisterAscent => "register-ascent",
            ObligationKind::GrooveDestabilization => "groove-destabilization",
        }
    }
}

/// How a debt was discharged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettleHow {
    /// The debt was paid in kind: the question answered, the cadence arrived, home regained.
    Paid,
    /// The debt was acknowledged and turned aside, not paid: the motif question is dropped into a
    /// return or a dissolve instead of being answered. Settled, but honestly labelled.
    Deflected,
}

impl SettleHow {
    /// A short dump label.
    pub fn label(self) -> &'static str {
        match self {
            SettleHow::Paid => "paid",
            SettleHow::Deflected => "deflected",
        }
    }
}

/// The receipt for a settled debt: which phrase discharged it, how, and (once bound by
/// [`bind_settlement_witnesses`]) the concrete action that makes the discharge audible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settlement {
    /// The phrase that settled the debt.
    pub by_phrase: u32,
    /// Paid or deflected.
    pub how: SettleHow,
    /// The action inside the settling phrase that musically discharges the kind, if one exists.
    pub witness: Option<ActionId>,
}

/// Where a debt stands at a given phrase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObligationStatus {
    /// Open, and still inside its deadline (or legitimately deferred to the end).
    Pending,
    /// Settled on or before its deadline.
    Settled(SettleHow),
    /// Settled, but after its deadline — the debt was paid, the promise was not kept.
    Late,
    /// Unsettled past its deadline (as of the phrase asked about; a later settlement turns an
    /// overdue debt `Late`, so at the end of the piece `Abandoned` means never settled at all).
    Abandoned,
}

/// Why an attempted [`ObligationLedger::settle`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettleError {
    /// No obligation with this id is in the ledger.
    Unknown(ObligationId),
    /// The debt already has a settlement.
    AlreadySettled { id: ObligationId, by_phrase: u32 },
    /// The settling phrase does not come strictly after the phrase that opened the debt.
    NotYetOpened {
        id: ObligationId,
        source_phrase: u32,
        by_phrase: u32,
    },
    /// The settling role cannot discharge this kind of debt (or cannot discharge it the way the
    /// caller claimed — a role that can only deflect may not be recorded as paying).
    IncompatibleKind {
        id: ObligationId,
        kind: ObligationKind,
        role: DiscourseRole,
        how: SettleHow,
    },
}

/// Whether a phrase's rhetorical `role` strips the kit back — mirrors the rule in `groove.rs`
/// (`Withhold | Question` → a stark kick+snare backbone).
fn strips_groove(role: DiscourseRole) -> bool {
    matches!(role, DiscourseRole::Withhold | DiscourseRole::Question)
}

/// The settlement table: how (if at all) a phrase in `role` can lawfully settle a debt of `kind`.
///
/// - **MotifQuestion**: an `Answer` pays it; a `Return` or `Dissolve` can only deflect it (the
///   question is dropped, not answered).
/// - **SuspendedCadence**: an `Answer` or a `Return` pays it with a real arrival. A `Dissolve` does
///   *not*: the coda's full stop is the end of the piece, not the discharge of a withheld arrival —
///   letting it pay would let every intentionally unresolved arc balance its books in the last bar.
/// - **HarmonicDeparture**: any homecoming — `Return`, `Restate` or `Dissolve`.
/// - **RegisterAscent**: a descent or homecoming — `Answer`, `Return` or `Dissolve`. Not
///   `Culminate`: the peak is the top of the ascent, not the way down the kind owes.
/// - **GrooveDestabilization**: any phrase that runs the full kit again (anything but a
///   `Withhold`/`Question`).
pub fn compatible(kind: ObligationKind, role: DiscourseRole) -> Option<SettleHow> {
    use DiscourseRole as R;
    use ObligationKind as K;
    match (kind, role) {
        (K::MotifQuestion, R::Answer) => Some(SettleHow::Paid),
        (K::MotifQuestion, R::Return | R::Dissolve) => Some(SettleHow::Deflected),
        (K::SuspendedCadence, R::Answer | R::Return) => Some(SettleHow::Paid),
        (K::HarmonicDeparture, R::Return | R::Restate | R::Dissolve) => Some(SettleHow::Paid),
        (K::RegisterAscent, R::Answer | R::Return | R::Dissolve) => Some(SettleHow::Paid),
        (K::GrooveDestabilization, r) if !strips_groove(r) => Some(SettleHow::Paid),
        _ => None,
    }
}

/// One cross-phrase debt: opened by `source_phrase`, due by `deadline`, and (once settled)
/// carrying its [`Settlement`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Obligation {
    /// Stable id within the ledger (its index in [`ObligationLedger::obligations`]).
    pub id: ObligationId,
    /// What kind of debt.
    pub kind: ObligationKind,
    /// The phrase that opened it.
    pub source_phrase: u32,
    /// The last phrase that may settle it on time — always strictly after `source_phrase`.
    /// `None` only when nothing follows the source (the debt was opened in the final phrase).
    pub deadline: Option<u32>,
    /// How load-bearing the debt is `[0,1]`.
    pub strength: f32,
    /// Whether it may cross phrases (even ones that could settle it) up to its deadline, and — when
    /// nothing at all follows its source — be left open at the end without counting as abandoned.
    /// It may never outlive a deadline it has.
    pub deferrable: bool,
    /// How it was settled, if it was.
    pub settlement: Option<Settlement>,
}

impl Obligation {
    /// Whether this debt is still open (never settled).
    pub fn is_open(&self) -> bool {
        self.settlement.is_none()
    }

    /// The phrase that settled it, if any.
    pub fn resolved_by(&self) -> Option<u32> {
        self.settlement.map(|s| s.by_phrase)
    }

    /// Where the debt stands at phrase `now`, in a piece of `end` phrases. Pass `now = end` for the
    /// final verdict.
    fn status(&self, now: u32, end: u32) -> ObligationStatus {
        if let Some(s) = self.settlement.filter(|s| s.by_phrase <= now) {
            return if self.deadline.is_some_and(|d| s.by_phrase > d) {
                ObligationStatus::Late
            } else {
                ObligationStatus::Settled(s.how)
            };
        }
        let overdue = match self.deadline {
            Some(d) => now > d,
            None => now >= end && !self.deferrable,
        };
        if overdue {
            ObligationStatus::Abandoned
        } else {
            ObligationStatus::Pending
        }
    }
}

/// The full set of cross-phrase debts a piece opened, and how each was settled. The ledger — not
/// the per-phrase goals — owns the opens/settles relation, which is many-to-many: one phrase may
/// open several debts (a Question poses a motif question *and* strips the groove) and settle
/// several (an Answer discharges its whole cycle).
#[derive(Debug, Clone, Default)]
pub struct ObligationLedger {
    /// Every debt, indexed by its [`ObligationId`].
    pub obligations: Vec<Obligation>,
    /// Phrases in the piece — the end against which the final status is judged.
    pub phrases: u32,
}

impl ObligationLedger {
    /// An empty ledger for a piece of `phrases` phrases.
    pub fn new(phrases: u32) -> ObligationLedger {
        ObligationLedger {
            obligations: Vec::new(),
            phrases,
        }
    }

    /// Open a debt of `kind` at `source_phrase`, returning its id.
    pub fn open_debt(
        &mut self,
        kind: ObligationKind,
        source_phrase: u32,
        deadline: Option<u32>,
        strength: f32,
        deferrable: bool,
    ) -> ObligationId {
        let id = ObligationId(self.obligations.len() as u32);
        self.obligations.push(Obligation {
            id,
            kind,
            source_phrase,
            deadline,
            strength,
            deferrable,
            settlement: None,
        });
        id
    }

    /// The obligation with id `id`.
    pub fn get(&self, id: ObligationId) -> Option<&Obligation> {
        self.obligations.get(id.index()).filter(|o| o.id == id)
    }

    /// Settle the named debt `id` at phrase `by_phrase`, whose rhetorical role is `role`, as `how`.
    /// Refuses a debt that does not exist, is already settled, has not been opened yet, or that
    /// `role` cannot lawfully settle `how` (see [`compatible`]; claiming `Deflected` where `Paid` is
    /// lawful is allowed — understating is not lying).
    pub fn settle(
        &mut self,
        id: ObligationId,
        by_phrase: u32,
        role: DiscourseRole,
        how: SettleHow,
    ) -> Result<(), SettleError> {
        let o = self
            .obligations
            .get_mut(id.index())
            .filter(|o| o.id == id)
            .ok_or(SettleError::Unknown(id))?;
        if let Some(s) = o.settlement {
            return Err(SettleError::AlreadySettled {
                id,
                by_phrase: s.by_phrase,
            });
        }
        if by_phrase <= o.source_phrase {
            return Err(SettleError::NotYetOpened {
                id,
                source_phrase: o.source_phrase,
                by_phrase,
            });
        }
        let lawful = compatible(o.kind, role);
        let ok = matches!(
            (lawful, how),
            (Some(SettleHow::Paid), _) | (Some(SettleHow::Deflected), SettleHow::Deflected)
        );
        if !ok {
            return Err(SettleError::IncompatibleKind {
                id,
                kind: o.kind,
                role,
                how,
            });
        }
        o.settlement = Some(Settlement {
            by_phrase,
            how,
            witness: None,
        });
        Ok(())
    }

    /// Where debt `id` stands at phrase `now_phrase` (`None` for an unknown id). Use
    /// [`ObligationLedger::final_status`] for the end-of-piece verdict.
    pub fn status_at(&self, id: ObligationId, now_phrase: u32) -> Option<ObligationStatus> {
        self.get(id).map(|o| o.status(now_phrase, self.phrases))
    }

    /// Where debt `id` stands once the piece is over.
    pub fn final_status(&self, id: ObligationId) -> Option<ObligationStatus> {
        self.status_at(id, self.phrases)
    }

    fn count_final(&self, want: ObligationStatus) -> usize {
        self.obligations
            .iter()
            .filter(|o| o.status(self.phrases, self.phrases) == want)
            .count()
    }

    /// Debts still open at the end of the piece.
    pub fn open(&self) -> impl Iterator<Item = &Obligation> {
        self.obligations.iter().filter(|o| o.is_open())
    }

    /// Debts opened by phrase `phrase`.
    pub fn created_by(&self, phrase: u32) -> impl Iterator<Item = &Obligation> {
        self.obligations
            .iter()
            .filter(move |o| o.source_phrase == phrase)
    }

    /// Debts settled by phrase `phrase`.
    pub fn settled_by(&self, phrase: u32) -> impl Iterator<Item = &Obligation> {
        self.obligations
            .iter()
            .filter(move |o| o.resolved_by() == Some(phrase))
    }

    /// Count of debts never settled that outlived their deadline (or, with no deadline, were not
    /// deferrable) — the number diagnostics want at zero for a piece that resolves what it raised.
    pub fn abandoned_count(&self) -> usize {
        self.count_final(ObligationStatus::Abandoned)
    }

    /// Count of debts settled after their deadline.
    pub fn late_count(&self) -> usize {
        self.count_final(ObligationStatus::Late)
    }

    /// Count of debts actually settled (on time or late).
    pub fn resolved_count(&self) -> usize {
        self.obligations
            .iter()
            .filter(|o| o.settlement.is_some())
            .count()
    }

    /// Settlements with no bound action witness — a discharge the plan claims but no concrete
    /// action inside the settling phrase makes audible.
    pub fn unwitnessed_settlements(&self) -> impl Iterator<Item = &Obligation> {
        self.obligations
            .iter()
            .filter(|o| o.settlement.is_some_and(|s| s.witness.is_none()))
    }
}

/// Whether `a` musically discharges a debt of `kind`:
///
/// - SuspendedCadence / HarmonicDeparture — a `Resolve`, or a cadence `Hit` that pays a `Resolve`.
/// - MotifQuestion — an `Answer`, or a lead statement `Call` that some `Answer` answers.
/// - GrooveDestabilization — a `ReEntry`, or a `Fill` aimed at a `ReEntry`.
/// - RegisterAscent — nothing yet: no action family carries a register homecoming, so such a
///   settlement stays honestly unwitnessed rather than borrowing an unrelated action.
fn discharges(kind: ObligationKind, a: &MusicalAction, plan: &ActionPlan) -> bool {
    match kind {
        ObligationKind::SuspendedCadence | ObligationKind::HarmonicDeparture => {
            a.kind == ActionKind::Resolve
                || (a.kind == ActionKind::Hit
                    && a.pays
                        .and_then(|p| plan.get(p))
                        .is_some_and(|p| p.kind == ActionKind::Resolve))
        }
        ObligationKind::MotifQuestion => {
            a.kind == ActionKind::Answer
                || (a.kind == ActionKind::Call
                    && a.initiator == Agent::Lead
                    && matches!(a.cause, ActionCause::Statement { .. })
                    && plan
                        .of_kind(ActionKind::Answer)
                        .any(|b| b.pays == Some(a.id)))
        }
        ObligationKind::GrooveDestabilization => {
            a.kind == ActionKind::ReEntry
                || (a.kind == ActionKind::Fill
                    && a.target_beat.is_some_and(|t| {
                        plan.of_kind(ActionKind::ReEntry)
                            .any(|r| (r.start_beat - t).abs() < 1e-6)
                    }))
        }
        ObligationKind::RegisterAscent => false,
    }
}

/// Bind each settlement to the first action (in time order) that starts inside the settling
/// phrase's beat span `phrase_span(by_phrase) = [start, end)` and musically discharges the debt's
/// kind (see `discharges`). A settlement with no such action has its witness cleared, so
/// [`ObligationLedger::unwitnessed_settlements`] reports it. Pure: reads the actions, writes only
/// the ledger's witnesses.
pub fn bind_settlement_witnesses(
    ledger: &mut ObligationLedger,
    actions: &ActionPlan,
    phrase_span: impl Fn(u32) -> (f64, f64),
) {
    let chrono = actions.chronological();
    for o in &mut ledger.obligations {
        let kind = o.kind;
        let Some(s) = o.settlement.as_mut() else {
            continue;
        };
        let (start, end) = phrase_span(s.by_phrase);
        s.witness = chrono
            .iter()
            .find(|a| {
                a.start_beat >= start - 1e-6
                    && a.start_beat < end - 1e-6
                    && discharges(kind, a, actions)
            })
            .map(|a| a.id);
    }
}

/// A phrase's discourse commitment: its rhetorical role, permitted closure and the targets the
/// realizers steer toward. This is the discourse half of what becomes a `PhraseTarget` once the
/// realizers consume it. The debts a phrase opens and settles live in the [`ObligationLedger`]
/// ([`ObligationLedger::created_by`] / [`ObligationLedger::settled_by`]) — the relation is
/// many-to-many, and a `Copy` goal has no business pretending it is one-to-one.
#[derive(Debug, Clone, Copy)]
pub struct PhraseGoal {
    /// The phrase this goal governs.
    pub phrase_ix: u32,
    /// Its rhetorical job.
    pub role: DiscourseRole,
    /// How strongly it is permitted to close.
    pub closure: Closure,
    /// Earlier phrase this refers to (thesis home for Restate/Return; for an Answer, the phrase
    /// that opened the strongest debt it settles — its own cycle's culmination, not the global one).
    pub refers_to: Option<u32>,
    /// The next phrase carrying a landing role (Culminate/Answer/Return/Dissolve) — what this
    /// phrase is leading toward.
    pub next_goal: Option<u32>,
    /// Target energy / tension / density / register `[0,1]` the realizers steer this phrase toward.
    pub energy_target: f32,
    pub tension_target: f32,
    pub density_target: f32,
    pub register_target: f32,
    /// Target distance from the thesis `[0,1]` (thematic and harmonic).
    pub thematic_distance: f32,
    pub harmonic_distance: f32,
    /// The novelty this phrase may spend `[0,1]`.
    pub novelty_budget: f32,
}

/// The discourse plan for a whole piece: its thesis, a goal per phrase, and the obligation ledger.
#[derive(Debug, Clone)]
pub struct DiscoursePlan {
    pub thesis: MusicalThesis,
    pub goals: Vec<PhraseGoal>,
    pub ledger: ObligationLedger,
    /// The phrase index of the culmination (peak commitment).
    pub culmination: u32,
    /// The phrase index of the answer (the discharge), if the piece resolves.
    pub answer: Option<u32>,
}

impl DiscoursePlan {
    /// The goal for phrase `ix` (clamped to the last).
    pub fn goal(&self, ix: usize) -> &PhraseGoal {
        self.goals
            .get(ix)
            .unwrap_or_else(|| self.goals.last().expect("discourse plan has no goals"))
    }

    /// The adversarial **"shuffle the sentences"**: reverse the rhetorical roles across phrases
    /// while keeping every phrase's position and trajectory targets fixed, then recompute the
    /// obligation ledger for that order. Each phrase is still a locally valid musical object, but
    /// the argument now runs backwards — the discourse diagnostics must score it worse (abandoned
    /// debts, wrong-sign role directions, a culmination landing after its answer). This is exactly
    /// the maintainer's "grammatically correct song whose meaning has been scrambled", made into a
    /// regression the machine can catch without an aesthetic oracle.
    pub fn scrambled(&self) -> DiscoursePlan {
        let n = self.goals.len();
        if n == 0 {
            return self.clone();
        }
        let roles: Vec<DiscourseRole> = self.goals.iter().rev().map(|g| g.role).collect();
        let ledger = resolve_obligations(&roles);
        let goals: Vec<PhraseGoal> = self
            .goals
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let role = roles[i];
                PhraseGoal {
                    role,
                    closure: closure_for(role),
                    next_goal: None,
                    thematic_distance: role_thematic_distance(role),
                    harmonic_distance: role_harmonic_distance(role),
                    // Each phrase KEEPS its own trajectory targets — only the rhetoric is permuted.
                    ..*g
                }
            })
            .collect();
        DiscoursePlan {
            thesis: self.thesis.clone(),
            goals,
            ledger,
            culmination: (n as u32 - 1) - self.culmination,
            answer: self.answer.map(|a| (n as u32 - 1) - a),
        }
    }

    /// Build the discourse plan from the causal timeline, the form graph and the contract.
    ///
    /// The plan is derived from the intent **trajectory** (each phrase's [`super::timeline::IntentSpan`]),
    /// planning from both directions: a forward scan establishes home and accumulates what has been
    /// stated, while the future anchors — the culmination (peak commitment) and the answer (the
    /// later release it makes necessary) — shape the phrases leading in and decide which phrases are
    /// allowed to stay open.
    pub fn build(
        timeline: &IntentTimeline,
        form: &FormGraph,
        contract: &CoherenceContract,
    ) -> DiscoursePlan {
        let phrases = &form.phrases;
        let n = phrases.len();

        // The thesis: the settled home, taken from the first A-family statement (or the opening).
        let established_by = phrases
            .iter()
            .find(|p| matches!(p.family, SectionFamily::A))
            .map(|p| p.ix)
            .unwrap_or(0);
        let home = phrases
            .get(established_by as usize)
            .map(|p| p.intent)
            .unwrap_or(timeline.initial);
        let thesis = MusicalThesis {
            home_energy: home.energy,
            home_tension: home.tension,
            home_register: home.register,
            home_density: home.density,
            established_by,
            anchors: contract.anchors.clone(),
        };

        if n == 0 {
            return DiscoursePlan {
                thesis,
                goals: Vec::new(),
                ledger: ObligationLedger::default(),
                culmination: 0,
                answer: None,
            };
        }

        // Culmination = the phrase of maximum commitment, via the SINGLE shared definition of "the
        // peak" the form graph also uses for its `Climax` family — so the arrangement and the melody
        // agree on which phrase is the peak.
        let spans: Vec<IntentSpan> = phrases.iter().map(|p| p.span).collect();
        let culmination = culmination_index(&spans);
        let peak_tension = phrases[culmination].span.peak_tension.tension;

        // Answer = the first phrase after the culmination whose trajectory actually lands a release
        // (tension falls well below the peak). If none does, the piece stays unresolved — a
        // legitimate rise-then-remain-open shape, and the diagnostics will say so.
        let answer = (culmination + 1..n).find(|&i| {
            phrases[i].span.end.tension <= (peak_tension - 0.2).max(0.0)
                && phrases[i].span.end.tension < phrases[i].span.start.tension + 1e-6
        });

        // Establish/rise midpoint (for splitting Depart from Intensify in the run-in).
        let rise_mid = culmination / 2;

        // Assign a rhetorical role to every phrase. Most grammars derive roles positionally from
        // the trajectory (`role_for`); a grammar with an explicit SONG BACKBONE (DeflectedLift)
        // imposes its own recurrent sequence instead.
        let roles: Vec<DiscourseRole> =
            SongBackbone::roles(contract.grammar, n).unwrap_or_else(|| {
                (0..n)
                    .map(|i| {
                        role_for(
                            i,
                            n,
                            culmination,
                            answer,
                            rise_mid,
                            matches!(
                                phrases[i].family,
                                SectionFamily::A | SectionFamily::APrime { .. }
                            ),
                        )
                    })
                    .collect()
            });

        // Open and settle the cross-phrase obligation ledger for this ordering.
        let ledger = resolve_obligations(&roles);

        // A goal per phrase: targets grounded in the phrase's own trajectory; closure, distances
        // and novelty from its role; referents to earlier material.
        let mut goals: Vec<PhraseGoal> = phrases
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let role = roles[i];
                let refers_to = match role {
                    DiscourseRole::Restate | DiscourseRole::Return => Some(established_by),
                    DiscourseRole::Answer => Some(answer_referent(&ledger, &roles, i, culmination)),
                    _ => None,
                };
                let (energy_target, tension_target, density_target, register_target) =
                    targets_for(role, p);
                PhraseGoal {
                    phrase_ix: i as u32,
                    role,
                    closure: closure_for(role),
                    refers_to,
                    next_goal: None, // filled below
                    energy_target,
                    tension_target,
                    density_target,
                    register_target,
                    thematic_distance: role_thematic_distance(role),
                    harmonic_distance: role_harmonic_distance(role),
                    novelty_budget: role_novelty(role, contract.novelty_budget),
                }
            })
            .collect();

        // --- Backward pass: each phrase's next landing goal (what it leads toward). ---
        let mut next_landing: Option<u32> = None;
        for i in (0..n).rev() {
            goals[i].next_goal = next_landing;
            if is_landing(goals[i].role) {
                next_landing = Some(i as u32);
            }
        }

        DiscoursePlan {
            thesis,
            goals,
            ledger,
            culmination: culmination as u32,
            answer: answer.map(|a| a as u32),
        }
    }
}

/// The culmination phrase index — the phrase of maximum commitment: peak (energy + tension) across
/// its [`IntentSpan`], with an interior clamp for pieces of three or more phrases. This is the
/// **single definition of "the peak"**, shared by the form graph's positional `Climax` family and
/// this layer's [`DiscourseRole::Culminate`], so the arrangement (keyed on the family) and the
/// melody (keyed on the role) can never disagree about which phrase is the peak — the Round IV fix
/// for the second split-brain. "First strictly greater" wins ties toward the earlier peak (the
/// impact, not a later echo of the same level).
pub(crate) fn culmination_index(spans: &[IntentSpan]) -> usize {
    let n = spans.len();
    if n == 0 {
        return 0;
    }
    let score = |i: usize| spans[i].peak_energy.energy + spans[i].peak_tension.tension;
    let mut culmination = 0usize;
    let mut best = f32::MIN;
    for i in 0..n {
        if score(i) > best + 1e-6 {
            best = score(i);
            culmination = i;
        }
    }
    if n >= 3 {
        culmination = culmination.clamp(1, n - 2);
    }
    culmination
}

/// The deadline for a debt of `kind` opened at `source` under a role ordering: a non-deferrable
/// debt is due at the FIRST later phrase whose role can lawfully settle it (else the last phrase);
/// a deferrable one may run to the last phrase. Always strictly after `source` — `None` only when
/// nothing follows it. (The old rule pinned every question/cadence to the piece's single global
/// answer, so a debt opened at phrase 5 fell due at phrase 3: a bill dated before its purchase.)
fn deadline_for(
    kind: ObligationKind,
    deferrable: bool,
    source: usize,
    roles: &[DiscourseRole],
) -> Option<u32> {
    let n = roles.len();
    if source + 1 >= n {
        return None;
    }
    let last = (n - 1) as u32;
    if deferrable {
        return Some(last);
    }
    Some(
        (source + 1..n)
            .find(|&j| compatible(kind, roles[j]).is_some())
            .map_or(last, |j| j as u32),
    )
}

/// Open and settle the cross-phrase obligation ledger for a role ordering. Shared by
/// [`DiscoursePlan::build`] and the adversarial [`DiscoursePlan::scrambled`] probe, so both resolve
/// debts by exactly the same rules — the scramble only changes the *order*, never the bookkeeping.
///
/// Each phrase first settles, by id and oldest first, every open debt its role can lawfully
/// discharge ([`compatible`]) — so an Answer pays its own cycle's question, withheld cadences and
/// culmination while a harmonic departure waits for a homecoming — then opens its own debts. No
/// "most recent of any kind" fallback: a phrase that cannot pay a debt does not get to pretend.
pub(crate) fn resolve_obligations(roles: &[DiscourseRole]) -> ObligationLedger {
    let mut ledger = ObligationLedger::new(roles.len() as u32);
    for (i, &role) in roles.iter().enumerate() {
        let due: Vec<(ObligationId, SettleHow)> = ledger
            .open()
            .filter_map(|o| compatible(o.kind, role).map(|how| (o.id, how)))
            .collect();
        for (id, how) in due {
            ledger
                .settle(id, i as u32, role, how)
                .expect("an open, earlier, compatible debt always settles");
        }

        let mut to_open: Vec<(ObligationKind, f32, bool)> = Vec::with_capacity(2);
        match role {
            DiscourseRole::Question => to_open.push((ObligationKind::MotifQuestion, 0.7, false)),
            DiscourseRole::Withhold => to_open.push((ObligationKind::SuspendedCadence, 0.8, false)),
            DiscourseRole::Culminate => {
                to_open.push((ObligationKind::SuspendedCadence, 1.0, false))
            }
            DiscourseRole::Depart => to_open.push((ObligationKind::HarmonicDeparture, 0.6, true)),
            DiscourseRole::Intensify => to_open.push((ObligationKind::RegisterAscent, 0.5, true)),
            _ => {}
        }
        // The groove debt opens where the kit is actually stripped — once per stripped run, since a
        // second Withhold does not strip a kit that is already bare.
        if strips_groove(role) && !(i > 0 && strips_groove(roles[i - 1])) {
            to_open.push((ObligationKind::GrooveDestabilization, 0.4, false));
        }
        for (kind, strength, deferrable) in to_open {
            let deadline = deadline_for(kind, deferrable, i, roles);
            ledger.open_debt(kind, i as u32, deadline, strength, deferrable);
        }
    }
    ledger
}

/// The phrase an Answer at `i` refers to: the source of the strongest debt it settled (latest on a
/// tie) — its own cycle's culmination — else the nearest earlier Culminate, else the global one.
fn answer_referent(
    ledger: &ObligationLedger,
    roles: &[DiscourseRole],
    i: usize,
    culmination: usize,
) -> u32 {
    let settled = ledger
        .settled_by(i as u32)
        .max_by(|a, b| {
            a.strength
                .total_cmp(&b.strength)
                .then(a.source_phrase.cmp(&b.source_phrase))
        })
        .map(|o| o.source_phrase);
    settled
        .or_else(|| {
            roles[..i]
                .iter()
                .rposition(|&r| r == DiscourseRole::Culminate)
                .map(|j| j as u32)
        })
        .unwrap_or(culmination as u32)
}

/// A reusable **song backbone** — the recurrent section identity a grammar imposes, expressed as a
/// per-phrase [`DiscourseRole`] sequence. Most grammars derive roles positionally from the
/// trajectory (`role_for`); a grammar that carries an explicit backbone overrides that with its
/// own shape. Today only [`CompositionGrammar::DeflectedLift`] does.
pub struct SongBackbone;

impl SongBackbone {
    /// The role sequence a `grammar` imposes over `n` phrases, or `None` to fall back to the
    /// trajectory-positional `role_for`.
    pub fn roles(grammar: CompositionGrammar, n: usize) -> Option<Vec<DiscourseRole>> {
        match grammar {
            CompositionGrammar::DeflectedLift => Some(deflected_lift_roles(n)),
            _ => None,
        }
    }
}

/// The DeflectedLift bounce as a RHETORICAL role sequence (the "why" layer): `Establish` (the verse)
/// opens; then a recurring cycle of `Depart` → `Culminate` → `Answer` → `Return`; and `Dissolve`
/// (the tag) closes. The hook (`Culminate`) recurs across cycles — the song keeps becoming itself
/// instead of building to one cinematic climax.
///
/// This is deliberately ORTHOGONAL to the harmonic gesture cell (Lift → Deflect → Open → Reset),
/// which lives in `backbone::BackboneTimeline`: a role is *why* a phrase exists in the argument; a
/// gesture is *what the harmony does* under it. Round V conflated the two — encoding the harmonic
/// contour with these roles, in a muddled order, which is why the spine was inaudible; Round VI
/// keeps them separate.
///
/// Each cycle opens exactly the obligations a later phrase in the SAME cycle settles (Depart's
/// harmonic departure paid by its Return; Culminate's suspended cadence paid by its Answer), so the
/// ledger stays balanced — no cinematic debt is left hanging. An interior remainder that does not
/// fill a whole cycle is filled with a ledger-BALANCED mini-cycle so the hook still recurs: a
/// `Culminate`+`Answer` pair (a self-closing bounce — the suspended cadence it opens is settled at
/// once), with a lone leftover phrase a ledger-neutral `Restate`. This guarantees the hook recurs
/// (>=2 `Culminate`) whenever there is room, without abandoning an obligation.
fn deflected_lift_roles(n: usize) -> Vec<DiscourseRole> {
    use DiscourseRole::*;
    match n {
        0 => Vec::new(),
        1 => vec![Establish],
        2 => vec![Establish, Dissolve],
        _ => {
            let mut roles = Vec::with_capacity(n);
            roles.push(Establish);
            let interior = n - 2;
            let cycle = [Depart, Culminate, Answer, Return];
            for _ in 0..(interior / cycle.len()) {
                roles.extend_from_slice(&cycle);
            }
            // A ledger-balanced remainder that still recurs the hook (Culminate+Answer self-closes;
            // a lone leftover is a neutral Restate).
            match interior % cycle.len() {
                0 => {}
                1 => roles.push(Restate),
                2 => roles.extend_from_slice(&[Culminate, Answer]),
                _ => roles.extend_from_slice(&[Culminate, Answer, Restate]),
            }
            roles.push(Dissolve);
            roles
        }
    }
}

/// The rhetorical role of phrase `i` given the piece's anchors. Roles are consequences of the
/// trajectory: the opening establishes, the peak culminates, the release answers, the tail returns
/// and dissolves; recurrences of the thesis restate; the run-in departs then intensifies with a
/// withheld arrival right before the peak.
fn role_for(
    i: usize,
    n: usize,
    culmination: usize,
    answer: Option<usize>,
    rise_mid: usize,
    is_a_family: bool,
) -> DiscourseRole {
    if i == 0 {
        return DiscourseRole::Establish;
    }
    if i == n - 1 {
        return DiscourseRole::Dissolve;
    }
    if i == culmination {
        return DiscourseRole::Culminate;
    }
    if answer == Some(i) {
        return DiscourseRole::Answer;
    }
    if let Some(a) = answer {
        if i > a {
            return DiscourseRole::Return;
        }
    }
    // Immediately before the peak: withhold the expected arrival.
    if culmination >= 1 && i == culmination - 1 {
        return DiscourseRole::Withhold;
    }
    // Between the peak and the answer (or, if unresolved, to the end): sustain the open tension.
    if i > culmination {
        return DiscourseRole::Withhold;
    }
    // The run-in to the culmination. A recurrence restates the thesis; otherwise the phrase two
    // before the peak poses the open question (an incomplete gesture the later Answer completes),
    // the earlier run-in departs from home, and the approach intensifies.
    if is_a_family {
        DiscourseRole::Restate
    } else if culmination >= 2 && i == culmination - 2 {
        DiscourseRole::Question
    } else if i >= rise_mid {
        DiscourseRole::Intensify
    } else {
        DiscourseRole::Depart
    }
}

/// Whether a role is a "landing" the earlier phrases lead toward.
fn is_landing(role: DiscourseRole) -> bool {
    matches!(
        role,
        DiscourseRole::Culminate
            | DiscourseRole::Answer
            | DiscourseRole::Return
            | DiscourseRole::Dissolve
    )
}

/// The permitted closure strength for a role. Only the discharge roles get a strong terminal
/// cadence; the culmination is left maximally unresolved (a half cadence's dominant pull).
fn closure_for(role: DiscourseRole) -> Closure {
    match role {
        DiscourseRole::Establish | DiscourseRole::Restate => Closure::Weak,
        DiscourseRole::Depart => Closure::Open,
        DiscourseRole::Intensify | DiscourseRole::Question => Closure::Half,
        DiscourseRole::Withhold => Closure::Deferred,
        DiscourseRole::Culminate => Closure::Half,
        DiscourseRole::Answer | DiscourseRole::Return | DiscourseRole::Dissolve => Closure::Strong,
    }
}

/// Target energy/tension/density/register for a phrase — grounded in its own realized trajectory
/// (`IntentSpan`), so targets are causal, not invented. The culmination targets its peak; every
/// other phrase targets the state it concludes in.
fn targets_for(role: DiscourseRole, p: &super::plan::Phrase) -> (f32, f32, f32, f32) {
    let s = &p.span;
    match role {
        DiscourseRole::Culminate => (
            s.peak_energy.energy,
            s.peak_tension.tension,
            s.peak_energy.density,
            s.peak_energy.register,
        ),
        _ => (s.end.energy, s.end.tension, s.end.density, s.end.register),
    }
}

fn role_thematic_distance(role: DiscourseRole) -> f32 {
    match role {
        DiscourseRole::Establish => 0.0,
        DiscourseRole::Restate | DiscourseRole::Return => 0.12,
        DiscourseRole::Answer => 0.30,
        DiscourseRole::Dissolve => 0.40,
        DiscourseRole::Depart | DiscourseRole::Question => 0.50,
        DiscourseRole::Intensify => 0.55,
        DiscourseRole::Withhold => 0.60,
        DiscourseRole::Culminate => 0.80,
    }
}

fn role_harmonic_distance(role: DiscourseRole) -> f32 {
    match role {
        DiscourseRole::Establish => 0.0,
        DiscourseRole::Restate | DiscourseRole::Return => 0.10,
        DiscourseRole::Dissolve => 0.20,
        DiscourseRole::Answer => 0.25,
        DiscourseRole::Intensify => 0.50,
        DiscourseRole::Question => 0.55,
        DiscourseRole::Depart => 0.60,
        DiscourseRole::Withhold => 0.70,
        DiscourseRole::Culminate => 0.80,
    }
}

fn role_novelty(role: DiscourseRole, budget: f32) -> f32 {
    let factor = match role {
        DiscourseRole::Culminate => 1.0,
        DiscourseRole::Depart | DiscourseRole::Question => 0.7,
        DiscourseRole::Intensify | DiscourseRole::Withhold => 0.6,
        DiscourseRole::Answer => 0.4,
        _ => 0.3,
    };
    (budget * factor).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::super::semantic::{calm_loop, demo_trace, false_climax, rise_unresolved};
    use super::*;

    fn demo() -> (DiscoursePlan, FormGraph) {
        let tl = IntentTimeline::walk(&demo_trace(120.0));
        let contract = CoherenceContract::infer(&tl);
        let form = FormGraph::build(&tl, 30, &contract);
        let plan = DiscoursePlan::build(&tl, &form, &contract);
        (plan, form)
    }

    #[test]
    fn every_phrase_has_a_goal_and_referential_roles_point_somewhere() {
        let (plan, form) = demo();
        assert_eq!(
            plan.goals.len(),
            form.phrases.len(),
            "a goal per phrase (no orphans)"
        );
        for g in &plan.goals {
            if g.role.is_referential() {
                assert!(
                    g.refers_to.is_some(),
                    "referential role {:?} at phrase {} points nowhere",
                    g.role,
                    g.phrase_ix
                );
            }
        }
    }

    #[test]
    fn culmination_precedes_the_answer_and_is_not_resolved() {
        let (plan, _) = demo();
        let answer = plan.answer.expect("the demo arc resolves");
        assert!(
            plan.culmination < answer,
            "culmination {} must precede the answer {}",
            plan.culmination,
            answer
        );
        // The culmination is the peak, NOT the discharge: its permitted closure is not terminal.
        let cg = plan.goal(plan.culmination as usize);
        assert_eq!(cg.role, DiscourseRole::Culminate);
        assert!(
            !cg.closure.is_terminal(),
            "the culmination must not be a strong terminal cadence"
        );
        // The answer pays the debt the culmination opened.
        let ag = plan.goal(answer as usize);
        assert_eq!(ag.role, DiscourseRole::Answer);
        assert!(
            plan.ledger.settled_by(answer).next().is_some(),
            "the answer settles no obligation"
        );
    }

    #[test]
    fn the_canonical_arc_reads_establish_culminate_answer_dissolve_in_order() {
        let (plan, _) = demo();
        let roles: Vec<DiscourseRole> = plan.goals.iter().map(|g| g.role).collect();
        let find = |r: DiscourseRole| roles.iter().position(|&x| x == r);
        let est = find(DiscourseRole::Establish).expect("no establish");
        let cul = find(DiscourseRole::Culminate).expect("no culminate");
        let ans = find(DiscourseRole::Answer).expect("no answer");
        let dis = roles
            .iter()
            .rposition(|&x| x == DiscourseRole::Dissolve)
            .expect("no dissolve");
        assert!(
            est < cul && cul < ans && ans <= dis,
            "arc out of order: {roles:?}"
        );
    }

    #[test]
    fn a_resolving_arc_abandons_no_obligation() {
        let (plan, _) = demo();
        assert!(
            !plan.ledger.obligations.is_empty(),
            "no obligations were ever opened"
        );
        assert_eq!(
            plan.ledger.abandoned_count(),
            0,
            "the resolving demo arc left a non-deferrable debt unpaid: {:?}",
            plan.ledger
                .obligations
                .iter()
                .filter(|o| plan.ledger.final_status(o.id) == Some(ObligationStatus::Abandoned))
                .collect::<Vec<_>>()
        );
        // The culmination's debt in particular is settled — and by the ANSWER, not whichever
        // strong closure happened to be passing (the old LIFO had a Return pay it).
        let o = plan
            .ledger
            .created_by(plan.culmination)
            .find(|o| o.kind == ObligationKind::SuspendedCadence)
            .expect("culmination opened no debt");
        assert_eq!(
            o.resolved_by(),
            plan.answer,
            "the culmination's debt was not paid by the answer"
        );
    }

    #[test]
    fn is_deterministic() {
        let (a, _) = demo();
        let (b, _) = demo();
        let ra: Vec<_> = a.goals.iter().map(|g| (g.role, g.closure)).collect();
        let rb: Vec<_> = b.goals.iter().map(|g| (g.role, g.closure)).collect();
        assert_eq!(ra, rb);
        assert_eq!(a.ledger.obligations.len(), b.ledger.obligations.len());
    }

    fn plan_for(trace: &super::super::semantic::SemanticTrace) -> (DiscoursePlan, FormGraph) {
        let tl = IntentTimeline::walk(trace);
        let total_bars = (trace.total_beats / super::super::form::BEATS_PER_BAR).round() as u32;
        let contract = CoherenceContract::infer(&tl);
        let form = FormGraph::build(&tl, total_bars, &contract);
        let plan = DiscoursePlan::build(&tl, &form, &contract);
        (plan, form)
    }

    // --- Anti-overfitting: the planner responds to the SHAPE, not the one canonical demo. ---
    #[test]
    fn an_unresolved_arc_has_no_answer_and_leaves_a_debt_open() {
        let (plan, _) = plan_for(&rise_unresolved(120.0));
        assert!(
            plan.answer.is_none(),
            "a rise-then-unresolved arc must not manufacture an answer"
        );
        assert!(
            plan.ledger.abandoned_count() >= 1,
            "the unresolved culmination's debt should be left open, not silently paid"
        );
    }

    #[test]
    fn false_climax_puts_the_culmination_on_the_true_peak() {
        let (plan, form) = plan_for(&false_climax(120.0));
        let cul_beat = form.phrases[plan.culmination as usize].start_beat();
        // The true (larger) impact is at ~0.64*120 ≈ beat 77; the false one at ~beat 26. The
        // culmination must land in the second half, not on the earlier, smaller peak.
        assert!(
            cul_beat > 48.0,
            "culmination landed on the false climax at beat {cul_beat}, not the true peak"
        );
    }

    #[test]
    fn a_calm_loop_invents_no_high_pressure_culmination() {
        let (plan, form) = plan_for(&calm_loop(120.0));
        let peak = form.phrases[plan.culmination as usize]
            .span
            .peak_tension
            .tension;
        assert!(
            peak < 0.5,
            "a calm loop should not be given a high-tension climax: peak {peak}"
        );
    }

    // --- The ledger is deadline-honest. ---
    #[test]
    fn a_deferrable_debt_is_pending_before_its_deadline() {
        let mut led = ObligationLedger::new(6);
        let hd = led.open_debt(ObligationKind::HarmonicDeparture, 1, Some(5), 0.6, true);
        let sc = led.open_debt(ObligationKind::SuspendedCadence, 1, Some(3), 0.8, false);
        for now in 1..=3 {
            assert_eq!(led.status_at(hd, now), Some(ObligationStatus::Pending));
            assert_eq!(led.status_at(sc, now), Some(ObligationStatus::Pending));
        }
        // The deferrable one may cross phrase 4 (even a compatible one); the other may not.
        assert_eq!(led.status_at(hd, 4), Some(ObligationStatus::Pending));
        assert_eq!(led.status_at(sc, 4), Some(ObligationStatus::Abandoned));
        assert_eq!(led.status_at(ObligationId(7), 0), None, "unknown id");
    }

    #[test]
    fn a_debt_never_settled_past_its_deadline_is_abandoned() {
        let mut led = ObligationLedger::new(6);
        let sc = led.open_debt(ObligationKind::SuspendedCadence, 1, Some(2), 0.8, false);
        // Deferrable is not a licence to outlive a deadline.
        let hd = led.open_debt(ObligationKind::HarmonicDeparture, 1, Some(4), 0.6, true);
        assert_eq!(led.status_at(sc, 2), Some(ObligationStatus::Pending));
        assert_eq!(led.status_at(sc, 3), Some(ObligationStatus::Abandoned));
        assert_eq!(led.final_status(hd), Some(ObligationStatus::Abandoned));
        assert_eq!(led.abandoned_count(), 2);

        // Through the builder: deferrable debts no role ever settles. The old ledger never read a
        // deadline and forgave every deferrable debt, so this counted 0 abandoned.
        use DiscourseRole::*;
        let led = resolve_obligations(&[Establish, Depart, Intensify, Establish]);
        assert_eq!(led.obligations.len(), 2);
        assert!(led.obligations.iter().all(|o| o.deferrable));
        assert_eq!(led.abandoned_count(), 2, "{:?}", led.obligations);
    }

    #[test]
    fn a_debt_settled_after_its_deadline_is_late() {
        let mut led = ObligationLedger::new(6);
        let sc = led.open_debt(ObligationKind::SuspendedCadence, 1, Some(2), 0.8, false);
        led.settle(sc, 4, DiscourseRole::Answer, SettleHow::Paid)
            .unwrap();
        // Overdue in the gap, late once paid — and never both late and abandoned at the end.
        assert_eq!(led.status_at(sc, 3), Some(ObligationStatus::Abandoned));
        assert_eq!(led.status_at(sc, 4), Some(ObligationStatus::Late));
        assert_eq!(led.final_status(sc), Some(ObligationStatus::Late));
        assert_eq!((led.late_count(), led.abandoned_count()), (1, 0));
        // On time is Settled, carrying how.
        let mq = led.open_debt(ObligationKind::MotifQuestion, 1, Some(4), 0.7, false);
        led.settle(mq, 4, DiscourseRole::Return, SettleHow::Deflected)
            .unwrap();
        assert_eq!(
            led.final_status(mq),
            Some(ObligationStatus::Settled(SettleHow::Deflected))
        );
    }

    // --- Settlement is specific: the answer pays the question it answers, nothing else. ---
    #[test]
    fn an_answer_settles_the_question_and_leaves_the_departure_for_a_homecoming() {
        use DiscourseRole::*;
        let led = resolve_obligations(&[Establish, Question, Depart, Answer, Dissolve]);
        let kinds: Vec<(ObligationId, ObligationKind, u32)> = led
            .obligations
            .iter()
            .map(|o| (o.id, o.kind, o.source_phrase))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (ObligationId(0), ObligationKind::MotifQuestion, 1),
                (ObligationId(1), ObligationKind::GrooveDestabilization, 1),
                (ObligationId(2), ObligationKind::HarmonicDeparture, 2),
            ]
        );
        // The old LIFO had the Answer pay the most recent debt of ANY kind: the departure.
        let by_answer: Vec<ObligationId> = led.settled_by(3).map(|o| o.id).collect();
        assert_eq!(by_answer, vec![ObligationId(0)]);
        assert_eq!(
            led.final_status(ObligationId(0)),
            Some(ObligationStatus::Settled(SettleHow::Paid))
        );
        // The stripped groove comes back with the Depart's full kit.
        assert_eq!(led.get(ObligationId(1)).unwrap().resolved_by(), Some(2));
        // The departure is still owed at the answer, and paid by the dissolve's homecoming.
        assert_eq!(
            led.status_at(ObligationId(2), 3),
            Some(ObligationStatus::Pending)
        );
        assert_eq!(led.get(ObligationId(2)).unwrap().resolved_by(), Some(4));
        assert_eq!((led.abandoned_count(), led.late_count()), (0, 0));
    }

    #[test]
    fn settle_refuses_unknown_early_incompatible_and_double_settlement() {
        let mut led = ObligationLedger::new(6);
        let hd = led.open_debt(ObligationKind::HarmonicDeparture, 2, Some(5), 0.6, true);
        let mq = led.open_debt(ObligationKind::MotifQuestion, 2, Some(4), 0.7, false);
        let paid = SettleHow::Paid;
        assert_eq!(
            led.settle(ObligationId(9), 3, DiscourseRole::Return, paid),
            Err(SettleError::Unknown(ObligationId(9)))
        );
        for by in [1, 2] {
            assert_eq!(
                led.settle(hd, by, DiscourseRole::Return, paid),
                Err(SettleError::NotYetOpened {
                    id: hd,
                    source_phrase: 2,
                    by_phrase: by
                })
            );
        }
        assert_eq!(
            led.settle(hd, 3, DiscourseRole::Answer, paid),
            Err(SettleError::IncompatibleKind {
                id: hd,
                kind: ObligationKind::HarmonicDeparture,
                role: DiscourseRole::Answer,
                how: paid
            })
        );
        // A Return can only DEFLECT a motif question; recording it as paid is refused.
        assert!(matches!(
            led.settle(mq, 3, DiscourseRole::Return, paid),
            Err(SettleError::IncompatibleKind { .. })
        ));
        assert_eq!(
            led.resolved_count(),
            0,
            "a refused settlement wrote nothing"
        );
        assert_eq!(led.settle(hd, 3, DiscourseRole::Return, paid), Ok(()));
        assert_eq!(
            led.settle(hd, 4, DiscourseRole::Return, paid),
            Err(SettleError::AlreadySettled {
                id: hd,
                by_phrase: 3
            })
        );
    }

    // --- The flagship: every deadline after its source; each cycle settles its own debt. ---
    #[test]
    fn the_deflected_lift_flagship_settles_each_cycle_on_time() {
        use super::super::contract::CompositionGrammar;
        use super::super::semantic::deflected_lift_trace;
        use DiscourseRole::*;
        let tl = IntentTimeline::walk(&deflected_lift_trace(120.0));
        let contract = CoherenceContract::for_grammar(CompositionGrammar::DeflectedLift);
        let form = FormGraph::build(&tl, 30, &contract);
        let plan = DiscoursePlan::build(&tl, &form, &contract);
        let roles: Vec<DiscourseRole> = plan.goals.iter().map(|g| g.role).collect();
        assert_eq!(
            roles,
            vec![
                Establish, Depart, Culminate, Answer, Return, Culminate, Answer, Restate, Dissolve
            ]
        );
        let led = &plan.ledger;
        for o in &led.obligations {
            let d = o.deadline.expect("every flagship debt has a later phrase");
            assert!(d > o.source_phrase, "{} due@{d} <= source", o.id);
        }
        // The second hook's debt: opened at 5, due at 6 (the old rule said 3), paid by 6 alone.
        let second = led
            .created_by(5)
            .find(|o| o.kind == ObligationKind::SuspendedCadence)
            .expect("the second culmination opened no debt");
        assert_eq!(second.deadline, Some(6));
        let by6: Vec<ObligationId> = led.settled_by(6).map(|o| o.id).collect();
        assert_eq!(by6, vec![second.id]);
        assert_eq!(plan.goal(6).refers_to, Some(5), "phrase 6 answers phrase 5");
        assert_eq!(plan.goal(3).refers_to, Some(2), "phrase 3 answers phrase 2");
        assert_eq!((led.abandoned_count(), led.late_count()), (0, 0));
        assert!(
            led.obligations
                .iter()
                .all(|o| matches!(led.final_status(o.id), Some(ObligationStatus::Settled(_)))),
            "{:?}",
            led.obligations
        );
    }

    // --- Witnesses: a settlement cites the concrete action that makes it audible. ---
    fn act(
        id: u32,
        kind: ActionKind,
        initiator: Agent,
        start: f64,
        target: Option<f64>,
        pays: Option<u32>,
        cause: ActionCause,
    ) -> MusicalAction {
        MusicalAction {
            id: ActionId(id),
            cause,
            initiator,
            start_beat: start,
            dur_beats: 1.0,
            kind,
            target_beat: target,
            responders: Vec::new(),
            binding: None,
            pays: pays.map(ActionId),
        }
    }

    #[test]
    fn settlements_bind_to_the_action_that_discharges_them() {
        use ActionKind as K;
        use DiscourseRole::*;
        // ob0 MQ@1, ob1 GD@1 → Answer@2; ob2 SC@3, ob3 GD@3 → Return@4; ob4 RA@5 → Dissolve@6.
        let mut led = resolve_obligations(&[
            Establish, Question, Answer, Withhold, Return, Intensify, Dissolve,
        ]);
        assert_eq!(led.resolved_count(), 5);
        let st = ActionCause::Statement { phrase: 2 };
        let actions = ActionPlan {
            actions: vec![
                // Phrase 2 (beats 16..24): an answered lead call, then a re-entry.
                act(0, K::Call, Agent::Lead, 16.0, None, None, st),
                act(1, K::Answer, Agent::Keys, 18.0, None, Some(0), st),
                act(2, K::ReEntry, Agent::Keys, 17.0, None, None, st),
                // Phrase 4 (beats 32..40): decoys first — a hit paying nothing, a fill aimed at no
                // re-entry — then the real cadence hit (pays the resolve) and a fill into a re-entry.
                act(3, K::Hit, Agent::Ensemble, 33.0, None, None, st),
                act(4, K::Fill, Agent::Drums, 34.0, Some(35.0), None, st),
                act(5, K::Resolve, Agent::Ensemble, 36.0, None, None, st),
                act(6, K::Hit, Agent::Ensemble, 36.0, None, Some(5), st),
                act(7, K::Fill, Agent::Drums, 38.0, Some(40.0), None, st),
                act(8, K::ReEntry, Agent::Pad, 40.0, None, None, st),
                // Phrase 6: a resolve — which does NOT witness a register homecoming.
                act(9, K::Resolve, Agent::Ensemble, 50.0, None, None, st),
            ],
            ..ActionPlan::default()
        };
        let span = |p: u32| (p as f64 * 8.0, p as f64 * 8.0 + 8.0);
        bind_settlement_witnesses(&mut led, &actions, span);
        let w = |led: &ObligationLedger, i: u32| {
            led.get(ObligationId(i))
                .unwrap()
                .settlement
                .unwrap()
                .witness
        };
        assert_eq!(w(&led, 0), Some(ActionId(0)), "the answered lead call");
        assert_eq!(w(&led, 1), Some(ActionId(2)), "the re-entry");
        assert_eq!(
            w(&led, 2),
            Some(ActionId(6)),
            "the cadence hit, not the decoy"
        );
        assert_eq!(
            w(&led, 3),
            Some(ActionId(7)),
            "the fill into a re-entry, not the stray"
        );
        assert_eq!(w(&led, 4), None, "nothing witnesses a register homecoming");
        let unwitnessed: Vec<ObligationId> = led.unwitnessed_settlements().map(|o| o.id).collect();
        assert_eq!(unwitnessed, vec![ObligationId(4)]);

        // Negative control: no actions, no witnesses — every settlement is unwitnessed again.
        bind_settlement_witnesses(&mut led, &ActionPlan::none(), span);
        assert_eq!(led.unwitnessed_settlements().count(), led.resolved_count());
        // And an unanswered call is not an answer.
        let lonely = ActionPlan {
            actions: vec![act(0, K::Call, Agent::Lead, 16.0, None, None, st)],
            ..ActionPlan::default()
        };
        bind_settlement_witnesses(&mut led, &lonely, span);
        assert_eq!(w(&led, 0), None);
    }
}
