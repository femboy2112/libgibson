//! Render the C137 long-form song. THE JERRY LAW: the ordinary, no-flag invocation renders C137
//! **Full** — the complete 16-bar verse→hook→bridge→return arc, the expressive lead, and the ensemble
//! breath response. Excellence is the default; a downgrade is the explicit experiment.
//!
//! `cargo run --release --example c137_song -- --out=/tmp/c137-song --seeds=2112,7,99 --tempo=144`
//!
//! A **Jerry switch** removes one capability (see `c137_song/preset.rs`):
//!   `--jerry=short` renders the 8-bar verse→hook control (no bridge/return/rug).
//!   `--jerry=plain`  plays the lead robotically (legato re-tile + flat ramp; no band answer).
//!   `--jerry=deaf`   keeps the expressive lead but makes the band deaf to its breaths.
//! Switches combine (`--jerry=short,plain` is the frozen 8-bar plain control = the old default).
//!
//! The pre-preset flags still work for compatibility and fold into the same mechanism, so they are
//! reported honestly as the downgrades they are: `--bars=8` ⇒ `short`; `--phrasing=plain` ⇒ `plain`;
//! `--phrasing=expressive` ⇒ `deaf`. `--bridge-seed=N` swaps in a different LAWFUL bridge over the same
//! chart and returning hook (the earned-return counterfactual; only meaningful on the 16-bar arc).
//!
//! Each run prints and writes an EFFECTIVE-FEATURE MANIFEST (`manifest.txt`): the resolved preset, the
//! active capabilities with their witnessed effect, the Jerry switches in force, world/tempo/seed, and
//! the per-seed Score fingerprint — so running the wrong musical engine is immediately visible. If the
//! full C137 source cannot be admitted the run ERRORS; it never silently falls back to lesser music.
//!
//! WAVs are 48 kHz stereo PCM16 with the world's original gains and no normalization. These receipts
//! establish reproducibility and the Score fingerprint; they are NOT human-listening acceptance.

// The source fixtures and the preset are `#[path]`-shared with the integration tests. The example
// binary drives the sources through their `*_phrased` entry points, so the plain-delegating wrappers
// (`source`, `source_with_bridge_seed`) read as unused HERE while the tests still exercise them.
#[allow(dead_code)]
#[path = "c137_song/preset.rs"]
mod preset;
#[allow(dead_code)]
#[path = "c137_song/source.rs"]
mod source;
#[allow(dead_code)]
#[path = "c137_song/source_bridge.rs"]
mod source_bridge;

use gibson::audio::human_music::{
    argument::CompiledArgument,
    kernel,
    score::{Role, Score},
    synth::{ProductionControl, StemMask},
    theme_family::Phrasing,
    theory::Mode,
    HumanMusicSynth, MusicWorld,
};
use gibson::audio::{buffer::StereoBlock, wav::write_wav_i16, OfflineRenderer, SampleRate};
use preset::{C137Preset, Jerry, Resolved};
use std::{error::Error, fmt::Write as _, fs, path::PathBuf};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Options {
    out: PathBuf,
    seeds: Vec<u64>,
    tempo: f32,
    /// The resolved C137 preset (Full by default; Jerry switches downgrade it).
    preset: C137Preset,
    /// The bridge motif's seed, for the earned-return counterfactual (only used by the 16-bar arc).
    /// `None` ties the bridge to the song seed (the shipped arc); `Some(bs)` swaps in a different
    /// LAWFUL bridge over the same chart and the same returning hook.
    bridge_seed: Option<u64>,
}

