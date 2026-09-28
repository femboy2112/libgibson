//! **Interaction** — the lead's statements, the calls anyone can make, and the time-local
//! responses to them (Round VII; split out of [`super::performance`] in Round VIIb).
//!
//! A [`Call`] is something one player said that others may answer; a [`Response`] is a planned
//! answer inside a free response window, chosen with [`InteractionMemory`] so the band does not
//! repeat the same initiator / responder / latency / placement / transform. The
//! [`ResponseMode::Clockwork`] probe is the lifeless control.

use super::action::{ActionCause, ActionKind, ActionPlan, Agent, MusicalAction};
use super::discourse::DiscourseRole;
use super::form::BEATS_PER_BAR;
use super::harmony::ChordSpan;
use super::ids::ActionId;
use super::language::MusicalLanguage;
use super::motif::{Handoff, Motif, MotifBank, ThematicTrajectory};
use super::performance::AccentGrid;
use super::plan::{ArrangementRole, CompositionPlan};
use super::rng::Rng;
use super::score::Role;

/// How responses are planned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseMode {
    /// Time-local interaction planning with memory (the real model).
    Free,
    /// The adversarial calibration probe: every answer by the same responder at the same metric
    /// offset with the same transform — legal and lifeless.
    Clockwork,
}

/// How a response relates to its call's material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Transform {
    /// Repeat the call's tail.
    Quote,
    /// Continue the call's contour past where it stopped.
    Complete,
    /// Answer with the contour mirrored.
    Invert,
    /// The call's head, diminished.
    Compress,
    /// The call's rhythm on the responder's own pitches.
    Echo,
    /// A deliberate non-answer: the space is left open.
    Silence,
}

impl Transform {
    /// A short lowercase label.
    pub fn label(self) -> &'static str {
        match self {
            Transform::Quote => "quote",
            Transform::Complete => "complete",
            Transform::Invert => "invert",
            Transform::Compress => "compress",
            Transform::Echo => "echo",
            Transform::Silence => "silence",
        }
    }
}

/// A planned lead statement.
#[derive(Debug, Clone, PartialEq)]
pub struct LeadStatement {
    pub phrase: u32,
    pub start_beat: f64,
    pub motif: Motif,
    pub handoff: Handoff,
    pub role: DiscourseRole,
    pub energy: f32,
    pub register: f32,
    pub is_rupture: bool,
    /// The call (or answer) action this statement is, when actions are planned.
    pub call: Option<ActionId>,
}

impl LeadStatement {
    /// One past the statement's last beat.
    pub fn end_beat(&self) -> f64 {
        self.start_beat + self.motif.total_beats() as f64
    }
}

/// A call: something one player said that others may answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Call {
    /// The action id of the call.
    pub action: ActionId,
    pub initiator: Agent,
    pub start_beat: f64,
    pub end_beat: f64,
    /// The lead statement it is, when the lead called.
    pub statement: Option<usize>,
}

/// A planned response.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Response {
    /// The action id of the response (`None` for a deliberate silence, which is not an action).
    pub action: Option<ActionId>,
    pub responder: Agent,
    pub start_beat: f64,
    pub dur_beats: f64,
    /// Beats from the call's end to the response's start (negative = overlapping).
    pub latency: f64,
    pub overlap: bool,
    pub transform: Transform,
    /// Whether the response window crosses into a new harmony.
    pub crosses_chord: bool,
}

/// One call with its (possibly absent) response.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interaction {
    pub call: Call,
    pub response: Option<Response>,
}

/// An interaction signature, for memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Signature {
    initiator: Agent,
    responder: Agent,
    latency_q: i8,
    step: u8,
    transform: Transform,
}

/// What the band has already done, so repeated interactions don't reuse the same initiator,
/// responder, latency, metric placement and transform. Deterministic does not mean rigid.
#[derive(Debug, Clone, Default)]
pub struct InteractionMemory {
    seen: Vec<Signature>,
}

