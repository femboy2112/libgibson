//! Reaction-cut SOUNDTRACK — the 0.4 audio flagship (offline render).
//!
//! Produces the audible layer for the keyed reaction cut: the ACTUAL local source audio
//! at phrase-complete, re-auditioned cue windows, over an original HumanMusic score that
//! scores the film's narrative, with the music DUCKING under speech, plus synthesized SFX
//! at cinematic beats. No prerecorded soundtrack; the only recorded audio is the local
//! reaction source itself (never committed, never shipped).
//!
//! It renders to a single mixed WAV on the SAME edit clock the visual reaction director
//! uses (`edit_seconds`), so it is A/V-aligned: play it alongside
//! `cargo run --example libgibson_intro_reaction` and the reactions land with the cuts.
//! (Live device playback / audio-as-master-clock is the `audio-cpal` path; this offline
//! render is the deterministic, listenable proof.)
//!
//! The cue windows below were RE-AUDITIONED from the source (instrument-assisted silence +
//! loudness analysis, then chosen to be phrase-complete) and LENGTHENED from the silent
//! 0.3.1 visual cuts so speech is never clipped mid-word — the edit clock breathes around
//! the dialogue rather than the dialogue being cut to the old windows.
//!
//! Run:
//!   cargo run --release --example reaction_soundtrack
//!   cargo run --release --example reaction_soundtrack -- --clip=/path/to.webm --out=/tmp/rs
//!   cargo run --release --example reaction_soundtrack -- --world=vapor95 --seed=7

use std::path::PathBuf;

use gibson::audio::human_music::semantic::{
    Density, Elevation, Emphasis, EventKind, SemanticEvent, SemanticState, SemanticTrace, Tone,
};
use gibson::audio::human_music::synth::HumanMusicSynth;
use gibson::audio::human_music::{compose, MusicWorld, WorldId};
use gibson::audio::media::{extract_clip_audio, DialogueTrack};
use gibson::audio::render::OfflineRenderer;
use gibson::audio::wav::write_wav_i16;
use gibson::audio::{SampleRate, StereoBlock};

/// A re-edited audio-aware media cue on the edit timeline.
struct MediaCue {
    name: &'static str,
    /// When it starts on the edit clock (seconds).
    edit_start: f64,
    /// Source span (seconds) — phrase-complete, re-auditioned, lengthened.
    src_start: f64,
    src_dur: f64,
    /// Dialogue placement gain.
    gain: f32,
    /// The semantic tone this beat drives the score toward.
    tone: Tone,
    kind: EventKind,
}

/// The re-authored cue sheet. Source spans are phrase-complete picks from the loudness map
/// (bracketed by real silence gaps); windows are spaced so speech never overlaps and the
/// film continues underneath. The final sting extends the edit clock past the original
/// intro — intentional (see docs/FRANK_REACTION_CUT_PLAN.md).
// A src_start of 6.28s is a source timestamp, not an approximation of TAU.
#[allow(clippy::approx_constant)]
const CUES: &[MediaCue] = &[
    MediaCue {
        name: "welcome",
        edit_start: 4.0,
        src_start: 6.28,
        src_dur: 8.91,
        gain: 0.85,
        tone: Tone::Info,
        kind: EventKind::FocusAcquired,
    },
    MediaCue {
        name: "prank_smash",
        edit_start: 16.0,
        src_start: 47.75,
        src_dur: 1.76,
        gain: 0.9,
        tone: Tone::Danger,
        kind: EventKind::Impact,
    },
    MediaCue {
        name: "escalate",
        edit_start: 22.0,
        src_start: 61.66,
        src_dur: 5.58,
        gain: 0.88,
        tone: Tone::Warning,
        kind: EventKind::ModalEntered,
    },
    MediaCue {
        name: "city_reveal",
        edit_start: 34.0,
        src_start: 99.54,
        src_dur: 5.09,
        gain: 0.85,
        tone: Tone::Accent,
        kind: EventKind::ToneShift,
    },
    MediaCue {
        name: "couriers",
        edit_start: 46.0,
        src_start: 202.90,
        src_dur: 6.50,
        gain: 0.85,
        tone: Tone::Info,
        kind: EventKind::FocusAcquired,
    },
    MediaCue {
        name: "ascent",
        edit_start: 58.0,
        src_start: 243.83,
        src_dur: 13.38,
        gain: 0.85,
        tone: Tone::Success,
        kind: EventKind::Confirmation,
    },
    MediaCue {
        name: "earth_title",
        edit_start: 74.0,
        src_start: 275.91,
        src_dur: 10.04,
        gain: 0.85,
        tone: Tone::Accent,
        kind: EventKind::ActChanged,
    },
    MediaCue {
        name: "final_sting",
        edit_start: 90.0,
        src_start: 295.11,
        src_dur: 6.59,
        gain: 0.9,
        tone: Tone::Success,
        kind: EventKind::Confirmation,
    },
];

