//! Falsifiable long-form tests: two causally-connected episodes in one continuous 32-beat Score,
//! and the memory law that couples the second episode's band decision to the first.
#[path = "../examples/c137_song/source.rs"]
mod source;
#[path = "../examples/c137_song/source_bridge.rs"]
mod source_bridge;
// `parse`/`label`/`removes` are exercised by the example binary (CLI + manifest), not by these
// tests, so they read as dead code HERE — the same shared-fixture treatment the source modules get.
#[allow(dead_code)]
#[path = "../examples/c137_song/preset.rs"]
mod preset;

use gibson::audio::human_music::{
    kernel::{self, CommittedSong},
    plan::ArrangementRole,
    score::{DrumVoice, Role, Score},
    theme_family::Phrasing,
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
            // The comp slot is gated a full sixteenth clear of any lead attack and then laid into the
            // shared pocket, so it is never within a doubling distance of the melody.
            assert!(
                !lead.iter().any(|l| (l - c).abs() < 0.15),
                "seed {seed}: a comp stroke at {c} collides with a lead attack instead of making room"
            );
        }
    }
}

#[test]
fn the_backing_grooves_a_pocket() {
    // Groove = reasoned micro-timing, not a dead grid and not random jitter. The backing lays a
    // structured, bounded pocket around exact anchors, and its dynamics are shaped (not uniform).
    for seed in [2112u64, 7, 99] {
        let a = take(seed);
        let s = a.score();

        // (a) The anchors stay DEAD ON: the bass downbeat root is exactly on the grid in every bar.
        for bar in 0..8 {
            let at = bar as f64 * 4.0;
            assert!(
                s.role_notes(Role::Bass)
                    .any(|n| (n.start_beat - at).abs() < 1e-9),
                "seed {seed}: the bass downbeat anchor left the grid at bar {bar}"
            );
        }

        // (b) Every backing stroke stays within the bounded groove tolerance of the sixteenth lattice
        // (structured, never a wild offset).
        for d in s
            .drums
            .iter()
            .filter(|d| d.prov.role_note == "pulse" || d.prov.role_note == "comp")
        {
            let lattice = (d.start_beat * 4.0).round() / 4.0;
            assert!(
                (d.start_beat - lattice).abs() <= 0.05 + 1e-9,
                "seed {seed}: a drum stroke at {} escaped the groove bound",
                d.start_beat
            );
        }

        // (c) The backbeat snare LAYS BACK off the exact grid — a pocket exists, it is not a march.
        let laid = s
            .drums
            .iter()
            .filter(|d| matches!(d.voice, DrumVoice::Snare) && d.prov.role_note == "pulse")
            .filter(|d| {
                let lattice = (d.start_beat * 4.0).round() / 4.0;
                (d.start_beat - lattice).abs() > 1e-9
            })
            .count();
        assert!(
            laid > 0,
            "seed {seed}: the backbeat never lays back — the backing is a dead grid"
        );

        // (d) The pulse dynamics are SHAPED, not uniform: the backbeat snares carry more than one
        // velocity across the song (harmonic energy × phrase swell), i.e. de-roboticized.
        let bb_vels: std::collections::BTreeSet<u32> = s
            .drums
            .iter()
            .filter(|d| matches!(d.voice, DrumVoice::Snare) && d.prov.role_note == "pulse")
            .map(|d| d.velocity.to_bits())
            .collect();
        assert!(
            bb_vels.len() >= 2,
            "seed {seed}: backbeat velocities are uniform — still robotic"
        );
    }
}

#[test]
fn the_lead_is_dynamically_shaped_not_flat() {
    // The developed lead has FOREGROUND and BACKGROUND within one voice: structural notes sing, the
    // passing sixteenths are ghosts, and a phrase contour rides over both — so it is a played line,
    // not a flat wall of equally-loud notes (the "pretentious" show-off sixteenths).
    for seed in [2112u64, 7, 99] {
        let a = take(seed);
        let vels: Vec<f32> = a
            .score()
            .role_notes(Role::Lead)
            .map(|n| n.velocity)
            .collect();
        let lo = vels.iter().cloned().fold(f32::MAX, f32::min);
        let hi = vels.iter().cloned().fold(0.0_f32, f32::max);
        assert!(
            hi - lo > 0.25,
            "seed {seed}: lead dynamics are too flat (range {lo:.3}..{hi:.3}) — no foreground/background"
        );
    }
}

