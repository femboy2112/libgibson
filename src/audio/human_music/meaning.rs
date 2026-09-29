//! **MeaningPlan** — what the listener is meant to learn, to expect, to have fulfilled or missed,
//! and to recognize on return (Round X). A plan of listener EVENTS, not a pleasantness number:
//! whether the result is enjoyable is decided by a human listening, never here.
//!
//! ```text
//!   SemanticTrace ──F──▶ MeaningPlan ◀──μ── SongMap ◀──π── Performance
//! ```
//!
//! - **F** ([`MeaningPlan::target`]) reads the story — how high its pressure peaks, and whether
//!   that peak is ever released — and the song's FORM (the phrase grid, the discourse roles, the
//!   phrases the lead is seated in, the backbone's gesture slots). It never reads the theme or the
//!   chart: those are what a composer chooses, and F is what they are chosen FOR.
//! - **μ** ([`MeaningPlan::observe`]) reads the theme and the chart on that same grid and classifies
//!   what a listener is actually given at each place, in the same vocabulary — plus the events no
//!   plan ever asks for (material developed before it was learned, a miss nobody was led to
//!   expect, surprise spent where none was meant).
//! - **The law** ([`Commutation::check`]): μ(song) = F(trace), event by event, categorically. Round
//!   IX's π says every performance plays the song; this says the song means what the story asked.
//!
//! The listener model is small and transparent on purpose — no corpus, no learned weights:
//! familiarity is counted statements of the thesis as written (learned at [`LEARNED_AFTER`]);
//! expectation is the pointer's dominant pull toward home; surprise at a deflection is how much of
//! the expected arrival it keeps (and whether it also prolongs the pointer); relief is a familiar
//! step away from the miss that does not raise the pull; closure is home, reached by a familiar
//! relation; a chart relation outside the designated miss that is borrowed or remote spends
//! surprise nobody planned.

use super::backbone::{lead_sheet, BackboneTimeline, HarmonicGesture};
use super::context::{common_tones, PullEvidence};
use super::discourse::DiscourseRole;
use super::harmony::ChordSpan;
use super::motif::Motif;
use super::plan::{CompositionPlan, PhraseTarget};
use super::semantic::{EventKind, SemanticTrace};
use super::song::SongMap;
use super::theory::{Chord, Scale};

/// Statements of the thesis, as written, before the listener has learned it — and before a
/// transformation of it can be heard AS a transformation of something known.
pub const LEARNED_AFTER: u32 = 2;

/// A coarse three-way level. Categories, not scores: the law compares them for equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Low,
    Mid,
    High,
}

/// Where a line — or a story — comes to rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Close {
    /// On home: the thing is settled.
    Home,
    /// Anywhere else: the thing is left hanging.
    Open,
}

/// Which strand of the song an event belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lane {
    /// The lead's thematic statements, one site per lead-seated phrase.
    Theme,
    /// The chart, one event per backbone gesture slot.
    Harmony,
}

/// A listener event. The first thirteen are what a plan can ask for; the rest are only ever
/// OBSERVED — what a song does that no plan asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeaningKind {
    /// The thesis's character: how far its salient reach goes.
    Thesis(Level),
    /// The first time the listener hears the thesis — stated as written.
    Learn,
    /// Heard again as written, before it is learned.
    Reinforce,
    /// The culmination states the identity the listener has heard: the hook as payoff.
    Payoff,
    /// The consequent: the thesis's head and rhythm, coming to rest at home or left hanging.
    Answer(Close),
    /// A transformation of learned material.
    Develop,
    /// The thesis returns as written, and the listener already knows it.
    Recognize,
    /// Home is heard before the first expectation is raised.
    Establish,
    /// A Lift closes on a pointer with this pull toward home.
    Prepare(Level),
    /// A prepared, related miss of the expected arrival, with this much surprise.
    Miss(Level),
    /// The window the miss opens: a familiar move away from it that does not raise the pull.
    Open,
    /// Home again, reached by a familiar relation.
    Reset,
    /// Observed only: a variant or transformation of material the listener has not learned.
    Premature,
    /// Observed only: the first pointer sounds before home ever has.
    Unestablished,
    /// Observed only: a deflection nothing pointed at (no dominant before it).
    Unprepared,
    /// Observed only: a "miss" that keeps nothing of the expected arrival.
    Unrelated,
    /// Observed only: the deflection slot lands exactly the expected arrival.
    Arrive,
    /// Observed only: the Open is no familiar step from the miss, or raises the pull.
    NoRelief,
    /// Observed only: the Reset is not home, or is reached by a borrowed/remote relation.
    NoHome,
    /// Observed only: surprise spent outside the designated miss (a borrowed or remote relation).
    Stray,
}

