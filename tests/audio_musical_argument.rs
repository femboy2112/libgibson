//! Finite semantic contrast and hostile realization tests. Expected outcomes are
//! stated before each mutation: raw source-preserving A/B pass; false relationships
//! fail; irrelevant labels and lawful octave transport remain equivalent.
use gibson::audio::{
    buffer::StereoBlock,
    human_music::{
        action::Agent,
        argument::{
            ArgumentEnding, ArgumentFamily, ArgumentRelation, CompiledArgument, MusicalArgument,
        },
        argument_witness::{acoustic_exposure, observe_argument},
        composer::Composer,
        contract::CompositionGrammar,
        functor::perform_argument,
        meaning::MeaningPlan,
        performance::PerformanceOptions,
        policy::PerformanceProfile,
        score::{Note, Provenance, Role, Score},
        semantic::deflected_lift_trace,
        synth::StemMask,
        theory::Mode,
        HumanMusicSynth, MusicWorld, SongMap,
    },
    OfflineRenderer, SampleRate,
};

fn base(seed: u64) -> SongMap {
    SongMap::compose(
        &deflected_lift_trace(128.0),
        seed,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    )
}

fn world() -> MusicWorld {
    let mut world = MusicWorld::black_ice();
    world.tempo_bpm = 108.0;
    world
}

fn argument_options() -> PerformanceOptions {
    PerformanceOptions {
        actions: false,
        ..PerformanceOptions::default()
    }
}

fn composition(family: ArgumentFamily, seed: u64) -> (CompiledArgument, Score) {
    let compiled = MusicalArgument::calibration(family, seed)
        .unwrap()
        .compile(&base(2112))
        .unwrap();
    let score = perform_argument(
        &compiled,
        &world(),
        argument_options(),
        PerformanceProfile::BAND,
    )
    .unwrap()
    .score;
    assert!(observe_argument(&compiled, &score, world().tonic_pc).valid());
    (compiled, score)
}

fn selected(
    n: &Note,
    p: &gibson::audio::human_music::argument::ArgumentPlacement,
    role: Role,
) -> bool {
    n.role == role
        && n.start_beat >= p.start_beat - 1e-6
        && n.start_beat < p.start_beat + p.span_beats - 1e-6
}

#[test]
fn same_affect_distinct_generated_arguments_are_raw_score_distinguishable() {
    // Contrast domain D = A/B x these held-out seeds, one fixed affect/form source.
    for seed in [770_201, 770_213, 770_237] {
        let (a, sa) = composition(ArgumentFamily::CallAndEarnedAnswer, seed);
        let (b, sb) = composition(ArgumentFamily::PromiseDeniedReturn, seed);
        let wa = observe_argument(&a, &sa, 9);
        let wb = observe_argument(&b, &sb, 9);
        assert!(wa.valid() && wb.valid());
        assert_ne!(
            wa.signature(),
            wb.signature(),
            "raw A/B collision at fresh seed {seed}"
        );
        assert_eq!(a.placements.len(), b.placements.len());
        assert_eq!(
            a.placements
                .iter()
                .map(|p| (p.start_beat, p.span_beats))
                .collect::<Vec<_>>(),
            b.placements
                .iter()
                .map(|p| (p.start_beat, p.span_beats))
                .collect::<Vec<_>>(),
            "matched form timing is not an argument discriminator"
        );
        assert_eq!(
            format!("{:?}", a.song.timeline),
            format!("{:?}", b.song.timeline),
            "affect source must stay fixed"
        );
    }
}

#[test]
fn wrong_answer_and_destroyed_gate_rhythm_are_rejected_without_trusting_labels() {
    let (compiled, score) = composition(ArgumentFamily::CallAndEarnedAnswer, 770_401);
    let answer = compiled
        .placements
        .iter()
        .find(|p| matches!(p.relation, ArgumentRelation::Answer { .. }))
        .unwrap();
    let original_meaning = MeaningPlan::observe(&compiled.song);
    let original_signature = observe_argument(&compiled, &score, 9).signature().to_vec();
    // μ takes SongMap, so it cannot distinguish any of these hostile Score realizations.
    for mutation in 0..5 {
        let mut changed = score.clone();
        let indices: Vec<_> = changed
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| selected(n, answer, Role::Keys))
            .map(|(i, _)| i)
            .collect();
        assert!(indices.len() >= 6);
        match mutation {
            0 => changed.notes[indices[2]].pitch += 3, // defining interval destroyed
            1 => {
                for i in indices {
                    changed.notes[i].pitch += 1;
                }
            } // plausible contour, wrong tonic
            2 => changed.notes[indices[1]].dur_beats *= 0.5, // onset preserved, sounding gate lost
            3 => changed.notes[indices[1]].start_beat += 0.125, // identity rhythm destroyed
            _ => {
                let a = changed.notes[indices[0]].pitch;
                changed.notes[indices[0]].pitch = changed.notes[indices[1]].pitch;
                changed.notes[indices[1]].pitch = a;
            }
        }
        let mutated = observe_argument(&compiled, &changed, 9);
        assert!(!mutated.valid(), "mutation {mutation} accepted");
        assert_ne!(
            mutated.signature(),
            original_signature,
            "finite raw observer collapsed mutation {mutation}"
        );
        assert_eq!(MeaningPlan::observe(&compiled.song), original_meaning);
    }
}

