//! Reproducible argument audition on the production compiler/player/synth path.
//! `cargo run --release --example humanmusic_argument -- --out=PATH --source-sha=SHA`
//! A/B, raw-source-only controls, wrong answers and scrambled sources use matched
//! world/tempo/seed/form. The historical C137 generated control remains separate.
//! No human listening verdict is inferred; stems and raw PCM exposure are evidence
//! layers, not a single musical-quality number.

use gibson::audio::{
    buffer::StereoBlock,
    human_music::{
        action::Agent,
        argument::{
            ArgumentFamily, ArgumentPlacement, ArgumentRelation, CompiledArgument, MusicalArgument,
        },
        argument_witness::{acoustic_exposure, observe_argument},
        composer::Composer,
        contract::CompositionGrammar,
        functor::{perform_argument, perform_with_profile, Composition},
        meaning::Commutation,
        performance::PerformanceOptions,
        policy::{LeadLifePolicy, NarrativePolicy, PerformanceProfile},
        receipt::PerformanceReceipt,
        score::{Note, Role, Score},
        semantic::deflected_lift_trace,
        synth::StemMask,
        theory::Mode,
        voice::VoiceEventId,
        HumanMusicSynth, MusicWorld, SongMap,
    },
    wav::write_wav_i16,
    OfflineRenderer, SampleRate,
};
use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
};

fn role(agent: Agent) -> Option<Role> {
    match agent {
        Agent::Lead => Some(Role::Lead),
        Agent::Keys => Some(Role::Keys),
        Agent::Bass => Some(Role::Bass),
        _ => None,
    }
}

fn inside(note: &Note, placement: &ArgumentPlacement, player: Role) -> bool {
    note.role == player
        && note.start_beat >= placement.start_beat - 1e-6
        && note.start_beat < placement.start_beat + placement.span_beats - 1e-6
}

fn source_note(note: &Note, compiled: &CompiledArgument) -> bool {
    compiled.placements.iter().any(|p| {
        p.carriers
            .iter()
            .filter_map(|a| role(*a))
            .any(|r| inside(note, p, r))
    })
}

fn prune_continuities(score: &mut Score) {
    let remaining: Vec<_> = score.notes.iter().map(VoiceEventId::of).collect();
    score.voice_continuity.retain(|edge| remaining.contains(&edge.from) && remaining.contains(&edge.to));
}

fn pcm(
    score: &Score,
    world: &MusicWorld,
    mask: StemMask,
    rate: SampleRate,
    block: usize,
) -> Result<StereoBlock, Box<dyn std::error::Error>> {
    let mut synth = HumanMusicSynth::new(score, world, rate);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let rendered = OfflineRenderer::new(rate, block).render(&mut synth, frames);
    if rendered.had_nonfinite {
        return Err("nonfinite argument PCM".into());
    }
    Ok(rendered.audio)
}

struct Packet<'a> {
    out: &'a Path,
    source_sha: &'a str,
    rate: SampleRate,
    manifest: String,
    evidence: String,
}

