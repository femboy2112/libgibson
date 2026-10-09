//! **Interaction** — the lead's statements, the calls anyone can make, and the time-local
//! responses to them (Round VII; split out of [`super::performance`] in Round VIIb).
//!
//! A [`Call`] is something one player said that others may answer; a [`Response`] is a planned
//! answer inside a free response window, chosen with [`InteractionMemory`] so the band does not
//! repeat the same initiator / responder / latency / placement / transform. The
//! [`ResponseMode::Clockwork`] probe is the lifeless control.
//!
//! Round VIIb makes the conversation causal and selective:
//! - every call owns [`InteractionMaterial`] (built in the plan, before anybody plays), and every
//!   response derives ITS material from the call's — a keys answer to a bass figure transforms the
//!   bass figure, not whatever the lead happened to be playing;
//! - not every statement is a call: an [`InteractionOpportunity`] weighs whether the statement is
//!   rhetorically open, whether an action opened space, whether anybody is on stage and has room,
//!   and whether the last exchange already said this ([`CallPolicy::EveryStatement`] is the
//!   saturation probe);
//! - drums have agency: a drum fill is a rhythm-only call the band can answer on the landing.

use super::action::{ActionCause, ActionKind, ActionPlan, Agent, EffectVector, MusicalAction};
use super::discourse::DiscourseRole;
use super::ensemble::Stage;
use super::form::BEATS_PER_BAR;
use super::harmony::ChordSpan;
use super::ids::{ActionId, InteractionId, MaterialId};
use super::language::MusicalLanguage;
use super::material::{transform_material, InteractionMaterial, MaterialSource};
use super::motif::{Handoff, Motif};
use super::performance::{AccentGrid, PerformanceOptions};
use super::plan::CompositionPlan;
use super::rng::Rng;
use super::song::ThematicMap;

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

/// When a lead statement becomes a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallPolicy {
    /// A statement calls only when it opens conversational space (the real model).
    Selective,
    /// The saturation probe: every statement is a call (and the planner answers what it can).
    EveryStatement,
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
    /// The call or answer action this statement is — `None` when it stands alone (it closes its
    /// own thought) or actions are off.
    pub call: Option<ActionId>,
    /// The material the statement states.
    pub material: MaterialId,
    /// The figure call this statement answers: `(call action, call material)`.
    pub answers: Option<(ActionId, MaterialId)>,
    /// The lead Fragment action this statement realizes (it states the fragmented motif).
    pub fragment: Option<ActionId>,
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
    /// What the call says — the material every answer derives from.
    pub material: MaterialId,
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
    /// The response's own material (derived from the call's by `transform`); `None` for silence
    /// or for a lead statement that answers a figure (its material is the statement's).
    pub material: Option<MaterialId>,
    /// Another action this response performs — the lead Fragment the band carries before the
    /// thesis has been stated (the compressed answer IS the fragmentation).
    pub realizes: Option<ActionId>,
}

/// One call with its response (a planned silence is a response too).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interaction {
    pub id: InteractionId,
    pub call: Call,
    pub response: Option<Response>,
}

/// What an opportunity came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpportunitySource {
    /// A lead statement (index into the statements).
    Statement(usize),
    /// A figure a player states for an action.
    Figure(ActionId),
}

/// Whether an opportunity became a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Call,
    /// It stands alone, with the reason.
    StandsAlone(&'static str),
}

/// One chance for a conversation, weighed before any call exists (Round VIIb): the planner no
/// longer turns every lead statement into "lead speaks → somebody answers".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InteractionOpportunity {
    pub source: OpportunitySource,
    pub initiator: Agent,
    pub start_beat: f64,
    pub end_beat: f64,
    /// How rhetorically open the statement is (a question wants an answer; a return closes).
    pub openness: f32,
    /// An action (a break, a hold, a thinning, a re-entry, a fill) opened space around its end.
    pub space: f32,
    /// Headroom: a sparse statement leaves the band room to talk.
    pub headroom: f32,
    /// The last exchange already said this.
    pub redundancy: f32,
    pub score: f32,
    pub verdict: Verdict,
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

/// Everything the interaction planner produced.
pub(super) struct InteractionPlan {
    pub statements: Vec<LeadStatement>,
    pub interactions: Vec<Interaction>,
    pub materials: Vec<InteractionMaterial>,
    pub opportunities: Vec<InteractionOpportunity>,
}

