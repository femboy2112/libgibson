//! **Songcraft listening probe** (the fourth rung, layered on `groove_probe`) — reframed around
//! Leah's own-music study (see memory `leah-musical-language`): her flat velocity and dead-grid
//! timing are *limitations she hit*, not aesthetic verdicts, so the engine should try to EXCEED them
//! as additive features done coherently over song structure — each tested in isolation by her ear.
//!
//! Held FIXED from `groove_probe` (verbatim): tempo, the Am–F–C–G home loop, the split guide-tone /
//! open-fifth **interdependent voicing** (load-bearing, per her), the cross-voice suspension, the drum
//! pattern and fills, and the unified POCKET (whole band leans behind the kick/hats anchor). On top:
//! a germ-based melody (verse antecedent asks, hook consequent answers — one "reach upward" germ) and
//! a bridge on Dm–F–C–E (the raised-G# dominant → V-ish pull home) into a biggest final chorus.
//!
//! Velocity shaping is now **coherent over song structure**: a baked per-note phrase arc PLUS section
//! terracing (verse 0.90, chorus 1.00, bridge crescendo 0.95→1.05, final chorus 1.12), with a varied
//! 2nd pass (a chromatic grace a semitone under the phrase's peak). Structured + relational, never
//! random jitter (standing rule): every render is byte-identical.
//!
//!   cargo run --release --example songcraft_probe -- --out=target/humanmusic-beefup/songcraft
//!
//! Three ISOLATION toggles (each changes ONE axis; everything else is held so the A/B is clean):
//!   * `--flatvel`  — flatten the lead to one constant velocity (keep grace, rhythm, pocket, bridge).
//!                    Isolates velocity SHAPING vs her flat baseline.
//!   * `--grid`     — zero the pocket lean (dead grid), everything else identical. Isolates MICRO-TIMING.
//!   * `--open`     — the final hook hangs (ends on D, outro holds B) instead of resolving home to A.
//!                    Isolates the ENDING (open-vs-resolved is a dial).
//! Plus `--no-bridge` (40-bar form), `--world=…`, `--out=`. Not shipped; a hand-authored listening
//! control on the beef-up branch, NOT the generator.

use std::path::PathBuf;

use gibson::audio::human_music::form::SectionKind;
use gibson::audio::human_music::score::{DrumHit, DrumVoice, Note, Provenance, Role, Score};
use gibson::audio::human_music::{HumanMusicSynth, MusicWorld, WorldId};
use gibson::audio::wav::write_wav_i16;
use gibson::audio::{OfflineRenderer, SampleRate};

const TEMPO: f32 = 104.0;
const BPB: f64 = 4.0;

/// The confirmed POCKET lean, in beats (~13ms at this tempo). Zeroed under `--grid`.
const LEAN: f64 = 0.022;
/// The flat lead velocity used under `--flatvel` (her baseline: pitched leads at one level).
const FLAT_LEAD: f32 = 0.80;

/// Per-render configuration. The three isolation axes plus the bridge switch.
#[derive(Clone, Copy)]
struct Cfg {
    lean: f64,     // LEAN normally, 0.0 under --grid
    flat_vel: bool, // --flatvel
    open: bool,     // --open
    bridge: bool,   // !--no-bridge
}

/// Place a pitched onset in the pocket: every pitched voice leans together by `cfg.lean`.
fn place_note(beat: f64, cfg: Cfg) -> f64 {
    (beat + cfg.lean).max(0.0)
}

/// Place a drum onset: kick+hats keep the time tight (the anchor); only the backbeat leans.
fn place_drum(beat: f64, voice: DrumVoice, cfg: Cfg) -> f64 {
    let off = match voice {
        DrumVoice::Snare | DrumVoice::Clap => cfg.lean,
        _ => 0.0,
    };
    (beat + off).max(0.0)
}

/// The chords, as roles-in-a-band. Home loop (Am/F/C/G) verbatim from `groove_probe`; `Dm`/`E` are the
/// bridge's contrasting harmony — `E` carries the raised G# (the colored dominant that pulls home).
#[derive(Clone, Copy, PartialEq)]
enum Ch {
    Am,
    F,
    C,
    G,
    Dm,
    E,
}