// ===========================================================================================
// R0 — categorical-provenance falsifiers (PR #107 audit G2). These pin the causal LEDGER, not
// the audio: every fix they drive is sound-preserving (the rendered fingerprints are unchanged).
// ===========================================================================================

/// G2 / assertion 04 — a carried answer must credit the call it actually discharges.
///
/// In the muted take, episode 1's call goes unpaid, so episode 2 discharges the carried VERSE
/// debt (it quotes `materials[0]`). The `Answer` action must therefore name the verse's call
/// (statement 0), not the hook's call (statement 1) that merely happens to be the current episode.
#[test]
fn a_carried_answer_credits_the_call_it_discharges() {
    use gibson::audio::human_music::action::{ActionCause, ActionKind};
    let b = kernel::perform_song(&first_episode_keys_muted(2112), &world()).unwrap();
    let perf = &b.composition().perf;
    let verse_call = perf.statements[0].call.expect("verse statement has a call");
    let hook_call = perf.statements[1].call.expect("hook statement has a call");
    assert_ne!(
        verse_call, hook_call,
        "the two episodes' calls are distinct ids"
    );

    let answer = perf
        .actions
        .actions
        .iter()
        .find(|a| a.kind == ActionKind::Answer && a.start_beat >= 16.0)
        .expect("episode 2 produces exactly one Answer, in the hook window");
    assert_eq!(
        answer.cause,
        ActionCause::Interaction { call: verse_call },
        "the discharged answer must credit the VERSE call it actually quotes, not the hook call"
    );
    assert_eq!(
        answer.pays,
        Some(verse_call),
        "the answer pays the verse debt, not the hook call"
    );
}

/// G2 / assertions 05-08 — every interaction a committed note references must resolve to a real
/// `Interaction` row in the plan (the causal ledger is closed under referential identity). Fails
/// on BOTH the default render and the muted control: `perform_song` stamps `InteractionId(e)` on
/// keys answers but never registers the matching rows.
#[test]
fn every_referenced_interaction_resolves_to_a_row() {
    for (label, arg) in [
        ("default", source::source(2112)),
        ("muted-ep1", first_episode_keys_muted(2112)),
    ] {
        let song = kernel::perform_song(&arg, &world()).unwrap();
        let perf = &song.composition().perf;
        for n in song.score().role_notes(Role::Keys) {
            if let Some(iid) = n.prov.interaction {
                assert!(
                    perf.interactions.iter().any(|row| row.id == iid),
                    "[{label}] keys note @{:.3} references {iid:?} with no matching Interaction row",
                    n.start_beat
                );
            }
        }
    }
}

/// G7 / assertions 09-10 — section provenance must be truthful, with the LEAD as referee. The lead
/// carrier is already sectioned from the plan (verse and hook are different sections); every
/// accompaniment event must AGREE with the lead about which section the band is in, and the Score
/// must carry the form it realized. Currently `provenance()` hardcodes `SectionKind::A` and
/// `Score.sections` is never assigned, so the band contradicts the lead and the form is invisible.
///
/// (Lead-as-referee, not `!= A`: a phrase-true derivation legitimately yields `A` for some hook
/// phrases, so pinning against a literal kind would reject a correct fix — the invariant is that
/// the band and the lead name the SAME section, whatever it is.)
#[test]
fn section_provenance_agrees_with_the_lead() {
    let a = take(2112);
    let s = a.score();
    assert!(
        !s.sections.is_empty(),
        "Score.sections must be populated from the realized form"
    );
    let lead_section = |lo: f64, hi: f64| {
        s.role_notes(Role::Lead)
            .find(|n| n.start_beat >= lo && n.start_beat < hi)
            .expect("the lead sings in every episode")
            .prov
            .section
    };
    let verse = lead_section(0.0, 16.0);
    let hook = lead_section(16.0, 32.0);
    assert_ne!(verse, hook, "the verse and hook are distinct sections");
    for (lo, hi, want) in [(0.0, 16.0, verse), (16.0, 32.0, hook)] {
        for role in [Role::Keys, Role::Bass] {
            for n in s
                .role_notes(role)
                .filter(|n| n.start_beat >= lo && n.start_beat < hi)
            {
                assert_eq!(
                    n.prov.section, want,
                    "{role:?} note @{:.3} disagrees with the lead about the section",
                    n.start_beat
                );
            }
        }
        for d in s
            .drums
            .iter()
            .filter(|d| d.start_beat >= lo && d.start_beat < hi)
        {
            assert_eq!(
                d.prov.section, want,
                "drum @{:.3} disagrees with the lead about the section",
                d.start_beat
            );
        }
    }
}

