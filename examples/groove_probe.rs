//! **Groove / feel listening probe** (the third rung, layered on `ensemble_probe`).
//!
//! The ensemble probe confirmed the direction ("much better… the right direction") but left two
//! rhythmic complaints standing: the pitched voices had no groove — only the drum lane carried
//! syncopation, so bass/inner-voice/melody were still "agreeing simultaneously," just on the grid
//! now — and even the drums were "too quantized," pinned dead-on to the mathematical grid.
//!
//! The fix is the SAME ensemble idea carried into the time domain. A groove is not each voice
//! quantized independently; it is a shared POCKET the whole band sits in, each instrument placed
//! characteristically relative to it, voices syncopating WITH each other and the kick. This is
//! structured and relational, explicitly NOT random onset jitter (a standing rule): every render is
//! byte-identical. Two deterministic dials plus composed syncopation:
//!
//! * SWING — a single global ratio that moves the off-eighth later (0.5 = straight). Piecewise
//!   linear and monotonic, so sixteenths drag consistently too. Drives the shuffle in hats/fills
//!   and every off-beat pitched hit.
//! * POCKET — a per-instrument constant offset (in beats): the kick is the anchor (0), the bass
//!   locks just behind it, the backbeat snare drags a hair, the melody sings a touch late. ~6–26ms
//!   at this tempo — felt as feel, not heard as error. A shared pocket, each voice's own seat in it.
//! * Composed syncopation — the bass and inner voice now share an "and-of-3" push and an
//!   anticipation of the next chord, so the rhythm section grooves together instead of stating
//!   downbeats in parallel. This is COMPOSITION (deliberate off-beat placement), not timing noise.
//!
//! Held FIXED from `ensemble_probe`: tempo, the 40-bar form, the Am–F–C–G roots, the split
//! guide-tone / open-fifth harmony, the cross-voice suspension, the drum PATTERN and fills, and the
//! verse/hook PITCHES. Changed: where every onset sits in time, and the rhythm section's figure.
//!
//!   cargo run --release --example groove_probe -- --out=target/humanmusic-beefup/groove
//!
//! Flags: `--world=…` (default all three), `--out=`, and a feel mode:
//!
//! * default — `Shuffle`: the original swing + per-voice pocket. This is the one that galloped
//!   AND frame-dragged (a shuffle fighting a drag; three clocks at once). Kept for reference.
//! * `--straight` — `Tight`: no swing, no offsets. The syncopated writing on a dead grid.
//! * `--pocket` — `Pocket`: no swing; the whole band leans behind the tight drums by ONE shared
//!   amount. The only coherent "laid back" — a single pocket, not a clock per instrument.
//!
//! Not shipped; a hand-authored listening control on the beef-up branch, NOT the generator.

// Hand-authored listening probe: note() legitimately takes many positional musical coordinates.
#![allow(clippy::too_many_arguments)]

use std::path::PathBuf;

use gibson::audio::human_music::form::SectionKind;
use gibson::audio::human_music::score::{DrumHit, DrumVoice, Note, Provenance, Role, Score};
use gibson::audio::human_music::{HumanMusicSynth, MusicWorld, WorldId};
use gibson::audio::wav::write_wav_i16;
use gibson::audio::{OfflineRenderer, SampleRate};

const TEMPO: f32 = 104.0;
const BPB: f64 = 4.0;

// ---- Feel dials (deterministic; off under `--straight`) --------------------------------------
/// Where the off-eighth sits. 0.5 = straight; ~0.57 = a subtle shuffle. The groove's swing.
const SWING: f64 = 0.57;
/// Per-instrument pocket, in beats (positive = later / laid back). The kick is the time (0.0); the
/// bass sits just behind it; the backbeat drags; the melody sings late. A shared pocket, own seats.
const POCKET_KICK: f64 = 0.000;
const POCKET_SNARE: f64 = 0.018; // backbeat (and claps) drag a hair
const POCKET_HAT: f64 = 0.004;
const POCKET_BASS: f64 = 0.010; // locks just behind the kick
const POCKET_KEYS: f64 = 0.006; // the inner comp
const POCKET_LEAD: f64 = 0.022; // the melody sings a touch behind
const POCKET_PAD: f64 = 0.006;
/// Pocket mode: ONE shared lean (in beats) the whole band sits behind the tight drums. The lesson
/// from the galloping Shuffle mode — a groove is a single shared pocket, not a clock per instrument.
const LEAN: f64 = 0.022;

