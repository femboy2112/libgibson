//! The drummer: one percussion surface. The historical drummer appends every producer's strokes
//! (a budget governs only its ghosts), so locally justified ornaments accumulate over a band that
//! is already speaking. Fresh seeds 77_600_0xx.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    functor::{perform_with_profile, Composition},
    language::MusicalLanguage,
    performance::PerformanceOptions,
    policy::PerformanceProfile,
    score::{DrumHit, DrumVoice, Role},
    semantic::deflected_lift_trace,
    MusicWorld, SongMap,
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