// ================================================================================================
// R2 — the four-section phase-change arc: verse → hook → bridge(departs) → hook(returns), the first
// AUDIBLE zoom-out of the long-form engine. These falsifiably pin the arc's INTEGRITY — a bridge
// that does not genuinely depart, or a return that abandons the hook, is REJECTED, so the ending is
// earned by a real journey and never by pasting the hook into the middle — and that the full arc
// performs and passes the content witness. The eight-bar control above is unchanged (same kernel).
// ================================================================================================
use gibson::audio::human_music::argument::{ArgumentError, ArgumentRelation, ReferentId};
use gibson::audio::human_music::theory::Quality;

fn arc_take(seed: u64) -> CommittedSong {
    kernel::perform_song(&source_bridge::source(seed), &world()).unwrap()
}

#[test]
fn the_arc_states_verse_hook_bridge_return() {
    let (argument, _song) = source_bridge::argument(2112);
    assert_eq!(argument.referents.len(), 3, "verse + hook + bridge");
    assert_eq!(argument.steps.len(), 4);
    assert!(matches!(
        argument.steps[0].relation,
        ArgumentRelation::Establish
    ));
    assert!(matches!(
        argument.steps[1].relation,
        ArgumentRelation::Consequent { .. }
    ));
    assert!(matches!(
        argument.steps[2].relation,
        ArgumentRelation::Depart { .. }
    ));
    assert!(matches!(
        argument.steps[3].relation,
        ArgumentRelation::Return { .. }
    ));
    // The return restates the HOOK referent (identity preserved); its dependency graph records the
    // earned journey — it depends on BOTH the hook it restates and the bridge it returns FROM.
    assert_eq!(argument.steps[3].referent, ReferentId(1));
    assert_eq!(argument.steps[1].referent, ReferentId(1));
    assert!(
        argument.steps[3].depends_on.contains(&argument.steps[2].id),
        "the return must depend on the departure (the home-coming follows the bridge)"
    );
    assert!(argument.steps[3].depends_on.contains(&argument.steps[1].id));
    // The argument validates: the departure and consequent laws hold on the DEVELOPED material.
    assert_eq!(argument.validate(), Ok(()));
}

#[test]
fn the_four_section_arc_performs_and_passes_the_witness() {
    for seed in [2112u64, 7, 99] {
        let take = arc_take(seed);
        let s = take.score();
        assert_eq!(s.total_beats, 64.0, "seed {seed}: four 16-beat episodes");
        assert!(s.mono_voice);
        // The lead sings in every episode (no dead sections); all four sections are realized.
        for (lo, hi) in [(0.0, 16.0), (16.0, 32.0), (32.0, 48.0), (48.0, 64.0)] {
            assert!(
                s.role_notes(Role::Lead)
                    .any(|n| n.start_beat >= lo && n.start_beat < hi),
                "seed {seed}: the lead is silent in [{lo},{hi})"
            );
        }
        // The content witness accepts the realized arc (source identity, dependency order, the
        // departure and return laws all observed in the RAW Score); the candidate re-validates.
        assert!(
            gibson::audio::human_music::argument_witness::observe_argument(
                &source_bridge::source(seed),
                s,
                world().tonic_pc
            )
            .valid(),
            "seed {seed}: the content witness refused the realized arc"
        );
        take.validate_candidate(s).unwrap();
    }
}

#[test]
fn the_bridge_departs_harmonically_and_melodically() {
    let take = arc_take(2112);
    let s = take.score();
    // Harmonic departure: the bridge section [32,48) pivots onto a DOMINANT (A7 = V7/iv, then E7 =
    // V7) — an active, driving sound, not the home the verse/hook rest on. (This is also the A7-is-
    // not-Home fix at work: the tonic-root dominant drives the band instead of anchoring it.)
    assert!(
        s.chords.iter().any(|c| c.start_beat >= 32.0
            && c.start_beat < 48.0
            && c.chord.quality == Quality::Dom7),
        "the bridge must pivot onto a dominant"
    );
    // Melodic departure: the bridge lead is a genuinely new line, not a repeat of the hook.
    let bridge_lead = pitches(s, Role::Lead, 32.0, 48.0);
    let hook_lead = pitches(s, Role::Lead, 16.0, 32.0);
    assert!(!bridge_lead.is_empty());
    assert_ne!(
        bridge_lead, hook_lead,
        "the bridge must depart, not restate the hook verbatim"
    );
}