/// How the whole band relates to the grid in time. The original `Shuffle` layered a swing on top of
/// per-instrument offsets — a shuffle fighting a drag, which galloped AND frame-dragged at once. The
/// corrective modes pick ONE coherent feel: `Tight` (dead on the grid) or `Pocket` (the band leans
/// behind the drums as one body). No swing in either corrective mode — swing was the gallop.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Shuffle, // original: global swing + per-voice pocket (kept for reference; this is the one that galloped)
    Tight,   // no swing, no offsets — dead on the grid (what `--straight` produced before)
    Pocket,  // no swing; the whole band leans behind the tight drums by ONE shared amount
}

/// The feel transform, applied to every onset at emit time.
#[derive(Clone, Copy)]
struct Feel {
    mode: Mode,
}

impl Feel {
    /// Move a beat under the swing curve — only in `Shuffle`; the corrective modes keep the grid.
    fn swung(self, t: f64) -> f64 {
        if self.mode != Mode::Shuffle {
            return t;
        }
        let r = SWING;
        let base = t.floor();
        let frac = t - base;
        let nf = if frac < 0.5 {
            frac * (r / 0.5)
        } else {
            r + (frac - 0.5) * ((1.0 - r) / 0.5)
        };
        base + nf
    }
    fn place_note(self, beat: f64, role: Role) -> f64 {
        (self.swung(beat) + self.role_offset(role)).max(0.0)
    }
    fn place_drum(self, beat: f64, v: DrumVoice) -> f64 {
        (self.swung(beat) + self.drum_offset(v)).max(0.0)
    }
    fn role_offset(self, role: Role) -> f64 {
        match self.mode {
            Mode::Tight => 0.0,
            Mode::Pocket => LEAN, // the whole harmonic/melodic band leans together, locked
            Mode::Shuffle => match role {
                Role::Bass => POCKET_BASS,
                Role::Keys => POCKET_KEYS,
                Role::Lead => POCKET_LEAD,
                Role::Pad => POCKET_PAD,
            },
        }
    }
    fn drum_offset(self, v: DrumVoice) -> f64 {
        match self.mode {
            Mode::Tight => 0.0,
            // The drums keep the time tight (kick + hats are the anchor); only the backbeat leans
            // with the band, so the kit drives while the band leans into it — one coherent pocket.
            Mode::Pocket => match v {
                DrumVoice::Snare | DrumVoice::Clap => LEAN,
                _ => 0.0,
            },
            Mode::Shuffle => match v {
                DrumVoice::Kick => POCKET_KICK,
                DrumVoice::Snare | DrumVoice::Clap => POCKET_SNARE,
                DrumVoice::ClosedHat | DrumVoice::OpenHat => POCKET_HAT,
            },
        }
    }
}

/// The chords of the loop, as roles-in-a-band — unchanged from `ensemble_probe`.
#[derive(Clone, Copy, PartialEq)]
enum Ch {
    Am,
    F,
    C,
    G,
}

