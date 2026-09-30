//! Source-generation contracts exposed by the consolidated holdout v1 (post-contact falsifiers).
//!
//! Holdout v1 is known data; these tests reproduce each diagnosed mechanism on NEW seeds
//! (77_100_0xx) outside its rows. Each one was committed red before its repair.
use gibson::audio::human_music::{
    composer::Composer,
    contract::{CoherenceAnchor, CompositionGrammar},
    cover::{CoverMap, CoverSpec},
    functor::{perform_with_profile, Composition},
    ids::ActionStamp,
    language::MusicalLanguage,
    occupancy,
    performance::{Admission, PerformanceOptions, REHEARSAL_REJECTION},
    policy::{ActionAdmission, PerformanceProfile},
    score::{DrumVoice, Role},
    semantic::{deflected_lift_trace, demo_trace},
    song::{AnchorPresence, AnchorReport, SongMapConformance},
    witness, MusicWorld, SongMap,
};

fn song(beats: f64, seed: u64, deflected: bool, grammar: CompositionGrammar) -> SongMap {
    let trace = if deflected {
        deflected_lift_trace(beats)
    } else {
        demo_trace(beats)
    };
    SongMap::compose(&trace, seed, Some(grammar), Composer::StructuralR9)
}

fn pocket(song: &SongMap, world: &MusicWorld, language: MusicalLanguage) -> Composition {
    let opts = PerformanceOptions {
        language,
        ..PerformanceOptions::default()
    };
    perform_with_profile(song, world, opts, PerformanceProfile::POCKET).expect("pocket source")
}

/// Every reservation and every final note of a role lies inside `[0, total]`, and the shared
/// occupancy contract (never loosened) accepts the score.
fn assert_domain(c: &Composition, tag: &str) {
    let total = c.perf.total_beats;
    let v = occupancy::violations(&c.perf, &c.score, true);
    assert!(v.is_empty(), "{tag}: {v:?}");
    for role in [Role::Lead, Role::Bass] {
        for owner in c.score.occupancy.iter().filter(|o| o.role == role) {
            for r in &owner.rhythm {
                assert!(
                    r.beat < total - 1e-9 && r.end_beat <= total + 1e-9,
                    "{tag} {role:?} reservation {}..{} outside 0..{total}",
                    r.beat,
                    r.end_beat
                );
            }
        }
        for n in c.score.role_notes(role) {
            let end = n.start_beat + f64::from(n.dur_beats);
            assert!(
                end <= total + 1e-6,
                "{tag} {role:?} note ends {end} past {total}"
            );
        }
    }
}

const WORLDS: fn() -> [MusicWorld; 3] = || {
    [
        MusicWorld::black_ice(),
        MusicWorld::swiss_signal(),
        MusicWorld::vapor95(),
    ]
};

/// U1: a partial final bar. Holdout v1 failed 16/21 partial-final-bar sources (0/9 bar-aligned)
/// with `invalid reservation for Bass`: the bass planner's last-onset gate ran to the bar line,
/// past the requested end, and the final clip trimmed the note but not the reservation read from it.
#[test]
fn u1_partial_final_bar_reservations_share_the_clipped_domain() {
    for world in WORLDS() {
        for (i, beats) in [4.5, 5.25, 7.5, 9.25, 11.75, 13.0, 15.5, 32.5]
            .into_iter()
            .enumerate()
        {
            for deflected in [false, true] {
                for (language, lang) in [
                    (MusicalLanguage::simple(), "simple"),
                    (MusicalLanguage::fusion_conversation(), "fusion"),
                ] {
                    let s = song(
                        beats,
                        77_100_001 + i as u64,
                        deflected,
                        CompositionGrammar::LoopEvolution,
                    );
                    let c = pocket(&s, &world, language);
                    assert_domain(
                        &c,
                        &format!("{} {beats} deflected={deflected} {lang}", world.name),
                    );
                }
            }
        }
    }
}