#[test]
fn a_bridge_that_does_not_depart_is_rejected() {
    // Replace the bridge referent with a relabelled restatement of the HOOK (same register, lands
    // CLOSED on the tonic): it does not lift and does not end open, so it is NOT a departure. The
    // contract must refuse it — the bridge is load-bearing; the ending is earned only by a genuine
    // journey, never by pasting the hook into the middle.
    let (mut argument, _song) = source_bridge::argument(7);
    argument.referents[2].events = argument.referents[1].events.clone();
    assert!(
        matches!(
            argument.validate(),
            Err(ArgumentError::DepartureNotRelated(_))
        ),
        "a hook restatement in the bridge slot must fail the departure law"
    );
}

#[test]
fn a_return_that_abandons_the_hook_is_rejected() {
    // Point the return step at the VERSE referent instead of the hook: the home-coming would land on
    // a different tune, breaking the "return restates the hook" identity. The contract must refuse it.
    let (mut argument, _song) = source_bridge::argument(99);
    argument.steps[3].referent = ReferentId(0);
    assert!(
        matches!(argument.validate(), Err(ArgumentError::WrongSource(_))),
        "a return that wanders off the hook must fail the same-head law"
    );
}

// ================================================================================================
// THE EARNED RETURN — the home-coming carries a consequence of the SPECIFIC bridge, not just a
// `depends_on` label. Two LAWFUL bridges over the same verse, hook, chart and the same returning
// hook must produce a DIFFERENT return performance, in a Score-observable way that is not the lead
// melody (which stays the exact hook) and not mere metadata or loudness. The mechanism: the return
// keys re-enter from the pitch the specific bridge left hanging open and resolve it into the home
// harmony, so a different journey resolves differently. (Directive §4 / audit control #1 & T8.)
// ================================================================================================
#[test]
fn lawful_bridge_variants_change_the_return() {
    let seed = 2112u64;
    // Two lawful bridges: identical verse/hook/chart/returning-hook, only the bridge MOTIF differs
    // (its own seed). Both perform — so both are legal arguments (the departure/return laws hold).
    let b1 = source_bridge::source_with_bridge_seed(seed, seed);
    let b2 = source_bridge::source_with_bridge_seed(seed, 9001);
    let t1 = kernel::perform_song(&b1, &world()).unwrap();
    let t2 = kernel::perform_song(&b2, &world()).unwrap();
    let (s1, s2) = (t1.score(), t2.score());

    // The counterfactual is real: the two bridge motifs genuinely differ (not a relabel).
    assert_ne!(
        pitches(s1, Role::Lead, 32.0, 48.0),
        pitches(s2, Role::Lead, 32.0, 48.0),
        "the two bridge motifs must differ for this to be a counterfactual"
    );
    // Everything BEFORE the bridge is untouched by the bridge seed — the verse and first hook are
    // identical, so any later difference is attributable to the bridge alone.
    assert_eq!(
        pitches(s1, Role::Lead, 0.0, 32.0),
        pitches(s2, Role::Lead, 0.0, 32.0),
        "the verse and first hook must not depend on the bridge seed"
    );
    // The returning HOOK melody is the exact same tune under both bridges — the headline is NOT what
    // changes (directive T8: a louder or re-pitched return would be a different, illegitimate claim).
    assert_eq!(
        pitches(s1, Role::Lead, 48.0, 64.0),
        pitches(s2, Role::Lead, 48.0, 64.0),
        "the return LEAD must be the identical hook under both bridges"
    );
    // THE EARNED RETURN: the return-window keys DIFFER between the two journeys — a witnessed,
    // Score-observable consequence of WHICH bridge was travelled. This is RED when the bridge only
    // feeds a formal `depends_on` label (the keys would be identical) and GREEN once the bridge's
    // hanging tendency actually drives the home-coming's accompaniment.
    assert_ne!(
        keys_in(&t1, 48.0, 64.0),
        keys_in(&t2, 48.0, 64.0),
        "the return accompaniment must depend on the SPECIFIC bridge, not just its dependency edge"
    );
}