impl Options {
    fn parse() -> Result<Self> {
        let mut out = PathBuf::from("/tmp/c137-song");
        let mut seeds = vec![2112u64, 7, 99];
        let mut tempo = 144.0f32;
        let mut bridge_seed = None;
        // The no-flag command is C137 FULL. Downgrades accumulate here, from `--jerry=` and from the
        // compatibility flags, which fold into the same mechanism so they manifest as the Jerry
        // switches they are.
        let mut preset = C137Preset::full();
        for arg in std::env::args().skip(1) {
            if let Some(value) = arg.strip_prefix("--out=") {
                if value.is_empty() {
                    return Err("--out requires a directory".into());
                }
                out = value.into();
            } else if let Some(value) = arg.strip_prefix("--seeds=") {
                seeds = value
                    .split(',')
                    .map(str::parse)
                    .collect::<std::result::Result<_, _>>()?;
            } else if let Some(value) = arg.strip_prefix("--tempo=") {
                tempo = value.parse()?;
            } else if let Some(value) = arg.strip_prefix("--jerry=") {
                for token in value.split(',').filter(|t| !t.is_empty()) {
                    preset = preset.jerry(Jerry::parse(token)?);
                }
            } else if let Some(value) = arg.strip_prefix("--bars=") {
                // Compatibility: an explicit bar count folds into the Jerry mechanism.
                match value.parse::<u32>()? {
                    16 => {} // the Full arc — no downgrade
                    8 => preset = preset.jerry(Jerry::Short),
                    _ => {
                        return Err("--bars must be 8 (verse→hook control) or 16 (full arc)".into())
                    }
                }
            } else if let Some(value) = arg.strip_prefix("--bridge-seed=") {
                bridge_seed = Some(value.parse()?);
            } else if let Some(value) = arg.strip_prefix("--phrasing=") {
                // Compatibility: explicit phrasing folds into the Jerry mechanism (Full is `ensemble`).
                match value {
                    "ensemble" => {} // the Full lead — no downgrade
                    "expressive" => preset = preset.jerry(Jerry::DeafBand),
                    "plain" => preset = preset.jerry(Jerry::Plain),
                    _ => return Err("--phrasing must be plain, expressive, or ensemble".into()),
                }
            } else {
                return Err(format!("unknown argument: {arg}").into());
            }
        }
        if !tempo.is_finite() || !(30.0..=300.0).contains(&tempo) {
            return Err("--tempo must be finite and between 30 and 300 BPM".into());
        }
        Ok(Self {
            out,
            seeds,
            tempo,
            preset,
            bridge_seed,
        })
    }
}

/// Build the committed source for one seed under the resolved config. The SAME `Resolved` that drives
/// the manifest chooses the arc and the lead performance here, so the manifest can never over-report.
/// Any compile/admission failure propagates — the run never silently substitutes lesser music.
fn build_source(resolved: &Resolved, seed: u64, bridge_seed: Option<u64>) -> CompiledArgument {
    if resolved.bars == 16 {
        match bridge_seed {
            Some(bs) => source_bridge::source_with_bridge_seed_phrased(seed, bs, resolved.phrasing),
            None => source_bridge::source_phrased(seed, resolved.phrasing),
        }
    } else {
        source::source_phrased(seed, resolved.phrasing)
    }
}

fn render(score: &Score, world: &MusicWorld, block: usize) -> StereoBlock {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::with_production(score, world, rate, ProductionControl::NORMAL);
    synth.set_stem_mask(StemMask::full());
    let frames = synth.total_samples();
    OfflineRenderer::new(rate, block)
        .render(&mut synth, frames)
        .audio
}

