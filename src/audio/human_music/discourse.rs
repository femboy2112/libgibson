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

use super::contract::{CoherenceAnchor, CoherenceContract};
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
    /// Orchestration was subtracted and owes a re-entry.
    OrchestrationSubtraction,
    /// The groove was destabilized and owes a restoration.
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
            ObligationKind::OrchestrationSubtraction => "orchestration-subtraction",
            ObligationKind::GrooveDestabilization => "groove-destabilization",
        }
    }
}

/// One cross-phrase debt: opened by `source_phrase`, settled by `resolved_by` (or left open).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Obligation {
    /// Stable id within the ledger.
    pub id: u32,
    /// What kind of debt.
    pub kind: ObligationKind,
    /// The phrase that opened it.
    pub source_phrase: u32,
    /// The phrase by which it ought to be settled, if any.
    pub deadline: Option<u32>,
    /// How load-bearing the debt is `[0,1]`.
    pub strength: f32,
    /// Whether it may be deferred/left open without counting as abandoned.
    pub deferrable: bool,
    /// The phrase that settled it, if any (`None` = still open at the end).
    pub resolved_by: Option<u32>,
}

impl Obligation {
    /// Whether this debt is still open (never settled).
    pub fn is_open(&self) -> bool {
        self.resolved_by.is_none()
    }

    /// Whether this debt was abandoned: open at the end and not deferrable, or open past its
    /// deadline. An abandoned debt is exactly the "unresolved expectation" a listener registers as
    /// scrambled direction.
    pub fn is_abandoned(&self) -> bool {
        self.is_open() && !self.deferrable
    }
}

/// The full set of cross-phrase debts a piece opened, and how each was settled.
#[derive(Debug, Clone, Default)]
pub struct ObligationLedger {
    pub obligations: Vec<Obligation>,
}

impl ObligationLedger {
    /// Debts still open at the end of the piece.
    pub fn open(&self) -> impl Iterator<Item = &Obligation> {
        self.obligations.iter().filter(|o| o.is_open())
    }

    /// Count of debts abandoned (open and non-deferrable) — the number diagnostics want at zero
    /// for a piece that resolves what it raised.
    pub fn abandoned_count(&self) -> usize {
        self.obligations.iter().filter(|o| o.is_abandoned()).count()
    }

    /// Count of debts actually settled.
    pub fn resolved_count(&self) -> usize {
        self.obligations
            .iter()
            .filter(|o| o.resolved_by.is_some())
            .count()
    }
}

/// A phrase's discourse commitment: its rhetorical role, permitted closure, the targets the
/// realizers steer toward, and the obligation it opens or pays. This is the discourse half of what
/// becomes a `PhraseTarget` once the realizers consume it.
#[derive(Debug, Clone, Copy)]
pub struct PhraseGoal {
    /// The phrase this goal governs.
    pub phrase_ix: u32,
    /// Its rhetorical job.
    pub role: DiscourseRole,
    /// How strongly it is permitted to close.
    pub closure: Closure,
    /// Earlier phrase this refers to (thesis home for Restate/Return, the culmination for Answer).
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
    /// The obligation id this phrase opens, if any.
    pub creates: Option<u32>,
    /// The obligation id this phrase settles, if any.
    pub pays: Option<u32>,
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
        let answer = self.answer.map(|a| n - 1 - a as usize);
        let (ledger, creates, pays) = resolve_obligations(&roles, answer);
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
                    creates: creates[i],
                    pays: pays[i],
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

        // Assign a rhetorical role to every phrase — a consequence of the trajectory and anchors.
        let roles: Vec<DiscourseRole> = (0..n)
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
            .collect();

        // Open and settle the cross-phrase obligation ledger for this ordering.
        let (ledger, creates, pays) = resolve_obligations(&roles, answer);