// ================================================================================================
// Expressive phrasing — the opt-in arm that moves the lead's information onto its PERFORMANCE
// channels (gates, breaths, dynamics) without touching one pitch, onset, or the note count. These
// pin the whole render (not just the generator) and guard the two load-bearing invariants Leah's
// directive sets: do not lower the information density, and do not regress the audible earned return.
// ================================================================================================

/// Treatment 1, at the committed-Score level: the expressive lead has the SAME onsets, pitches and
/// COUNT as the plain control (density preserved — not thinned), but its durations move (articulation)
/// and its velocity SPREAD is strictly wider (the information expressed as dynamics, not more notes).
#[test]
fn expressive_lead_moves_information_off_bare_notes() {
    for seed in [2112u64, 7, 99] {
        let plain = kernel::perform_song(&source_bridge::source(seed), &world()).unwrap();
        let expr = kernel::perform_song(
            &source_bridge::source_phrased(seed, Phrasing::Expressive),
            &world(),
        )
        .unwrap();
        let lead = |t: &CommittedSong| -> Vec<(f64, i32, f32, f32)> {
            let mut ns: Vec<_> = t
                .score()
                .role_notes(Role::Lead)
                .map(|n| (n.start_beat, n.pitch, n.dur_beats, n.velocity))
                .collect();
            ns.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            ns
        };
        let (lp, le) = (lead(&plain), lead(&expr));
        assert_eq!(
            lp.len(),
            le.len(),
            "seed {seed}: lead note COUNT must be preserved"
        );
        for (a, b) in lp.iter().zip(&le) {
            assert!(
                (a.0 - b.0).abs() < 1e-6,
                "seed {seed}: lead onsets must match"
            );
            assert_eq!(a.1, b.1, "seed {seed}: lead pitches must match");
        }
        let dur_moved = lp
            .iter()
            .zip(&le)
            .filter(|(a, b)| (a.2 - b.2).abs() > 1e-6)
            .count();
        assert!(
            dur_moved > 0,
            "seed {seed}: expressive must re-articulate the lead"
        );
        let spread = |ns: &[(f64, i32, f32, f32)]| {
            let hi = ns.iter().map(|n| n.3).fold(0.0_f32, f32::max);
            let lo = ns.iter().map(|n| n.3).fold(f32::MAX, f32::min);
            hi - lo
        };
        assert!(
            spread(&le) > spread(&lp),
            "seed {seed}: expressive velocity spread ({:.3}) must exceed plain ({:.3})",
            spread(&le),
            spread(&lp),
        );
    }
}

/// H: the earned return survives the expressive policy. Two lawful bridges under `Expressive` still
/// bring home the SAME returning-hook melody (the headline is not what changes) while the return
/// accompaniment DIFFERS by which bridge was travelled — the B result, intact under the new phrasing.
#[test]
fn expressive_preserves_the_earned_bridge_return() {
    let seed = 2112u64;
    let b1 = source_bridge::source_with_bridge_seed_phrased(seed, seed, Phrasing::Expressive);
    let b2 = source_bridge::source_with_bridge_seed_phrased(seed, 9001, Phrasing::Expressive);
    let t1 = kernel::perform_song(&b1, &world()).unwrap();
    let t2 = kernel::perform_song(&b2, &world()).unwrap();
    let (s1, s2) = (t1.score(), t2.score());
    assert_ne!(
        pitches(s1, Role::Lead, 32.0, 48.0),
        pitches(s2, Role::Lead, 32.0, 48.0),
        "the two bridge motifs must differ (a real counterfactual)"
    );
    assert_eq!(
        pitches(s1, Role::Lead, 48.0, 64.0),
        pitches(s2, Role::Lead, 48.0, 64.0),
        "the return LEAD must be the identical hook under both bridges, even expressive"
    );
    assert_ne!(
        keys_in(&t1, 48.0, 64.0),
        keys_in(&t2, 48.0, 64.0),
        "the expressive return accompaniment must still depend on the SPECIFIC bridge"
    );
}

