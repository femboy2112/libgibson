//! Opt-in SMOOTH-POP groove source (R2): a lead-free, Keys-carried band pocket at a pop harmonic
//! loop, carrying a syncopated bass identity. No foreground lead — the probe is whether kick / snare
//! / bass / keys alone make a recognizable, intentional pocket (the plan's R2 "can the band groove
//! without a virtuoso lead"). Both episodes are Keys-carried `Establish`, so the lead lane is
//! genuinely EMPTY (not a bus-muted ghost); the bass plays the authored per-bar [`GrooveLaw`] cells;
//! the harmony is a repeating [`SourceDomain::SmoothPop`] loop that closes as a groove-cycle, never on
//! a literal tonic. Everything here is additive and opt-in; no C137 source is touched.
//!
//! The first cut used a single stamped bass cell over the Axis-of-Awesome loop (i–VI–III–VII); the
//! ear's verdict was "offensively cliché" harmony and a "robotic" groove. This cut answers BOTH:
//! - Two fresh, less-worn progressions offered as an ear-gated **A/B** ([`PopFlavor`]) — a
//!   circle-of-fourths turnaround and a leading-tone "lift" — never the four-chord loop.
//! - The groove breathes: FOUR varied per-bar bass cells (a settled statement, a walking answer, a
//!   driving bar, a turnaround that pushes back to the top) with ghost notes and a dynamic arc,
//!   instead of one identical bar stamped eight times.
//! - The keys are re-voiced to the **guide tones** (3rd/7th + a 9th colour) of EACH flavor's chords,
//!   because the carried keys comp does not follow the route — it must be authored to match it or it
//!   clashes, and no gate but the ear catches a clash.
use gibson::audio::human_music::{
    action::Agent,
    argument::{
        ArcSpan, ArgumentEnding, ArgumentFamily, ArgumentRelation, ArgumentStep, ArgumentStepId,
        ArgumentTransform, BassDegree, CompiledArgument, GrooveLaw, MusicalArgument,
        MusicalReferent, PopSourceGrammar, ReferentId, SourceDomain,
    },
    contract::CompositionGrammar,
    material::MaterialEvent,
    plan::ArrangementRole,
    semantic::{EventKind, SemanticEvent, SemanticState, SemanticTrace, Tone},
    theory::{PitchBasis, Quality},
    SongMap,
};

/// The ear-gated A/B of smooth-pop harmonic flavors. Both are lawful A-Aeolian (black_ice world,
/// `color_depth=2`, modal mixture admitted) and both are deliberately NOT the i–VI–III–VII cliché;
/// the maintainer's ear picks the harmonic world (nothing is promoted). Each flavor pairs a 4-bar
/// progression with a keys comp that voices THAT progression's guide tones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopFlavor {
    /// `Am9 – Dm9 – G9 – Cmaj9` — a circle-of-fourths turnaround (i9–iv9–♭VII9–♭IIImaj9; a vi–ii–V–I
    /// in the relative major). Ascending-fourth root motion is the smoothest motion in tonal music;
    /// warm, forward, jazzy-smooth. Closes by cycling back to Am9 (groove-cycle, not a tonic arrival).
    Circle,
    /// `Am9 – Fmaj9 – Dm9 – E7` — a leading-tone "lift" (i9–VImaj9–iv9–V7). The iv→V with a real
    /// dominant (E7 borrows G♯, the leading tone, via modal mixture) is the authentic minor-key pull;
    /// cinematic, emotional-smooth. The dynamic arc climbs to the dominant and resolves back to the top.
    Lift,
}

