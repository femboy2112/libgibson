//! The typed phase law with TEETH on the real Homosapians Score.
//!
//! `pocket_time` proves its group/cycle laws in isolation (unit tests). Here it bites the actual
//! frozen rehearsal Score: the band DECLARES relations (the riff-comp locks the kit; the comp and the
//! required Keys carrier are one keyboard and must agree), and we check whether the scheduled Score
//! HONOURS them. Today it does not — the `KEYS_ATTACK_COMP` pull violates both declared locks by
//! exactly 0.03 beat — and the no-advance treatment keeps them. This is the executable form of the
//! audit's Law C1: a square labelled "Lock" that drifts 17 ms does not commute.
//!
//! These are SCORE-level (scheduled-time) witnesses, not PCM or aesthetic claims. No default changes;
//! the no-advance variant is produced by shifting the frozen Score's comp notes, exactly as the R1
//! metrology lab does.

#[path = "../examples/c137_song/rehearsal.rs"]
#[allow(dead_code)]
mod rehearsal;

use gibson::audio::human_music::{
    kernel,
    pocket_time::{Gesture, PhaseEdge, PhaseGraph},
    score::{DrumVoice, Role, Score},
    theory::Mode,
    MusicWorld,
};

const KEYS_ATTACK_COMP: f64 = 0.03;
const BOUND_SECONDS: f64 = 0.005; // a 5 ms musical lock tolerance

fn world() -> MusicWorld {
    let mut w = MusicWorld::black_ice();
    w.tonic_pc = 9;
    w.mode = Mode::Aeolian;
    w.use_sevenths = true;
    w.allow_modal_mixture = true;
    w.tempo_bpm = 104.0;
    w
}

fn performed(seed: u64) -> Score {
    let draft = rehearsal::rehearse(seed);
    let source = rehearsal::freeze(&draft);
    kernel::perform_song(&source, &world())
        .expect("opt-in rehearsal Score is lawful")
        .score()
        .clone()
}

#[test]
fn declared_comp_locks_kick_is_violated_today_and_kept_by_no_advance() {
    let score = performed(2112);
    let bpm = f64::from(score.tempo_bpm);
    let spb = 60.0 / bpm;

    // The bar-2 downbeat: the comp is scheduled at 3.97 (grid 4.0 − 0.03), the kick holds 4.0. They
    // own the SAME pocketed slot (3.97 + 0.03 = 4.0), so the band's intent is that they LOCK.
    let comp = score
        .role_notes(Role::Keys)
        .find(|n| n.prov.role_note == "comp" && (n.start_beat - 3.97).abs() < 1e-6)
        .expect("the bar-boundary comp chord");
    let kick = score
        .drums
        .iter()
        .find(|d| d.voice == DrumVoice::Kick && (d.start_beat - 4.0).abs() < 1e-6)
        .expect("the kit kick on the bar-2 downbeat");

    let g = PhaseGraph::with_edges(vec![PhaseEdge::lock(
        Gesture::KeysComp,
        Gesture::Kick,
        "the riff-comp is meant to lock the kit",
    )]);

    // CURRENT Score: the declared lock is NOT kept — the comp is 0.03 beat (17.3 ms) early.
    let now = g.observed_residuals(
        &[
            (Gesture::KeysComp, comp.start_beat * spb),
            (Gesture::Kick, kick.start_beat * spb),
        ],
        bpm,
        BOUND_SECONDS,
    );
    assert_eq!(now.len(), 1);
    assert!(
        !now[0].within_bound,
        "the current Score breaks the declared comp->kick lock"
    );
    assert!((now[0].residual_seconds - KEYS_ATTACK_COMP * spb).abs() < 1e-6);

    // NO-ADVANCE: shift the comp onto the shared pocket (+0.03 beat). The lock is now kept.
    let kept = g.observed_residuals(
        &[
            (
                Gesture::KeysComp,
                (comp.start_beat + KEYS_ATTACK_COMP) * spb,
            ),
            (Gesture::Kick, kick.start_beat * spb),
        ],
        bpm,
        BOUND_SECONDS,
    );
    assert!(kept[0].within_bound, "no-advance keeps the comp->kick lock");
    assert!(kept[0].residual_seconds.abs() < 1e-6);
}

#[test]
fn declared_carrier_comp_agreement_is_violated_today_and_kept_by_no_advance() {
    // One keyboard cannot flam against itself: the required Keys carrier and the riff-comp, when they
    // share a metric slot, must agree. On a seed whose riff has a 5.5-beat onset, the carrier rides
    // 5.534 (+pocket) and the comp 5.504 (+pocket − 0.03): a 0.03-beat self-flam.
    let seed = (0u64..256)
        .find(|&s| {
            rehearsal::rehearse(s)
                .riff()
                .events
                .iter()
                .any(|e| (e.onset - 5.5).abs() < 1e-6)
        })
        .expect("the finite riff vocabulary includes a 5.5-beat onset");
    let score = performed(seed);
    let bpm = f64::from(score.tempo_bpm);
    let spb = 60.0 / bpm;

    let carrier = score
        .role_notes(Role::Keys)
        .find(|n| n.prov.role_note == "argument" && (n.start_beat - 5.534).abs() < 1e-6)
        .expect("the required Keys carrier at 5.534");
    let comp = score
        .role_notes(Role::Keys)
        .find(|n| n.prov.role_note == "comp" && (n.start_beat - 5.504).abs() < 1e-6)
        .expect("the riff-comp at 5.504");

    // Intent: the comp agrees with (locks) the carrier — same instrument, same slot.
    let g = PhaseGraph::with_edges(vec![PhaseEdge::lock(
        Gesture::KeysComp,
        Gesture::KeysCarrier,
        "one keyboard: comp and carrier must agree on the slot",
    )]);

    let now = g.observed_residuals(
        &[
            (Gesture::KeysComp, comp.start_beat * spb),
            (Gesture::KeysCarrier, carrier.start_beat * spb),
        ],
        bpm,
        BOUND_SECONDS,
    );
    assert!(!now[0].within_bound, "the comp flams the carrier today");
    assert!((now[0].residual_seconds - KEYS_ATTACK_COMP * spb).abs() < 1e-6);

    let kept = g.observed_residuals(
        &[
            (
                Gesture::KeysComp,
                (comp.start_beat + KEYS_ATTACK_COMP) * spb,
            ),
            (Gesture::KeysCarrier, carrier.start_beat * spb),
        ],
        bpm,
        BOUND_SECONDS,
    );
    assert!(
        kept[0].within_bound,
        "no-advance makes the keyboard agree with itself"
    );
    assert!(kept[0].residual_seconds.abs() < 1e-6);
}