impl InteractionMemory {
    fn penalty(&self, s: &Signature) -> f32 {
        let mut p = 0.0;
        for o in &self.seen {
            if o == s {
                p += 1.5;
            }
            if o.responder == s.responder {
                p += 0.35;
            }
            if o.latency_q == s.latency_q {
                p += 0.3;
            }
            if o.step == s.step {
                p += 0.3;
            }
            if o.transform == s.transform {
                p += 0.25;
            }
        }
        p
    }
    /// A bonus (negative cost) for a kind of timing the band has not used yet in this piece —
    /// overlapping, immediate, or delayed — so memory seeks variety, not only avoids repeats.
    fn novelty(&self, latency_q: i8) -> f32 {
        let cat = |q: i8| match q {
            q if q < 0 => 0u8,
            0 | 1 => 1,
            _ => 2,
        };
        if self.seen.iter().any(|o| cat(o.latency_q) == cat(latency_q)) {
            0.0
        } else {
            -0.45
        }
    }
    fn record(&mut self, s: Signature) {
        self.seen.push(s);
    }
}

/// Plan the lead statements, the calls, and the responses.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_interactions(
    plan: &CompositionPlan,
    actions: &mut ActionPlan,
    accent: &AccentGrid,
    chords: &[ChordSpan],
    bank: &MotifBank,
    lang: &MusicalLanguage,
    mode: ResponseMode,
    interact: bool,
    seed: u64,
) -> (Vec<LeadStatement>, Vec<Interaction>) {
    let mut rng = Rng::new(seed ^ 0x1A7E_4C71);
    let mut memory = InteractionMemory::default();
    let mut interactions: Vec<Interaction> = Vec::new();
    let mut statements: Vec<LeadStatement> = Vec::new();
    let total_beats = plan.form.total_bars as f64 * BEATS_PER_BAR;
    let two_bar = 2.0 * BEATS_PER_BAR;

    // Non-lead figure calls: pickups/fragments initiated by bass or keys (distributed agency).
    let mut figure_calls: Vec<Call> = Vec::new();
    if lang.distributed_agency && interact && mode == ResponseMode::Free {
        for a in &actions.actions {
            // Pickups, fragments and a keys-led re-entry are figures a player STATES — calls that
            // open a response window (initiative is distributed, not the lead's alone).
            if matches!(
                a.kind,
                ActionKind::Pickup | ActionKind::Fragment | ActionKind::ReEntry
            ) && matches!(a.initiator, Agent::Bass | Agent::Keys)
            {
                let len = a.dur_beats.clamp(1.0, 2.0);
                figure_calls.push(Call {
                    action: a.id,
                    initiator: a.initiator,
                    start_beat: a.start_beat,
                    end_beat: (a.start_beat + len).min(total_beats),
                    statement: None,
                });
            }
        }
    }

    // Lead statements along the audible line, developed by the thematic trajectory; start offsets
    // vary lawfully (a pickup into the bar, on the beat, a displaced entry) instead of always
    // beginning on the phrase downbeat.
    let mut traj = ThematicTrajectory::new(bank);
    let mut offset_memory: Vec<i32> = Vec::new();
    let mut thesis_stated = false;
    // Statement starts whose Fragment verb passes to the band (before the thesis was stated).
    let mut band_fragments: Vec<f64> = Vec::new();
    for t in plan.targets() {
        let phrase = t.phrase;
        if !plan.arrangement.at(phrase.ix as usize).lead.is_audible() {
            continue;
        }
        let (motif, handoff) = traj.next_for(t.goal.role);
        let len = motif.total_beats() as f64;
        if len < 1e-6 {
            continue;
        }
        let pe = phrase.end_beat();
        let mut at = phrase.start_beat();
        let mut guard = 0;
        while at + len <= pe + 1e-6 && guard < 32 {
            // A figure call from bass/keys just before this statement: the lead ANSWERS it.
            let answering = figure_calls
                .iter()
                .find(|c| c.end_beat > at - 2.0 && c.end_beat <= at + 1.0 + 1e-6)
                .copied();
            let offset = if let Some(c) = answering {
                // Enter after the figure (a short lawful latency), never on top of it.
                ((c.end_beat - at).max(0.0) + 0.5)
                    .min(pe - at - len)
                    .max(0.0)
            } else if lang.id == super::language::LanguageId::Simple {
                0.0
            } else {
                // Choose among lawful entries by the grid's pickup/syncopation weight and memory.
                let opts: [(f64, i32); 4] = [(0.0, 0), (-0.5, -1), (0.5, 1), (1.0, 2)];
                let mut best = (f32::INFINITY, 0.0);
                for (o, q) in opts {
                    let s = at + o;
                    if s < 0.0 || s + len > pe + 0.5 + 1e-6 {
                        continue;
                    }
                    let w = accent.at_beat(s);
                    let fit = -(w.structural + w.pickup + w.syncopation);
                    let used = offset_memory
                        .iter()
                        .rev()
                        .take(3)
                        .filter(|&&u| u == q)
                        .count();
                    let cost = fit + 0.6 * used as f32 + rng.range_f32(0.0, 0.1);
                    if cost < best.0 {
                        best = (cost, o);
                    }
                }
                offset_memory.push((best.1 * 2.0).round() as i32);
                best.1
            };
            let start = (at + offset).max(0.0);
            if start + len > total_beats + 1e-6 {
                break;
            }
            // A lead-initiated Fragment action in force here fragments THIS statement: the verb
            // reaches the thematic material instead of only nudging a scalar.
            // Development presupposes exposition: the lead fragments only material it has already
            // stated in full. A Fragment before that passes to the band — the answer to this
            // statement is then a compressed fragment of it (see the response planner).
            let fragment_here = actions.actions.iter().any(|a| {
                a.kind == ActionKind::Fragment && a.initiator == Agent::Lead && a.covers(start)
            });
            // ...and never the hook or a return of the thesis: those are the song's persistent
            // identity, so the band carries the fragmentation around them instead.
            let identity_role = matches!(
                t.goal.role,
                DiscourseRole::Culminate
                    | DiscourseRole::Restate
                    | DiscourseRole::Return
                    | DiscourseRole::Establish
            );
            let fragmenting = fragment_here && thesis_stated && !identity_role;
            if fragment_here && !fragmenting {
                band_fragments.push(start);
            }
            let motif = if fragmenting && motif.len() > 2 {
                motif.fragment(motif.len().div_ceil(2).max(2))
            } else {
                motif.clone()
            };
            let len = motif.total_beats() as f64;
            let motif_is_full = motif.len() >= bank.identity.len();
            let call_id = if !interact {
                None
            } else {
                Some(actions.push(MusicalAction {
                    id: ActionId(0),
                    cause: match answering {
                        Some(c) => ActionCause::Interaction { call: c.action },
                        None => ActionCause::Statement { phrase: phrase.ix },
                    },
                    initiator: Agent::Lead,
                    start_beat: start,
                    dur_beats: len,
                    kind: if answering.is_some() {
                        ActionKind::Answer
                    } else {
                        ActionKind::Call
                    },
                    target_beat: None,
                    responders: vec![Agent::Keys, Agent::Bass, Agent::Drums],
                    binding: None,
                    pays: answering.map(|c| c.action),
                }))
            };
            if let Some(c) = answering {
                // Record the figure call as answered by the lead.
                let lat = start - c.end_beat;
                interactions.push(Interaction {
                    call: c,
                    response: Some(Response {
                        action: call_id,
                        responder: Agent::Lead,
                        start_beat: start,
                        dur_beats: len,
                        latency: lat,
                        overlap: lat < 0.0,
                        transform: Transform::Complete,
                        crosses_chord: crosses(chords, c.end_beat, start + 1.0),
                    }),
                });
                memory.record(Signature {
                    initiator: c.initiator,
                    responder: Agent::Lead,
                    latency_q: (lat * 2.0).round() as i8,
                    step: AccentGrid::step_of(start).1 as u8,
                    transform: Transform::Complete,
                });
            }
            statements.push(LeadStatement {
                phrase: phrase.ix,
                start_beat: start,
                motif,
                handoff,
                role: t.goal.role,
                energy: t.goal.energy_target,
                register: t.goal.register_target,
                is_rupture: phrase.is_rupture,
                call: call_id,
            });
            if motif_is_full {
                thesis_stated = true;
            }
            let after = start + len;
            let boundary = (after / two_bar).ceil() * two_bar;
            at = if boundary > after + 1e-6 {
                boundary
            } else {
                after
            };
            guard += 1;
        }
    }
    // Figure calls the lead did not answer still get planned responses below.
    let answered: Vec<ActionId> = interactions.iter().map(|i| i.call.action).collect();

    // Responses to the lead's statements (and to unanswered figures).
    if !interact {
        // The mood-without-action probe: the lead still sings, but nobody answers anybody.
        return (statements, interactions);
    }
    let mut calls: Vec<Call> = statements
        .iter()
        .enumerate()
        .filter_map(|(si, s)| {
            Some(Call {
                action: s.call?,
                initiator: Agent::Lead,
                start_beat: s.start_beat,
                end_beat: s.end_beat(),
                statement: Some(si),
            })
        })
        .collect();
    calls.extend(
        figure_calls
            .iter()
            .filter(|c| !answered.contains(&c.action))
            .copied(),
    );
    calls.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));

    for (ci, call) in calls.iter().enumerate() {
        // The room before the next lead statement begins (the answer must not trample it, unless
        // it deliberately overlaps its tail).
        let next_start = statements
            .iter()
            .map(|s| s.start_beat)
            .filter(|&s| s > call.start_beat + 1e-6)
            .fold(total_beats, f64::min);
        let gap = next_start - call.end_beat;
        let phrase_ix = plan
            .form
            .phrase_at(call.end_beat.min(total_beats - 1e-6))
            .ix as usize;
        let arr = plan.arrangement.at(phrase_ix);
        let audible = |a: Agent| match a {
            Agent::Keys => arr.role_for(Role::Keys) != ArrangementRole::Silent,
            Agent::Bass => arr.role_for(Role::Bass) != ArrangementRole::Silent,
            Agent::Drums => arr.drums != ArrangementRole::Silent,
            Agent::Lead => arr.role_for(Role::Lead) != ArrangementRole::Silent,
            _ => false,
        };
        let response = match mode {
            ResponseMode::Clockwork => {
                // Same responder, same metric offset (beat 3.5 of the call's last bar), same
                // transform — every time.
                let bar_start = (call.end_beat / BEATS_PER_BAR).floor() * BEATS_PER_BAR;
                let start = bar_start + 2.5;
                (start < total_beats - 0.5).then(|| Response {
                    action: None,
                    responder: Agent::Keys,
                    start_beat: start,
                    dur_beats: 1.0,
                    latency: start - call.end_beat,
                    overlap: start < call.end_beat,
                    transform: Transform::Echo,
                    crosses_chord: crosses(chords, call.end_beat, start + 1.0),
                })
            }
            ResponseMode::Free => {
                let mut cands: Vec<Agent> = vec![Agent::Keys];
                if lang.distributed_agency {
                    cands.extend([Agent::Bass, Agent::Drums]);
                }
                if call.initiator != Agent::Lead {
                    cands.push(Agent::Lead);
                }
                cands.retain(|&a| a != call.initiator && audible(a));
                let latencies: &[f64] = if lang.distributed_agency {
                    &[-1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 3.0]
                } else {
                    &[0.5, 1.0]
                };
                let band_fragment = call.statement.is_some()
                    && band_fragments
                        .iter()
                        .any(|&b| (b - call.start_beat).abs() < 1e-6);
                if band_fragment {
                    cands.retain(|&a| a != Agent::Drums);
                }
                let transforms: &[Transform] = if band_fragment {
                    &[Transform::Compress]
                } else if lang.distributed_agency {
                    &[
                        Transform::Quote,
                        Transform::Complete,
                        Transform::Invert,
                        Transform::Compress,
                        Transform::Echo,
                    ]
                } else {
                    &[Transform::Echo, Transform::Quote]
                };
                let answer_chance = lang.interaction;
                let mut best: Option<(f32, Response, Signature)> = None;
                for &who in &cands {
                    for &lat in latencies {
                        let start = call.end_beat + lat;
                        if start < call.start_beat + 1.0 || start >= total_beats - 0.5 {
                            continue;
                        }
                        // Drums answer briefly; pitched answers take up to two beats.
                        let max_dur = if who == Agent::Drums { 1.0 } else { 2.0 };
                        // An overlapping answer starts inside the call, so it has that much more room.
                        let room = gap - lat;
                        let dur = room.min(max_dur);
                        if dur < 0.5 - 1e-6 {
                            continue;
                        }
                        if accent.is_hole(start) {
                            continue;
                        }
                        for &tf in transforms {
                            if who == Agent::Drums && tf != Transform::Echo {
                                continue; // a drummer echoes the rhythm; it has no pitches
                            }
                            let sig = Signature {
                                initiator: call.initiator,
                                responder: who,
                                latency_q: (lat * 2.0).round() as i8,
                                step: AccentGrid::step_of(start).1 as u8,
                                transform: tf,
                            };
                            let w = accent.at_beat(start);
                            let fit = -(w.syncopation + w.pickup) * 0.6;
                            let overlap_cost = if lat < 0.0 { 0.35 } else { 0.0 };
                            let cost = memory.penalty(&sig)
                                + memory.novelty(sig.latency_q)
                                + fit
                                + overlap_cost
                                + rng.range_f32(0.0, 0.2);
                            if best.as_ref().is_none_or(|b| cost < b.0) {
                                best = Some((
                                    cost,
                                    Response {
                                        action: None,
                                        responder: who,
                                        start_beat: start,
                                        dur_beats: dur,
                                        latency: lat,
                                        overlap: lat < 0.0,
                                        transform: tf,
                                        crosses_chord: crosses(chords, call.end_beat, start + dur),
                                    },
                                    sig,
                                ));
                            }
                        }
                    }
                }
                // Sometimes the right answer is to leave the space open.
                let silent = ci > 0 && !band_fragment && rng.chance(1.0 - answer_chance);
                match best {
                    Some((_, r, sig)) if !silent => {
                        memory.record(sig);
                        Some(r)
                    }
                    Some((_, r, _)) => Some(Response {
                        transform: Transform::Silence,
                        ..r
                    }),
                    None => None,
                }
            }
        };
        let response = response.map(|mut r| {
            if r.transform != Transform::Silence {
                r.action = Some(actions.push(MusicalAction {
                    id: ActionId(0),
                    cause: ActionCause::Interaction { call: call.action },
                    initiator: r.responder,
                    start_beat: r.start_beat,
                    dur_beats: r.dur_beats,
                    kind: ActionKind::Answer,
                    target_beat: None,
                    responders: vec![],
                    binding: None,
                    pays: Some(call.action),
                }));
            }
            r
        });
        interactions.push(Interaction {
            call: *call,
            response,
        });
    }
    interactions.sort_by(|a, b| a.call.start_beat.total_cmp(&b.call.start_beat));
    (statements, interactions)
}

/// Whether `[a, b)` crosses a chord boundary.
pub(super) fn crosses(chords: &[ChordSpan], a: f64, b: f64) -> bool {
    chords
        .iter()
        .any(|c| c.start_beat > a + 1e-6 && c.start_beat < b - 1e-6)
}