const TAIL_SECS: f64 = 2.5;

fn arg(flag: &str) -> Option<String> {
    std::env::args()
        .skip(1)
        .find_map(|a| a.strip_prefix(flag).map(|s| s.to_string()))
}

/// Glob ~/Downloads for the reaction source (prefers a webm containing "frank"+"stop").
fn default_clip() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = std::path::Path::new(&home).join("Downloads");
    let mut webms: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "webm").unwrap_or(false))
        .collect();
    webms.sort();
    webms
        .iter()
        .find(|p| {
            let n = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            n.contains("frank") && n.contains("stop")
        })
        .cloned()
        .or_else(|| webms.first().cloned())
}

fn build_trace(bpm: f64, total_edit_secs: f64) -> SemanticTrace {
    let to_beats = |s: f64| s * bpm / 60.0;
    let mut events = vec![SemanticEvent {
        at_beat: 0.0,
        state: SemanticState {
            tone: Tone::Neutral,
            emphasis: Emphasis::Muted,
            density: Density::Spacious,
            elevation: Elevation::Flat,
        },
        kind: EventKind::ActChanged,
    }];
    for c in CUES {
        let (emphasis, density, elevation) = match c.tone {
            Tone::Danger => (Emphasis::Strong, Density::Compact, Elevation::Overlay),
            Tone::Warning => (Emphasis::Strong, Density::Compact, Elevation::Raised),
            Tone::Success => (Emphasis::Normal, Density::Normal, Elevation::Raised),
            Tone::Accent => (Emphasis::Normal, Density::Normal, Elevation::Raised),
            _ => (Emphasis::Normal, Density::Normal, Elevation::Raised),
        };
        events.push(SemanticEvent {
            at_beat: to_beats(c.edit_start),
            state: SemanticState {
                tone: c.tone,
                emphasis,
                density,
                elevation,
            },
            kind: c.kind,
        });
    }
    SemanticTrace::new(events, to_beats(total_edit_secs))
}