/// R2 (ensemble distribution): the `Ensemble` arm makes the DRUMMER answer into the lead's breaths —
/// bounded "answer" strokes appear — while the plain and expressive arms add none, and the LEAD is
/// untouched (R2 moves information to the band, it does not re-perform the melody). This is the
/// cross-role exchange, kept opt-in: only a source built with `Phrasing::Ensemble` triggers it.
#[test]
fn ensemble_answers_the_breaths_only_in_the_ensemble_arm() {
    let seed = 2112u64;
    let plain = kernel::perform_song(&source_bridge::source(seed), &world()).unwrap();
    let expr = kernel::perform_song(
        &source_bridge::source_phrased(seed, Phrasing::Expressive),
        &world(),
    )
    .unwrap();
    let ens = kernel::perform_song(
        &source_bridge::source_phrased(seed, Phrasing::Ensemble),
        &world(),
    )
    .unwrap();
    let answers = |t: &CommittedSong| {
        t.score()
            .drums
            .iter()
            .filter(|d| d.prov.role_note == "answer")
            .count()
    };
    assert_eq!(
        answers(&plain),
        0,
        "the plain arm must add no breath answers"
    );
    assert_eq!(
        answers(&expr),
        0,
        "the expressive arm must add no breath answers"
    );
    assert!(
        answers(&ens) > 0,
        "the ensemble arm must answer the lead's breaths"
    );
    // R2 does not touch the LEAD: the ensemble lead lane is identical to the expressive one.
    let lead = |t: &CommittedSong| -> Vec<(u64, i32, u32)> {
        t.score()
            .role_notes(Role::Lead)
            .map(|n| (n.start_beat.to_bits(), n.pitch, n.velocity.to_bits()))
            .collect()
    };
    assert_eq!(
        lead(&expr),
        lead(&ens),
        "ensemble must not change the lead — only the band answers"
    );
}

// ======================================================================================
// THE JERRY LAW — default-correctness tests (D1–D7).
//
// These are red-first against the PRE-FLIP behavior: the old default resolved to the 8-bar
// plain control, so D1 (bare == Full) and D5 (Full leaves a trace) would have failed before
// `C137Preset` and the default flip existed. They pin that excellence is now the default and a
// Jerry switch is the explicit, real, deterministic downgrade. Nothing here reads a WAV hash —
// equality is relational (resolved config + Score fingerprint), per the regression audit.
// ======================================================================================

use preset::{C137Preset, Jerry, Resolved};

/// Build the committed source for a resolved config — the same dispatch the example's `build_source`
/// performs, so these tests exercise the exact path the no-flag command renders.
fn build(resolved: &Resolved, seed: u64) -> gibson::audio::human_music::argument::CompiledArgument {
    if resolved.bars == 16 {
        source_bridge::source_phrased(seed, resolved.phrasing)
    } else {
        source::source_phrased(seed, resolved.phrasing)
    }
}

fn render_cfg(resolved: &Resolved, seed: u64) -> CommittedSong {
    kernel::perform_song(&build(resolved, seed), &world()).unwrap()
}

/// The harmonic route witness: the ordered chord anchors. An unrelated-commitment invariant — a
/// lead-performance Jerry switch must leave this untouched.
fn chord_route(score: &Score) -> Vec<(u64, i32)> {
    score
        .chords
        .iter()
        .map(|c| (c.start_beat.to_bits(), c.chord.root_pc))
        .collect()
}

/// D1 — the bare, no-flag command resolves to C137 Full, and renders the identical Score as an
/// explicitly-Full configuration. (The old default was 8-bar plain; this pins the flip.)
#[test]
fn d1_bare_c137_equals_full_c137() {
    let bare = C137Preset::default().resolve();
    let full = C137Preset::full().resolve();
    assert_eq!(bare, full, "the bare preset must BE C137 Full");
    assert!(full.is_full(), "Full must report itself full");
    assert_eq!(full.bars, 16, "Full renders the complete 16-bar arc");
    assert_eq!(
        full.phrasing,
        Phrasing::Ensemble,
        "Full plays the ensemble lead"
    );
    assert!(full.jerry.is_empty(), "Full carries no downgrade");
    // The flip: Full is NOT the old 8-bar plain default.
    let old_default = C137Preset::full()
        .jerry(Jerry::Short)
        .jerry(Jerry::Plain)
        .resolve();
    assert_ne!(
        full, old_default,
        "Full must differ from the former 8-bar plain default"
    );
    for seed in [2112u64, 7, 99] {
        let a = render_cfg(&bare, seed);
        let b = render_cfg(&full, seed);
        assert_eq!(
            a.score().fingerprint(),
            b.score().fingerprint(),
            "seed {seed}: bare and explicit-Full must render the identical Score (⇒ identical PCM)"
        );
    }
}