/// Bar-aligned control: the same law on pieces whose last bar is complete.
#[test]
fn u1_bar_aligned_control_keeps_the_same_law() {
    for world in WORLDS() {
        for beats in [8.0, 16.0, 28.0] {
            let s = song(beats, 77_100_011, true, CompositionGrammar::HookArc);
            assert_domain(
                &pocket(&s, &world, MusicalLanguage::fusion_conversation()),
                &format!("{} {beats}", world.name),
            );
        }
    }
}

/// Pieces shorter than one full bar never outlast themselves either.
#[test]
fn u1_pieces_shorter_than_one_bar() {
    for world in WORLDS() {
        for beats in [2.0, 3.0, 3.75] {
            let s = song(beats, 77_100_012, false, CompositionGrammar::HookArc);
            assert_domain(
                &pocket(&s, &world, MusicalLanguage::fusion_conversation()),
                &format!("{} {beats}", world.name),
            );
        }
    }
}

/// A phrase-expressed (retimed) predecessor reaches the unison reservation rewrite
/// (`AuthoredOccupancy::from_bass` with expression decisions); it too stays inside the piece.
#[test]
fn u1_phrase_retimed_predecessor_stays_in_domain() {
    for beats in [7.5, 9.25, 15.5] {
        let s = song(beats, 77_100_013, true, CompositionGrammar::DeflectedLift);
        let c = perform_with_profile(
            &s,
            &MusicWorld::black_ice(),
            PerformanceOptions::default(),
            PerformanceProfile::PHRASED,
        )
        .expect("phrased source");
        assert_domain(&c, &format!("phrased {beats}"));
    }
}

/// The observer is not loosened: a reservation one epsilon past the end is still rejected, and
/// one ending exactly at the end is lawful.
#[test]
fn u1_the_checker_still_rejects_a_reservation_past_the_end() {
    let s = song(16.0, 77_100_014, false, CompositionGrammar::HookArc);
    let c = pocket(
        &s,
        &MusicWorld::black_ice(),
        MusicalLanguage::fusion_conversation(),
    );
    let total = c.perf.total_beats;
    let mut score = c.score.clone();
    let bass = score
        .occupancy
        .iter_mut()
        .find(|o| o.role == Role::Bass)
        .expect("bass owner");
    let last = bass.rhythm.last_mut().expect("bass sounds");
    last.end_beat = total + 1e-3;
    assert!(occupancy::violations(&c.perf, &score, true)
        .iter()
        .any(|s| s == "invalid reservation for Bass"));
    let bass = score
        .occupancy
        .iter_mut()
        .find(|o| o.role == Role::Bass)
        .unwrap();
    bass.rhythm.last_mut().unwrap().end_beat = total;
    assert!(!occupancy::violations(&c.perf, &score, true)
        .iter()
        .any(|s| s.starts_with("invalid reservation")));
}

/// U2 (Groove): holdout v1 had generated sources whose contract declares Groove as load-bearing but
/// which never sound a kit stroke — every Intro/Coda-only form seats the drums Silent, and the
/// arrangement's coverage guard covered the pitched voices only. A piece with at least one full
/// bar of room has the opportunity to state its groove; it may not silently omit it.
#[test]
fn u2_declared_groove_sounds_when_the_form_has_a_full_bar() {
    for (i, (beats, deflected)) in [(7.5, false), (12.0, false), (16.0, true), (23.75, false)]
        .into_iter()
        .enumerate()
    {
        for grammar in [
            CompositionGrammar::HookArc,
            CompositionGrammar::RiffDrive,
            CompositionGrammar::LoopEvolution,
        ] {
            let s = song(beats, 77_100_021 + i as u64, deflected, grammar);
            assert!(s.plan.contract.anchors.contains(&CoherenceAnchor::Groove));
            for (language, lang) in [
                (MusicalLanguage::simple(), "simple"),
                (MusicalLanguage::fusion_conversation(), "fusion"),
            ] {
                let c = pocket(&s, &MusicWorld::black_ice(), language);
                let pocket_strokes = c
                    .score
                    .drums
                    .iter()
                    .filter(|d| matches!(d.voice, DrumVoice::Kick | DrumVoice::Snare))
                    .count();
                assert!(
                    pocket_strokes > 0,
                    "{grammar:?} {beats} deflected={deflected} {lang}: Groove declared, never sounded"
                );
            }
        }
    }
}

