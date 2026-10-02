//! VAPOR95 palette revision — archival reproducibility, the world-change law, and the
//! production screens.
//!
//! The archival VAPOR95 v1 world lives in `tests/common/vapor95_v1.rs` (a fixture, never a
//! `WorldId`); the in-crate copy used by unit tests is checked against the same committed dump.
#[path = "common/vapor95_v1.rs"]
mod vapor95_v1;

use gibson::audio::{
    human_music::{
        composer::Composer,
        contract::CompositionGrammar,
        fingerprint::CanonicalFingerprint,
        functor::{perform_checked, Composition},
        language::MusicalLanguage,
        performance::PerformanceOptions,
        policy::PerformanceProfile,
        receipt::PerformanceRejection,
        semantic::{deflected_lift_trace, demo_trace},
        song::SongMapConformance,
        synth::{ProductionControl, StemMask},
        HumanMusicSynth, MusicWorld, SemanticTrace, SongMap,
    },
    render::OfflineRenderer,
    SampleRate, StereoBlock,
};

/// The representative takes of the style baseline (`vapor95_style_lab`): name, trace, beats,
/// seed, grammar, composer, fusion language.
type Take = (
    &'static str,
    fn(f64) -> SemanticTrace,
    f64,
    u64,
    CompositionGrammar,
    Composer,
    bool,
);
const TAKES: [Take; 4] = [
    (
        "hookarc",
        demo_trace,
        64.0,
        98_000_101,
        CompositionGrammar::HookArc,
        Composer::StructuralR9,
        true,
    ),
    (
        "deflected",
        deflected_lift_trace,
        64.0,
        98_000_202,
        CompositionGrammar::DeflectedLift,
        Composer::StructuralR9,
        true,
    ),
    (
        "propulsive",
        demo_trace,
        64.0,
        98_000_303,
        CompositionGrammar::PropulsiveReturn,
        Composer::StablePropulsion,
        false,
    ),
    (
        "riffdrive",
        demo_trace,
        64.0,
        98_000_404,
        CompositionGrammar::RiffDrive,
        Composer::MeaningDirected,
        true,
    ),
];

fn song(t: &Take) -> SongMap {
    SongMap::compose(&(t.1)(t.2), t.3, Some(t.4), t.5)
}

fn options(t: &Take) -> PerformanceOptions {
    PerformanceOptions {
        language: if t.6 {
            MusicalLanguage::fusion_conversation()
        } else {
            MusicalLanguage::simple()
        },
        ..PerformanceOptions::default()
    }
}

fn band(song: &SongMap, world: &MusicWorld, t: &Take) -> Result<Composition, PerformanceRejection> {
    perform_checked(song, world, options(t), PerformanceProfile::BAND)
}

#[test]
fn the_archival_v1_fixture_is_the_committed_dump() {
    assert_eq!(
        format!("{:#?}\n", vapor95_v1::vapor95_v1()),
        include_str!("../docs/fixtures/humanmusic-vaporize/v1-baseline/world-v1.txt")
    );
}

/// Historical VAPOR95 stays reproducible: under the archival world every baseline take is
/// re-performed with the exact SongMap, Score and PerformancePlan fingerprints recorded at
/// `abc3f9f` (the PCM of all 30 baseline WAVs is the lab's `--record --world=v1` gate).
#[test]
fn the_archival_v1_world_reproduces_the_frozen_style_baseline() {
    let record = include_str!("../docs/fixtures/humanmusic-vaporize/v1-baseline/record.txt");
    let v1 = vapor95_v1::vapor95_v1();
    for t in TAKES.iter().take(2) {
        let line = record
            .lines()
            .find(|l| l.starts_with(&format!("{}: song=", t.0)))
            .expect("recorded take");
        let c = band(&song(t), &v1, t).expect("admitted under v1");
        let got = format!(
            "{}: song={:016x} score={:016x} perf={:016x} tempo={} swing={}",
            t.0,
            c.song.fingerprint(),
            c.score.canonical_fingerprint(),
            c.perf.canonical_fingerprint(),
            c.score.tempo_bpm,
            v1.swing
        );
        assert_eq!(got, line);
    }
}

