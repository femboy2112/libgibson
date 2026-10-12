//! Hostile admission probes against f042bb055c966fd79fab0823e72a53409bdaffad.
//! These assert the desired contract. Their first execution is intentionally red evidence.
#[path = "../examples/c137_conversation/source.rs"]
mod source;

use gibson::audio::human_music::{
    argument::CompiledArgument,
    kernel::{self, CommittedPerformance},
    score::{Role, Score},
    theory::PitchBasis,
    voice::VoiceEventId,
    MusicWorld,
};

fn take() -> CommittedPerformance {
    kernel::perform(&source::source(2112), &MusicWorld::black_ice()).unwrap()
}

fn change_note_identity(
    score: &mut Score,
    index: usize,
    mutate: impl FnOnce(&mut gibson::audio::human_music::score::Note),
) {
    let old = VoiceEventId::of(&score.notes[index]);
    mutate(&mut score.notes[index]);
    let new = VoiceEventId::of(&score.notes[index]);
    for link in &mut score.voice_continuity {
        if link.from == old {
            link.from = new;
        }
        if link.to == old {
            link.to = new;
        }
    }
}

#[test]
fn audible_foundation_cannot_be_replaced_with_zero_velocity_markers() {
    let t = take();
    let mut s = t.score().clone();
    for n in &mut s.notes {
        if n.role == Role::Bass {
            n.velocity = 0.0;
        }
    }
    assert!(t.hearings().iter().all(|h| h.matches(&s)));
    assert!(
        t.validate_candidate(&s).is_err(),
        "silent bass markers passed foundation duty"
    );
}

#[test]
fn bass_foundation_cannot_be_relabelled_as_an_unowned_counterline() {
    let t = take();
    let mut s = t.score().clone();
    let i = s.notes.iter().position(|n| n.role == Role::Bass).unwrap();
    change_note_identity(&mut s, i, |n| n.prov.role_note = "unowned-counterline");
    assert!(
        t.validate_candidate(&s).is_err(),
        "unowned foundation source label admitted"
    );
}

#[test]
fn bass_arpeggio_cannot_become_an_unjustified_chromatic_pitch() {
    let t = take();
    let mut s = t.score().clone();
    let i = s
        .notes
        .iter()
        .position(|n| n.role == Role::Bass && n.start_beat == 2.0)
        .unwrap();
    change_note_identity(&mut s, i, |n| n.pitch = 46); // Bb against the declared Am.
    assert!(
        t.validate_candidate(&s).is_err(),
        "off-chord bass falsely retained ChordTone duty"
    );
}

#[test]
fn bass_duty_cannot_be_shifted_off_its_declared_pulse() {
    let t = take();
    let mut s = t.score().clone();
    let i = s
        .notes
        .iter()
        .position(|n| n.role == Role::Bass && n.start_beat == 2.0)
        .unwrap();
    change_note_identity(&mut s, i, |n| n.start_beat += 0.137);
    assert!(
        t.validate_candidate(&s).is_err(),
        "off-phase bass duty admitted"
    );
}

#[test]
fn consumed_keys_response_cannot_leave_a_missing_drum_consequence() {
    let t = take();
    let mut s = t.score().clone();
    let before = s.drums.len();
    s.drums.retain(|h| h.prov.role_note != "heard-keys");
    assert!(s.drums.len() < before);
    assert!(t.hearings().iter().all(|h| h.matches(&s)));
    assert!(
        t.validate_candidate(&s).is_err(),
        "heard-keys dependency survived deletion of its consequence"
    );
}

#[test]
fn committed_kit_cannot_be_deleted_wholesale() {
    let t = take();
    let mut s = t.score().clone();
    s.drums.clear();
    assert!(
        t.validate_candidate(&s).is_err(),
        "missing shared beat still admitted"
    );
}

#[test]
fn arbitrary_off_phase_drum_cannot_claim_a_committed_cue() {
    let t = take();
    let mut s = t.score().clone();
    let mut h = *s
        .drums
        .iter()
        .find(|h| h.prov.role_note == "heard-keys")
        .unwrap();
    h.start_beat = 8.137;
    s.drums.push(h);
    assert!(
        t.validate_candidate(&s).is_err(),
        "undeclared extra drum reply admitted"
    );
}

#[test]
fn final_drum_dynamics_are_part_of_the_committed_response() {
    let t = take();
    let mut s = t.score().clone();
    for h in &mut s.drums {
        h.velocity = 0.0;
    }
    assert!(
        t.validate_candidate(&s).is_err(),
        "silent kit admitted as completed performance"
    );
}

#[test]
fn declared_metric_base_cannot_change_after_commit() {
    let t = take();
    let mut s = t.score().clone();
    s.beats_per_bar = 3.0;
    assert!(
        t.validate_candidate(&s).is_err(),
        "3/4 score admitted against frozen four-bar common base"
    );
}

#[test]
fn final_harmonic_base_cannot_disappear_after_commit() {
    let t = take();
    let mut s = t.score().clone();
    s.chords.clear();
    assert!(
        t.validate_candidate(&s).is_err(),
        "empty score harmony admitted against final source route"
    );
}

fn refused(mutate: impl FnOnce(&mut CompiledArgument)) {
    let mut s = source::source(2112);
    mutate(&mut s);
    assert!(
        kernel::perform(&s, &MusicWorld::black_ice()).is_err(),
        "incoherent compiled source metadata admitted"
    );
}

#[test]
fn compiled_return_cannot_claim_the_opening_phrase_identity() {
    refused(|s| s.placements[1].phrase = 0);
}

#[test]
fn source_arc_must_agree_with_compiled_placement_time() {
    refused(|s| s.argument.arc.as_mut().unwrap()[1].start_beat = 8.0);
}

#[test]
fn source_arc_must_not_reference_a_nonexistent_phrase() {
    refused(|s| s.argument.arc.as_mut().unwrap()[1].start_phrase = u32::MAX);
}

#[test]
fn source_arc_must_have_a_span_for_each_step() {
    refused(|s| s.argument.arc.as_mut().unwrap().clear());
}

#[test]
fn effective_hearing_rejects_dynamic_and_false_withholding_mutations() {
    // Positive controls for the hostile instrument: these are already refused at f042bb0.
    let t = take();
    let mut s = t.score().clone();
    s.notes
        .iter_mut()
        .find(|n| n.role == Role::Keys)
        .unwrap()
        .velocity *= 0.1;
    assert!(t.validate_candidate(&s).is_err());
    let mut s = t.score().clone();
    let mut n = *s.role_notes(Role::Keys).next().unwrap();
    n.start_beat = 9.0;
    s.notes.push(n);
    assert!(t.validate_candidate(&s).is_err());
}

#[test]
fn source_pitch_basis_must_match_the_compiled_interpretation() {
    refused(|s| s.argument.referents[0].pitch_basis = PitchBasis::ScaleSteps);
}
