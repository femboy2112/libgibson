//! Falsifiable long-form tests: two causally-connected episodes in one continuous 32-beat Score,
//! and the memory law that couples the second episode's band decision to the first.
#[path = "../examples/c137_song/source.rs"]
mod source;

use gibson::audio::human_music::{
    kernel::{self, CommittedSong},
    plan::ArrangementRole,
    score::{DrumVoice, Role, Score},
    theory::Mode,
    MusicWorld,
};

/// The validated long-form world: A-Aeolian with sevenths and modal mixture, 144 BPM — the same
/// world the `c137_song` example renders under, so the contour/`closed` decisions match.
fn world() -> MusicWorld {
    let mut world = MusicWorld::black_ice();
    world.tempo_bpm = 144.0;
    world.tonic_pc = 9;
    world.mode = Mode::Aeolian;
    world.use_sevenths = true;
    world.allow_modal_mixture = true;
    world
}

fn take(seed: u64) -> CommittedSong {
    kernel::perform_song(&source::source(seed), &world()).unwrap()
}

/// The same source with the keyboard taken off-stage for the FIRST episode only (the form phrases
/// that begin before beat 16). Lead and bass carriage is untouched; only keys staging changes.
fn first_episode_keys_muted(seed: u64) -> gibson::audio::human_music::argument::CompiledArgument {
    let mut muted = source::source(seed);
    let boundary = muted
        .song
        .plan
        .form
        .phrases
        .iter()
        .position(|p| p.start_beat() >= 16.0)
        .expect("episode boundary at beat 16");
    for phrase in muted
        .song
        .plan
        .arrangement
        .phrases
        .iter_mut()
        .take(boundary)
    {
        phrase.keys = ArrangementRole::Silent;
    }
    muted
}

/// Note signature for exact lane comparison between two takes.
fn lane(score: &Score, role: Role) -> Vec<(f64, u32, i32, u32)> {
    score
        .role_notes(role)
        .map(|n| {
            (
                n.start_beat,
                n.dur_beats.to_bits(),
                n.pitch,
                n.velocity.to_bits(),
            )
        })
        .collect()
}

/// The ordered pitch content of one lane inside a beat window (used to show two episodes differ).
fn pitches(score: &Score, role: Role, lo: f64, hi: f64) -> Vec<i32> {
    let mut ns: Vec<_> = score
        .role_notes(role)
        .filter(|n| n.start_beat >= lo && n.start_beat < hi)
        .collect();
    ns.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    ns.iter().map(|n| n.pitch).collect()
}

/// The shared pocket: the fixed drum pulse, independent of any band response.
fn pulse(take: &CommittedSong) -> Vec<(f64, u32)> {
    take.score()
        .drums
        .iter()
        .filter(|d| d.prov.role_note == "pulse")
        .map(|d| (d.start_beat, d.velocity.to_bits()))
        .collect()
}

fn keys_in(take: &CommittedSong, lo: f64, hi: f64) -> Vec<(f64, i32, u32)> {
    let mut ns: Vec<_> = take
        .score()
        .role_notes(Role::Keys)
        .filter(|n| n.start_beat >= lo && n.start_beat < hi)
        .map(|n| (n.start_beat, n.pitch, n.velocity.to_bits()))
        .collect();
    ns.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    ns
}

fn heard_keys_fills(take: &CommittedSong, lo: f64, hi: f64) -> usize {
    take.score()
        .drums
        .iter()
        .filter(|d| d.prov.role_note == "heard-keys" && d.start_beat >= lo && d.start_beat < hi)
        .count()
}

