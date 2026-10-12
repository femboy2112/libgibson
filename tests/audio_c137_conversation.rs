#[path = "../examples/c137_conversation/source.rs"]
mod source;
use gibson::audio::human_music::{kernel, MusicWorld};
#[test]
fn first_conversation_is_a_real_committed_score() {
    let source = source::source(2112);
    let take = kernel::perform(&source, &MusicWorld::black_ice()).unwrap();
    println!("{}", take.score().summary());
    assert!(take.score().stale_hearings().is_empty());
    assert_eq!(take.hearings().len(), 2);
}

use gibson::audio::human_music::{
    action::Agent,
    argument::{CompiledArgument, ReferentId},
    interaction::Transform,
    plan::ArrangementRole,
    score::Role,
    voice::VoiceEventId,
};

fn take(seed: u64) -> kernel::CommittedPerformance {
    kernel::perform(&source::source(seed), &MusicWorld::black_ice()).unwrap()
}
fn notes(
    score: &gibson::audio::human_music::score::Score,
    role: Role,
) -> Vec<(f64, f32, i32, f32)> {
    score
        .role_notes(role)
        .map(|n| (n.start_beat, n.dur_beats, n.pitch, n.velocity))
        .collect()
}
fn recompile(s: &CompiledArgument) -> CompiledArgument {
    let mut out = s.argument.compile(&s.song).unwrap();
    out.route = s.route.clone();
    out
}
#[test]
fn different_equal_count_equal_energy_calls_make_different_choices() {
    let a = source::source(2112);
    let mut b = a.clone();
    b.argument.referents[0].events.last_mut().unwrap().step = Some(0);
    let b = recompile(&b);
    let a = kernel::perform(&a, &MusicWorld::black_ice()).unwrap();
    let b = kernel::perform(&b, &MusicWorld::black_ice()).unwrap();
    let call = |t: &kernel::CommittedPerformance| {
        t.score()
            .role_notes(Role::Lead)
            .filter(|n| n.start_beat < 4.0)
            .map(|n| (n.start_beat, n.dur_beats, n.velocity))
            .collect::<Vec<_>>()
    };
    assert_eq!(call(&a), call(&b));
    assert_ne!(
        a.composition().perf.interactions[0]
            .response
            .unwrap()
            .transform,
        Transform::Silence
    );
    assert_eq!(
        b.composition().perf.interactions[0]
            .response
            .unwrap()
            .transform,
        Transform::Silence
    );
    assert!(b.score().role_notes(Role::Keys).next().is_none());
    assert_eq!(notes(a.score(), Role::Bass), notes(b.score(), Role::Bass));
}
#[test]
fn muted_cue_changes_only_its_later_drum_response() {
    let s = source::source(2112);
    let a = kernel::perform(&s, &MusicWorld::black_ice()).unwrap();
    let mut muted = s;
    for p in &mut muted.song.plan.arrangement.phrases {
        p.keys = ArrangementRole::Silent;
    }
    let b = kernel::perform(&muted, &MusicWorld::black_ice()).unwrap();
    let anchors = |t: &kernel::CommittedPerformance| {
        t.score()
            .drums
            .iter()
            .filter(|h| h.prov.role_note == "pulse")
            .map(|h| (h.start_beat, h.voice, h.velocity))
            .collect::<Vec<_>>()
    };
    assert_eq!(anchors(&a), anchors(&b));
    assert_eq!(notes(a.score(), Role::Lead), notes(b.score(), Role::Lead));
    assert_eq!(notes(a.score(), Role::Bass), notes(b.score(), Role::Bass));
    assert!(a
        .score()
        .drums
        .iter()
        .any(|h| h.prov.role_note == "heard-keys"));
    assert!(!b
        .score()
        .drums
        .iter()
        .any(|h| h.prov.role_note == "heard-keys"));
    assert_eq!(b.hearings()[1].event_count(), 0);
}
#[test]
fn dynamic_and_source_changes_cannot_keep_the_final_hearing() {
    let t = take(2112);
    let mut changed = t.score().clone();
    changed
        .notes
        .iter_mut()
        .find(|n| n.role == Role::Keys)
        .unwrap()
        .velocity *= 0.5;
    assert!(changed.stale_hearings().is_empty()); // the compatibility observer is unchanged
    assert!(!t.hearings()[1].matches(&changed));
    assert!(matches!(
        t.validate_candidate(&changed),
        Err(kernel::KernelError::StaleObservation(_))
    ));
    let mut changed = t.score().clone();
    changed
        .notes
        .iter_mut()
        .find(|n| n.role == Role::Keys)
        .unwrap()
        .prov
        .material = None;
    assert!(!t.hearings()[1].matches(&changed));
}
#[test]
fn consumed_velocity_changes_a_real_downstream_fill() {
    let s = source::source(2112);
    let a = kernel::perform(&s, &MusicWorld::black_ice()).unwrap();
    let mut quieter = s.clone();
    for e in &mut quieter.argument.referents[0].events {
        e.accent *= 0.7;
    }
    let b = kernel::perform(&recompile(&quieter), &MusicWorld::black_ice()).unwrap();
    let fill = |t: &kernel::CommittedPerformance| {
        t.score()
            .drums
            .iter()
            .filter(|h| h.prov.role_note == "heard-keys")
            .map(|h| h.velocity)
            .collect::<Vec<_>>()
    };
    assert_ne!(fill(&a), fill(&b));
    assert_eq!(notes(a.score(), Role::Bass), notes(b.score(), Role::Bass));
}
#[test]
fn source_silence_and_bass_capacity_are_partial_composition_boundaries() {
    let t = take(2112);
    let mut crowded = t.score().clone();
    let mut extra = *crowded.role_notes(Role::Bass).next().unwrap();
    extra.pitch += 7;
    crowded.notes.push(extra);
    assert!(matches!(
        t.validate_candidate(&crowded),
        Err(kernel::KernelError::Capacity(Role::Bass))
    ));
    let mut intrusion = t.score().clone();
    let mut key = *intrusion.role_notes(Role::Keys).next().unwrap();
    key.start_beat = 9.0;
    intrusion.notes.push(key);
    assert!(t.validate_candidate(&intrusion).is_err());
    let mut stolen = t.score().clone();
    let root = *stolen
        .role_notes(Role::Bass)
        .find(|n| n.start_beat == 12.0)
        .unwrap();
    stolen
        .notes
        .retain(|n| VoiceEventId::of(n) != VoiceEventId::of(&root));
    stolen
        .voice_continuity
        .retain(|e| e.from != VoiceEventId::of(&root) && e.to != VoiceEventId::of(&root));
    assert!(t.validate_candidate(&stolen).is_err());
    let mut lead_intrusion = t.score().clone();
    let mut note = *lead_intrusion.role_notes(Role::Lead).next().unwrap();
    note.start_beat = 8.0;
    lead_intrusion.notes.push(note);
    assert!(t.validate_candidate(&lead_intrusion).is_err());
}
#[test]
fn source_owner_route_register_and_stage_refuse_before_a_take_exists() {
    let s = source::source(2112);
    let mut foreign = s.clone();
    foreign.placements[0].referent = ReferentId(99);
    assert!(kernel::perform(&foreign, &MusicWorld::black_ice()).is_err());
    let mut route = s.clone();
    route.route.pop();
    assert!(kernel::perform(&route, &MusicWorld::black_ice()).is_err());
    let mut off = s.clone();
    for p in &mut off.song.plan.arrangement.phrases {
        p.bass = ArrangementRole::Silent;
    }
    assert!(matches!(
        kernel::perform(&off, &MusicWorld::black_ice()),
        Err(kernel::KernelError::Stage(Agent::Bass))
    ));
    let mut overlap = s.clone();
    overlap.argument.referents[0].events[0].dur = 1.0;
    assert!(kernel::perform(&recompile(&overlap), &MusicWorld::black_ice()).is_err());
    let mut not_grid = s.clone();
    not_grid.argument.referents[0].events[1].onset += 0.125;
    assert!(kernel::perform(&recompile(&not_grid), &MusicWorld::black_ice()).is_err());
}
#[test]
fn relative_phase_recovers_and_available_keys_opportunity_stays_silent() {
    let t = take(2112);
    let p = &t.composition().perf;
    assert!(p.stage.on_stage_span(Agent::Keys, 8.0, 12.0));
    assert_eq!(
        p.interactions[1].response.unwrap().transform,
        Transform::Silence
    );
    assert!(t.score().role_notes(Role::Keys).all(|n| n.start_beat < 8.0));
    for n in t.score().role_notes(Role::Keys) {
        assert!(
            (((n.start_beat - 0.0625) * 4.0).round() - (n.start_beat - 0.0625) * 4.0).abs() < 1e-8
        );
    }
    assert!(t
        .score()
        .role_notes(Role::Lead)
        .any(|n| n.start_beat == 12.0));
    assert!(t
        .score()
        .role_notes(Role::Bass)
        .any(|n| n.start_beat == 11.5 && n.pitch == 44));
    assert!(t
        .score()
        .role_notes(Role::Bass)
        .any(|n| n.start_beat == 12.0 && n.pitch == 45));
}
#[test]
fn fresh_sources_and_transposed_rooms_preserve_the_bounded_contract() {
    for seed in [7, 99, 321, 8191] {
        for tonic in [2, 5, 9] {
            let mut world = MusicWorld::black_ice();
            world.tonic_pc = tonic;
            world.tempo_bpm = 144.0;
            let s = source::source(seed);
            let t = kernel::perform(&s, &world).unwrap();
            t.validate_candidate(t.score()).unwrap();
            assert!(t.hearings().iter().all(|h| h.matches(t.score())));
        }
    }
}