/// How open a discourse role leaves the floor.
fn openness(role: DiscourseRole) -> f32 {
    match role {
        DiscourseRole::Question | DiscourseRole::Withhold => 0.9,
        DiscourseRole::Depart | DiscourseRole::Establish => 0.65,
        DiscourseRole::Culminate | DiscourseRole::Intensify => 0.55,
        DiscourseRole::Restate => 0.35,
        DiscourseRole::Answer | DiscourseRole::Return | DiscourseRole::Dissolve => 0.2,
    }
}

/// Whether `[a, b)` crosses a chord boundary.
pub(super) fn crosses(chords: &[ChordSpan], a: f64, b: f64) -> bool {
    chords
        .iter()
        .any(|c| c.start_beat > a + 1e-6 && c.start_beat < b - 1e-6)
}

/// The response candidates for `call` (who, latency, duration), before transforms and memory.
#[allow(clippy::too_many_arguments)]
fn feasible(
    call: &Call,
    unpitched: bool,
    next_start: f64,
    total_beats: f64,
    cands: &[Agent],
    lang: &MusicalLanguage,
    accent: &AccentGrid,
    stage: &Stage,
) -> Vec<(Agent, f64, f64)> {
    let gap = next_start - call.end_beat;
    let latencies: &[f64] = if unpitched {
        // A drum figure is answered on (or just after) its landing.
        &[0.0, 0.5, 1.0]
    } else if lang.distributed_agency {
        &[-1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 3.0]
    } else {
        &[0.5, 1.0]
    };
    let mut out = Vec::new();
    for &who in cands {
        for &lat in latencies {
            let start = call.end_beat + lat;
            if start < call.start_beat + 0.5 || start >= total_beats - 0.5 {
                continue;
            }
            // Answers take up to two beats (a drum echo long enough to carry the call's rhythm).
            let max_dur = 2.0;
            // An overlapping answer starts inside the call, so it has that much more room.
            let dur = (gap - lat).min(max_dur).min(total_beats - start);
            if dur < 0.5 - 1e-6 || accent.is_hole(start) {
                continue;
            }
            if !stage.on_stage_span(who, start, start + dur) {
                continue;
            }
            out.push((who, lat, dur));
        }
    }
    out
}