impl Ch {
    fn root(self) -> i32 {
        match self {
            Ch::Am => 45,
            Ch::F => 41,
            Ch::C => 48,
            Ch::G => 43,
        }
    }
    fn fifth(self) -> i32 {
        // The bass's octave/fifth neighbour, for a little syncopated motion.
        match self {
            Ch::Am => 52, // E3
            Ch::F => 48,  // C3
            Ch::C => 55,  // G3
            Ch::G => 50,  // D3
        }
    }
    fn pad_fifth(self) -> [i32; 2] {
        match self {
            Ch::Am => [57, 64],
            Ch::F => [53, 60],
            Ch::C => [60, 67],
            Ch::G => [55, 62],
        }
    }
    fn guide_low(self) -> i32 {
        match self {
            Ch::Am => 60,
            Ch::F => 57,
            Ch::C => 59,
            Ch::G => 59,
        }
    }
    fn guide_hi(self) -> i32 {
        match self {
            Ch::Am => 67,
            Ch::F => 64,
            Ch::C => 64,
            Ch::G => 65,
        }
    }
    fn approach_to_next(self) -> i32 {
        match self {
            Ch::Am => 43,
            Ch::F => 47,
            Ch::C => 42,
            Ch::G => 44,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
    Intro,
    Verse,
    Chorus,
    Outro,
}

impl Part {
    fn section_kind(self) -> SectionKind {
        match self {
            Part::Intro => SectionKind::Intro,
            Part::Verse => SectionKind::A,
            Part::Chorus => SectionKind::Climax,
            Part::Outro => SectionKind::Coda,
        }
    }
}

/// VERSE / HOOK pitches — IDENTICAL to the earlier probes (the controlled constant).
const VERSE: &[(f64, i32, f32)] = &[
    (0.0, 60, 1.0),
    (1.0, 64, 1.0),
    (2.0, 69, 1.0),
    (3.0, 64, 1.0),
    (4.0, 60, 1.0),
    (5.0, 65, 1.0),
    (6.0, 69, 2.0),
    (8.0, 67, 1.0),
    (9.0, 64, 1.0),
    (10.0, 60, 2.0),
    (12.0, 62, 1.0),
    (13.0, 67, 1.0),
    (14.0, 71, 1.0),
    (15.0, 62, 1.0),
];
const HOOK: &[(f64, i32, f32)] = &[
    (0.0, 69, 1.0),
    (1.0, 72, 1.0),
    (2.0, 76, 2.0),
    (4.0, 77, 1.0),
    (5.0, 76, 1.0),
    (6.0, 72, 2.0),
    (8.0, 76, 1.0),
    (9.0, 74, 1.0),
    (10.0, 72, 2.0),
    (12.0, 74, 1.0),
    (13.0, 71, 1.0),
    (14.0, 74, 2.0),
];

fn prov(part: Part) -> Provenance {
    Provenance::new(part.section_kind())
}

/// Emit a pitched note, placed in the pocket for its role.
fn note(
    score: &mut Score,
    feel: Feel,
    bar: u32,
    beat: f64,
    pitch: i32,
    dur: f32,
    vel: f32,
    role: Role,
    part: Part,
) {
    let raw = bar as f64 * BPB + beat;
    let start = feel.place_note(raw, role);
    score
        .notes
        .push(Note::new(start, dur, pitch, vel, role, prov(part)));
}

/// Emit a drum hit, placed in the pocket for its voice (swing drives the shuffle on the off-beats).
fn hit(score: &mut Score, feel: Feel, bar: u32, beat: f64, voice: DrumVoice, vel: f32, part: Part) {
    let raw = bar as f64 * BPB + beat;
    let start = feel.place_drum(raw, voice);
    score.drums.push(DrumHit {
        start_beat: start,
        voice,
        velocity: vel,
        prov: prov(part),
    });
}

/// The drum PATTERN — unchanged from the earlier probes. Only its placement in time now differs
/// (swing on the off-beats, a dragged backbeat), via `hit`.
fn drums(score: &mut Score, feel: Feel, bar: u32, part: Part, fill: Option<bool>) {
    let chorus = part == Part::Chorus;
    hit(score, feel, bar, 0.0, DrumVoice::Kick, 0.95, part);
    hit(score, feel, bar, 2.0, DrumVoice::Kick, 0.80, part);
    if chorus {
        hit(score, feel, bar, 2.5, DrumVoice::Kick, 0.68, part);
    }
    hit(score, feel, bar, 1.0, DrumVoice::Snare, 0.85, part);
    if chorus {
        hit(score, feel, bar, 1.0, DrumVoice::Clap, 0.58, part);
        hit(score, feel, bar, 3.0, DrumVoice::Clap, 0.58, part);
    }
    let hat_end: f64 = if matches!(fill, Some(true)) { 2.0 } else { 4.0 };
    let mut b: f64 = 0.0;
    while b < hat_end - 1e-9 {
        let v = if b.fract() == 0.0 { 0.48 } else { 0.38 };
        hit(score, feel, bar, b, DrumVoice::ClosedHat, v, part);
        b += 0.5;
    }
    if chorus && !matches!(fill, Some(true)) {
        hit(score, feel, bar, 3.5, DrumVoice::OpenHat, 0.52, part);
    }
    match fill {
        None => {
            hit(score, feel, bar, 3.0, DrumVoice::Snare, 0.85, part);
        }
        Some(false) => {
            hit(score, feel, bar, 3.0, DrumVoice::Snare, 0.68, part);
            hit(score, feel, bar, 3.5, DrumVoice::Snare, 0.78, part);
        }
        Some(true) => {
            for (i, &b) in [2.0, 2.5, 3.0, 3.25, 3.5, 3.75].iter().enumerate() {
                let vel = (0.60 + 0.055 * i as f32).min(0.95);
                hit(score, feel, bar, b, DrumVoice::Snare, vel, part);
            }
        }
    }
}

/// One bar of interdependent backing — now with a grooving, syncopated rhythm section. The harmony
/// (split guide tones, open-fifth pad, cross-voice suspension) is unchanged; the bass and inner
/// voice now share an "and-of-3" push and the bass anticipates the next root.
fn lay_bar(
    score: &mut Score,
    feel: Feel,
    bar: u32,
    ch: Ch,
    part: Part,
    with_pad: bool,
    last: bool,
) {
    // Bass: the "one", a syncopated "and-of-3" push, then the late pickup/anticipation.
    note(
        score,
        feel,
        bar,
        0.0,
        ch.root(),
        1.0,
        0.82,
        Role::Bass,
        part,
    );
    note(
        score,
        feel,
        bar,
        2.5,
        ch.fifth(),
        0.75,
        0.70,
        Role::Bass,
        part,
    ); // and-of-3 (swung)
    if !last {
        note(
            score,
            feel,
            bar,
            3.5,
            ch.approach_to_next(),
            0.5,
            0.60,
            Role::Bass,
            part,
        );
    }

    // Inner guide voice — states on the "one", then re-hits on the and-of-3 to lock with the bass.
    if ch == Ch::Am {
        // Suspension: F (b6, hung from the prior G7) resolves to E, then the duo settles on the
        // and-of-3 (low C + G), syncopated with the bass.
        note(score, feel, bar, 0.0, 60, 1.0, 0.46, Role::Keys, part); // low C
        note(score, feel, bar, 0.0, 65, 1.0, 0.56, Role::Keys, part); // hi F  (suspension)
        note(score, feel, bar, 1.0, 64, 1.0, 0.44, Role::Keys, part); // hi E  (resolution)
        note(score, feel, bar, 2.5, 60, 1.5, 0.44, Role::Keys, part); // low C (and-of-3)
        note(score, feel, bar, 2.5, 67, 1.5, 0.50, Role::Keys, part); // hi G  (settle, and-of-3)
    } else {
        note(
            score,
            feel,
            bar,
            0.0,
            ch.guide_low(),
            1.5,
            0.46,
            Role::Keys,
            part,
        );
        note(
            score,
            feel,
            bar,
            0.0,
            ch.guide_hi(),
            1.5,
            0.52,
            Role::Keys,
            part,
        );
        note(
            score,
            feel,
            bar,
            2.5,
            ch.guide_low(),
            1.5,
            0.42,
            Role::Keys,
            part,
        ); // and-of-3
        note(
            score,
            feel,
            bar,
            2.5,
            ch.guide_hi(),
            1.5,
            0.48,
            Role::Keys,
            part,
        );
    }

    // Pad: an open fifth (no third), at arrivals only — unchanged.
    if with_pad {
        for &p in &ch.pad_fifth() {
            note(score, feel, bar, 0.0, p, 4.0, 0.40, Role::Pad, part);
        }
    }
}

/// Lay a melodic phrase — SAME pitches/rhythm every call; it now rides the pocket (laid back) via `note`.
fn lead_phrase(
    score: &mut Score,
    feel: Feel,
    start_bar: u32,
    phrase: &[(f64, i32, f32)],
    vel: f32,
    part: Part,
) {
    for &(b, p, d) in phrase {
        note(score, feel, start_bar, b, p, d, vel, Role::Lead, part);
    }
}

fn build_song(feel: Feel) -> Score {
    let total_beats = 40.0 * BPB;
    let mut score = Score::new(TEMPO, BPB, total_beats);

    let seq = [Ch::Am, Ch::F, Ch::C, Ch::G];
    let chord_at = |bar: u32| -> Ch {
        match bar {
            36 => Ch::Am,
            37 => Ch::F,
            38 => Ch::G,
            39 => Ch::Am,
            _ => seq[(bar % 4) as usize],
        }
    };
    let part_at = |bar: u32| -> Part {
        match bar {
            0..=3 => Part::Intro,
            4..=11 | 20..=27 => Part::Verse,
            12..=19 | 28..=35 => Part::Chorus,
            _ => Part::Outro,
        }
    };
    let fill_at = |bar: u32| -> Option<bool> {
        match bar {
            3 => Some(false),
            11 => Some(true),
            19 => Some(false),
            27 => Some(true),
            35 => Some(false),
            _ => None,
        }
    };

    for bar in 0..40u32 {
        let part = part_at(bar);
        let ch = chord_at(bar);
        let with_pad = part == Part::Chorus || (part == Part::Outro && ch == Ch::Am);
        let last = bar == 39;
        lay_bar(&mut score, feel, bar, ch, part, with_pad, last);
        drums(&mut score, feel, bar, part, fill_at(bar));
    }

    lead_phrase(&mut score, feel, 4, VERSE, 0.70, Part::Verse);
    lead_phrase(&mut score, feel, 8, VERSE, 0.70, Part::Verse);
    lead_phrase(&mut score, feel, 12, HOOK, 0.82, Part::Chorus);
    lead_phrase(&mut score, feel, 16, HOOK, 0.82, Part::Chorus);
    lead_phrase(&mut score, feel, 20, VERSE, 0.70, Part::Verse);
    lead_phrase(&mut score, feel, 24, VERSE, 0.70, Part::Verse);
    lead_phrase(&mut score, feel, 28, HOOK, 0.82, Part::Chorus);
    lead_phrase(&mut score, feel, 32, HOOK, 0.82, Part::Chorus);
    note(
        &mut score,
        feel,
        39,
        0.0,
        69,
        4.0,
        0.68,
        Role::Lead,
        Part::Outro,
    );

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
    std::env::args()
        .find(|a| a.starts_with(prefix))
        .map(|a| a[prefix.len()..].to_string())
}

fn main() -> std::io::Result<()> {
    let out =
        PathBuf::from(arg("--out=").unwrap_or_else(|| "target/humanmusic-beefup/groove".into()));
    std::fs::create_dir_all(&out)?;

    let mode = if std::env::args().any(|a| a == "--straight") {
        Mode::Tight
    } else if std::env::args().any(|a| a == "--pocket") {
        Mode::Pocket
    } else {
        Mode::Shuffle
    };
    let feel = Feel { mode };
    let suffix = match mode {
        Mode::Tight => "_straight",
        Mode::Pocket => "_pocket",
        Mode::Shuffle => "",
    };

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
    let score = build_song(feel);

    println!(
        "grooving ensemble ({}): {} notes, {} drum hits, {:.0} beats @ {} bpm ({:.1}s)",
        match mode {
            Mode::Tight => "straight grid",
            Mode::Pocket => "unified pocket, no swing",
            Mode::Shuffle => "swing + pocket (the galloping one)",
        },
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
        let wav = out.join(format!("groove_{name}{suffix}.wav"));
        write_wav_i16(&wav, &res.audio, sr)?;
        println!(
            "  {name:12} -> {} ({} frames)",
            wav.display(),
            res.audio.frames()
        );
    }
    Ok(())
}
