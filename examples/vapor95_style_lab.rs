//! VAPOR95 style lab — the listening device for the deliberate VAPOR95 palette revision.
//!
//! It separates two questions that a single "new world" render would conflate:
//!
//! - **Palette (A):** one fixed Score — performed once under the archival VAPOR95 v1 world —
//!   rendered through the v1 palette and through the current palette. Same notes, onsets and
//!   dynamics (the Score fingerprint is printed for both sides and must agree); only timbre, mix
//!   and production differ.
//! - **World (B):** the same SongMap performed (BAND, checked) under the v1 world and under the
//!   current world — the two `--record` runs. Everything the world law may change — tempo, swing, voicing, harmony colour,
//!   and the performance laws' reaction to the new patches' sounding tails — is allowed to differ;
//!   the song (SongMap fingerprint) may not.
//!
//! Every render also writes its stems and a machine report (bus RMS/peak, ceiling contact,
//! low-band side energy, tail level). The machine checks reject only gross failures; nothing here
//! claims a musical result — the maintainer listens.
//!
//! ```sh
//! cargo run --release --example vapor95_style_lab -- --record=<dir> --world=v1      # baseline; A/B left
//! cargo run --release --example vapor95_style_lab -- --record=<dir> --world=current # B right
//! cargo run --release --example vapor95_style_lab -- --palette=<dir>               # A right
//! cargo run --release --example vapor95_style_lab -- --ablate=<dir>                # production factors
//! cargo run --release --example vapor95_style_lab -- --candidates=<dir>            # tempo/swing cells
//! ```
#[path = "../tests/common/vapor95_v1.rs"]
mod vapor95_v1;

use gibson::audio::{
    dsp::filter::Svf,
    human_music::{
        composer::Composer,
        contract::CompositionGrammar,
        cover::{cover, CoverFidelityPreset, CoverFidelityProfile, CoverTarget},
        fingerprint::CanonicalFingerprint,
        functor::{perform_checked, Composition},
        language::MusicalLanguage,
        performance::PerformanceOptions,
        policy::PerformanceProfile,
        reference_song::ReferenceSong,
        rhythm::MetricPosition,
        semantic::{deflected_lift_trace, demo_trace},
        synth::{BusLevel, ProductionControl, StemMask},
        HumanMusicSynth, MusicWorld, SemanticTrace, SongMap,
    },
    render::OfflineRenderer,
    wav::write_wav_i16,
    SampleRate, StereoBlock,
};
use std::fmt::Write as _;
use std::path::Path;

type Error = Box<dyn std::error::Error>;

/// A lab mode: render into a directory, return its report.
type Mode = fn(&Path) -> Result<String, Error>;

/// One representative generated song. Composition is world-independent: the SongMap is a
/// function of trace, seed, grammar and composer only.
struct Song {
    name: &'static str,
    trace: fn(f64) -> SemanticTrace,
    beats: f64,
    seed: u64,
    grammar: CompositionGrammar,
    composer: Composer,
    fusion: bool,
}

/// The representative set (fixed seeds, all admitted by BAND under the v1 world when recorded).
/// `StablePropulsion` selects its own grammar (PropulsiveReturn); the grammar field is ignored there.
const SONGS: [Song; 4] = [
    Song {
        name: "hookarc",
        trace: demo_trace,
        beats: 64.0,
        seed: 98_000_101,
        grammar: CompositionGrammar::HookArc,
        composer: Composer::StructuralR9,
        fusion: true,
    },
    Song {
        name: "deflected",
        trace: deflected_lift_trace,
        beats: 64.0,
        seed: 98_000_202,
        grammar: CompositionGrammar::DeflectedLift,
        composer: Composer::StructuralR9,
        fusion: true,
    },
    Song {
        name: "propulsive",
        trace: demo_trace,
        beats: 64.0,
        seed: 98_000_303,
        grammar: CompositionGrammar::PropulsiveReturn,
        composer: Composer::StablePropulsion,
        fusion: false,
    },
    Song {
        name: "riffdrive",
        trace: demo_trace,
        beats: 64.0,
        seed: 98_000_404,
        grammar: CompositionGrammar::RiffDrive,
        composer: Composer::MeaningDirected,
        fusion: true,
    },
];

