//! HumanMusic proving ground.
//!
//! Renders ONE frozen semantic event fixture through three MusicWorlds — BLACK_ICE,
//! VAPOR95, SWISS_SIGNAL — and writes an offline WAV per world plus a structural dump
//! (form, harmony, motif development, groove summary, provenance) and render receipts.
//!
//! The point: the three tracks are recognizably the *same music* in FORM and meaning
//! (same section skeleton, same motif identity, same resolutions) yet clearly different
//! musical dialects. Machines can certify the structure and safety; taste is for your ears
//! — the WAVs are the artifact to listen to.
//!
//! Run:
//!   cargo run --release --example human_music_lab
//!   cargo run --release --example human_music_lab -- --world=vapor95 --out=/tmp/hm
//!   cargo run --release --example human_music_lab -- --beats=96 --seed=2112

use std::path::PathBuf;

use gibson::audio::buffer::StereoBlock;
use gibson::audio::human_music::contract::CompositionGrammar;
use gibson::audio::human_music::diagnostics::{
    CoherenceDiagnostics, DiscourseDiagnostics, RealizationDiagnostics,
};
use gibson::audio::human_music::functor::{compose_with_grammar, compose_with_plan};
use gibson::audio::human_music::semantic::{calm_loop, rise_unresolved};
use gibson::audio::human_music::synth::HumanMusicSynth;
use gibson::audio::human_music::{demo_trace, MusicWorld, WorldId};
use gibson::audio::render::OfflineRenderer;
use gibson::audio::wav::write_wav_i16;
use gibson::audio::SampleRate;

fn arg(flag: &str) -> Option<String> {
    std::env::args()
        .skip(1)
        .find_map(|a| a.strip_prefix(flag).map(|s| s.to_string()))
}

fn main() -> std::io::Result<()> {
    let seed: u64 = arg("--seed=").and_then(|s| s.parse().ok()).unwrap_or(2112);
    let beats: f64 = arg("--beats=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(120.0);
    let which = arg("--world=").unwrap_or_else(|| "all".into());
    let out_dir = arg("--out=")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("libgibson_human_music"));
    std::fs::create_dir_all(&out_dir)?;

    let sr = SampleRate::STUDIO;
    let block = 512;

    // Calibration mode: render the four grammar probes, each exercising a different definition of
    // musical direction, and dump their plan + coherence + discourse diagnostics.
    if std::env::args().any(|a| a == "--calibrate") {
        return calibrate(&out_dir, sr, block, seed);
    }

    let trace = demo_trace(beats);

    let worlds: Vec<WorldId> = match which.as_str() {
        "black_ice" => vec![WorldId::BlackIce],
        "vapor95" => vec![WorldId::Vapor95],
        "swiss_signal" => vec![WorldId::SwissSignal],
        _ => vec![WorldId::BlackIce, WorldId::Vapor95, WorldId::SwissSignal],
    };

    println!(
        "HumanMusic lab — one semantic trace, {} events, {beats:.0} beats",
        trace.events.len()
    );
    println!(
        "seed={seed}  sr={} Hz  out={}\n",
        sr.get(),
        out_dir.display()
    );

    for id in worlds {
        let world = MusicWorld::from_id(id);
        let file_stem = world.name.to_lowercase();
        let path = out_dir.join(format!("{file_stem}.wav"));

        let (score, plan) = compose_with_plan(&trace, &world, seed);
        score.validate().expect("score invariants");

        let mut synth = HumanMusicSynth::new(&score, &world, sr);
        let frames = synth.total_samples();
        let t0 = std::time::Instant::now();
        let out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
        let render_wall = t0.elapsed();

        write_wav_i16(&path, &out.audio, sr)?;

        let real_secs = out.duration().as_secs_f64();
        let (ldc, rdc) = out.audio.dc_offset();
        println!(
            "=== {} =========================================",
            world.name
        );
        print!("{}", score.summary());
        print!("{}", plan.dump());
        print!("{}", CoherenceDiagnostics::measure(&plan, &score).report());
        print!("{}", DiscourseDiagnostics::measure(&plan, &score).report());
        println!(
            "{}",
            RealizationDiagnostics::measure(&plan, &score).report()
        );
        println!(
            "render: {real_secs:.1}s audio in {:.0}ms  ({:.1}x realtime)  peak={:.3} rms={:.3} dc=({:.4},{:.4})",
            render_wall.as_secs_f64() * 1000.0,
            real_secs / render_wall.as_secs_f64().max(1e-6),
            out.peak,
            out.rms,
            ldc,
            rdc,
        );
        println!(
            "safety: nonfinite={}  clipped_blocks={}  max_voices={}  worst_block={:.2}ms",
            out.had_nonfinite,
            out.reports.iter().filter(|r| r.clipped()).count(),
            out.max_active_voices,
            out.worst_block_time().as_secs_f64() * 1000.0,
        );
        println!("wav: {}\n", path.display());
    }

    println!("Listen to the WAVs above. Same form + motif + resolutions, three dialects.");
    Ok(())
}