/// T/N/D/C columns, mirroring the conversation example's score dump.
fn score_tsv(score: &Score) -> String {
    let mut text = format!(
        "T\t{}\t{}\t{}\n",
        score.tempo_bpm, score.beats_per_bar, score.total_beats
    );
    for n in &score.notes {
        writeln!(
            text,
            "N\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{:?}\t{}\t{:?}\t{:?}",
            n.role.label(),
            n.start_beat,
            n.dur_beats,
            n.pitch,
            n.velocity,
            n.prov.section.label(),
            n.prov.phrase,
            n.function,
            n.prov.role_note,
            n.prov.material,
            n.prov.actions
        )
        .unwrap();
    }
    for d in &score.drums {
        writeln!(
            text,
            "D\t{:?}\t{}\t{}\t{}\t{}\t{:?}",
            d.voice,
            d.start_beat,
            d.velocity,
            d.prov.section.label(),
            d.prov.role_note,
            d.prov.actions
        )
        .unwrap();
    }
    for c in &score.chords {
        writeln!(
            text,
            "C\t{}\t{}\t{}\t{:?}\t{:?}\t{}",
            c.start_beat, c.dur_beats, c.chord.root_pc, c.chord.quality, c.function, c.degree
        )
        .unwrap();
    }
    text
}

/// The global capability block of the effective-feature manifest (everything independent of seed).
fn manifest_header(resolved: &Resolved, world: &MusicWorld, bridge_seed: Option<u64>) -> String {
    let mut m = String::new();
    let title = if resolved.is_full() {
        "C137 Full  (default — excellence is not opt-in)".to_string()
    } else {
        let names: Vec<_> = resolved.jerry.iter().map(|j| j.label()).collect();
        format!("C137 Jerry[{}]  — DOWNGRADED", names.join(","))
    };
    let _ = writeln!(m, "C137 EFFECTIVE FEATURE MANIFEST");
    let _ = writeln!(m, "  preset                : {title}");
    // Capability lines — resolved from the config, each tagged FULL or the Jerry that reduced it.
    let arc = if resolved.bars == 16 {
        "16 bars — verse→hook→bridge→return (FULL)"
    } else {
        "8 bars — verse→hook control (Jerry: short)"
    };
    let _ = writeln!(m, "  song arc              : {arc}");
    let lead = match resolved.phrasing {
        Phrasing::Ensemble => {
            "expressive articulation + tension-release contour + ensemble breath answer (FULL)"
        }
        Phrasing::Expressive => {
            "expressive articulation + tension-release contour; band DEAF (Jerry: deaf)"
        }
        Phrasing::Plain => "plain legato re-tile + flat ramp (Jerry: plain)",
    };
    let _ = writeln!(m, "  lead performance      : {lead}");
    let earned = if resolved.bars == 16 {
        "active — the return accompaniment depends on the bridge travelled (counterfactual: --bridge-seed)"
    } else {
        "n/a — the 8-bar control has no bridge"
    };
    let _ = writeln!(m, "  earned bridge→return  : {earned}");
    let rug = if resolved.bars == 16 {
        "active — the pad frames the home arrivals (verse/bridge stay dry)"
    } else {
        "n/a — the 8-bar control stages the pad silent"
    };
    let _ = writeln!(m, "  selective pad (rug)   : {rug}");
    let _ = writeln!(
        m,
        "  shared pocket/hearing : active — all voices lock one pocket; drums read lead+harmony only (keys-independent pulse)"
    );
    let jerry = if resolved.jerry.is_empty() {
        "none".to_string()
    } else {
        resolved
            .jerry
            .iter()
            .map(|j| format!("{} (removes {})", j.label(), j.removes()))
            .collect::<Vec<_>>()
            .join("\n                          ")
    };
    let _ = writeln!(m, "  jerry switches        : {jerry}");
    let _ = writeln!(
        m,
        "  negative controls     : CoupledR8 and low-register BASS_WEAVE are ear-REJECTED — never in Full, reachable only by their own explicit pathways"
    );
    let _ = writeln!(
        m,
        "  world                 : tonic_pc={}, {:?}, sevenths={}, modal_mixture={}, tempo={} BPM",
        world.tonic_pc, world.mode, world.use_sevenths, world.allow_modal_mixture, world.tempo_bpm
    );
    if let Some(bs) = bridge_seed {
        let note = if resolved.bars == 16 {
            format!("bridge-seed={bs} (earned-return counterfactual arm)")
        } else {
            format!("bridge-seed={bs} IGNORED (no bridge on the 8-bar control)")
        };
        let _ = writeln!(m, "  counterfactual        : {note}");
    }
    let git = option_env!("GIT_SHA")
        .unwrap_or("not embedded — the per-seed Score fingerprint is the content-integrity anchor");
    let _ = writeln!(m, "  source                : {git}");
    m
}

