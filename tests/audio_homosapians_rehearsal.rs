//! OPERATION THE HOMOSAPIANS — falsification gates for the REHEARSAL COMPOSER.
//!
//! These are the executable evidence for the rehearsal process (brief §10). Unlike the aesthetic
//! A/Bs, they need no human-quality inference: they prove the seed chooses the material, that each
//! musician's choice is a traceable function of what they heard, that the guitarist revises after the
//! band plays, that rejected ideas and committed takes stay clean, and that the frozen Score carries
//! LITERALLY ZERO Lead notes with real chordal support. They do NOT prove the riff is catchy — the
//! owner's ear owns that.
//!
//! Companion to `audio_homosapians_rehearsal_gap.rs` (which pins the OLD fixed-authoring path as the
//! "before" control); this pins the NEW generative path.

#[path = "../examples/c137_song/rehearsal.rs"]
#[allow(dead_code)]
mod rehearsal;

use gibson::audio::human_music::{
    argument::BassDegree,
    kernel,
    material::MaterialEvent,
    score::Role,
    theory::{Mode, Quality},
    MusicWorld,
};
use rehearsal::{
    bass_for_riff, drums_for_bass, fingerprint, freeze, generate_riffs, guitar_proposals,
    interpret_harmony, rehearse, revision_is_lawful, syncopation_score, RevKind,
};

fn world() -> MusicWorld {
    let mut w = MusicWorld::black_ice();
    w.tonic_pc = 9;
    w.mode = Mode::Aeolian;
    w.use_sevenths = true;
    w.allow_modal_mixture = true;
    w.tempo_bpm = 104.0;
    w
}

/// A hand-made monophonic line from `(onset, step)` rows, strictly increasing — a test fixture riff.
fn line(rows: &[(f64, i32)]) -> Vec<MaterialEvent> {
    rows.iter()
        .map(|&(onset, step)| MaterialEvent {
            onset,
            dur: 0.4,
            accent: 0.55,
            step: Some(step),
        })
        .collect()
}

/// The onsets of a riff candidate (for feeding the syncopation metric).
fn onsets(c: &rehearsal::RiffCandidate) -> Vec<f64> {
    c.events.iter().map(|e| e.onset).collect()
}

// ------------------------------------------------------------------------------------------------
// Determinism — the whole process is reproducible from the seed.
// ------------------------------------------------------------------------------------------------
#[test]
fn rehearsal_is_deterministic_in_the_seed() {
    let a = rehearse(2112);
    let b = rehearse(2112);
    assert_eq!(
        fingerprint(&a.riff().events),
        fingerprint(&b.riff().events),
        "same seed must give the same negotiated riff"
    );
    assert_eq!(a.bass_verse, b.bass_verse, "same seed, same bass");
    assert_eq!(a.drums, b.drums, "same seed, same drums");
    assert_eq!(a.revision, b.revision, "same seed, same revision count");
}

// ------------------------------------------------------------------------------------------------
// G0 / falsifier #1 — changing the seed generates a different original riff (not fixed material).
// ------------------------------------------------------------------------------------------------
#[test]
fn g0_the_seed_chooses_the_backing_material() {
    let a = rehearse(2112);
    let b = rehearse(7);
    assert_ne!(
        fingerprint(&a.riff().events),
        fingerprint(&b.riff().events),
        "different seeds must negotiate different riffs (the OLD path gave identical material)"
    );
    // And the harmony the band camps in is riff-derived, and the bass follows the riff, so the seed
    // reaches the route and the groove too (not just a metadata jiggle).
    assert!(
        a.riff().camp != b.riff().camp || a.bass_verse.cells != b.bass_verse.cells,
        "seed should be able to move the camp and/or the bass, not only the melody"
    );
}

// ------------------------------------------------------------------------------------------------
// G9 / falsifier #10 — held-out seeds give MULTIPLE distinct riff identities (no stock clone).
// ------------------------------------------------------------------------------------------------
#[test]
fn g9_held_out_seeds_are_not_one_stock_riff() {
    let seeds = [1u64, 2, 3, 7, 13, 42, 99, 777, 2112, 31337];
    let fps: Vec<u64> = seeds
        .iter()
        .map(|&s| fingerprint(&rehearse(s).riff().events))
        .collect();
    let mut uniq = fps.clone();
    uniq.sort_unstable();
    uniq.dedup();
    assert!(
        uniq.len() >= seeds.len() / 2,
        "expected many distinct riffs across held-out seeds, got {} of {}",
        uniq.len(),
        seeds.len()
    );
}

