//! Source-level characterization of the current Homosapians "band-only" authoring path.
//!
//! These are NOT acceptance criteria for a future rehearsal composer. They expose what
//! the current 16-bar arrangement does *before* rendering: seeded metadata exists,
//! but the authored guitar-like/lead germ and per-bar bass cells are fixed; the
//! required carrier is a real (quiet) Lead lane; all sections claim Establish; and
//! loop_bars == route.len() proves no genuine repetition law. A future generative
//! rehearsal implementation should deliberately replace these characterizations.
//!
//! Unlike aesthetic A/Bs, this test requires no human-quality inference.

#[path = "../examples/c137_song/source_pop.rs"]
#[allow(dead_code)] // The example's CLI helpers are included for source inspection.
mod source_pop;

use gibson::audio::human_music::{
    argument::{ArgumentRelation, SourceDomain},
    kernel,
    score::Role,
    theory::Mode,
    MusicWorld,
};

fn world() -> MusicWorld {
    let mut w = MusicWorld::black_ice();
    w.tonic_pc = 9;
    w.mode = Mode::Aeolian;
    w.use_sevenths = true;
    w.allow_modal_mixture = true;
    w.tempo_bpm = 104.0;
    w
}

#[test]
fn current_authoring_seed_does_not_choose_the_backing_riff_harmony_or_bass_cells() {
    let a = source_pop::source_homosapians_backing(2112, source_pop::PopFlavor::Circle);
    let b = source_pop::source_homosapians_backing(7, source_pop::PopFlavor::Circle);
    assert_ne!(a.song.seed, b.song.seed, "different song seed inputs");

    // The live builder explicitly writes two literal referents, sixteen literal
    // bass cells and a fixed chord route, after constructing SongMap from seed.
    // This says NOTHING about independent seeded SongMap/plan differences.
    assert_eq!(
        a.argument.referents, b.argument.referents,
        "authored backing statements currently ignore seed variation"
    );
    assert_eq!(a.route, b.route, "the authored chord route is fixed");
    assert_eq!(
        a.groove.as_ref().expect("groove").bass_cells,
        b.groove.as_ref().expect("groove").bass_cells,
        "the bassist still receives the fixed per-bar figure"
    );
}

#[test]
fn chorus_is_labeled_establish_and_reuses_a_literal_source_not_a_rehearsed_response() {
    let source = source_pop::source_homosapians_backing(2112, source_pop::PopFlavor::Lift);
    let steps = &source.argument.steps;
    assert_eq!(steps.len(), 4, "four 16-beat episodes over 16 bars");
    assert!(
        steps
            .iter()
            .all(|s| matches!(s.relation, ArgumentRelation::Establish)),
        "verse AND chorus presently carry the same relation type"
    );
    assert_eq!(steps[0].referent, steps[1].referent);
    assert_eq!(steps[2].referent, steps[3].referent);
    assert_ne!(
        steps[0].referent, steps[2].referent,
        "verse and chorus materials are different prewritten referents"
    );

    // There is a dependency ordering but no source-derived 'guitarist proposed,
    // bassist answered, drummer answered, and guitar revised' provenance.
    // This characterization must not be mistaken for an audited rehearsal log.
}

#[test]
fn current_backing_uses_real_low_velocity_lead_events_to_occupy_a_carrier() {
    let source = source_pop::source_homosapians_backing(2112, source_pop::PopFlavor::Circle);
    let take = kernel::perform_song(&source, &world()).expect("backing is lawful");
    let score = take.score();
    let lead: Vec<_> = score.role_notes(Role::Lead).collect();
    assert!(!lead.is_empty(), "the symbolic backing is NOT lead-empty");
    assert!(
        lead.iter().all(|n| n.velocity <= 0.07),
        "the placeholder carrier is deliberately very quiet"
    );
    assert!(
        score.role_notes(Role::Keys).count() > 0,
        "the comp lane sounds alongside the scaffold"
    );

    // Physical audibility depends on the render patch, mix and nonlinear bus.
    // The assertion characterizes Score presence; it does NOT certify silence.
}

#[test]
fn current_full_length_loop_period_is_a_tautology_not_a_song_form_witness() {
    let source = source_pop::source_homosapians_backing(2112, source_pop::PopFlavor::Circle);
    let SourceDomain::SmoothPop(grammar) = &source.domain else {
        panic!("backing must be in the pop source domain");
    };
    let k = grammar.loop_bars;
    assert_eq!(source.route.len(), 16);
    assert_eq!(
        k,
        source.route.len(),
        "declaring the whole song as one loop leaves no actual recurrence to test"
    );

    // Arithmetic behind the current pop_route_law:
    // for every index i < k, i % k == i. ANY length-k route satisfies it,
    // including one with a changed interior chord.
    let mut mutated = source.route.clone();
    assert_ne!(mutated[4], mutated[5]);
    mutated[5] = mutated[4];
    assert_ne!(mutated, source.route);
    assert!(
        mutated.iter().enumerate().all(|(i, c)| *c == mutated[i % k]),
        "the loop-equality predicate is true even for an arbitrary changed route"
    );
}
