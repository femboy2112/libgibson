//! **PerformancePlan** — one shared performance that every instrument realizes a projection of.
//!
//! Through Round VI each part was realized blind from common static inputs: keys saw only the
//! chords, bass saw chords plus kick times, the lead saw chords plus the plan, and nobody saw
//! anybody else's notes. Call and response was a *slot relation* — `ThematicTrajectory::next_for`
//! handed the call to one discourse role and the answer to another, so "who answers" and "when"
//! were fixed by the form. Five composers shared a skeleton; none of them listened.
//!
//! The pipeline is now
//!
//! ```text
//! CompositionPlan (form, discourse, backbone timeline)   — the SONG
//!        │  + MusicWorld (timbre)  + MusicalLanguage (idiom)
//!        ▼
//! PerformancePlan                                         — the shared PERFORMANCE
//!   harmony + HarmonicContext timeline (local pitch palettes, relations, expectations)
//!   ActionPlan (path-lifted morphisms + the spine's own verbs + interaction actions)
//!   AccentGrid (one rhythmic field every player draws different subsets from)
//!   lead statement plan + calls → ResponseWindows → Interactions (with InteractionMemory)
//!   EnsembleBar per bar (who is foreground, each player's mode, the complexity budget,
//!                        the kinetic target)
//!        ▼
//! Score realization — lead, keys, bass, drums, pad each read THE SAME plan, and later players
//!                     read the earlier players' notes (they listen).
//! ```

use super::action::{ActionCause, ActionKind, ActionPlan, Agent, MusicalAction};
use super::backbone::{BackboneTimeline, DeflectWitness, HarmonicGesture};
use super::context::{analyze, HarmonicContext};
use super::discourse::DiscourseRole;
use super::form::BEATS_PER_BAR;
use super::harmony::{ChordSpan, HarmonyEngine};
use super::language::MusicalLanguage;
use super::motif::{Handoff, Motif, MotifBank, ThematicTrajectory};
use super::plan::{ArrangementRole, CompositionPlan};
use super::rng::Rng;
use super::score::Role;
use super::theory::{Chord, Quality, Scale};
use super::timeline::IntentTimeline;
use super::world::MusicWorld;

/// Sixteenth-note steps per 4/4 bar.
pub const STEPS: usize = 16;
/// Beats per step.
pub const STEP_BEATS: f64 = BEATS_PER_BAR / STEPS as f64;

/// The accent weights of one grid step, each in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StepWeight {
    /// Metric structure (the downbeat, beat 3).
    pub structural: f32,
    /// The backbeat (beats 2 and 4).
    pub backbeat: f32,
    /// An opportunity to syncopate (off-beats, the piece's gesture rhythm cells).
    pub syncopation: f32,
    /// A lead-in onto the next structural event.
    pub pickup: f32,
    /// An anticipation push onto a target.
    pub push: f32,
    /// A planned hole: players should leave it empty.
    pub hole: f32,
    /// An ensemble hit.
    pub hit: f32,
}

/// The shared rhythmic field: one [`StepWeight`] per sixteenth of every bar.
#[derive(Debug, Clone, PartialEq)]
pub struct AccentGrid {
    pub bars: Vec<[StepWeight; STEPS]>,
    /// The rhythm cell (off-beat steps) each gesture uses in this piece — its rhythmic identity.
    pub cells: [Vec<usize>; 4],
}

impl AccentGrid {
    /// The beat of `(bar, step)`.
    pub fn beat_of(bar: u32, step: usize) -> f64 {
        bar as f64 * BEATS_PER_BAR + step as f64 * STEP_BEATS
    }

    /// The `(bar, step)` holding `beat` (floor).
    pub fn step_of(beat: f64) -> (u32, usize) {
        let b = (beat / BEATS_PER_BAR).floor().max(0.0);
        let s = ((beat - b * BEATS_PER_BAR) / STEP_BEATS).round() as usize;
        if s >= STEPS {
            (b as u32 + 1, 0)
        } else {
            (b as u32, s)
        }
    }

