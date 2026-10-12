//! Render the bounded C137 source-driven conversation and controlled listening artifacts.
//!
//! `cargo run --release --example c137_conversation -- --out=/tmp/c137-conversation
//! --seeds=2112,7,99 --tempo=144 --source-sha=<40-hex-code-revision>`
//!
//! WAVs are 48 kHz stereo PCM16 with the world's original gains and no normalization. Source
//! controls are re-performed; exported mute mutations are never promoted to committed takes.
//! These receipts establish reproducibility and causal changes, not human listening acceptance.

#[path = "c137_conversation/source.rs"]
mod source;

use gibson::audio::human_music::{
    argument::CompiledArgument,
    kernel::{self, CommittedPerformance},
    plan::ArrangementRole,
    score::{Role, Score},
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
    source_sha: String,
}

impl Options {
    fn parse() -> Result<Self> {
        let mut options = Self {
            out: PathBuf::from("/tmp/c137-conversation"),
            seeds: vec![2112, 7, 99],
            tempo: 144.0,
            source_sha: "unrecorded".into(),
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
            } else if let Some(value) = arg.strip_prefix("--source-sha=") {
                if value.len() != 40 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(
                        "--source-sha must be a complete 40-digit hexadecimal revision".into(),
                    );
                }
                options.source_sha = value.into();
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

/// Explicit archival checksum, not a cryptographic or cross-version semantic identity.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

struct Artifacts {
    root: PathBuf,
    manifest: String,
    audio: String,
}

impl Artifacts {
    fn new(root: PathBuf) -> Result<Self> {
        // Refuse a populated output directory so an interrupted run cannot mix two revisions.
        if root.exists() && fs::read_dir(&root)?.next().is_some() {
            return Err(format!("output directory is not empty: {}", root.display()).into());
        }
        fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            manifest: "path\tbytes\tfile_fnv1a64\n".into(),
            audio: "path\tscore_fingerprint\tproduction\tstem\tframes\tpeak\trms\tstatus\n".into(),
        })
    }

    fn record(&mut self, name: &str) -> Result<()> {
        let bytes = fs::read(self.root.join(name))?;
        writeln!(
            self.manifest,
            "{name}\t{}\t{:016x}",
            bytes.len(),
            fnv1a(&bytes)
        )?;
        Ok(())
    }

    fn text(&mut self, name: &str, content: &str) -> Result<()> {
        let path = self.root.join(name);
        fs::create_dir_all(path.parent().ok_or("artifact has no parent")?)?;
        fs::write(path, content)?;
        self.record(name)
    }

    fn wav(
        &mut self,
        name: &str,
        score: &Score,
        world: &MusicWorld,
        production: ProductionControl,
        stem: &str,
        status: &str,
    ) -> Result<StereoBlock> {
        let mask = if stem == "full" {
            StemMask::full()
        } else {
            StemMask::solo(stem)
        };
        let audio = render(score, world, production, mask, 256);
        if !audio.left.iter().chain(&audio.right).all(|s| s.is_finite()) {
            return Err(format!("non-finite rendered PCM: {name}").into());
        }
        write_wav_i16(self.root.join(name), &audio, SampleRate::STUDIO)?;
        self.record(name)?;
        writeln!(
            self.audio,
            "{name}\t{:016x}\t{}\t{stem}\t{}\t{}\t{}\t{status}",
            score.fingerprint(),
            production.label(),
            audio.frames(),
            audio.peak(),
            audio.rms(),
        )?;
        Ok(audio)
    }

    fn finish(mut self) -> Result<()> {
        self.text("audio.tsv", &self.audio.clone())?;
        // The manifest deliberately excludes itself; every other emitted artifact is included.
        fs::write(self.root.join("manifest.tsv"), self.manifest)?;
        Ok(())
    }
}

fn render(
    score: &Score,
    world: &MusicWorld,
    production: ProductionControl,
    mask: StemMask,
    block: usize,
) -> StereoBlock {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::with_production(score, world, rate, production);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    OfflineRenderer::new(rate, block)
        .render(&mut synth, frames)
        .audio
}

fn same_pcm(a: &StereoBlock, b: &StereoBlock) -> bool {
    a.frames() == b.frames()
        && a.left
            .iter()
            .chain(&a.right)
            .zip(b.left.iter().chain(&b.right))
            .all(|(a, b)| a.to_bits() == b.to_bits())
}

/// T/N/D/C columns preserve fusion_gates' parser; extra provenance follows its columns.
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

