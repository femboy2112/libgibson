//! Falsifiable long-form tests: two causally-connected episodes in one continuous 32-beat Score,
//! and the memory law that couples the second episode's band decision to the first.
#[path = "../examples/c137_song/source.rs"]
mod source;
#[path = "../examples/c137_song/source_bridge.rs"]
mod source_bridge;
#[path = "../examples/c137_song/source_song.rs"]
mod source_song;
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
    theory::{Mode, Scale},
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

/// Johnny Five R1 — the keys' answer must tell the truth about its knowledge channel.
///
/// The base keys answer QUOTES the lead's authored cadence tail, which has not sounded when the
/// answer begins (negative latency): a rehearsed anticipation of the written phrase, not a reaction
/// to heard notes. The old receipt stamped `overlap: false` on that negative latency — a lie the
/// diagnostics already contradicted (they count it as an overlapping response). This pins the
/// repair: every SOUNDING keys response's `overlap` equals the sign of its latency, and its
/// `knowledge()` channel agrees — an overlapping answer is `KnownAuthoredFuture`, a post-hoc one
/// (the earned Return / memory-law discharge, which quote a fully-sounded PAST episode) is
/// `HeardActual`. Sound-preserving by construction: `overlap`/`latency` live only in the receipt
/// and diagnostics, never in the Score, so the PCM is untouched; only the receipt stops lying.
#[test]
fn the_keys_answer_declares_its_knowledge_channel_truthfully() {
    use gibson::audio::human_music::interaction::Knowledge;
    let mut saw_authored_future = false;
    let mut saw_heard_actual = false;
    for (label, arg) in [
        ("slice", source::source(2112)),
        ("song", source_song::source_song(2112)),
    ] {
        let song = kernel::perform_song(&arg, &world()).unwrap();
        let perf = &song.composition().perf;
        for row in &perf.interactions {
            let Some(r) = &row.response else { continue };
            if r.action.is_none() {
                continue; // a deliberate silence sounds nothing; its latency is bookkeeping
            }
            // The receipt can no longer deny its own overlap (the J3 lie).
            assert_eq!(
                r.overlap,
                r.latency < 0.0,
                "[{label}] {:?}: overlap={} contradicts latency={:.4}",
                row.id,
                r.overlap,
                r.latency
            );
            // ...and the declared knowledge channel is exactly that overlap truth.
            match r.knowledge() {
                Knowledge::KnownAuthoredFuture => {
                    assert!(
                        r.overlap,
                        "[{label}] {:?}: an anticipatory answer must record overlap",
                        row.id
                    );
                    saw_authored_future = true;
                }
                Knowledge::HeardActual => {
                    assert!(
                        !r.overlap,
                        "[{label}] {:?}: a heard answer must not record overlap",
                        row.id
                    );
                    saw_heard_actual = true;
                }
            }
        }
    }
    // The Full song exercises BOTH channels: the base keys answers anticipate the authored cadence
    // tail (KnownAuthoredFuture), while the earned Return / memory-law answers quote a fully-sounded
    // past episode (HeardActual). If either vanishes, the distinction has collapsed to one channel.
    assert!(
        saw_authored_future,
        "expected at least one anticipatory (KnownAuthoredFuture) keys answer"
    );
    assert!(
        saw_heard_actual,
        "expected at least one reactive (HeardActual) keys answer"
    );
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
    let mut compiled = if resolved.bars == 16 {
        source_bridge::source_phrased(seed, resolved.phrasing)
    } else {
        source::source_phrased(seed, resolved.phrasing)
    };
    // The decoupling: the resolved capability booleans override the source builder's phrasing-derived
    // defaults, exactly as the example's `build_source` does. For the Full/short/plain/deaf configs the
    // resolved caps equal the phrasing-derived ones (so D1–D7 stay byte-frozen); the narrow switches
    // change exactly their own capability.
    resolved.apply_caps(&mut compiled);
    compiled
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

// ======================================================================================
// THE JERRY LAW — independent feature ownership (D8–D11).
//
// The narrow Jerry switches decouple the four band capabilities from the lead phrasing. Each test
// pins that switching ONE capability changes only its own promised causal domain and leaves the
// unrelated commitments intact — the §3 "independent feature ownership" requirement. Where a switch
// genuinely changes the lead's input (flat-lead), the downstream band reads legitimately differ, so
// those tests pin the CAPABILITY survived, not byte-invariance; where the lead is untouched
// (no-keys / straight-bass / generic-drums), the unrelated lanes are byte-identical to Full.
// ======================================================================================

/// D8 — `flat-lead` is the LEAD-ONLY downgrade: the lead goes plain, but the accepted keys/bass/drums
/// survive — unlike the compound `plain`, which strips them all. This is the whole point of the fix.
#[test]
fn d8_flat_lead_downgrades_only_the_lead() {
    for seed in [2112u64, 7, 99] {
        let full = render_cfg(&C137Preset::full().resolve(), seed);
        let flat = render_cfg(&C137Preset::full().jerry(Jerry::FlatLead).resolve(), seed);
        let plain = render_cfg(&C137Preset::full().jerry(Jerry::Plain).resolve(), seed);

        // The lead went plain — the one thing flat-lead touches. Its lead is the frozen plain lead.
        assert!(
            preset::lead_velocity_spread(full.score()) > preset::lead_velocity_spread(flat.score()),
            "seed {seed}: flat-lead must flatten the lead dynamics"
        );
        assert_eq!(
            lane(flat.score(), Role::Lead),
            lane(plain.score(), Role::Lead),
            "seed {seed}: flat-lead plays the same plain lead as the compound plain control"
        );

        // THE DECOUPLING: the keyboard comp SURVIVES the plain lead (plain kills it).
        assert!(
            preset::keys_comps(flat.score()) > 0,
            "seed {seed}: flat-lead must KEEP the keyboard comp"
        );
        assert_eq!(
            preset::keys_comps(plain.score()),
            0,
            "seed {seed}: the compound plain removes the comp — the contrast that proves decoupling"
        );

        // The bass is lead-independent (reads the chart), so flat-lead keeps Full's destination-aware
        // bass byte-for-byte, while the compound plain reverts it to the blind chromatic approach.
        assert_eq!(
            lane(flat.score(), Role::Bass),
            lane(full.score(), Role::Bass),
            "seed {seed}: flat-lead must keep Full's destination-aware bass (bass is lead-independent)"
        );
        assert_ne!(
            lane(flat.score(), Role::Bass),
            lane(plain.score(), Role::Bass),
            "seed {seed}: the compound plain strips the bass anticipation that flat-lead keeps"
        );
        assert!(
            preset::bass_anticipations(flat.score(), 9) > 0,
            "seed {seed}: flat-lead bass still anticipates the home arrival"
        );

        // The drummer still translates the (now plain) lead's rhythm — translation survived.
        assert!(
            preset::lead_echoes(flat.score()) > 0,
            "seed {seed}: flat-lead must keep the drum lead-rhythm translation"
        );
    }
}

/// D9 — `no-keys` removes ONLY the keyboard comp; the lead, bass, drums and breath answer are the
/// identical Full commitment (the lead is unchanged, so every band read of it is unchanged too).
#[test]
fn d9_no_key_comp_removes_only_the_comp() {
    for seed in [2112u64, 7, 99] {
        let full = render_cfg(&C137Preset::full().resolve(), seed);
        let nk = render_cfg(&C137Preset::full().jerry(Jerry::NoKeyComp).resolve(), seed);

        assert!(preset::keys_comps(full.score()) > 0);
        assert_eq!(
            preset::keys_comps(nk.score()),
            0,
            "seed {seed}: no-keys removes the keyboard comp"
        );
        // The per-episode keys answer remains (the comp was additive to it).
        assert!(
            nk.score().role_notes(Role::Keys).count() < full.score().role_notes(Role::Keys).count(),
            "seed {seed}: no-keys leaves fewer keys (comp gone) but keeps the answer"
        );
        // Every unrelated lane is byte-identical to Full.
        assert_eq!(lane(nk.score(), Role::Lead), lane(full.score(), Role::Lead));
        assert_eq!(
            lane(nk.score(), Role::Bass),
            lane(full.score(), Role::Bass),
            "seed {seed}: removing keys comp must not touch the bass"
        );
        assert_eq!(lane(nk.score(), Role::Pad), lane(full.score(), Role::Pad));
        assert_eq!(
            nk.score().drums.len(),
            full.score().drums.len(),
            "seed {seed}: removing keys comp must not touch the drums"
        );
        assert_eq!(
            preset::lead_echoes(nk.score()),
            preset::lead_echoes(full.score()),
            "seed {seed}: drum translation unchanged"
        );
        assert_eq!(
            preset::breath_answers(nk.score()),
            preset::breath_answers(full.score()),
            "seed {seed}: breath answer unchanged"
        );
    }
}

/// D10 — `straight-bass` removes ONLY the bass's V→I destination anticipation (reverts to the blind
/// chromatic approach); the keys, lead and drums are the identical Full commitment, and the bass keeps
/// its note count (a redistribution, not thinning).
#[test]
fn d10_straight_bass_removes_only_the_anticipation() {
    for seed in [2112u64, 7, 99] {
        let full = render_cfg(&C137Preset::full().resolve(), seed);
        let sb = render_cfg(
            &C137Preset::full().jerry(Jerry::StraightBass).resolve(),
            seed,
        );

        assert!(preset::bass_anticipations(full.score(), 9) > 0);
        assert_eq!(
            preset::bass_anticipations(sb.score(), 9),
            0,
            "seed {seed}: straight-bass removes the V->I anticipation"
        );
        assert_eq!(
            sb.score().role_notes(Role::Bass).count(),
            full.score().role_notes(Role::Bass).count(),
            "seed {seed}: straight-bass keeps the same bass note count (redistribution, not thinning)"
        );
        // Keys, lead, drum translation and breath answer unchanged.
        assert_eq!(
            lane(sb.score(), Role::Keys),
            lane(full.score(), Role::Keys),
            "seed {seed}: removing the bass anticipation must not change the keys' chords"
        );
        assert_eq!(lane(sb.score(), Role::Lead), lane(full.score(), Role::Lead));
        assert_eq!(
            preset::lead_echoes(sb.score()),
            preset::lead_echoes(full.score()),
            "seed {seed}: drum translation unchanged"
        );
    }
}

/// D11 — `generic-drums` removes ONLY the drum lead-rhythm translation (reverts to the archived
/// generic run); removing it must NOT change the keys' chords or the bass approaches (the plan's
/// explicit cross-domain guarantee).
#[test]
fn d11_generic_drums_removes_only_the_translation() {
    for seed in [2112u64, 7, 99] {
        let full = render_cfg(&C137Preset::full().resolve(), seed);
        let gd = render_cfg(
            &C137Preset::full().jerry(Jerry::GenericDrums).resolve(),
            seed,
        );

        assert!(preset::lead_echoes(full.score()) > 0);
        assert_eq!(
            preset::lead_echoes(gd.score()),
            0,
            "seed {seed}: generic-drums removes the lead-echo translation"
        );
        assert_eq!(
            lane(gd.score(), Role::Keys),
            lane(full.score(), Role::Keys),
            "seed {seed}: removing drum translation must not change the keys' chords"
        );
        assert_eq!(
            lane(gd.score(), Role::Bass),
            lane(full.score(), Role::Bass),
            "seed {seed}: removing drum translation must not change the bass approaches"
        );
        assert_eq!(lane(gd.score(), Role::Lead), lane(full.score(), Role::Lead));
        assert_eq!(
            preset::breath_answers(gd.score()),
            preset::breath_answers(full.score()),
            "seed {seed}: breath answer unchanged by generic-drums"
        );
    }
}

// ======================================================================================
// §4 — the phrase-seam snare collision is a pinned invariant, not a float accident.
//
// At the four phrase seams a backbeat "pulse" snare and a "lead-echo" stroke quantize to the EXACT
// same sample (144 BPM @ 48 kHz => 20000 samples/beat; the lead-echo lands at pocket(3.0)=0.030, bit-
// equal to the backbeat's). The synth owns a single Snare voice with last-trigger-wins, so the Score
// carries two strokes where one sounds — the lead-echo (ordered last) overwrites the backbeat. That
// overwrite is ear-accepted. This pins it as a DETERMINISTIC, intended invariant rather than an
// accident of float ordering, and guards the latent fragility where a 1-ULP beat difference could
// silently flip the winner to the louder backbeat with no Score-visible change. Pin, do not repair:
// the accepted audio is unchanged.
// ======================================================================================

#[test]
fn the_phrase_seam_snare_collision_is_pinned_pulse_then_lead_echo() {
    // 144 BPM at the 48 kHz studio rate: one beat is exactly 20000 samples.
    let to_sample = |beat: f64| (beat * 20000.0).round() as i64;
    for seed in [2112u64, 7, 99] {
        let full = render_cfg(&C137Preset::full().resolve(), seed);
        let drums = &full.score().drums;
        let mut collisions = 0usize;
        for (i, a) in drums.iter().enumerate() {
            for b in drums.iter().skip(i + 1) {
                if a.voice == b.voice && to_sample(a.start_beat) == to_sample(b.start_beat) {
                    collisions += 1;
                    // The ONLY same-voice same-sample drum collision in the Full arc is the phrase-seam
                    // backbeat pulse snare (earlier Score index) overwritten by a lead-echo (later index,
                    // so the single Snare voice plays it). Pin exactly that role pair AND that order —
                    // any other same-sample pile-up, or the order flipping, fails here.
                    assert!(
                        a.prov.role_note == "pulse" && b.prov.role_note == "lead-echo",
                        "seed {seed}: same-sample {:?} collision at beat {:.3} is not the expected \
                         pulse->lead-echo: {:?} then {:?}",
                        a.voice,
                        a.start_beat,
                        a.prov.role_note,
                        b.prov.role_note
                    );
                }
            }
        }
        assert_eq!(
            collisions, 4,
            "seed {seed}: expected exactly the 4 phrase-seam snare collisions, found {collisions}"
        );
    }
}

// ======================================================================================
// §5 — the drummer TRULY answers (opt-in `source_answer`).
//
// The accepted fixed-slot breath answer reads no lead content and carries a dead provenance stamp —
// it answers nothing in particular. With `source_answer` the answer QUOTES the lead's question (each
// stroke sits a half-bar after a specific lead attack in its bar's first half — content-derived) and
// realizes a real `ActionKind::Answer` that CREDITS the lead statement's call it answers (a
// reconstructible initiating source, via the existing Action ontology). The upgrade is opt-in and
// audio-changing, so the default render stays byte-frozen (pinned by D1); these tests pin the honesty.
// ======================================================================================

fn source_answer_take(seed: u64) -> CommittedSong {
    let mut src = source_bridge::source_phrased(seed, Phrasing::Ensemble);
    src.source_answer = true;
    kernel::perform_song(&src, &world()).unwrap()
}

/// Content-derivation: every source-grounded answer stroke quotes a SPECIFIC lead attack from the
/// first half of its bar, a half-bar later — so reshaping the question reshapes the answer. (The
/// hollow default fires on fixed slots regardless of what the lead played.)
#[test]
fn the_source_answer_quotes_the_lead_question() {
    for seed in [2112u64, 7, 99] {
        let t = source_answer_take(seed);
        let s = t.score();
        let lead: Vec<f64> = s.role_notes(Role::Lead).map(|n| n.start_beat).collect();
        let answers: Vec<f64> = s
            .drums
            .iter()
            .filter(|d| d.prov.role_note == "answer")
            .map(|d| d.start_beat)
            .collect();
        assert!(
            !answers.is_empty(),
            "seed {seed}: the source answer must place some strokes"
        );
        for a in &answers {
            let bar_start = (a / 4.0).floor() * 4.0;
            // the answer onset is a first-half lead attack + 2.0, within the shared pocket tolerance
            let quotes = lead
                .iter()
                .any(|&l| l >= bar_start && l < bar_start + 2.0 && ((a - 2.0) - l).abs() < 0.06);
            assert!(
                quotes,
                "seed {seed}: source answer at {a:.3} must quote a first-half lead attack + 2.0"
            );
        }
    }
}

/// Reconstructible source: every source-grounded answer stroke realizes a real `Agent::Drums`
/// `ActionKind::Answer` whose `pays` is a lead statement's call — the initiating question. (The hollow
/// default carries an empty stamp: it credits nothing.)
#[test]
fn the_source_answer_credits_the_lead_call() {
    use gibson::audio::human_music::action::{ActionKind, Agent};
    for seed in [2112u64, 7, 99] {
        let t = source_answer_take(seed);
        let perf = &t.composition().perf;
        let lead_calls: std::collections::HashSet<_> =
            perf.statements.iter().filter_map(|st| st.call).collect();
        let answers: Vec<_> = t
            .score()
            .drums
            .iter()
            .filter(|d| d.prov.role_note == "answer")
            .collect();
        assert!(!answers.is_empty(), "seed {seed}: expected source answers");
        let acts = perf.actions.chronological();
        for d in &answers {
            let credited = acts.iter().any(|act| {
                d.prov.actions.has(act.id)
                    && act.kind == ActionKind::Answer
                    && act.initiator == Agent::Drums
                    && act.pays.is_some_and(|c| lead_calls.contains(&c))
            });
            assert!(
                credited,
                "seed {seed}: source answer at {:.3} must realize a Drums Answer that pays a lead call",
                d.start_beat
            );
        }
    }
}

/// The upgrade is strictly opt-in: the default Full answer is the fixed-slot ghost that credits
/// nothing (empty stamp), and turning the upgrade on genuinely changes the render (it is ear-gated,
/// not a silent default).
#[test]
fn the_source_answer_is_opt_in_and_audio_changing() {
    for seed in [2112u64, 7, 99] {
        let full = render_cfg(&C137Preset::full().resolve(), seed);
        let src = source_answer_take(seed);
        let default_answers: Vec<_> = full
            .score()
            .drums
            .iter()
            .filter(|d| d.prov.role_note == "answer")
            .collect();
        assert!(!default_answers.is_empty());
        assert!(
            default_answers.iter().all(|d| d.prov.actions.is_empty()),
            "seed {seed}: the default (accepted) answer credits nothing — the hollow stamp"
        );
        assert_ne!(
            full.score().fingerprint(),
            src.score().fingerprint(),
            "seed {seed}: source-answer must change the render (it is an ear-gated upgrade)"
        );
    }
}

// ======================================================================================
// CP2 — honest, source-grounded breath causality.
//
// The band answers a QUESTION, not a landing: a breath is answered only where it sits in a
// genuine lead silence AND the bar's harmony is still open (not a home-tonic resolution). Some
// breaths are deliberately never answered — silence is a legitimate response. These two tests
// are the independent source/Score witness + the falsifier that proves the answer is bound to
// the source harmony, not blind to it.
// ======================================================================================

/// Does `chord` land home (a tonic resolution: degree 0 and not a secondary dominant)? Mirrors the
/// kernel's `harmonic_drive` Home test, computed independently here from the committed Score.
fn is_home_resolution(root_pc: i32, quality: Quality, tonic: i32) -> bool {
    let degree = (root_pc - tonic).rem_euclid(12);
    degree == 0 && !matches!(quality, Quality::Dom7 | Quality::Dom9)
}

/// CP2 witness — the permitted breath-response relationship, read independently from the Score:
/// every "answer" stroke (a) sits in a genuine lead breath and (b) is on an OPEN bar, never a home
/// resolution. This is what makes the answer source-grounded rather than a blind silence filler.
#[test]
fn the_band_answers_only_the_unresolved_breaths() {
    let tonic = 9i32;
    for seed in [2112u64, 7, 99] {
        let t = render_cfg(&C137Preset::full().resolve(), seed);
        let s = t.score();
        let lead: Vec<_> = s.role_notes(Role::Lead).collect();
        let answers: Vec<f64> = s
            .drums
            .iter()
            .filter(|d| d.prov.role_note == "answer")
            .map(|d| d.start_beat)
            .collect();
        assert!(
            !answers.is_empty(),
            "seed {seed}: the ensemble must answer some breaths"
        );
        for &b in &answers {
            // (a) a genuine lead silence at the grid slot the answer was placed on (mirrors the
            // kernel's `lead_silent_at`: nothing sounding across it within a small margin).
            let grid = (b * 4.0).round() / 4.0;
            let sounding = lead.iter().any(|n| {
                grid >= n.start_beat - 0.08 && grid <= n.start_beat + f64::from(n.dur_beats) + 0.08
            });
            assert!(
                !sounding,
                "seed {seed}: answer at {b:.3} must sit in a genuine lead breath"
            );
            // (b) licensed: the covering chord is NOT a home resolution.
            let bar_start = (b / 4.0).floor() * 4.0;
            let chord = s
                .chords
                .iter()
                .find(|c| {
                    c.start_beat <= bar_start + 1e-9
                        && bar_start < c.start_beat + f64::from(c.dur_beats)
                })
                .expect("a chord covers every bar");
            assert!(
                !is_home_resolution(chord.chord.root_pc, chord.chord.quality, tonic),
                "seed {seed}: answer at {b:.3} must NOT fall on a home resolution (left in peace)"
            );
        }
    }
}

/// CP2 falsifier — a changed harmonic moment with the SAME lead (same onsets, pitches, silences)
/// changes the response: flip one open bar to a home resolution and the answer there must vanish.
/// The drummer is not blind to whether the moment is a question or a landing.
#[test]
fn a_resolved_moment_is_left_in_peace() {
    let seed = 2112u64;
    let answers_in = |t: &CommittedSong, lo: f64, hi: f64| {
        t.score()
            .drums
            .iter()
            .filter(|d| d.prov.role_note == "answer" && d.start_beat >= lo && d.start_beat < hi)
            .count()
    };
    // Bar 1 (route index 1 = the verse's predominant) is OPEN and carries a breath answer.
    let open = kernel::perform_song(
        &source_bridge::source_phrased(seed, Phrasing::Ensemble),
        &world(),
    )
    .unwrap();
    assert!(
        answers_in(&open, 4.0, 8.0) > 0,
        "precondition: the open predominant bar is answered"
    );
    // Flip ONLY bar 1's harmony to the home tonic (a lawful chord); the lead is untouched.
    let mut resolved = source_bridge::source_phrased(seed, Phrasing::Ensemble);
    resolved.route[1] = (0, Quality::Min);
    let resolved_take = kernel::perform_song(&resolved, &world()).unwrap();
    assert_eq!(
        answers_in(&resolved_take, 4.0, 8.0),
        0,
        "a breath over a home resolution must be left UNANSWERED"
    );
    // The lead is byte-identical: the response moved because the HARMONY moved, not the call's notes.
    assert_eq!(
        lane(open.score(), Role::Lead),
        lane(resolved_take.score(), Role::Lead),
        "the lead (onsets, pitches, durations, silences) is unchanged between the two arms"
    );
}

// ======================================================================================
// CP3 (keys) — the keyboard comps the holes under the home arrivals.
//
// The per-episode answer alone reads very sparse. Full (an expressive lead) also COMPS — short
// mid-register guide-tone stabs in the rhythmic holes — but only under the hook and the return,
// making room for the lead. The plain control keeps its frozen, sparse keyboard (so the archived
// control stays byte-identical). This test pins all of that.
// ======================================================================================

#[test]
fn the_keyboard_comps_the_holes_under_the_home_arrivals() {
    for seed in [2112u64, 7, 99] {
        let full = render_cfg(&C137Preset::full().resolve(), seed);
        let plain = kernel::perform_song(
            &source_bridge::source_phrased(seed, Phrasing::Plain),
            &world(),
        )
        .unwrap();
        let fk = full.score().role_notes(Role::Keys).count();
        let pk = plain.score().role_notes(Role::Keys).count();
        assert!(
            fk > pk,
            "seed {seed}: the Full keyboard ({fk}) must be fuller than the plain control ({pk})"
        );
        let comps: Vec<_> = full
            .score()
            .role_notes(Role::Keys)
            .filter(|n| n.prov.role_note == "comp")
            .collect();
        assert!(
            !comps.is_empty(),
            "seed {seed}: Full must add keyboard comp"
        );
        let lead: Vec<f64> = full
            .score()
            .role_notes(Role::Lead)
            .map(|n| n.start_beat)
            .collect();
        for c in &comps {
            // make room for the lead's attack
            assert!(
                !lead.iter().any(|&l| (l - c.start_beat).abs() < 0.12),
                "seed {seed}: comp at {:.3} must make room for the lead",
                c.start_beat
            );
            // only under the home arrivals — episode 1 (hook) or 3 (return) of the 16-bar arc; the
            // verse (0) and the bridge (2) keep the keyboard to its lone answer.
            let ep = (c.start_beat / 16.0).floor() as usize;
            assert!(
                ep == 1 || ep == 3,
                "seed {seed}: comp at {:.3} (episode {ep}) must sit under a home arrival, not the verse/bridge",
                c.start_beat
            );
        }
    }
}

/// The plain control keeps its frozen sparse keyboard — the comp is an EXPRESSIVE-lead capability, so
/// `--jerry=plain` adds no comp (and stays byte-identical, pinned by D3).
#[test]
fn the_plain_control_keeps_its_sparse_keyboard() {
    for seed in [2112u64, 7, 99] {
        let plain = kernel::perform_song(
            &source_bridge::source_phrased(seed, Phrasing::Plain),
            &world(),
        )
        .unwrap();
        assert_eq!(
            plain
                .score()
                .role_notes(Role::Keys)
                .filter(|n| n.prov.role_note == "comp")
                .count(),
            0,
            "seed {seed}: the plain control must add NO keyboard comp"
        );
    }
}

/// The comp plays like a pianist: ACTUAL CHORDS (a rootless voicing sounded polyphonically, more than
/// one keyboard note on the same beat — not a single stab), and whatever COLOUR it adds on top fits the
/// song's key (every non-chord-tone it introduces stays inside the mode). The falsifier for the colour
/// law is a blind extension: a 9th added without the in-scale guard would, on the `v` chord, sound an
/// F# the A-Aeolian key does not hold, and this test would catch it.
#[test]
fn the_keyboard_comps_with_real_chord_voicings() {
    let scale = Scale::new(9, Mode::Aeolian);
    for seed in [2112u64, 7, 99] {
        let full = render_cfg(&C137Preset::full().resolve(), seed);
        let comps: Vec<_> = full
            .score()
            .role_notes(Role::Keys)
            .filter(|n| n.prov.role_note == "comp")
            .collect();
        assert!(
            !comps.is_empty(),
            "seed {seed}: Full must add keyboard comp"
        );

        // Actual chords: at least one comp onset sounds two or more keyboard notes together.
        let mut by_onset: std::collections::BTreeMap<u64, usize> =
            std::collections::BTreeMap::new();
        for c in &comps {
            *by_onset.entry(c.start_beat.to_bits()).or_default() += 1;
        }
        assert!(
            by_onset.values().any(|&n| n >= 2),
            "seed {seed}: the comp must voice real chords (>=2 keys notes on one beat), not single stabs"
        );

        // The colour it adds fits the key: every comp note that is NOT a chord tone (the 9th and the
        // grace ornament — `pitched` leaves their `function` unset) stays inside the mode.
        for c in &comps {
            if c.function.is_none() {
                assert!(
                    scale.contains_pc(c.pitch.rem_euclid(12)),
                    "seed {seed}: comp colour/ornament {} at {:.3} leaves the key",
                    c.pitch,
                    c.start_beat
                );
            }
        }
    }
}

// ======================================================================================
// CP3 (bass) — the foundation anticipates the harmonic destination.
//
// Where the archived control's walking approach steps a blind chromatic half-step into the next
// downbeat, the Full foundation leans toward where the music is GOING: into a home (tonic) arrival
// it plays the destination's DOMINANT — the V the ear hears resolving to the I. Same note count and
// register (a redistribution, not more notes); gated so the plain control stays byte-identical.
// ======================================================================================

#[test]
fn the_bass_anticipates_the_home_arrival() {
    const TONIC: i32 = 9; // A — the world's tonic
    let dominant = (TONIC + 7).rem_euclid(12); // E — the V the foundation leans on
    let chromatic = (TONIC - 1).rem_euclid(12); // G# — the archived blind leading tone
    for seed in [2112u64, 7, 99] {
        let full = render_cfg(&C137Preset::full().resolve(), seed);
        let plain = kernel::perform_song(
            &source_bridge::source_phrased(seed, Phrasing::Plain),
            &world(),
        )
        .unwrap();
        let bass = |score: &Score| -> Vec<(f64, i32)> {
            let mut v: Vec<(f64, i32)> = score
                .role_notes(Role::Bass)
                .map(|n| (n.start_beat, n.pitch.rem_euclid(12)))
                .collect();
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            v
        };
        let fb = bass(full.score());
        let pb = bass(plain.score());
        // The bass note nearest a target beat (the pocket leans attacks a hair off the grid).
        let near = |v: &[(f64, i32)], target: f64| -> Option<i32> {
            v.iter()
                .find(|(b, _)| (b - target).abs() < 0.3)
                .map(|(_, pc)| *pc)
        };
        let total_bars = (full.score().total_beats / 4.0).round() as i64;
        let mut anticipations = 0;
        for bar in 1..total_bars {
            // a tonic (home) downbeat arrival — the destination the foundation is leaning toward
            if near(&fb, bar as f64 * 4.0) != Some(TONIC) {
                continue;
            }
            // the previous bar's beat four — present only if that bar WALKED (a home bar lays back on
            // root + fifth and has no beat four; its seam pickup is a separate, unchanged gesture)
            let beat_four = (bar - 1) as f64 * 4.0 + 3.0;
            if let Some(f_pc) = near(&fb, beat_four) {
                assert_eq!(
                    f_pc, dominant,
                    "seed {seed}: Full bass into the tonic at bar {bar} must lean on the dominant (V->I)"
                );
                assert_eq!(
                    near(&pb, beat_four),
                    Some(chromatic),
                    "seed {seed}: the archived control steps a blind chromatic leading tone there, not the V"
                );
                anticipations += 1;
            }
        }
        assert!(
            anticipations > 0,
            "seed {seed}: the foundation must anticipate at least one home arrival with its dominant"
        );
    }
}

// ======================================================================================
// CP3 (drums) — the drummer translates the lead's rhythm.
//
// The phrase-end fill no longer plays a generic rising run: it quotes the melody's own phrase-ending
// figure (the lead's attacks across the first half of the fill bar), echoed a half-bar later so the
// kit drives the band into the downbeat on the lead's rhythm. Reads the LEAD only (the keys-independent
// pulse is untouched); gated so the plain control keeps its archived fill byte-identical.
// ======================================================================================

#[test]
fn the_drums_translate_the_lead_rhythm() {
    for seed in [2112u64, 7, 99] {
        let full = render_cfg(&C137Preset::full().resolve(), seed);
        let plain = kernel::perform_song(
            &source_bridge::source_phrased(seed, Phrasing::Plain),
            &world(),
        )
        .unwrap();
        let lead: Vec<f64> = full
            .score()
            .role_notes(Role::Lead)
            .map(|n| n.start_beat)
            .collect();
        let echoes: Vec<f64> = full
            .score()
            .drums
            .iter()
            .filter(|d| d.prov.role_note == "lead-echo")
            .map(|d| d.start_beat)
            .collect();
        assert!(
            !echoes.is_empty(),
            "seed {seed}: the Full drums must translate the lead's rhythm (a lead-echo fill)"
        );
        // Each echo stroke QUOTES a lead attack, shifted exactly a half-bar later — it is the melody's
        // own rhythm, not a generic run (falsifier: a grid fill would not line up with a lead attack).
        for &t in &echoes {
            assert!(
                lead.iter().any(|&o| (o - (t - 2.0)).abs() < 0.01),
                "seed {seed}: lead-echo at {t:.3} must quote the lead attack at {:.3}",
                t - 2.0
            );
        }
        // The plain control keeps its archived fill — no translation.
        assert!(
            plain
                .score()
                .drums
                .iter()
                .all(|d| d.prov.role_note != "lead-echo"),
            "seed {seed}: the plain control must keep the archived fill (no lead-echo)"
        );
    }
}

// ================================================================================================
// FULL SONG FORM (--form=song): the zoom-out from the 16-bar vertical slice to an eight-section pop
// skeleton — VERSE/CHORUS/VERSE/CHORUS/BRIDGE/CHORUS×3 — in ONE continuous 128-beat Score. These pin
// that named sections are wired into the categorical machinery (the band reads `relation`, never a
// bar-range label): the chorus hook RECURS verbatim (a refrain, via Consequent/Return to the one hook
// referent), verse/chorus dynamic contrast is relation-gated (keys comp only on choruses), and the
// post-bridge choruses EARN their return (the keys carry a consequence of the specific bridge).
// ================================================================================================

fn song_take(seed: u64) -> CommittedSong {
    kernel::perform_song(&source_song::source_song(seed), &world()).unwrap()
}

#[test]
fn the_song_form_states_eight_typed_sections() {
    let (argument, _song) = source_song::argument_song(2112);
    assert_eq!(
        argument.referents.len(),
        3,
        "the SAME verse + hook + bridge"
    );
    assert_eq!(argument.steps.len(), 8, "eight typed sections");
    // VERSE Establish, CHORUS Consequent, VERSE Establish, CHORUS Consequent, BRIDGE Depart, then
    // three post-bridge CHORUS Returns — the pop arc, declared as relations not bar ranges.
    use ArgumentRelation::{Consequent, Depart, Establish, Return};
    assert!(matches!(argument.steps[0].relation, Establish));
    assert!(matches!(argument.steps[1].relation, Consequent { .. }));
    assert!(matches!(argument.steps[2].relation, Establish));
    assert!(matches!(argument.steps[3].relation, Consequent { .. }));
    assert!(matches!(argument.steps[4].relation, Depart { .. }));
    assert!(matches!(argument.steps[5].relation, Return { .. }));
    assert!(matches!(argument.steps[6].relation, Return { .. }));
    assert!(matches!(argument.steps[7].relation, Return { .. }));
    // Every chorus restates the ONE hook referent; both verses restate the ONE verse referent.
    for chorus in [1usize, 3, 5, 6, 7] {
        assert_eq!(
            argument.steps[chorus].referent,
            ReferentId(1),
            "chorus step {chorus} must restate the hook referent"
        );
    }
    assert_eq!(argument.steps[0].referent, ReferentId(0));
    assert_eq!(argument.steps[2].referent, ReferentId(0));
    // The post-bridge choruses depend on BOTH the hook they restate and the bridge they return FROM.
    for ret in [5usize, 6, 7] {
        assert!(
            argument.steps[ret]
                .depends_on
                .contains(&argument.steps[4].id),
            "post-bridge chorus {ret} must depend on the bridge (the earned return)"
        );
    }
    assert_eq!(
        argument.validate(),
        Ok(()),
        "the eight-section arc is lawful"
    );
}

#[test]
fn the_song_form_performs_and_passes_the_witness() {
    for seed in [2112u64, 7, 99] {
        let take = song_take(seed);
        let s = take.score();
        assert_eq!(s.total_beats, 128.0, "seed {seed}: eight 16-beat episodes");
        assert!(s.mono_voice);
        // The lead sings in every one of the eight episodes — no dead sections.
        for e in 0..8 {
            let (lo, hi) = (e as f64 * 16.0, e as f64 * 16.0 + 16.0);
            assert!(
                s.role_notes(Role::Lead)
                    .any(|n| n.start_beat >= lo && n.start_beat < hi),
                "seed {seed}: the lead is silent in episode {e} [{lo},{hi})"
            );
        }
        // The content witness accepts the realized full-song arc (source identity, dependency order,
        // the consequent/departure/return laws all observed in the RAW Score).
        assert!(
            gibson::audio::human_music::argument_witness::observe_argument(
                &source_song::source_song(seed),
                s,
                world().tonic_pc
            )
            .valid(),
            "seed {seed}: the content witness refused the realized song"
        );
        take.validate_candidate(s).unwrap();
    }
}

#[test]
fn the_chorus_hook_recurs_verbatim_as_a_refrain() {
    let take = song_take(2112);
    let s = take.score();
    // A refrain: the chorus hook melody is the IDENTICAL tune at every chorus pillar (ep1/3/5/6/7).
    let chorus1 = pitches(s, Role::Lead, 16.0, 32.0);
    assert!(!chorus1.is_empty(), "the first chorus must sing");
    for ep in [3usize, 5, 6, 7] {
        let (lo, hi) = (ep as f64 * 16.0, ep as f64 * 16.0 + 16.0);
        assert_eq!(
            pitches(s, Role::Lead, lo, hi),
            chorus1,
            "chorus at episode {ep} must restate the hook verbatim (a refrain)"
        );
    }
    // The verse recurs too (ep0 == ep2), and is a DIFFERENT tune from the chorus.
    let verse1 = pitches(s, Role::Lead, 0.0, 16.0);
    assert_eq!(
        pitches(s, Role::Lead, 32.0, 48.0),
        verse1,
        "verse 2 must restate verse 1 verbatim"
    );
    assert_ne!(
        verse1, chorus1,
        "the verse and chorus are distinct material"
    );
    // The bridge is distinct from both (a genuine departure).
    let bridge = pitches(s, Role::Lead, 64.0, 80.0);
    assert_ne!(bridge, chorus1, "the bridge departs from the hook");
    assert_ne!(bridge, verse1, "the bridge departs from the verse");
}

#[test]
fn the_song_choruses_are_fuller_than_the_verses() {
    // Verse/chorus dynamic contrast is RELATION-gated, not bar-position gated: the keyboard comp
    // (pianist chord voicings) fires on the choruses (Consequent|Return) and is absent on the verses
    // (Establish). This is the "wired into the machinery" claim — names map to behavior. The comp is
    // an expressive-arm capability (`comp_keys`), so this exercises the Ensemble song the Full render
    // uses, not the plain default.
    let take = kernel::perform_song(
        &source_song::source_song_phrased(2112, Phrasing::Ensemble),
        &world(),
    )
    .unwrap();
    let comps = |lo: f64, hi: f64| {
        take.score()
            .role_notes(Role::Keys)
            .filter(|n| n.prov.role_note == "comp" && n.start_beat >= lo && n.start_beat < hi)
            .count()
    };
    for verse in [0usize, 2] {
        let (lo, hi) = (verse as f64 * 16.0, verse as f64 * 16.0 + 16.0);
        assert_eq!(comps(lo, hi), 0, "verse episode {verse} must not comp");
    }
    for chorus in [1usize, 3, 5, 6, 7] {
        let (lo, hi) = (chorus as f64 * 16.0, chorus as f64 * 16.0 + 16.0);
        assert!(
            comps(lo, hi) > 0,
            "chorus episode {chorus} must comp (fuller)"
        );
    }
    // The bridge (Depart) does not comp either — it departs on the thin texture.
    assert_eq!(comps(64.0, 80.0), 0, "the bridge must not comp");
}

#[test]
fn the_song_earns_its_return_across_the_whole_form() {
    // The earned-return counterfactual, pulled across the full song: two lawful bridges (only the
    // bridge MOTIF differs) must leave everything BEFORE the bridge identical — including the
    // pre-bridge choruses — and change only the POST-bridge choruses' accompaniment, while the
    // returning hook MELODY stays the exact same tune. The journey is remembered; the headline is not.
    let seed = 2112u64;
    let b1 = source_song::source_song_with_bridge_seed_phrased(seed, seed, Phrasing::Plain);
    let b2 = source_song::source_song_with_bridge_seed_phrased(seed, 9001, Phrasing::Plain);
    let t1 = kernel::perform_song(&b1, &world()).unwrap();
    let t2 = kernel::perform_song(&b2, &world()).unwrap();
    let (s1, s2) = (t1.score(), t2.score());

    // The counterfactual is real: the two bridge motifs genuinely differ (ep4 = [64,80)).
    assert_ne!(
        pitches(s1, Role::Lead, 64.0, 80.0),
        pitches(s2, Role::Lead, 64.0, 80.0),
        "the two bridge motifs must differ for this to be a counterfactual"
    );
    // Everything before the bridge is untouched by the bridge seed — both verses and both pre-bridge
    // choruses (ep0..ep4 = [0,64)) are identical, so any later difference is the bridge's alone.
    assert_eq!(
        pitches(s1, Role::Lead, 0.0, 64.0),
        pitches(s2, Role::Lead, 0.0, 64.0),
        "verses and pre-bridge choruses must not depend on the bridge seed"
    );
    assert_eq!(
        keys_in(&t1, 16.0, 32.0),
        keys_in(&t2, 16.0, 32.0),
        "the PRE-bridge chorus accompaniment cannot depend on a bridge not yet travelled"
    );
    // The returning hook MELODY is the exact same tune under both bridges (every post-bridge chorus).
    for ep in [5usize, 6, 7] {
        let (lo, hi) = (ep as f64 * 16.0, ep as f64 * 16.0 + 16.0);
        assert_eq!(
            pitches(s1, Role::Lead, lo, hi),
            pitches(s2, Role::Lead, lo, hi),
            "the return LEAD at episode {ep} must be the identical hook under both bridges"
        );
    }
    // THE EARNED RETURN: the post-bridge chorus KEYS differ between the two journeys — a witnessed,
    // Score-observable consequence of WHICH bridge was travelled.
    assert_ne!(
        keys_in(&t1, 80.0, 96.0),
        keys_in(&t2, 80.0, 96.0),
        "the post-bridge chorus accompaniment must depend on the SPECIFIC bridge"
    );
}

#[test]
fn the_song_form_is_opt_in_and_the_slice_stays_frozen() {
    // The song form is a separate opt-in constructor. The 16-bar vertical slice and the 8-bar control
    // are produced by their own untouched sources — performing them here re-confirms the song work
    // could not have perturbed them (their pinned fingerprints live in the canonical-fingerprint and
    // D3 suites; here we assert the lengths are what the default/short arcs are, 64 and 32 beats).
    assert_eq!(
        song_take(2112).score().total_beats,
        128.0,
        "song is 32 bars"
    );
    assert_eq!(
        kernel::perform_song(&source_bridge::source(2112), &world())
            .unwrap()
            .score()
            .total_beats,
        64.0,
        "the 16-bar slice is unchanged at 64 beats"
    );
    assert_eq!(
        kernel::perform_song(&source::source(2112), &world())
            .unwrap()
            .score()
            .total_beats,
        32.0,
        "the 8-bar control is unchanged at 32 beats"
    );
}

// ================================================================================================
// FULL SONG FORM WITH A FRONT INTRO (--form=song-intro): the honest intro the machinery admits —
// "establish the pocket, earn the entrance". Bass + drums lay the shared pocket while the keys are
// SILENT and the lead is still absent; a thin lead then enters LATE (a pickup that QUOTES the verse
// head's contour), and the first verse states the head in full. The arc opens on an `Establish` of a
// sparse intro referent and drops one trailing chorus to keep the 128-beat budget:
// INTRO/VERSE/CHORUS/VERSE/CHORUS/BRIDGE/CHORUS/CHORUS. A real intro is a NEW ARC SPEC (zero kernel
// change). A truly SILENT lead is NOT admissible — every long-form placement is hard-required to carry
// the lead — so the intro lead is thin and late, never absent, and these pin exactly that.
// ================================================================================================

fn intro_song_take(seed: u64) -> CommittedSong {
    kernel::perform_song(&source_song::source_song_intro(seed), &world()).unwrap()
}

#[test]
fn the_intro_song_opens_with_an_intro_section() {
    let (argument, _song) = source_song::argument_song_intro(2112);
    assert_eq!(
        argument.referents.len(),
        4,
        "verse + hook + bridge + the new sparse intro referent"
    );
    assert_eq!(
        argument.steps.len(),
        8,
        "eight typed sections (the intro replaces one trailing chorus)"
    );
    use ArgumentRelation::{Consequent, Depart, Establish, Return};
    // INTRO Establish (its own referent), VERSE Establish, CHORUS Consequent, VERSE Establish,
    // CHORUS Consequent, BRIDGE Depart, then TWO post-bridge CHORUS Returns — declared as relations.
    assert!(matches!(argument.steps[0].relation, Establish));
    assert_eq!(
        argument.steps[0].referent,
        ReferentId(3),
        "the intro states its own sparse referent, not the verse's"
    );
    assert!(matches!(argument.steps[1].relation, Establish));
    assert_eq!(
        argument.steps[1].referent,
        ReferentId(0),
        "ep1 is the verse"
    );
    assert!(
        argument.steps[1].depends_on.contains(&argument.steps[0].id),
        "the verse depends on the intro it grows out of (a provenance edge the witness orders)"
    );
    assert!(matches!(argument.steps[2].relation, Consequent { .. }));
    assert!(matches!(argument.steps[3].relation, Establish));
    assert!(matches!(argument.steps[4].relation, Consequent { .. }));
    assert!(matches!(argument.steps[5].relation, Depart { .. }));
    assert!(matches!(argument.steps[6].relation, Return { .. }));
    assert!(matches!(argument.steps[7].relation, Return { .. }));
    for chorus in [2usize, 4, 6, 7] {
        assert_eq!(
            argument.steps[chorus].referent,
            ReferentId(1),
            "chorus step {chorus} must restate the hook referent"
        );
    }
    for ret in [6usize, 7] {
        assert!(
            argument.steps[ret]
                .depends_on
                .contains(&argument.steps[5].id),
            "post-bridge chorus {ret} must depend on the bridge (the earned return)"
        );
    }
    assert_eq!(argument.validate(), Ok(()), "the intro arc is lawful");
}

#[test]
fn the_intro_establishes_the_pocket_before_the_lead_enters() {
    for seed in [2112u64, 7, 99] {
        let take = intro_song_take(seed);
        let s = take.score();
        // The intro's keys are SILENT — no keys note sounds in episode 0 [0,16).
        assert_eq!(
            s.role_notes(Role::Keys)
                .filter(|n| n.start_beat < 16.0)
                .count(),
            0,
            "seed {seed}: the intro keys must be silent (the pocket carries it, not the comp)"
        );
        // Bass AND drums lay the pocket in the intro — the band plays; the lead simply hasn't entered.
        assert!(
            s.role_notes(Role::Bass).any(|n| n.start_beat < 16.0),
            "seed {seed}: the intro bass must lay the pocket"
        );
        assert!(
            s.drums.iter().any(|d| d.start_beat < 16.0),
            "seed {seed}: the intro drums must lay the pocket"
        );
        // The lead enters LATE and THIN: a handful of notes, the first well into the intro — a pickup,
        // never a full statement. (It cannot be fully silent: the long-form lead carrier is required.)
        let mut lead0: Vec<f64> = s
            .role_notes(Role::Lead)
            .map(|n| n.start_beat)
            .filter(|b| *b < 16.0)
            .collect();
        lead0.sort_by(f64::total_cmp);
        assert!(
            !lead0.is_empty(),
            "seed {seed}: the lead still carries the intro (a long-form placement cannot be silent)"
        );
        assert!(
            lead0[0] >= 8.0,
            "seed {seed}: the lead enters late — first onset {} should be >= 8.0 beats in",
            lead0[0]
        );
        assert!(
            lead0.len() <= 6,
            "seed {seed}: the intro lead is a thin pickup, got {} notes",
            lead0.len()
        );
        // The verse that follows is a FULL statement: many more lead notes than the thin intro pickup.
        let verse_n = s
            .role_notes(Role::Lead)
            .filter(|n| n.start_beat >= 16.0 && n.start_beat < 32.0)
            .count();
        assert!(
            verse_n > lead0.len() * 2,
            "seed {seed}: the verse enters in full ({verse_n}) vs the thin intro ({})",
            lead0.len()
        );
    }
}

#[test]
fn the_intro_foreshadows_the_verse_head() {
    // The intro lead QUOTES the verse head's CONTOUR: the intervals between the intro's notes match the
    // intervals between the verse's first notes. The intro cell is the verse's own first steps,
    // re-anchored, so the contour is shared by construction and survives transport into the intro's
    // harmonic context (the absolute pitches may differ; the shape does not). A pre-echo of the head.
    for seed in [2112u64, 7, 99] {
        let s = intro_song_take(seed);
        let s = s.score();
        let intro = pitches(s, Role::Lead, 0.0, 16.0);
        let verse = pitches(s, Role::Lead, 16.0, 32.0);
        assert!(
            intro.len() >= 3 && verse.len() >= intro.len(),
            "seed {seed}: need an intro cell and a verse head to compare"
        );
        let deltas = |v: &[i32]| v.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
        assert_eq!(
            deltas(&intro),
            deltas(&verse[..intro.len()]),
            "seed {seed}: the intro must quote the verse head's contour (intro {intro:?} vs head {:?})",
            &verse[..intro.len()]
        );
    }
}

#[test]
fn the_intro_song_performs_and_passes_the_witness() {
    for seed in [2112u64, 7, 99] {
        let take = intro_song_take(seed);
        let s = take.score();
        assert_eq!(s.total_beats, 128.0, "seed {seed}: eight 16-beat episodes");
        assert!(s.mono_voice);
        // The lead sings in every one of the eight episodes — including the thin intro (no dead section).
        for e in 0..8 {
            let (lo, hi) = (e as f64 * 16.0, e as f64 * 16.0 + 16.0);
            assert!(
                s.role_notes(Role::Lead)
                    .any(|n| n.start_beat >= lo && n.start_beat < hi),
                "seed {seed}: the lead is silent in episode {e} [{lo},{hi})"
            );
        }
        // The content witness accepts the realized intro arc (source identity, the verse's dependency
        // on the intro ordered correctly, the consequent/departure/return laws all observed).
        assert!(
            gibson::audio::human_music::argument_witness::observe_argument(
                &source_song::source_song_intro(seed),
                s,
                world().tonic_pc
            )
            .valid(),
            "seed {seed}: the content witness refused the realized intro song"
        );
        take.validate_candidate(s).unwrap();
    }
}

#[test]
fn the_intro_song_earns_its_return() {
    // The earned-return counterfactual on the intro arc: the bridge is ep5 [80,96), the post-bridge
    // choruses are ep6/ep7 [96,128). Two lawful bridges (only the MOTIF differs) must leave everything
    // BEFORE the bridge identical and change only the post-bridge accompaniment, while the returning
    // hook MELODY stays the exact same tune. The intro front matter is part of "before the bridge".
    let seed = 2112u64;
    let b1 = source_song::source_song_intro_with_bridge_seed_phrased(seed, seed, Phrasing::Plain);
    let b2 = source_song::source_song_intro_with_bridge_seed_phrased(seed, 9001, Phrasing::Plain);
    let t1 = kernel::perform_song(&b1, &world()).unwrap();
    let t2 = kernel::perform_song(&b2, &world()).unwrap();
    let (s1, s2) = (t1.score(), t2.score());

    // The counterfactual is real: the two bridge motifs genuinely differ (ep5 = [80,96)).
    assert_ne!(
        pitches(s1, Role::Lead, 80.0, 96.0),
        pitches(s2, Role::Lead, 80.0, 96.0),
        "the two bridge motifs must differ for this to be a counterfactual"
    );
    // Everything before the bridge — the intro, both verses and both pre-bridge choruses (ep0..ep5 =
    // [0,80)) — is identical, so any later difference is the bridge's alone.
    assert_eq!(
        pitches(s1, Role::Lead, 0.0, 80.0),
        pitches(s2, Role::Lead, 0.0, 80.0),
        "the intro, verses and pre-bridge choruses must not depend on the bridge seed"
    );
    // The returning hook MELODY is the exact same tune under both bridges (both post-bridge choruses).
    for ep in [6usize, 7] {
        let (lo, hi) = (ep as f64 * 16.0, ep as f64 * 16.0 + 16.0);
        assert_eq!(
            pitches(s1, Role::Lead, lo, hi),
            pitches(s2, Role::Lead, lo, hi),
            "the return LEAD at episode {ep} must be the identical hook under both bridges"
        );
    }
    // THE EARNED RETURN: the post-bridge chorus KEYS differ between the two journeys — a witnessed,
    // Score-observable consequence of WHICH bridge was travelled.
    assert_ne!(
        keys_in(&t1, 96.0, 112.0),
        keys_in(&t2, 96.0, 112.0),
        "the post-bridge chorus accompaniment must depend on the SPECIFIC bridge"
    );
}

#[test]
fn the_intro_builds_instead_of_entering_full_blast() {
    // The intro must ENTER REDUCED and swell into the first verse — not slam in at full density under a
    // thin lead. With the build on (expressive/ensemble), the opening bar is thinner AND softer than
    // the bar that hands off to the verse, and the backbeat is laid OUT of bar 0 (kick + hats only).
    // Keyed to episode 0 alone — the gate below proves the no-intro song does NOT thin its opening.
    let take = kernel::perform_song(
        &source_song::source_song_intro_phrased(2112, Phrasing::Ensemble),
        &world(),
    )
    .unwrap();
    let s = take.score();
    let drums_in = |lo: f64, hi: f64| {
        s.drums
            .iter()
            .filter(|d| d.start_beat >= lo && d.start_beat < hi)
            .count()
    };
    let vel_mean = |lo: f64, hi: f64| {
        let v: Vec<f32> = s
            .drums
            .iter()
            .filter(|d| d.start_beat >= lo && d.start_beat < hi)
            .map(|d| d.velocity)
            .collect();
        v.iter().sum::<f32>() / v.len().max(1) as f32
    };
    // Density builds: the opening bar is thinner than the hand-off bar (bar 3).
    assert!(
        drums_in(0.0, 4.0) < drums_in(12.0, 16.0),
        "intro bar 0 ({}) must be thinner than the hand-off bar 3 ({})",
        drums_in(0.0, 4.0),
        drums_in(12.0, 16.0)
    );
    // Crescendo: the opening bar is softer than the hand-off bar.
    assert!(
        vel_mean(0.0, 4.0) < vel_mean(12.0, 16.0),
        "the intro must crescendo: bar 0 {:.3} < bar 3 {:.3}",
        vel_mean(0.0, 4.0),
        vel_mean(12.0, 16.0)
    );
    // The signature thinning: NO backbeat snare in bar 0 (kick + hats only — the kit opens bare).
    let backbeat = |score: &Score, lo: f64, hi: f64| {
        score.drums.iter().any(|d| {
            matches!(d.voice, DrumVoice::Snare)
                && d.prov.role_note == "pulse"
                && d.start_beat >= lo + 0.5
                && d.start_beat < hi
        })
    };
    assert!(
        !backbeat(s, 0.0, 4.0),
        "the intro's first bar must lay the backbeat out (kick + hats only)"
    );
    // NOT a uniform fader: the SKELETON (kick) holds a steady pocket from the top while the TEXTURE
    // (hats) washes in — so the opening-bar kick is a FAR larger fraction of its hand-off level than
    // the opening hats are of theirs. This decoupling is what reads as a band leaning in, not a knob.
    let kick_mean = |lo: f64, hi: f64| {
        let v: Vec<f32> = s
            .drums
            .iter()
            .filter(|d| {
                matches!(d.voice, DrumVoice::Kick) && d.start_beat >= lo && d.start_beat < hi
            })
            .map(|d| d.velocity)
            .collect();
        v.iter().sum::<f32>() / v.len().max(1) as f32
    };
    let hat_mean = |lo: f64, hi: f64| {
        let v: Vec<f32> = s
            .drums
            .iter()
            .filter(|d| {
                matches!(d.voice, DrumVoice::ClosedHat | DrumVoice::OpenHat)
                    && d.start_beat >= lo
                    && d.start_beat < hi
            })
            .map(|d| d.velocity)
            .collect();
        v.iter().sum::<f32>() / v.len().max(1) as f32
    };
    let kick_ratio = kick_mean(0.0, 4.0) / kick_mean(12.0, 16.0);
    let hat_ratio = hat_mean(0.0, 4.0) / hat_mean(12.0, 16.0);
    assert!(
        kick_ratio > hat_ratio + 0.2,
        "the skeleton must hold while the texture swells: kick bar0/bar3 {kick_ratio:.2} should exceed hats {hat_ratio:.2} (a uniform fader would make them equal)"
    );
    // THE GATE: the no-intro song does NOT build — its opening bar keeps its backbeat, proving the
    // build is keyed to the intro section alone and never leaks into an ordinary opening.
    let no_intro = kernel::perform_song(
        &source_song::source_song_phrased(2112, Phrasing::Ensemble),
        &world(),
    )
    .unwrap();
    assert!(
        backbeat(no_intro.score(), 0.0, 4.0),
        "the no-intro song must keep its opening backbeat (the build is intro-only)"
    );
}
