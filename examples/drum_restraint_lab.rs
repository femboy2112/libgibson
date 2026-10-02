//! Drum restraint A/B: the same songs, the same plan and the same band — only the drummer's
//! percussion law changes (the historical unarbitrated drummer, then the arbitrated surface at
//! each restraint). Full mixes and drums-only stems. The lab proves the isolation before it
//! renders: every non-drum note is identical to the historical take.
#[path = "../tests/common/vapor95_v1.rs"]
mod vapor95_v1;
use gibson::audio::{
    human_music::{
        composer::Composer,
        contract::CompositionGrammar,
        functor::{perform_with_profile, Composition},
        percussion::DrumRestraint,
        performance::PerformanceOptions,
        policy::PerformanceProfile,
        score::DrumVoice,
        semantic::{deflected_lift_trace, demo_trace},
        synth::StemMask,
        HumanMusicSynth, MusicWorld, SongMap,
    },
    render::OfflineRenderer,
    wav::write_wav_i16,
    SampleRate,
};
use std::{fmt::Write, path::Path};

fn render(
    path: &Path,
    c: &Composition,
    world: &MusicWorld,
    mask: StemMask,
) -> Result<(), Box<dyn std::error::Error>> {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(&c.score, world, rate);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(rate, 256).render(&mut synth, frames);
    if audio.had_nonfinite {
        return Err("nonfinite render".into());
    }
    write_wav_i16(path, &audio.audio, rate)?;
    Ok(())
}

/// Strokes that add information around the pocket (not its anchors or time-line).
fn ornaments(c: &Composition) -> usize {
    c.score
        .drums
        .iter()
        .filter(|d| {
            d.voice == DrumVoice::OpenHat
                || matches!(
                    d.prov.groove_variation,
                    Some("ghost" | "unison" | "answer" | "fill")
                )
        })
        .count()
}

/// VAPOR95 as this run asks for it: the product world, or with `--vapor95=v1` the archival v1
/// world (`tests/common/vapor95_v1.rs`), which reproduces historical VAPOR95 audio and receipts.
fn vapor95_world() -> MusicWorld {
    if std::env::args().any(|a| a == "--vapor95=v1") {
        vapor95_v1::vapor95_v1()
    } else {
        MusicWorld::vapor95()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let out = Path::new(
        args.iter()
            .find_map(|a| a.strip_prefix("--out="))
            .unwrap_or("target/humanmusic-drums/restraint"),
    );
    std::fs::create_dir_all(out)?;
    let should_render = args.iter().any(|a| a == "--render");
    let songs = [
        (
            "generated_black_ice",
            SongMap::build(&demo_trace(64.0), 2112, Some(CompositionGrammar::HookArc)),
            MusicWorld::black_ice(),
        ),
        (
            "riffdrive_vapor95",
            SongMap::compose(
                &deflected_lift_trace(64.0),
                77_800_001,
                Some(CompositionGrammar::RiffDrive),
                Composer::StructuralR9,
            ),
            vapor95_world(),
        ),
    ];
    let mut arms: Vec<(String, PerformanceProfile)> =
        vec![("0_historical".into(), PerformanceProfile::POCKET)];
    for (i, r) in DrumRestraint::ALL.into_iter().enumerate() {
        arms.push((
            format!("{}_{}", i + 1, r.label()),
            PerformanceProfile::POCKET.with_drum_restraint(r),
        ));
    }
    let mut report = String::from(
        "Drum restraint A/B: same song, plan and band per row; only the drummer changes.\n\
         Question: does the drummer support the pocket and choose moments to speak?\n",
    );
    for (song_name, song, world) in &songs {
        writeln!(report, "\n== {song_name} ({})", world.name)?;
        let historical = perform_with_profile(
            song,
            world,
            PerformanceOptions::default(),
            PerformanceProfile::POCKET,
        )?;
        let band_notes = |c: &Composition| {
            c.score
                .notes
                .iter()
                .map(|n| {
                    (
                        n.start_beat.to_bits(),
                        n.dur_beats.to_bits(),
                        n.pitch,
                        n.role as u8,
                    )
                })
                .collect::<Vec<_>>()
        };
        for (arm, profile) in &arms {
            let c = perform_with_profile(song, world, PerformanceOptions::default(), *profile)?;
            let isolated = band_notes(&c) == band_notes(&historical);
            if !isolated {
                return Err(format!("{song_name} {arm}: a non-drum note changed").into());
            }
            let admitted =
                c.score
                    .percussion
                    .as_ref()
                    .map_or("historical: unarbitrated".to_string(), |p| {
                        format!(
                            "ornaments admitted {}/{} offered, required {}",
                            p.admitted_ornaments(),
                            p.offered_ornaments(),
                            p.required
                        )
                    });
            writeln!(
                report,
                "  {arm}: strokes {}, ornament strokes {}; {admitted}; band notes identical to historical: {isolated}",
                c.score.drums.len(),
                ornaments(&c)
            )?;
            if let Some(p) = &c.score.percussion {
                std::fs::write(
                    out.join(format!("{song_name}.{arm}.percussion.txt")),
                    p.report(),
                )?;
            }
            if should_render {
                render(
                    &out.join(format!("{song_name}.{arm}.wav")),
                    &c,
                    world,
                    StemMask::full(),
                )?;
                render(
                    &out.join(format!("{song_name}.{arm}.drums.wav")),
                    &c,
                    world,
                    StemMask::solo("drums"),
                )?;
            }
        }
    }
    std::fs::write(out.join("report.txt"), &report)?;
    print!("{report}");
    Ok(())
}