fn dump_take(
    artifacts: &mut Artifacts,
    name: &str,
    source: &CompiledArgument,
    take: &CommittedPerformance,
    world: &MusicWorld,
) -> Result<()> {
    take.validate_candidate(take.score())?;
    let source_dump = format!("{source:#?}\n");
    let score = take.score();
    let composition = take.composition();
    artifacts.text(&format!("{name}/source.debug"), &source_dump)?;
    artifacts.text(&format!("{name}/score.tsv"), &score_tsv(score))?;
    artifacts.text(&format!("{name}/score.debug"), &format!("{score:#?}\n"))?;
    artifacts.text(
        &format!("{name}/performance.dump"),
        &composition.perf.dump(),
    )?;
    artifacts.text(
        &format!("{name}/hearings.debug"),
        &format!("{:#?}\n", take.hearings()),
    )?;
    artifacts.text(&format!("{name}/receipts.txt"), &format!(
        "status=committed_event_contract_pass\nhuman_listening=pending\nsource_debug_fnv1a64={:016x}\nsong_fingerprint={:016x}\nperformance_fingerprint={:016x}\nscore_fingerprint={:016x}\nlegacy_stale_hearings={}\neffective_hearings={}\neffective_hearings_matching={}\nkeys_events={}\nkeys_dependent_drum_hits={}\nbass_approaches={}\npad_events={}\n",
        fnv1a(source_dump.as_bytes()), composition.song.fingerprint(), composition.perf.fingerprint(),
        score.fingerprint(), score.stale_hearings().len(), take.hearings().len(),
        take.hearings().iter().filter(|h| h.matches(score)).count(), score.role_notes(Role::Keys).count(),
        score.drums.iter().filter(|d| d.prov.role_note == "heard-keys").count(),
        score.role_notes(Role::Bass).filter(|n| n.prov.role_note == "approach").count(),
        score.role_notes(Role::Pad).count(),
    ))?;
    let normal = artifacts.wav(
        &format!("{name}/full.normal.wav"),
        score,
        world,
        ProductionControl::NORMAL,
        "full",
        "committed_event_contract_pass",
    )?;
    let other_block = render(
        score,
        world,
        ProductionControl::NORMAL,
        StemMask::full(),
        127,
    );
    let identical = same_pcm(&normal, &other_block);
    artifacts.text(&format!("{name}/block-independence.txt"), &format!(
        "block_frames=256,127\ncomparison=every_float32_sample_bit\nidentical={identical}\nframes={}\n", normal.frames()))?;
    if !identical {
        return Err(format!("block size changes exact PCM: {name}").into());
    }
    for (label, production) in [
        ("nocomp", ProductionControl::parse("nocomp")?),
        ("harmonic-reference", ProductionControl::HARMONIC_REFERENCE),
    ] {
        artifacts.wav(
            &format!("{name}/full.{label}.wav"),
            score,
            world,
            production,
            "full",
            "committed_same_score_production_control",
        )?;
    }
    for stem in ["lead", "keys", "bass", "drums", "pad"] {
        let pcm = artifacts.wav(
            &format!("{name}/{stem}.normal.wav"),
            score,
            world,
            ProductionControl::NORMAL,
            stem,
            "committed_same_score_solo_bus",
        )?;
        if stem == "pad" && pcm.peak() != 0.0 {
            return Err(format!("intentionally empty pad bus is audible: {name}").into());
        }
    }
    Ok(())
}

