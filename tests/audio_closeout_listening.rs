//! Closeout-round listening device — NOT a test of quality, and never run by default.
//!
//! Renders the BAND candidate (`perform_candidate`: what the realizer produces, admitted or not)
//! of every fresh falsifier the closeout round's three laws were derived from, as a full mix plus
//! the stem(s) the law touches, and writes each take's pitched notes and receipt verdict as text.
//! The same file is run at the pre-round head and at the final head into two directories, so
//! "before" and "after" come from one device; a script then diffs the note lists to explain
//! machine-wise what changed. The maintainer listens; nothing here claims a musical result.
//!
//! Run: `HUMANMUSIC_CLOSEOUT_LISTENING_OUT=<new dir> cargo test --release --test
//! audio_closeout_listening -- --ignored --nocapture`.
//!
//! The closeout corpus was rendered under the archival VAPOR95 v1 world; its VAPOR95 cases use the
//! fixture so the recorded hashes stay reproducible after the palette revision.
#[path = "common/vapor95_v1.rs"]
mod vapor95_v1;

use gibson::audio::{
    human_music::{
        composer::Composer,
        contract::CompositionGrammar,
        functor::perform_candidate,
        language::MusicalLanguage,
        performance::PerformanceOptions,
        policy::PerformanceProfile,
        receipt::PerformanceReceipt,
        semantic::{deflected_lift_trace, demo_trace},
        synth::StemMask,
        HumanMusicSynth, MusicWorld, SemanticTrace, SongMap,
    },
    render::OfflineRenderer,
    wav::write_wav_i16,
    SampleRate,
};
use std::fmt::Write as _;
use std::path::Path;

struct Case {
    name: &'static str,
    trace: fn(f64) -> SemanticTrace,
    beats: f64,
    seed: u64,
    grammar: CompositionGrammar,
    composer: Composer,
    world: fn() -> MusicWorld,
    fusion: bool,
    tempo: Option<f32>,
    stems: &'static [&'static str],
}

const CASES: [Case; 12] = [
    // Law 1 — the sounding support answers for the chart's identity.
    Case {
        name: "identity_pad_vapor95_demo7.25_s96800019",
        trace: demo_trace,
        beats: 7.25,
        seed: 96_800_019,
        grammar: CompositionGrammar::DeflectedLift,
        composer: Composer::StructuralR9,
        world: vapor95_v1::vapor95_v1,
        fusion: true,
        tempo: None,
        stems: &["pad"],
    },
    Case {
        name: "identity_keys_vapor95_deflected33.25_s96800001",
        trace: deflected_lift_trace,
        beats: 33.25,
        seed: 96_800_001,
        grammar: CompositionGrammar::DeflectedLift,
        composer: Composer::StructuralR9,
        world: vapor95_v1::vapor95_v1,
        fusion: true,
        tempo: None,
        stems: &["keys"],
    },
    Case {
        name: "identity_keys_vapor95_deflected64_s96860001",
        trace: deflected_lift_trace,
        beats: 64.0,
        seed: 96_860_001,
        grammar: CompositionGrammar::DeflectedLift,
        composer: Composer::StructuralR9,
        world: vapor95_v1::vapor95_v1,
        fusion: true,
        tempo: None,
        stems: &["keys"],
    },
    Case {
        name: "identity_swell_vapor95_demo64_meaning_s96860007",
        trace: demo_trace,
        beats: 64.0,
        seed: 96_860_007,
        grammar: CompositionGrammar::DeflectedLift,
        composer: Composer::MeaningDirected,
        world: vapor95_v1::vapor95_v1,
        fusion: true,
        tempo: None,
        stems: &["pad", "keys"],
    },
    Case {
        name: "identity_pad_black_ice_deflected64_meaning_s96900010",
        trace: deflected_lift_trace,
        beats: 64.0,
        seed: 96_900_010,
        grammar: CompositionGrammar::DeflectedLift,
        composer: Composer::MeaningDirected,
        world: MusicWorld::black_ice,
        fusion: true,
        tempo: None,
        stems: &["pad"],
    },
    Case {
        name: "identity_pad_vapor95_66bpm_deflected64_meaning_s96910000",
        trace: deflected_lift_trace,
        beats: 64.0,
        seed: 96_910_000,
        grammar: CompositionGrammar::DeflectedLift,
        composer: Composer::MeaningDirected,
        world: vapor95_v1::vapor95_v1,
        fusion: true,
        tempo: Some(66.0),
        stems: &["pad"],
    },
    Case {
        name: "identity_release_vapor95_96bpm_deflected33.25_propulsive_s96910002",
        trace: deflected_lift_trace,
        beats: 33.25,
        seed: 96_910_002,
        grammar: CompositionGrammar::PropulsiveReturn,
        composer: Composer::StructuralR9,
        world: vapor95_v1::vapor95_v1,
        fusion: true,
        tempo: Some(96.0),
        stems: &["pad"],
    },
    // Law 2 — a relational function names a destination that sounds and resolves.
    Case {
        name: "appoggiatura_black_ice_deflected33.25_s96800004",
        trace: deflected_lift_trace,
        beats: 33.25,
        seed: 96_800_004,
        grammar: CompositionGrammar::DeflectedLift,
        composer: Composer::StructuralR9,
        world: MusicWorld::black_ice,
        fusion: true,
        tempo: None,
        stems: &["lead"],
    },
    Case {
        name: "appoggiatura_black_ice_deflected33.25_s96800009",
        trace: deflected_lift_trace,
        beats: 33.25,
        seed: 96_800_009,
        grammar: CompositionGrammar::DeflectedLift,
        composer: Composer::StructuralR9,
        world: MusicWorld::black_ice,
        fusion: true,
        tempo: None,
        stems: &["lead"],
    },
    // Law 3 — a declared bass figure keeps the downbeat its own line needs.
    Case {
        name: "bassfigure_black_ice_deflected2.5_riffdrive_s96810002",
        trace: deflected_lift_trace,
        beats: 2.5,
        seed: 96_810_002,
        grammar: CompositionGrammar::RiffDrive,
        composer: Composer::StructuralR9,
        world: MusicWorld::black_ice,
        fusion: true,
        tempo: None,
        stems: &["bass"],
    },
    Case {
        name: "bassfigure_vapor95_deflected4_worldswitch_s96810002",
        trace: deflected_lift_trace,
        beats: 4.0,
        seed: 96_810_002,
        grammar: CompositionGrammar::WorldSwitch,
        composer: Composer::StructuralR9,
        world: vapor95_v1::vapor95_v1,
        fusion: true,
        tempo: None,
        stems: &["bass"],
    },
    Case {
        name: "bassfigure_swiss_deflected2.5_worldswitch_meaning_s96810007",
        trace: deflected_lift_trace,
        beats: 2.5,
        seed: 96_810_007,
        grammar: CompositionGrammar::WorldSwitch,
        composer: Composer::MeaningDirected,
        world: MusicWorld::swiss_signal,
        fusion: true,
        tempo: None,
        stems: &["bass"],
    },
];