/// The cover: Ode to Joy at the Faithful preset (seed 904, the cover labs' VAPOR95 seed), at the
/// target world's own tempo.
const COVER_SEED: u64 = 904;

/// The stems written beside every full mix.
const STEMS: [&str; 5] = ["keys", "pad", "bass", "lead", "drums"];

/// Single-factor production ablations of the current world (each removes exactly one factor).
const ABLATIONS: [&str; 5] = ["nochorus", "noecho", "fullband", "nosat", "dry"];

/// Tempo/swing cells of the current palette: (label, bpm, swing).
const CELLS: [(&str, f32, f32); 4] = [
    ("t78_s16", 78.0, 0.16),
    ("t84_s16", 84.0, 0.16),
    ("t84_s32", 84.0, 0.32),
    ("t90_s00", 90.0, 0.0),
];

fn world_named(name: &str) -> Result<MusicWorld, Error> {
    match name {
        "v1" => Ok(vapor95_v1::vapor95_v1()),
        "current" => Ok(MusicWorld::vapor95()),
        other => Err(format!("unknown --world={other} (v1 | current)").into()),
    }
}

fn options(song: &Song) -> PerformanceOptions {
    PerformanceOptions {
        language: if song.fusion {
            MusicalLanguage::fusion_conversation()
        } else {
            MusicalLanguage::simple()
        },
        ..PerformanceOptions::default()
    }
}

fn song_map(song: &Song) -> SongMap {
    SongMap::compose(
        &(song.trace)(song.beats),
        song.seed,
        Some(song.grammar),
        song.composer,
    )
}

/// The BAND take of `song` under `world`, or the typed reason it was not returned.
fn perform(song: &Song, world: &MusicWorld) -> Result<Composition, String> {
    perform_checked(
        &song_map(song),
        world,
        options(song),
        PerformanceProfile::BAND,
    )
    .map_err(|e| e.to_string())
}

/// The Ode cover under `world` (checked: CoverConformance + receipt), or the typed refusal.
fn perform_cover(world: &MusicWorld) -> Result<Composition, Error> {
    let reference = ReferenceSong::from_tsv(
        &std::fs::read_to_string("docs/fixtures/humanmusic-cover/ode-import/reference.tsv")?,
        "sop",
    )?;
    let derived = reference.derive_harmony(MetricPosition::new(2, 1).expect("two beats"))?;
    let preset = CoverFidelityPreset::Faithful;
    let (map, _) = reference.extract_fidelity(
        &CoverFidelityProfile::preset(preset),
        Some(preset),
        Some(&derived),
    )?;
    Ok(cover(
        &map,
        CoverTarget {
            world,
            seed: COVER_SEED,
            grammar: CompositionGrammar::HookArc,
            options: PerformanceOptions::default(),
            profile: PerformanceProfile::BAND,
        },
    )?)
}

/// Machine measurements of one rendered mix (gross-failure screens, not quality).
struct Measure {
    peak_db: f32,
    rms_db: f32,
    /// Samples within 0.05 dB of the world's master ceiling (the limiter is working at the wall).
    at_ceiling: usize,
    /// Below-150 Hz side energy over mid energy, in dB (a mono low end reads very negative).
    low_side_db: f32,
    /// RMS of the final second (inside the render tail) relative to the whole render, in dB.
    tail_db: f32,
}

fn db(x: f64) -> f32 {
    (10.0 * x.max(1e-20).log10()) as f32
}