    /// The weights at `(bar, step)` (zero outside the piece).
    pub fn at(&self, bar: u32, step: usize) -> StepWeight {
        self.bars
            .get(bar as usize)
            .map(|b| b[step.min(STEPS - 1)])
            .unwrap_or_default()
    }

    /// The weights at `beat`.
    pub fn at_beat(&self, beat: f64) -> StepWeight {
        let (b, s) = Self::step_of(beat);
        self.at(b, s)
    }

    /// Whether `beat` falls in a planned hole.
    pub fn is_hole(&self, beat: f64) -> bool {
        self.at_beat(beat).hole >= 0.5
    }
}

fn gesture_ix(g: HarmonicGesture) -> usize {
    match g {
        HarmonicGesture::Lift => 0,
        HarmonicGesture::Deflect => 1,
        HarmonicGesture::Open => 2,
        HarmonicGesture::Reset => 3,
    }
}

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
    /// The call action this statement opens.
    pub call: u32,
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
    pub action: u32,
    pub initiator: Agent,
    pub start_beat: f64,
    pub end_beat: f64,
    /// The lead statement it is, when the lead called.
    pub statement: Option<usize>,
}

/// A planned response.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Response {
    /// The action id of the response.
    pub action: u32,
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

/// Pad behaviour for a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadMode {
    /// Hold the full voicing.
    Sustain,
    /// A partial shell (guide tones only).
    Shell,
    /// Hold only the tones common with the previous harmony.
    CommonToneCarry,
    /// A swelling entry.
    Swell,
    /// A wide upper-structure layer (colour tones high).
    UpperStructure,
    /// Out.
    Silent,
}

/// Keys behaviour for a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeysMode {
    /// Syncopated shell comping on the accent grid, around the lead.
    Comp,
    /// Answering a call in a response window.
    Answer,
    /// Short stabs on the planned hits.
    Stab,
    /// Holding a voicing (a suspension).
    Sustain,
    /// Leaving space.
    Space,
}

/// Bass behaviour for a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BassMode {
    /// Roots/fifths on the structural steps.
    Foundation,
    /// One held pitch under changing harmony.
    Pedal,
    /// A stepwise line into the next harmony.
    Walk,
    /// A melodic counterline in the lead's gaps.
    Counter,
    /// Quoting motif material (a call or an answer).
    Quote,
}

/// Drums behaviour for a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumsMode {
    Pocket,
    HalfTime,
    DoubleTime,
    Break,
    Fill,
}

/// The ensemble's per-bar plan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnsembleBar {
    pub bar: u32,
    pub gesture: Option<HarmonicGesture>,
    pub cycle: u32,
    pub foreground: Agent,
    pub pad: PadMode,
    pub keys: KeysMode,
    pub bass: BassMode,
    pub drums: DrumsMode,
    /// Simultaneous-information budget for the bar (capped per-role onsets per beat, summed).
    pub budget: f32,
    /// The kinetic target in `[0, 1]`: forward motion independent of loudness.
    pub kinetic: f32,
}

/// Options that select the calibration probes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerformanceOptions {
    pub language: MusicalLanguage,
    /// Build the action plan (false = the mood-without-action probe).
    pub actions: bool,
    pub responses: ResponseMode,
}

impl Default for PerformanceOptions {
    fn default() -> Self {
        PerformanceOptions {
            language: MusicalLanguage::default(),
            actions: true,
            responses: ResponseMode::Free,
        }
    }
}

/// A harmonic recolouring or tonicization applied by an action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicEdit {
    pub action: u32,
    pub at_beat: f64,
    pub before: Chord,
    pub after: Chord,
}