/// U2 (Motif): a theme site is a promise to state the motif in that phrase. Holdout v1 planned
/// sites in phrases shorter than the statement (a 4.5-beat piece, a 1.25-beat coda), which no
/// performance can ever keep. A planned site must fit its phrase, and every planned identity site
/// must then actually be stated.
#[test]
fn u2_theme_sites_fit_their_phrase_and_are_stated() {
    for (i, (beats, deflected)) in [(4.5, false), (9.25, true), (12.0, false), (20.0, true)]
        .into_iter()
        .enumerate()
    {
        for composer in [
            Composer::StructuralR9,
            Composer::MeaningDirected,
            Composer::StablePropulsion,
        ] {
            let trace = if deflected {
                deflected_lift_trace(beats)
            } else {
                demo_trace(beats)
            };
            let s = SongMap::compose(
                &trace,
                77_100_031 + i as u64,
                Some(CompositionGrammar::HookArc),
                composer,
            );
            let tag = format!("{composer:?} {beats} deflected={deflected}");
            for site in &s.thematic.sites {
                let p = &s.plan.form.phrases[site.phrase as usize];
                let span = p.end_beat() - p.start_beat();
                assert!(
                    f64::from(site.motif.total_beats()) <= span + 1e-6,
                    "{tag}: site in phrase {} needs {} beats, phrase has {span}",
                    site.phrase,
                    site.motif.total_beats()
                );
            }
            let c = pocket(
                &s,
                &MusicWorld::black_ice(),
                MusicalLanguage::fusion_conversation(),
            );
            let conf = SongMapConformance::check(&s, &c.perf, &c.score);
            assert!(
                conf.missing_theme_sites.is_empty(),
                "{tag}: {:?}",
                conf.missing_theme_sites
            );
        }
    }
}

/// The distinction, proved both ways: no room is not a promise; room plus omission is a
/// violation the report names; and a generated source's established spec extracts.
#[test]
fn u2_anchor_report_separates_no_room_from_omission() {
    let world = MusicWorld::black_ice();
    // Shorter than one bar: no phrase can hold a groove, so Groove is structurally inapplicable.
    let short = song(3.0, 77_100_041, false, CompositionGrammar::HookArc);
    let c = pocket(&short, &world, MusicalLanguage::fusion_conversation());
    let report = AnchorReport::check(&short, &c.perf, &c.score);
    let groove = report
        .anchors
        .iter()
        .find(|(a, _)| *a == CoherenceAnchor::Groove)
        .expect("HookArc declares Groove");
    assert!(
        matches!(groove.1, AnchorPresence::StructurallyInapplicable(_)),
        "{report:?}"
    );
    assert!(report.violations().is_empty(), "{report:?}");
    // Room, and the performance states it.
    for (beats, deflected) in [(7.5, false), (12.0, false), (16.0, true), (23.75, false)] {
        for grammar in [CompositionGrammar::HookArc, CompositionGrammar::RiffDrive] {
            let s = song(beats, 77_100_042, deflected, grammar);
            let c = pocket(&s, &world, MusicalLanguage::simple());
            let report = AnchorReport::check(&s, &c.perf, &c.score);
            assert!(
                report.violations().is_empty(),
                "{grammar:?} {beats}: {report:?}"
            );
            let spec = CoverSpec::established(&report);
            assert!(spec.contains(gibson::audio::human_music::cover::CoverAxis::Groove));
            CoverMap::extract(&c, &world, spec)
                .unwrap_or_else(|e| panic!("{grammar:?} {beats}: established spec: {e:?}"));
            // Mutation: the same score with its kit removed is a declared-but-missing violation,
            // not an inapplicable anchor — the detector sees the omission.
            let mut silent = c.score.clone();
            silent.drums.clear();
            let broken = AnchorReport::check(&s, &c.perf, &silent);
            assert_eq!(
                broken.violations(),
                vec![(CoherenceAnchor::Groove, "no kick or snare stroke sounds")]
            );
        }
    }
}