fn measure(audio: &StereoBlock, ceiling: f32, sr: f32) -> Measure {
    let n = audio.left.len();
    let (mut peak, mut sumsq, mut at_ceiling) = (0.0f32, 0.0f64, 0usize);
    let wall = ceiling * 10f32.powf(-0.05 / 20.0);
    let mut lo_l = Svf::new(sr);
    let mut lo_r = Svf::new(sr);
    lo_l.set(150.0, 0.0);
    lo_r.set(150.0, 0.0);
    let (mut mid, mut side) = (0.0f64, 0.0f64);
    for (&l, &r) in audio.left.iter().zip(&audio.right) {
        let p = l.abs().max(r.abs());
        peak = peak.max(p);
        if p >= wall {
            at_ceiling += 1;
        }
        let m = 0.5 * (l + r);
        sumsq += f64::from(m) * f64::from(m);
        let (a, b) = (lo_l.process(l), lo_r.process(r));
        mid += f64::from(0.5 * (a + b)).powi(2);
        side += f64::from(0.5 * (a - b)).powi(2);
    }
    let tail_from = n.saturating_sub(sr as usize);
    let tail: f64 = audio.left[tail_from..]
        .iter()
        .zip(&audio.right[tail_from..])
        .map(|(&l, &r)| f64::from(0.5 * (l + r)).powi(2))
        .sum::<f64>()
        / (n - tail_from).max(1) as f64;
    let body = sumsq / n.max(1) as f64;
    Measure {
        peak_db: db(f64::from(peak) * f64::from(peak)),
        rms_db: db(body),
        at_ceiling,
        low_side_db: db(side / mid.max(1e-20)),
        tail_db: db(tail / body.max(1e-20)),
    }
}

fn bus(l: BusLevel) -> String {
    format!(
        "{:>6.1}/{:>5.1}",
        db(f64::from(l.rms) * f64::from(l.rms)),
        db(f64::from(l.peak) * f64::from(l.peak))
    )
}

/// Render `score` through `world`'s palette under `production` and `mask`; returns the audio and,
/// for the full mask, the per-bus levels.
fn render(
    path: &Path,
    c: &Composition,
    world: &MusicWorld,
    production: ProductionControl,
    mask: StemMask,
) -> Result<(StereoBlock, String), Error> {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::with_production(&c.score, world, rate, production);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(rate, 256).render(&mut synth, frames);
    if audio.had_nonfinite {
        return Err(format!("nonfinite render: {}", path.display()).into());
    }
    write_wav_i16(path, &audio.audio, rate)?;
    let b = synth.bus_levels();
    let levels = format!(
        "bus rms/peak dBFS: pad {} keys {} bass {} lead {} drums {}",
        bus(b.pad),
        bus(b.keys),
        bus(b.bass),
        bus(b.lead),
        bus(b.drums)
    );
    Ok((audio.audio, levels))
}

/// Full mix + stems of one take; appends fingerprints, levels and measurements to `report`.
fn render_take(
    out: &Path,
    stem: &str,
    c: &Composition,
    world: &MusicWorld,
    production: ProductionControl,
    stems: bool,
    report: &mut String,
) -> Result<(), Error> {
    let sr = SampleRate::STUDIO.as_f64() as f32;
    let (audio, levels) = render(
        &out.join(format!("{stem}.full.wav")),
        c,
        world,
        production,
        StemMask::full(),
    )?;
    let m = measure(&audio, world.master_ceiling, sr);
    writeln!(
        report,
        "{stem}: song={:016x} score={:016x} perf={:016x} tempo={} swing={}\n  \
         full: peak {:.2} dBFS, rms {:.2} dBFS, at-ceiling {} samples, low-band side {:.1} dB, tail {:.1} dB\n  {levels}",
        c.song.fingerprint(),
        c.score.canonical_fingerprint(),
        c.perf.canonical_fingerprint(),
        c.score.tempo_bpm,
        world.swing,
        m.peak_db,
        m.rms_db,
        m.at_ceiling,
        m.low_side_db,
        m.tail_db,
    )?;
    if stems {
        for s in STEMS {
            render(
                &out.join(format!("{stem}.{s}.wav")),
                c,
                world,
                production,
                StemMask::solo(s),
            )?;
        }
    }
    Ok(())
}

/// The style record of one world: every song performed and rendered under it.
fn record(out: &Path, world: &MusicWorld) -> Result<String, Error> {
    let mut report = format!("world: {world:#?}\n\n");
    for song in &SONGS {
        match perform(song, world) {
            Ok(c) => render_take(
                out,
                song.name,
                &c,
                world,
                ProductionControl::NORMAL,
                true,
                &mut report,
            )?,
            Err(e) => writeln!(report, "{}: NOT RETURNED: {e}", song.name)?,
        }
    }
    match perform_cover(world) {
        Ok(c) => render_take(
            out,
            "ode_faithful",
            &c,
            world,
            ProductionControl::NORMAL,
            true,
            &mut report,
        )?,
        Err(e) => writeln!(report, "ode_faithful: NOT RETURNED: {e}")?,
    }
    Ok(report)
}

