//! Round XII: independent audit of the unchanged Round XI specimen.
use gibson::audio::human_music::{
    composer::Composer,
    contract::CompositionGrammar,
    diagnostics::RealizationDiagnostics,
    functor::perform,
    performance::PerformanceOptions,
    phenomenal::PhenomenalTrajectory,
    score::Score,
    semantic::deflected_lift_trace,
    song::{SongMap, SongMapConformance},
    temporal::TemporalPitchDiagnostics,
    theory::note_name,
    MusicWorld,
};
use std::{fmt::Write, path::Path};

fn notes(score: &Score) -> String {
    let mut out =
        String::from("index\tbeat\tduration\trole\tpitch\tname\tfunction\tmaterial\trole_note\n");
    for (i, n) in score.notes.iter().enumerate() {
        let _ = writeln!(
            out,
            "{i}\t{:.17}\t{:.9}\t{}\t{}\t{}\t{:?}\t{:?}\t{}",
            n.start_beat,
            n.dur_beats,
            n.role.label(),
            n.pitch,
            note_name(n.pitch),
            n.function,
            n.prov.material,
            n.prov.role_note
        );
    }
    out
}
fn main() -> std::io::Result<()> {
    let out = std::env::args()
        .find_map(|a| a.strip_prefix("--out=").map(str::to_owned))
        .unwrap_or_else(|| "target/humanmusic-r12/baseline".into());
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
            let c = perform(&song, &world, opts);
            let old = RealizationDiagnostics::measure(&song.plan, &c.score);
            let audit = TemporalPitchDiagnostics::measure(&c.perf, &c.score);
            let law = SongMapConformance::check(&song, &c.perf, &c.score);
            assert!(law.passes(), "{}", law.report());
            let header = format!("regime={regime} world={name} seed=2112 beats=120 options={opts:?}\nsong={:#018x} performance={:#018x} score={:#018x}\nold_unjustified={:?}\n{}\n",song.fingerprint(),c.perf.fingerprint(),c.score.fingerprint(),old.unjustified_by_role,law.report());
            println!("{header}{}", audit.report());
            let stem = format!("{name}_{regime}_r11");
            std::fs::write(dir.join(format!("{stem}.notes.tsv")), notes(&c.score))?;
            std::fs::write(
                dir.join(format!("{stem}.audit.txt")),
                format!("{header}{}", audit.report()),
            )?;
            std::fs::write(
                dir.join(format!("{stem}.score.txt")),
                format!("{:?}", c.score),
            )?;
            std::fs::write(
                dir.join(format!("{regime}.trajectory.txt")),
                PhenomenalTrajectory::observe(&song).report(),
            )?;
        }
    }
    Ok(())
}