#[test]
fn generate_riffs_yields_several_distinct_candidates() {
    let riffs = generate_riffs(2112, 5);
    assert!(
        riffs.len() >= 4,
        "expected ~5 candidates, got {}",
        riffs.len()
    );
    let mut ids: Vec<u64> = riffs.iter().map(|r| r.source_id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        ids.len(),
        riffs.len(),
        "candidate fingerprints must be distinct"
    );
    for r in &riffs {
        assert!(
            (3..=64).contains(&r.events.len()),
            "a riff must be a lawful referent length"
        );
        assert!(
            revision_is_lawful(&r.events),
            "a generated riff must be lawful"
        );
    }
}

// ------------------------------------------------------------------------------------------------
// G1 / falsifier #2 — the bassist reads THAT riff's rhythm, not just the chord. Same count + camp,
// different placement => different bass. A "deaf" fixed bass is identical for both (the control).
// ------------------------------------------------------------------------------------------------
#[test]
fn g1_bassist_responds_to_the_riff_not_just_the_chord() {
    let template = generate_riffs(2112, 5).into_iter().next().unwrap();

    // Riff A: six notes CLUSTERED in bar 0 (dense bar 0, empty bar 1).
    let mut riff_a = template.clone();
    riff_a.events = line(&[(0.0, 7), (0.5, 8), (1.0, 10), (1.5, 8), (2.0, 7), (2.5, 5)]);
    // Riff B: the SAME six notes SPREAD across both bars (different rhythmic identity, same count/camp).
    let mut riff_b = template.clone();
    riff_b.events = line(&[(0.0, 7), (1.5, 8), (3.0, 10), (4.0, 8), (5.5, 7), (7.0, 5)]);

    assert_eq!(riff_a.events.len(), riff_b.events.len(), "same note count");
    assert_eq!(riff_a.camp, riff_b.camp, "same harmonic camp");

    let bass_a = bass_for_riff(&riff_a, false);
    let bass_b = bass_for_riff(&riff_b, false);
    assert_ne!(
        bass_a, bass_b,
        "a bassist who is LISTENING answers the two riffs differently"
    );

    // The deaf control: a fixed bass that ignores the riff is identical for both — what "not
    // listening" sounds like. (We model it as the same response to a zeroed-out riff.)
    let deaf = |_r: &rehearsal::RiffCandidate| -> Vec<Vec<(f64, BassDegree, f32, f32)>> {
        vec![
            vec![
                (0.0, BassDegree::Root, 0.5, 0.8),
                (2.0, BassDegree::Fifth, 0.5, 0.56)
            ];
            4
        ]
    };
    assert_eq!(
        deaf(&riff_a),
        deaf(&riff_b),
        "the deaf control does not distinguish the riffs (the negative control)"
    );
}

// ------------------------------------------------------------------------------------------------
// G2 / falsifier #3 — the drummer's discretionary choices follow the bass; the spine is preserved.
// ------------------------------------------------------------------------------------------------
#[test]
fn g2_drummer_responds_to_the_bass() {
    let template = generate_riffs(2112, 5).into_iter().next().unwrap();
    let mut dense = template.clone();
    dense.events = line(&[
        (0.0, 7),
        (0.5, 8),
        (1.0, 10),
        (2.0, 8),
        (2.5, 7),
        (4.0, 5),
        (4.5, 7),
        (5.0, 8),
        (6.0, 7),
    ]);
    let mut pushy = template.clone();
    pushy.events = line(&[
        (0.0, 7),
        (2.0, 8),
        (3.75, 10),
        (4.0, 8),
        (6.0, 7),
        (7.75, 5),
    ]);

    let bass_dense = bass_for_riff(&dense, false);
    let bass_pushy = bass_for_riff(&pushy, false);
    assert_ne!(
        bass_dense, bass_pushy,
        "precondition: the two basses differ"
    );

    let drums_dense = drums_for_bass(&bass_dense);
    let drums_pushy = drums_for_bass(&bass_pushy);
    assert_ne!(
        drums_dense, drums_pushy,
        "a drummer who is LISTENING chooses differently for different bass"
    );
    // The spine/backbone is preserved: both decisions span the same number of bars.
    assert_eq!(
        drums_dense.reinforce_push.len(),
        drums_pushy.reinforce_push.len()
    );
    assert_eq!(drums_dense.fill.len(), drums_pushy.fill.len());
    assert!(
        drums_dense.fill.iter().any(|f| *f),
        "a fill always lands somewhere"
    );
}