impl Packet<'_> {
    #[allow(clippy::too_many_arguments)]
    fn emit(
        &mut self,
        score: &Score,
        world: &MusicWorld,
        label: &str,
        seed: u64,
        family: &str,
        condition: &str,
        stem: &str,
    ) -> Result<StereoBlock, Box<dyn std::error::Error>> {
        let audio = pcm(
            score,
            world,
            if stem == "mix" {
                StemMask::full()
            } else {
                StemMask::solo(stem)
            },
            self.rate,
            256,
        )?;
        let file = format!("{label}_{condition}_{stem}.wav");
        write_wav_i16(self.out.join(&file), &audio, self.rate)?;
        let comparison = if condition == "source_only" {
            String::new()
        } else {
            format!("{}_seed{seed}", world.name)
        };
        writeln!(
            self.manifest,
            "{file}\t{condition}\t{family}\t{seed}\t{}\t{}\t{stem}\t{comparison}\t{}",
            world.name, score.tempo_bpm, self.source_sha
        )?;
        writeln!(
            self.evidence,
            "- {file}: peak {:.6}, RMS {:.6}, {} stereo frames; Score {:016x}.",
            audio.peak(),
            audio.rms(),
            audio.frames(),
            score.fingerprint()
        )?;
        Ok(audio)
    }

    fn argument(
        &mut self,
        compiled: &CompiledArgument,
        c: &Composition,
        world: &MusicWorld,
        label: &str,
        seed: u64,
        family: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let witness = observe_argument(compiled, &c.score, world.tonic_pc);
        if !witness.valid() {
            return Err(format!("source witness failed: {:?}", witness.violations()).into());
        }
        let commutation = Commutation::check(&c.song);
        let receipt = PerformanceReceipt::measure_under(c, world, PerformanceProfile::BAND);
        writeln!(self.evidence, "\n## {label}\n\nArgument source: `{family}`, seed {seed}; new raw witness PASS. Legacy coarse commutation passes={} stale={} divergences={}; general performance receipt passes={}.\n\nLegacy failures: {:?}\n\nThese old receipts are reported separately. The optional source contract does not claim that `perform_checked` accepted this take.\n\n```text\n{:?}\n```\n", commutation.commutes(), commutation.stale, commutation.divergences.len(), receipt.passes(), receipt.failures(), compiled.argument)?;
        writeln!(self.evidence, "- Unclassified pitched notes: {}; source-bearing notes: {}; required carrier duties: {}.", c.score.notes.iter().filter(|n| n.function.is_none()).count(), c.score.notes.iter().filter(|n| source_note(n, compiled)).count(), compiled.placements.iter().map(|p| p.carriers.len()).sum::<usize>())?;
        for player in [Role::Lead, Role::Keys, Role::Bass, Role::Pad] {
            writeln!(
                self.evidence,
                "- {}: total written notes {}; raw required-source notes {}.",
                player.label(),
                c.score.notes.iter().filter(|n| n.role == player).count(),
                c.score
                    .notes
                    .iter()
                    .filter(|n| n.role == player && source_note(n, compiled))
                    .count()
            )?;
        }
        std::fs::write(
            self.out.join(format!("{label}_score.txt")),
            format!("{:#?}", c.score),
        )?;
        std::fs::write(
            self.out.join(format!("{label}_argument.txt")),
            format!("{:#?}", compiled),
        )?;
        let full = self.emit(&c.score, world, label, seed, family, "correct", "mix")?;
        for stem in ["lead", "keys", "bass", "drums"] {
            self.emit(&c.score, world, label, seed, family, "correct", stem)?;
        }
        // Re-render exact source as an exposure control; the full raw-onset witness
        // still applies, while backing and drums are explicitly removed.
        let mut source_only = c.score.clone();
        source_only.notes.retain(|n| source_note(n, compiled));
        source_only.drums.clear();
        source_only.sfx.clear();
        prune_continuities(&mut source_only);
        self.emit(
            &source_only,
            world,
            label,
            seed,
            family,
            "source_only",
            "mix",
        )?;
        let mut ablated = c.score.clone();
        ablated.notes.retain(|n| !source_note(n, compiled));
        prune_continuities(&mut ablated);
        let ablation_pcm = pcm(&ablated, world, StemMask::full(), self.rate, 256)?;
        for placement in &compiled.placements {
            let factor = self.rate.as_f64() * 60.0 / f64::from(c.score.tempo_bpm);
            let start = (placement.start_beat * factor).round() as usize;
            let end = ((placement.start_beat + placement.span_beats) * factor).round() as usize;
            let exposure = acoustic_exposure(&full, &ablation_pcm, start, end);
            writeln!(self.evidence, "- Step {:?} raw PCM exposure: finite={}, ablation residual RMS={:.8}, absent-signal rejection={}. Residual is not a masking or listener recognition certificate.", placement.step, exposure.finite, exposure.ablation_residual_rms, exposure.reject_absent_signal)?;
        }
        // False sources retain the same rhythm section, timbre, loudness settings,
        // tempo and form. Their raw source rejection is predeclared and measured.
        let answer = compiled
            .placements
            .iter()
            .find(|p| matches!(p.relation, ArgumentRelation::Answer { .. }))
            .ok_or("calibration lacks an answer")?;
        let mut wrong = c.score.clone();
        let indices: Vec<_> = wrong
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| inside(n, answer, Role::Keys))
            .map(|(i, _)| i)
            .collect();
        if indices.len() < 3 {
            return Err("missing keys answer".into());
        }
        wrong.notes[indices[1]].pitch += 3;
        wrong.notes[indices[2]].pitch -= 2;
        let bad = observe_argument(compiled, &wrong, world.tonic_pc);
        if bad.valid() {
            return Err("unrelated answer control falsely accepted".into());
        }
        writeln!(
            self.evidence,
            "- Wrong answer REJECTED: {:?}.",
            bad.violations()
        )?;
        self.emit(&wrong, world, label, seed, family, "wrong_answer", "mix")?;
        let mut scramble = c.score.clone();
        for placement in &compiled.placements {
            for player in placement.carriers.iter().filter_map(|a| role(*a)) {
                let indices: Vec<_> = scramble
                    .notes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| inside(n, placement, player))
                    .map(|(i, _)| i)
                    .collect();
                let pitches: Vec<_> = indices.iter().map(|i| scramble.notes[*i].pitch).collect();
                for (i, pitch) in indices.iter().zip(pitches.into_iter().rev()) {
                    scramble.notes[*i].pitch = pitch;
                }
            }
        }
        if observe_argument(compiled, &scramble, world.tonic_pc).valid() {
            return Err("scrambled source control falsely accepted".into());
        }
        self.emit(&scramble, world, label, seed, family, "scrambled", "mix")?;
        let mut missing = c.score.clone();
        let last = compiled.placements.last().ok_or("missing return")?;
        missing.notes.retain(|n| !inside(n, last, Role::Keys));
        prune_continuities(&mut missing);
        if observe_argument(compiled, &missing, world.tonic_pc).valid() {
            return Err("missing carrier falsely accepted".into());
        }
        self.emit(
            &missing,
            world,
            label,
            seed,
            family,
            "missing_carrier",
            "mix",
        )?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let arg = |prefix: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(prefix).map(str::to_string))
    };
    let out = PathBuf::from(arg("--out=").unwrap_or_else(|| "target/humanmusic-argument".into()));
    std::fs::create_dir_all(&out)?;
    let source_sha = arg("--source-sha=")
        .unwrap_or_else(|| "UNCOMMITTED: record source tree before acceptance".into());
    let tempo = arg("--tempo=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(108.0);
    let beats = arg("--beats=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(128.0);
    let rate = SampleRate::new(
        arg("--sample-rate=")
            .and_then(|s| s.parse().ok())
            .unwrap_or(48_000),
    )
    .ok_or("zero sample rate")?;
    let seed_list = arg("--seeds=").unwrap_or_else(|| "2112,770919".into());
    let seeds: Vec<u64> = seed_list
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    let worlds = arg("--worlds=").unwrap_or_else(|| "black_ice".into());
    let mut packet = Packet { out: &out, source_sha: &source_sha, rate,
        manifest: "path\tcondition\tfamily\tseed\tworld\ttempo\tstem\tcomparison\tsource_sha\n".into(),
        evidence: format!("# Optional musical argument audition\n\nSource: {source_sha}. Sample rate {} Hz; tempo {tempo}; duration {beats} beats.\n\nA/B use the SAME fixed affect trace and compositional seed. Source phrases vary with argument seed. C conflict/reconciliation is explicitly UNIMPLEMENTED. Human listening is UNVERIFIED.\n\nWorld controls use the named world's timbre/production with an explicit shared A Aeolian frame, admitted sevenths/mixture, and tempo {tempo}; native world mode/vocabulary equivalence is not claimed. No incompatible major/minor hybrid is rated as poor music.\n", rate.get()) };
    let base = SongMap::compose(
        &deflected_lift_trace(beats),
        2112,
        Some(CompositionGrammar::DeflectedLift),
        Composer::MeaningDirected,
    );
    for name in worlds.split(',') {
        let mut world = match name {
            "black_ice" => MusicWorld::black_ice(),
            "vapor95" => MusicWorld::vapor95(),
            "swiss_signal" => MusicWorld::swiss_signal(),
            _ => return Err("unknown world".into()),
        };
        world.tempo_bpm = tempo;
        world.tonic_pc = 9;
        world.mode = Mode::Aeolian;
        world.use_sevenths = true;
        world.allow_modal_mixture = true;
        for seed in &seeds {
            let old_profile = PerformanceProfile::BAND
                .with_narrative(NarrativePolicy::Ensemble)
                .with_lead_life(LeadLifePolicy {
                    development: true,
                    spacing: false,
                    dynamics: true,
                });
            let old =
                perform_with_profile(&base, &world, PerformanceOptions::default(), old_profile)?;
            packet.emit(
                &old.score,
                &world,
                &format!("{name}_{seed}_baseline"),
                *seed,
                "historical_c137_generated",
                "baseline",
                "mix",
            )?;
            for (family, letter) in [
                (ArgumentFamily::CallAndEarnedAnswer, "a"),
                (ArgumentFamily::PromiseDeniedReturn, "b"),
            ] {
                let argument = MusicalArgument::calibration(family, *seed)?;
                let compiled = argument.compile(&base)?;
                let c = perform_argument(
                    &compiled,
                    &world,
                    PerformanceOptions {
                        actions: false,
                        ..PerformanceOptions::default()
                    },
                    PerformanceProfile::BAND,
                )?;
                packet.argument(
                    &compiled,
                    &c,
                    &world,
                    &format!("{name}_{seed}_{letter}"),
                    *seed,
                    letter,
                )?;
            }
        }
    }
    packet.evidence.push_str("\n## Interpretation limits\n\nSource identity and question binding are exact finite contracts. PCM ablation establishes a contribution reaches the rendered buffer, not that the band exposes it saliently or a human follows it. These takes await ACCEPT/REJECT/AMBIGUOUS from the maintainer. The handcrafted Rick reference is generated separately by the unchanged `rick_probe` example; it has a different chart and length, so it is a calibration reference rather than a matched causal A/B cell.\n");
    std::fs::write(out.join("manifest.tsv"), &packet.manifest)?;
    std::fs::write(out.join("EVIDENCE.md"), &packet.evidence)?;
    println!(
        "Wrote {} and manifest.tsv; listening verdict UNVERIFIED",
        out.display()
    );
    Ok(())
}
