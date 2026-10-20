//! The typed phase law with TEETH on the real Homosapians Score.
//!
//! `pocket_time` proves its group/cycle laws in isolation (unit tests). Here it bites the actual
//! frozen rehearsal Score: the band DECLARES relations (the riff-comp locks the kit; the comp and the
//! required Keys carrier are one keyboard and must agree), and we check whether the scheduled Score
//! HONOURS them. The band now SHIPS the lock (`comp_attack = ZERO`, the 2026-10-19 ear verdict), so
//! the shipped Score keeps both declared locks; the LEGACY early-pull (`comp_attack =
//! LEGACY_KEYS_PULL`) breaks each by exactly 0.03 beat. This is the executable form of the audit's
//! Law C1: a square labelled "Lock" that drifts 17 ms does not commute — and the fix makes it commute.
//!
//! These are SCORE-level (scheduled-time) witnesses, not PCM or aesthetic claims. The two treatments
//! are produced by setting the opt-in typed `comp_attack` on the frozen source, nothing else.

#[path = "../examples/c137_song/rehearsal.rs"]
#[allow(dead_code)]
mod rehearsal;

use gibson::audio::human_music::{
    kernel,
    pocket_time::{Displacement, Gesture, PhaseEdge, PhaseGraph},
    score::{DrumVoice, Role, Score},
    theory::Mode,
    MusicWorld,
};

/// The legacy advance in beats (0.03), taken from its single typed home so no magic number repeats.
const KEYS_PULL_BEATS: f64 = -Displacement::LEGACY_KEYS_PULL.beats;
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

/// The real rehearsal Score with a chosen comp-timing treatment (the only thing that moves).
fn performed_with(seed: u64, attack: Displacement) -> Score {
    let draft = rehearsal::rehearse(seed);
    let mut source = rehearsal::freeze(&draft);
    source.comp_attack = attack;
    kernel::perform_song(&source, &world())
        .expect("opt-in rehearsal Score is lawful")
        .score()
        .clone()
}

fn comp_at(score: &Score, beat: f64) -> f64 {
    score
        .role_notes(Role::Keys)
        .find(|n| n.prov.role_note == "comp" && (n.start_beat - beat).abs() < 1e-6)
        .unwrap_or_else(|| panic!("a riff-comp chord at beat {beat}"))
        .start_beat
}

fn carrier_at(score: &Score, beat: f64) -> f64 {
    score
        .role_notes(Role::Keys)
        .find(|n| n.prov.role_note == "argument" && (n.start_beat - beat).abs() < 1e-6)
        .unwrap_or_else(|| panic!("the required Keys carrier at beat {beat}"))
        .start_beat
}

fn kick_at(score: &Score, beat: f64) -> f64 {
    score
        .drums
        .iter()
        .find(|d| d.voice == DrumVoice::Kick && (d.start_beat - beat).abs() < 1e-6)
        .unwrap_or_else(|| panic!("the kit kick at beat {beat}"))
        .start_beat
}

#[test]
fn comp_kick_lock_is_broken_by_the_legacy_pull_and_kept_by_the_shipped_lock() {
    // The bar-2 downbeat: the comp and the kick own the SAME pocketed slot (grid 4.0), so the band's
    // intent is that they LOCK.
    let g = PhaseGraph::with_edges(vec![PhaseEdge::lock(
        Gesture::KeysComp,
        Gesture::Kick,
        "the riff-comp is meant to lock the kit",
    )]);

    // LEGACY: the comp is scheduled at 3.97 (grid 4.0 − 0.03) while the kick holds 4.0 — a 17.3 ms
    // violation of the declared lock.
    let legacy = performed_with(2112, Displacement::LEGACY_KEYS_PULL);
    let bpm = f64::from(legacy.tempo_bpm);
    let spb = 60.0 / bpm;
    let now = g.observed_residuals(
        &[
            (Gesture::KeysComp, comp_at(&legacy, 3.97) * spb),
            (Gesture::Kick, kick_at(&legacy, 4.0) * spb),
        ],
        bpm,
        BOUND_SECONDS,
    );
    assert_eq!(now.len(), 1);
    assert!(
        !now[0].within_bound,
        "the legacy pull breaks the declared comp->kick lock"
    );
    assert!((now[0].residual_seconds - KEYS_PULL_BEATS * spb).abs() < 1e-6);

    // SHIPPED: the comp is locked onto 4.0 with the kick — the declared lock is kept.
    let shipped = performed_with(2112, Displacement::ZERO);
    let kept = g.observed_residuals(
        &[
            (Gesture::KeysComp, comp_at(&shipped, 4.0) * spb),
            (Gesture::Kick, kick_at(&shipped, 4.0) * spb),
        ],
        bpm,
        BOUND_SECONDS,
    );
    assert!(
        kept[0].within_bound,
        "the shipped lock keeps the comp->kick lock"
    );
    assert!(kept[0].residual_seconds.abs() < 1e-6);
}

#[test]
fn keyboard_self_flam_is_caused_by_the_legacy_pull_and_removed_by_the_shipped_lock() {
    // One keyboard cannot flam against itself: the required Keys carrier and the riff-comp, when they
    // share a metric slot, must agree. On a seed whose riff has a 5.5-beat onset, the carrier rides
    // 5.534 (+pocket) in both treatments; the comp rides 5.504 under the legacy pull and locks onto
    // 5.534 when shipped.
    let seed = (0u64..256)
        .find(|&s| {
            rehearsal::rehearse(s)
                .riff()
                .events
                .iter()
                .any(|e| (e.onset - 5.5).abs() < 1e-6)
        })
        .expect("the finite riff vocabulary includes a 5.5-beat onset");

    let g = PhaseGraph::with_edges(vec![PhaseEdge::lock(
        Gesture::KeysComp,
        Gesture::KeysCarrier,
        "one keyboard: comp and carrier must agree on the slot",
    )]);

    // LEGACY: comp 5.504 vs carrier 5.534 — a 0.03-beat self-flam.
    let legacy = performed_with(seed, Displacement::LEGACY_KEYS_PULL);
    let bpm = f64::from(legacy.tempo_bpm);
    let spb = 60.0 / bpm;
    let now = g.observed_residuals(
        &[
            (Gesture::KeysComp, comp_at(&legacy, 5.504) * spb),
            (Gesture::KeysCarrier, carrier_at(&legacy, 5.534) * spb),
        ],
        bpm,
        BOUND_SECONDS,
    );
    assert!(!now[0].within_bound, "the legacy comp flams the carrier");
    assert!((now[0].residual_seconds - KEYS_PULL_BEATS * spb).abs() < 1e-6);

    // SHIPPED: comp locks to the carrier's slot (both 5.534) — the keyboard agrees with itself.
    let shipped = performed_with(seed, Displacement::ZERO);
    let kept = g.observed_residuals(
        &[
            (Gesture::KeysComp, comp_at(&shipped, 5.534) * spb),
            (Gesture::KeysCarrier, carrier_at(&shipped, 5.534) * spb),
        ],
        bpm,
        BOUND_SECONDS,
    );
    assert!(
        kept[0].within_bound,
        "the shipped lock makes the keyboard agree with itself"
    );
    assert!(kept[0].residual_seconds.abs() < 1e-6);
}