/// Render the four grammar calibration probes. Each is a deterministic composition constructed to
/// exercise a *different* definition of musical direction:
///   - hook_arc: a directed arc with functional cadences (the resolved demo).
///   - loop_evolution: a calm loop — meaning comes from evolution, not cadences (cyclic harmony).
///   - riff_drive: a rising, unresolved trace over a modal pedal (harmonic stasis is allowed).
///   - world_switch: two locally coherent regimes (two sonic dialects) of the SAME composition
///     plan, joined — the transported identity is the form + discourse skeleton and the seed.
fn calibrate(
    out_dir: &std::path::Path,
    sr: SampleRate,
    block: usize,
    seed: u64,
) -> std::io::Result<()> {
    println!(
        "HumanMusic calibration — four grammar probes\nseed={seed}  out={}\n",
        out_dir.display()
    );

    let cases = [
        (
            "hook_arc",
            CompositionGrammar::HookArc,
            demo_trace(120.0),
            WorldId::BlackIce,
        ),
        (
            "loop_evolution",
            CompositionGrammar::LoopEvolution,
            calm_loop(120.0),
            WorldId::Vapor95,
        ),
        (
            "riff_drive",
            CompositionGrammar::RiffDrive,
            rise_unresolved(120.0),
            WorldId::SwissSignal,
        ),
    ];
    for (name, grammar, trace, wid) in cases {
        let world = MusicWorld::from_id(wid);
        let (score, plan) = compose_with_grammar(&trace, &world, seed, grammar);
        score.validate().expect("calibration score invariants");
        let mut synth = HumanMusicSynth::new(&score, &world, sr);
        let frames = synth.total_samples();
        let out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
        let path = out_dir.join(format!("calib_{name}.wav"));
        write_wav_i16(&path, &out.audio, sr)?;

        println!("=== {name}  ({grammar:?}, {}) ===", world.name);
        print!("{}", plan.dump());
        print!("{}", CoherenceDiagnostics::measure(&plan, &score).report());
        print!("{}", DiscourseDiagnostics::measure(&plan, &score).report());
        println!(
            "{}",
            RealizationDiagnostics::measure(&plan, &score).report()
        );
        println!(
            "safety: nonfinite={}  peak={:.3}  max_voices={}\nwav: {}\n",
            out.audio.has_nonfinite(),
            out.peak,
            out.max_active_voices,
            path.display(),
        );
    }

    // world_switch: the same composition plan realized in TWO sonic dialects, joined. The
    // transported identity across the switch is the form + discourse skeleton (world-independent)
    // and the seed; regime A is BLACK_ICE, regime B is VAPOR95.
    let trace = demo_trace(120.0);
    let (a_world, b_world) = (MusicWorld::black_ice(), MusicWorld::vapor95());
    let (a_score, plan) =
        compose_with_grammar(&trace, &a_world, seed, CompositionGrammar::WorldSwitch);
    let (b_score, _) =
        compose_with_grammar(&trace, &b_world, seed, CompositionGrammar::WorldSwitch);
    a_score.validate().expect("world-switch A invariants");
    b_score.validate().expect("world-switch B invariants");

    let mut sa = HumanMusicSynth::new(&a_score, &a_world, sr);
    let fa = sa.total_samples();
    let ra = OfflineRenderer::new(sr, block).render(&mut sa, fa);
    let mut sb = HumanMusicSynth::new(&b_score, &b_world, sr);
    let fb = sb.total_samples();
    let rb = OfflineRenderer::new(sr, block).render(&mut sb, fb);

    let mut combined = StereoBlock::new(0);
    ra.audio.append_to(&mut combined);
    rb.audio.append_to(&mut combined);
    let path = out_dir.join("calib_world_switch.wav");
    write_wav_i16(&path, &combined, sr)?;

    println!("=== world_switch  (BLACK_ICE → VAPOR95, transported plan) ===");
    print!("{}", plan.dump());
    println!(
        "transport: same discourse plan + seed, two dialects joined at the switch\nnonfinite={}  wav: {}\n",
        combined.has_nonfinite(),
        path.display(),
    );

    println!("Four probes rendered. Each exercises a different definition of musical direction.");
    Ok(())
}
