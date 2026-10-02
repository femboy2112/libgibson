//! Ode to Joy at every cover fidelity: the same target world, seed and performance profile —
//! only the fidelity profile changes. Performed by The homosapians (the in-universe cover band;
//! the name is presentation only and appears in titles and metadata, never in the maths).
//!
//! Source facts vs derived analysis: the Mutopia #528 symbolic import states pitches, onsets and
//! durations per voice. Harmony is NOT in the source; the chord spans pinned by the stronger
//! presets are our declared analyzer's reading of the observed SATB simultaneities and are
//! labelled `DerivedAnalysis` in every receipt. Human recognition is UNVERIFIED.
#[path = "../tests/common/vapor95_v1.rs"]
mod vapor95_v1;
use gibson::audio::{
    human_music::{
        contract::{CoherenceContract, CompositionGrammar},
        cover::{
            cover_candidate, CoverConformance, CoverFidelityPreset, CoverFidelityProfile, CoverMap,
            CoverSpec, CoverTarget,
        },
        fingerprint::CanonicalFingerprint,
        functor::Composition,
        performance::PerformanceOptions,
        policy::PerformanceProfile,
        reference_song::ReferenceSong,
        rhythm::MetricPosition,
        synth::StemMask,
        HumanMusicSynth, MusicWorld,
    },
    render::OfflineRenderer,
    wav::write_wav_i16,
    SampleRate,
};
use std::{fmt::Write, path::Path};

/// The canonical HumanMusic cover band (presentation layer only).
const BAND_NAME: &str = "The homosapians";

fn render(
    path: &Path,
    c: &Composition,
    world: &MusicWorld,
    mask: StemMask,
) -> Result<(), Box<dyn std::error::Error>> {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(&c.score, world, rate);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(rate, 256).render(&mut synth, frames);
    if audio.had_nonfinite {
        return Err("nonfinite render".into());
    }
    write_wav_i16(path, &audio.audio, rate)?;
    Ok(())
}

/// VAPOR95 as this run asks for it: the product world, or with `--vapor95=v1` the archival v1
/// world (`tests/common/vapor95_v1.rs`), which reproduces historical VAPOR95 audio and receipts.
fn vapor95_world() -> MusicWorld {
    if std::env::args().any(|a| a == "--vapor95=v1") {
        vapor95_v1::vapor95_v1()
    } else {
        MusicWorld::vapor95()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let value = |prefix: &str| args.iter().find_map(|a| a.strip_prefix(prefix));
    let out = Path::new(value("--out=").unwrap_or("target/humanmusic-cover/ode-fidelity"));
    std::fs::create_dir_all(out)?;
    let should_render = args.iter().any(|a| a == "--render");
    let reference = ReferenceSong::from_tsv(
        &std::fs::read_to_string(
            value("--reference=")
                .unwrap_or("docs/fixtures/humanmusic-cover/ode-import/reference.tsv"),
        )?,
        "sop",
    )?;
    let derived = reference.derive_harmony(MetricPosition::new(2, 1).expect("two beats"))?;
    let mut report = format!(
        "{BAND_NAME} — Ode to Joy: one target, five fidelities.\n\
         Source: Mutopia #528 symbolic import (observed voices: {}).\n\
         DERIVED ANALYSIS (not score metadata): {} over {}-beat windows:\n",
        reference
            .voices
            .iter()
            .map(|v| v.label.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        derived.method,
        derived.window.beats()
    );
    for c in &derived.chords {
        writeln!(
            report,
            "  {:>5.2}..{:>5.2}  root +{} {:?}",
            c.at.beats(),
            c.end.beats(),
            c.relative_root,
            c.quality
        )?;
    }
    // The current (v1) cover lab map: the grammar contract's anchors on the observed melody only.
    let v1 = reference.extract(CoverSpec::from_contract(&CoherenceContract::for_grammar(
        CompositionGrammar::HookArc,
    )))?;
    let mut maps: Vec<(String, CoverMap, String)> = vec![(
        "0_v1-motif-only".into(),
        v1,
        "v1 (current binary spec): observed melody pinned with its notated rests; everything else unknown\n".into(),
    )];
    for (i, preset) in CoverFidelityPreset::ALL.into_iter().enumerate() {
        let (map, fidelity) = reference.extract_fidelity(
            &CoverFidelityProfile::preset(preset),
            Some(preset),
            Some(&derived),
        )?;
        maps.push((
            format!("{}_{}", i + 1, preset.label()),
            map,
            fidelity.report(),
        ));
    }
    let targets = [
        ("black_ice", MusicWorld::black_ice(), 901u64, 95.0f32),
        ("vapor95", vapor95_world(), 904, 95.0),
    ];
    let profile = PerformanceProfile::BAND;
    writeln!(
        report,
        "\nTarget performance profile for every cover: PerformanceProfile::BAND (rehearsed admission, Balanced drummer). Seeds and worlds fixed per column; only the fidelity changes.\n"
    )?;
    for (name, map, fidelity) in &maps {
        writeln!(
            report,
            "== {name}: CoverMap canonical={:016x}\n{fidelity}",
            map.canonical_fingerprint()
        )?;
        std::fs::write(
            out.join(format!("{name}.cover-map.txt")),
            format!("{map:#?}\n"),
        )?;
        for (world_name, mut world, seed, tempo) in targets.clone() {
            world.tempo_bpm = tempo;
            let title = format!(
                "{BAND_NAME} — Ode to Joy ({} cover, fidelity: {})",
                world.name,
                name.split_once('_').map_or(name.as_str(), |x| x.1)
            );
            match cover_candidate(
                map,
                CoverTarget {
                    world: &world,
                    seed,
                    grammar: CompositionGrammar::HookArc,
                    options: PerformanceOptions::default(),
                    profile,
                },
            ) {
                Ok(c) => {
                    let conformance = CoverConformance::check(map, &c, &world);
                    writeln!(
                        report,
                        "  {title}\n    file {name}.{world_name}.wav; score={:016x}; {}",
                        c.score.canonical_fingerprint(),
                        conformance.report().replace('\n', "\n    ")
                    )?;
                    if should_render {
                        render(
                            &out.join(format!("{name}.{world_name}.wav")),
                            &c,
                            &world,
                            StemMask::full(),
                        )?;
                    }
                }
                Err(e) => writeln!(report, "  {title}\n    EXPLICIT REFUSAL: {e}")?,
            }
        }
        report.push('\n');
    }
    report.push_str(
        "Listening question: which fidelity crosses from new music using Ode material to obviously the same song, and which becomes too literal? Recognition is UNVERIFIED by any machine check here.\n",
    );
    std::fs::write(out.join("report.txt"), &report)?;
    print!("{report}");
    Ok(())
}