/// The place an event answers for — the key the law compares on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    Thesis,
    Site,
    Establish,
    Prepare,
    Deflect,
    Open,
    Reset,
    Stray,
}

impl MeaningKind {
    fn class(self) -> Class {
        use MeaningKind as K;
        match self {
            K::Thesis(_) => Class::Thesis,
            K::Learn
            | K::Reinforce
            | K::Payoff
            | K::Answer(_)
            | K::Develop
            | K::Recognize
            | K::Premature => Class::Site,
            K::Establish | K::Unestablished => Class::Establish,
            K::Prepare(_) => Class::Prepare,
            K::Miss(_) | K::Unprepared | K::Unrelated | K::Arrive => Class::Deflect,
            K::Open | K::NoRelief => Class::Open,
            K::Reset | K::NoHome => Class::Reset,
            K::Stray => Class::Stray,
        }
    }
}

/// One listener event, at a place in the song: a phrase index (theme lane) or a backbone slot
/// index (harmony lane), with the beat it starts on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeaningEvent {
    pub lane: Lane,
    pub at: u32,
    pub beat: f64,
    pub kind: MeaningKind,
}

/// **F(trace)**: what the listener should learn, expect, miss, receive and recognize — the target
/// a song is composed toward (Round X), derived from the story and the form alone.
#[derive(Debug, Clone, PartialEq)]
pub struct MeaningPlan {
    /// How high the story's pressure peaks: how far the thesis reaches, how hard the miss lands.
    pub arc: Level,
    /// Whether the story releases its peak ([`Close::Home`]) or leaves it hanging.
    pub resolution: Close,
    pub events: Vec<MeaningEvent>,
}

/// The lead-seated phrases, in order — where a song states its thematic material.
fn lead_sites(plan: &CompositionPlan) -> Vec<PhraseTarget> {
    plan.targets()
        .into_iter()
        .filter(|t| plan.arrangement.at(t.phrase.ix as usize).lead.is_audible())
        .collect()
}

/// The backbone slots that start inside the piece, with their indices.
fn live_slots<'a>(
    bb: &'a BackboneTimeline,
    plan: &CompositionPlan,
) -> impl Iterator<Item = (usize, &'a super::backbone::GestureSlot)> {
    let end = plan.form.total_beats;
    bb.slots
        .iter()
        .enumerate()
        .filter(move |(_, s)| s.start_beat() < end - 1e-9)
}

/// The story's arc (its peak semantic pressure: below 0.5 low, below 0.85 mid, else high) and
/// resolution (settled when the peak is low or a confirmation / resolved section follows it).
pub fn story_arc(trace: &SemanticTrace) -> (Level, Close) {
    let (i, peak) = trace
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| (i, e.state.pressure()))
        .fold((0, 0.0f32), |a, b| if b.1 > a.1 { b } else { a });
    let arc = if peak < 0.5 {
        Level::Low
    } else if peak < 0.85 {
        Level::Mid
    } else {
        Level::High
    };
    let released = peak < 0.5
        || trace
            .events
            .iter()
            .skip(i + 1)
            .any(|e| matches!(e.kind, EventKind::Confirmation | EventKind::SectionResolved));
    (arc, if released { Close::Home } else { Close::Open })
}