/// GEN-3a: develop a statement with GOAL-DIRECTED connective motion. The motif's notes stay as
/// structural ANCHORS at their original onsets (the skeleton stays locked in the pocket and the
/// statement stays recognizable); the span before each anchor's next landing is filled with a short
/// stepwise run that LEADS from this anchor toward the next one. A leap gets stepwise fill toward the
/// target; a repeat gets one upper neighbour so it still moves. The run subdivides the anchor's OWN
/// duration, so the total length and every anchor onset are preserved exactly: the band's pocket is
/// untouched, and the lead stops skipping between orbit points and instead makes transitions that
/// arrive somewhere. Connectors come only on longer notes, gated by `energy` and varied per statement
/// by `salt`. Fully deterministic (seeded by salt + start).
fn develop_lead_rhythm(
    motif: &super::motif::Motif,
    start: f64,
    energy: f32,
    salt: usize,
) -> super::motif::Motif {
    let n = motif.degrees.len().min(motif.rhythm.len());
    if n < 2 {
        return motif.clone(); // too short to connect without erasing its identity
    }
    let strength = (0.4 + 0.6 * energy.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let mut rng = Rng::new((salt as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ start.to_bits());
    let mut degrees: Vec<i32> = Vec::with_capacity(n * 2);
    let mut rhythm: Vec<f32> = Vec::with_capacity(n * 2);
    for i in 0..n {
        let d0 = motif.degrees[i];
        let dur = motif.rhythm[i];
        // The final note is the cadence landing: keep it whole (identity).
        if i + 1 >= n {
            degrees.push(d0);
            rhythm.push(dur);
            break;
        }
        let d1 = motif.degrees[i + 1];
        let span = (d1 - d0).abs();
        // How many connectors lead this anchor toward the next landing: only on longer notes,
        // gated by energy; a wide gap over a long note earns a two-note run, else one.
        let mut k = if dur < 0.75 || rng.range_f32(0.0, 1.0) > strength {
            0
        } else if dur >= 1.5 && span >= 2 {
            2
        } else {
            1
        };
        // Keep each subdivision musical: no sub-sixteenth chatter at these tempos.
        while k > 0 && dur / (k as f32 + 1.0) < 0.25 {
            k -= 1;
        }
        if k == 0 {
            degrees.push(d0);
            rhythm.push(dur);
            continue;
        }
        let sub = dur / (k as f32 + 1.0);
        degrees.push(d0);
        rhythm.push(sub);
        for j in 1..=k {
            // Step TOWARD the next landing: a leap gets stepwise fill, a repeat an upper neighbour.
            let step = if span == 0 {
                d0 + 1
            } else {
                let t = j as f32 / (k as f32 + 1.0);
                d0 + ((d1 - d0) as f32 * t).round() as i32
            };
            degrees.push(step);
            rhythm.push(sub);
        }
    }
    super::motif::Motif {
        pitch_basis: motif.pitch_basis,
        id: motif.id,
        degrees,
        rhythm,
    }
}

/// Plan the lead statements, their materials, the opportunities, the calls and the responses.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_interactions(
    plan: &CompositionPlan,
    actions: &mut ActionPlan,
    accent: &AccentGrid,
    chords: &[ChordSpan],
    thematic: &ThematicMap,
    lang: &MusicalLanguage,
    opts: &PerformanceOptions,
    stage: &Stage,
    seed: u64,
    cover: Option<&super::cover::CoverConstraints>,
    vetoed: &[super::rehearsal::ActionKey],
    functions: super::policy::FunctionPolicy,
    lead_life: super::policy::LeadLifePolicy,
    narrative: Option<&super::narrative::NarrativePlan>,
) -> InteractionPlan {
    let bank = &thematic.bank;
    let mode = opts.responses;
    let interact = opts.actions;
    let mut rng = Rng::new(seed ^ 0x1A7E_4C71);
    let mut memory = InteractionMemory::default();
    let mut materials: Vec<InteractionMaterial> = Vec::new();
    let mut opportunities: Vec<InteractionOpportunity> = Vec::new();
    let mut interactions: Vec<Interaction> = Vec::new();
    let mut statements: Vec<LeadStatement> = Vec::new();
    let total_beats = plan.form.total_beats;
    let two_bar = 2.0 * BEATS_PER_BAR;
    let calls_open = lang.distributed_agency && interact && mode == ResponseMode::Free;

    // A declared bass figure is part of what makes this song this song. Under earned functions the
    // bass's BORROWED material — a figure quoting the lead's motif, an answer — never takes the
    // last bar downbeat its own line could sound on: the downbeat is the one onset the bass's own
    // line is guaranteed in every bar mode, and the bass never plays its line inside its own
    // quote. (Responses are judged after their random draw, so the stream is unchanged and only
    // the guarded decision differs.)
    let keeps_bass_figure = functions == super::policy::FunctionPolicy::Earned
        && plan
            .contract
            .anchors
            .contains(&super::contract::CoherenceAnchor::BassFigure);
    let bass_downbeats: Vec<f64> = (0..)
        .map(|bar| bar as f64 * BEATS_PER_BAR)
        .take_while(|&d| d < total_beats - 1e-9)
        .filter(|&d| stage.on_stage(Agent::Bass, d) && !accent.is_hole(d))
        .collect();
    let takes_last_bass_downbeat = |materials: &[InteractionMaterial],
                                    interactions: &[Interaction],
                                    start: f64,
                                    end: f64|
     -> bool {
        let inside = |d: f64, s: f64, e: f64| d >= s - 1e-6 && d < e - 1e-6;
        let borrowed = |d: f64| {
            materials
                .iter()
                .filter(|m| {
                    m.owner == Agent::Bass && matches!(m.source, MaterialSource::Figure { .. })
                })
                .any(|m| inside(d, m.start_beat, m.start_beat + m.length()))
                || interactions
                    .iter()
                    .filter_map(|i| i.response.as_ref())
                    .filter(|r| r.responder == Agent::Bass && r.transform != Transform::Silence)
                    .any(|r| inside(d, r.start_beat, r.start_beat + r.dur_beats))
        };
        // The downbeats the bass's own line still has, and whether this window takes all of them.
        let free: Vec<f64> = bass_downbeats
            .iter()
            .copied()
            .filter(|&d| !borrowed(d))
            .collect();
        keeps_bass_figure && !free.is_empty() && free.iter().all(|&d| inside(d, start, end))
    };

    // --- 1. Figures: every figure-bearing action gets its material, stated by its initiator
    //        whether or not anybody answers it. Figures open calls only in the free model. ---
    let mut figure_calls: Vec<Call> = Vec::new();
    let figure_actions: Vec<MusicalAction> = actions
        .actions
        .iter()
        .filter(|a| matches!(a.initiator, Agent::Bass | Agent::Keys | Agent::Drums))
        .cloned()
        .collect();
    for a in &figure_actions {
        // One player, one line: a figure that begins inside a figure the same player is already
        // stating (a Reset's fill and the Lift's pickup into the same downbeat) is performed BY
        // that figure — its window is covered, so it gets no second, overlapping line.
        if materials.iter().any(|m: &InteractionMaterial| {
            m.owner == a.initiator
                && a.start_beat >= m.start_beat - 1e-6
                && a.start_beat < m.start_beat + m.length() - 1e-6
        }) {
            continue;
        }
        let id = MaterialId(materials.len() as u32);
        let Some(m) = InteractionMaterial::figure(id, a, bank, a.effect.strength) else {
            continue;
        };
        if a.initiator == Agent::Bass
            && takes_last_bass_downbeat(
                &materials,
                &interactions,
                m.start_beat,
                m.start_beat + m.length(),
            )
        {
            continue; // the verb stays; the bass performs it in its own line
        }
        materials.push(m);
        if calls_open {
            let len = a.dur_beats.clamp(0.5, 2.0);
            figure_calls.push(Call {
                action: a.id,
                initiator: a.initiator,
                start_beat: a.start_beat,
                end_beat: (a.start_beat + len).min(total_beats),
                statement: None,
                material: id,
            });
        }
    }

    // --- 2. Lead statements at the song's theme sites, stating what the song states there (the
    //        thematic trajectory ran once, upstream, in the SongMap); start offsets vary lawfully
    //        (a pickup into the bar, on the beat, a displaced entry). ---
    let mut offset_memory: Vec<i32> = Vec::new();
    let mut thesis_stated = false;
    // Statement starts whose Fragment verb passes to the band (before the thesis was stated),
    // with the Fragment action the band's compressed answer will perform.
    let mut band_fragments: Vec<(f64, ActionId)> = Vec::new();
    if let Some(c) = cover.filter(|c| c.identity.line(super::score::Role::Lead).is_some()) {
        c.plan_statements(plan, &mut statements, &mut materials);
    } else {
        for t in plan.targets() {
            let phrase = t.phrase;
            let Some(site) = thematic.site(phrase.ix) else {
                continue;
            };
            if !stage.on_stage(Agent::Lead, phrase.start_beat()) {
                continue;
            }
            // Ensemble narrative: at a WITHHELD site (the Miss), the lead does not state — it
            // withholds at the expected arrival and a band carrier takes the site instead. No lead
            // statement is pushed for this phrase; the carrier's response (below) carries the germ.
            if narrative
                .and_then(|n| n.at(phrase.ix))
                .is_some_and(|c| c.lead_role == super::narrative::LeadRole::Withheld)
            {
                continue;
            }
            let (motif, handoff) = (&site.motif, site.handoff);
            let len = motif.total_beats() as f64;
            if len < 1e-6 {
                continue;
            }
            let pe = phrase.end_beat();
            let mut at = phrase.start_beat();
            let mut guard = 0;
            while at + len <= pe + 1e-6 && guard < 32 {
                // A figure call from bass/keys/drums just before this statement: the lead ANSWERS it.
                let answering = figure_calls
                    .iter()
                    .find(|c| c.end_beat > at - 2.0 && c.end_beat <= at + 1.0 + 1e-6)
                    .copied();
                let offset = if let Some(c) = answering {
                    // Enter after the figure (a short lawful latency), never on top of it.
                    ((c.end_beat - at).max(0.0) + 0.5)
                        .min(pe - at - len)
                        .max(0.0)
                } else if lang.id == super::language::LanguageId::Simple && !lead_life.spacing {
                    0.0
                } else {
                    // Choose among lawful entries by the grid's pickup/syncopation weight and memory —
                    // never an entry that would put the lead where the stage has it out. GEN-3b: with
                    // the spacing axis on, widen the lawful entry set (later anacruses are admitted)
                    // and prefer the best NON-ZERO entry, so statements stop landing on the phrase
                    // start every two bars; the accent grid still chooses which non-zero entry, and
                    // the memory keeps consecutive phrases from repeating one. Simple-language songs,
                    // which historically pin the lead to the downbeat, get the varied entry too.
                    let wide: [(f64, i32); 6] =
                        [(0.0, 0), (-0.5, -1), (0.5, 1), (1.0, 2), (1.5, 3), (2.0, 4)];
                    let narrow: [(f64, i32); 4] = [(0.0, 0), (-0.5, -1), (0.5, 1), (1.0, 2)];
                    let opts: &[(f64, i32)] = if lead_life.spacing { &wide } else { &narrow };
                    let mut best = (f32::INFINITY, 0.0);
                    let mut best_nonzero: Option<(f32, f64)> = None;
                    for &(o, q) in opts {
                        let s = at + o;
                        if s < 0.0 || s + len > pe + 0.5 + 1e-6 || !stage.on_stage(Agent::Lead, s) {
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
                        if o != 0.0 && best_nonzero.is_none_or(|(bc, _)| cost < bc) {
                            best_nonzero = Some((cost, o));
                        }
                    }
                    // Spacing on: take the best lawful non-zero entry (fall back to 0 only if no
                    // non-zero entry fits the phrase). Off: the historical best-overall choice.
                    let chosen = if lead_life.spacing {
                        best_nonzero.map_or(best.1, |(_, o)| o)
                    } else {
                        best.1
                    };
                    offset_memory.push((chosen * 2.0).round() as i32);
                    chosen
                };
                let start = (at + offset).max(0.0);
                if start + len > total_beats + 1e-6 {
                    break;
                }
                // A lead-initiated Fragment action in force here fragments THIS statement: the verb
                // reaches the thematic material instead of only nudging a scalar. Development
                // presupposes exposition, and the hook and thesis returns are identity: before the
                // thesis is stated (or on an identity role) the band carries the fragmentation.
                let fragment_action = actions
                    .actions
                    .iter()
                    .find(|a| {
                        a.kind == ActionKind::Fragment
                            && a.initiator == Agent::Lead
                            && a.covers(start)
                    })
                    .map(|a| a.id);
                // Identity is the SONG's declaration (Round X: a site the song restates to teach it
                // is identity whatever its role), not re-derived here from the role.
                let identity_role = site.is_identity();
                let fragmenting = fragment_action.is_some() && thesis_stated && !identity_role;
                if let (Some(f), false) = (fragment_action, fragmenting) {
                    band_fragments.push((start, f));
                }
                let motif = if fragmenting && motif.len() > 2 {
                    motif.fragment(motif.len().div_ceil(2).max(2))
                } else {
                    motif.clone()
                };
                // GEN-3a: develop the statement with goal-directed connective motion — the motif's
                // notes stay as anchors in the pocket, and short stepwise runs lead from each anchor
                // into the next landing (smooth transitions, more notes). Off on the historical path.
                let motif = if lead_life.development && !fragmenting {
                    develop_lead_rhythm(&motif, start, t.goal.energy_target, statements.len())
                } else {
                    motif
                };
                let len = motif.total_beats() as f64;
                let motif_is_full = motif.len() >= bank.identity.len();
                let mid = MaterialId(materials.len() as u32);
                materials.push(InteractionMaterial::from_motif(
                    mid,
                    Agent::Lead,
                    &motif,
                    start,
                    MaterialSource::Statement {
                        statement: statements.len(),
                        motif: motif.id,
                    },
                ));
                statements.push(LeadStatement {
                    phrase: phrase.ix,
                    start_beat: start,
                    motif,
                    handoff,
                    role: t.goal.role,
                    energy: t.goal.energy_target,
                    register: t.goal.register_target,
                    is_rupture: phrase.is_rupture,
                    call: None,
                    material: mid,
                    answers: answering.map(|c| (c.action, c.material)),
                    fragment: if fragmenting { fragment_action } else { None },
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
    }
    if !interact {
        // The mood-without-action probe: the lead still sings, but nobody answers anybody.
        return InteractionPlan {
            statements,
            interactions,
            materials,
            opportunities,
        };
    }

    let responder_pool = |initiator: Agent, declared: &[Agent]| -> Vec<Agent> {
        let mut c: Vec<Agent> = vec![Agent::Keys];
        if lang.distributed_agency {
            c.extend([Agent::Bass, Agent::Drums]);
        }
        // Declared responders widen the pool only in a language with distributed agency (the
        // plain language keeps its keys-only answers).
        if lang.distributed_agency {
            for &d in declared {
                if !c.contains(&d) && d != Agent::Ensemble && d != Agent::Pad {
                    c.push(d);
                }
            }
        }
        // The lead answers a figure through its own next statement (see the statement pass),
        // never with a free response: the lead realizer plays statements.
        c.retain(|&a| a != initiator && a != Agent::Lead);
        // A fully pinned source line/pattern has no discretionary response slot.
        // Admission happens before generating response material or promises.
        if let Some(pin) = cover {
            c.retain(|a| match a {
                Agent::Bass => pin.identity.line(super::score::Role::Bass).is_none(),
                Agent::Drums => pin.identity.groove.is_none(),
                _ => true,
            });
        }
        c
    };
    let next_statement_after = |b: f64, statements: &[LeadStatement]| -> f64 {
        statements
            .iter()
            .map(|s| s.start_beat)
            .filter(|&s| s > b + 1e-6)
            .fold(total_beats, f64::min)
    };

    // --- 3. Statement actions: an answer to a figure is an Answer; a statement that opens
    //        conversational space is a Call; the rest stand alone. ---
    let theta = 0.9 - 0.4 * lang.interaction;
    let mut last_call_motif: Option<Motif> = None;
    let mut last_exchange_end = f64::NEG_INFINITY;
    for si in 0..statements.len() {
        let st = statements[si].clone();
        let end = st.end_beat();
        let band_fragment = band_fragments
            .iter()
            .any(|&(b, _)| (b - st.start_beat).abs() < 1e-6);
        let space = if actions.actions.iter().any(|a| {
            a.initiator != Agent::Lead
                && matches!(
                    a.kind,
                    ActionKind::Break
                        | ActionKind::Hold
                        | ActionKind::Pullback
                        | ActionKind::Thin
                        | ActionKind::ReEntry
                        | ActionKind::Fill
                )
                && a.start_beat < end + 2.0
                && a.end_beat() > end - 0.5
        }) {
            0.3
        } else {
            0.0
        };
        let density = st.motif.len() as f32 / st.motif.total_beats().max(1.0);
        let headroom = 0.3 * (1.0 - density / 2.0).clamp(0.0, 1.0);
        let mut redundancy = 0.0;
        if last_call_motif.as_ref() == Some(&st.motif) {
            redundancy += 0.6;
        }
        if last_exchange_end > st.start_beat - 2.0 {
            redundancy += 0.25;
        }
        let open = openness(st.role);
        let score = open + space + headroom - redundancy;
        let probe = Call {
            action: ActionId(u32::MAX),
            initiator: Agent::Lead,
            start_beat: st.start_beat,
            end_beat: end,
            statement: Some(si),
            material: st.material,
        };
        let cands = responder_pool(Agent::Lead, &[]);
        let room = feasible(
            &probe,
            false,
            next_statement_after(st.start_beat, &statements),
            total_beats,
            &cands,
            lang,
            accent,
            stage,
        );
        let room_left = room.iter().any(|&(who, lat, dur)| {
            let at = probe.end_beat + lat;
            who != Agent::Bass || !takes_last_bass_downbeat(&materials, &interactions, at, at + dur)
        });
        // Ensemble narrative: a statement at a carrier phrase opens a call so the named carrier can
        // answer it (carrying the germ). Without this the carrier has no response to ride, and the
        // opportunistic scorer may never open a call there.
        let narrative_wants_carry = narrative
            .and_then(|n| n.at(st.phrase))
            .is_some_and(|c| !c.carriers.is_empty());
        let mut verdict = if !room_left {
            Verdict::StandsAlone("nobody on stage has room to answer")
        } else if mode == ResponseMode::Clockwork
            || opts.calls == CallPolicy::EveryStatement
            || band_fragment
            || narrative_wants_carry
            || score >= theta
        {
            Verdict::Call
        } else {
            Verdict::StandsAlone("it closes its own thought")
        };
        // The verb this statement would be (a call, or the lead's answer to a figure call).
        let verb = |is_call: bool| MusicalAction {
            id: ActionId(0),
            cause: match st.answers {
                Some((c, _)) => ActionCause::Interaction { call: c },
                None => ActionCause::Statement { phrase: st.phrase },
            },
            initiator: Agent::Lead,
            start_beat: st.start_beat,
            dur_beats: st.motif.total_beats() as f64,
            kind: if st.answers.is_some() {
                ActionKind::Answer
            } else {
                ActionKind::Call
            },
            target_beat: None,
            responders: if is_call {
                vec![Agent::Keys, Agent::Bass, Agent::Drums]
            } else {
                vec![]
            },
            binding: None,
            pays: st.answers.map(|(c, _)| c),
            effect: EffectVector::NEUTRAL,
        };
        // A rehearsal found nobody performing this verb: the statement stands alone.
        let struck = !vetoed.is_empty()
            && (st.answers.is_some() || verdict == Verdict::Call)
            && vetoed.contains(&super::rehearsal::ActionKey::of(
                &verb(verdict == Verdict::Call),
                actions,
            ));
        if struck && verdict == Verdict::Call {
            verdict = Verdict::StandsAlone(super::performance::REHEARSAL_REJECTION);
        }
        opportunities.push(InteractionOpportunity {
            source: OpportunitySource::Statement(si),
            initiator: Agent::Lead,
            start_beat: st.start_beat,
            end_beat: end,
            openness: open,
            space,
            headroom,
            redundancy,
            score,
            verdict,
        });
        let is_call = verdict == Verdict::Call;
        if (st.answers.is_none() && !is_call) || (struck && st.answers.is_some()) {
            continue;
        }
        let id = actions.push(verb(is_call));
        statements[si].call = Some(id);
        if is_call {
            last_call_motif = Some(st.motif.clone());
        }
        if let Some((c_action, c_mat)) = st.answers {
            let c = figure_calls
                .iter()
                .find(|c| c.action == c_action)
                .copied()
                .expect("an answered figure call exists");
            let lat = st.start_beat - c.end_beat;
            interactions.push(Interaction {
                id: InteractionId(0),
                call: Call {
                    material: c_mat,
                    ..c
                },
                response: Some(Response {
                    action: Some(id),
                    responder: Agent::Lead,
                    start_beat: st.start_beat,
                    dur_beats: st.motif.total_beats() as f64,
                    latency: lat,
                    overlap: lat < 0.0,
                    transform: Transform::Complete,
                    crosses_chord: crosses(chords, c.end_beat, st.start_beat + 1.0),
                    material: None,
                    realizes: None,
                }),
            });
            memory.record(Signature {
                initiator: c.initiator,
                responder: Agent::Lead,
                latency_q: (lat * 2.0).round() as i8,
                step: AccentGrid::step_of(st.start_beat).1 as u8,
                transform: Transform::Complete,
            });
            last_exchange_end = end;
        }
    }
    // Figure calls the lead did not answer are opportunities for the band.
    let answered: Vec<ActionId> = interactions.iter().map(|i| i.call.action).collect();

    // --- 4. Responses to the statements that call, and to unanswered figures. ---
    let mut calls: Vec<Call> = statements
        .iter()
        .enumerate()
        .filter_map(|(si, s)| {
            let is_call = actions
                .get(s.call?)
                .is_some_and(|a| a.kind == ActionKind::Call || !a.responders.is_empty());
            is_call.then(|| Call {
                action: s.call.expect("checked"),
                initiator: Agent::Lead,
                start_beat: s.start_beat,
                end_beat: s.end_beat(),
                statement: Some(si),
                material: s.material,
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
        let unpitched = !materials[call.material.index()].pitched();
        let next_start = next_statement_after(call.start_beat, &statements);
        let declared: Vec<Agent> = actions
            .get(call.action)
            .map(|a| a.responders.clone())
            .unwrap_or_default();
        let band_fragment_of = band_fragments
            .iter()
            .find(|&&(b, _)| call.statement.is_some() && (b - call.start_beat).abs() < 1e-6)
            .map(|&(_, f)| f);
        let band_fragment = band_fragment_of.is_some();
        let mut cands = responder_pool(call.initiator, &declared);
        if band_fragment {
            cands.retain(|&a| a != Agent::Drums);
        }
        if unpitched {
            // A drum figure is answered by a pitched player (the lead answers with its next entry).
            cands.retain(|&a| a != Agent::Drums && a != Agent::Lead);
        }
        let room = feasible(
            call,
            unpitched,
            next_start,
            total_beats,
            &cands,
            lang,
            accent,
            stage,
        );
        let lawful = |who: Agent, lat: f64, dur: f64| {
            let at = call.end_beat + lat;
            who != Agent::Bass || !takes_last_bass_downbeat(&materials, &interactions, at, at + dur)
        };
        if call.statement.is_none() && !room.iter().any(|&(w, l, d)| lawful(w, l, d)) {
            opportunities.push(InteractionOpportunity {
                source: OpportunitySource::Figure(call.action),
                initiator: call.initiator,
                start_beat: call.start_beat,
                end_beat: call.end_beat,
                openness: 0.0,
                space: 0.0,
                headroom: 0.0,
                redundancy: 0.0,
                score: 0.0,
                verdict: Verdict::StandsAlone("nobody on stage has room to answer the figure"),
            });
            continue;
        }
        if call.statement.is_none() {
            opportunities.push(InteractionOpportunity {
                source: OpportunitySource::Figure(call.action),
                initiator: call.initiator,
                start_beat: call.start_beat,
                end_beat: call.end_beat,
                openness: 0.6,
                space: 0.0,
                headroom: 0.0,
                redundancy: 0.0,
                score: 0.6,
                verdict: Verdict::Call,
            });
        }
        // Ensemble narrative: a lead statement at a carrier phrase is answered by the NAMED carrier
        // (keys on a Reinforce, bass on a Develop), not the cost+rng pick — this is how the band
        // carries the germ the lead taught. It only biases the choice among agents that already have
        // lawful room; if the carrier has none, the ordinary pick stands.
        let narrative_carrier: Option<Agent> = call
            .statement
            .and_then(|si| statements.get(si))
            .and_then(|st| narrative.and_then(|n| n.at(st.phrase)))
            .and_then(|c| c.carriers.first().copied());
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
                    material: None,
                    realizes: None,
                })
            }
            ResponseMode::Free => {
                let transforms: &[Transform] = if band_fragment {
                    &[Transform::Compress]
                } else if unpitched {
                    &[Transform::Echo]
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
                let mut best: Option<(f32, Response, Signature)> = None;
                for &(who, lat, dur) in &room {
                    let start = call.end_beat + lat;
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
                        // The named carrier wins over any other lawful responder; among its own
                        // transforms a literal/inverted germ (Quote/Invert) is preferred, so the
                        // handoff is recognizably the same theme.
                        let narrative_bias = if Some(who) == narrative_carrier {
                            -1000.0
                                + if matches!(tf, Transform::Quote | Transform::Invert) {
                                    -1.0
                                } else {
                                    0.0
                                }
                        } else {
                            0.0
                        };
                        let cost = memory.penalty(&sig)
                            + memory.novelty(sig.latency_q)
                            + fit
                            + overlap_cost
                            + rng.range_f32(0.0, 0.2)
                            + narrative_bias;
                        if !lawful(who, lat, dur) {
                            continue;
                        }
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
                                    material: None,
                                    realizes: band_fragment_of,
                                },
                                sig,
                            ));
                        }
                    }
                }
                // Sometimes the right answer is to leave the space open — but a call that the
                // planner kept because it opens space is usually answered.
                let answer_chance = if opts.calls == CallPolicy::EveryStatement {
                    1.0
                } else {
                    lang.interaction
                };
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
        let response = response.and_then(|mut r| {
            if r.transform == Transform::Silence {
                return Some(r);
            }
            // A rehearsal found nobody performing this answer: the space is left open instead.
            let answer = MusicalAction {
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
                effect: EffectVector::NEUTRAL,
            };
            if !vetoed.is_empty()
                && vetoed.contains(&super::rehearsal::ActionKey::of(&answer, actions))
            {
                return Some(Response {
                    transform: Transform::Silence,
                    ..r
                });
            }
            // The response's own material, derived from the CALL's material.
            let mid = MaterialId(materials.len() as u32);
            let m = transform_material(
                mid,
                &materials[call.material.index()],
                r.transform,
                r.responder,
                r.start_beat,
                r.dur_beats,
            )?;
            materials.push(m);
            r.material = Some(mid);
            r.action = Some(actions.push(answer));
            Some(r)
        });
        interactions.push(Interaction {
            id: InteractionId(0),
            call: *call,
            response,
        });
    }
    interactions.sort_by(|a, b| a.call.start_beat.total_cmp(&b.call.start_beat));
    for (i, it) in interactions.iter_mut().enumerate() {
        it.id = InteractionId(i as u32);
    }
    opportunities.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    InteractionPlan {
        statements,
        interactions,
        materials,
        opportunities,
    }
}
