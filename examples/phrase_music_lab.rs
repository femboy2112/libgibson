//! Round XVI listening lab: historical arms, decomposed diagnostics, isolated role surfaces.
use gibson::audio::{
    human_music::{
        composer::Composer,
        expression::ExpressionDiagnostics,
        functor::{perform_coherent, perform_expressive, Composition},
        identity::IdentityDiagnostics,
        performance::PerformanceOptions,
        phrase_diagnostics::PhraseSurfaceDiagnostics,
        semantic::deflected_lift_trace,
        song::{SongMap, SongMapConformance},
        synth::StemMask,
        temporal::TemporalPitchDiagnostics,
        voicing_diagnostics::VoicingSurfaceDiagnostics,
        witness, HumanMusicSynth, MusicWorld,
    },
    render::OfflineRenderer,
    wav::write_wav_i16,
    SampleRate,
};
use std::{fmt::Write, path::Path};

fn render(
    dir: &Path,
    name: &str,
    c: &Composition,
    w: &MusicWorld,
    mask: StemMask,
) -> std::io::Result<String> {
    let sr = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(&c.score, w, sr);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(sr, 256).render(&mut synth, frames);
    assert!(!audio.had_nonfinite);
    let path = dir.join(format!("{name}.wav"));
    write_wav_i16(&path, &audio.audio, sr)?;
    Ok(format!(
        "{} frames={} peak={} rms={}\n",
        path.display(),
        audio.audio.frames(),
        audio.peak,
        audio.rms
    ))
}
fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .iter()
        .find_map(|s| s.strip_prefix("--out="))
        .unwrap_or("target/humanmusic-r16/final");
    let dir = Path::new(out);
    std::fs::create_dir_all(dir)?;
    let song = SongMap::compose(
        &deflected_lift_trace(120.0),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let mut summary = format!(
        "StablePropulsion seed=2112 FusionConversation default options song={:016x}\n",
        song.fingerprint()
    );
    let mut renders = String::new();
    for (name, w) in [
        ("black_ice", MusicWorld::black_ice()),
        ("swiss", MusicWorld::swiss_signal()),
    ] {
        let r14 = perform_coherent(&song, &w, PerformanceOptions::default());
        let r15 = perform_expressive(&song, &w, PerformanceOptions::default());
        for (arm, c) in [("r14", &r14), ("r15", &r15)] {
            let prefix = format!("{name}_{arm}");
            let id =
                IdentityDiagnostics::measure(&c.score.notes, &c.perf.contexts, &w, w.tempo_bpm);
            let temporal = TemporalPitchDiagnostics::measure(&c.perf, &c.score);
            let actions = witness::audit(&c.perf, &c.score);
            writeln!(
                summary,
                "{prefix} score={:016x} notes={} flips={} stale={} temporal_false={} actions={}/{}",
                c.score.fingerprint(),
                c.score.notes.len(),
                id.flips().count(),
                c.score.stale_hearings().len(),
                temporal.false_function_claims,
                actions.rows.iter().filter(|a| a.witnessed).count(),
                actions.rows.len()
            )
            .unwrap();
            for (suffix, data) in [
                (
                    "phrase",
                    PhraseSurfaceDiagnostics::measure(&c.song.plan, &c.perf, &c.score, &w).report(),
                ),
                (
                    "voicing",
                    VoicingSurfaceDiagnostics::measure(&c.perf, &c.score, &w).report(),
                ),
                (
                    "notes",
                    c.score
                        .notes
                        .iter()
                        .map(|n| format!("{n:?}\n"))
                        .collect::<String>(),
                ),
                ("drums", format!("{:#?}", c.score.drums)),
                (
                    "expression",
                    ExpressionDiagnostics::measure(&c.perf, &c.score, &w).report(w.tempo_bpm),
                ),
                ("identity", id.report(&c.score.notes, &c.perf.contexts)),
                (
                    "conformance",
                    SongMapConformance::check(&song, &c.perf, &c.score).report(),
                ),
                (
                    "hearings",
                    format!(
                        "{:?}\nstale={:?}\n",
                        c.score.hearings,
                        c.score.stale_hearings()
                    ),
                ),
                ("actions", format!("{:#?}", actions.rows)),
            ] {
                std::fs::write(dir.join(format!("{prefix}.{suffix}.txt")), data)?;
            }
            if args.iter().any(|s| s == "--render") {
                let masks = [
                    ("full", StemMask::full()),
                    ("lead", StemMask::solo("lead")),
                    ("bass", StemMask::solo("bass")),
                    ("keys", StemMask::solo("keys")),
                    ("pad", StemMask::solo("pad")),
                    (
                        "lead_bass",
                        StemMask {
                            lead: true,
                            bass: true,
                            ..StemMask::silent()
                        },
                    ),
                    (
                        "support",
                        StemMask {
                            pad: true,
                            keys: true,
                            bass: true,
                            ..StemMask::silent()
                        },
                    ),
                    (
                        "rhythm",
                        StemMask {
                            keys: true,
                            bass: true,
                            drums: true,
                            ..StemMask::silent()
                        },
                    ),
                ];
                for (stem, mask) in masks {
                    renders.push_str(&render(dir, &format!("{prefix}.{stem}"), c, &w, mask)?);
                }
            }
        }
    }
    std::fs::write(dir.join("summary.txt"), &summary)?;
    if !renders.is_empty() {
        std::fs::write(dir.join("render.txt"), &renders)?;
    }
    print!("{summary}{renders}");
    Ok(())
}
