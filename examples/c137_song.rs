//! Render the bounded C137 long-form song: two causally-connected episodes in one 32-beat Score.
//!
//! `cargo run --release --example c137_song -- --out=/tmp/c137-song --seeds=2112,7,99 --tempo=144`
//!
//! WAVs are 48 kHz stereo PCM16 with the world's original gains and no normalization. These
//! receipts establish reproducibility and the Score fingerprint; they are not human-listening
//! acceptance. The full mix and the score TSV are emitted per seed.

#[path = "c137_song/source.rs"]
mod source;

use gibson::audio::human_music::{
    kernel,
    score::Score,
    synth::{ProductionControl, StemMask},
    theory::Mode,
    HumanMusicSynth, MusicWorld,
};
use gibson::audio::{buffer::StereoBlock, wav::write_wav_i16, OfflineRenderer, SampleRate};
use std::{error::Error, fmt::Write as _, fs, path::PathBuf};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Options {
    out: PathBuf,
    seeds: Vec<u64>,
    tempo: f32,
}

impl Options {
    fn parse() -> Result<Self> {
        let mut options = Self {
            out: PathBuf::from("/tmp/c137-song"),
            seeds: vec![2112, 7, 99],
            tempo: 144.0,
        };
        for arg in std::env::args().skip(1) {
            if let Some(value) = arg.strip_prefix("--out=") {
                if value.is_empty() {
                    return Err("--out requires a directory".into());
                }
                options.out = value.into();
            } else if let Some(value) = arg.strip_prefix("--seeds=") {
                options.seeds = value
                    .split(',')
                    .map(str::parse)
                    .collect::<std::result::Result<_, _>>()?;
            } else if let Some(value) = arg.strip_prefix("--tempo=") {
                options.tempo = value.parse()?;
            } else {
                return Err(format!("unknown argument: {arg}").into());
            }
        }
        if !options.tempo.is_finite() || !(30.0..=300.0).contains(&options.tempo) {
            return Err("--tempo must be finite and between 30 and 300 BPM".into());
        }
        Ok(options)
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

fn main() -> Result<()> {
    let options = Options::parse()?;
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

    for seed in &options.seeds {
        let candidate_source = source::source(*seed);
        let take = kernel::perform_song(&candidate_source, &world)?;
        // Re-derive the committed candidate before rendering: the exported Score is a candidate
        // that must still satisfy the event/source contract it was committed under.
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
        println!(
            "seed {seed}: score {:016x}; lead={} keys={} bass={} drums={} frames={}",
            score.fingerprint(),
            score
                .notes
                .iter()
                .filter(|n| n.role == gibson::audio::human_music::score::Role::Lead)
                .count(),
            score
                .notes
                .iter()
                .filter(|n| n.role == gibson::audio::human_music::score::Role::Keys)
                .count(),
            score
                .notes
                .iter()
                .filter(|n| n.role == gibson::audio::human_music::score::Role::Bass)
                .count(),
            score.drums.len(),
            audio.frames(),
        );
    }
    println!(
        "Artifacts: {}. Human listening remains pending.",
        options.out.display()
    );
    Ok(())
}