/// D2 — each Jerry switch changes EXACTLY its advertised capability, preserves the unrelated source
/// commitments, and is OFF by default.
#[test]
fn d2_jerry_switches_are_deliberate() {
    let seed = 2112u64;
    let full = C137Preset::full().resolve();
    assert!(full.jerry.is_empty(), "no Jerry switch is on by default");
    let full_t = render_cfg(&full, seed);

    // short: the 8-bar arc; does not touch the lead performance.
    let short = C137Preset::full().jerry(Jerry::Short).resolve();
    assert_eq!(short.bars, 8);
    assert_eq!(
        short.phrasing,
        Phrasing::Ensemble,
        "short must not touch the lead performance"
    );
    let short_t = render_cfg(&short, seed);
    assert_eq!(full_t.score().total_beats, 64.0);
    assert_eq!(
        short_t.score().total_beats,
        32.0,
        "short renders the verse→hook control"
    );

    // plain: robotic lead + no band answer; harmony preserved; arc preserved.
    let plain = C137Preset::full().jerry(Jerry::Plain).resolve();
    assert_eq!(plain.phrasing, Phrasing::Plain);
    assert_eq!(plain.bars, 16, "plain must not touch the arc");
    let plain_t = render_cfg(&plain, seed);
    assert!(
        preset::lead_velocity_spread(full_t.score())
            > preset::lead_velocity_spread(plain_t.score()),
        "plain must flatten the lead dynamics"
    );
    assert_eq!(
        preset::breath_answers(plain_t.score()),
        0,
        "plain silences the band answer"
    );
    assert_eq!(
        chord_route(full_t.score()),
        chord_route(plain_t.score()),
        "plain must preserve the unrelated harmonic commitment"
    );

    // deaf: the expressive lead is retained identically; only the band answer is removed.
    let deaf = C137Preset::full().jerry(Jerry::DeafBand).resolve();
    assert_eq!(deaf.phrasing, Phrasing::Expressive);
    let deaf_t = render_cfg(&deaf, seed);
    assert_eq!(
        lane(full_t.score(), Role::Lead),
        lane(deaf_t.score(), Role::Lead),
        "deaf must keep the IDENTICAL expressive lead"
    );
    assert!(
        preset::breath_answers(full_t.score()) > 0,
        "Full answers the breaths"
    );
    assert_eq!(
        preset::breath_answers(deaf_t.score()),
        0,
        "deaf removes exactly the band answer"
    );
}

/// D3 — the archived controls remain reachable and reproduce. `short+plain` IS the former default and
/// reproduces the frozen 8-bar plain render; `plain` alone reproduces the 16-bar plain render.
#[test]
fn d3_archived_controls_remain_available() {
    let old = C137Preset::full()
        .jerry(Jerry::Short)
        .jerry(Jerry::Plain)
        .resolve();
    assert_eq!((old.bars, old.phrasing), (8, Phrasing::Plain));
    for seed in [2112u64, 7, 99] {
        let via_preset = render_cfg(&old, seed);
        let frozen =
            kernel::perform_song(&source::source_phrased(seed, Phrasing::Plain), &world()).unwrap();
        let historical = kernel::perform_song(&source::source(seed), &world()).unwrap();
        assert_eq!(
            via_preset.score().fingerprint(),
            frozen.score().fingerprint(),
            "seed {seed}: short+plain must reproduce the frozen 8-bar plain control"
        );
        assert_eq!(
            frozen.score().fingerprint(),
            historical.score().fingerprint(),
            "seed {seed}: the plain 8-bar path is the byte-identical historical control"
        );
        // 16-bar plain (jerry=plain alone) reproduces the frozen 16-bar plain render.
        let bars16_plain = C137Preset::full().jerry(Jerry::Plain).resolve();
        let via16 = render_cfg(&bars16_plain, seed);
        let frozen16 = kernel::perform_song(
            &source_bridge::source_phrased(seed, Phrasing::Plain),
            &world(),
        )
        .unwrap();
        assert_eq!(
            via16.score().fingerprint(),
            frozen16.score().fingerprint(),
            "seed {seed}: jerry=plain must reproduce the frozen 16-bar plain render"
        );
    }
}