impl MeaningPlan {
    /// **F**: the listener plan for `trace` over the song's form `plan`. Theme: the first lead
    /// statement teaches the thesis; until it is learned every statement reinforces it; the
    /// culmination pays it off; an answer is its consequent, settling as the story does; a
    /// departure develops it only once learned; a restatement is recognized once learned.
    /// Harmony: home before the first pointer; every Lift closes on a strong pointer; every
    /// Deflect is a prepared, related miss as hard as the story's arc; every Open relieves;
    /// every Reset is home; no surprise anywhere else.
    pub fn target(trace: &SemanticTrace, plan: &CompositionPlan) -> MeaningPlan {
        use DiscourseRole as R;
        use MeaningKind as K;
        let (arc, resolution) = story_arc(trace);
        let mut events = Vec::new();
        let mut known = 0u32;
        for (n, t) in lead_sites(plan).iter().enumerate() {
            let (at, beat) = (t.phrase.ix, t.start_beat());
            if n == 0 {
                events.push(MeaningEvent {
                    lane: Lane::Theme,
                    at,
                    beat,
                    kind: K::Thesis(arc),
                });
            }
            let kind = if known == 0 {
                K::Learn
            } else {
                match t.goal.role {
                    R::Culminate => K::Payoff,
                    R::Answer => K::Answer(resolution),
                    R::Establish | R::Restate | R::Return if known >= LEARNED_AFTER => K::Recognize,
                    R::Establish | R::Restate | R::Return => K::Reinforce,
                    _ if known >= LEARNED_AFTER => K::Develop,
                    _ => K::Reinforce,
                }
            };
            if matches!(kind, K::Learn | K::Reinforce | K::Payoff | K::Recognize) {
                known += 1;
            }
            events.push(MeaningEvent {
                lane: Lane::Theme,
                at,
                beat,
                kind,
            });
        }
        if let Some(bb) = &plan.backbone {
            let mut first_lift = true;
            for (i, s) in live_slots(bb, plan) {
                let at = i as u32;
                let beat = s.start_beat();
                if s.gesture == HarmonicGesture::Lift && first_lift {
                    first_lift = false;
                    events.push(MeaningEvent {
                        lane: Lane::Harmony,
                        at,
                        beat,
                        kind: K::Establish,
                    });
                }
                let kind = match s.gesture {
                    HarmonicGesture::Lift => K::Prepare(Level::High),
                    HarmonicGesture::Deflect => K::Miss(arc),
                    HarmonicGesture::Open => K::Open,
                    HarmonicGesture::Reset => K::Reset,
                };
                events.push(MeaningEvent {
                    lane: Lane::Harmony,
                    at,
                    beat,
                    kind,
                });
            }
        }
        MeaningPlan {
            arc,
            resolution,
            events,
        }
    }

    /// **μ**: what `song` actually gives a listener, event by event, read from its theme sites and
    /// its lead sheet on the song's own grid — without the story.
    pub fn observe(song: &SongMap) -> Observation {
        let mut events = observe_theme(song);
        events.extend(observe_harmony(song));
        Observation { events }
    }
}

/// How a statement relates to the thesis a listener may know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeRelation {
    /// The thesis as written (degrees and rhythm).
    Literal,
    /// The thesis's rhythm and head (its first half), continued otherwise: recognizably it.
    Variant,
    /// Anything else: transposed, inverted, fragmented, re-rhythmed.
    Transformed,
}

impl ThemeRelation {
    /// The relation of `m` to `thesis`.
    pub fn of(thesis: &Motif, m: &Motif) -> ThemeRelation {
        let head = thesis.len().div_ceil(2);
        if m.degrees == thesis.degrees && m.rhythm == thesis.rhythm {
            ThemeRelation::Literal
        } else if m.rhythm == thesis.rhythm && m.degrees[..head] == thesis.degrees[..head] {
            ThemeRelation::Variant
        } else {
            ThemeRelation::Transformed
        }
    }
}

/// Where a line comes to rest: on the tonic (any octave) it settles, anywhere else it hangs.
pub fn landing(m: &Motif) -> Close {
    match m.degrees.last() {
        Some(d) if d.rem_euclid(7) == 0 => Close::Home,
        _ => Close::Open,
    }
}

/// The level of a line's largest leap, in scale steps: a third or less is no salient reach (low),
/// a fourth or fifth is one (mid), a sixth or more is a wide one (high).
pub fn reach(m: &Motif) -> Level {
    let widest = m
        .degrees
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .max()
        .unwrap_or(0);
    match widest {
        0..=2 => Level::Low,
        3..=4 => Level::Mid,
        _ => Level::High,
    }
}

/// How one chord moves to the next, in the listener model's terms (reference frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// The same root (a colour change or a pedal).
    Prolong,
    /// Two or more common tones between diatonic chords (a third relation).
    Neighbour,
    /// Root motion by a fourth or fifth between diatonic chords.
    Fifth,
    /// Root motion by step between diatonic chords.
    Step,
    /// A chord outside the scale, sharing at least one tone with its neighbour.
    Borrowed,
    /// Anything else: nothing shared, no fifth, no step.
    Remote,
}

