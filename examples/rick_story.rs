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
//! opt-out), `--out=`.
//!
//! The finale-soundtrack dials (opt-in; each changes NO shipped default, exactly like the stock
//! `--trace=rise` fixtures):
//!
//! * `--trace=intro_demo` — the impossible-demo flight arc (hook→breath→build→DROP→reveal→triumph)
//!   instead of the default bittersweet bounce.
//! * `--tempo=NNN` — override the world's tempo (the finale baseline dials 108).
//! * `--mode=ionian|dorian|phrygian|lydian|mixolydian|aeolian|locrian|harmonic_minor` — override
//!   the world's home mode (the finale baseline dials Lydian for the awe/triumph affect).
//! * `--chart=travel` — a traveling chart whose journey reaches ♭VII and ♭III (an Am–F–C–G-shaped
//!   reach, one chord per bar) instead of prolonging home.
//! * `--germ=seed` — hand the composer a clear hand-authored question→answer hook instead of its
//!   grammar-searched germ (the chooser-vs-developer diagnostic): if seeding the obvious hook
//!   coheres, melodic SELECTION is the weak link; if not, DEVELOPMENT is.
//! * `--matrix` — render the 2×2 `narrative {off,Ensemble}` × `lead_life {off, dev+dynamics}`
//!   audition instead of the default 2 arms. The default `band`/`story` arms both inherit
//!   `BAND.lead_life = {off,off,off}` — the v0.4 robotic lead (fixed rhythm, constant velocity).
//!   This cell turns ON the already-built lead DEVELOPMENT and DYNAMICS axes (spacing held off,
//!   a separate later contrast) to hear whether a developing, breathing lead is the missing
//!   information. Every default is untouched; all four cells hold one fixed song/chart/trace/
//!   world/tempo/seed so only the two policy axes vary. The summary prints the `lead_life` that
//!   actually reached the PerformancePlan as the witness.
//!
//! A listening instrument on the beef-up branch, not shipped.

use gibson::audio::{
    human_music::{
        backbone::{ChartCell, ChartRoot},
        composer::{Composer, ThemeSeed},
        contract::CompositionGrammar,
        functor::{perform_with_profile, Composition},
        motif::Motif,
        performance::PerformanceOptions,
        policy::{LeadLifePolicy, NarrativePolicy, PerformanceProfile},
        score::Role,
        semantic::{calm_loop, deflected_lift_trace, false_climax, intro_demo, rise_unresolved},
        synth::StemMask,
        theory::{Mode, PitchBasis},
        HumanMusicSynth, MusicWorld, SongMap,
    },
    render::OfflineRenderer,
    wav::write_wav_i16,
    SampleRate,
};
use std::{fmt::Write as _, path::Path};

/// The hand-seeded diagnostic germ (STEP 3): a deliberately clear question→answer hook in
/// scale-degree coordinates (the song's Ionian reference frame; each room re-modes it). The
/// thesis and its consequent share the same opening gesture — a `do–re–mi–octave` reach, the
/// bugle-call identity — then split: the thesis hangs UNRESOLVED on the fifth (the question), the
/// consequent steps down and resolves HOME to the tonic (the answer). Supplied to the composer via
/// [`SongMap::compose_with_germ`] to A/B against the grammar-searched germ: if seeding this
/// obvious hook makes the song cohere, the weak link is melodic SELECTION; if it still does not,
/// the weak link is DEVELOPMENT. Six beats, so it slots identically to a grammar germ — only the
/// notes differ.
fn diagnostic_germ() -> ThemeSeed {
    let line = |degrees: Vec<i32>, rhythm: Vec<f32>| Motif {
        pitch_basis: PitchBasis::ScaleSteps,
        id: 0,
        degrees,
        rhythm,
    };
    // A shape change, not a paint change: a bold leaping gesture (do–sol–octave) with a distinct
    // dotted rhythm — nothing like the grammar germ's even stepwise run. Thesis and answer share
    // the leap head (the identity) then take genuinely DIFFERENT shapes: the question holds high
    // and hangs on the fifth; the answer turns and steps down home. If even THIS does not read as
    // a different shape over the trace-driven band, the shape lives in the band/trace, not the lead.
    ThemeSeed {
        // Question: leap up to the octave, lean on the 7th, hang open on the fifth.
        thesis: line(vec![0, 4, 7, 6, 4, 4], vec![0.5, 0.5, 1.0, 0.5, 0.5, 3.0]),
        // Answer: same leap head, then a conclusive stepwise descent home.
        answer: line(vec![0, 4, 7, 4, 2, 0], vec![0.5, 0.5, 1.0, 1.0, 1.0, 2.0]),
    }
}