#[test]
fn continuous_eight_bar_score() {
    let a = take(2112);
    let s = a.score();
    assert_eq!(s.total_beats, 32.0);
    assert!(s.mono_voice);
    assert_eq!(
        s.observed_lifetime,
        Some(gibson::audio::human_music::voice::ObservedLifetimePolicy::ExplicitContinuity)
    );

    // The lead sings in BOTH episodes, and the two episodes are genuinely different tunes (full
    // verse vs full hook) — not one cell repeated.
    let ep1 = pitches(s, Role::Lead, 0.0, 16.0);
    let ep2 = pitches(s, Role::Lead, 16.0, 32.0);
    assert!(!ep1.is_empty(), "episode 1 lead is silent");
    assert!(!ep2.is_empty(), "episode 2 lead is silent");
    assert_ne!(ep1, ep2, "the two episodes must be different tunes");

    // The bass holds one uninterrupted foundation lane: a root-bearing downbeat in all eight bars,
    // with no reset or gap at the beat-16 seam.
    for bar in 0..8 {
        let at = bar as f64 * 4.0;
        let root = a.composition().perf.context_at(at).unwrap().chord.root_pc;
        assert!(
            s.role_notes(Role::Bass)
                .any(|n| n.start_beat == at && n.pitch.rem_euclid(12) == root),
            "missing bass foundation at bar {bar} (beat {at})"
        );
    }

    // The harmony spans all eight bars (a chord covers every bar downbeat).
    for bar in 0..8 {
        let at = bar as f64 * 4.0;
        assert!(
            s.chords
                .iter()
                .any(|c| c.start_beat <= at + 1e-6
                    && c.start_beat + f64::from(c.dur_beats) > at + 1e-6),
            "no chord covers bar {bar}"
        );
    }

    assert!(a.validate_candidate(s).is_ok());
}

#[test]
fn second_episode_remembers_the_first() {
    let a = take(2112);

    // Mute keys ONLY in the first episode (phrases that start before beat 16). The lead and bass
    // are untouched; only the keyboard's staging changes.
    let b = kernel::perform_song(&first_episode_keys_muted(2112), &world()).unwrap();

    // Episode 1 keys: present in a, absent in b (the band could not answer off-stage).
    assert!(
        !keys_in(&a, 0.0, 16.0).is_empty(),
        "episode 1 keys should answer in the control"
    );
    assert!(
        keys_in(&b, 0.0, 16.0).is_empty(),
        "episode 1 keys should be withheld when the keyboard is off-stage"
    );

    // THE DEPENDENCY: episode 2 keys DIFFER between a and b. In a there is no outstanding debt, so
    // episode 2 answers its OWN hook cell; in b the first episode's call went unpaid, so episode 2
    // discharges the carried VERSE cell instead — different source material, different notes.
    let a_ep2 = keys_in(&a, 16.0, 32.0);
    let b_ep2 = keys_in(&b, 16.0, 32.0);
    assert!(!a_ep2.is_empty(), "episode 2 keys should sound in a");
    assert!(
        !b_ep2.is_empty(),
        "episode 2 keys should discharge the debt in b"
    );
    assert_ne!(
        a_ep2, b_ep2,
        "episode 2 keys must reflect whether episode 1 was answered"
    );

    // The pocket is NOT reset by the keys change: the drum pulse is bit-identical across a and b.
    assert_eq!(pulse(&a), pulse(&b), "the shared pulse must be invariant");

    // Lead and bass are entirely independent of the keyboard's staging.
    assert_eq!(lane(a.score(), Role::Lead), lane(b.score(), Role::Lead));
    assert_eq!(lane(a.score(), Role::Bass), lane(b.score(), Role::Bass));
}

#[test]
fn muted_cue_changes_only_its_later_drum_response_per_episode() {
    let a = take(2112);
    let b = kernel::perform_song(&first_episode_keys_muted(2112), &world()).unwrap();

    // Episode 1's heard-keys fill tracks episode 1's keys: present in a, gone in b.
    assert!(
        heard_keys_fills(&a, 0.0, 16.0) > 0,
        "episode 1 answered in a, so its drum fill should sound"
    );
    assert_eq!(
        heard_keys_fills(&b, 0.0, 16.0),
        0,
        "episode 1 keys are withheld in b, so its drum fill must vanish"
    );

    // Episode 2's fill is unaffected by episode 1's muting (episode 2 answers in both takes).
    assert!(heard_keys_fills(&a, 16.0, 32.0) > 0);
    assert!(heard_keys_fills(&b, 16.0, 32.0) > 0);

    // The pulse is identical — only the causal keys-dependent fill of the muted episode changed.
    assert_eq!(pulse(&a), pulse(&b));
}

