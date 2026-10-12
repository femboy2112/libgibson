//! C137 legacy counterexamples. These demonstrate what continuity actually guarantees;
//! they intentionally do not upgrade it into an instrument-capacity or listening witness.
//!
//! Predeclared outcomes: a disconnected bass intrusion passes edge validation and changes
//! rendered PCM; simultaneous pitches cannot share a continuation edge but are accepted;
//! a rapid same-pitch retrigger passes continuity and remains acoustically distinct from a hold.

use gibson::audio::{
    human_music::{
        form::SectionKind,
        score::{Note, Provenance, Role, Score},
        synth::{HumanMusicSynth, ProductionControl},
        voice::{continuity_violations, effective_audible_end, VoiceContinuation},
        MusicWorld,
    },
    OfflineRenderer, SampleRate, StereoBlock,
};

fn bass(at: f64, dur: f32, pitch: i32) -> Note {
    Note::new(
        at,
        dur,
        pitch,
        0.65,
        Role::Bass,
        Provenance::new(SectionKind::A),
    )
}

fn score(notes: Vec<Note>) -> Score {
    let mut out = Score::new(120.0, 4.0, 4.0);
    out.mono_voice = true;
    out.notes = notes;
    out
}

fn render(score: &Score) -> StereoBlock {
    let sr = SampleRate::STUDIO;
    let world = MusicWorld::black_ice();
    let mut synth =
        HumanMusicSynth::with_production(score, &world, sr, ProductionControl::HARMONIC_REFERENCE);
    // One second is sufficient to hear both onsets and avoids downstream render tails.
    OfflineRenderer::new(sr, 257)
        .render(&mut synth, 48_000)
        .audio
}

#[test]
fn legacy_edge_validation_admits_a_disconnected_second_bass_line() {
    let from = bass(0.0, 2.0, 36);
    let to = bass(2.0, 1.0, 43);
    let link = VoiceContinuation::new(&from, &to).unwrap();
    let mut foundation = score(vec![from, to]);
    foundation.voice_continuity.push(link);
    let mut intruded = foundation.clone();
    let extra = bass(0.5, 1.0, 40);
    intruded.notes.push(extra);

    // The foundation's valid link does not cover or choke the additional bass stream.
    assert!(continuity_violations(&intruded.notes, &intruded.voice_continuity).is_empty());
    let world = MusicWorld::black_ice();
    assert!(effective_audible_end(&from, &world, &[link]) > extra.start_beat);
    assert_eq!(
        effective_audible_end(&extra, &world, &[link]),
        effective_audible_end(&extra, &world, &[])
    );
    assert_ne!(render(&foundation).left, render(&intruded).left);
}

#[test]
fn legacy_edge_validation_admits_simultaneous_bass_pitches() {
    let root = bass(0.0, 1.0, 36);
    let third = bass(0.0, 1.0, 40);
    assert!(VoiceContinuation::new(&root, &third).is_none());
    let chord = score(vec![root, third]);
    assert!(continuity_violations(&chord.notes, &chord.voice_continuity).is_empty());
    // The synth's bass pool really sounds the second event; this is not metadata only.
    assert_ne!(render(&score(vec![root])).left, render(&chord).left);
}

#[test]
fn legacy_continuity_does_not_turn_a_retrigger_into_a_hold() {
    let first = bass(0.0, 0.25, 36);
    let second = bass(0.25, 0.75, 36);
    let mut retrigger = score(vec![first, second]);
    retrigger
        .voice_continuity
        .push(VoiceContinuation::new(&first, &second).unwrap());
    assert!(continuity_violations(&retrigger.notes, &retrigger.voice_continuity).is_empty());
    let held = score(vec![bass(0.0, 1.0, 36)]);
    assert_ne!(render(&held).left, render(&retrigger).left);
    // This establishes a real acoustic difference, not a human verdict that it stutters.
}