/// One seed's witness line: the Score fingerprint plus the realized effect of each advertised
/// capability (D5: no capability is reported active unless it left a trace; a requested-but-idle
/// capability is reported as such rather than silently claimed).
fn manifest_seed_line(
    resolved: &Resolved,
    seed: u64,
    score: &Score,
    audio: &StereoBlock,
) -> String {
    let count = |r: Role| score.role_notes(r).count();
    let breaths = preset::lead_breaths(score);
    let answers = preset::breath_answers(score);
    let pad = preset::pad_notes(score);
    let spread = preset::lead_velocity_spread(score);
    // Honest status for the two witnessed, downgradable capabilities.
    let expr_status = if resolved.phrasing == Phrasing::Plain {
        "off(plain)".to_string()
    } else if breaths > 0 {
        format!("ACTIVE(breaths={breaths}, vspread={spread:.2})")
    } else {
        format!("requested, 0 breaths this seed (vspread={spread:.2})")
    };
    let ans_status = if resolved.phrasing != Phrasing::Ensemble {
        "off".to_string()
    } else if answers > 0 {
        format!("ACTIVE({answers})")
    } else {
        "requested, 0 breath-opportunities this seed".to_string()
    };
    format!(
        "  seed {seed:<5}: fp={:016x}  lead={} keys={} bass={} pad={} drums={} frames={}  | expressive={expr_status}  breath-answer={ans_status}  rug={pad}",
        score.fingerprint(),
        count(Role::Lead),
        count(Role::Keys),
        count(Role::Bass),
        count(Role::Pad),
        score.drums.len(),
        audio.frames(),
    )
}

fn main() -> Result<()> {
    let options = Options::parse()?;
    let resolved = options.preset.resolve();

    let mut world = MusicWorld::black_ice();
    world.tempo_bpm = options.tempo;
    world.tonic_pc = 9;
    world.mode = Mode::Aeolian;
    world.use_sevenths = true;
    world.allow_modal_mixture = true;

    if options.out.exists() && fs::read_dir(&options.out)?.next().is_some() {
        return Err(format!("output directory is not empty: {}", options.out.display()).into());
    }
    fs::create_dir_all(&options.out)?;

    let mut manifest = manifest_header(&resolved, &world, options.bridge_seed);
    print!("{manifest}");

    for seed in &options.seeds {
        // Build under the resolved config. A failure here propagates — no silent fallback to Jerry.
        let candidate_source = build_source(&resolved, *seed, options.bridge_seed);
        let take = kernel::perform_song(&candidate_source, &world)?;
        // Re-derive the committed candidate before rendering: the exported Score is a candidate that
        // must still satisfy the event/source contract it was committed under.
        take.validate_candidate(take.score())?;
        let score = take.score();
        let dir = options.out.join(format!("seed-{seed}"));
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("score.tsv"), score_tsv(score))?;
        let audio = render(score, &world, 256);
        if !audio.left.iter().chain(&audio.right).all(|s| s.is_finite()) {
            return Err(format!("non-finite rendered PCM: seed {seed}").into());
        }
        write_wav_i16(dir.join("full.normal.wav"), &audio, SampleRate::STUDIO)?;
        let line = manifest_seed_line(&resolved, *seed, score, &audio);
        println!("{line}");
        let _ = writeln!(manifest, "{line}");
    }

    fs::write(options.out.join("manifest.txt"), &manifest)?;
    println!(
        "Artifacts: {} (manifest.txt written). Human listening remains pending.",
        options.out.display()
    );
    Ok(())
}
