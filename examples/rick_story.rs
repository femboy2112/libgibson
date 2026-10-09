//! **Rick-C137 GEN-STORY — does the GENERATOR tell the story through the band?**
//!
//! The band-story round proved BY EAR, on a hand-authored song, that a song reads as a story when
//! the band ENACTS it — a thematic identity handed from musician to musician — not when a lead sings
//! over fixed backing (see `docs/HUMAN_MUSIC_BAND_STORY.md`). GEN-STORY-1..3 built the opt-in
//! `NarrativePolicy::Ensemble` that derives who carries which meaning from μ(song) and enacts it in
//! the performance. This example renders the GENERATOR's own song through:
//!
//! * `band`  — `PerformanceProfile::BAND`, no narrative: the lead carries, the band backs (control).
//! * `story` — `BAND.with_narrative(Ensemble)`: the band carries the germ the lead taught, directed
//!   by the song's actual meaning.
//!
//! Each is rendered as a full mix and as a `--no-lead` stem — the §32 flagship test: with the lead
//! muted, `story` should STILL carry the germ (the band is holding it), while `band` goes to pure
//! backing with no tune. The ear decides; the printed peak/rms only guard against a mix artifact.
//!
//!   cargo run --release --example rick_story -- --out=target/humanmusic-beefup/story
//!
//! Flags: `--beats=NNN` (default 96), `--world=black_ice|vapor95|swiss_signal` (default black_ice,
//! the judged voice), `--composer=meaning|propulsion` (default meaning — its μ hands the germ off
//! the lead), `--grammar=deflected|none|propulsive` (default deflected — the judged voice ADOPTS
//! the deflecting grammar so the band can carry the harmonic deflection; `none` = harmony-less
//! opt-out), `--out=`. A listening instrument on the beef-up branch, not shipped.

use gibson::audio::{
    human_music::{
        composer::Composer,
        contract::CompositionGrammar,
        functor::{perform_with_profile, Composition},
        performance::PerformanceOptions,
        policy::{NarrativePolicy, PerformanceProfile},
        score::Role,
        semantic::{calm_loop, deflected_lift_trace, false_climax, rise_unresolved},
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
    mask: StemMask,
) -> Result<String, Box<dyn std::error::Error>> {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(&c.score, world, rate);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(rate, 256).render(&mut synth, frames);
    if audio.had_nonfinite {
        return Err("nonfinite render".into());
    }
    write_wav_i16(path, &audio.audio, rate)?;
    Ok(format!("peak={:.3} rms={:.3}", audio.peak, audio.rms))
}

/// How many non-lead notes the score carries (the band's mass) and how many lead notes — a coarse
/// witness that the band is doing more under the narrative. The ear still decides.
fn shape(c: &Composition) -> String {
    let lead = c
        .score
        .notes
        .iter()
        .filter(|n| n.role == Role::Lead)
        .count();
    let band = c.score.notes.len() - lead;
    format!(
        "notes: lead={lead} band={band} total={}",
        c.score.notes.len()
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |p: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(p).map(str::to_string))
    };

    let out =
        Path::new(&arg("--out=").unwrap_or_else(|| "target/humanmusic-beefup/story".to_string()))
            .to_path_buf();
    std::fs::create_dir_all(&out)?;
    let beats: f64 = arg("--beats=").and_then(|s| s.parse().ok()).unwrap_or(96.0);
    let composer = match arg("--composer=").as_deref() {
        Some("propulsion") => Composer::StablePropulsion,
        _ => Composer::MeaningDirected,
    };
    // The GEN-STORY-5 withhold fixture is `--trace=rise --beats=160`: the only stock (trace x
    // grammar) whose μ aligns a harmonic Miss with a non-protected Develop lead phrase, so the
    // band-story's lead-withhold fires from the generator. The judged voice ADOPTS the deflecting
    // grammar by default (ear-gate 2026-10-09: "sounds fucking dope"/"in pocket"), so it carries
    // the harmonic lane the story turns on; pass `--grammar=none` to hear the old harmony-less
    // voice (theme carry only — the pre-adoption default).
    let trace = match arg("--trace=").as_deref() {
        Some("rise") | Some("rise_unresolved") => rise_unresolved(beats),
        Some("false_climax") => false_climax(beats),
        Some("calm") | Some("calm_loop") => calm_loop(beats),
        _ => deflected_lift_trace(beats),
    };
    let grammar = match arg("--grammar=").as_deref() {
        // Explicit opt-out: the old harmony-less judged voice (theme carry only).
        Some("none") => None,
        Some("propulsive") | Some("propulsive_return") => {
            Some(CompositionGrammar::PropulsiveReturn)
        }
        // Default (and `--grammar=deflected`): the judged voice adopts the deflecting grammar, so
        // it gains the harmonic Prepare→Miss lane the band carries. Adoption decision 2026-10-09.
        _ => Some(CompositionGrammar::DeflectedLift),
    };
    let song = SongMap::compose(&trace, 2112, grammar, composer);
    let wname = arg("--world=").unwrap_or_else(|| "black_ice".to_string());
    let world = match wname.as_str() {
        "vapor95" => MusicWorld::vapor95(),
        "swiss_signal" => MusicWorld::swiss_signal(),
        _ => MusicWorld::black_ice(),
    };

    let no_lead = {
        let mut m = StemMask::full();
        m.lead = false;
        m
    };
    let arms: [(&str, PerformanceProfile); 2] = [
        ("band", PerformanceProfile::BAND),
        (
            "story",
            PerformanceProfile::BAND.with_narrative(NarrativePolicy::Ensemble),
        ),
    ];

    let mut summary = format!(
        "Rick-C137 GEN-STORY — band vs story. world={} song={:016x} beats={beats} composer={composer:?}\n",
        world.name,
        song.fingerprint()
    );
    for (name, profile) in arms {
        let c = perform_with_profile(&song, &world, PerformanceOptions::default(), profile)?;
        let full = render(
            &out.join(format!("{wname}_{name}.wav")),
            &c,
            &world,
            StemMask::full(),
        )?;
        let nl = render(
            &out.join(format!("{wname}_{name}_nolead.wav")),
            &c,
            &world,
            no_lead,
        )?;
        writeln!(
            summary,
            "[{name:<5}] score={:016x} narrative={} {}\n         full: {full}\n         no-lead: {nl}",
            c.score.fingerprint(),
            c.perf.narrative.is_some(),
            shape(&c),
        )?;
    }
    std::fs::write(out.join("summary.txt"), &summary)?;
    print!("{summary}");
    Ok(())
}
