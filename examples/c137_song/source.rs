//! Two-episode long-form source: a full VERSE that hangs open, and a distinct-but-related HOOK
//! that lands home, laid as one continuous 32-beat argument. Shared by the playable example and
//! the long-form kernel tests.
//!
//! Modeled on `examples/c137_conversation/source.rs`, but where that source crops the generated
//! phrase to its four-event opening CELL and converses within four bars, this one carries the
//! COMPLETE generated phrase (the only scale at which `theme_family`'s `AlteredConsequent` law
//! holds: the verse→hook kinship is a whole-phrase relationship, false on a one-bar prefix). The
//! verse `Establish`es; the hook is its resolving `Consequent` — a genuinely different tune that
//! shares the antecedent's DNA and closes on the tonic — so the eight bars read as one arc:
//! verse-asks-open → hook-answers-home.
use gibson::audio::human_music::{
    action::Agent,
    argument::{
        ArcSpan, ArgumentEnding, ArgumentFamily, ArgumentRelation, ArgumentStep, ArgumentStepId,
        ArgumentTransform, CompiledArgument, MusicalArgument, MusicalReferent, ReferentId,
    },
    contract::CompositionGrammar,
    plan::ArrangementRole,
    semantic::{EventKind, SemanticEvent, SemanticState, SemanticTrace, Tone},
    theme_family::generate_pair,
    theory::{PitchBasis, Quality},
    SongMap,
};

/// The region-relative per-bar charts the verse and hook travel. The verse moves
/// pre-dominant → dominant and hangs open on its V7; the hook lands home on the tonic.
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

pub fn source(seed: u64) -> CompiledArgument {
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
    for p in &mut song.plan.arrangement.phrases {
        p.lead = ArrangementRole::Support;
        p.keys = ArrangementRole::Support;
        p.bass = ArrangementRole::Foundation;
        p.drums = ArrangementRole::Support;
        p.pad = ArrangementRole::Silent;
    }
    // The full generated phrases, used as-is: the verse rises and ends Open, the hook is its
    // AlteredConsequent and ends Closed. No cropping, no re-clamping — those obligations ARE the
    // referents (and the kinship law is measured over the whole phrase, not a prefix).
    let pair = generate_pair(seed, 16.0, 4.0, &VERSE_PROG, &HOOK_PROG);
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
    // Episode 2 begins at the one-bar phrase that starts on beat 16 (the eight one-bar phrases
    // land on beats 0,4,…,28). The verse section covers the first four, the hook the last four.
    let hook_phrase =
        song.plan
            .form
            .phrases
            .iter()
            .position(|p| p.start_beat() == 16.0)
            .expect("eight one-bar phrases put an episode boundary at beat 16") as u32;
    let total_phrases = song.plan.form.phrases.len() as u32;
    let argument = MusicalArgument {
        family: ArgumentFamily::CallAndEarnedAnswer,
        referents: vec![verse, hook],
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
        ],
        arc: Some(vec![
            ArcSpan {
                step: ArgumentStepId(0),
                start_phrase: 0,
                phrase_count: hook_phrase,
                start_beat: 0.0,
                span_beats: 16.0,
            },
            ArcSpan {
                step: ArgumentStepId(1),
                start_phrase: hook_phrase,
                phrase_count: total_phrases - hook_phrase,
                start_beat: 16.0,
                span_beats: 16.0,
            },
        ]),
    };
    let mut compiled = argument
        .compile(&song)
        .expect("seeded long-form source compiles");
    // The eight-bar route is the verse chart followed by the hook chart (verse bars travel to the
    // open V7; hook bars land home). Declared here so the long-form domain and witness can read it.
    let mut route = VERSE_PROG.to_vec();
    route.extend_from_slice(&HOOK_PROG);
    compiled.route = route;
    compiled
}