/// The whole shared performance.
#[derive(Debug, Clone)]
pub struct PerformancePlan {
    pub language: MusicalLanguage,
    pub region: Scale,
    pub chords: Vec<ChordSpan>,
    pub contexts: Vec<HarmonicContext>,
    pub deflects: Vec<DeflectWitness>,
    pub edits: Vec<HarmonicEdit>,
    pub actions: ActionPlan,
    pub accent: AccentGrid,
    pub statements: Vec<LeadStatement>,
    pub interactions: Vec<Interaction>,
    pub ensemble: Vec<EnsembleBar>,
    pub bank: MotifBank,
    pub response_mode: ResponseMode,
    pub total_beats: f64,
}

impl PerformancePlan {
    /// Build the performance of `plan` under `world`.
    pub fn build(
        timeline: &IntentTimeline,
        plan: &CompositionPlan,
        world: &MusicWorld,
        seed: u64,
        opts: PerformanceOptions,
    ) -> PerformancePlan {
        let lang = opts.language;
        let region = Scale::new(world.tonic_pc, world.mode);
        let total_beats = plan.form.total_bars as f64 * BEATS_PER_BAR;
        let targets = plan.targets();

        // 1. Harmony: the backbone realized in this world and language, or the phrase engine.
        let (mut chords, deflects) = match &plan.backbone {
            Some(tl) => {
                let r = super::backbone::realize(tl, world, &lang, seed);
                (r.spans, r.deflects)
            }
            None => (
                HarmonyEngine::new(world, seed).generate(&targets, plan.contract.resolution),
                Vec::new(),
            ),
        };

        // 2. Actions.
        let mut actions = if opts.actions {
            ActionPlan::build(timeline, plan.backbone.as_ref(), &lang, total_beats, seed)
        } else {
            ActionPlan::none()
        };

        // 2b. An ensemble push/hit needs an ensemble: where the arrangement leaves fewer than two
        //     players on stage (the intro's pad+bass), the verb is carried by the one who is there —
        //     it becomes that player's pickup, not a phantom tutti.
        for a in actions.actions.iter_mut() {
            if !matches!(a.kind, ActionKind::Push | ActionKind::Hit)
                || a.initiator != Agent::Ensemble
            {
                continue;
            }
            let ph = plan.form.phrase_at(a.start_beat.min(total_beats - 1e-6)).ix as usize;
            let arr = plan.arrangement.at(ph);
            let on_stage: Vec<Agent> = [
                (Agent::Bass, arr.bass),
                (Agent::Keys, arr.keys),
                (Agent::Drums, arr.drums),
                (Agent::Lead, arr.lead),
            ]
            .into_iter()
            .filter(|(_, r)| r.is_audible())
            .map(|(g, _)| g)
            .collect();
            if on_stage.len() < 2 {
                if let Some(&solo) = on_stage.first() {
                    a.kind = ActionKind::Pickup;
                    a.initiator = solo;
                    a.target_beat = a.target_beat.or(Some(a.end_beat()));
                }
            }
        }

        // 3. Harmonic actions edit the harmony (so they are heard, not merely labelled).
        let edits = apply_harmonic_actions(&mut chords, &actions, &region);
        let contexts = analyze(&chords, &region);

        // 4. The shared rhythmic field.
        let accent = build_accent_grid(plan, &actions, &lang, seed);

        // 5. Lead statements, calls, responses.
        let bank = MotifBank::generate(&region, seed ^ 0x3E10_D1E5);
        let (statements, interactions) = plan_interactions(
            plan,
            &mut actions,
            &accent,
            &chords,
            &bank,
            &lang,
            opts.responses,
            opts.actions,
            seed,
        );

        // 6. The ensemble per bar.
        let ensemble = plan_ensemble(plan, &actions, &statements, &interactions, &lang);

        PerformancePlan {
            language: lang,
            region,
            chords,
            contexts,
            deflects,
            edits,
            actions,
            accent,
            statements,
            interactions,
            ensemble,
            bank,
            response_mode: opts.responses,
            total_beats,
        }
    }

    /// The ensemble plan for `bar`.
    pub fn bar(&self, bar: u32) -> Option<&EnsembleBar> {
        self.ensemble.get(bar as usize)
    }