impl Family {
    /// The motion from `a` to `b` in `region`.
    pub fn of(a: &Chord, b: &Chord, region: &Scale) -> Family {
        if a.root_pc == b.root_pc {
            return Family::Prolong;
        }
        let common = common_tones(a, b);
        let diatonic = |c: &Chord| c.pitch_classes().iter().all(|&pc| region.contains_pc(pc));
        if !diatonic(a) || !diatonic(b) {
            return if common >= 1 {
                Family::Borrowed
            } else {
                Family::Remote
            };
        }
        let iv = (b.root_pc - a.root_pc).rem_euclid(12);
        if iv == 5 || iv == 7 {
            Family::Fifth
        } else if common >= 2 {
            Family::Neighbour
        } else if matches!(iv, 1 | 2 | 10 | 11) {
            Family::Step
        } else {
            Family::Remote
        }
    }

    /// A relation a listener hears as ordinary syntax (it spends no surprise).
    pub fn is_familiar(self) -> bool {
        matches!(
            self,
            Family::Prolong | Family::Neighbour | Family::Fifth | Family::Step
        )
    }
}

/// What μ saw at an event — the evidence a diagnostic prints and a test can pin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Witness {
    /// The thesis's widest leap, in scale steps.
    Thesis { widest: i32 },
    /// A theme site: its relation to the thesis, the literal statements heard before it, and
    /// where it comes to rest.
    Site {
        relation: ThemeRelation,
        familiarity: u32,
        landing: Close,
    },
    /// Home before the first pointer: when the tonic and the first pointer are first heard.
    Home {
        tonic_at: Option<f64>,
        pointer_at: Option<f64>,
    },
    /// A chord and the chord it follows: the move's family, the pull toward home before and after
    /// (dominant evidence, 0..1), the tones the heard chord keeps of the expected arrival, and how
    /// certain the expectation was (home already heard, and a strong pointer).
    Move {
        from: Chord,
        to: Chord,
        family: Family,
        pull_before: f32,
        pull_after: f32,
        kept: u8,
        certainty: Level,
    },
}

/// One observed event and its evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Witnessed {
    pub event: MeaningEvent,
    pub witness: Witness,
}

/// **μ(song)**: the listener events a song actually gives, with their evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub events: Vec<Witnessed>,
}

impl Observation {
    /// The observed kind at a place (lane, index, class), if any.
    fn kind_at(&self, lane: Lane, at: u32, class: Class) -> Option<&Witnessed> {
        self.events
            .iter()
            .find(|w| w.event.lane == lane && w.event.at == at && w.event.kind.class() == class)
    }

    /// Every observed event of `kind`.
    pub fn count(&self, kind: MeaningKind) -> usize {
        self.events.iter().filter(|w| w.event.kind == kind).count()
    }
}

fn observe_theme(song: &SongMap) -> Vec<Witnessed> {
    use DiscourseRole as R;
    use MeaningKind as K;
    let thesis = &song.thematic.bank.identity;
    let mut out = Vec::new();
    let mut known = 0u32;
    for (n, site) in song.thematic.sites.iter().enumerate() {
        let beat = song
            .plan
            .form
            .phrases
            .iter()
            .find(|p| p.ix == site.phrase)
            .map(|p| p.start_beat())
            .unwrap_or(0.0);
        if n == 0 {
            out.push(Witnessed {
                event: MeaningEvent {
                    lane: Lane::Theme,
                    at: site.phrase,
                    beat,
                    kind: K::Thesis(reach(thesis)),
                },
                witness: Witness::Thesis {
                    widest: thesis
                        .degrees
                        .windows(2)
                        .map(|w| (w[1] - w[0]).abs())
                        .max()
                        .unwrap_or(0),
                },
            });
        }
        let relation = ThemeRelation::of(thesis, &site.motif);
        let kind = match relation {
            ThemeRelation::Literal if known == 0 => K::Learn,
            ThemeRelation::Literal => match site.role {
                R::Culminate => K::Payoff,
                R::Establish | R::Restate | R::Return if known >= LEARNED_AFTER => K::Recognize,
                _ => K::Reinforce,
            },
            ThemeRelation::Variant if known >= 1 && site.role == R::Answer => {
                K::Answer(landing(&site.motif))
            }
            _ if known >= LEARNED_AFTER => K::Develop,
            _ => K::Premature,
        };
        out.push(Witnessed {
            event: MeaningEvent {
                lane: Lane::Theme,
                at: site.phrase,
                beat,
                kind,
            },
            witness: Witness::Site {
                relation,
                familiarity: known,
                landing: landing(&site.motif),
            },
        });
        if relation == ThemeRelation::Literal {
            known += 1;
        }
    }
    out
}