// ------------------------------------------------------------------------------------------------
// G3 / falsifier #4 — the guitarist revises AFTER hearing the band (the breakthrough back-edge).
// ------------------------------------------------------------------------------------------------
#[test]
fn g3_guitarist_revises_after_hearing_the_whole_band() {
    // Build a real draft, then force a DENSE selected riff that collides with the drummer's fill, and
    // re-negotiate the band around it so the guitarist has something to react to.
    let mut draft = rehearse(2112);
    let sel = draft.selected;
    draft.candidates[sel].events = line(&[
        (0.0, 7),
        (0.5, 8),
        (1.0, 10),
        (1.5, 8),
        (2.0, 7),
        (4.0, 5),
        (5.5, 7),
    ]);
    draft.bass_verse = bass_for_riff(&draft.candidates[sel], false);
    draft.bass_chorus = bass_for_riff(&draft.candidates[sel], true);
    draft.drums = drums_for_bass(&draft.bass_verse);

    let proposals = guitar_proposals(&draft);
    // A Simplify must be proposed (the fill collided with the dense bar) and it must be lawful + shorter.
    let simplify = proposals
        .iter()
        .find(|(k, rev, _)| *k == RevKind::Simplify && rev.is_some());
    let (_, Some(revised), reason) =
        simplify.expect("guitarist proposes a Simplify after hearing the fill")
    else {
        unreachable!()
    };
    assert!(
        revision_is_lawful(revised),
        "the revision stays a lawful referent"
    );
    assert!(
        revised.len() < draft.riff().events.len(),
        "Simplify removes a note to make room for the fill"
    );
    assert!(
        !reason.is_empty(),
        "the revision is logged with a musical reason"
    );

    // The flourish is always offered AND always rejected (None revised).
    assert!(
        proposals
            .iter()
            .any(|(k, rev, _)| *k == RevKind::Flourish && rev.is_none()),
        "the flashy flourish is offered but carries no accepted change"
    );

    // And across real seeds, the back-edge actually FIRES (some draft reaches revision > 0).
    let fired = (0u64..64).any(|s| rehearse(s).revision > 0);
    assert!(
        fired,
        "on real seeds, at least one rehearsal accepts a revision"
    );
}

// ------------------------------------------------------------------------------------------------
// G4 / falsifiers #5,#6 — rejected proposals don't contaminate; committed audition takes are immutable.
// ------------------------------------------------------------------------------------------------
#[test]
fn g4_rejected_proposals_and_committed_takes_stay_clean() {
    // Find a seed whose rehearsal accepted at least one revision (so the draft actually moved).
    let seed = (0u64..64)
        .find(|&s| rehearse(s).revision > 0)
        .expect("some seed accepts a revision");
    let draft = rehearse(seed);

    // The opening audition take (take 0) still holds the ORIGINAL riff identity, even though the live
    // riff has since been revised — rehearsal time moved without editing the committed trial.
    let original = &generate_riffs(seed, 5)[draft.selected];
    assert_eq!(
        draft.takes[0].fingerprint,
        fingerprint(&original.events),
        "the first audition take is frozen at the pre-revision identity"
    );
    assert_ne!(
        draft.takes[0].fingerprint,
        fingerprint(&draft.riff().events),
        "the live riff has moved on from take 0 (revision really happened)"
    );
    assert_eq!(draft.takes[0].revision, 0, "take 0 is revision 0");

    // No accepted revision is a Flourish — the rejected flashy idea never entered the draft.
    assert!(
        !draft
            .log
            .iter()
            .any(|r| r.accepted && r.kind == RevKind::Flourish),
        "a rejected flourish must never become an accepted change"
    );
    assert!(
        draft.log.iter().any(|r| !r.accepted),
        "at least one proposal was rejected (the flourish)"
    );
}

