//! Original seeded theme cell; shared by the playable example and hostile integration tests.
use gibson::audio::human_music::{
    action::Agent,
    argument::{
        ArcSpan, ArgumentEnding, ArgumentFamily, ArgumentRelation, ArgumentStep, ArgumentStepId,
        ArgumentTransform, CompiledArgument, Completion, MusicalArgument, MusicalReferent,
        ReferentId,
    },
    contract::CompositionGrammar,
    plan::ArrangementRole,
    semantic::{EventKind, SemanticEvent, SemanticState, SemanticTrace, Tone},
    theme_family::generate_pair,
    theory::{PitchBasis, Quality},
    SongMap,
};

pub fn source(seed: u64) -> CompiledArgument {
    let trace = SemanticTrace {
        total_beats: 16.0,
        events: vec![
            SemanticEvent {
                at_beat: 0.0,
                state: SemanticState::toned(Tone::Accent),
                kind: EventKind::ActChanged,
            },
            SemanticEvent {
                at_beat: 12.0,
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
        16.0,
        contract,
    );
    for p in &mut song.plan.arrangement.phrases {
        p.lead = ArrangementRole::Support;
        p.keys = ArrangementRole::Support;
        p.bass = ArrangementRole::Foundation;
        p.drums = ArrangementRole::Support;
        p.pad = ArrangementRole::Silent;
    }
    let verse = [
        (0, Quality::Min),
        (5, Quality::Min),
        (7, Quality::Dom7),
        (0, Quality::Min),
    ];
    let hook = [
        (8, Quality::Maj),
        (10, Quality::Maj),
        (5, Quality::Min),
        (0, Quality::Min),
    ];
    let pair = generate_pair(seed, 16.0, 4.0, &verse, &hook);
    let mut events = pair.verse[..4].to_vec();
    // A source-level cell extraction, with an explicit breath and grid-aligned completion site.
    // No performed events are cropped. These exact gates become the referent's obligations.
    for i in 0..events.len() - 1 {
        events[i].dur = events[i].dur.min(events[i + 1].onset - events[i].onset);
    }
    events.last_mut().unwrap().dur = 0.5;
    let source = MusicalReferent {
        id: ReferentId(0),
        pitch_basis: PitchBasis::Semitones,
        events,
    };
    let last_phrase = song
        .plan
        .form
        .phrases
        .iter()
        .position(|p| p.start_beat() == 12.0)
        .expect("four-bar source has return boundary at12");
    let argument = MusicalArgument {
        family: ArgumentFamily::PromiseDeniedReturn,
        referents: vec![source],
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
                referent: ReferentId(0),
                transform: ArgumentTransform {
                    completion: Some(Completion { step: 0, dur: 0.75 }),
                    ..Default::default()
                },
                relation: ArgumentRelation::Return {
                    source: ArgumentStepId(0),
                },
                depends_on: vec![ArgumentStepId(0)],
                carriers: vec![Agent::Lead],
            },
        ],
        arc: Some(vec![
            ArcSpan {
                step: ArgumentStepId(0),
                start_phrase: 0,
                phrase_count: last_phrase as u32,
                start_beat: 0.0,
                span_beats: 12.0,
            },
            ArcSpan {
                step: ArgumentStepId(1),
                start_phrase: last_phrase as u32,
                phrase_count: 1,
                start_beat: 12.0,
                span_beats: 4.0,
            },
        ]),
    };
    let mut compiled = argument.compile(&song).expect("seeded source compiles");
    compiled.route = verse.to_vec();
    compiled
}