/// Surprise at a deflection that keeps `kept` tones of the expected arrival, reached by `family`
/// from the pointer: keeping two tones AND prolonging the pointer barely registers (low); keeping
/// two is the textbook deceptive miss (mid); keeping one lands hard (high).
fn surprise(kept: u8, family: Family) -> Level {
    match (kept, family) {
        (2.., Family::Neighbour | Family::Prolong) => Level::Low,
        (2.., _) => Level::Mid,
        _ => Level::High,
    }
}

fn level_of_pull(p: f32) -> Level {
    if p >= 0.9 {
        Level::High
    } else if p >= 0.5 {
        Level::Mid
    } else {
        Level::Low
    }
}

fn observe_harmony(song: &SongMap) -> Vec<Witnessed> {
    use MeaningKind as K;
    let (Some(bb), Some(hm)) = (&song.plan.backbone, &song.harmonic) else {
        return Vec::new();
    };
    let region = Scale::new(0, song.frame);
    let cell = hm.cell.realize(&region);
    let (home, expected) = (cell.expected.root_pc, cell.expected);
    let end = song.plan.form.total_beats;
    let sheet: Vec<ChordSpan> = lead_sheet(bb, hm, song.frame)
        .into_iter()
        .filter(|s| s.start_beat < end - 1e-9)
        .collect();
    let pull = |c: &Chord| PullEvidence::of(c, home).strength();
    let before = |beat: f64| {
        sheet
            .iter()
            .rev()
            .find(|s| s.start_beat < beat - 1e-9)
            .map(|s| s.chord)
    };
    let at_beat = |beat: f64| {
        sheet
            .iter()
            .find(|s| (s.start_beat - beat).abs() < 1e-6)
            .map(|s| s.chord)
    };
    let tonic_heard_by = |beat: f64| {
        sheet
            .iter()
            .any(|s| s.start_beat < beat - 1e-9 && s.chord.root_pc == home)
    };
    let mv = |from: Chord, to: Chord, beat: f64| {
        let p = pull(&from);
        let certainty = match (tonic_heard_by(beat), p >= 0.9) {
            (true, true) => Level::High,
            (false, false) => Level::Low,
            _ => Level::Mid,
        };
        Witness::Move {
            from,
            to,
            family: Family::of(&from, &to, &region),
            pull_before: p,
            pull_after: pull(&to),
            kept: common_tones(&to, &expected),
            certainty,
        }
    };

    let mut out = Vec::new();
    let slots: Vec<(usize, &super::backbone::GestureSlot)> = live_slots(bb, &song.plan).collect();
    let mut deflect_starts: Vec<f64> = Vec::new();
    let mut first_lift = true;
    for &(i, s) in &slots {
        let (at, beat) = (i as u32, s.start_beat());
        let ev = |kind| MeaningEvent {
            lane: Lane::Harmony,
            at,
            beat,
            kind,
        };
        let slot_end = s.end_beat().min(end);
        let last_in_slot = sheet
            .iter()
            .rev()
            .find(|sp| sp.start_beat >= beat - 1e-9 && sp.start_beat < slot_end - 1e-9)
            .map(|sp| (sp.start_beat, sp.chord));
        let Some(first) = at_beat(beat) else {
            continue;
        };
        match s.gesture {
            HarmonicGesture::Lift => {
                let (p_at, pointer) = last_in_slot.unwrap_or((beat, first));
                if first_lift {
                    first_lift = false;
                    let tonic_at = sheet
                        .iter()
                        .find(|sp| sp.chord.root_pc == home)
                        .map(|sp| sp.start_beat);
                    let kind = match tonic_at {
                        Some(t) if t < p_at - 1e-9 => K::Establish,
                        _ => K::Unestablished,
                    };
                    out.push(Witnessed {
                        event: ev(kind),
                        witness: Witness::Home {
                            tonic_at,
                            pointer_at: Some(p_at),
                        },
                    });
                }
                let from = before(p_at).unwrap_or(pointer);
                out.push(Witnessed {
                    event: ev(K::Prepare(level_of_pull(pull(&pointer)))),
                    witness: mv(from, pointer, p_at),
                });
            }
            HarmonicGesture::Deflect => {
                deflect_starts.push(beat);
                let from = before(beat).unwrap_or(first);
                let w = mv(from, first, beat);
                let kind = if pull(&from) < 0.5 {
                    K::Unprepared
                } else if first.root_pc == home {
                    K::Arrive
                } else {
                    match w {
                        Witness::Move { kept: 0, .. } => K::Unrelated,
                        Witness::Move { kept, family, .. } => K::Miss(surprise(kept, family)),
                        _ => unreachable!(),
                    }
                };
                out.push(Witnessed {
                    event: ev(kind),
                    witness: w,
                });
            }
            HarmonicGesture::Open => {
                let from = before(beat).unwrap_or(first);
                let w = mv(from, first, beat);
                let relieves = matches!(w, Witness::Move { family, pull_before, pull_after, .. }
                    if family.is_familiar() && pull_after <= pull_before + 1e-6);
                out.push(Witnessed {
                    event: ev(if relieves { K::Open } else { K::NoRelief }),
                    witness: w,
                });
            }
            HarmonicGesture::Reset => {
                let from = before(beat).unwrap_or(first);
                let w = mv(from, first, beat);
                let home_again = first.root_pc == home
                    && matches!(w, Witness::Move { family, .. } if family.is_familiar());
                out.push(Witnessed {
                    event: ev(if home_again { K::Reset } else { K::NoHome }),
                    witness: w,
                });
            }
        }
    }

    // Surprise outside the designated miss: every chart move not into a deflection and not out of
    // the deflecting chord itself, that is borrowed or remote.
    for pair in sheet.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let into_miss = deflect_starts
            .iter()
            .any(|&d| (b.start_beat - d).abs() < 1e-6);
        let out_of_miss = deflect_starts
            .iter()
            .any(|&d| (a.start_beat - d).abs() < 1e-6);
        let family = Family::of(&a.chord, &b.chord, &region);
        if into_miss || out_of_miss || family.is_familiar() {
            continue;
        }
        let at = slots
            .iter()
            .rev()
            .find(|(_, s)| s.start_beat() <= b.start_beat + 1e-9)
            .map(|(i, _)| *i as u32)
            .unwrap_or(0);
        out.push(Witnessed {
            event: MeaningEvent {
                lane: Lane::Harmony,
                at,
                beat: b.start_beat,
                kind: K::Stray,
            },
            witness: mv(a.chord, b.chord, b.start_beat),
        });
    }
    out
}

