//! Round XII pre-intervention falsifiers. These assert the desired contracts and initially fail.
use super::composer::Composer;
use super::material::{resolve_roles, MaterialRole};
use super::performance::{PerformanceOptions, PerformancePlan};
use super::pitch::{classify, pc_mask, PitchContext};
use super::score::PitchFunction;
use super::semantic::deflected_lift_trace;
use super::song::SongMap;
use super::theory::{Chord, Mode, Quality, Scale};
use super::world::MusicWorld;

fn context(pitch: i32) -> PitchContext {
    PitchContext {
        pitch,
        onset: 5.0,
        duration: 0.5,
        prev: Some(67),
        next: Some(60),
        next_onset: Some(5.5),
        prev_chord: Some(Chord::new(7, Quality::Dom7)),
        cur: Some(Chord::new(0, Quality::Maj)),
        next_chord: None,
        next_boundary: None,
        is_strong: true,
        licensed: 0,
    }
}

#[test]
fn r12_false_suspension() {
    assert_ne!(
        classify(&context(62), &Scale::new(0, Mode::Ionian)),
        Some(PitchFunction::Suspension)
    );
}

#[test]
fn r12_fake_arrival() {
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let perf = PerformancePlan::from_song(
        &song,
        &MusicWorld::swiss_signal(),
        PerformanceOptions::default(),
    );
    let c = &perf.contexts[0];
    let pc = *c.palette.tensions.first().expect("available color witness");
    let ev = [(
        c.start_beat,
        Some(60 + pc),
        Some(PitchFunction::LicensedExtension),
    )];
    assert_ne!(resolve_roles(&ev, &perf)[0], MaterialRole::Arrival);
}

#[test]
fn r12_local_license_cannot_distinguish_orphan_from_owned_color() {
    let mut c = context(69);
    c.cur = Some(Chord::new(0, Quality::Maj7));
    c.licensed = pc_mask(&[9]);
    c.next_chord = Some(Chord::new(5, Quality::Maj7));
    c.next_boundary = Some(8.0);
    c.next = Some(60); // unrelated leap, no carried color or resolving step
    let orphan = classify(&c, &Scale::new(0, Mode::Ionian));
    c.next = Some(69); // common-tone continuation is available
    assert_eq!(orphan, Some(PitchFunction::LicensedExtension));
    assert_eq!(orphan, classify(&c, &Scale::new(0, Mode::Ionian)));
}

#[test]
#[ignore = "R12 pre-intervention falsifier: next-target intervention cannot affect search"]
fn r12_next_targets_have_no_live_reader_at_baseline() {
    use super::harmony::ChordSpan;
    use super::motif::{realize_line, LineRequest, LineStyle, Motif};
    let scale = Scale::new(0, Mode::Ionian);
    let chords = [
        ChordSpan::test(0.0, 4.0, Chord::new(0, Quality::Maj7)),
        ChordSpan::test(4.0, 4.0, Chord::new(5, Quality::Maj7)),
    ];
    let contexts = super::context::analyze(&chords, &scale);
    let mut changed = contexts.clone();
    changed[0].palette.next_targets = vec![1, 6];
    let motif = Motif::seed_a();
    let req = LineRequest {
        motif: &motif,
        chords: &chords,
        contexts: &contexts,
        scale: &scale,
        root_degree: 0,
        octave: 4,
        start_beat: 0.0,
        prev_pitch: None,
        style: LineStyle::plain(),
        max_candidates: 6,
        arrival: None,
    };
    let a = realize_line(&req);
    let b = realize_line(&LineRequest {
        contexts: &changed,
        ..req
    });
    assert_ne!(format!("{:?}", a.notes), format!("{:?}", b.notes));
}
