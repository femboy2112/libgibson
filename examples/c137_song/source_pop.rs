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
//! - Keys and bass LOCK and LIFT: both are laid into the shared pocket (so the melodic voice lays
//!   back WITH the rhythm section instead of floating ahead of it, dead on the grid) and PUSH the
//!   "and of 4" together every bar — the band coalescing into one collective gesture while each voice
//!   keeps its own expressive line. The comp is driving (four off-beat stabs a bar) for pop energy.
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
    theme_family::pocket_offset,
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

    /// A driving, syncopated guide-tone comp over one 16-beat episode. Two things make it lock and
    /// lift instead of float: (1) every onset is laid into the shared [`pocket_offset`], so the keys
    /// lay back WITH the bass and drums rather than sitting dead on the grid ahead of the pocket; (2)
    /// the "and of 4" push (offset 3.5, the loudest stab) lands TOGETHER with the bass's octave push —
    /// the band coalescing into one gesture while each voice keeps its own line. Four off-beat stabs a
    /// bar (the "a" of 1, the "and" of 2 and 3, the push); guide tones (3rd/7th) with a 9th colour,
    /// voiced to THIS flavor's loop so the carried keys and the sounded harmony agree.
    fn keys_comp(self) -> Vec<MaterialEvent> {
        // Per-bar onset / dur / velocity. The push (offset 3.5) is the loudest — the collective hit.
        const PATTERN: [(f64, f64, f32); 4] = [
            (0.75, 0.45, 0.52),
            (1.5, 0.45, 0.56),
            (2.5, 0.45, 0.50),
            (3.5, 0.40, 0.62),
        ];
        // Guide tones per stab (semitones above A), 16 = 4 bars × 4 stabs.
        let steps: [i32; 16] = match self {
            // Am9: C G E · Dm9: F C A · G9: F B D · Cmaj9: E B G (the push repeats the bar's anchor tone).
            PopFlavor::Circle => [3, 10, 7, 3, 8, 3, 12, 8, 8, 2, 5, 8, 7, 2, 10, 7],
            // Am9: C G E · Fmaj9: A E C · Dm9: F C A · E7: G♯ D B (the push drives the leading tone on E7).
            PopFlavor::Lift => [3, 10, 7, 3, 12, 7, 3, 12, 8, 3, 12, 8, 11, 5, 2, 11],
        };
        let mut events = Vec::with_capacity(16);
        for bar in 0..4 {
            for (i, &(off, dur, accent)) in PATTERN.iter().enumerate() {
                let raw = bar as f64 * 4.0 + off;
                events.push(MaterialEvent {
                    onset: raw + pocket_offset(raw), // lock into the shared pocket the band orbits
                    dur,
                    accent,
                    step: Some(steps[bar * 4 + i]),
                });
            }
        }
        events
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
        // Bar 0 — land & bounce: the root anchors, a soft octave bounce, a laid-back fifth, an
        // off-beat root, then the collective PUSH on the "and" of 4. Five attacks (the signature).
        vec![
            (0.0, BassDegree::Root, 0.6, 0.80),
            (0.75, BassDegree::Octave, 0.22, 0.46),
            (1.5, BassDegree::Fifth, 0.5, 0.56),
            (2.5, BassDegree::Root, 0.5, 0.60),
            (3.5, BassDegree::Octave, 0.4, 0.62),
        ],
        // Bar 1 — walking answer: stepping fifth/octave/fifth with a ghost root, same "and of 4" push.
        vec![
            (0.0, BassDegree::Root, 0.45, 0.76),
            (0.5, BassDegree::Fifth, 0.3, 0.44),
            (1.0, BassDegree::Octave, 0.4, 0.50),
            (2.0, BassDegree::Fifth, 0.5, 0.58),
            (2.75, BassDegree::Root, 0.5, 0.48),
            (3.5, BassDegree::Octave, 0.4, 0.60),
        ],
        // Bar 2 — driving: the bounciest bar (and the flavor's loud chord lands here), two octave
        // bounces around a ghost root and fifth, pushing hardest into the turn.
        vec![
            (0.0, BassDegree::Root, 0.5, 0.82),
            (0.75, BassDegree::Octave, 0.22, 0.48),
            (1.5, BassDegree::Fifth, 0.4, 0.62),
            (2.0, BassDegree::Octave, 0.3, 0.50),
            (2.75, BassDegree::Fifth, 0.5, 0.54),
            (3.5, BassDegree::Octave, 0.4, 0.64),
        ],
        // Bar 3 — turnaround: opens with root + fifth, a mid-bar octave bounce, then the STRONGEST
        // push on the "and" of 4 (the loudest in the loop) that pulls the ear back to the top.
        vec![
            (0.0, BassDegree::Root, 0.7, 0.76),
            (1.0, BassDegree::Fifth, 0.5, 0.56),
            (1.75, BassDegree::Octave, 0.4, 0.52),
            (2.5, BassDegree::Root, 0.4, 0.56),
            (3.0, BassDegree::Fifth, 0.3, 0.58),
            (3.5, BassDegree::Octave, 0.45, 0.66),
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