fn render(
    path: &Path,
    c: &gibson::audio::human_music::functor::Composition,
    world: &MusicWorld,
    mask: StemMask,
) {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::new(&c.score, world, rate);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    let audio = OfflineRenderer::new(rate, 256).render(&mut synth, frames);
    assert!(!audio.had_nonfinite, "nonfinite render");
    write_wav_i16(path, &audio.audio, rate).expect("wav");
}

#[test]
#[ignore = "listening device; release only; set HUMANMUSIC_CLOSEOUT_LISTENING_OUT"]
fn closeout_listening() {
    let out = std::env::var("HUMANMUSIC_CLOSEOUT_LISTENING_OUT").expect("output directory");
    let out = Path::new(&out);
    std::fs::create_dir_all(out).expect("output directory");
    for c in &CASES {
        let song = SongMap::compose(&(c.trace)(c.beats), c.seed, Some(c.grammar), c.composer);
        let mut world = (c.world)();
        if let Some(t) = c.tempo {
            world.tempo_bpm = t;
        }
        let opts = PerformanceOptions {
            language: if c.fusion {
                MusicalLanguage::fusion_conversation()
            } else {
                MusicalLanguage::simple()
            },
            ..PerformanceOptions::default()
        };
        let take = perform_candidate(&song, &world, opts, PerformanceProfile::BAND)
            .expect("lawful candidate");
        let receipt = PerformanceReceipt::measure_under(&take, &world, PerformanceProfile::BAND);
        let mut text = format!(
            "# {}\n# receipt: {}\n",
            c.name,
            if receipt.passes() {
                "PASS".to_string()
            } else {
                format!("FAIL {:?}", receipt.failures())
            }
        );
        let mut notes: Vec<_> = take.score.notes.iter().collect();
        notes.sort_by(|a, b| {
            (a.role.label(), a.start_beat, a.pitch)
                .partial_cmp(&(b.role.label(), b.start_beat, b.pitch))
                .expect("finite")
        });
        for n in notes {
            let _ = writeln!(
                text,
                "{}\t{:.4}\t{:.4}\t{}\t{}\t{:?}",
                n.role.label(),
                n.start_beat,
                n.dur_beats,
                n.pitch,
                n.prov.role_note,
                n.function
            );
        }
        std::fs::write(out.join(format!("{}.notes.tsv", c.name)), text).expect("notes");
        render(
            &out.join(format!("{}.full.wav", c.name)),
            &take,
            &world,
            StemMask::full(),
        );
        for stem in c.stems {
            render(
                &out.join(format!("{}.{stem}.wav", c.name)),
                &take,
                &world,
                StemMask::solo(stem),
            );
        }
    }
}