    /// The ensemble plan for the bar holding `beat`.
    pub fn bar_at(&self, beat: f64) -> Option<&EnsembleBar> {
        self.bar((beat / BEATS_PER_BAR).floor().max(0.0) as u32)
    }

    /// The harmonic context sounding at `beat`.
    pub fn context_at(&self, beat: f64) -> Option<&HarmonicContext> {
        super::context::context_at(&self.contexts, beat)
    }

    /// The responses assigned to `agent`.
    pub fn responses_for(&self, agent: Agent) -> impl Iterator<Item = (&Call, &Response)> {
        self.interactions.iter().filter_map(move |i| {
            i.response
                .as_ref()
                .filter(|r| r.responder == agent && r.transform != Transform::Silence)
                .map(|r| (&i.call, r))
        })
    }

    /// The calls initiated by `agent` other than lead statements (figures a player states).
    pub fn figure_calls_for(&self, agent: Agent) -> impl Iterator<Item = &Call> {
        self.interactions
            .iter()
            .map(|i| &i.call)
            .filter(move |c| c.initiator == agent && c.statement.is_none())
    }

    /// A compact dump of the performance: statements, interactions, ensemble.
    pub fn dump(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "performance: language={} responses={:?} statements={} interactions={} edits={}",
            self.language.id.label(),
            self.response_mode,
            self.statements.len(),
            self.interactions.len(),
            self.edits.len()
        );
        for i in &self.interactions {
            let c = &i.call;
            match &i.response {
                Some(r) => {
                    let _ = writeln!(
                        s,
                        "  call a{} {:<5} {:>6.2}..{:<6.2} -> {:<5} {:+.2}b {:<8}{}{}",
                        c.action,
                        c.initiator.label(),
                        c.start_beat,
                        c.end_beat,
                        r.responder.label(),
                        r.latency,
                        r.transform.label(),
                        if r.overlap { " overlap" } else { "" },
                        if r.crosses_chord {
                            " crosses-chord"
                        } else {
                            ""
                        }
                    );
                }
                None => {
                    let _ = writeln!(
                        s,
                        "  call a{} {:<5} {:>6.2}..{:<6.2} -> (no room)",
                        c.action,
                        c.initiator.label(),
                        c.start_beat,
                        c.end_beat
                    );
                }
            }
        }
        let _ = writeln!(
            s,
            "  ensemble (bar: gesture fg pad/keys/bass/drums kinetic):"
        );
        for e in &self.ensemble {
            let _ = writeln!(
                s,
                "    {:>2} {:<8} {:<6} {:?}/{:?}/{:?}/{:?} k={:.2}",
                e.bar,
                e.gesture.map(|g| g.label()).unwrap_or("-"),
                e.foreground.label(),
                e.pad,
                e.keys,
                e.bass,
                e.drums,
                e.kinetic
            );
        }
        s
    }
}

