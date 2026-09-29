//! Round XIII: temporal-mass audit of the frozen Round XII scores, and human-timestamp lookup.
//!
//! `--around-seconds=S` (repeatable) prints every note sounding within `--radius=R` seconds
//! (default 1.5) of `S` in the SWISS and BLACK_ICE StablePropulsion scores: harmony, written and
//! audible seconds, mass evidence, Round XII path and the mass verdict. Text only.
use gibson::audio::human_music::{
    composer::Composer, contract::CompositionGrammar, functor::perform_temporal,
    mass::MassDiagnostics, performance::PerformanceOptions, semantic::deflected_lift_trace,
    song::SongMap, MusicWorld,
};
use std::path::Path;

/// Round XII final-receipt score fingerprints (docs/fixtures/humanmusic-r12/final/*_temporal.audit.txt).
const FROZEN: [(&str, &str, u64); 4] = [
    ("stable", "swiss", 0x8ab3cd9fa10d751c),
    ("stable", "black_ice", 0x06446ebcbbd14038),
    ("deflected", "swiss", 0),
    ("deflected", "black_ice", 0),
];

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(k).map(str::to_owned))
    };
    let out = arg("--out=").unwrap_or_else(|| "target/humanmusic-r13/baseline".into());
    let radius: f64 = arg("--radius=").map_or(1.5, |r| r.parse().expect("--radius=seconds"));
    let around: Vec<f64> = args
        .iter()
        .filter_map(|a| a.strip_prefix("--around-seconds="))
        .map(|s| s.parse().expect("--around-seconds=seconds"))
        .collect();
    let dir = Path::new(&out);
    std::fs::create_dir_all(dir)?;
    let trace = deflected_lift_trace(120.0);
    let opts = PerformanceOptions::default();
    for (regime, composer, grammar) in [
        ("stable", Composer::StablePropulsion, None),
        (
            "deflected",
            Composer::MeaningDirected,
            Some(CompositionGrammar::DeflectedLift),
        ),
    ] {
        let song = SongMap::compose(&trace, 2112, grammar, composer);
        for (name, world) in [
            ("swiss", MusicWorld::swiss_signal()),
            ("black_ice", MusicWorld::black_ice()),
        ] {
            let a = perform_temporal(&song, &world, opts);
            let frozen = FROZEN
                .iter()
                .find(|f| f.0 == regime && f.1 == name)
                .map_or(0, |f| f.2);
            if frozen != 0 {
                assert_eq!(
                    a.score.fingerprint(),
                    frozen,
                    "not the frozen Round XII score"
                );
            }
            let d = MassDiagnostics::measure(&a.perf, &a.score, &world);
            let header = format!(
                "regime={regime} world={name} seed=2112 beats=120 tempo={} arm=r12-temporal\nsong={:#018x} performance={:#018x} score={:#018x}\n",
                a.score.tempo_bpm,
                song.fingerprint(),
                a.perf.fingerprint(),
                a.score.fingerprint()
            );
            std::fs::write(
                dir.join(format!("{name}_{regime}_r12.mass.txt")),
                format!("{header}{}", d.report()),
            )?;
            println!(
                "{name}/{regime}: unowned_asserted_colors={} accidental_reharmonizations={} overdrawn_gestures={} soft_orphans={} asserted_events={}",
                d.unowned_asserted_colors,
                d.accidental_reharmonizations,
                d.overdrawn_gestures,
                d.soft_orphans,
                d.asserted_events
            );
            if regime == "stable" {
                for &s in &around {
                    let text = d.around_seconds(&a.score, s, radius);
                    std::fs::write(
                        dir.join(format!("{name}_stable_r12.around-{s}s.txt")),
                        format!("{header}{text}"),
                    )?;
                    println!("--- {name} around {s} s\n{text}");
                }
            }
        }
    }
    Ok(())
}
