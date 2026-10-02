//! **Rehearsal** — [`super::policy::ActionAdmission::Rehearsed`] as a finite normalization.
//!
//! ```text
//! plan(inputs) → realize → audit → { every verb performed      ⇒ the take
//!                                  { else strike / recast verbs ⇒ inputs' ⊋ inputs, repeat
//! ```
//!
//! **Identity.** A verb is named across rebuilds by its [`ActionKey`]: its family, its exact
//! metric onset, and a typed cause in which a response names its call *by the call's own key*,
//! never by the call's number (a veto renumbers the plan).
//!
//! **Normalization.** Each non-final pass adds at least one element to the inputs `(V, R)` —
//! `V` the struck verbs, `R` the recast settlements — and neither ever shrinks. A struck verb is
//! never planned again (every planner that makes verbs honours `V`); a settlement is recast at
//! most once (`|R| ≤` the song's settled debts). Every verb any build of one song can hold is drawn
//! from a finite set (onsets on the song's finite phrase / slot / transition lattice, finite
//! causes), so the strictly growing sequence of inputs terminates. [`REHEARSAL_FUEL`] is the
//! executable form of that bound: exhausting it, or a pass that cannot make progress, is
//! **refused** — a take with an unperformed verb is never returned.
//!
//! **Debts.** Rejecting a verb never erases the discourse obligation it served. The obligation
//! layer decides: a debt the song settles in a phrase stays settled in the performance only while
//! a performed verb discharges it there; otherwise it is left open, with its reason
//! ([`OpenDebt`]).

use super::action::{ActionCause, ActionKind, ActionPlan, Agent, MusicalAction};
use super::backbone::HarmonicGesture;
use super::discourse::ObligationKind;
use super::ids::ObligationId;
use super::intent::IntentMorphism;
use super::rhythm::MetricPosition;

/// Passes a band may rehearse before a take is refused (see the module's termination argument).
pub const REHEARSAL_FUEL: usize = 64;

/// Where a verb sits: its exact metric coordinate when its onset is one (every planner onset on
/// the metric lattice is), else the exact float bits — never rounded, never tolerance-matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbOnset {
    Metric(MetricPosition),
    Exact(u64),
}

impl VerbOnset {
    pub fn of(beat: f64) -> Self {
        MetricPosition::from_exact_beats(beat)
            .map(VerbOnset::Metric)
            .unwrap_or(VerbOnset::Exact(beat.to_bits()))
    }
}

/// A verb's family. The harmonic verbs are one family because the harmony may recast one as
/// another after admission (a Reharmonize with no lawful substitute becomes a Recolor): striking
/// the recast verb strikes the verb that was planned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbFamily {
    Harmonic,
    Verb(ActionKind),
}

impl VerbFamily {
    pub fn of(kind: ActionKind) -> Self {
        match kind {
            ActionKind::Tonicize
            | ActionKind::Reharmonize
            | ActionKind::Recolor
            | ActionKind::Modulate => VerbFamily::Harmonic,
            kind => VerbFamily::Verb(kind),
        }
    }
}

/// A verb's cause, with every reference to another verb replaced by that verb's identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CauseKey {
    Morphism {
        transition: usize,
        morphism: IntentMorphism,
    },
    Gesture {
        slot: usize,
        gesture: HarmonicGesture,
    },
    /// A response names its call by identity (`None`: the call is not in the plan).
    Interaction {
        call: Option<Box<ActionKey>>,
    },
    Statement {
        phrase: u32,
    },
    Discourse {
        obligation: ObligationId,
        phrase: u32,
    },
}

/// A verb's identity across rebuilds of one plan: family, exact onset and typed cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionKey {
    pub family: VerbFamily,
    pub onset: VerbOnset,
    pub cause: CauseKey,
}

impl ActionKey {
    /// The identity of `a`, a verb of `plan`.
    pub fn of(a: &MusicalAction, plan: &ActionPlan) -> Self {
        Self::of_depth(a, plan, 0)
    }

    fn of_depth(a: &MusicalAction, plan: &ActionPlan, depth: usize) -> Self {
        let cause = match a.cause {
            ActionCause::Morphism {
                transition,
                morphism,
            } => CauseKey::Morphism {
                transition,
                morphism,
            },
            ActionCause::Gesture { slot, gesture } => CauseKey::Gesture { slot, gesture },
            // A call precedes its response in the plan, so the chain is finite; the depth guard
            // only protects against a hand-forged cycle.
            ActionCause::Interaction { call } => CauseKey::Interaction {
                call: plan
                    .get(call)
                    .filter(|_| depth < 8)
                    .map(|c| Box::new(Self::of_depth(c, plan, depth + 1))),
            },
            ActionCause::Statement { phrase } => CauseKey::Statement { phrase },
            ActionCause::Discourse { obligation, phrase } => {
                CauseKey::Discourse { obligation, phrase }
            }
        };
        Self {
            family: VerbFamily::of(a.kind),
            onset: VerbOnset::of(a.start_beat),
            cause,
        }
    }