/// Recolour/tonicize the harmony where a Reharmonize/Tonicize action fires.
fn apply_harmonic_actions(
    chords: &mut Vec<ChordSpan>,
    actions: &ActionPlan,
    region: &Scale,
) -> Vec<HarmonicEdit> {
    let mut edits = Vec::new();
    for a in &actions.actions {
        match a.kind {
            ActionKind::Reharmonize => {
                let Some(ix) = chords
                    .iter()
                    .rposition(|c| c.start_beat <= a.start_beat + 1e-6)
                else {
                    continue;
                };
                let before = chords[ix].chord;
                let q = match before.quality {
                    Quality::Maj | Quality::Maj6 => Quality::Maj7,
                    Quality::Maj7 | Quality::Add9 => Quality::Maj9,
                    Quality::Maj9 => Quality::Maj6,
                    Quality::Min | Quality::Min6 => Quality::Min7,
                    Quality::Min7 => Quality::Min9,
                    Quality::Min9 => Quality::Min6,
                    Quality::Dom7 => Quality::Dom9,
                    Quality::Dom9 => Quality::Dom7,
                    other => other,
                };
                if q != before.quality {
                    let after = Chord::new(before.root_pc, q);
                    chords[ix].chord = after;
                    edits.push(HarmonicEdit {
                        action: a.id,
                        at_beat: chords[ix].start_beat,
                        before,
                        after,
                    });
                }
            }
            ActionKind::Tonicize => {
                // Tonicize the harmony arriving at (or after) the action: its preceding span's
                // second half becomes the applied dominant — a real local region change.
                let Some(ix) = chords
                    .iter()
                    .position(|c| c.start_beat >= a.start_beat - 1e-6)
                else {
                    continue;
                };
                if ix == 0 {
                    continue;
                }
                let target = chords[ix].chord;
                if target.root_pc == region.tonic_pc {
                    continue; // a dominant to home is an arrival, not a tonicization
                }
                let prev = chords[ix - 1];
                if prev.dur_beats < 2.0 - 1e-6 {
                    continue;
                }
                let half = (prev.dur_beats / 2.0).max(1.0);
                let dom = Chord::new((target.root_pc + 7).rem_euclid(12), Quality::Dom7);
                chords[ix - 1].dur_beats = prev.dur_beats - half;
                chords.insert(
                    ix,
                    ChordSpan {
                        start_beat: prev.start_beat + (prev.dur_beats - half) as f64,
                        dur_beats: half,
                        chord: dom,
                        function: super::theory::Function::Dominant,
                        degree: -1,
                        note: prev.note,
                    },
                );
                edits.push(HarmonicEdit {
                    action: a.id,
                    at_beat: prev.start_beat + (prev.dur_beats - half) as f64,
                    before: prev.chord,
                    after: dom,
                });
            }
            _ => {}
        }
    }
    edits
}