/// The world change is a natural transformation: for one SongMap, every world's BAND take is
/// admitted (or lawfully refused by its vocabulary) and keeps the same song — fingerprint, form
/// topology, identity sites, harmonic landmarks, chord changes and settled obligations — while
/// tempo, voicing, colour, timbre and production are the world's.
#[test]
fn the_revised_vapor95_is_a_natural_transformation_of_the_song() {
    let worlds = [
        ("v1", vapor95_v1::vapor95_v1()),
        ("vapor95", MusicWorld::vapor95()),
        ("black_ice", MusicWorld::black_ice()),
        ("swiss", MusicWorld::swiss_signal()),
    ];
    for t in &TAKES {
        let song = song(t);
        let mut coordinates: Vec<(&str, String)> = Vec::new();
        for (name, world) in &worlds {
            let c = match band(&song, world, t) {
                Ok(c) => c,
                Err(PerformanceRejection::Refused(e)) if *name == "swiss" => {
                    eprintln!("{}: swiss lawfully refused: {e}", t.0);
                    continue;
                }
                Err(e) => panic!("{} under {name}: {e}", t.0),
            };
            let law = SongMapConformance::check(&song, &c.perf, &c.score);
            assert!(law.passes(), "{} under {name}: {}", t.0, law.report());
            let form: Vec<(String, u32, u32)> = c
                .score
                .sections
                .iter()
                .map(|s| (format!("{:?}", s.kind), s.start_bar, s.bars))
                .collect();
            let unresolved: Vec<String> = law
                .unresolved_song_obligations
                .iter()
                .map(|d| format!("{:?}", d.obligation))
                .collect();
            coordinates.push((
                name,
                format!(
                    "song={:016x} performed={:016x} form={form:?} sites={} landmarks={} \
                     changes={} open={unresolved:?} beats={}",
                    song.fingerprint(),
                    law.performed,
                    law.sites_checked,
                    law.landmarks_checked,
                    law.changes_checked,
                    c.score.total_beats
                ),
            ));
            if *name == "vapor95" {
                assert_eq!(c.score.tempo_bpm, world.tempo_bpm);
            }
        }
        let (first, reference) = &coordinates[0];
        for (name, got) in &coordinates[1..] {
            assert_eq!(got, reference, "{}: {name} vs {first}", t.0);
        }
    }
}

fn render(c: &Composition, world: &MusicWorld, production: ProductionControl) -> StereoBlock {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::with_production(&c.score, world, rate, production);
    synth.set_stem_mask(StemMask::full());
    let frames = synth.total_samples();
    let out = OfflineRenderer::new(rate, 256).render(&mut synth, frames);
    assert!(!out.had_nonfinite, "nonfinite render");
    out.audio
}

fn energy(x: &[f32]) -> f64 {
    x.iter().map(|&s| f64::from(s) * f64::from(s)).sum()
}

/// Machine screens of the new production (gross failures only — no quality claim): a short take
/// renders finite and deterministic, never exceeds the ceiling, its echo tail decays instead of
/// running away, the new stages each change the sound, and the harmonic reference removes them.
#[test]
fn the_new_vapor95_production_is_bounded_deterministic_and_ablatable() {
    let world = MusicWorld::vapor95();
    let t = &TAKES[0];
    let song = SongMap::compose(&demo_trace(12.0), t.3, Some(t.4), t.5);
    let c = band(&song, &world, t).expect("admitted");
    let a = render(&c, &world, ProductionControl::NORMAL);
    let b = render(&c, &world, ProductionControl::NORMAL);
    assert!(
        a.left
            .iter()
            .zip(&b.left)
            .all(|(x, y)| x.to_bits() == y.to_bits())
            && a.right
                .iter()
                .zip(&b.right)
                .all(|(x, y)| x.to_bits() == y.to_bits()),
        "nondeterministic render"
    );
    let peak = a
        .left
        .iter()
        .chain(&a.right)
        .fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(peak <= world.master_ceiling + 1e-6, "peak {peak}");
    // The final half second (inside the 2.5 s render tail) is far below the body: no runaway.
    let n = a.left.len();
    let tail = n - SampleRate::STUDIO.as_f64() as usize / 2;
    let body = energy(&a.left[..tail]) / tail as f64;
    let end = energy(&a.left[tail..]) / (n - tail) as f64;
    assert!(end < body * 1e-3, "tail {end} vs body {body}");
    // Every new stage is audible on its own, and the harmonic reference strips them all.
    for spec in ["nochorus", "noecho", "fullband"] {
        let x = render(&c, &world, ProductionControl::parse(spec).expect("toggle"));
        let diff: f64 = x
            .left
            .iter()
            .zip(&a.left)
            .map(|(p, q)| f64::from(p - q).powi(2))
            .sum();
        assert!(diff > 0.0, "{spec} changed nothing");
    }
    let r = ProductionControl::HARMONIC_REFERENCE;
    assert!(r.no_chorus && r.no_echo && r.full_band_space && r.dry);
}
