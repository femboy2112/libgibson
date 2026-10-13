//! Four-section long-form source: the first AUDIBLE zoom-out of the C137 phase-change engine. A full
//! VERSE that hangs open, a distinct-but-related HOOK that lands home, a BRIDGE that DEPARTS (recycles
//! a hook cell, lifted and thinned, ending open and wanting the return), and a RETURN that restates
//! the hook — laid as one continuous 64-beat argument across four 16-beat episodes.
//!
//! This is the generalization of `c137_song/source.rs` (the frozen two-episode verse→hook control)
//! to the full narrative arc the semantic layer always meant: verse-asks-open → hook-answers-home →
//! bridge-departs → hook-returns-home, closing `Resolved` on an EARNED ending. The referents are the
//! SAME developed material the control uses — the verse and hook come from
//! [`generate_developed_pair`] (embellished sixteenths, shared pocket, phrase dynamics) so NO lead
//! density is lost in the zoom-out — plus a developed BRIDGE from [`generate_developed_bridge`]
//! (deliberately thinner, per the teacher, so the texture contrast itself signals the phase change).
//!
//! The band is never re-sequenced by hand: the four episodes drive the SAME memory-law call/answer,
//! one-shared-pocket ensemble the control drives, so the return comes home sounding different from the
//! first hook because the band has carried the bridge's debt — a semantically justified phase change,
//! not a copy-paste. The eight-bar control (`source.rs`) stays byte-identical; this is an opt-in arm.
use gibson::audio::human_music::{
    action::Agent,
    argument::{
        ArcSpan, ArgumentEnding, ArgumentFamily, ArgumentRelation, ArgumentStep, ArgumentStepId,
        ArgumentTransform, CompiledArgument, MusicalArgument, MusicalReferent, ReferentId,
    },
    contract::CompositionGrammar,
    plan::ArrangementRole,
    semantic::{EventKind, SemanticEvent, SemanticState, SemanticTrace, Tone},
    theme_family::{generate_developed_bridge, generate_developed_pair},
    theory::{PitchBasis, Quality},
    SongMap,
};

/// The region-relative per-bar charts each section travels. The verse moves pre-dominant → dominant
/// and hangs open on its V7; the hook lands home on the tonic; the bridge pivots away through the
/// relative-minor region and ends OPEN on the home dominant (wanting the return); the return lands
/// home again on the hook chart.
const VERSE_PROG: [(i32, Quality); 4] = [
    (0, Quality::Min),
    (5, Quality::Min),
    (7, Quality::Dom7),
    (0, Quality::Min),
];
const HOOK_PROG: [(i32, Quality); 4] = [
    (8, Quality::Maj),
    (10, Quality::Maj),
    (5, Quality::Min),
    (0, Quality::Min),
];
/// The bridge's four-bar departure: the tonic turns into its own dominant (A7 = V7/iv, the pivot —
/// read as a DRIVING function, not home, by `harmonic_drive`), tonicises the relative-minor region,
/// lifts to the submediant, and ends OPEN on the home dominant (E7 = V7), wanting the return.
const BRIDGE_PROG: [(i32, Quality); 4] = [
    (0, Quality::Dom7),
    (5, Quality::Min),
    (8, Quality::Maj),
    (7, Quality::Dom7),
];

