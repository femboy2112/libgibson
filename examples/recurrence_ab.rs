//! **Recurrence dial listening renders** (Wave 3, §16).
//!
//! The same trace / world / seed / grammar, rendered at each position of the `motif_repetition`
//! dial — `Develop` (the v0.4 control), `Return`, `Restate`. Only the song-level recurrence policy
//! changes; the world, the seed and the performance are held fixed. For each take we print the
//! **SongMap fingerprint** (it must change when the dial changes the song's identity, §33) and how
//! many theme sites restate the thesis, then write a stereo WAV + a sidecar receipt.
//!
//! These are **listening artifacts**: the numbers only say whether the structure changed, never
//! whether a stronger return reads as "the song coming home." The maintainer's ear decides that.
//!
//!   cargo run --release --example recurrence_ab -- --out=target/humanmusic-beefup/recurrence
//!
//! Flags: `--seed=`, `--beats=`, `--world=black_ice|vapor95|swiss_signal`, `--out=`.
//! Not shipped in the library; a listening instrument on the beef-up branch.

use std::fmt::Write as _;
use std::path::PathBuf;

use gibson::audio::human_music::contract::CompositionGrammar;
use gibson::audio::human_music::motif::Handoff;
use gibson::audio::human_music::semantic::demo_trace;
use gibson::audio::human_music::{
    render_with_options, CompositionOptions, MotifRepetition, MusicWorld, SongMap, WorldId,
};
use gibson::audio::wav::write_wav_f32;
use gibson::audio::{SampleRate, StereoBlock};

fn arg(prefix: &str) -> Option<String> {
    std::env::args()
        .find(|a| a.starts_with(prefix))
        .map(|a| a[prefix.len()..].to_string())
}

fn world_from(name: &str) -> (WorldId, &'static str) {
    match name {
        "vapor95" => (WorldId::Vapor95, "VAPOR95"),
        "swiss_signal" => (WorldId::SwissSignal, "SWISS_SIGNAL"),
        _ => (WorldId::BlackIce, "BLACK_ICE"),
    }
}

fn main() -> std::io::Result<()> {
    let seed: u64 = arg("--seed=").and_then(|s| s.parse().ok()).unwrap_or(2112);
    let beats: f64 = arg("--beats=").and_then(|s| s.parse().ok()).unwrap_or(96.0);
    let (world_id, world_name) = world_from(&arg("--world=").unwrap_or_default());
    let out = PathBuf::from(
        arg("--out=").unwrap_or_else(|| "target/humanmusic-beefup/recurrence".into()),
    );
    std::fs::create_dir_all(&out)?;

    let sr = SampleRate::STUDIO;
    let block = 1024usize;
    let world = MusicWorld::from_id(world_id);
    // A Motif-anchored grammar, so the dial actually governs recurrence (riff/loop grammars carry
    // their recurrence on a different anchor and the dial is a no-op there). DeflectedLift's small
    // returning harmonic loop also lets a scale-degree restatement land more literally.
    let grammar = CompositionGrammar::DeflectedLift;
    let trace = demo_trace(beats);

    let mut summary = String::new();
    let _ = writeln!(
        summary,
        "recurrence dial — world={world_name} seed={seed} beats={beats} grammar=DeflectedLift sr={}",
        sr.get()
    );

    for (label, rep) in [
        ("develop", MotifRepetition::Develop),
        ("return", MotifRepetition::Return),
        ("restate", MotifRepetition::Restate),
    ] {
        let opts = CompositionOptions {
            grammar: Some(grammar),
            motif_repetition: rep,
            ..Default::default()
        };
        let (score, res) = render_with_options(&trace, &world, seed, sr, block, opts);
        // Same song, read back for its structural signatures (deterministic in the same inputs).
        let song = SongMap::build_with_options(&trace, seed, Some(grammar), rep);
        let fp = song.fingerprint();
        let sites = song.thematic.sites.len();
        let restated = song
            .thematic
            .sites
            .iter()
            .filter(|s| s.handoff == Handoff::Restatement)
            .count();

        let frames = res.audio.frames();
        let stereo = StereoBlock {
            left: res.audio.left.clone(),
            right: res.audio.right.clone(),
        };
        let wav = out.join(format!("recurrence_{label}.wav"));
        write_wav_f32(&wav, &stereo, sr)?;

        let line = format!(
            "  {label:8} fingerprint={fp:016x} sites={sites} restated={restated} notes={} drums={} frames={frames}",
            score.notes.len(),
            score.drums.len(),
        );
        println!("{line}");
        let _ = writeln!(summary, "{line}\n    wav={}", wav.display());

        let receipt = out.join(format!("recurrence_{label}.receipt.txt"));
        std::fs::write(
            &receipt,
            format!(
                "take {label}: motif_repetition={rep:?}\nworld={world_name} seed={seed} beats={beats} grammar=DeflectedLift\n\
                 SongMap fingerprint(fnv1a)={fp:016x}\ntheme sites={sites}, restating the thesis={restated}\n\
                 score: {} notes, {} drums; render {frames} frames\nwav={}\n",
                score.notes.len(),
                score.drums.len(),
                wav.display(),
            ),
        )?;
    }

    std::fs::write(out.join("SUMMARY.txt"), &summary)?;
    println!("wrote {}", out.join("SUMMARY.txt").display());
    Ok(())
}