/// Build the accent grid: meter, the piece's per-gesture rhythm cells (varied per bar by
/// rotation and omission), and the actions' pushes, hits, holes, pickups and displacements.
fn build_accent_grid(
    plan: &CompositionPlan,
    actions: &ActionPlan,
    lang: &MusicalLanguage,
    seed: u64,
) -> AccentGrid {
    let total_bars = plan.form.total_bars as usize;
    let mut rng = Rng::new(seed ^ 0x6A1D_0C3E);
    let sync = lang.syncopation;
    let sixteenths = lang.surface_subdivision >= 4;

    // Per-gesture rhythm cells: a small set of off-beat steps chosen once per piece — the
    // gesture's rhythmic identity (Lift pushes most, Open least).
    let offbeats: Vec<usize> = (0..STEPS)
        .filter(|s| s % 4 != 0 && (sixteenths || s % 2 == 0))
        .collect();
    let sizes = [4usize, 3, 2, 3];
    let cells: [Vec<usize>; 4] = std::array::from_fn(|g| {
        let mut pool = offbeats.clone();
        let mut cell = Vec::new();
        for _ in 0..sizes[g].min(pool.len()) {
            // Prefer eighth off-beats (stronger syncopation), sixteenths as colour.
            let weights: Vec<f32> = pool
                .iter()
                .map(|s| if s % 2 == 0 { 1.0 } else { 0.55 })
                .collect();
            let total: f32 = weights.iter().sum();
            let mut r = rng.range_f32(0.0, total);
            let mut k = 0;
            for (i, w) in weights.iter().enumerate() {
                if r < *w {
                    k = i;
                    break;
                }
                r -= w;
            }
            cell.push(pool.remove(k));
        }
        cell.sort_unstable();
        cell
    });

    let mut bars = vec![[StepWeight::default(); STEPS]; total_bars];
    for (bar, w) in bars.iter_mut().enumerate() {
        for (s, sw) in w.iter_mut().enumerate() {
            sw.structural = match s {
                0 => 1.0,
                8 => 0.8,
                4 | 12 => 0.35,
                _ => 0.0,
            };
            sw.backbeat = if s == 4 || s == 12 { 1.0 } else { 0.0 };
            sw.syncopation = if s % 4 == 2 {
                0.4 * sync
            } else if s % 2 == 1 && sixteenths {
                0.2 * sync
            } else {
                0.0
            };
            if s >= 14 {
                sw.pickup = 0.3;
            }
        }
        // The gesture's cell, varied per bar: rotated by an eighth on odd bars of the slot,
        // thinned on the slot's last bar (a breath), with a pickup into the next slot.
        if let Some(bb) = &plan.backbone {
            if let Some(slot) = bb.slot_at_bar(bar as u32) {
                let in_slot = bar as u32 - slot.start_bar;
                let last = bar as u32 + 1 == slot.end_bar();
                let cell = &cells[gesture_ix(slot.gesture)];
                let rot = if in_slot % 2 == 1 { 2 } else { 0 };
                for (k, &c) in cell.iter().enumerate() {
                    if last && k + 1 == cell.len() && cell.len() > 1 {
                        continue;
                    }
                    let st = (c + rot) % STEPS;
                    if st % 4 == 0 {
                        continue;
                    }
                    w[st].syncopation = w[st].syncopation.max(0.35 + 0.6 * sync);
                }
                if last {
                    w[14].pickup = 0.8;
                    w[15].pickup = 0.7;
                }
            }
        }
    }

    // Actions mutate the grid.
    for a in &actions.actions {
        let (b0, s0) = AccentGrid::step_of(a.start_beat);
        let steps = ((a.dur_beats / STEP_BEATS).round() as usize).max(1);
        let mut each = |f: &mut dyn FnMut(&mut StepWeight, usize)| {
            for k in 0..steps {
                let abs = b0 as usize * STEPS + s0 + k;
                let (bi, si) = (abs / STEPS, abs % STEPS);
                if let Some(bw) = bars.get_mut(bi) {
                    f(&mut bw[si], si);
                }
            }
        };
        match a.kind {
            ActionKind::Push => each(&mut |w, _| {
                w.push = 1.0;
                w.hit = w.hit.max(0.7);
            }),
            ActionKind::Hit => {
                let (b, s) = AccentGrid::step_of(a.start_beat);
                if let Some(bw) = bars.get_mut(b as usize) {
                    bw[s].hit = 1.0;
                }
            }
            ActionKind::Break => each(&mut |w, _| w.hole = 1.0),
            ActionKind::Displace => each(&mut |w, s| {
                if s % 4 != 0 {
                    w.syncopation = (w.syncopation * 1.5 + 0.1).min(1.0);
                } else if s == 8 {
                    w.structural *= 0.5;
                }
            }),
            ActionKind::Pullback => each(&mut |w, s| {
                w.syncopation *= 0.5;
                if s % 2 == 1 {
                    w.hole = w.hole.max(0.5);
                }
            }),
            ActionKind::Accelerate => each(&mut |w, s| {
                if s % 2 == 1 {
                    w.syncopation = (w.syncopation + 0.3).min(1.0);
                }
            }),
            ActionKind::Fill | ActionKind::Pickup => each(&mut |w, _| {
                w.pickup = w.pickup.max(0.9);
            }),
            _ => {}
        }
    }
    AccentGrid { bars, cells }
}