/// The 64-beat, four-section argument: verse `Establish` → hook `Consequent` → bridge `Depart` →
/// hook `Return`, all carried by the lead, with DEVELOPED referents so the zoom-out keeps the
/// control's lead density. Factored out so the red tests can assert the departure and return laws on
/// the pre-compiled argument.
pub fn argument(seed: u64) -> (MusicalArgument, SongMap) {
    let trace = SemanticTrace {
        total_beats: 64.0,
        events: vec![
            SemanticEvent {
                at_beat: 0.0,
                state: SemanticState::toned(Tone::Accent),
                kind: EventKind::ActChanged,
            },
            SemanticEvent {
                at_beat: 60.0,
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
        64.0,
        contract,
    );
    for p in &mut song.plan.arrangement.phrases {
        p.lead = ArrangementRole::Support;
        p.keys = ArrangementRole::Support;
        p.bass = ArrangementRole::Foundation;
        p.drums = ArrangementRole::Support;
        p.pad = ArrangementRole::Silent;
    }
    // The developed verse/hook pair (the control's exact material path): embellished into played
    // lines, laid into the shared pocket, shaped by a phrase dynamic — with the kinship law re-checked
    // on the DEVELOPED pair. The bridge DEPARTS from the developed hook (recycles a cell, lifts the
    // register, ends open) and is itself developed (pocket + dynamics, no embellishment — it stays
    // thin so the texture contrast carries the phase change). The return reuses the hook referent.
    let pair = generate_developed_pair(seed, 16.0, 4.0, &VERSE_PROG, &HOOK_PROG);
    let bridge_events = generate_developed_bridge(seed, &pair.hook, 16.0, 4.0, &BRIDGE_PROG);
    let verse = MusicalReferent {
        id: ReferentId(0),
        pitch_basis: PitchBasis::Semitones,
        events: pair.verse,
    };
    let hook = MusicalReferent {
        id: ReferentId(1),
        pitch_basis: PitchBasis::Semitones,
        events: pair.hook,
    };
    let bridge = MusicalReferent {
        id: ReferentId(2),
        pitch_basis: PitchBasis::Semitones,
        events: bridge_events,
    };
    // Each episode is four one-bar phrases (sixteen phrases land on beats 0,4,…,60). The sections
    // begin on beats 0, 16, 32, 48 — found by position so the arc binds to real phrase indices.
    let at = |beat: f64| -> u32 {
        song.plan
            .form
            .phrases
            .iter()
            .position(|p| p.start_beat() == beat)
            .expect("sixteen one-bar phrases put episode boundaries at 0/16/32/48") as u32
    };
    let (p0, p1, p2, p3) = (at(0.0), at(16.0), at(32.0), at(48.0));
    let total_phrases = song.plan.form.phrases.len() as u32;
    let argument = MusicalArgument {
        family: ArgumentFamily::CallAndEarnedAnswer,
        referents: vec![verse, hook, bridge],
        ending: ArgumentEnding::Resolved,
        steps: vec![
            ArgumentStep {
                id: ArgumentStepId(0),
                referent: ReferentId(0),
                transform: ArgumentTransform::default(),
                relation: ArgumentRelation::Establish,
                depends_on: vec![],
                carriers: vec![Agent::Lead],
            },
            ArgumentStep {
                id: ArgumentStepId(1),
                referent: ReferentId(1),
                transform: ArgumentTransform::default(),
                relation: ArgumentRelation::Consequent {
                    antecedent: ArgumentStepId(0),
                },
                depends_on: vec![ArgumentStepId(0)],
                carriers: vec![Agent::Lead],
            },
            ArgumentStep {
                id: ArgumentStepId(2),
                referent: ReferentId(2),
                transform: ArgumentTransform::default(),
                relation: ArgumentRelation::Depart {
                    source: ArgumentStepId(1),
                },
                depends_on: vec![ArgumentStepId(1)],
                carriers: vec![Agent::Lead],
            },
            ArgumentStep {
                id: ArgumentStepId(3),
                referent: ReferentId(1),
                transform: ArgumentTransform::default(),
                relation: ArgumentRelation::Return {
                    source: ArgumentStepId(1),
                },
                // The return depends on BOTH the hook it restates and the bridge it returns FROM, so
                // the dependency graph records the earned journey: the home-coming follows the
                // departure (the content witness enforces the bridge completes before the return).
                depends_on: vec![ArgumentStepId(1), ArgumentStepId(2)],
                carriers: vec![Agent::Lead],
            },
        ],
        arc: Some(vec![
            ArcSpan {
                step: ArgumentStepId(0),
                start_phrase: p0,
                phrase_count: p1 - p0,
                start_beat: 0.0,
                span_beats: 16.0,
            },
            ArcSpan {
                step: ArgumentStepId(1),
                start_phrase: p1,
                phrase_count: p2 - p1,
                start_beat: 16.0,
                span_beats: 16.0,
            },
            ArcSpan {
                step: ArgumentStepId(2),
                start_phrase: p2,
                phrase_count: p3 - p2,
                start_beat: 32.0,
                span_beats: 16.0,
            },
            ArcSpan {
                step: ArgumentStepId(3),
                start_phrase: p3,
                phrase_count: total_phrases - p3,
                start_beat: 48.0,
                span_beats: 16.0,
            },
        ]),
    };
    (argument, song)
}

pub fn source(seed: u64) -> CompiledArgument {
    let (argument, song) = argument(seed);
    let mut compiled = argument
        .compile(&song)
        .expect("seeded four-section long-form source compiles");
    // The sixteen-bar route is verse → hook → bridge → hook: the verse travels to its open V7, the
    // hook lands home, the bridge pivots and ends open on the home dominant, the return lands home
    // again. Declared here so the long-form domain and witness read the exact traveling harmony.
    let mut route = VERSE_PROG.to_vec();
    route.extend_from_slice(&HOOK_PROG);
    route.extend_from_slice(&BRIDGE_PROG);
    route.extend_from_slice(&HOOK_PROG);
    compiled.route = route;
    compiled
}