// ------------------------------------------------------------------------------------------------
// G5 / falsifier #7 — the frozen Score is LITERALLY Lead-empty, with real chordal support.
// ------------------------------------------------------------------------------------------------
#[test]
fn g5_frozen_backing_is_literally_lead_empty_with_chords() {
    let draft = rehearse(2112);
    let source = freeze(&draft);
    let take = kernel::perform_song(&source, &world()).expect("rehearsal-frozen backing is lawful");
    let score = take.score();

    let lead = score.role_notes(Role::Lead).count();
    let keys = score.role_notes(Role::Keys).count();
    let pad = score.role_notes(Role::Pad).count();
    let bass = score.role_notes(Role::Bass).count();

    assert_eq!(
        lead, 0,
        "the frozen backing has ZERO Lead notes (no fake quiet carrier)"
    );
    assert!(keys > 0, "the Keys lane sounds");
    assert!(pad > 0, "the Pad bed supplies the chordal harmony");
    assert!(bass > 0, "the bass plays the negotiated groove");

    // The keys must read as CHORDS, not a single-note line trying to lead (the owner's verdict on the
    // first cut): assert the Keys lane is genuinely polyphonic — at least one onset carries >= 2
    // simultaneous Keys notes (a comp voicing), which only the engine comp provides.
    let mut keys_onsets: Vec<i64> = score
        .role_notes(Role::Keys)
        .map(|n| (n.start_beat * 16.0).round() as i64)
        .collect();
    keys_onsets.sort_unstable();
    let polyphonic = keys_onsets.windows(2).any(|w| w[0] == w[1]);
    assert!(
        polyphonic,
        "the Keys lane must sound real CHORDS (>= 2 simultaneous notes), not a monophonic line"
    );
}

// ------------------------------------------------------------------------------------------------
// G7 / falsifier #10-support — same note count, different rhythm => different identity AND bass.
// ------------------------------------------------------------------------------------------------
#[test]
fn g7_same_count_different_rhythm_changes_identity_and_bass() {
    let template = generate_riffs(777, 5).into_iter().next().unwrap();
    let mut a = template.clone();
    a.events = line(&[(0.0, 7), (1.0, 10), (2.0, 12), (3.0, 10)]);
    let mut b = template.clone();
    b.events = line(&[(0.0, 7), (0.5, 10), (1.5, 12), (2.5, 10)]);
    assert_eq!(a.events.len(), b.events.len(), "same count");
    assert_ne!(
        fingerprint(&a.events),
        fingerprint(&b.events),
        "identity differs with rhythm"
    );
    assert_ne!(
        bass_for_riff(&a, false),
        bass_for_riff(&b, false),
        "the bassist hears the rhythmic difference"
    );
}

// ------------------------------------------------------------------------------------------------
// VISION — the harmony is INTERPRETED from the riff (not hash-picked), the chorus REINTERPRETS the
// verse (where the loop wants to go), and the band contemplates toward the goal of a song.
// ------------------------------------------------------------------------------------------------

/// A hand-made line from `(onset, step, dur)` rows (lets a test weight a pitch class by dwell time).
fn line_dur(rows: &[(f64, i32, f64)]) -> Vec<MaterialEvent> {
    rows.iter()
        .map(|&(onset, step, dur)| MaterialEvent {
            onset,
            dur,
            accent: 0.55,
            step: Some(step),
        })
        .collect()
}

#[test]
fn harmony_is_read_out_of_the_riff_not_defaulted() {
    // A riff that DWELLS on E (pc 7, the 5th) implies a different loop than one dwelling on F (pc 8).
    let dwell_e = line_dur(&[(0.0, 7, 1.5), (2.0, 7, 1.5), (4.0, 12, 1.0), (6.0, 0, 1.0)]);
    let dwell_f = line_dur(&[(0.0, 8, 1.5), (2.0, 8, 1.5), (4.0, 0, 1.0), (6.0, 12, 1.0)]);
    let he = interpret_harmony(&dwell_e);
    let hf = interpret_harmony(&dwell_f);
    assert_ne!(
        he.verse, hf.verse,
        "different emphasised degrees must imply different harmonic loops"
    );
    assert_eq!(
        he.verse[0],
        (0, Quality::Min9),
        "the tonic is always the home pole (the fixed gravity)"
    );
    assert_eq!(
        he.verse[1],
        (7, Quality::Min7),
        "a riff dwelling on E reads as the i-v loop (Am9/Em7)"
    );
    assert_eq!(
        hf.verse[1],
        (8, Quality::Maj9),
        "a riff dwelling on F reads as the i-bVI loop (Am9/Fmaj9)"
    );
}