/// Fresh songs for the action/obligation contracts: short and long, straight and deflected,
/// several grammars and composers (seeds 77_300_0xx, outside holdout v1).
fn verb_corpus() -> Vec<(String, SongMap)> {
    let mut out = Vec::new();
    let mut seed = 77_300_000;
    for (beats, deflected) in [
        (9.25, true),
        (16.0, true),
        (20.0, false),
        (32.5, true),
        (64.0, true),
        (96.0, false),
    ] {
        for grammar in [
            CompositionGrammar::HookArc,
            CompositionGrammar::RiffDrive,
            CompositionGrammar::DeflectedLift,
            CompositionGrammar::PropulsiveReturn,
        ] {
            for composer in [Composer::StructuralR9, Composer::MeaningDirected] {
                seed += 1;
                let trace = if deflected {
                    deflected_lift_trace(beats)
                } else {
                    demo_trace(beats)
                };
                out.push((
                    format!("{grammar:?} {composer:?} {beats} deflected={deflected} seed={seed}"),
                    SongMap::compose(&trace, seed, Some(grammar), composer),
                ));
            }
        }
    }
    out
}

/// U3, the mechanism: the historical `Planned` admission (every historical profile, POCKET
/// included) admits verbs no realizer performs and settles song obligations by discourse role
/// alone, before any action exists. On fresh seeds the audit finds them unwitnessed. This
/// characterizes the historical arm (which stays byte-exact); the contract is enforced for the
/// rehearsed admission law.
#[test]
fn u3_historical_planned_admission_leaves_verbs_and_debts_unperformed() {
    use std::collections::BTreeSet;
    let mut kinds = BTreeSet::new();
    let mut debts = 0;
    for (_, s) in verb_corpus() {
        for language in [
            MusicalLanguage::simple(),
            MusicalLanguage::fusion_conversation(),
        ] {
            let c = pocket(&s, &MusicWorld::black_ice(), language);
            for row in witness::audit(&c.perf, &c.score).rows {
                if !row.witnessed {
                    kinds.insert(format!("{:?}", row.kind));
                }
            }
            debts += SongMapConformance::check(&s, &c.perf, &c.score).unwitnessed_song_obligations;
        }
    }
    for kind in ["Fragment", "Hit", "Hold", "Resolve", "ReEntry", "Thicken"] {
        assert!(
            kinds.contains(kind),
            "{kind} no longer unperformed: {kinds:?}"
        );
    }
    assert!(debts > 0);
}

fn rehearsed(song: &SongMap, world: &MusicWorld, language: MusicalLanguage) -> Composition {
    let opts = PerformanceOptions {
        language,
        ..PerformanceOptions::default()
    };
    let profile = PerformanceProfile::POCKET.with_admission(ActionAdmission::Rehearsed);
    perform_with_profile(song, world, opts, profile).expect("rehearsed source")
}

/// U3, the contract: under rehearsed admission every verb left in the plan is performed. A verb
/// no player performed is struck before the take and recorded as a rejection with its reason —
/// it is never stamped onto something that was not played.
#[test]
fn u3_rehearsed_admission_performs_every_admitted_verb() {
    let mut struck = 0;
    for (tag, s) in verb_corpus() {
        for world in [MusicWorld::black_ice(), MusicWorld::vapor95()] {
            for language in [
                MusicalLanguage::simple(),
                MusicalLanguage::fusion_conversation(),
            ] {
                let c = rehearsed(&s, &world, language);
                let audit = witness::audit(&c.perf, &c.score);
                let open: Vec<_> = audit.rows.iter().filter(|r| !r.witnessed).collect();
                assert!(open.is_empty(), "{tag} {}: {open:?}", world.name);
                struck += c
                    .perf
                    .admissions
                    .iter()
                    .filter(|r| {
                        r.outcome
                            == Admission::Rejected {
                                reason: REHEARSAL_REJECTION,
                            }
                    })
                    .count();
            }
        }
    }
    assert!(struck > 0, "the corpus exercises rehearsal rejections");
}