/// Who can repair a divergence: the composer (the theme and the chart it chose) or the form it
/// was handed (roles, seats and slots — Round IX's plan, shared by every composer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Composer,
    Form,
}

/// A place where the song does not mean what the story asked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Divergence {
    pub lane: Lane,
    pub at: u32,
    pub beat: f64,
    /// What F asked for here (`None`: nothing — the song spent surprise unasked).
    pub wanted: Option<MeaningKind>,
    /// What μ heard here (`None`: nothing — the song left the event out).
    pub heard: Option<MeaningKind>,
    pub owner: Owner,
}

/// **The law, checked**: F(trace) against μ(song), event by event. Exact categorical equality at
/// every place F names, and no observed-only surprise anywhere F does not.
#[derive(Debug, Clone, PartialEq)]
pub struct Commutation {
    pub target: MeaningPlan,
    pub observed: Observation,
    pub divergences: Vec<Divergence>,
    /// Target events compared (a pass over nothing is visible).
    pub checked: usize,
}

impl Commutation {
    /// Check `song` against the plan its own story and form ask for.
    pub fn check(song: &SongMap) -> Commutation {
        Commutation::against(MeaningPlan::target(&song.trace, &song.plan), song)
    }

    /// Check `song` against an explicit `target` (a control can hand it another story's plan).
    pub fn against(target: MeaningPlan, song: &SongMap) -> Commutation {
        let observed = MeaningPlan::observe(song);
        let slots = song.plan.backbone.as_ref().map(|b| b.slots.as_slice());
        let owner = |lane: Lane, at: u32, heard: Option<MeaningKind>| {
            // A deflection the form placed with no Lift before it cannot be prepared by any chart.
            let unliftable = lane == Lane::Harmony
                && heard == Some(MeaningKind::Unprepared)
                && slots.is_some_and(|s| {
                    at == 0 || s[at as usize - 1].gesture != HarmonicGesture::Lift
                });
            if unliftable {
                Owner::Form
            } else {
                Owner::Composer
            }
        };
        let mut divergences = Vec::new();
        for e in &target.events {
            let heard = observed
                .kind_at(e.lane, e.at, e.kind.class())
                .map(|w| w.event.kind);
            if heard != Some(e.kind) {
                divergences.push(Divergence {
                    lane: e.lane,
                    at: e.at,
                    beat: e.beat,
                    wanted: Some(e.kind),
                    heard,
                    owner: owner(e.lane, e.at, heard),
                });
            }
        }
        for w in observed
            .events
            .iter()
            .filter(|w| w.event.kind == MeaningKind::Stray)
        {
            divergences.push(Divergence {
                lane: w.event.lane,
                at: w.event.at,
                beat: w.event.beat,
                wanted: None,
                heard: Some(MeaningKind::Stray),
                owner: Owner::Composer,
            });
        }
        Commutation {
            checked: target.events.len(),
            target,
            observed,
            divergences,
        }
    }

