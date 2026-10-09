//! **Rick-C137 R0 — baseline diagnostic** (no mechanism, no hand-authored notes).
//!
//! Renders the GENERATOR'S OWN output — `SemanticTrace → SongMap::compose → perform` — through two
//! profiles, so the maintainer can hear where the engine ALREADY stands before any C137 rung is
//! built:
//!
//! * **WRITTEN** — `perform()`, the byte-exact v0.4 default route (`PerformanceProfile::WRITTEN`).
//! * **BAND** — `perform_with_profile(.., PerformanceProfile::BAND)`, the hardened opt-in route
//!   ("new work starts here"): rehearsed verb admission, a balanced drummer, the world/language
//!   harmonic vocabulary, support functions earned where they sound. `perform_checked` reports
//!   whether that take passes its own performance receipt (a REJECT is itself an R0 finding —
//!   the candidate is still rendered so it can be heard).
//!
//! One generated song (world-independent `compose`) rendered through each product world. This is a
//! measurement, NOT a proposal: it tells us which GEN rungs are real work and which are already
//! climbed. The probe (`rick_probe.rs`) proved the music is reachable by hand; R0 asks how far the
//! generator already reaches on its own.
//!
//!   cargo run --release --example rick_r0 -- --out=target/humanmusic-beefup/rick_r0
//!
//! Flags: `--beats=NNN` (song length in beats, default 96), `--world=black_ice|vapor95|swiss_signal`
//! (default all three), `--out=`. Byte-identical renders; not shipped (a diagnostic on the beef-up
//! branch, excluded from the published crate).

use gibson::audio::{
    human_music::{
        composer::Composer,
        functor::{perform, perform_checked, perform_with_profile, Composition},
        performance::PerformanceOptions,
        policy::PerformanceProfile,
        semantic::deflected_lift_trace,
        synth::StemMask,
        HumanMusicSynth, MusicWorld, SongMap,
    },
    render::OfflineRenderer,
    wav::write_wav_i16,
    SampleRate,
};
use std::{fmt::Write as _, path::Path};

/// Render a composition's full mix to a WAV and return a one-line level receipt.
fn render(
    path: &Path,
    c: &Composition,
    world: &MusicWorld,
) -> Result<String, Box<dyn std::error::Error>> {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(&c.score, world, rate);
    synth.set_stem_mask(StemMask::full());
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(rate, 256).render(&mut synth, frames);
    if audio.had_nonfinite {
        return Err("nonfinite render".into());
    }
    write_wav_i16(path, &audio.audio, rate)?;
    Ok(format!(
        "-> {} peak={:.3} rms={:.3} frames={}",
        path.display(),
        audio.peak,
        audio.rms,
        audio.audio.frames()
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |p: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(p).map(str::to_string))
    };

    let out =
        Path::new(&arg("--out=").unwrap_or_else(|| "target/humanmusic-beefup/rick_r0".to_string()))
            .to_path_buf();
    std::fs::create_dir_all(&out)?;
    let beats: f64 = arg("--beats=").and_then(|s| s.parse().ok()).unwrap_or(96.0);
    let seed: u64 = 2112;

    // The generator's own song, composed ONCE (compose is world-independent).
    let trace = deflected_lift_trace(beats);
    let song = SongMap::compose(&trace, seed, None, Composer::StablePropulsion);

    let all = [
        ("black_ice", MusicWorld::black_ice()),
        ("vapor95", MusicWorld::vapor95()),
        ("swiss_signal", MusicWorld::swiss_signal()),
    ];
    let want = arg("--world=");
    let worlds: Vec<_> = all
        .into_iter()
        .filter(|(n, _)| want.as_deref().map(|w| w == *n).unwrap_or(true))
        .collect();

    let mut summary = String::new();
    writeln!(
        summary,
        "Rick-C137 R0 baseline — the GENERATOR's own output, WRITTEN (v0.4) vs BAND (opt-in).\n\
         trace=deflected_lift_trace({beats}) seed={seed} composer=StablePropulsion\n\
         song_fingerprint={:016x}",
        song.fingerprint()
    )?;

    for (wname, w) in &worlds {
        let opts = PerformanceOptions::default();

        // WRITTEN — the byte-exact v0.4 route (infallible).
        let written = perform(&song, w, opts);
        let wline = render(&out.join(format!("{wname}_written.wav")), &written, w)?;
        writeln!(
            summary,
            "\n[{wname}] WRITTEN  score={:016x} perf={:016x} notes={}\n          {wline}",
            written.score.fingerprint(),
            written.perf.fingerprint(),
            written.score.notes.len()
        )?;

        // BAND — the hardened opt-in route; render the candidate even if the checked receipt rejects.
        match perform_with_profile(&song, w, opts, PerformanceProfile::BAND) {
            Ok(band) => {
                let receipt = if perform_checked(&song, w, opts, PerformanceProfile::BAND).is_ok() {
                    "PASS"
                } else {
                    "REJECT (candidate still rendered)"
                };
                let bline = render(&out.join(format!("{wname}_band.wav")), &band, w)?;
                writeln!(
                    summary,
                    "[{wname}] BAND     score={:016x} perf={:016x} notes={} receipt={receipt}\n          {bline}",
                    band.score.fingerprint(),
                    band.perf.fingerprint(),
                    band.score.notes.len()
                )?;
            }
            Err(e) => writeln!(
                summary,
                "[{wname}] BAND     REFUSED before realize: {}",
                e.0
            )?,
        }
    }

    std::fs::write(out.join("summary.txt"), &summary)?;
    print!("{summary}");
    Ok(())
}