#[test]
fn a_two_carrier_payoff_requires_both_carriers_and_full_written_rests() {
    let (compiled, score) = composition(ArgumentFamily::PromiseDeniedReturn, 770_509);
    let returning = compiled.placements.last().unwrap();
    assert!(returning.carriers.contains(&Agent::Lead) && returning.carriers.contains(&Agent::Keys));
    let mut only_one = score.clone();
    only_one
        .notes
        .retain(|n| !selected(n, returning, Role::Keys));
    assert!(!observe_argument(&compiled, &only_one, 9).valid());
    let mut filled_rest = score.clone();
    let rest_start = returning
        .events
        .iter()
        .map(|e| e.onset + e.dur)
        .fold(0.0, f64::max);
    assert!(rest_start + 0.25 < returning.span_beats);
    filled_rest.notes.push(Note::new(
        returning.start_beat + rest_start + 0.1,
        0.1,
        69,
        0.5,
        Role::Lead,
        Provenance::new(gibson::audio::human_music::form::SectionKind::Coda),
    ));
    assert!(
        !observe_argument(&compiled, &filled_rest, 9).valid(),
        "extra note inside reserved silence accepted"
    );
}

#[test]
fn return_before_introduction_and_generic_answer_fail_independent_gluing() {
    let (mut compiled, score) = composition(ArgumentFamily::CallAndEarnedAnswer, 770_611);
    compiled.placements.last_mut().unwrap().start_beat = 0.0;
    assert!(observe_argument(&compiled, &score, 9)
        .violations()
        .iter()
        .any(|v| v.reason == "unearned dependency order"));
    let (mut compiled, score) = composition(ArgumentFamily::CallAndEarnedAnswer, 770_611);
    let answer = compiled
        .argument
        .steps
        .iter_mut()
        .find(|s| matches!(s.relation, ArgumentRelation::Answer { .. }))
        .unwrap();
    answer.depends_on.clear();
    assert!(!observe_argument(&compiled, &score, 9).valid());
}

#[test]
fn unrelated_return_and_untaught_denial_fail_source_relations_even_with_true_notes() {
    let (mut compiled, score) = composition(ArgumentFamily::PromiseDeniedReturn, 770_619);
    // Coherent material in its own frame is insufficient to return to a different source.
    let mut foreign = compiled.argument.referents[0].clone();
    foreign.id.0 = 1;
    foreign.events[2].step = Some(4);
    compiled.argument.referents.push(foreign);
    compiled.argument.steps.last_mut().unwrap().referent.0 = 1;
    assert!(observe_argument(&compiled, &score, 9)
        .violations()
        .iter()
        .any(|v| v.reason == "relationship changes the defining source"));
    let (mut compiled, score) = composition(ArgumentFamily::PromiseDeniedReturn, 770_619);
    compiled.argument.steps[0].transform.completion = None;
    assert!(observe_argument(&compiled, &score, 9)
        .violations()
        .iter()
        .any(|v| v.reason == "denied destination was never taught"));
}

#[test]
fn a_resolution_label_without_actual_harmonic_preparation_is_rejected() {
    let (compiled, mut score) = composition(ArgumentFamily::CallAndEarnedAnswer, 770_627);
    let question = compiled
        .placements
        .iter()
        .find(|p| matches!(p.relation, ArgumentRelation::Question { .. }))
        .unwrap();
    let good = score.clone();
    // Reproduce the independently observed source bug: generic Lift colour
    // changed source E7 into Emaj7. Root identity alone did not preserve the promise.
    for chord in &mut score.chords {
        if chord.start_beat >= question.start_beat
            && chord.start_beat < question.start_beat + question.span_beats
        {
            chord.chord = gibson::audio::human_music::theory::Chord::new(
                4,
                gibson::audio::human_music::theory::Quality::Maj7,
            );
        }
    }
    assert!(!observe_argument(&compiled, &score, 9).valid());
    score = good;
    for chord in &mut score.chords {
        if chord.start_beat >= question.start_beat
            && chord.start_beat < question.start_beat + question.span_beats
        {
            chord.chord = gibson::audio::human_music::theory::Chord::new(
                9,
                gibson::audio::human_music::theory::Quality::Min7,
            );
        }
    }
    assert!(!observe_argument(&compiled, &score, 9).valid());
    score.chords.clear();
    assert!(!observe_argument(&compiled, &score, 9).valid());
}