impl PopFlavor {
    /// The region-relative per-bar pop loop for this flavor (one 4-bar cycle). Offsets are semitones
    /// above the region tonic (A): C=+3, D=+5, E=+7, F=+8, G=+10. Every quality is admitted by the
    /// black_ice A-Aeolian vocabulary (sevenths + colour + mixture), and the sounded chord is exactly
    /// this (the route is not conformed on the perform path), so what is authored is what is heard.
    fn loop4(self) -> [(i32, Quality); 4] {
        match self {
            // i9 – iv9 – ♭VII9 – ♭IIImaj9 (circle of fourths: A→D→G→C).
            PopFlavor::Circle => [
                (0, Quality::Min9),
                (5, Quality::Min9),
                (10, Quality::Dom9),
                (3, Quality::Maj9),
            ],
            // i9 – VImaj9 – iv9 – V7 (the lift: A→F→D→E, resolving E7→Am).
            PopFlavor::Lift => [
                (0, Quality::Min9),
                (8, Quality::Maj9),
                (5, Quality::Min9),
                (7, Quality::Dom7),
            ],
        }
    }

    /// The sparse, syncopated keys comp over one 16-beat episode — the GUIDE TONES (3rd + 7th) of each
    /// bar's chord with a 9th colour, voiced as a flowing monophonic line in conscious negative space.
    /// Tonic-relative semitone steps (`PitchBasis::Semitones`), tuned to THIS flavor's loop so the
    /// carried keys and the sounded harmony agree. `(onset, semitone_above_A, dur, velocity)`.
    fn keys_comp(self) -> Vec<MaterialEvent> {
        let rows: &[(f64, i32, f64, f32)] = match self {
            // Am9: C(♭3) G(♭7) B(9) · Dm9: F(♭3) C(♭7) E(9) · G9: F(♭7) B(3) A(9) · Cmaj9: E(3) B(7) D(9)
            PopFlavor::Circle => &[
                (0.5, 3, 1.3, 0.56),
                (2.0, 10, 1.0, 0.50),
                (3.25, 14, 0.6, 0.44),
                (4.5, 8, 1.3, 0.54),
                (6.0, 3, 1.0, 0.48),
                (7.25, 7, 0.6, 0.44),
                (8.5, 8, 1.3, 0.54),
                (10.0, 2, 1.0, 0.50),
                (11.25, 12, 0.6, 0.44),
                (12.5, 7, 1.3, 0.54),
                (14.0, 14, 0.9, 0.48),
                (15.0, 5, 0.75, 0.44),
            ],
            // Am9: C(♭3) G(♭7) B(9) · Fmaj9: A(3) E(7) G(9) · Dm9: F(♭3) C(♭7) E(9) · E7: G♯(3) D(♭7) F(♭9)
            PopFlavor::Lift => &[
                (0.5, 3, 1.3, 0.56),
                (2.0, 10, 1.0, 0.50),
                (3.25, 14, 0.6, 0.44),
                (4.5, 12, 1.3, 0.54),
                (6.0, 7, 1.0, 0.48),
                (7.25, 10, 0.6, 0.44),
                (8.5, 8, 1.3, 0.54),
                (10.0, 3, 1.0, 0.50),
                (11.25, 7, 0.6, 0.44),
                (12.5, 11, 1.3, 0.56),
                (14.0, 5, 0.9, 0.48),
                (15.0, 8, 0.75, 0.44),
            ],
        };
        rows.iter()
            .map(|&(onset, step, dur, accent)| MaterialEvent {
                onset,
                dur,
                accent,
                step: Some(step),
            })
            .collect()
    }

    /// A short honest label of the progression for the manifest header. (Used by the example binary;
    /// the test crate includes this module via `#[path]` but does not call it.)
    #[allow(dead_code)]
    pub fn harmony_label(self) -> &'static str {
        match self {
            PopFlavor::Circle => {
                "Am9 - Dm9 - G9 - Cmaj9 (i9-iv9-bVII9-bIIImaj9, circle-of-4ths turnaround, A-Aeolian)"
            }
            PopFlavor::Lift => {
                "Am9 - Fmaj9 - Dm9 - E7 (i9-VImaj9-iv9-V7, leading-tone pull, A-Aeolian + mixture)"
            }
        }
    }

    /// The `--form=` token that selects this flavor. (Used by the example binary; the test crate
    /// includes this module via `#[path]` but does not call it.)
    #[allow(dead_code)]
    pub fn form_token(self) -> &'static str {
        match self {
            PopFlavor::Circle => "pop-groove",
            PopFlavor::Lift => "pop-groove-lift",
        }
    }
}