/// Plan the lead statements, the calls, and the responses.
#[allow(clippy::too_many_arguments)]
fn plan_interactions(
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
                u32::MAX
            } else {
                actions.push(MusicalAction {
                    id: 0,
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
                })
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
    let answered: Vec<u32> = interactions.iter().map(|i| i.call.action).collect();

    // Responses to the lead's statements (and to unanswered figures).
    if !interact {
        // The mood-without-action probe: the lead still sings, but nobody answers anybody.
        return (statements, interactions);
    }
    let mut calls: Vec<Call> = statements
        .iter()
        .enumerate()
        .map(|(si, s)| Call {
            action: s.call,
            initiator: Agent::Lead,
            start_beat: s.start_beat,
            end_beat: s.end_beat(),
            statement: Some(si),
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
                    action: 0,
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
                                        action: 0,
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
                r.action = actions.push(MusicalAction {
                    id: 0,
                    cause: ActionCause::Interaction { call: call.action },
                    initiator: r.responder,
                    start_beat: r.start_beat,
                    dur_beats: r.dur_beats,
                    kind: ActionKind::Answer,
                    target_beat: None,
                    responders: vec![],
                    binding: None,
                    pays: Some(call.action),
                });
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
fn crosses(chords: &[ChordSpan], a: f64, b: f64) -> bool {
    chords
        .iter()
        .any(|c| c.start_beat > a + 1e-6 && c.start_beat < b - 1e-6)
}

/// Plan each bar's ensemble: foreground, player modes, budget and kinetic target.
fn plan_ensemble(
    plan: &CompositionPlan,
    actions: &ActionPlan,
    statements: &[LeadStatement],
    interactions: &[Interaction],
    lang: &MusicalLanguage,
) -> Vec<EnsembleBar> {
    let bb: Option<&BackboneTimeline> = plan.backbone.as_ref();
    let total_bars = plan.form.total_bars;
    let mut out = Vec::with_capacity(total_bars as usize);
    for bar in 0..total_bars {
        let s = bar as f64 * BEATS_PER_BAR;
        let e = s + BEATS_PER_BAR;
        let in_bar = |a: &MusicalAction| a.start_beat < e - 1e-6 && a.end_beat() > s + 1e-6;
        let has = |k: ActionKind| actions.actions.iter().any(|a| a.kind == k && in_bar(a));
        let slot = bb.and_then(|b| b.slot_at_bar(bar));
        let gesture = slot.map(|s| s.gesture);
        let cycle = slot.map(|s| s.cycle).unwrap_or(0);
        let lead_busy: f64 = statements
            .iter()
            .map(|st| (st.end_beat().min(e) - st.start_beat.max(s)).max(0.0))
            .sum();
        let responder = interactions.iter().find_map(|i| {
            i.response.as_ref().and_then(|r| {
                (r.transform != Transform::Silence
                    && r.start_beat < e - 1e-6
                    && r.start_beat + r.dur_beats > s + 1e-6)
                    .then_some(r.responder)
            })
        });
        let foreground = if lead_busy >= 2.0 {
            Agent::Lead
        } else if let Some(r) = responder {
            r
        } else {
            Agent::Keys
        };
        let keys = if responder == Some(Agent::Keys) {
            KeysMode::Answer
        } else if has(ActionKind::Hold) {
            KeysMode::Sustain
        } else if has(ActionKind::Hit) || has(ActionKind::Push) {
            KeysMode::Stab
        } else if lead_busy >= 3.0 && lang.complexity_budget < 8.0 {
            KeysMode::Space
        } else {
            KeysMode::Comp
        };
        let pad = if has(ActionKind::Thin) {
            PadMode::Silent
        } else if has(ActionKind::Thicken) {
            PadMode::UpperStructure
        } else {
            match gesture {
                Some(HarmonicGesture::Lift) => PadMode::Shell,
                Some(HarmonicGesture::Deflect) => PadMode::CommonToneCarry,
                Some(HarmonicGesture::Open) => {
                    if has(ActionKind::ReEntry) {
                        PadMode::Swell
                    } else {
                        PadMode::UpperStructure
                    }
                }
                Some(HarmonicGesture::Reset) => PadMode::Sustain,
                None => PadMode::Sustain,
            }
        };
        let bass = if responder == Some(Agent::Bass)
            || interactions.iter().any(|i| {
                i.call.initiator == Agent::Bass && i.call.start_beat < e && i.call.end_beat > s
            }) {
            BassMode::Quote
        } else {
            match gesture {
                Some(HarmonicGesture::Lift) => BassMode::Walk,
                Some(HarmonicGesture::Deflect) => BassMode::Pedal,
                Some(HarmonicGesture::Open) if lead_busy < 2.0 => BassMode::Counter,
                _ => BassMode::Foundation,
            }
        };
        let drums = if has(ActionKind::Break) {
            DrumsMode::Break
        } else if actions
            .actions
            .iter()
            .any(|a| a.kind == ActionKind::Fill && a.start_beat >= s && a.start_beat < e)
        {
            DrumsMode::Fill
        } else if has(ActionKind::Pullback) {
            DrumsMode::HalfTime
        } else if has(ActionKind::Accelerate) {
            DrumsMode::DoubleTime
        } else {
            DrumsMode::Pocket
        };
        let base = match gesture {
            Some(HarmonicGesture::Lift) => 0.6,
            Some(HarmonicGesture::Deflect) => 0.75,
            Some(HarmonicGesture::Open) => 0.45,
            Some(HarmonicGesture::Reset) => 0.5,
            None => 0.5,
        };
        let acts = actions.actions.iter().filter(|a| in_bar(a)).count() as f32;
        let kinetic = (base * (1.0 + 0.2 * cycle as f32) + 0.04 * acts).clamp(0.0, 1.0);
        out.push(EnsembleBar {
            bar,
            gesture,
            cycle,
            foreground,
            pad,
            keys,
            bass,
            drums,
            budget: lang.complexity_budget * (0.7 + 0.5 * kinetic),
            kinetic,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::semantic::deflected_lift_trace;
    use super::*;

    fn flagship(opts: PerformanceOptions) -> PerformancePlan {
        let trace = deflected_lift_trace(120.0);
        let tl = IntentTimeline::walk(&trace);
        let plan = CompositionPlan::build_with_contract(
            &tl,
            30,
            super::super::contract::CoherenceContract::for_grammar(
                super::super::contract::CompositionGrammar::DeflectedLift,
            ),
        );
        PerformancePlan::build(&tl, &plan, &MusicWorld::black_ice(), 2112, opts)
    }

    #[test]
    fn free_responses_vary_responder_and_latency() {
        let p = flagship(PerformanceOptions::default());
        let rs: Vec<&Response> = p
            .interactions
            .iter()
            .filter_map(|i| i.response.as_ref())
            .filter(|r| r.transform != Transform::Silence)
            .collect();
        assert!(rs.len() >= 3, "too few responses: {}", rs.len());
        let responders: std::collections::BTreeSet<Agent> =
            rs.iter().map(|r| r.responder).collect();
        assert!(responders.len() >= 2, "one responder only: {responders:?}");
        let lat: std::collections::BTreeSet<i64> = rs
            .iter()
            .map(|r| (r.latency * 2.0).round() as i64)
            .collect();
        assert!(lat.len() >= 3, "latencies too uniform: {lat:?}");
    }

    #[test]
    fn clockwork_responses_are_rigid_by_construction() {
        let p = flagship(PerformanceOptions {
            responses: ResponseMode::Clockwork,
            ..PerformanceOptions::default()
        });
        for r in p.interactions.iter().filter_map(|i| i.response.as_ref()) {
            assert_eq!(r.responder, Agent::Keys);
            assert_eq!(AccentGrid::step_of(r.start_beat).1, 10);
        }
    }

    #[test]
    fn the_accent_grid_varies_across_bars_of_a_slot() {
        let p = flagship(PerformanceOptions::default());
        let sig = |bar: u32| -> Vec<usize> {
            (0..STEPS)
                .filter(|&s| p.accent.at(bar, s).syncopation > 0.5)
                .collect()
        };
        // Bars 4..8 are one Lift slot: the rotation makes consecutive bars differ.
        assert_ne!(sig(4), sig(5));
    }

    #[test]
    fn disabling_actions_keeps_the_harmony_and_removes_the_verbs() {
        let on = flagship(PerformanceOptions::default());
        let off = flagship(PerformanceOptions {
            actions: false,
            ..PerformanceOptions::default()
        });
        assert!(off.actions.actions.is_empty());
        assert!(off.interactions.is_empty());
        assert_eq!(on.statements.len(), off.statements.len());
        assert!(on.actions.actions.len() > off.actions.actions.len());
        assert_eq!(on.chords.len(), off.chords.len());
    }
}