#[test]
fn the_developed_lead_is_a_played_line_not_a_quantized_skeleton() {
    for seed in [2112u64, 7, 99] {
        // perform_song succeeds only if the DEVELOPED verse/hook still satisfy every source guard,
        // including the kinship law (the hook a lawful altered consequent of the verse) on the
        // developed pair and the relaxed-but-bounded grid/pocket guard.
        let take = kernel::perform_song(&source::source(seed), &world())
            .unwrap_or_else(|e| panic!("seed {seed}: developed source refused: {e:?}"));
        let lead: Vec<_> = take.score().role_notes(Role::Lead).collect();
        // Denser than the old stated skeleton (38 notes across the whole song) — a played line.
        assert!(
            lead.len() >= 60,
            "seed {seed}: developed lead is only {} notes, not a played line",
            lead.len()
        );
        // The structured pocket laid part of the line off the exact sixteenth grid (de-quantized),
        // yet every onset stays within the bounded pocket tolerance of the lattice (never random).
        let off_grid = lead
            .iter()
            .filter(|n| ((n.start_beat * 4.0).round() - n.start_beat * 4.0).abs() > 1e-6)
            .count();
        assert!(
            off_grid > 0,
            "seed {seed}: lead is still perfectly quantized"
        );
        for n in &lead {
            let lattice = (n.start_beat * 4.0).round() / 4.0;
            assert!(
                (n.start_beat - lattice).abs() <= 0.06 + 1e-9,
                "seed {seed}: a lead onset escaped the bounded pocket tolerance"
            );
        }
    }
}

#[test]
fn the_band_density_follows_the_harmony() {
    // Ensemble density is ALLOCATED by the song's own state — not held flat with every voice maxed
    // at once (the "virtuosos competing" texture). Three falsifiable consequences, on all seeds.
    // The route is seeded-independent, so the harmonic classification is identical across seeds; the
    // melody that the band must make room for is not.
    for seed in [2112u64, 7, 99] {
        let a = take(seed);
        let s = a.score();
        let comp = a.composition();

        // The harmony the model itself computes, bar by bar (tonic_pc = 9 in this world): a bar sits
        // on "home" exactly when its chord root IS the tonic.
        let is_home = |bar: usize| {
            (comp
                .perf
                .context_at(bar as f64 * 4.0)
                .unwrap()
                .chord
                .root_pc
                - 9)
            .rem_euclid(12)
                == 0
        };
        let home_bar = (0..8).find(|&b| is_home(b)).expect("a tonic (home) bar");
        let walk_bar = (0..8)
            .find(|&b| !is_home(b))
            .expect("a non-tonic (walking) bar");

        // (1) The bass lays back on the tonic (root + fifth, a breath) and walks everywhere else.
        let bass_in = |bar: usize| {
            s.role_notes(Role::Bass)
                .filter(|n| {
                    n.start_beat >= bar as f64 * 4.0 && n.start_beat < bar as f64 * 4.0 + 4.0
                })
                .count()
        };
        assert!(
            bass_in(home_bar) < bass_in(walk_bar),
            "seed {seed}: bass should lay back at home (bar {home_bar}={}) vs walk (bar {walk_bar}={})",
            bass_in(home_bar),
            bass_in(walk_bar)
        );

        // (2) The ride breathes: eighths at home, sixteenths in motion — fewer hats in a home bar.
        let hats_in = |bar: usize| {
            s.drums
                .iter()
                .filter(|d| {
                    matches!(d.voice, DrumVoice::ClosedHat | DrumVoice::OpenHat)
                        && d.start_beat >= bar as f64 * 4.0
                        && d.start_beat < bar as f64 * 4.0 + 4.0
                })
                .count()
        };
        assert!(
            hats_in(home_bar) < hats_in(walk_bar),
            "seed {seed}: ride should open up at home (bar {home_bar}={}) vs fill in (bar {walk_bar}={})",
            hats_in(home_bar),
            hats_in(walk_bar)
        );

        // (3) The drummer's comping MAKES ROOM: every conversational "comp" stroke sits in a crack the
        // lead left open — it never doubles a lead attack. The band interlocks instead of competing.
        let lead: Vec<f64> = s.role_notes(Role::Lead).map(|n| n.start_beat).collect();
        let comp_hits: Vec<f64> = s
            .drums
            .iter()
            .filter(|d| d.prov.role_note == "comp")
            .map(|d| d.start_beat)
            .collect();
        assert!(
            !comp_hits.is_empty(),
            "seed {seed}: the drummer never comps — no interplay with the lead at all"
        );
        for c in &comp_hits {
            assert!(
                !lead.iter().any(|l| (l - c).abs() < 0.2475),
                "seed {seed}: a comp stroke at {c} doubles a lead attack instead of making room"
            );
        }
    }
}