/// The audit is not weakened by rehearsal: removing the stamps a witnessed verb relies on makes
/// it unwitnessed again, exactly as before.
#[test]
fn u3_rehearsal_leaves_the_audit_able_to_catch_a_forgery() {
    let (_, s) = &verb_corpus()[20];
    let c = rehearsed(
        s,
        &MusicWorld::black_ice(),
        MusicalLanguage::fusion_conversation(),
    );
    let audit = witness::audit(&c.perf, &c.score);
    let stamped: Vec<_> = audit
        .rows
        .iter()
        .filter(|r| r.witnessed && r.stamped > 0)
        .collect();
    assert!(!stamped.is_empty());
    for row in stamped {
        let mut forged = c.score.clone();
        let strip = |st: ActionStamp| {
            st.iter()
                .filter(|&id| id != row.action)
                .fold(ActionStamp::NONE, ActionStamp::with)
        };
        for n in &mut forged.notes {
            n.prov.actions = strip(n.prov.actions);
        }
        for d in &mut forged.drums {
            d.prov.actions = strip(d.prov.actions);
        }
        let again = witness::audit(&c.perf, &forged);
        let r = again.rows.iter().find(|r| r.action == row.action).unwrap();
        assert!(
            !r.witnessed,
            "{:?} still witnessed with its stamps removed",
            row.kind
        );
    }
}

/// U3, song obligations under rehearsed admission: each settled debt gets its discharging event
/// from the source planner (a real drum re-entry, a Resolve on the home chord). What stays
/// unwitnessed is named, not hidden: a cadence debt settled where no home chord arrives or whose
/// planned Resolve no player performs, or a motif question no answer reaches. A groove debt is
/// always paid by the kit's return.
#[test]
fn u3_rehearsed_obligations_are_paid_or_named() {
    use gibson::audio::human_music::discourse::ObligationKind as K;
    let (mut paid, mut open) = (0, 0);
    for (tag, s) in verb_corpus() {
        for language in [
            MusicalLanguage::simple(),
            MusicalLanguage::fusion_conversation(),
        ] {
            let c = rehearsed(&s, &MusicWorld::black_ice(), language);
            paid += c
                .perf
                .obligations
                .obligations
                .iter()
                .filter(|o| o.settlement.is_some_and(|s| s.witness.is_some()))
                .count();
            for o in c.perf.obligations.unwitnessed_settlements() {
                open += 1;
                assert!(
                    matches!(
                        o.kind,
                        K::SuspendedCadence | K::HarmonicDeparture | K::MotifQuestion
                    ),
                    "{tag}: {:?} settled without a discharging event",
                    o.kind
                );
            }
        }
    }
    assert!(paid > open, "paid {paid}, open {open}");
}

/// G02/G22 (holdout v1): a keys hold — a voicing chosen for its harmony — rang up to half a beat
/// into the next harmony, where its voice was not a member, and the temporal observer rejected
/// the false function claim. Reproduced on fresh seeds; the note is never relabelled.
#[test]
fn temporal_hold_voices_do_not_ring_into_a_nonmember_harmony() {
    use gibson::audio::human_music::temporal::TemporalPitchDiagnostics;
    for seed in 77_400_001..77_400_041u64 {
        for beats in [9.25, 12.0] {
            let s = song(beats, seed, true, CompositionGrammar::LoopEvolution);
            for world in WORLDS() {
                let c = pocket(&s, &world, MusicalLanguage::fusion_conversation());
                let law = TemporalPitchDiagnostics::measure(&c.perf, &c.score);
                let hold_breaks = c
                    .score
                    .notes
                    .iter()
                    .filter(|n| n.role == Role::Keys && n.prov.role_note == "hold")
                    .filter(|n| {
                        let end = n.start_beat + f64::from(n.dur_beats);
                        c.score.chords.iter().any(|h| {
                            h.start_beat > n.start_beat + 1e-6
                                && h.start_beat < end - 1e-6
                                && !h.chord.contains_pc(n.pitch.rem_euclid(12))
                        })
                    })
                    .count();
                assert_eq!(
                    hold_breaks,
                    0,
                    "seed {seed} {beats} {}: {} hold voices ring into a nonmember harmony\n{}",
                    world.name,
                    hold_breaks,
                    law.report()
                );
            }
        }
    }
}