    /// Whether μ(song) = F(trace) on every modelled event.
    pub fn commutes(&self) -> bool {
        self.divergences.is_empty()
    }

    /// Divergences the composer owns (its theme and chart could have met the plan).
    pub fn composer_divergences(&self) -> usize {
        self.divergences
            .iter()
            .filter(|d| d.owner == Owner::Composer)
            .count()
    }

    /// The expectation receipt: every target event beside what was heard there, with the evidence.
    pub fn report(&self) -> String {
        let mut s = format!(
            "MeaningPlan: arc {:?}, resolution {:?} — {} events checked, {} divergent ({} composer-owned)\n",
            self.target.arc,
            self.target.resolution,
            self.checked,
            self.divergences.len(),
            self.composer_divergences()
        );
        for e in &self.target.events {
            let seen = self.observed.kind_at(e.lane, e.at, e.kind.class());
            let verdict = if seen.map(|w| w.event.kind) == Some(e.kind) {
                "ok"
            } else {
                "DIVERGES"
            };
            s.push_str(&format!(
                "  {:<7} {} @{:>5.1}  want {:<14} heard {:<14} {verdict}  {}\n",
                format!("{:?}", e.lane),
                match e.lane {
                    Lane::Theme => format!("p{:<2}", e.at),
                    Lane::Harmony => format!("s{:<2}", e.at),
                },
                e.beat,
                format!("{:?}", e.kind),
                seen.map(|w| format!("{:?}", w.event.kind))
                    .unwrap_or_else(|| "-".into()),
                seen.map(|w| describe(&w.witness)).unwrap_or_default(),
            ));
        }
        for w in self
            .observed
            .events
            .iter()
            .filter(|w| w.event.kind == MeaningKind::Stray)
        {
            s.push_str(&format!(
                "  Harmony s{:<2} @{:>5.1}  want -              heard Stray          DIVERGES  {}\n",
                w.event.at,
                w.event.beat,
                describe(&w.witness)
            ));
        }
        s
    }
}

fn chord_name(c: &Chord) -> String {
    const N: [&str; 12] = [
        "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
    ];
    format!("{}{:?}", N[c.root_pc.rem_euclid(12) as usize], c.quality)
}

fn describe(w: &Witness) -> String {
    match *w {
        Witness::Thesis { widest } => format!("widest leap {widest} steps"),
        Witness::Site {
            relation,
            familiarity,
            landing,
        } => format!("{relation:?}, heard as written {familiarity}x before, lands {landing:?}"),
        Witness::Home {
            tonic_at,
            pointer_at,
        } => format!(
            "tonic first @{} · first pointer @{}",
            tonic_at.map_or("never".into(), |b| format!("{b:.1}")),
            pointer_at.map_or("never".into(), |b| format!("{b:.1}"))
        ),
        Witness::Move {
            from,
            to,
            family,
            pull_before,
            pull_after,
            kept,
            certainty,
        } => format!(
            "{} -> {} ({family:?}) · pull {pull_before:.2} -> {pull_after:.2} · keeps {kept} of home · certainty {certainty:?}",
            chord_name(&from),
            chord_name(&to)
        ),
    }
}
