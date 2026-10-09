//! **Rick-C137 GEN-3 — the alive lead A/B matrix.**
//!
//! The SAME generated song as R0 (`deflected_lift_trace(96)`, seed 2112, `StablePropulsion`),
//! rendered through `PerformanceProfile::BAND` and then BAND with each opt-in lead-life axis turned
//! on in ISOLATION, so the maintainer's ear can tell which one removes the "robotic" quality (the
//! drivers — content repetition vs metronomic spacing — are confounded, so they are heard apart):
//!
//! * `band_baseline` — BAND, every lead-life axis off: the robotic lead heard in R0.
//! * `dev`      — only `development`: the motif's RHYTHM developed per statement (syncopation/pickups
//!   from the accent grid), contour + length preserved.
//! * `spacing`  — only `spacing`: statements enter at varied points in the phrase instead of hugging
//!   its start every two bars.
//! * `dynamics` — only `dynamics`: a per-note dynamic arc on the lead (phrase contour + metric weight).
//! * `alive`    — all three at once.
//!
//! Every axis off is byte-exact v0.4 (guarded by the `gen3_lead_life_is_byte_exact_off_and_active_on`
//! unit test). Structured/relational only, no onset jitter; byte-identical renders.
//!
//!   cargo run --release --example rick_gen3 -- --out=target/humanmusic-beefup/rick_gen3
//!
//! Flags: `--beats=NNN` (default 96, matches R0), `--world=black_ice|vapor95|swiss_signal` (default
//! black_ice, the judged voice), `--out=`. Not shipped (a diagnostic on the beef-up branch).

use gibson::audio::{
    human_music::{
        composer::Composer,
        functor::{perform_with_profile, Composition},
        performance::PerformanceOptions,
        policy::{LeadLifePolicy, PerformanceProfile},
        semantic::deflected_lift_trace,
        synth::StemMask,
        HumanMusicSynth, MusicWorld, SongMap,
    },
    render::OfflineRenderer,
    wav::write_wav_i16,
    SampleRate,
};
use std::{fmt::Write as _, path::Path};

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
        "peak={:.3} rms={:.3} frames={}",
        audio.peak,
        audio.rms,
        audio.audio.frames()
    ))
}

/// The number of lead notes and their distinct inter-onset intervals (a coarse "is the rhythm
/// actually varied" witness — the ear still decides).
fn lead_shape(c: &Composition) -> String {
    use gibson::audio::human_music::score::Role;
    let mut starts: Vec<f64> = c
        .score
        .notes
        .iter()
        .filter(|n| n.role == Role::Lead)
        .map(|n| n.start_beat)
        .collect();
    starts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut iois: Vec<i64> = starts
        .windows(2)
        .map(|w| ((w[1] - w[0]) * 100.0).round() as i64)
        .filter(|&d| d > 0)
        .collect();
    iois.sort_unstable();
    iois.dedup();
    let vels: std::collections::BTreeSet<i64> = c
        .score
        .notes
        .iter()
        .filter(|n| n.role == Role::Lead)
        .map(|n| (n.velocity * 1000.0).round() as i64)
        .collect();
    format!(
        "lead notes={} distinct_IOIs={} distinct_vel={}",
        starts.len(),
        iois.len(),
        vels.len()
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |p: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(p).map(str::to_string))
    };

    let out = Path::new(
        &arg("--out=").unwrap_or_else(|| "target/humanmusic-beefup/rick_gen3".to_string()),
    )
    .to_path_buf();
    std::fs::create_dir_all(&out)?;
    let beats: f64 = arg("--beats=").and_then(|s| s.parse().ok()).unwrap_or(96.0);

    let song = SongMap::compose(
        &deflected_lift_trace(beats),
        2112,
        None,
        Composer::StablePropulsion,
    );
    let wname = arg("--world=").unwrap_or_else(|| "black_ice".to_string());
    let world = match wname.as_str() {
        "vapor95" => MusicWorld::vapor95(),
        "swiss_signal" => MusicWorld::swiss_signal(),
        _ => MusicWorld::black_ice(),
    };

    let only = |l: LeadLifePolicy| PerformanceProfile::BAND.with_lead_life(l);
    let arms: [(&str, PerformanceProfile); 6] = [
        ("band_baseline", PerformanceProfile::BAND),
        (
            "dev",
            only(LeadLifePolicy {
                development: true,
                ..LeadLifePolicy::default()
            }),
        ),
        (
            "spacing",
            only(LeadLifePolicy {
                spacing: true,
                ..LeadLifePolicy::default()
            }),
        ),
        (
            "dynamics",
            only(LeadLifePolicy {
                dynamics: true,
                ..LeadLifePolicy::default()
            }),
        ),
        (
            "dev_dyn",
            only(LeadLifePolicy {
                development: true,
                dynamics: true,
                ..LeadLifePolicy::default()
            }),
        ),
        ("alive", only(LeadLifePolicy::ALIVE)),
    ];

    let mut summary = format!(
        "Rick-C137 GEN-3 — alive-lead A/B. world={} song={:016x} beats={beats}\n",
        world.name,
        song.fingerprint()
    );
    for (name, profile) in arms {
        let c = perform_with_profile(&song, &world, PerformanceOptions::default(), profile)?;
        let line = render(&out.join(format!("{wname}_{name}.wav")), &c, &world)?;
        writeln!(
            summary,
            "[{name:<14}] score={:016x} {}  {line}",
            c.score.fingerprint(),
            lead_shape(&c)
        )?;
    }
    std::fs::write(out.join("summary.txt"), &summary)?;
    print!("{summary}");
    Ok(())
}
