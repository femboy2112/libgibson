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
    CoherenceDiagnostics, DiscourseDiagnostics, LeadOutlineDiagnostics, RealizationDiagnostics,
};
use gibson::audio::human_music::functor::compose_with_grammar;
use gibson::audio::human_music::harmony::ChordSpan;
use gibson::audio::human_music::semantic::{
    calm_loop, deflected_lift_trace, rise_unresolved, SemanticTrace,
};
use gibson::audio::human_music::synth::{HumanMusicSynth, StemMask};
use gibson::audio::human_music::{demo_trace, MusicWorld, WorldId};
use gibson::audio::render::OfflineRenderer;
use gibson::audio::wav::write_wav_i16;
use gibson::audio::SampleRate;

fn arg(flag: &str) -> Option<String> {
    std::env::args()
        .skip(1)
        .find_map(|a| a.strip_prefix(flag).map(|s| s.to_string()))
}

/// Select the semantic story fixture by name. The flagship default is the DeflectedLift *bounce*
/// (`deflected_lift_trace`), so the default listen finally matches the default grammar instead of
/// fighting the old single-arc `demo_trace` — pass `--story=cinematic` for that stress test.
fn story_trace(name: &str, beats: f64) -> SemanticTrace {
    match name {
        "cinematic" | "demo" | "arc" => demo_trace(beats),
        "calm" => calm_loop(beats),
        "rise" | "unresolved" => rise_unresolved(beats),
        _ => deflected_lift_trace(beats),
    }
}

/// Print the DeflectedLift harmonic spine from the realized chords, grouped into cycles, so a
/// listener can point to reach -> miss -> open -> reset directly in the Score dump.
fn print_spine(chords: &[ChordSpan]) {
    let tagged: Vec<&ChordSpan> = chords
        .iter()
        .filter(|c| matches!(c.note, "lift" | "deflect" | "open" | "reset"))
        .collect();
    if tagged.is_empty() {
        return;
    }
    println!("harmonic spine (DeflectedLift cell — reach -> miss -> open -> reset):");
    for (cyc, chunk) in tagged.chunks(4).enumerate().take(3) {
        let parts: Vec<String> = chunk
            .iter()
            .map(|c| format!("{}={}", c.note, c.chord.label()))
            .collect();
        println!("  cycle {cyc}: {}", parts.join("  "));
    }
}

fn main() -> std::io::Result<()> {
    let seed: u64 = arg("--seed=").and_then(|s| s.parse().ok()).unwrap_or(2112);
    let beats: f64 = arg("--beats=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(120.0);
    let which = arg("--world=").unwrap_or_else(|| "all".into());
    // The canonical audition now orbits the DeflectedLift bounce (a small cyclic harmonic identity
    // and a recurring hook). Pass --grammar=hookarc (or loop / riff) to A/B against the earlier
    // cinematic-arc song.
    let grammar = match arg("--grammar=").as_deref() {
        Some("hookarc") => CompositionGrammar::HookArc,
        Some("loop") | Some("loop_evolution") => CompositionGrammar::LoopEvolution,
        Some("riff") | Some("riff_drive") => CompositionGrammar::RiffDrive,
        Some("deflected") | Some("deflected_lift") | Some("bounce") => {
            CompositionGrammar::DeflectedLift
        }
        _ => CompositionGrammar::DeflectedLift,
    };
    // The semantic story fixture (default: the DeflectedLift bounce, so the flagship listen matches
    // the flagship grammar). Pass --story=cinematic to render the old single-arc demo as a stress test.
    let story = arg("--story=").unwrap_or_else(|| "bounce".into());
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

    // Stem-isolation mode: render each bus of ONE world alone (plus the full mix) so a bad tone
    // can be pinned to a specific voice family — "the wrong pitch is in bass at bar N" instead of
    // "something sounds fucky". Rust-only debug surface; defaults to BLACK_ICE.
    if std::env::args().any(|a| a == "--stems") {
        let wid = match which.as_str() {
            "vapor95" => WorldId::Vapor95,
            "swiss_signal" => WorldId::SwissSignal,
            _ => WorldId::BlackIce,
        };
        return stems(&out_dir, sr, block, seed, wid, grammar, &story);
    }

    let trace = story_trace(&story, beats);

    let worlds: Vec<WorldId> = match which.as_str() {
        "black_ice" => vec![WorldId::BlackIce],
        "vapor95" => vec![WorldId::Vapor95],
        "swiss_signal" => vec![WorldId::SwissSignal],
        _ => vec![WorldId::BlackIce, WorldId::Vapor95, WorldId::SwissSignal],
    };

    println!(
        "HumanMusic lab — {grammar:?} / story={story} — {} events, {beats:.0} beats",
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

        let (score, plan) = compose_with_grammar(&trace, &world, seed, grammar);
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
        print_spine(&score.chords);
        print!("{}", plan.dump());
        print!("{}", CoherenceDiagnostics::measure(&plan, &score).report());
        print!("{}", DiscourseDiagnostics::measure(&plan, &score).report());
        println!(
            "{}",
            RealizationDiagnostics::measure(&plan, &score).report()
        );
        print!("{}", LeadOutlineDiagnostics::measure(&score).report());
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
        print!("{}", LeadOutlineDiagnostics::measure(&score).report());
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

/// Stem isolation: render one world's six buses (pad/keys/bass/lead/drums/sfx) each in isolation,
/// plus the full mix, to `<world>.stem_<bus>.wav`. Each stem uses a fresh synth (clean DSP state)
/// with the corresponding [`StemMask`]. The point is diagnostic: listen to one bus at a time to
/// find which voice family a bad tone lives in.
fn stems(
    out_dir: &std::path::Path,
    sr: SampleRate,
    block: usize,
    seed: u64,
    wid: WorldId,
    grammar: CompositionGrammar,
    story: &str,
) -> std::io::Result<()> {
    let world = MusicWorld::from_id(wid);
    let file_stem = world.name.to_lowercase();
    let trace = story_trace(story, 120.0);
    let (score, _plan) = compose_with_grammar(&trace, &world, seed, grammar);
    score.validate().expect("score invariants");

    println!(
        "HumanMusic stems — {} @ seed {seed}  ({} sfx, {} notes)\nout={}\n",
        world.name,
        score.sfx.len(),
        score.notes.len(),
        out_dir.display()
    );

    let mut masks: Vec<(String, StemMask)> = StemMask::NAMES
        .iter()
        .map(|n| ((*n).to_string(), StemMask::solo(n)))
        .collect();
    masks.push(("full".to_string(), StemMask::full()));

    for (name, mask) in masks {
        let mut synth = HumanMusicSynth::new(&score, &world, sr);
        synth.set_stem_mask(mask);
        let frames = synth.total_samples();
        let out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
        let path = out_dir.join(format!("{file_stem}.stem_{name}.wav"));
        write_wav_i16(&path, &out.audio, sr)?;
        println!(
            "  {name:6}  peak={:.3} rms={:.3} nonfinite={}  {}",
            out.peak,
            out.rms,
            out.audio.has_nonfinite(),
            path.display()
        );
    }

    println!(
        "\nStems rendered. Solo each bus to localize a bad tone; `full` is the reference mix."
    );
    Ok(())
}