/// Parse a [`Mode`] name for `--mode=`.
fn parse_mode(name: &str) -> Option<Mode> {
    Some(match name.to_ascii_lowercase().as_str() {
        "ionian" | "major" => Mode::Ionian,
        "dorian" => Mode::Dorian,
        "phrygian" => Mode::Phrygian,
        "lydian" => Mode::Lydian,
        "mixolydian" => Mode::Mixolydian,
        "aeolian" | "minor" => Mode::Aeolian,
        "locrian" => Mode::Locrian,
        "harmonic_minor" | "harmonicminor" => Mode::HarmonicMinor,
        _ => return None,
    })
}

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
        Some("intro") | Some("intro_demo") => intro_demo(beats),
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
    // `--germ=seed`: hand the composer a deliberately clear question→answer hook instead of its
    // grammar-searched germ (the STEP-3 chooser-vs-developer diagnostic). Default: the searched
    // germ (unchanged behavior).
    let seed_germ = matches!(arg("--germ=").as_deref(), Some("seed") | Some("seeded"));
    let germ = diagnostic_germ();
    let mut song = if seed_germ {
        SongMap::compose_with_germ(&trace, 2112, grammar, &germ)
    } else {
        SongMap::compose(&trace, 2112, grammar, composer)
    };

    // `--chart=travel`: an opt-in traveling chart — override the composed cell's Lift and Open so
    // the journey reaches ♭VII (Degree 6) and ♭III (Degree 2) under black_ice's re-moding (an
    // Am–F–C–G-shaped reach), at one chord per bar. Every default is untouched; the base cell (so
    // the rest of the vocabulary) stays the composer's own choice.
    if matches!(
        arg("--chart=").as_deref(),
        Some("travel") | Some("traveling")
    ) {
        if let Some(h) = song.harmonic.as_mut() {
            h.cell = ChartCell {
                lift: ChartRoot::Degree(6),
                open: ChartRoot::Degree(2),
                pointer: ChartRoot::Degree(4),
                ..h.cell
            };
            h.bars_per_chord = 1;
        }
    }

    let wname = arg("--world=").unwrap_or_else(|| "black_ice".to_string());
    let mut world = match wname.as_str() {
        "vapor95" => MusicWorld::vapor95(),
        "swiss_signal" => MusicWorld::swiss_signal(),
        _ => MusicWorld::black_ice(),
    };
    // `--tempo=NNN` and `--mode=NAME`: opt-in world overrides (default: the world's own). The
    // finale diagnostic baseline dials tempo 108 + Lydian; neither changes any shipped default.
    if let Some(bpm) = arg("--tempo=").and_then(|s| s.parse::<f32>().ok()) {
        world.tempo_bpm = bpm;
    }
    if let Some(m) = arg("--mode=").as_deref().and_then(parse_mode) {
        world.mode = m;
    }

    let no_lead = {
        let mut m = StemMask::full();
        m.lead = false;
        m
    };
    // `--matrix`: the 2×2 discriminating audition (narrative OFF/ON × lead-life OFF/ON). The audit
    // (PR #93) proved the default `band`/`story` arms both inherit `BAND.lead_life={off,off,off}`,
    // so the earlier "sounds the same" verdict was rendered on the v0.4 robotic lead. This turns on
    // the DEVELOPMENT (per-statement rhythm: antecedent states, consequent answers, returns develop
    // further) and DYNAMICS (per-note arc) axes — spacing held off as a separate later contrast.
    let lead_devdyn = LeadLifePolicy {
        development: true,
        spacing: false,
        dynamics: true,
    };
    let matrix = args.iter().any(|a| a == "--matrix");
    let arms: Vec<(&str, PerformanceProfile)> = if matrix {
        vec![
            // narrative OFF × lead OFF — the v0.4 control (= the old `band` arm).
            ("band", PerformanceProfile::BAND),
            // narrative OFF × lead ON — does a living lead alone revive it?
            (
                "band_lead",
                PerformanceProfile::BAND.with_lead_life(lead_devdyn),
            ),
            // narrative ON × lead OFF — the arm Leah judged "same shape" (= the old `story` arm).
            (
                "story",
                PerformanceProfile::BAND.with_narrative(NarrativePolicy::Ensemble),
            ),
            // narrative ON × lead ON — the audit's proposed combined arm: band handoff + living lead.
            (
                "story_lead",
                PerformanceProfile::BAND
                    .with_lead_life(lead_devdyn)
                    .with_narrative(NarrativePolicy::Ensemble),
            ),
        ]
    } else {
        vec![
            ("band", PerformanceProfile::BAND),
            (
                "story",
                PerformanceProfile::BAND.with_narrative(NarrativePolicy::Ensemble),
            ),
        ]
    };

    let chart_kind = match arg("--chart=").as_deref() {
        Some("travel") | Some("traveling") => "travel",
        _ => "composed",
    };
    let germ_kind = if seed_germ { "seeded" } else { "searched" };
    let mut summary = format!(
        "Rick-C137 GEN-STORY — band vs story. world={} song={:016x} beats={beats} composer={composer:?}\n\
         tempo={:.1}bpm mode={:?} germ={germ_kind} chart={chart_kind}\n",
        world.name,
        song.fingerprint(),
        world.tempo_bpm,
        world.mode,
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
        let ll = c.perf.lead_life;
        writeln!(
            summary,
            "[{name:<10}] score={:016x} narrative={} lead_life(dev={} spc={} dyn={}) {}\n         full: {full}\n         no-lead: {nl}",
            c.score.fingerprint(),
            c.perf.narrative.is_some(),
            ll.development as u8,
            ll.spacing as u8,
            ll.dynamics as u8,
            shape(&c),
        )?;
    }
    std::fs::write(out.join("summary.txt"), &summary)?;
    print!("{summary}");
    Ok(())
}