fn dump_mutants(
    artifacts: &mut Artifacts,
    prefix: &str,
    take: &CommittedPerformance,
    world: &MusicWorld,
) -> Result<()> {
    for kind in ["keys", "bass-approach", "drum-fill"] {
        let mut score = take.score().clone();
        let mut changed = 0;
        for n in &mut score.notes {
            if (kind == "keys" && n.role == Role::Keys)
                || (kind == "bass-approach"
                    && n.role == Role::Bass
                    && n.prov.role_note == "approach")
            {
                n.velocity = 0.0;
                changed += 1;
            }
        }
        if kind == "drum-fill" {
            for d in &mut score.drums {
                if d.prov.role_note == "heard-keys" {
                    d.velocity = 0.0;
                    changed += 1;
                }
            }
        }
        let name = format!("{prefix}/exported-mute-{kind}");
        artifacts.text(&format!("{name}/score.tsv"), &score_tsv(&score))?;
        artifacts.text(&format!("{name}/score.debug"), &format!("{score:#?}\n"))?;
        artifacts.text(&format!("{name}/receipts.txt"), &format!(
            "status=exported_mutation_not_committed\noperation=selected_event_velocity_zero\nevents_changed={changed}\nparent_score_fingerprint={:016x}\nscore_fingerprint={:016x}\nvalidate_candidate={:?}\neffective_hearings_matching={}\neffective_hearings_total={}\nsource_and_plan=parent_candidate_unchanged\ncausal_dependents=not_replanned\nhuman_listening=pending\n",
            take.score().fingerprint(), score.fingerprint(), take.validate_candidate(&score),
            take.hearings().iter().filter(|h| h.matches(&score)).count(), take.hearings().len(),
        ))?;
        artifacts.wav(
            &format!("{name}/full.normal.wav"),
            &score,
            world,
            ProductionControl::NORMAL,
            "full",
            "exported_mutation_not_committed",
        )?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let options = Options::parse()?;
    let mut world = MusicWorld::black_ice();
    world.tempo_bpm = options.tempo;
    world.tonic_pc = 9;
    world.mode = Mode::Aeolian;
    world.use_sevenths = true;
    world.allow_modal_mixture = true;
    let mut artifacts = Artifacts::new(options.out.clone())?;
    artifacts.text("run.txt", &format!(
        "example=c137_conversation\ncode_revision_caller_supplied={}\nseeds={:?}\ntempo={}\nsample_rate=48000\nencoding=stereo_PCM16\nnormalization=none\nsource_debug_checksum=FNV1a64_of_exact_dump_not_cryptographic\nfile_checksum=FNV1a64_of_exact_bytes_not_cryptographic\nsource_controls=replanned\nexported_mute_controls=uncommitted_velocity_mutations\nsolo_stems=individual_synth_buses_with_shared_production_chain_not_additive_separation\nhuman_listening=pending_against_teacher_golden7\nteacher_media=not_generated_or_substituted_by_this_example\n",
        options.source_sha, options.seeds, options.tempo))?;
    artifacts.text("world.debug", &format!("{world:#?}\n"))?;
    for seed in options.seeds {
        let candidate_source = source::source(seed);
        let candidate = kernel::perform(&candidate_source, &world)?;
        let prefix = format!("seed-{seed}");
        dump_take(
            &mut artifacts,
            &format!("{prefix}/candidate"),
            &candidate_source,
            &candidate,
            &world,
        )?;
        dump_mutants(&mut artifacts, &prefix, &candidate, &world)?;

        let mut withheld_source = candidate_source.clone();
        for phrase in &mut withheld_source.song.plan.arrangement.phrases {
            phrase.keys = ArrangementRole::Silent;
        }
        let withheld = kernel::perform(&withheld_source, &world)?;
        if withheld.score().role_notes(Role::Keys).count() != 0
            || withheld
                .score()
                .drums
                .iter()
                .any(|d| d.prov.role_note == "heard-keys")
        {
            return Err(
                format!("keys withholding did not remove its dependent fill: seed {seed}").into(),
            );
        }
        dump_take(
            &mut artifacts,
            &format!("{prefix}/keys-withheld-replanned"),
            &withheld_source,
            &withheld,
            &world,
        )?;

        let mut closed_argument = candidate_source.argument.clone();
        closed_argument.referents[0]
            .events
            .last_mut()
            .ok_or("source cell is empty")?
            .step = Some(0);
        let mut closed_source = closed_argument.compile(&candidate_source.song)?;
        closed_source.route.clone_from(&candidate_source.route);
        let closed = kernel::perform(&closed_source, &world)?;
        dump_take(
            &mut artifacts,
            &format!("{prefix}/closed-call-replanned"),
            &closed_source,
            &closed,
            &world,
        )?;
        artifacts.text(&format!("{prefix}/control-comparison.txt"), &format!(
            "candidate_source_call={:?}\nclosed_source_call={:?}\nwithheld_source_argument_byte_equal={}\nwithheld_route_equal={}\nclosed_route_equal={}\ncandidate_keys_events={}\nwithheld_keys_events={}\nclosed_keys_events={}\ncandidate_keys_dependent_drum_hits={}\nwithheld_keys_dependent_drum_hits={}\nclosed_keys_dependent_drum_hits={}\n",
            candidate_source.argument.referents[0].events, closed_source.argument.referents[0].events,
            format!("{:?}", candidate_source.argument) == format!("{:?}", withheld_source.argument),
            candidate_source.route == withheld_source.route, candidate_source.route == closed_source.route,
            candidate.score().role_notes(Role::Keys).count(), withheld.score().role_notes(Role::Keys).count(), closed.score().role_notes(Role::Keys).count(),
            candidate.score().drums.iter().filter(|d| d.prov.role_note == "heard-keys").count(),
            withheld.score().drums.iter().filter(|d| d.prov.role_note == "heard-keys").count(),
            closed.score().drums.iter().filter(|d| d.prov.role_note == "heard-keys").count(),
        ))?;
        println!("seed {seed}: score {:016x}; rendered candidate, source controls, stems, and exported mute mutations", candidate.score().fingerprint());
    }
    artifacts.finish()?;
    println!(
        "Artifacts: {}. Human listening remains pending.",
        options.out.display()
    );
    Ok(())
}