/// The breathing per-bar bass cells (one 4-bar cycle), SHARED across flavors because the degrees
/// resolve per bar against whatever chord sits under them. A smooth bassist does not stamp one bar
/// eight times — these four bars are a settled statement, a walking answer, a driving bar, and a
/// turnaround that pushes back to the top, each with ghost notes (soft velocities) and a dynamic
/// contour. Every cell opens on `(0.0, Root, ..)` — the downbeat anchor the foundation law reads.
/// `(offset_in_bar_beats, degree, dur_beats, velocity)`.
fn bass_cells() -> Vec<Vec<(f64, BassDegree, f32, f32)>> {
    vec![
        // Bar 0 — land & breathe: the root rings, a soft octave pop, a laid-back fifth, an off-beat
        // root, a colour third into the turn. Five attacks (the cell's signature statement).
        vec![
            (0.0, BassDegree::Root, 0.6, 0.78),
            (0.75, BassDegree::Octave, 0.22, 0.42),
            (1.75, BassDegree::Fifth, 0.6, 0.56),
            (2.5, BassDegree::Root, 0.6, 0.60),
            (3.25, BassDegree::Third, 0.5, 0.46),
        ],
        // Bar 1 — walking answer: busier, stepping through root/fifth/third with ghost notes, an
        // octave push on the "and" of 4 into the next bar.
        vec![
            (0.0, BassDegree::Root, 0.45, 0.74),
            (0.5, BassDegree::Fifth, 0.4, 0.44),
            (1.0, BassDegree::Third, 0.5, 0.58),
            (2.0, BassDegree::Fifth, 0.5, 0.60),
            (2.75, BassDegree::Root, 0.6, 0.48),
            (3.5, BassDegree::Octave, 0.4, 0.56),
        ],
        // Bar 2 — driving: the firmest bar (and the flavor's loud chord lands here), octave pop and a
        // double fifth around a ghost root, a third push into the turn.
        vec![
            (0.0, BassDegree::Root, 0.6, 0.80),
            (0.75, BassDegree::Octave, 0.22, 0.46),
            (1.5, BassDegree::Fifth, 0.6, 0.64),
            (2.25, BassDegree::Root, 0.4, 0.48),
            (2.75, BassDegree::Fifth, 0.6, 0.52),
            (3.5, BassDegree::Third, 0.45, 0.56),
        ],
        // Bar 3 — turnaround: opens wide (root + fifth held), then a quick root/third/fifth climb and
        // a strong octave push on the "and" of 4 that pulls the ear back to the top of the loop.
        vec![
            (0.0, BassDegree::Root, 0.9, 0.74),
            (1.0, BassDegree::Fifth, 0.9, 0.56),
            (2.0, BassDegree::Root, 0.4, 0.58),
            (2.5, BassDegree::Third, 0.4, 0.44),
            (3.0, BassDegree::Fifth, 0.4, 0.60),
            (3.5, BassDegree::Octave, 0.45, 0.62),
        ],
    ]
}