#[test]
fn conflict_reconciliation_is_explicitly_unimplemented() {
    assert!(MusicalArgument::calibration(ArgumentFamily::ConflictReconciliation, 7).is_err());
}

#[test]
fn native_swiss_world_refuses_the_required_dominant_seventh_instead_of_faking_it() {
    let argument =
        MusicalArgument::calibration(ArgumentFamily::CallAndEarnedAnswer, 770_631).unwrap();
    let compiled = argument.compile(&base(2112)).unwrap();
    assert!(perform_argument(
        &compiled,
        &MusicWorld::swiss_signal(),
        argument_options(),
        PerformanceProfile::BAND
    )
    .is_err());
}

#[test]
fn octave_orchestration_world_and_irrelevant_metadata_controls_remain_equivalent() {
    let (compiled, score) = composition(ArgumentFamily::CallAndEarnedAnswer, 770_701);
    let original = observe_argument(&compiled, &score, 9);
    let mut octaves = score.clone();
    for n in &mut octaves.notes {
        n.pitch += 12;
    }
    let moved = observe_argument(&compiled, &octaves, 9);
    assert!(moved.valid());
    assert_eq!(original.signature(), moved.signature());
    let mut metadata = score.clone();
    for n in &mut metadata.notes {
        n.prov.role_note = "irrelevant metadata mutation";
        n.prov.motif_id = Some(251);
    }
    let renamed = observe_argument(&compiled, &metadata, 9);
    assert!(renamed.valid());
    assert_eq!(original.signature(), renamed.signature());
    for mut w in [MusicWorld::vapor95(), MusicWorld::swiss_signal()] {
        // Fixed chart frame controls timbre/world changes; hybrid major/minor is not judged.
        w.mode = Mode::Aeolian;
        w.tonic_pc = 9;
        w.tempo_bpm = 108.0;
        w.use_sevenths = true;
        w.allow_modal_mixture = true;
        let c =
            perform_argument(&compiled, &w, argument_options(), PerformanceProfile::BAND).unwrap();
        let observed = observe_argument(&compiled, &c.score, 9);
        assert!(observed.valid());
        assert_eq!(original.signature(), observed.signature());
    }
    // A lawful source-bound orchestration change is explicitly made before compilation.
    let mut orchestration = compiled.argument.clone();
    for step in &mut orchestration.steps {
        step.carriers = vec![Agent::Lead];
    }
    let arranged = orchestration.compile(&base(2112)).unwrap();
    let c = perform_argument(
        &arranged,
        &world(),
        argument_options(),
        PerformanceProfile::BAND,
    )
    .unwrap();
    let observed = observe_argument(&arranged, &c.score, 9);
    assert!(observed.valid());
    assert_eq!(original.signature(), observed.signature());
}

#[test]
fn intentional_unresolved_closure_is_allowed_but_undeclared_debt_is_rejected() {
    let mut argument =
        MusicalArgument::calibration(ArgumentFamily::CallAndEarnedAnswer, 770_811).unwrap();
    argument.steps.truncate(2);
    argument.ending = ArgumentEnding::IntentionallyUnresolved;
    let mut compiled = argument.compile(&base(2112)).unwrap();
    let score = perform_argument(
        &compiled,
        &world(),
        argument_options(),
        PerformanceProfile::BAND,
    )
    .unwrap()
    .score;
    assert!(observe_argument(&compiled, &score, 9).valid());
    compiled.argument.ending = ArgumentEnding::Resolved;
    assert!(!observe_argument(&compiled, &score, 9).valid());
}

fn render(score: &Score, mask: StemMask, block: usize) -> StereoBlock {
    let sr = SampleRate::new(8_000).unwrap();
    let mut synth = HumanMusicSynth::new(score, &world(), sr);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let output = OfflineRenderer::new(sr, block).render(&mut synth, frames);
    assert!(!output.had_nonfinite);
    output.audio
}

#[test]
fn pcm_exposure_is_a_separate_reject_only_gate_and_metadata_is_pcm_inert() {
    let mut score = Score::new(108.0, 4.0, 4.0);
    score.notes.push(Note::new(
        0.0,
        1.0,
        69,
        0.8,
        Role::Lead,
        Provenance::new(gibson::audio::human_music::form::SectionKind::A),
    ));
    let full = render(&score, StemMask::full(), 64);
    let silent = render(&score, StemMask::silent(), 64);
    let exposed = acoustic_exposure(&full, &silent, 0, full.frames());
    assert!(exposed.finite && !exposed.reject_absent_signal);
    let hidden = acoustic_exposure(&silent, &silent, 0, silent.frames());
    assert!(hidden.reject_absent_signal);
    score.notes[0].prov.role_note = "content-inert metadata";
    assert_eq!(full, render(&score, StemMask::full(), 64));
    assert_eq!(
        full,
        render(&score, StemMask::full(), 257),
        "PCM is block-size reproducible"
    );
}
