//! The drummer: one percussion surface. The historical drummer appends every producer's strokes
//! (a budget governs only its ghosts), so locally justified ornaments accumulate over a band that
//! is already speaking. Fresh seeds 77_600_0xx.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    cover::{cover_candidate, CoverAxis, CoverMap, CoverSpec, CoverTarget},
    functor::{perform_with_profile, Composition},
    percussion::{DrumRestraint, PercussionPolicy},
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::{DrumHit, DrumVoice, Role},
    semantic::deflected_lift_trace,
    witness, MusicWorld, SongMap,
};

fn corpus() -> Vec<SongMap> {
    let mut out = Vec::new();
    for (i, grammar) in [
        CompositionGrammar::HookArc,
        CompositionGrammar::LoopEvolution,
        CompositionGrammar::RiffDrive,
        CompositionGrammar::DeflectedLift,
    ]
    .into_iter()
    .enumerate()
    {
        for k in 0..3u64 {
            out.push(SongMap::compose(
                &deflected_lift_trace(64.0),
                77_600_001 + 10 * i as u64 + k,
                Some(grammar),
                Composer::StructuralR9,
            ));
        }
    }
    out
}

fn perform(song: &SongMap, world: &MusicWorld, profile: PerformanceProfile) -> Composition {
    perform_with_profile(song, world, PerformanceOptions::default(), profile).expect("source")
}

/// A stroke that adds information around the pocket rather than keeping it: a ghost, an open
/// hat, a unison accent, an answering echo or a fill stroke.
fn is_ornament(d: &DrumHit) -> bool {
    d.voice == DrumVoice::OpenHat
        || matches!(
            d.prov.groove_variation,
            Some("ghost" | "unison" | "answer" | "fill")
        )
}

fn bar_of(beat: f64) -> u32 {
    (beat / 4.0).floor() as u32
}

/// Bars where the lead alone already has five or more onsets while the drummer adds three or more
/// ornaments.
fn talking_over(c: &Composition) -> usize {
    let bars = (c.score.total_beats / 4.0).ceil() as u32;
    (0..bars)
        .filter(|&b| {
            let lead = c
                .score
                .role_notes(Role::Lead)
                .filter(|n| bar_of(n.start_beat) == b)
                .count();
            let ornaments = c
                .score
                .drums
                .iter()
                .filter(|d| bar_of(d.start_beat) == b && is_ornament(d))
                .count();
            lead >= 5 && ornaments >= 3
        })
        .count()
}

/// The mechanism, on the historical (unarbitrated) drummer: it keeps ornamenting bars in which
/// the lead is busy, because fills, unison accents, answers and open hats never meet the budget
/// that only its ghosts consult. Characterizes the historical arm, which stays byte-exact.
#[test]
fn historical_drummer_keeps_ornamenting_over_a_busy_band() {
    let mut bars = 0;
    for song in corpus() {
        for world in [MusicWorld::black_ice(), MusicWorld::vapor95()] {
            let c = perform(&song, &world, PerformanceProfile::POCKET);
            bars += talking_over(&c);
        }
    }
    eprintln!("historical kit talks over a busy lead in {bars} bars");
    assert!(
        bars > 0,
        "no bar where the historical kit talks over a busy lead"
    );
}

fn arbitrated(restraint: DrumRestraint) -> PerformanceProfile {
    // Planned admission: every restraint realizes the SAME plan, so differences are the drummer's.
    PerformanceProfile::POCKET.with_drum_restraint(restraint)
}

fn ornaments_admitted(c: &Composition) -> usize {
    c.score
        .percussion
        .as_ref()
        .expect("arbitrated drummer reports")
        .admitted_ornaments()
}

/// The historical arms keep the historical drummer; the new general profile arbitrates at
/// Balanced.
#[test]
fn historical_profiles_keep_the_unarbitrated_drummer() {
    assert_eq!(
        PerformanceProfile::POCKET.percussion,
        PercussionPolicy::Unarbitrated
    );
    assert_eq!(
        PerformanceProfile::BAND.percussion,
        PercussionPolicy::Arbitrated(DrumRestraint::Balanced)
    );
    let song = &corpus()[0];
    let c = perform(song, &MusicWorld::black_ice(), PerformanceProfile::POCKET);
    assert!(c.score.percussion.is_none());
}

/// Restraint is a dial: the same plan, more optional strokes as it opens, never fewer.
#[test]
fn restraint_orders_the_ornaments_it_admits() {
    for song in corpus() {
        for world in [MusicWorld::black_ice(), MusicWorld::vapor95()] {
            let counts: Vec<usize> = DrumRestraint::ALL
                .iter()
                .map(|&r| ornaments_admitted(&perform(&song, &world, arbitrated(r))))
                .collect();
            assert!(counts.windows(2).all(|w| w[0] <= w[1]), "{counts:?}");
            assert!(counts[0] < counts[3], "{counts:?}");
        }
    }
}