/// A: each v1 take's Score — the exact Score of the v1 record — rendered through the current
/// palette. The other side of the A/B is the v1 record itself (`--record --world=v1`); the
/// `score=` fingerprints of the two reports are equal take for take.
fn palette(out: &Path) -> Result<String, Error> {
    let (v1, now) = (vapor95_v1::vapor95_v1(), MusicWorld::vapor95());
    let mut report = String::from(
        "A — same Score, new palette. Each Score is the v1 world's BAND take (its score= equals the\n\
         v1 record's); only the synth's world (patches, drums, mix, production) is the current one.\n\n",
    );
    let mut takes: Vec<(&str, Result<Composition, Error>)> = SONGS
        .iter()
        .map(|s| (s.name, perform(s, &v1).map_err(Error::from)))
        .collect();
    takes.push(("ode_faithful", perform_cover(&v1)));
    for (name, take) in takes {
        match take {
            Ok(c) => render_take(
                out,
                name,
                &c,
                &now,
                ProductionControl::NORMAL,
                true,
                &mut report,
            )?,
            Err(e) => writeln!(report, "{name}: NOT RETURNED under v1: {e}")?,
        }
    }
    Ok(report)
}

/// Single-factor production ablations of the current world (full mixes only).
fn ablate(out: &Path) -> Result<String, Error> {
    let world = MusicWorld::vapor95();
    let mut report = String::from("Ablations of the current world (one factor removed each).\n\n");
    for song in SONGS.iter().take(2) {
        let c = perform(song, &world)?;
        for spec in ["normal"].into_iter().chain(ABLATIONS) {
            let production = ProductionControl::parse(spec)?;
            render_take(
                out,
                &format!("{}.{spec}", song.name),
                &c,
                &world,
                production,
                false,
                &mut report,
            )?;
        }
    }
    Ok(report)
}

/// Tempo/swing cells of the current palette (full mixes only).
fn candidates(out: &Path) -> Result<String, Error> {
    let mut report = String::from("Tempo/swing cells under the current palette.\n\n");
    for (label, bpm, swing) in CELLS {
        let mut world = MusicWorld::vapor95();
        world.tempo_bpm = bpm;
        world.swing = swing;
        for song in SONGS.iter().take(2) {
            match perform(song, &world) {
                Ok(c) => render_take(
                    out,
                    &format!("{}.{label}", song.name),
                    &c,
                    &world,
                    ProductionControl::NORMAL,
                    false,
                    &mut report,
                )?,
                Err(e) => writeln!(report, "{}.{label}: NOT RETURNED: {e}", song.name)?,
            }
        }
    }
    Ok(report)
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().collect();
    let value = |prefix: &str| args.iter().find_map(|a| a.strip_prefix(prefix));
    let modes: [(&str, Mode); 3] = [
        ("--palette=", palette),
        ("--ablate=", ablate),
        ("--candidates=", candidates),
    ];
    let mut ran = false;
    if let Some(dir) = value("--record=") {
        let world = world_named(value("--world=").unwrap_or("current"))?;
        let out = Path::new(dir);
        std::fs::create_dir_all(out)?;
        let report = record(out, &world)?;
        std::fs::write(out.join("record.txt"), &report)?;
        print!("{report}");
        ran = true;
    }
    for (flag, mode) in modes {
        if let Some(dir) = value(flag) {
            let out = Path::new(dir);
            std::fs::create_dir_all(out)?;
            let report = mode(out)?;
            std::fs::write(out.join("report.txt"), &report)?;
            print!("{report}");
            ran = true;
        }
    }
    if !ran {
        return Err(
            "give --record=<dir> [--world=v1|current], --palette=, --ablate= or --candidates="
                .into(),
        );
    }
    Ok(())
}
