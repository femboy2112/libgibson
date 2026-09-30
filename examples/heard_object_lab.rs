//! Round XIV: what musical object did the listener actually hear?
//!
//! The heard-identity audit ([`identity`]) and the gesture audit ([`gesture`]) over the frozen
//! arms: Round XII `perform_temporal` (the accepted listening baseline), Round XIII
//! `perform_mass`, Round XIIIb `perform_tension` (the rejected intervention). StablePropulsion,
//! seed 2112, `deflected_lift_trace(120)`, default options, SWISS_SIGNAL and BLACK_ICE.
//!
//! Writes `{world}_{arm}.identity.txt` (every rival run, every flipped slice spelled out) and
//! `{world}_{arm}.gesture.txt` (every note the analysis calls a slide, on its physical route),
//! and prints one summary line per arm.
//!
//! Run:
//!   cargo run --release --example heard_object_lab -- --out=target/humanmusic-r14
use gibson::audio::human_music::{
    composer::Composer,
    functor::{perform_mass, perform_temporal, perform_tension, Composition},
    gesture::GestureDiagnostics,
    identity::IdentityDiagnostics,
    performance::PerformanceOptions,
    semantic::deflected_lift_trace,
    song::SongMap,
    MusicWorld,
};
use std::path::Path;

fn identity(x: &Composition, world: &MusicWorld) -> IdentityDiagnostics {
    IdentityDiagnostics::measure(&x.score.notes, &x.perf.contexts, world, x.score.tempo_bpm)
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .iter()
        .find_map(|a| a.strip_prefix("--out="))
        .unwrap_or("target/humanmusic-r14")
        .to_string();
    let dir = Path::new(&out);
    std::fs::create_dir_all(dir)?;
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let opts = PerformanceOptions::default();
    let mut summary = String::new();
    for (name, world) in [
        ("swiss", MusicWorld::swiss_signal()),
        ("black_ice", MusicWorld::black_ice()),
    ] {
        let arms = [
            ("r12", perform_temporal(&song, &world, opts)),
            ("r13", perform_mass(&song, &world, opts)),
            ("r13b", perform_tension(&song, &world, opts)),
        ];
        for (arm, x) in &arms {
            let id = identity(x, &world);
            let g = GestureDiagnostics::measure(&x.perf, &x.score);
            let flips: Vec<String> = id
                .flips()
                .map(|r| {
                    format!(
                        "{:.2}-{:.2} s {} heard as {} ({:.3} s)",
                        id.secs(r.start_beat),
                        id.secs(r.end_beat),
                        r.chart.label(),
                        r.rival.label(),
                        r.secs
                    )
                })
                .collect();
            let line = format!(
                "{name:<9} {arm:<4} identity flips {} ({:.3} s): [{}]\n{name:<9} {arm:<4} {}\n",
                flips.len(),
                id.flipped_secs(),
                flips.join("; "),
                g.report(&x.score).lines().next().unwrap_or("")
            );
            print!("{line}");
            summary.push_str(&line);
            std::fs::write(
                dir.join(format!("{name}_{arm}.identity.txt")),
                id.report(&x.score.notes, &x.perf.contexts),
            )?;
            std::fs::write(
                dir.join(format!("{name}_{arm}.gesture.txt")),
                g.report(&x.score),
            )?;
        }
    }
    std::fs::write(dir.join("summary.txt"), summary)?;
    Ok(())
}