/// Balanced yields where the band speaks: a bar the ordered decision reads as "the band is
/// already speaking" spends no ornament, and the kit talks over a busy lead far less often than
/// the historical drummer.
#[test]
fn balanced_drummer_yields_where_the_band_speaks() {
    let (mut historical, mut balanced) = (0, 0);
    for song in corpus() {
        for world in [MusicWorld::black_ice(), MusicWorld::vapor95()] {
            historical += talking_over(&perform(&song, &world, PerformanceProfile::POCKET));
            let c = perform(&song, &world, arbitrated(DrumRestraint::Balanced));
            balanced += talking_over(&c);
            let report = c.score.percussion.as_ref().unwrap();
            for bar in &report.bars {
                if bar.reason == "the band is already speaking" {
                    assert_eq!(bar.admitted, 0, "{bar:?}");
                }
            }
        }
    }
    eprintln!("talking over a busy lead: historical {historical} bars, balanced {balanced}");
    assert!(
        balanced * 2 <= historical,
        "historical {historical}, balanced {balanced}"
    );
}

/// Arbitration never erases a required receipt: every action the historical drummer's take
/// witnesses is still witnessed by the most restrained drummer (same plan).
#[test]
fn restraint_never_erases_a_required_witness() {
    for song in corpus() {
        for world in [MusicWorld::black_ice(), MusicWorld::vapor95()] {
            let before = perform(&song, &world, PerformanceProfile::POCKET);
            let seen = witness::audit(&before.perf, &before.score);
            for r in DrumRestraint::ALL {
                let c = perform(&song, &world, arbitrated(r));
                let now = witness::audit(&c.perf, &c.score);
                for (was, is) in seen.rows.iter().zip(&now.rows) {
                    assert_eq!(was.action, is.action);
                    assert!(
                        !was.witnessed || is.witnessed,
                        "{:?} {:?} lost its witness at {r:?}",
                        was.kind,
                        was.action
                    );
                }
            }
        }
    }
}

/// The pocket does not move with the dial: downbeat kicks and backbeats sound at the same
/// instants, in the same voices, at every restraint.
#[test]
fn the_pocket_is_identical_at_every_restraint() {
    let anchors = |c: &Composition| -> Vec<(i64, DrumVoice)> {
        c.score
            .drums
            .iter()
            .filter(|d| {
                d.prov.groove_variation == Some("backbeat")
                    || (d.prov.groove_variation == Some("kick")
                        && (d.start_beat / 4.0 - (d.start_beat / 4.0).round()).abs() < 0.01)
            })
            .map(|d| ((d.start_beat * 1e6).round() as i64, d.voice))
            .collect()
    };
    for song in corpus().iter().take(4) {
        let world = MusicWorld::black_ice();
        let reference = anchors(&perform(song, &world, arbitrated(DrumRestraint::Busy)));
        assert!(!reference.is_empty());
        for r in DrumRestraint::ALL {
            assert_eq!(
                anchors(&perform(song, &world, arbitrated(r))),
                reference,
                "{r:?}"
            );
        }
    }
}

/// A pinned cover groove is the song's identity, not the drummer's ornament: every restraint
/// plays exactly the pinned drum part.
#[test]
fn a_pinned_cover_groove_ignores_the_dial() {
    let world = MusicWorld::black_ice();
    let song = &corpus()[1];
    let source = perform(song, &world, PerformanceProfile::POCKET);
    let map =
        CoverMap::extract(&source, &world, CoverSpec::new([CoverAxis::Groove])).expect("groove");
    let drums = |profile: PerformanceProfile| {
        let c = cover_candidate(
            &map,
            CoverTarget {
                world: &world,
                seed: 77_600_900,
                grammar: CompositionGrammar::HookArc,
                options: PerformanceOptions::default(),
                profile,
            },
        )
        .expect("groove cover");
        c.score
            .drums
            .iter()
            .map(|d| ((d.start_beat * 1e6).round() as i64, d.voice))
            .collect::<Vec<_>>()
    };
    let reference = drums(PerformanceProfile::POCKET);
    for r in DrumRestraint::ALL {
        assert_eq!(drums(arbitrated(r)), reference, "{r:?}");
    }
}

/// Foundation means the pocket: an ornament is admitted only in a bar where the drummer has the
/// floor (at most one there), or by the rate guard keeping a pull-back/acceleration's witness.
/// A surface verb is not a licence to open the whole bar.
#[test]
fn foundation_speaks_only_on_the_floor_or_for_a_surface_witness() {
    for song in corpus() {
        for world in [MusicWorld::black_ice(), MusicWorld::vapor95()] {
            let c = perform(&song, &world, arbitrated(DrumRestraint::Foundation));
            let report = c.score.percussion.as_ref().unwrap();
            let floor_bars = report
                .bars
                .iter()
                .filter(|b| b.evidence.drummer_floor)
                .count();
            for b in &report.bars {
                assert!(b.admitted <= usize::from(b.evidence.drummer_floor), "{b:?}");
            }
            assert!(
                report.admitted_ornaments() <= floor_bars + report.rate_guard,
                "{}",
                report.report()
            );
        }
    }
}
