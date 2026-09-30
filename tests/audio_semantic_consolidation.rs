//! Pre-main semantic consolidation: each falsifier here was committed red before its repair.
//!
//! Holdout v2 is known evidence; nothing here replays its rows. The seeds (78_301_0xx) are
//! fresh. Where a law is checked, the test restates what the world or language DECLARES
//! (world.rs / language.rs documentation) instead of calling the library's own predicate, so a
//! wrong law cannot certify itself.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    cover::{cover_candidate, CoverError, CoverMap, CoverSpec, CoverTarget},
    functor::{perform_with_profile, Composition},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    semantic::{deflected_lift_trace, demo_trace},
    song::AnchorReport,
    theory::{Chord, Quality, Scale},
    MusicWorld, SongMap,
};

const GRAMMARS: [CompositionGrammar; 5] = [
    CompositionGrammar::HookArc,
    CompositionGrammar::LoopEvolution,
    CompositionGrammar::RiffDrive,
    CompositionGrammar::DeflectedLift,
    CompositionGrammar::PropulsiveReturn,
];

fn worlds() -> [MusicWorld; 3] {
    [
        MusicWorld::black_ice(),
        MusicWorld::swiss_signal(),
        MusicWorld::vapor95(),
    ]
}

fn languages() -> [(MusicalLanguage, &'static str); 2] {
    [
        (MusicalLanguage::simple(), "simple"),
        (MusicalLanguage::fusion_conversation(), "fusion"),
    ]
}

fn options(language: MusicalLanguage) -> PerformanceOptions {
    PerformanceOptions {
        language,
        ..PerformanceOptions::default()
    }
}

/// The harmonic vocabulary `world` and `language` declare, restated from their documentation:
/// a four-note (or larger) quality only where the world uses sevenths; a sixth or ninth colour
/// only where the language's colour depth is non-zero ("0 = triads/7ths only"); a tone outside
/// the world's home scale only where the world allows modal mixture.
fn declared(world: &MusicWorld, language: &MusicalLanguage, chord: Chord) -> bool {
    let colour = matches!(
        chord.quality,
        Quality::Maj6
            | Quality::Min6
            | Quality::Add9
            | Quality::Maj9
            | Quality::Min9
            | Quality::Dom9
    );
    let home = Scale::new(world.tonic_pc, world.mode);
    (world.use_sevenths || chord.quality.intervals().len() <= 3)
        && (language.color_depth > 0 || !colour)
        && (world.allow_modal_mixture
            || chord
                .quality
                .intervals()
                .iter()
                .all(|i| home.contains_pc(chord.root_pc + i)))
}

/// The refusal a room gives a song whose chart names a chord the room cannot admit.
const VOCABULARY_REFUSAL: &str = "outside the world's harmonic vocabulary";

/// H05 (holdout v2, reproduced on fresh seeds): a SWISS_SIGNAL PropulsiveReturn source sounded
/// `Maj6`/`Maj7` Reset chords although SWISS declares no sevenths. The sweep showed the leak is
/// general: the backbone's colours and the harmonic edits ignore the world's sevenths and the
/// language's colour depth in every world. Under the hardened general profile every chord a
/// performance sounds is admitted by the vocabulary its world and language declare, or the
/// performance is refused before anybody plays.
#[test]
fn h05_band_sources_sound_only_their_declared_vocabulary() {
    let mut leaks = Vec::new();
    let mut refusals = 0;
    for world in worlds() {
        for (language, lang) in languages() {
            for grammar in GRAMMARS {
                for (i, beats) in [17.25, 40.0].into_iter().enumerate() {
                    let seed = 78_301_010 + i as u64;
                    let composer = if grammar == CompositionGrammar::PropulsiveReturn {
                        Composer::StablePropulsion
                    } else {
                        Composer::StructuralR9
                    };
                    let song = SongMap::compose(&demo_trace(beats), seed, Some(grammar), composer);
                    let tag = format!("{} {lang} {grammar:?} {beats}", world.name);
                    match perform_with_profile(
                        &song,
                        &world,
                        options(language),
                        PerformanceProfile::BAND,
                    ) {
                        Ok(c) => {
                            for s in &c.perf.chords {
                                if !declared(&world, &language, s.chord) {
                                    leaks.push(format!(
                                        "{tag}: {:?} on {} at {} ({})",
                                        s.chord.quality, s.chord.root_pc, s.start_beat, s.note
                                    ));
                                }
                            }
                            assert_eq!(c.score.chords.len(), c.perf.chords.len(), "{tag}");
                        }
                        Err(e) => {
                            assert!(e.0.contains(VOCABULARY_REFUSAL), "{tag}: {e}");
                            refusals += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(
        leaks.is_empty(),
        "{} undeclared chords:\n{}",
        leaks.len(),
        leaks.join("\n")
    );
    // A StructuralR9/StablePropulsion chart is diatonic: none of these may be refused.
    assert_eq!(refusals, 0, "a diatonic chart was refused");
}

/// The H05 consequence, end to end: a SWISS source covered into SWISS keeps its own harmony. The
/// target lawfully refuses a pinned chord outside its vocabulary; the source must not have sounded one.
#[test]
fn h05_a_swiss_source_covers_into_swiss_without_a_vocabulary_refusal() {
    let world = MusicWorld::swiss_signal();
    for (language, lang) in languages() {
        for (seed, beats) in [(78_301_020, 17.25), (78_301_021, 33.0)] {
            let song = SongMap::compose(
                &demo_trace(beats),
                seed,
                Some(CompositionGrammar::PropulsiveReturn),
                Composer::StablePropulsion,
            );
            let source =
                perform_with_profile(&song, &world, options(language), PerformanceProfile::BAND)
                    .expect("a diatonic chart performs in SWISS");
            let spec = CoverSpec::established(&AnchorReport::check(
                &source.song,
                &source.perf,
                &source.score,
            ));
            let map = CoverMap::extract(&source, &world, spec).expect("extract");
            let lifted = cover_candidate(
                &map,
                CoverTarget {
                    world: &world,
                    seed: seed + 100,
                    grammar: CompositionGrammar::PropulsiveReturn,
                    options: options(language),
                    profile: PerformanceProfile::BAND,
                },
            );
            if let Err(CoverError::Invalid(why)) = &lifted {
                assert!(
                    !why.contains("outside target vocabulary"),
                    "{lang} {beats}: the SWISS source sounded a chord SWISS refuses: {why}"
                );
            }
        }
    }
}

/// A chart root the room cannot admit is not recoloured into something else (a room never picks
/// a root): the performance is refused, typed, before anybody plays. The same song performs in a
/// room that admits it. The chart is written by hand (a deflection to bVI, `Chromatic{8, Maj}`),
/// because the current composers rarely sound a borrowed chart root: the law must hold anyway.
#[test]
fn h05_a_chart_the_room_cannot_admit_is_refused_not_leaked() {
    use gibson::audio::human_music::backbone::ChartRoot;
    let mut song = SongMap::compose(
        &deflected_lift_trace(48.0),
        78_301_001,
        Some(CompositionGrammar::DeflectedLift),
        Composer::StructuralR9,
    );
    let bvi = ChartRoot::Chromatic {
        semitones: 8,
        quality: Quality::Maj,
    };
    let chart = song
        .harmonic
        .as_mut()
        .expect("DeflectedLift charts a backbone");
    chart.cell.deflect = bvi;
    chart.cell.satellites[0] = bvi;
    let language = MusicalLanguage::fusion_conversation();
    let swiss = MusicWorld::swiss_signal();
    // The archived arm shows the chart really does sound the borrowed chord in this room.
    let archived =
        perform_with_profile(&song, &swiss, options(language), PerformanceProfile::POCKET).unwrap();
    assert!(
        archived
            .perf
            .chords
            .iter()
            .any(|s| !Scale::new(0, swiss.mode).contains_pc(s.chord.root_pc)),
        "fixture: the chart must name a borrowed root that sounds within 48 beats"
    );
    match perform_with_profile(&song, &swiss, options(language), PerformanceProfile::BAND) {
        Err(e) => assert!(e.0.contains(VOCABULARY_REFUSAL), "{e}"),
        Ok(c) => panic!(
            "SWISS performed a chart it cannot admit: {:?}",
            c.perf
                .chords
                .iter()
                .filter(|s| !declared(&swiss, &language, s.chord))
                .map(|s| (s.start_beat, s.chord))
                .collect::<Vec<_>>()
        ),
    }
    let vapor = MusicWorld::vapor95();
    let c = perform_with_profile(&song, &vapor, options(language), PerformanceProfile::BAND)
        .expect("a room with modal mixture admits bVI");
    let leaks: Vec<_> = c
        .perf
        .chords
        .iter()
        .filter(|s| !declared(&vapor, &language, s.chord))
        .map(|s| (s.start_beat, s.chord))
        .collect();
    assert!(leaks.is_empty(), "{leaks:?}");
}

/// The historical arm is characterized, not repaired: POCKET keeps its archived colours
/// (byte-exact R17), so the boundary between the two laws is visible.
#[test]
fn h05_the_historical_pocket_arm_keeps_its_archived_colours() {
    let song = SongMap::compose(
        &demo_trace(17.25),
        78_301_020,
        Some(CompositionGrammar::PropulsiveReturn),
        Composer::StablePropulsion,
    );
    let swiss = MusicWorld::swiss_signal();
    let language = MusicalLanguage::fusion_conversation();
    let c: Composition =
        perform_with_profile(&song, &swiss, options(language), PerformanceProfile::POCKET).unwrap();
    assert!(
        c.perf.chords.iter().any(|s| !declared(&swiss, &language, s.chord)),
        "the archived SWISS Reset colours are characterized here; if this changes, the R17 arm moved"
    );
}

/// Every temporal object a performance plans or realizes, as `(what, start, end)`: action and
/// stage windows, material and statement spans, chord and region spans, ownership spans,
/// reservations, notes and drum strokes.
fn temporal_objects(c: &Composition) -> Vec<(String, f64, f64)> {
    let mut out = Vec::new();
    for a in &c.perf.actions.actions {
        out.push((format!("action {:?}", a.kind), a.start_beat, a.end_beat()));
    }
    for w in &c.perf.stage.windows {
        out.push((
            format!("stage window {:?}", w.agent),
            w.start_beat,
            w.end_beat,
        ));
    }
    for m in &c.perf.materials {
        out.push((
            format!("material {:?} {:?}", m.id, m.owner),
            m.start_beat,
            m.start_beat + m.length(),
        ));
    }
    for s in &c.perf.statements {
        out.push((
            format!("statement {}", s.phrase),
            s.start_beat,
            s.end_beat(),
        ));
    }
    for s in &c.perf.chords {
        out.push((
            "chord".into(),
            s.start_beat,
            s.start_beat + f64::from(s.dur_beats),
        ));
    }
    for r in &c.perf.regions.spans {
        out.push(("region".into(), r.start_beat, r.end_beat));
    }
    for o in &c.score.occupancy {
        for s in &o.spans {
            out.push((
                format!("ownership {:?} {:?}", o.role, s.kind),
                s.start,
                s.end,
            ));
        }
        for r in &o.rhythm {
            out.push((format!("reservation {:?}", o.role), r.beat, r.end_beat));
        }
    }
    for n in &c.score.notes {
        out.push((
            format!("note {:?}", n.role),
            n.start_beat,
            n.start_beat + f64::from(n.dur_beats),
        ));
    }
    for d in &c.score.drums {
        out.push(("drum".into(), d.start_beat, d.start_beat));
    }
    out
}

/// H12 (holdout v2, reproduced on fresh seeds): U1 bounded the bass line of a partial final bar,
/// but an ownership span derived from a planned window could still run past the piece. The law is
/// not about one role: every planned and realized temporal object inhabits the performance's
/// domain `[0, total_beats]` (a drum stroke or an onset strictly before the end).
#[test]
fn h12_every_temporal_object_inhabits_the_performance_domain() {
    let mut outside = Vec::new();
    for world in worlds() {
        for (language, lang) in languages() {
            for grammar in GRAMMARS {
                for (i, beats) in [6.75, 7.25, 10.5, 13.25, 21.75].into_iter().enumerate() {
                    for (profile, arm) in [
                        (PerformanceProfile::POCKET, "pocket"),
                        (PerformanceProfile::BAND, "band"),
                    ] {
                        let seed = 78_301_030 + i as u64;
                        let trace = if i % 2 == 0 {
                            deflected_lift_trace(beats)
                        } else {
                            demo_trace(beats)
                        };
                        let composer = if grammar == CompositionGrammar::PropulsiveReturn {
                            Composer::StablePropulsion
                        } else {
                            Composer::MeaningDirected
                        };
                        let song = SongMap::compose(&trace, seed, Some(grammar), composer);
                        let Ok(c) = perform_with_profile(&song, &world, options(language), profile)
                        else {
                            continue;
                        };
                        let total = c.perf.total_beats;
                        assert_eq!(total, beats);
                        for (what, start, end) in temporal_objects(&c) {
                            let inside = start.is_finite()
                                && end.is_finite()
                                && start >= 0.0
                                && start < total
                                && end <= total + 1e-9;
                            if !inside {
                                outside.push(format!(
                                    "{} {lang} {grammar:?} {beats} {arm}: {what} {start}..{end}",
                                    world.name
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    let mut kinds: Vec<_> = outside
        .iter()
        .map(|o| {
            o.split(": ")
                .nth(1)
                .unwrap_or("")
                .split(' ')
                .take(2)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    kinds.sort();
    kinds.dedup();
    assert!(
        outside.is_empty(),
        "{} objects outside the domain; kinds {kinds:?}\n{}",
        outside.len(),
        outside
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Mutation controls for the domain law: the plan's own audit and the (unchanged) occupancy
/// checker still reject a window forged past the end; the clipping authority returns every
/// in-domain window bit for bit (no archived receipt may drift by rounding).
#[test]
fn h12_the_domain_audit_and_the_occupancy_checker_still_catch_a_forged_overrun() {
    use gibson::audio::human_music::{
        occupancy::{self, OwnershipKind, OwnershipSpan},
        performance::PerformanceDomain,
        score::Role,
    };
    let song = SongMap::compose(
        &deflected_lift_trace(7.25),
        78_301_031,
        Some(CompositionGrammar::HookArc),
        Composer::MeaningDirected,
    );
    let world = MusicWorld::black_ice();
    let mut c = perform_with_profile(
        &song,
        &world,
        options(MusicalLanguage::fusion_conversation()),
        PerformanceProfile::BAND,
    )
    .unwrap();
    assert!(c.perf.domain_violations().is_empty());
    assert!(occupancy::violations(&c.perf, &c.score, true).is_empty());

    let total = c.perf.total_beats;
    let mut forged = c.perf.clone();
    let a = forged.actions.actions.last_mut().expect("an action");
    a.dur_beats = total + 1.0 - a.start_beat;
    assert!(
        !forged.domain_violations().is_empty(),
        "an overrun action window must be named"
    );

    let bass = c
        .score
        .occupancy
        .iter_mut()
        .find(|o| o.role == Role::Bass)
        .expect("a bass owner");
    bass.spans.push(OwnershipSpan {
        start: total - 0.25,
        end: total + 0.25,
        material: None,
        kind: OwnershipKind::Action,
    });
    assert!(
        occupancy::violations(&c.perf, &c.score, true)
            .iter()
            .any(|v| v.contains("invalid ownership span")),
        "the checker is never loosened"
    );

    let d = PerformanceDomain::new(7.25);
    for (start, end) in [(0.1, 0.3), (1.0 / 3.0, 7.25), (6.9, 7.1)] {
        let (s, e) = d.clip(start, end).unwrap();
        assert_eq!(
            (s.to_bits(), e.to_bits()),
            (f64::to_bits(start), f64::to_bits(end))
        );
        assert_eq!(d.fit(start, end - start), Some(end - start));
    }
    assert_eq!(d.clip(7.0, 7.5), Some((7.0, 7.25)));
    assert_eq!(d.fit(7.0, 0.5), Some(0.25));
    assert_eq!(d.fit(7.25, 0.5), None);
    assert_eq!(d.clip(8.0, 9.0), None);
}

/// H16/H28/H30 (holdout v2), reproduced on fresh seeds: a debt the song's discourse settles in
/// a phrase was recorded settled in the performance although no event of the realized score
/// discharges it — the settling phrase never arrived home, or the planned resolution was struck at
/// rehearsal because nobody played it. Discourse says WHERE a debt should settle; only a realized
/// discharge makes it settled. A performance's settlement is true only when the realized score
/// witnesses the discharging verb (the action audit, unchanged, is the judge).
#[test]
fn u3b_a_settlement_is_true_only_when_the_realized_score_discharges_it() {
    use gibson::audio::human_music::witness;
    let mut false_claims = Vec::new();
    let mut settled = 0;
    for world in worlds() {
        for (language, lang) in languages() {
            for grammar in GRAMMARS {
                for seed in 78_302_000u64..78_302_004 {
                    for beats in [24.0, 40.5, 64.0] {
                        let composer = if seed % 2 == 0 {
                            Composer::StructuralR9
                        } else {
                            Composer::MeaningDirected
                        };
                        let trace = if seed % 2 == 0 {
                            deflected_lift_trace(beats)
                        } else {
                            demo_trace(beats)
                        };
                        let song = SongMap::compose(&trace, seed, Some(grammar), composer);
                        let Ok(c) = perform_with_profile(
                            &song,
                            &world,
                            options(language),
                            PerformanceProfile::BAND,
                        ) else {
                            continue;
                        };
                        let audit = witness::audit(&c.perf, &c.score);
                        for o in &c.perf.obligations.obligations {
                            let Some(s) = o.settlement else { continue };
                            settled += 1;
                            let realized = s.witness.is_some_and(|w| {
                                audit.rows.iter().any(|r| r.action == w && r.witnessed)
                            });
                            if !realized {
                                false_claims.push(format!(
                                    "{} {lang} {grammar:?} {seed} {beats}: {:?} settled by phrase {} ({:?}) with witness {:?}",
                                    world.name, o.kind, s.by_phrase, s.how, s.witness
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(settled > 0);
    assert!(
        false_claims.is_empty(),
        "{} of {settled} settlements have no realized discharge:\n{}",
        false_claims.len(),
        false_claims.join("\n")
    );
}

/// The obligation layer, not the verb rejection, decides a debt: every debt the song settles is,
/// in a rehearsed performance, either settled by a performed verb or left open with a reason —
/// and each stated reason is TRUE of the performance (checked here from the chords and the
/// rehearsal record, not from the reason string). A recast settlement lands on an arrival a
/// player made. Nothing is dropped silently, and the historical arm keeps its archived ledger.
#[test]
fn u3b_every_open_debt_is_named_and_its_reason_is_true() {
    use gibson::audio::human_music::{
        discourse::ObligationKind, rehearsal::RehearsalOutcome, song::SongMapConformance, witness,
    };
    let (mut open, mut recast, mut settled) = (0, 0, 0);
    for world in worlds() {
        for (language, lang) in languages() {
            for grammar in GRAMMARS {
                for seed in 78_302_010u64..78_302_014 {
                    for beats in [32.0, 56.75] {
                        let composer = if seed % 2 == 0 {
                            Composer::StructuralR9
                        } else {
                            Composer::MeaningDirected
                        };
                        let song = SongMap::compose(
                            &deflected_lift_trace(beats),
                            seed,
                            Some(grammar),
                            composer,
                        );
                        let c = perform_with_profile(
                            &song,
                            &world,
                            options(language),
                            PerformanceProfile::BAND,
                        )
                        .expect("no refusal on this corpus");
                        let tag = format!("{} {lang} {grammar:?} {seed} {beats}", world.name);
                        let law = SongMapConformance::check(&song, &c.perf, &c.score);
                        assert!(
                            law.dropped_song_obligations.is_empty(),
                            "{tag}: {}",
                            law.report()
                        );
                        assert_eq!(law.unwitnessed_song_obligations, 0, "{tag}");
                        let trace = c
                            .perf
                            .rehearsal
                            .as_ref()
                            .expect("a rehearsed plan keeps its trace");
                        for d in &trace.open_debts {
                            open += 1;
                            let phrase = &song.plan.form.phrases[d.by_phrase as usize];
                            let home = c.perf.region.tonic_pc.rem_euclid(12);
                            let sounds_home = c.perf.chords.iter().any(|s| {
                                s.start_beat >= phrase.start_beat() - 1e-9
                                    && s.start_beat < phrase.end_beat() - 1e-9
                                    && s.chord.root_pc.rem_euclid(12) == home
                            });
                            let rejected = trace.verbs.iter().any(|v| {
                                v.key.settles() == Some(d.obligation)
                                    && matches!(v.outcome, RehearsalOutcome::Rejected(_))
                            });
                            match d.reason {
                                r if r.starts_with("rehearsed:") => {
                                    assert!(rejected, "{tag}: {d:?}")
                                }
                                r if r.starts_with(
                                    "the settling phrase never sounds the home chord",
                                ) =>
                                {
                                    assert!(
                                        matches!(
                                            d.kind,
                                            ObligationKind::SuspendedCadence
                                                | ObligationKind::HarmonicDeparture
                                        ) && !sounds_home,
                                        "{tag}: false reason {d:?}"
                                    )
                                }
                                _ => {}
                            }
                            assert!(
                                c.perf
                                    .obligations
                                    .get(d.obligation)
                                    .is_some_and(|o| o.settlement.is_none()),
                                "{tag}: an open debt is still recorded settled"
                            );
                        }
                        let audit = witness::audit(&c.perf, &c.score);
                        for v in &trace.verbs {
                            if let RehearsalOutcome::Recast { beat, .. } = v.outcome {
                                recast += 1;
                                let a = c
                                    .perf
                                    .actions
                                    .actions
                                    .iter()
                                    .find(|a| a.start_beat == beat && a.kind == v.kind)
                                    .expect("the recast verb is on the chart where it moved");
                                assert!(
                                    audit.rows.iter().any(|r| r.action == a.id && r.witnessed),
                                    "{tag}: a recast settlement is unperformed"
                                );
                            }
                        }
                        settled += c
                            .perf
                            .obligations
                            .obligations
                            .iter()
                            .filter(|o| o.settlement.is_some())
                            .count();
                    }
                }
            }
        }
    }
    assert!(
        settled > 0 && open > 0,
        "the corpus exercises both outcomes: {settled} settled, {open} open, {recast} recast"
    );
}

/// Mutation: a settlement forged into a rehearsed performance's ledger without a discharging
/// verb, and a debt silently cleared, are both caught; the historical arm's archived ledger is
/// untouched (it never gains a trace and still reports its unwitnessed settlements).
#[test]
fn u3b_forged_and_dropped_settlements_are_caught() {
    use gibson::audio::human_music::{
        discourse::{SettleHow, Settlement},
        song::SongMapConformance,
    };
    let world = MusicWorld::vapor95();
    let language = MusicalLanguage::fusion_conversation();
    let mut found = false;
    for seed in 78_302_020u64..78_302_040 {
        let song = SongMap::compose(
            &deflected_lift_trace(48.0),
            seed,
            Some(CompositionGrammar::HookArc),
            Composer::StructuralR9,
        );
        let c = perform_with_profile(&song, &world, options(language), PerformanceProfile::BAND)
            .unwrap();
        let Some(d) = c
            .perf
            .rehearsal
            .as_ref()
            .and_then(|t| t.open_debts.first().copied())
        else {
            continue;
        };
        found = true;
        // Forge: claim the open debt settled with no witness.
        let mut forged = c.perf.clone();
        let o = forged
            .obligations
            .obligations
            .iter_mut()
            .find(|o| o.id == d.obligation)
            .unwrap();
        o.settlement = Some(Settlement {
            by_phrase: d.by_phrase,
            how: SettleHow::Paid,
            witness: None,
        });
        assert!(!SongMapConformance::check(&song, &forged, &c.score).passes());
        // Drop: clear the reason, keep the debt open.
        let mut dropped = c.perf.clone();
        dropped.rehearsal.as_mut().unwrap().open_debts.clear();
        let law = SongMapConformance::check(&song, &dropped, &c.score);
        assert!(
            !law.passes() && !law.dropped_song_obligations.is_empty(),
            "{}",
            law.report()
        );
        // The archived arm: no trace, archived ledger.
        let pocket =
            perform_with_profile(&song, &world, options(language), PerformanceProfile::POCKET)
                .unwrap();
        assert!(pocket.perf.rehearsal.is_none());
        break;
    }
    assert!(found, "the corpus holds an open debt to mutate");
}
