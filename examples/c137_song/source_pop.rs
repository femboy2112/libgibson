//! Opt-in SMOOTH-POP groove source (R2): a lead-free, Keys-carried band pocket at a pop harmonic
//! loop, carrying a syncopated bass identity. No foreground lead — the probe is whether kick / snare
//! / bass / keys alone make a recognizable, intentional pocket (the plan's R2 "can the band groove
//! without a virtuoso lead"). Both episodes are Keys-carried `Establish`, so the lead lane is
//! genuinely EMPTY (not a bus-muted ghost); the bass plays the authored [`GrooveLaw`] cell; the
//! harmony is a repeating [`SourceDomain::SmoothPop`] loop that closes as a groove-cycle, never on a
//! literal tonic. Everything here is additive and opt-in; no C137 source is touched.
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

/// The region-relative per-bar pop loop (one 4-bar cycle): i7 – VImaj7 – IIImaj7 – VII.
/// Am7 – Fmaj7 – Cmaj7 – G, all diatonic to the A-Aeolian world — so the scale pool stays valid and
/// no literal-tonic cadence is forced. Offsets are semitones above the region tonic (A); F=+8, C=+3,
/// G=+10. The loop closes by returning to Am7 (groove-cycle closure), never arriving on a bare tonic.
const POP_LOOP: [(i32, Quality); 4] = [
    (0, Quality::Min7),
    (8, Quality::Maj7),
    (3, Quality::Maj7),
    (10, Quality::Maj),
];

/// A sparse, syncopated keys comp over one 16-beat episode — chord/guide tones of each bar of the
/// loop, in A-Aeolian so the line stays consonant across the whole cycle. Tonic-relative semitone
/// steps (`PitchBasis::Semitones`), onsets on the quarter-beat lattice, conscious negative space.
fn keys_comp() -> Vec<MaterialEvent> {
    [
        (0.5f64, 7i32, 0.75f64, 0.60f32), // bar0 Am7: E (5th), off the "1"
        (2.5, 3, 0.50, 0.50),             // bar0 Am7: C (3rd)
        (4.5, 12, 0.75, 0.58),            // bar1 Fmaj7: A (3rd, up an octave)
        (6.5, 8, 0.50, 0.48),             // bar1 Fmaj7: F (colour)
        (8.5, 10, 0.75, 0.58),            // bar2 Cmaj7: G (5th)
        (10.5, 7, 0.50, 0.48),            // bar2 Cmaj7: E (3rd)
        (12.5, 10, 0.75, 0.56),           // bar3 G: G (root)
        (14.5, 5, 0.50, 0.50),            // bar3 G: D (5th)
    ]
    .into_iter()
    .map(|(onset, step, dur, accent)| MaterialEvent {
        onset,
        dur,
        accent,
        step: Some(step),
    })
    .collect()
}

/// The syncopated pop bass cell, one bar (4 beats): a punchy downbeat root anchor, an octave pop on
/// the "a" of 1, the fifth on the "and" of 2, an off-beat root on the "and" of 3, and a little fifth
/// pickup on the "e" of 4. Offset `0.0` is the chord root — the foundation guard's required anchor.
fn bass_cell() -> Vec<(f64, BassDegree, f32, f32)> {
    vec![
        (0.0, BassDegree::Root, 0.5, 0.82),
        (0.75, BassDegree::Octave, 0.25, 0.56),
        (1.5, BassDegree::Fifth, 0.5, 0.62),
        (2.5, BassDegree::Root, 0.5, 0.70),
        (3.25, BassDegree::Fifth, 0.25, 0.54),
    ]
}

/// Build the lead-free smooth-pop groove source for one seed. Two 16-beat Keys-carried `Establish`
/// episodes (8 bars = two loop cycles), the pop harmonic loop, and the syncopated bass cell.
pub fn source_pop_groove(seed: u64) -> CompiledArgument {
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
        events: keys_comp(),
    };
    let riff1 = MusicalReferent {
        id: ReferentId(1),
        pitch_basis: PitchBasis::Semitones,
        events: keys_comp(),
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
    let mut route = POP_LOOP.to_vec();
    route.extend_from_slice(&POP_LOOP);
    compiled.route = route;
    // Opt this ONE source into the smooth-pop closure law (groove-cycle, loop of 4 bars) and the
    // syncopated bass cell. No C137 source is affected; both are set after compile, like `route`.
    compiled.domain = SourceDomain::SmoothPop(PopSourceGrammar { loop_bars: 4 });
    compiled.groove = Some(GrooveLaw {
        bass_cell: bass_cell(),
    });
    compiled
}