/// D4 — no hidden downgrade: the rendered arc length and lead performance always match what the
/// resolved config (hence the manifest) advertises. The same `Resolved` drives both, so this pins
/// that there is no code path that reports Full while building less.
#[test]
fn d4_no_hidden_downgrade() {
    let seed = 2112u64;
    for p in [
        C137Preset::full(),
        C137Preset::full().jerry(Jerry::Short),
        C137Preset::full().jerry(Jerry::Plain),
        C137Preset::full().jerry(Jerry::DeafBand),
        C137Preset::full().jerry(Jerry::Short).jerry(Jerry::Plain),
    ] {
        let r = p.resolve();
        let t = render_cfg(&r, seed);
        assert_eq!(
            t.score().total_beats,
            r.total_beats(),
            "the rendered arc length must equal the resolved (manifested) length: {r:?}"
        );
        let answers = preset::breath_answers(t.score());
        if r.phrasing == Phrasing::Ensemble {
            assert!(
                answers > 0,
                "Ensemble manifested but nothing answered: {r:?}"
            );
        } else {
            assert_eq!(
                answers, 0,
                "a non-ensemble config must render no answer: {r:?}"
            );
        }
    }
}

/// D5 — no fake enablement: every capability the Full manifest advertises as ACTIVE leaves a witnessed
/// trace on the rendered Score (and the matching downgrade removes it — pinned in D2).
#[test]
fn d5_no_fake_enablement() {
    for seed in [2112u64, 7, 99] {
        let t = render_cfg(&C137Preset::full().resolve(), seed);
        let s = t.score();
        assert!(
            preset::lead_breaths(s) > 0,
            "seed {seed}: Full lead must open real breaths"
        );
        assert!(
            preset::breath_answers(s) > 0,
            "seed {seed}: Full band must answer the breaths"
        );
        assert!(preset::pad_notes(s) > 0, "seed {seed}: Full rug must sound");
        assert!(
            preset::lead_velocity_spread(s) > 0.0,
            "seed {seed}: Full lead must be dynamically shaped"
        );
    }
}

/// D6 — compatibility & known-bad exclusions: the pre-preset flags fold to the right Jerry switch (so
/// no secret positive combination is ever needed), and C137 Full carries no Jerry and does not reach
/// the ear-rejected controls (CoupledR8 / low-register BASS_WEAVE live only on their own pathways).
#[test]
fn d6_compat_and_known_bad_exclusions() {
    assert_eq!(
        C137Preset::full().jerry(Jerry::Short).resolve().bars,
        8,
        "--bars=8 ≡ jerry short"
    );
    assert_eq!(
        C137Preset::full().jerry(Jerry::Plain).resolve().phrasing,
        Phrasing::Plain,
        "--phrasing=plain ≡ jerry plain"
    );
    assert_eq!(
        C137Preset::full().jerry(Jerry::DeafBand).resolve().phrasing,
        Phrasing::Expressive,
        "--phrasing=expressive ≡ jerry deaf"
    );
    let full = C137Preset::full().resolve();
    assert!(
        full.is_full() && full.jerry.is_empty(),
        "excellence needs no secret flag combination"
    );
    // The Full path never stages the glitched low bass weave: Full renders a single monophonic bass
    // foundation (one note at a time), the invariant the BASS_WEAVE control would violate.
    let full_take = render_cfg(&full, 2112);
    let s = full_take.score();
    let mut bass: Vec<_> = s.role_notes(Role::Bass).collect();
    bass.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    for w in bass.windows(2) {
        let end = w[0].start_beat + f64::from(w[0].dur_beats);
        assert!(
            w[1].start_beat >= end - 1e-6,
            "Full bass must stay monophonic (no overlapping low weave): {:.3} overlaps {:.3}",
            w[0].start_beat,
            w[1].start_beat
        );
    }
}

/// D7 — determinism: Full and every Jerry reduction reproduce an identical Score under the same seed.
#[test]
fn d7_determinism() {
    for p in [
        C137Preset::full(),
        C137Preset::full().jerry(Jerry::Short),
        C137Preset::full().jerry(Jerry::Plain),
        C137Preset::full().jerry(Jerry::DeafBand),
        C137Preset::full().jerry(Jerry::Short).jerry(Jerry::Plain),
    ] {
        let r = p.resolve();
        let a = render_cfg(&r, 2112);
        let b = render_cfg(&r, 2112);
        assert_eq!(
            a.score().fingerprint(),
            b.score().fingerprint(),
            "{r:?} must render deterministically"
        );
    }
}