impl Ch {
    fn root(self) -> i32 {
        match self {
            Ch::Am => 45,
            Ch::F => 41,
            Ch::C => 48,
            Ch::G => 43,
            Ch::Dm => 38,
            Ch::E => 40,
        }
    }
    fn fifth(self) -> i32 {
        match self {
            Ch::Am => 52,
            Ch::F => 48,
            Ch::C => 55,
            Ch::G => 50,
            Ch::Dm => 45,
            Ch::E => 47,
        }
    }
    fn pad_fifth(self) -> [i32; 2] {
        match self {
            Ch::Am => [57, 64],
            Ch::F => [53, 60],
            Ch::C => [60, 67],
            Ch::G => [55, 62],
            Ch::Dm => [50, 57],
            Ch::E => [52, 59],
        }
    }
    fn guide_low(self) -> i32 {
        match self {
            Ch::Am => 60,
            Ch::F => 57,
            Ch::C => 59,
            Ch::G => 59,
            Ch::Dm => 53,
            Ch::E => 56, // G#3 — the raised leading tone
        }
    }
    fn guide_hi(self) -> i32 {
        match self {
            Ch::Am => 67,
            Ch::F => 64,
            Ch::C => 64,
            Ch::G => 65,
            Ch::Dm => 60,
            Ch::E => 62, // D4 — the dominant's b7
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
    Intro,
    Verse,
    Chorus,
    Bridge,
    Outro,
}

impl Part {
    fn section_kind(self) -> SectionKind {
        match self {
            Part::Intro => SectionKind::Intro,
            Part::Verse => SectionKind::A,
            Part::Chorus => SectionKind::Climax,
            Part::Bridge => SectionKind::Contrast,
            Part::Outro => SectionKind::Coda,
        }
    }
}

// One germ — "reach upward through the chord" — realized three ways. (beat, pitch, dur, vel); the vel
// column is the baked phrase arc, the dur column the light articulation (0.92 quarter, 1.9 half).
const VERSE: &[(f64, i32, f32, f32)] = &[
    (0.0, 64, 0.92, 0.60), (1.0, 69, 0.92, 0.68), (2.0, 71, 1.9, 0.74),
    (4.0, 69, 0.92, 0.66), (5.0, 67, 0.92, 0.62), (6.0, 65, 1.9, 0.58),
    (8.0, 67, 0.92, 0.60), (9.0, 64, 0.92, 0.56), (10.0, 62, 1.9, 0.54),
    (12.0, 62, 0.92, 0.56), (13.0, 64, 0.92, 0.58), (14.0, 62, 1.9, 0.52),
];
const HOOK: &[(f64, i32, f32, f32)] = &[
    (0.0, 69, 0.92, 0.80), (1.0, 72, 0.92, 0.86), (2.0, 76, 1.9, 0.92),
    (4.0, 77, 0.92, 0.88), (5.0, 76, 0.92, 0.82), (6.0, 72, 1.9, 0.74),
    (8.0, 76, 0.92, 0.78), (9.0, 74, 0.92, 0.74), (10.0, 72, 1.9, 0.70),
    (12.0, 74, 0.92, 0.70), (13.0, 71, 0.92, 0.66), (14.0, 69, 1.9, 0.64),
];
const BRIDGE: &[(f64, i32, f32, f32)] = &[
    (0.0, 62, 0.92, 0.60), (1.0, 65, 0.92, 0.64), (2.0, 69, 1.9, 0.68),
    (4.0, 69, 0.92, 0.70), (5.0, 72, 0.92, 0.74), (6.0, 77, 1.9, 0.80),
    (8.0, 76, 0.92, 0.82), (9.0, 79, 0.92, 0.86), (10.0, 76, 1.9, 0.86),
    (12.0, 71, 0.92, 0.88), (13.0, 68, 0.92, 0.90), (14.0, 71, 1.9, 0.92),
];

fn prov(part: Part) -> Provenance {
    Provenance::new(part.section_kind())
}

fn note(score: &mut Score, cfg: Cfg, bar: u32, beat: f64, pitch: i32, dur: f32, vel: f32, role: Role, part: Part) {
    let start = place_note(bar as f64 * BPB + beat, cfg);
    score.notes.push(Note::new(start, dur, pitch, vel, role, prov(part)));
}

fn hit(score: &mut Score, cfg: Cfg, bar: u32, beat: f64, voice: DrumVoice, vel: f32, part: Part) {
    let start = place_drum(bar as f64 * BPB + beat, voice, cfg);
    score.drums.push(DrumHit { start_beat: start, voice, velocity: vel, prov: prov(part) });
}

/// The drum PATTERN — verbatim from `groove_probe`. Only its placement in time (the pocket) differs.
fn drums(score: &mut Score, cfg: Cfg, bar: u32, part: Part, fill: Option<bool>) {
    let chorus = part == Part::Chorus;
    hit(score, cfg, bar, 0.0, DrumVoice::Kick, 0.95, part);
    hit(score, cfg, bar, 2.0, DrumVoice::Kick, 0.80, part);
    if chorus {
        hit(score, cfg, bar, 2.5, DrumVoice::Kick, 0.68, part);
    }
    hit(score, cfg, bar, 1.0, DrumVoice::Snare, 0.85, part);
    if chorus {
        hit(score, cfg, bar, 1.0, DrumVoice::Clap, 0.58, part);
        hit(score, cfg, bar, 3.0, DrumVoice::Clap, 0.58, part);
    }
    let hat_end: f64 = if matches!(fill, Some(true)) { 2.0 } else { 4.0 };
    let mut b: f64 = 0.0;
    while b < hat_end - 1e-9 {
        let v = if b.fract() == 0.0 { 0.48 } else { 0.38 };
        hit(score, cfg, bar, b, DrumVoice::ClosedHat, v, part);
        b += 0.5;
    }
    if chorus && !matches!(fill, Some(true)) {
        hit(score, cfg, bar, 3.5, DrumVoice::OpenHat, 0.52, part);
    }
    match fill {
        None => {
            hit(score, cfg, bar, 3.0, DrumVoice::Snare, 0.85, part);
        }
        Some(false) => {
            hit(score, cfg, bar, 3.0, DrumVoice::Snare, 0.68, part);
            hit(score, cfg, bar, 3.5, DrumVoice::Snare, 0.78, part);
        }
        Some(true) => {
            for (i, &b) in [2.0, 2.5, 3.0, 3.25, 3.5, 3.75].iter().enumerate() {
                let vel = (0.60 + 0.055 * i as f32).min(0.95);
                hit(score, cfg, bar, b, DrumVoice::Snare, vel, part);
            }
        }
    }
}

/// One bar of interdependent backing — verbatim from `groove_probe`, with the bass's chromatic approach
/// GENERIC: a half-step below the *next* root. That gives every junction its leading tone (E→Am = G#→A).
fn lay_bar(score: &mut Score, cfg: Cfg, bar: u32, ch: Ch, next_root: Option<i32>, part: Part, with_pad: bool) {
    note(score, cfg, bar, 0.0, ch.root(), 1.0, 0.82, Role::Bass, part);
    note(score, cfg, bar, 2.5, ch.fifth(), 0.75, 0.70, Role::Bass, part); // and-of-3
    if let Some(nr) = next_root {
        note(score, cfg, bar, 3.5, nr - 1, 0.5, 0.60, Role::Bass, part); // leading tone into the next root
    }

    if ch == Ch::Am {
        note(score, cfg, bar, 0.0, 60, 1.0, 0.46, Role::Keys, part); // low C
        note(score, cfg, bar, 0.0, 65, 1.0, 0.56, Role::Keys, part); // hi F (suspension)
        note(score, cfg, bar, 1.0, 64, 1.0, 0.44, Role::Keys, part); // hi E (resolution)
        note(score, cfg, bar, 2.5, 60, 1.5, 0.44, Role::Keys, part); // low C (and-of-3)
        note(score, cfg, bar, 2.5, 67, 1.5, 0.50, Role::Keys, part); // hi G (settle)
    } else {
        note(score, cfg, bar, 0.0, ch.guide_low(), 1.5, 0.46, Role::Keys, part);
        note(score, cfg, bar, 0.0, ch.guide_hi(), 1.5, 0.52, Role::Keys, part);
        note(score, cfg, bar, 2.5, ch.guide_low(), 1.5, 0.42, Role::Keys, part);
        note(score, cfg, bar, 2.5, ch.guide_hi(), 1.5, 0.48, Role::Keys, part);
    }

    if with_pad {
        for &p in &ch.pad_fifth() {
            note(score, cfg, bar, 0.0, p, 4.0, 0.40, Role::Pad, part);
        }
    }
}

/// Lay a melodic phrase. `gain` is the section terrace (verse 0.90 … final chorus 1.12). `varied` adds
/// the chromatic grace (an ornament a semitone under the phrase's peak, half a beat early). Under
/// `--flatvel` every lead note (grace included) is one flat level, isolating the velocity contour.
fn lead_phrase(score: &mut Score, cfg: Cfg, start_bar: u32, phrase: &[(f64, i32, f32, f32)], part: Part, varied: bool, gain: f32) {
    for &(b, p, d, v) in phrase {
        let vel = if cfg.flat_vel {
            FLAT_LEAD
        } else {
            (v * gain * if varied { 1.05 } else { 1.0 }).min(1.0)
        };
        note(score, cfg, start_bar, b, p, d, vel, Role::Lead, part);
    }
    if varied {
        let (peak_beat, peak_pitch, _, peak_vel) =
            phrase.iter().copied().max_by_key(|&(_, p, _, _)| p).unwrap();
        let gvel = if cfg.flat_vel { FLAT_LEAD } else { (peak_vel * gain * 1.05 * 0.92).min(1.0) };
        note(score, cfg, start_bar, peak_beat - 0.5, peak_pitch - 1, 0.25, gvel, Role::Lead, part);
    }
}

/// The last chorus phrase: resolves home (hook ends on A) by default, or hangs on D under `--open`.
fn final_hook(open: bool) -> Vec<(f64, i32, f32, f32)> {
    let mut v = HOOK.to_vec();
    if open {
        let n = v.len() - 1;
        v[n].1 = 74; // D instead of A — the consequent left unresolved
    }
    v
}

fn build_song(cfg: Cfg) -> Score {
    let bars: u32 = if cfg.bridge { 56 } else { 40 };
    let mut score = Score::new(TEMPO, BPB, bars as f64 * BPB);

    let seq = [Ch::Am, Ch::F, Ch::C, Ch::G];
    let bridge_seq = [Ch::Dm, Ch::F, Ch::C, Ch::E];
    let last_bar = bars - 1;
    let bridge = cfg.bridge;

    let chord_at = move |bar: u32| -> Ch {
        if bridge {
            match bar {
                36..=43 => bridge_seq[((bar - 36) % 4) as usize],
                52 => Ch::Am,
                53 => Ch::F,
                54 => Ch::G,
                55 => Ch::Am,
                _ => seq[(bar % 4) as usize],
            }
        } else {
            match bar {
                36 => Ch::Am,
                37 => Ch::F,
                38 => Ch::G,
                39 => Ch::Am,
                _ => seq[(bar % 4) as usize],
            }
        }
    };
    let part_at = move |bar: u32| -> Part {
        if bridge {
            match bar {
                0..=3 => Part::Intro,
                4..=11 | 20..=27 => Part::Verse,
                12..=19 | 28..=35 | 44..=51 => Part::Chorus,
                36..=43 => Part::Bridge,
                _ => Part::Outro,
            }
        } else {
            match bar {
                0..=3 => Part::Intro,
                4..=11 | 20..=27 => Part::Verse,
                12..=19 | 28..=35 => Part::Chorus,
                _ => Part::Outro,
            }
        }
    };
    let fill_at = move |bar: u32| -> Option<bool> {
        if bridge {
            match bar {
                3 | 19 | 35 | 51 => Some(false),
                11 | 27 | 43 => Some(true),
                _ => None,
            }
        } else {
            match bar {
                3 | 19 | 35 => Some(false),
                11 | 27 => Some(true),
                _ => None,
            }
        }
    };

    for bar in 0..bars {
        let part = part_at(bar);
        let ch = chord_at(bar);
        let next_root = if bar == last_bar { None } else { Some(chord_at(bar + 1).root()) };
        let with_pad = part == Part::Chorus || (part == Part::Outro && ch == Ch::Am);
        lay_bar(&mut score, cfg, bar, ch, next_root, part, with_pad);
        drums(&mut score, cfg, bar, part, fill_at(bar));
    }

    // Lead with section terracing (the "coherent over structure" dynamic arc).
    let (v, c, b) = (Part::Verse, Part::Chorus, Part::Bridge);
    const G_VERSE: f32 = 0.90;
    const G_CHORUS: f32 = 1.00;
    const G_BRIDGE_1: f32 = 0.95;
    const G_BRIDGE_2: f32 = 1.05;
    const G_FINAL: f32 = 1.12;

    lead_phrase(&mut score, cfg, 4, VERSE, v, false, G_VERSE);
    lead_phrase(&mut score, cfg, 8, VERSE, v, true, G_VERSE);
    lead_phrase(&mut score, cfg, 12, HOOK, c, false, G_CHORUS);
    lead_phrase(&mut score, cfg, 16, HOOK, c, true, G_CHORUS);
    lead_phrase(&mut score, cfg, 20, VERSE, v, false, G_VERSE);
    lead_phrase(&mut score, cfg, 24, VERSE, v, true, G_VERSE);
    lead_phrase(&mut score, cfg, 28, HOOK, c, false, G_CHORUS);

    let fin = final_hook(cfg.open);
    if cfg.bridge {
        lead_phrase(&mut score, cfg, 32, HOOK, c, true, G_CHORUS);
        lead_phrase(&mut score, cfg, 36, BRIDGE, b, false, G_BRIDGE_1);
        lead_phrase(&mut score, cfg, 40, BRIDGE, b, true, G_BRIDGE_2);
        lead_phrase(&mut score, cfg, 44, HOOK, c, false, G_FINAL);
        lead_phrase(&mut score, cfg, 48, &fin, c, true, G_FINAL); // FINAL chorus phrase
    } else {
        lead_phrase(&mut score, cfg, 32, &fin, c, true, G_CHORUS); // FINAL chorus phrase (no bridge)
    }

    // Outro: resolve home on A, or hang on B under --open.
    let ov = if cfg.flat_vel { FLAT_LEAD } else { 0.64 };
    let last_pitch = if cfg.open { 71 } else { 69 };
    note(&mut score, cfg, last_bar, 0.0, last_pitch, 4.0, ov, Role::Lead, Part::Outro);

    score
}

fn world_from(name: &str) -> Option<(WorldId, &'static str)> {
    match name {
        "black_ice" => Some((WorldId::BlackIce, "black_ice")),
        "vapor95" => Some((WorldId::Vapor95, "vapor95")),
        "swiss_signal" => Some((WorldId::SwissSignal, "swiss_signal")),
        _ => None,
    }
}

fn arg(prefix: &str) -> Option<String> {
    std::env::args().find(|a| a.starts_with(prefix)).map(|a| a[prefix.len()..].to_string())
}

fn main() -> std::io::Result<()> {
    let out = PathBuf::from(arg("--out=").unwrap_or_else(|| "target/humanmusic-beefup/songcraft".into()));
    std::fs::create_dir_all(&out)?;

    let has = |f: &str| std::env::args().any(|a| a == f);
    let cfg = Cfg {
        lean: if has("--grid") { 0.0 } else { LEAN },
        flat_vel: has("--flatvel"),
        open: has("--open"),
        bridge: !has("--no-bridge"),
    };

    let mut suffix = String::new();
    if cfg.flat_vel { suffix.push_str("_flatvel"); }
    if cfg.lean == 0.0 { suffix.push_str("_grid"); }
    if cfg.open { suffix.push_str("_open"); }
    if !cfg.bridge { suffix.push_str("_nobridge"); }

    let worlds: Vec<(WorldId, &'static str)> = match arg("--world=") {
        Some(name) => vec![world_from(&name).unwrap_or((WorldId::BlackIce, "black_ice"))],
        None => vec![
            (WorldId::BlackIce, "black_ice"),
            (WorldId::Vapor95, "vapor95"),
            (WorldId::SwissSignal, "swiss_signal"),
        ],
    };

    let sr = SampleRate::STUDIO;
    let block = 1024usize;
    let score = build_song(cfg);

    println!(
        "songcraft [{}{}{}{}]: {} notes, {} drum hits, {:.0} beats @ {} bpm ({:.1}s)",
        if cfg.flat_vel { "flat-vel" } else { "shaped" },
        if cfg.lean == 0.0 { ",grid" } else { ",pocket" },
        if cfg.open { ",open" } else { ",resolved" },
        if cfg.bridge { ",bridge" } else { ",no-bridge" },
        score.notes.len(),
        score.drums.len(),
        score.total_beats,
        TEMPO,
        score.total_beats * 60.0 / TEMPO as f64,
    );

    for (id, name) in worlds {
        let world = MusicWorld::from_id(id);
        let mut synth = HumanMusicSynth::new(&score, &world, sr);
        let frames = synth.total_samples();
        let res = OfflineRenderer::new(sr, block).render(&mut synth, frames);
        let wav = out.join(format!("songcraft_{name}{suffix}.wav"));
        write_wav_i16(&wav, &res.audio, sr)?;
        println!("  {name:12} -> {} ({} frames)", wav.display(), res.audio.frames());
    }
    Ok(())
}