fn main() -> std::io::Result<()> {
    let seed: u64 = arg("--seed=").and_then(|s| s.parse().ok()).unwrap_or(2112);
    let out_dir = arg("--out=")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("libgibson_reaction_audio"));
    std::fs::create_dir_all(&out_dir)?;
    let world_id = match arg("--world=").as_deref() {
        Some("vapor95") => WorldId::Vapor95,
        Some("swiss_signal") => WorldId::SwissSignal,
        _ => WorldId::BlackIce,
    };
    let world = MusicWorld::from_id(world_id);
    let sr = SampleRate::STUDIO;
    let block = 512;

    let clip = arg("--clip=").map(PathBuf::from).or_else(default_clip);
    let total_edit_secs = CUES
        .iter()
        .map(|c| c.edit_start + c.src_dur)
        .fold(0.0f64, f64::max)
        + TAIL_SECS;

    println!(
        "Reaction soundtrack — world={} seed={seed} sr={} Hz",
        world.name,
        sr.get()
    );
    println!(
        "edit duration: {total_edit_secs:.1}s  ({} cues)\n",
        CUES.len()
    );

    // --- HumanMusic underscore for the whole film. ---
    let trace = build_trace(world.tempo_bpm as f64, total_edit_secs);
    let score = compose(&trace, &world, seed);
    score.validate().expect("score invariants");
    let mut synth = HumanMusicSynth::new(&score, &world, sr);

    // --- Dialogue: extract each phrase-complete cue window. ---
    let mut dialogue = DialogueTrack::new();
    let mut placed = 0usize;
    match &clip {
        Some(path) => {
            println!("source clip: {}", path.display());
            println!("re-authored cues (phrase-complete, lengthened):");
            for c in CUES {
                match extract_clip_audio(path, c.src_start, c.src_dur, sr) {
                    Ok(mut ca) => {
                        ca.apply_fades(0.010, 0.015); // click-safe edges, not phrase cuts
                        let peak = ca.peak();
                        // Normalize hot clips (float/Opus decode can exceed unity) so the
                        // master limiter doesn't pump on a loud phrase; quiet clips keep
                        // the authored gain.
                        let gain = c.gain.min(0.9 / peak.max(1e-3));
                        println!(
                            "  {:<12} edit {:>5.1}s  src {:>6.2}..{:<6.2} ({:.2}s)  peak={:.2} gain={:.2}",
                            c.name, c.edit_start, c.src_start, c.src_start + c.src_dur, ca.seconds(), peak, gain
                        );
                        dialogue.place(ca, c.edit_start, gain);
                        placed += 1;
                    }
                    Err(e) => println!("  {:<12} SKIPPED (extract failed: {e})", c.name),
                }
            }
        }
        None => println!("no source clip found in ~/Downloads — rendering music-only underscore."),
    }
    println!();

    let total_samples = synth
        .total_samples()
        .max(dialogue.end_sample() + sr.samples_in(TAIL_SECS));

    // --- Ducking sidechain from the dialogue, applied to the music bus. ---
    if !dialogue.is_empty() {
        let duck = dialogue.duck_curve(total_samples as usize, sr, 0.34, 8.0, 260.0);
        synth.set_duck(duck);
    }

    // --- Render the two buses and mix. ---
    let music = OfflineRenderer::new(sr, block).render(&mut synth, total_samples);
    let dialogue_buf = OfflineRenderer::new(sr, block).render(&mut dialogue, total_samples);

    let mut master = StereoBlock::new(total_samples as usize);
    master.add_scaled(&music.audio, 0.72);
    master.add_scaled(&dialogue_buf.audio, 1.0);
    // Final safety limiter so dialogue+music overlaps never clip.
    let mut lim = gibson::audio::dsp::fx::Limiter::new(sr.as_f64() as f32);
    lim.set_ceiling(0.97);
    for i in 0..master.frames() {
        let (l, r) = lim.process_stereo(master.left[i], master.right[i]);
        master.left[i] = l;
        master.right[i] = r;
    }

    let path = out_dir.join(format!(
        "reaction_soundtrack_{}.wav",
        world.name.to_lowercase()
    ));
    write_wav_i16(&path, &master, sr)?;

    println!(
        "mix: {:.1}s  dialogue_clips={placed}  music_peak={:.2}  master_peak={:.3} rms={:.3}",
        master.frames() as f64 / sr.as_f64(),
        music.peak,
        master.peak(),
        master.rms()
    );
    println!(
        "safety: nonfinite={}  music_max_voices={}",
        master.has_nonfinite(),
        music.max_active_voices
    );
    println!("wav: {}", path.display());
    println!("\nA/V: this render shares the visual director's edit_seconds clock.");
    println!("Play alongside `cargo run --example libgibson_intro_reaction` for synced A/V.");
    Ok(())
}