    /// The obligation this verb was planned to discharge, if it is a settlement verb.
    pub fn settles(&self) -> Option<ObligationId> {
        match self.cause {
            CauseKey::Discourse { obligation, .. } => Some(obligation),
            _ => None,
        }
    }
}

/// A settlement verb moved, after a rehearsal, onto the arrival a player actually made there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recast {
    pub obligation: ObligationId,
    pub start_beat: f64,
    pub by: Agent,
}

/// What rehearsal did with one planned verb.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RehearsalOutcome {
    /// It is in the take and a player performs it.
    Performed,
    /// Nobody played it where it was planned; it moved onto the arrival a player made.
    Recast { beat: f64, by: Agent },
    /// Nobody performed it; struck from the chart.
    Rejected(&'static str),
    /// Nobody performed it; struck, and the semantic morphism it realized is deferred with the
    /// reason (the plan's deferral list names it).
    Deferred(&'static str),
}

/// One verb and the pass that decided it.
#[derive(Debug, Clone, PartialEq)]
pub struct RehearsedVerb {
    pub key: ActionKey,
    pub kind: ActionKind,
    /// Where it was planned (for a recast verb: where it was planned before the move).
    pub planned_beat: f64,
    /// The pass that decided it (the take's verbs: the final pass).
    pub pass: usize,
    pub outcome: RehearsalOutcome,
}

/// A debt the song's discourse settles in a phrase that this performance leaves open: no
/// performed verb discharges it there. Named, never silently settled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenDebt {
    pub obligation: ObligationId,
    pub kind: ObligationKind,
    /// The phrase the discourse named to settle it.
    pub by_phrase: u32,
    pub reason: &'static str,
}

/// The normalization's record: every planned verb's outcome and every debt left open.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RehearsalTrace {
    /// Rehearsals run before the take (0: the first realization was the take).
    pub passes: usize,
    pub verbs: Vec<RehearsedVerb>,
    pub open_debts: Vec<OpenDebt>,
}

impl RehearsalTrace {
    /// The verbs with `outcome`'s variant.
    pub fn count(&self, pred: impl Fn(&RehearsalOutcome) -> bool) -> usize {
        self.verbs.iter().filter(|v| pred(&v.outcome)).count()
    }
}

/// The arrival a player actually made in the phrase a settlement verb `a` was planned in (for
/// debt `obligation`): the earliest realized lead or bass attack (the players who perform a
/// resolution) inside the phrase, made while on stage, on a chord tone of a home-tonic harmony.
/// A rehearsal moves the unplayed settlement there; `None` when nobody arrives home in the phrase
/// (the debt then stays open, named).
pub(crate) fn observed_arrival(
    song: &super::song::SongMap,
    perf: &super::performance::PerformancePlan,
    score: &super::score::Score,
    a: &MusicalAction,
    obligation: ObligationId,
) -> Option<Recast> {
    use super::score::Role;
    let ActionCause::Discourse { phrase, .. } = a.cause else {
        return None;
    };
    let phrase = song.plan.form.phrases.get(phrase as usize)?;
    let (start, end) = (phrase.start_beat(), phrase.end_beat());
    let home = perf.region.tonic_pc.rem_euclid(12);
    score
        .notes
        .iter()
        .filter(|n| n.start_beat >= start - 1e-9 && n.start_beat < end - 1e-9)
        .filter_map(|n| {
            let by = match n.role {
                Role::Bass => Agent::Bass,
                Role::Lead => Agent::Lead,
                _ => return None,
            };
            let ctx = perf.context_at(n.start_beat)?;
            (perf.on_stage(by, n.start_beat)
                && ctx.chord.root_pc.rem_euclid(12) == home
                && ctx.chord.contains_pc(n.pitch.rem_euclid(12)))
            .then_some((n.start_beat, by))
        })
        // Earliest; at one onset the bass (its root arrives with the harmony) before the lead.
        .min_by(|x, y| {
            x.0.total_cmp(&y.0)
                .then((x.1 != Agent::Bass).cmp(&(y.1 != Agent::Bass)))
        })
        .map(|(start_beat, by)| Recast {
            obligation,
            start_beat,
            by,
        })
}