        // A goal per phrase: targets grounded in the phrase's own trajectory; closure, distances
        // and novelty from its role; referents to earlier material.
        let mut goals: Vec<PhraseGoal> = phrases
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let role = roles[i];
                let refers_to = match role {
                    DiscourseRole::Restate | DiscourseRole::Return => Some(established_by),
                    DiscourseRole::Answer => Some(culmination as u32),
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
                    creates: creates[i],
                    pays: pays[i],
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

/// Settle the most recent open obligation matching `prefer` (or the most recent of any kind),
/// marking it resolved by `payer`. Returns the settled obligation's id.
fn pay(
    obligations: &mut [Obligation],
    open: &mut Vec<usize>,
    payer: u32,
    prefer: Option<ObligationKind>,
) -> Option<u32> {
    let pos = prefer
        .and_then(|k| open.iter().rposition(|&oi| obligations[oi].kind == k))
        .or_else(|| open.len().checked_sub(1));
    let pos = pos?;
    let oi = open.remove(pos);
    obligations[oi].resolved_by = Some(payer);
    Some(obligations[oi].id)
}

/// Open and settle the cross-phrase obligation ledger for a role ordering, returning the ledger
/// and, per phrase, the obligation id it opens and the id it settles. Shared by
/// [`DiscoursePlan::build`] and the adversarial [`DiscoursePlan::scrambled`] probe, so both resolve
/// debts by exactly the same rules — the scramble only changes the *order*, never the bookkeeping.
pub(crate) fn resolve_obligations(
    roles: &[DiscourseRole],
    answer: Option<usize>,
) -> (ObligationLedger, Vec<Option<u32>>, Vec<Option<u32>>) {
    let n = roles.len();
    let mut ledger = ObligationLedger::default();
    let mut open: Vec<usize> = Vec::new(); // indices into ledger.obligations
    let mut next_id = 0u32;
    let mut creates = vec![None; n];
    let mut pays = vec![None; n];

    for (i, &role) in roles.iter().enumerate() {
        let to_open = match role {
            DiscourseRole::Question => Some((ObligationKind::MotifQuestion, 0.7, false)),
            DiscourseRole::Withhold => Some((ObligationKind::SuspendedCadence, 0.8, false)),
            DiscourseRole::Culminate => Some((ObligationKind::SuspendedCadence, 1.0, false)),
            DiscourseRole::Depart => Some((ObligationKind::HarmonicDeparture, 0.6, true)),
            DiscourseRole::Intensify => Some((ObligationKind::RegisterAscent, 0.5, true)),
            _ => None,
        };
        if let Some((kind, strength, deferrable)) = to_open {
            let id = next_id;
            next_id += 1;
            let deadline = match kind {
                ObligationKind::MotifQuestion | ObligationKind::SuspendedCadence => {
                    answer.map(|a| a as u32).or(Some((n - 1) as u32))
                }
                _ => Some((n - 1) as u32),
            };
            ledger.obligations.push(Obligation {
                id,
                kind,
                source_phrase: i as u32,
                deadline,
                strength,
                deferrable,
                resolved_by: None,
            });
            open.push(ledger.obligations.len() - 1);
            creates[i] = Some(id);
        }
        pays[i] = match role {
            DiscourseRole::Answer => pay(&mut ledger.obligations, &mut open, i as u32, None),
            DiscourseRole::Return => pay(
                &mut ledger.obligations,
                &mut open,
                i as u32,
                Some(ObligationKind::HarmonicDeparture),
            ),
            DiscourseRole::Dissolve => pay(&mut ledger.obligations, &mut open, i as u32, None),
            _ => None,
        };
    }
    (ledger, creates, pays)
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
        assert!(ag.pays.is_some(), "the answer settles no obligation");
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
                .filter(|o| o.is_abandoned())
                .collect::<Vec<_>>()
        );
        // The culmination's debt in particular is settled.
        let cg = plan.goal(plan.culmination as usize);
        let cul_debt = cg.creates.expect("culmination opened no debt");
        let o = plan
            .ledger
            .obligations
            .iter()
            .find(|o| o.id == cul_debt)
            .unwrap();
        assert!(
            o.resolved_by.is_some(),
            "the culmination's debt was never answered"
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
}
