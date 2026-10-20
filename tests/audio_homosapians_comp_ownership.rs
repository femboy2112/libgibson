//! The comp content-ownership witness with TEETH: a genuine rehearsal render reads zero, and every
//! forgery the audit named — a ghost chord, an unauthorised displacement, a wrong chord, a forged
//! `comp` label, a crossed barline, a wholly omitted onset — is independently detected.

#[path = "../examples/c137_song/rehearsal.rs"]
#[allow(dead_code)]
mod rehearsal;

use gibson::audio::human_music::{
    argument::CompiledArgument,
    comp_witness::observe_comp,
    kernel,
    score::{Note, Role, Score},
    theme_family::pocket_offset,
    theory::Mode,
    MusicWorld,
};

const SEEDS: [u64; 5] = [7, 42, 2112, 9, 13];
const EPS: f64 = 1e-4;

fn world() -> MusicWorld {
    let mut w = MusicWorld::black_ice();
    w.tonic_pc = 9;
    w.mode = Mode::Aeolian;
    w.use_sevenths = true;
    w.allow_modal_mixture = true;
    w.tempo_bpm = 104.0;
    w
}

/// The shipping rehearsal source + its frozen Score (comp locked to the pocket).
fn performed(seed: u64) -> (CompiledArgument, Score) {
    let draft = rehearsal::rehearse(seed);
    let source = rehearsal::freeze(&draft);
    let score = kernel::perform_song(&source, &world())
        .expect("opt-in rehearsal Score is lawful")
        .score()
        .clone();
    (source, score)
}

/// Clone the first comp note (to inherit a valid Keys/"comp" provenance) for building forgeries.
fn first_comp(score: &Score) -> Note {
    *score
        .notes
        .iter()
        .find(|n| n.role == Role::Keys && n.prov.role_note == "comp")
        .expect("a genuine render has comp notes")
}

#[test]
fn genuine_renders_are_owned_end_to_end() {
    for seed in SEEDS {
        let (source, score) = performed(seed);
        let w = observe_comp(&source, &score);
        assert!(
            w.valid(),
            "seed {seed}: genuine comp is not owned: {:?}",
            w.violations()
        );
        assert!(
            w.gesture_count() > 0,
            "seed {seed}: no comp gestures observed"
        );
        assert!(w.note_count() > 0, "seed {seed}: no comp notes observed");
    }
}

#[test]
fn a_ghost_comp_chord_is_detected() {
    let (source, mut score) = performed(2112);
    let mut ghost = first_comp(&score);
    ghost.start_beat = 2.37; // a beat no riff onset owns
    score.notes.push(ghost);
    let w = observe_comp(&source, &score);
    assert!(!w.valid(), "the ghost chord slipped past the witness");
    assert!(w.violations().iter().any(|v| (v.at - 2.37).abs() < EPS));
}

#[test]
fn an_unauthorised_displacement_is_detected() {
    let (source, mut score) = performed(2112);
    // Shove one comp note 0.1 beat off its locked slot.
    let moved = score
        .notes
        .iter_mut()
        .find(|n| n.role == Role::Keys && n.prov.role_note == "comp")
        .expect("a comp note");
    moved.start_beat += 0.1;
    let w = observe_comp(&source, &score);
    assert!(
        !w.valid(),
        "the 0.1-beat displacement slipped past the witness"
    );
}

#[test]
fn a_wrong_chord_tone_is_detected() {
    let (source, mut score) = performed(2112);
    // Keep the slot, push the pitch a semitone off its guide tone — off the chord.
    let bent = score
        .notes
        .iter_mut()
        .find(|n| n.role == Role::Keys && n.prov.role_note == "comp")
        .expect("a comp note");
    bent.pitch += 1;
    let w = observe_comp(&source, &score);
    assert!(!w.valid(), "the wrong chord tone slipped past the witness");
    assert!(w
        .violations()
        .iter()
        .any(|v| v.reason.contains("guide tone")));
}

#[test]
fn a_forged_comp_label_is_detected() {
    let (source, mut score) = performed(2112);
    // Relabel the required Keys CARRIER line as "comp": you cannot launder the melody into comp
    // provenance — at least one carrier note lands off a comp slot or off the guide tones.
    let mut forged = 0;
    for n in score.notes.iter_mut() {
        if n.role == Role::Keys && n.prov.role_note == "argument" {
            n.prov.role_note = "comp";
            forged += 1;
        }
    }
    assert!(forged > 0, "fixture has a Keys carrier to forge");
    let w = observe_comp(&source, &score);
    assert!(!w.valid(), "the forged comp label slipped past the witness");
}

#[test]
fn a_crossed_barline_is_detected() {
    let (source, mut score) = performed(2112);
    // Shove a comp note a whole beat later — into a different metric slot / bar harmony.
    let moved = score
        .notes
        .iter_mut()
        .find(|n| n.role == Role::Keys && n.prov.role_note == "comp")
        .expect("a comp note");
    moved.start_beat += 1.0;
    let w = observe_comp(&source, &score);
    assert!(!w.valid(), "the crossed barline slipped past the witness");
}

#[test]
fn an_omitted_source_gesture_is_detected() {
    let (source, mut score) = performed(2112);
    let onsets = source
        .comp_onsets
        .clone()
        .expect("the rehearsal source sets comp_onsets");
    let o0 = onsets[0];
    let episodes = (source.song.plan.form.total_beats / 16.0).round() as usize;
    let displacement = 0.0; // the locked band
                            // Every performed slot the first onset descends from.
    let mut ats = Vec::new();
    for e in 0..episodes {
        for rep in 0..2u32 {
            let at_grid = e as f64 * 16.0 + f64::from(rep) * 8.0 + o0;
            ats.push((at_grid + pocket_offset(o0.rem_euclid(4.0)) + displacement).max(0.0));
        }
    }
    let before = score.notes.len();
    score.notes.retain(|n| {
        !(n.role == Role::Keys
            && n.prov.role_note == "comp"
            && ats.iter().any(|a| (a - n.start_beat).abs() < EPS))
    });
    assert!(
        score.notes.len() < before,
        "the omission removed comp notes"
    );
    let w = observe_comp(&source, &score);
    assert!(
        !w.valid(),
        "the omitted onset gesture slipped past the witness"
    );
    assert!(w
        .violations()
        .iter()
        .any(|v| v.reason.contains("omitted source gesture")));
}