/// Why a settled debt has no performed discharge in its settling phrase.
pub(crate) fn open_reason(kind: ObligationKind, rejected_at_rehearsal: bool) -> &'static str {
    if rejected_at_rehearsal {
        return "rehearsed: no player performed the planned discharge in the settling phrase";
    }
    match kind {
        ObligationKind::SuspendedCadence | ObligationKind::HarmonicDeparture => {
            "the settling phrase never sounds the home chord: no arrival can discharge it"
        }
        ObligationKind::MotifQuestion => "no answer is performed in the settling phrase",
        ObligationKind::GrooveDestabilization => {
            "the kit's return is not performed in the settling phrase"
        }
        ObligationKind::RegisterAscent => "no verb witnesses a register homecoming",
    }
}

// Canonical encodings: the trace is part of what a rehearsed performance decided. Present only on
// rehearsed plans, so no historical receipt drifts.
use super::fingerprint::{CanonicalFingerprint, FingerprintWriter};

impl CanonicalFingerprint for VerbOnset {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("rehearsal/VerbOnset/v1");
        match self {
            Self::Metric(p) => {
                w.tag("metric");
                w.field("position", p);
            }
            Self::Exact(bits) => {
                w.tag("exact");
                w.field("bits", bits);
            }
        }
    }
}
impl CanonicalFingerprint for VerbFamily {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("rehearsal/VerbFamily/v1");
        match self {
            Self::Harmonic => w.tag("harmonic"),
            Self::Verb(kind) => {
                w.tag("verb");
                w.field("kind", kind);
            }
        }
    }
}
impl CanonicalFingerprint for CauseKey {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("rehearsal/CauseKey/v1");
        match self {
            Self::Morphism {
                transition,
                morphism,
            } => {
                w.tag("Morphism");
                w.field("transition", transition);
                w.field("morphism", morphism);
            }
            Self::Gesture { slot, gesture } => {
                w.tag("Gesture");
                w.field("slot", slot);
                w.field("gesture", gesture);
            }
            Self::Interaction { call } => {
                w.tag("Interaction");
                w.field("call", &call.as_deref());
            }
            Self::Statement { phrase } => {
                w.tag("Statement");
                w.field("phrase", phrase);
            }
            Self::Discourse { obligation, phrase } => {
                w.tag("Discourse");
                w.field("obligation", obligation);
                w.field("phrase", phrase);
            }
        }
    }
}
impl CanonicalFingerprint for ActionKey {
    fn encode(&self, w: &mut FingerprintWriter) {
        let Self {
            family,
            onset,
            cause,
        } = self;
        w.tag("rehearsal/ActionKey/v1");
        w.field("family", family);
        w.field("onset", onset);
        w.field("cause", cause);
    }
}
impl CanonicalFingerprint for RehearsalOutcome {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("rehearsal/RehearsalOutcome/v1");
        match self {
            Self::Performed => w.tag("performed"),
            Self::Recast { beat, by } => {
                w.tag("recast");
                w.field("beat", beat);
                w.field("by", by);
            }
            Self::Rejected(why) => {
                w.tag("rejected");
                w.field("why", *why);
            }
            Self::Deferred(why) => {
                w.tag("deferred");
                w.field("why", *why);
            }
        }
    }
}
impl CanonicalFingerprint for RehearsedVerb {
    fn encode(&self, w: &mut FingerprintWriter) {
        let Self {
            key,
            kind,
            planned_beat,
            pass,
            outcome,
        } = self;
        w.tag("rehearsal/RehearsedVerb/v1");
        w.field("key", key);
        w.field("kind", kind);
        w.field("planned_beat", planned_beat);
        w.field("pass", pass);
        w.field("outcome", outcome);
    }
}
impl CanonicalFingerprint for OpenDebt {
    fn encode(&self, w: &mut FingerprintWriter) {
        let Self {
            obligation,
            kind,
            by_phrase,
            reason,
        } = self;
        w.tag("rehearsal/OpenDebt/v1");
        w.field("obligation", obligation);
        w.field("kind", kind);
        w.field("by_phrase", by_phrase);
        w.field("reason", *reason);
    }
}
impl CanonicalFingerprint for RehearsalTrace {
    fn encode(&self, w: &mut FingerprintWriter) {
        let Self {
            passes,
            verbs,
            open_debts,
        } = self;
        w.tag("rehearsal/RehearsalTrace/v1");
        w.field("passes", passes);
        w.field("verbs", verbs);
        w.field("open_debts", open_debts);
    }
}
