//! Cover quotient and free-band listening lab. No external song-specific generator code.
use gibson::audio::{
    human_music::{
        contract::{CoherenceContract, CompositionGrammar},
        cover::{
            cover_candidate, cover_skeleton, CoverAxis, CoverConformance, CoverFreedom, CoverMap,
            CoverPipelineReceipt, CoverSpec, CoverTarget, OrderedChart, SkeletonSchedule,
        },
        fingerprint::CanonicalFingerprint,
        functor::{perform_pocketed, Composition},
        identity::IdentityDiagnostics,
        language::MusicalLanguage,
        performance::PerformanceOptions,
        policy::PerformanceProfile,
        reference_song::{ReferenceMode, ReferenceSong},
        score::Score,
        semantic::demo_trace,
        song::{SongMap, SongMapConformance},
        synth::StemMask,
        temporal::TemporalPitchDiagnostics,
        theory::Mode,
        witness, HumanMusicSynth, MusicWorld,
    },
    render::OfflineRenderer,
    wav::write_wav_i16,
    SampleRate,
};
use std::{fmt::Write, path::Path};
fn render(
    path: &Path,
    score: &Score,
    world: &MusicWorld,
) -> Result<(), Box<dyn std::error::Error>> {
    render_masked(path, score, world, StemMask::full())?;
    // `--stems` adds a drums-only bus next to every full mix, so a percussion change can be heard
    // (and hashed) in isolation from the rest of the band.
    if std::env::args().any(|a| a == "--stems") {
        render_masked(
            &path.with_extension("drums.wav"),
            score,
            world,
            StemMask::solo("drums"),
        )?;
    }
    Ok(())
}
fn render_masked(
    path: &Path,
    score: &Score,
    world: &MusicWorld,
    mask: StemMask,
) -> Result<(), Box<dyn std::error::Error>> {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(score, world, rate);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(rate, 256).render(&mut synth, frames);
    if audio.had_nonfinite {
        return Err("nonfinite render".into());
    }
    write_wav_i16(path, &audio.audio, rate)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let value = |prefix: &str| args.iter().find_map(|a| a.strip_prefix(prefix));
    let out = Path::new(value("--out=").unwrap_or("target/humanmusic-cover"));
    std::fs::create_dir_all(out)?;
    let should_render = args.iter().any(|a| a == "--render");
    let mut report = String::from(
        "Cover lab: machine invariance and freedom; human recognizability UNVERIFIED.\n",
    );
    let partial = value("--ordered-chart=");
    let mut observed_frame = None;
    let map = if let Some(path) = partial {
        let map =
            CoverMap::from_ordered_chart(OrderedChart::from_tsv(&std::fs::read_to_string(path)?)?)?;
        report.push_str("PARTIAL-COVER SKELETON: melody/bass/groove/source timing UNKNOWN. Target schedule: one bar per observed chord; unobserved intro gets a generated tonic chord. No exact transcription claim.\n");
        map
    } else if let Some(path) = value("--reference=") {
        let reference = ReferenceSong::from_tsv(
            &std::fs::read_to_string(path)?,
            value("--voice=").unwrap_or("sop"),
        )?;
        writeln!(report,"Reference voice={} tonic={} mode={:?} tempo={:?}; selected source melody only, accompaniment unobserved",reference.selected_voice,reference.tonic,reference.mode,reference.tempo)?;
        let mut source_world = MusicWorld::swiss_signal();
        source_world.tonic_pc = reference.tonic;
        source_world.mode = match reference.mode {
            ReferenceMode::Major => Mode::Ionian,
            ReferenceMode::Minor => Mode::Aeolian,
        };
        source_world.tempo_bpm = reference.tempo.ok_or("source tempo unknown")?;
        observed_frame = Some(source_world.clone());
        if should_render {
            render(
                &out.join("observed_source_melody.wav"),
                &reference.melody_score()?,
                &source_world,
            )?;
        }
        reference.extract(CoverSpec::from_contract(&CoherenceContract::for_grammar(
            CompositionGrammar::HookArc,
        )))?
    } else {
        let world = MusicWorld::black_ice();
        let song = SongMap::build(&demo_trace(64.0), 2112, Some(CompositionGrammar::HookArc));
        let source = perform_pocketed(&song, &world, PerformanceOptions::default());
        let map = CoverMap::extract(
            &source,
            &world,
            CoverSpec::from_contract(&song.plan.contract),
        )?;
        if should_render {
            render(&out.join("generated_source.wav"), &source.score, &world)?;
        }
        map
    };
    writeln!(
        report,
        "CoverMap canonical={:016x}",
        map.canonical_fingerprint()
    )?;
    for axis in CoverAxis::ALL {
        writeln!(report, "  {}: {:?}", axis.label(), map.knowledge(axis))?;
    }
    std::fs::write(out.join("cover-map.txt"), format!("{map:#?}\n"))?;
    let mut first: Option<Composition> = None;
    let mut variants = vec![
        ("black_seed_1", MusicWorld::black_ice(), 901, false, 0, 95.0),
        ("black_seed_2", MusicWorld::black_ice(), 902, false, 0, 95.0),
        ("swiss", MusicWorld::swiss_signal(), 903, true, 0, 100.0),
        ("vapor", MusicWorld::vapor95(), 904, false, 0, 95.0),
        (
            "transposed_faster",
            MusicWorld::swiss_signal(),
            905,
            false,
            5,
            118.0,
        ),
    ];
    if let Some(world) = observed_frame {
        let tempo = world.tempo_bpm;
        variants.insert(0, ("same_source_frame", world, 900, false, 0, tempo));
    }
    for (name, mut world, seed, simple, key_shift, tempo) in variants {
        world.tonic_pc = (world.tonic_pc + key_shift) % 12;
        world.tempo_bpm = tempo;
        let mut options = PerformanceOptions::default();
        if simple {
            options.language = MusicalLanguage::simple();
        }
        let target = CoverTarget {
            world: &world,
            seed,
            grammar: CompositionGrammar::HookArc,
            options,
            profile: PerformanceProfile::POCKET,
        };
        let candidate = if partial.is_some() {
            cover_skeleton(&map, target, SkeletonSchedule { bars_per_chord: 1 })
        } else {
            cover_candidate(&map, target)
        };
        let c = match candidate {
            Ok(c) => c,
            Err(error) => {
                writeln!(report, "{name}: INFEASIBLE {error}")?;
                continue;
            }
        };
        let law = CoverConformance::check(&map, &c, &world);
        let pipeline = CoverPipelineReceipt::measure(&c, &world);
        writeln!(report,"\n{name}: seed={seed} world={} key={} tempo={} language={} score={:016x} perf={:016x}\n{}\nPipeline {}: {:?}",world.name,world.tonic_pc,world.tempo_bpm,c.perf.language.id.label(),c.score.canonical_fingerprint(),c.perf.canonical_fingerprint(),law.report(),if pipeline.passes(){"PASS"}else{"FAIL (preserved)"},pipeline)?;
        if let Some(reference) = &first {
            writeln!(
                report,
                "Freedom versus first cover: {:?}",
                CoverFreedom::compare(reference, &c)
            )?;
        }
        std::fs::write(
            out.join(format!("{name}.song.txt")),
            SongMapConformance::check(&c.song, &c.perf, &c.score).report(),
        )?;
        std::fs::write(
            out.join(format!("{name}.temporal.txt")),
            TemporalPitchDiagnostics::measure(&c.perf, &c.score).report(),
        )?;
        std::fs::write(
            out.join(format!("{name}.identity.txt")),
            IdentityDiagnostics::measure_score(&c.score, &c.perf.contexts, &world)
                .report(&c.score.notes, &c.perf.contexts),
        )?;
        std::fs::write(
            out.join(format!("{name}.actions.txt")),
            format!("{:#?}", witness::audit(&c.perf, &c.score).rows),
        )?;
        std::fs::write(
            out.join(format!("{name}.hearings.txt")),
            format!(
                "{:#?}\nstale={:#?}",
                c.score.hearings,
                c.score.stale_hearings()
            ),
        )?;
        std::fs::write(
            out.join(format!("{name}.notes.tsv")),
            c.score
                .notes
                .iter()
                .map(|n| {
                    format!(
                        "{}\t{}\t{}\t{}\t{}\n",
                        n.role.label(),
                        n.start_beat,
                        n.dur_beats,
                        n.pitch,
                        n.velocity
                    )
                })
                .collect::<String>(),
        )?;
        if should_render {
            render(&out.join(format!("{name}.wav")), &c.score, &world)?;
        }
        if first.is_none() {
            first = Some(c);
        }
    }
    report.push_str("\nListening questions: is the identified melody unmistakably the same song? Does the free band interpret it rather than merely change patches? The partial chart has no source-melody recognition claim.\n");
    std::fs::write(out.join("report.txt"), &report)?;
    print!("{report}");
    Ok(())
}