/// Build the lead-free smooth-pop groove source for one seed and one [`PopFlavor`]. Two 16-beat
/// Keys-carried `Establish` episodes (8 bars = two loop cycles), the flavor's pop harmonic loop, its
/// matching guide-tone keys comp, and the shared breathing bass cells.
pub fn source_pop_groove(seed: u64, flavor: PopFlavor) -> CompiledArgument {
    let trace = SemanticTrace {
        total_beats: 32.0,
        events: vec![
            SemanticEvent {
                at_beat: 0.0,
                state: SemanticState::toned(Tone::Accent),
                kind: EventKind::ActChanged,
            },
            SemanticEvent {
                at_beat: 28.0,
                state: SemanticState::toned(Tone::Success),
                kind: EventKind::SectionResolved,
            },
        ],
    };
    let mut song = SongMap::build(&trace, seed, Some(CompositionGrammar::HookArc));
    let mut contract = song.plan.contract.clone();
    contract.phrase_bars = 1;
    song.plan = gibson::audio::human_music::plan::CompositionPlan::build_with_contract_for_beats(
        &song.timeline,
        32.0,
        contract,
    );
    // Lead-free staging: the lead is off-stage, the keys carry, the bass is the foundation, the
    // drummer keeps the pocket, the pad rests. The absence of any Lead carrier (below) is what makes
    // this genuinely lead-free; the staging just reflects it.
    for p in &mut song.plan.arrangement.phrases {
        p.lead = ArrangementRole::Support;
        p.keys = ArrangementRole::Support;
        p.bass = ArrangementRole::Foundation;
        p.drums = ArrangementRole::Support;
        p.pad = ArrangementRole::Silent;
    }
    let riff0 = MusicalReferent {
        id: ReferentId(0),
        pitch_basis: PitchBasis::Semitones,
        events: flavor.keys_comp(),
    };
    let riff1 = MusicalReferent {
        id: ReferentId(1),
        pitch_basis: PitchBasis::Semitones,
        events: flavor.keys_comp(),
    };
    let boundary =
        song.plan
            .form
            .phrases
            .iter()
            .position(|p| p.start_beat() == 16.0)
            .expect("eight one-bar phrases put an episode boundary at beat 16") as u32;
    let total_phrases = song.plan.form.phrases.len() as u32;
    let argument = MusicalArgument {
        family: ArgumentFamily::CallAndEarnedAnswer,
        referents: vec![riff0, riff1],
        ending: ArgumentEnding::Resolved,
        steps: vec![
            ArgumentStep {
                id: ArgumentStepId(0),
                referent: ReferentId(0),
                transform: ArgumentTransform::default(),
                relation: ArgumentRelation::Establish,
                depends_on: vec![],
                carriers: vec![Agent::Keys],
            },
            ArgumentStep {
                id: ArgumentStepId(1),
                referent: ReferentId(1),
                transform: ArgumentTransform::default(),
                relation: ArgumentRelation::Establish,
                depends_on: vec![ArgumentStepId(0)],
                carriers: vec![Agent::Keys],
            },
        ],
        arc: Some(vec![
            ArcSpan {
                step: ArgumentStepId(0),
                start_phrase: 0,
                phrase_count: boundary,
                start_beat: 0.0,
                span_beats: 16.0,
            },
            ArcSpan {
                step: ArgumentStepId(1),
                start_phrase: boundary,
                phrase_count: total_phrases - boundary,
                start_beat: 16.0,
                span_beats: 16.0,
            },
        ]),
    };
    let mut compiled = argument
        .compile(&song)
        .expect("smooth-pop groove source compiles");
    // The eight-bar route is the four-bar pop loop twice. Declared after compile, like the C137
    // builders set `route` — so the long-form domain reads it under the smooth-pop closure law.
    let loop4 = flavor.loop4();
    let mut route = loop4.to_vec();
    route.extend_from_slice(&loop4);
    compiled.route = route;
    // Opt this ONE source into the smooth-pop closure law (groove-cycle, loop of 4 bars) and the
    // breathing per-bar bass cells. No C137 source is affected; both are set after compile, like `route`.
    compiled.domain = SourceDomain::SmoothPop(PopSourceGrammar { loop_bars: 4 });
    compiled.groove = Some(GrooveLaw {
        bass_cells: bass_cells(),
    });
    compiled
}