#[test]
fn chorus_reinterprets_the_verse_not_a_copy() {
    for seed in [2112u64, 7, 42, 1, 99] {
        let d = rehearse(seed);
        let c = &d.riff().camp;
        assert_ne!(
            c.verse, c.chorus,
            "seed {seed}: the chorus must be where the verse WANTS to go, not a copy"
        );
        assert_eq!(
            c.verse[0],
            (0, Quality::Min9),
            "seed {seed}: the verse opens on the tonic home"
        );
    }
}

#[test]
fn the_band_contemplates_toward_a_song() {
    let d = rehearse(2112);
    assert!(
        d.shaping_log.len() >= 7,
        "the shaping log records the move-by-move contemplation, got {}",
        d.shaping_log.len()
    );
    assert!(
        d.shaping_log.iter().any(|l| l.starts_with("CLAY")),
        "it starts from the seed as clay"
    );
    assert!(
        d.shaping_log.iter().any(|l| l.contains("CONTEMPLATE")),
        "the band steps back and evaluates (vision, not throwing at the wall)"
    );
    assert!(
        d.shaping_log.iter().any(|l| l.starts_with("DONE")),
        "the process converges on a song shape"
    );
}

// ------------------------------------------------------------------------------------------------
// SYNCOPATION IS KEY — the band's pick is TASTE, not a blind draw. It reaches for the off-beat life
// and will NOT settle for a straight on-the-grid "chug" when a syncopated idea is on the table.
// ------------------------------------------------------------------------------------------------

#[test]
fn syncopation_score_ranks_offbeat_life_above_the_grid() {
    // A driving on-the-grid chug must score at or below a syncopated funk line and an anticipating
    // pickup line — the metric encodes the ear's verdict (2112-style eighths = bland), not note count.
    let driving = [0.0, 0.5, 1.0, 2.0, 2.5, 4.0, 4.5, 5.0, 6.0];
    let statement = [0.0, 1.0, 2.0, 3.5, 4.0, 6.0];
    let funk = [0.0, 0.75, 1.5, 3.5, 4.0, 4.75, 6.5];
    let pickups = [0.0, 2.0, 3.75, 4.0, 6.0, 7.75];
    assert!(
        syncopation_score(&funk) > syncopation_score(&driving),
        "syncopated funk must out-score the driving grid"
    );
    assert!(
        syncopation_score(&pickups) > syncopation_score(&driving),
        "anticipating pickups must out-score the driving grid"
    );
    assert!(
        syncopation_score(&statement) <= syncopation_score(&driving) + 1.0,
        "a pure on-beat statement is near-zero syncopation"
    );
    // More notes is NOT more syncopation: the 9-note chug scores below the 6-note pickups.
    assert!(
        syncopation_score(&pickups) > syncopation_score(&driving),
        "density is not syncopation"
    );
}

#[test]
fn the_band_reaches_for_syncopation_not_the_grid() {
    // For every seed whose candidate pool contains any syncopated idea, the band's chosen riff is
    // syncopated (score > 0) — it never settles for a pure-grid chug when it had an off-beat option.
    for seed in [1u64, 2, 3, 7, 13, 42, 99, 777, 2112, 31337] {
        let pool = generate_riffs(seed, 5);
        let pool_has_sync = pool.iter().any(|c| syncopation_score(&onsets(c)) > 0.0);
        if !pool_has_sync {
            continue;
        }
        let chosen = syncopation_score(&onsets(rehearse(seed).riff()));
        assert!(
            chosen > 0.0,
            "seed {seed}: the band settled for a grid chug (sync 0) with syncopation available"
        );
    }
}

// ------------------------------------------------------------------------------------------------
// AUDIBLE RIFF — the keys comp stabs its chords on the band's OWN riff rhythm (the hook is heard).
// ------------------------------------------------------------------------------------------------
#[test]
fn keys_comp_stabs_on_the_riff_rhythm() {
    let draft = rehearse(2112);
    let riff_onsets: Vec<f64> = draft.riff().events.iter().map(|e| e.onset).collect();
    let source = freeze(&draft);
    let take = kernel::perform_song(&source, &world()).expect("rehearsal-frozen backing is lawful");
    let score = take.score();
    // Every riff onset should have a Keys chord stab near it (within the pocket tolerance) — the
    // guitarist's figure is HEARD through the keys, not just implied in the harmony.
    for &o in &riff_onsets {
        let hit = score
            .role_notes(Role::Keys)
            .any(|n| (n.start_beat - o).abs() < 0.12);
        assert!(
            hit,
            "the comp must stab a chord on the riff onset at beat {o}"
        );
    }
}
