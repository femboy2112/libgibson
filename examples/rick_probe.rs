//! **Rick-level listening probe** (fifth rung) — reaching for Casiopea-level *throughput*: much more
//! semantically-coherent musical information per unit time, WITHOUT decohering the song.
//!
//! The first pass under-dosed it (a timid bass push + two comp stabs at 104 bpm — "too slow / not
//! enough information"). This pass calibrates the TARGET MAGNITUDE instead: a faster tempo and real
//! density — an active **melodic bass** (chord-tone arpeggiation + chromatic approaches into each
//! change), a syncopated **counter-comp** of moving guide tones in the holes the bass leaves, and a
//! **16th-note drive** (busy hats + ghost snares) — all placed *complementarily* so voices interlock
//! rather than pile up. Two further axes were then lifted by ear: a rhythmically-alive **lead**
//! (syncopation, 8th/16th runs, pickups, developing the germ — never a static 1-2-3), and a
//! **sophisticated, traveling harmony** — a ii7–V7–Imaj7–V7/vi verse and a IV–V–ii–i chorus (extended
//! voicings), with a **bridge that modulates DOWN to D minor through an earned A7 pivot** (the home Am
//! reinterpreted as its own dominant, V7/Dm), dwells there darker, then pivots home on E7 for the biggest
//! final chorus. The shared pocket and the structure-coherent dynamics are HELD from the Morty-level song;
//! the identity anchor is the germ + form, and the new information rides on top of it (coherence budget).
//!
//!   cargo run --release --example rick_probe -- --out=target/humanmusic-beefup/rick_probe
//!
//! Flags:
//!
//! * `--sparse` — fall back to the Morty-level backing (root/fifth bass, block guide-tone keys, plain
//!   backbeat) at the same tempo, to A/B density against a thin rhythm section.
//! * `--tempo=NNN` — set the tempo in BPM (default 138; the accepted Morty song was 104).
//! * `--backing-only` — drop the lead so the rhythm-section interplay is audible on its own.
//! * `--lead-only` — drop the backing so the melody is audible naked.
//! * `--world=black_ice|vapor95|swiss_signal` (default all three), `--out=`.
//!
//! Structured + relational only, never random jitter (standing rule): every render is byte-identical.
//! Not shipped; a hand-authored listening control on the beef-up branch, NOT the generator.

#![allow(clippy::too_many_arguments)]

use std::path::PathBuf;

use gibson::audio::human_music::form::SectionKind;
use gibson::audio::human_music::score::{DrumHit, DrumVoice, Note, Provenance, Role, Score};
use gibson::audio::human_music::{HumanMusicSynth, MusicWorld, WorldId};
use gibson::audio::wav::write_wav_i16;
use gibson::audio::{OfflineRenderer, SampleRate};

const BPB: f64 = 4.0;

/// The confirmed POCKET lean, in beats (~a dozen ms). Always on (the accepted default).
const LEAN: f64 = 0.022;

/// Per-render configuration.
#[derive(Clone, Copy)]
struct Cfg {
    tempo: f32,
    dense: bool,        // false under --sparse (the Morty backing)
    backing_only: bool, // true under --backing-only (drop the lead)
    lead_only: bool,    // true under --lead-only (drop the backing — hear the melody naked)
}

fn place_note(beat: f64) -> f64 {
    (beat + LEAN).max(0.0)
}

fn place_drum(beat: f64, voice: DrumVoice) -> f64 {
    let off = match voice {
        DrumVoice::Snare | DrumVoice::Clap => LEAN,
        _ => 0.0,
    };
    (beat + off).max(0.0)
}

/// The chords, as roles-in-a-band. Home loop (Am/F/C/G); `Dm`/`E` are the bridge's contrasting
/// harmony — `E` carries the raised G# (the colored dominant that pulls home). Verbatim from `songcraft`.
#[derive(Clone, Copy, PartialEq)]
enum Ch {
    Am,
    F,
    C,
    G,
    Dm,
    E,
    A7, // the pivot: Am reinterpreted as its own dominant (V7/Dm) to earn the modulation down to Dm
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
            Ch::A7 => 45, // A (same root as Am — the pivot)
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
            Ch::A7 => 52, // E
        }
    }
    /// The chord third (for the melodic bass arpeggio), in the bass octave.
    fn third(self) -> i32 {
        match self {
            Ch::Am => 48, // C
            Ch::F => 45,  // A
            Ch::C => 52,  // E
            Ch::G => 47,  // B
            Ch::Dm => 41, // F
            Ch::E => 44,  // G# (major third — the colored dominant)
            Ch::A7 => 49, // C# (major third — the chromatic leading tone into Dm)
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
            Ch::A7 => [57, 64],
        }
    }
    fn guide_low(self) -> i32 {
        match self {
            Ch::Am => 60,
            Ch::F => 57,
            Ch::C => 59,
            Ch::G => 59,
            Ch::Dm => 53,
            Ch::E => 56,  // G#3 — the raised leading tone
            Ch::A7 => 61, // C#4 — the 3rd
        }
    }
    fn guide_hi(self) -> i32 {
        match self {
            Ch::Am => 67,
            Ch::F => 64,
            Ch::C => 64,
            Ch::G => 65,
            Ch::Dm => 60,
            Ch::E => 62,  // D4 — the dominant's b7
            Ch::A7 => 67, // G4 — the dominant's b7
        }
    }
    /// The chord seventh in the bass octave (b7 for minor/dominant, maj7 for C/F) — for the arpeggio.
    fn seventh(self) -> i32 {
        self.root()
            + if matches!(self, Ch::C | Ch::F) {
                11
            } else {
                10
            }
    }
    /// One upper-structure extension (a 9th/13th, or the colored 9th on the dominant) in the comp
    /// octave — this is what turns the triads into extended, less-elementary chords.
    fn color(self) -> i32 {
        match self {
            Ch::Am => 71, // 9th (B)
            Ch::F => 67,  // 9th (G) → Fmaj9
            Ch::C => 62,  // 9th (D) → Cmaj9
            Ch::G => 64,  // 13th (E) → G13
            Ch::Dm => 64, // 9th (E) → Dm9
            Ch::E => 66,  // 9th (F#) → E9 (a chromatic color over the dominant)
            Ch::A7 => 71, // 9th (B) → A9 (V/Dm)
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

// The germ melody — verbatim from `songcraft_probe` (the accepted Morty-level lead). At a faster tempo
// it simply reads as more urgent; the identity is unchanged.
// The germ ("reach upward") as a rhythmically ALIVE antecedent — syncopation, 8th/16th motion, pickups,
// varied bar-to-bar, ending unresolved on D (the question). No more quarter-quarter-half every bar.
const VERSE: &[(f64, i32, f32, f32)] = &[
    // Bar 0 (Am): syncopated reach up to the over-reaching B, fall back
    (0.0, 64, 0.5, 0.60),
    (0.5, 67, 0.5, 0.62),
    (1.0, 69, 0.5, 0.68),
    (1.5, 71, 1.0, 0.72),
    (3.0, 69, 0.5, 0.62),
    (3.5, 67, 0.5, 0.58),
    // Bar 1 (F): leap up, a 16th turn, settle
    (4.0, 65, 0.5, 0.60),
    (4.5, 69, 0.5, 0.66),
    (5.0, 72, 0.5, 0.70),
    (5.5, 71, 0.25, 0.64),
    (5.75, 69, 0.25, 0.62),
    (6.0, 65, 1.0, 0.58),
    // Bar 2 (C): a 16th run up to the peak, then hold
    (8.0, 67, 0.5, 0.60),
    (8.5, 72, 0.5, 0.66),
    (9.0, 76, 0.25, 0.72),
    (9.25, 74, 0.25, 0.66),
    (9.5, 72, 0.25, 0.62),
    (9.75, 71, 0.25, 0.60),
    (10.0, 72, 1.0, 0.58),
    // Bar 3 (E7): fall through the raised G# to the unresolved D (b7) — the colored question hangs
    (12.0, 71, 0.5, 0.60),
    (12.5, 69, 0.5, 0.58),
    (13.0, 68, 0.5, 0.56),
    (13.5, 64, 0.5, 0.54),
    (14.0, 62, 1.5, 0.52),
];
// The consequent — the same reach, answered: a confident line that resolves HOME to A. Rhythmically
// alive, with a 16th flourish at the top.
const HOOK: &[(f64, i32, f32, f32)] = &[
    // Bar 0 (Am): reach to the F peak, step down
    (0.0, 69, 0.5, 0.80),
    (0.5, 72, 0.5, 0.84),
    (1.0, 76, 0.75, 0.90),
    (2.0, 77, 0.5, 0.88),
    (2.5, 76, 0.5, 0.84),
    (3.0, 74, 0.5, 0.78),
    (3.5, 72, 0.5, 0.74),
    // Bar 1 (F): leap to F, 16th descent, settle to A
    (4.0, 72, 0.5, 0.80),
    (4.5, 77, 0.5, 0.86),
    (5.0, 76, 0.5, 0.82),
    (5.5, 74, 0.25, 0.76),
    (5.75, 72, 0.25, 0.74),
    (6.0, 69, 1.0, 0.72),
    // Bar 2 (C): a 16th run at the top
    (8.0, 72, 0.5, 0.78),
    (8.5, 76, 0.5, 0.84),
    (9.0, 77, 0.25, 0.88),
    (9.25, 76, 0.25, 0.82),
    (9.5, 74, 0.25, 0.78),
    (9.75, 72, 0.25, 0.74),
    (10.0, 74, 1.0, 0.72),
    // Bar 3 (G): turn and resolve HOME to A — the answer
    (12.0, 74, 0.5, 0.72),
    (12.5, 72, 0.5, 0.70),
    (13.0, 71, 0.5, 0.68),
    (13.5, 72, 0.5, 0.70),
    (14.0, 69, 1.5, 0.66),
];
// Bridge, first half — over A7 | Dm | C | F: the C# of A7 leads down into D minor, then a D-minor theme.
const BRIDGE1: &[(f64, i32, f32, f32)] = &[
    // Bar 0 (A7 pivot): outline A7, C# leads down to D
    (0.0, 69, 0.5, 0.66),  // A
    (0.5, 73, 0.5, 0.68),  // C#
    (1.0, 76, 0.75, 0.72), // E
    (2.0, 74, 0.5, 0.68),  // D
    (2.5, 73, 0.5, 0.66),  // C#
    (3.0, 69, 0.5, 0.64),  // A
    (3.5, 74, 0.5, 0.68),  // D (into Dm)
    // Bar 1 (Dm): the darker tonic
    (4.0, 74, 0.5, 0.70), // D
    (4.5, 77, 0.5, 0.72), // F
    (5.0, 76, 0.5, 0.70), // E
    (5.5, 74, 0.5, 0.68), // D
    (6.0, 72, 1.0, 0.66), // C
    // Bar 2 (C):
    (8.0, 72, 0.5, 0.68),  // C
    (8.5, 76, 0.5, 0.72),  // E
    (9.0, 79, 0.5, 0.74),  // G
    (9.5, 76, 0.5, 0.70),  // E
    (10.0, 74, 1.0, 0.68), // D
    // Bar 3 (F):
    (12.0, 77, 0.5, 0.72), // F
    (12.5, 76, 0.5, 0.70), // E
    (13.0, 74, 0.5, 0.68), // D
    (13.5, 72, 0.5, 0.66), // C
    (14.0, 69, 1.5, 0.64), // A
];
// Bridge, second half — over Dm | A7 | Dm | E7#9: dwell in D minor, then climb the E7 leading tone home.
const BRIDGE2: &[(f64, i32, f32, f32)] = &[
    // Bar 0 (Dm):
    (0.0, 69, 0.5, 0.70),  // A
    (0.5, 72, 0.5, 0.72),  // C
    (1.0, 74, 0.75, 0.74), // D
    (2.0, 77, 0.5, 0.76),  // F
    (2.5, 76, 0.5, 0.72),  // E
    (3.0, 74, 0.5, 0.70),  // D
    (3.5, 72, 0.5, 0.68),  // C
    // Bar 1 (A7): the C# color, climbing
    (4.0, 73, 0.5, 0.72), // C#
    (4.5, 76, 0.5, 0.74), // E
    (5.0, 79, 0.5, 0.78), // G
    (5.5, 76, 0.5, 0.74), // E
    (6.0, 73, 1.0, 0.72), // C#
    // Bar 2 (Dm):
    (8.0, 74, 0.5, 0.74),  // D
    (8.5, 77, 0.5, 0.78),  // F
    (9.0, 79, 0.5, 0.80),  // G
    (9.5, 77, 0.5, 0.76),  // F
    (10.0, 74, 1.0, 0.74), // D
    // Bar 3 (E7#9): climb to the high G# leading tone — pulls home to A
    (12.0, 76, 0.5, 0.82), // E
    (12.5, 80, 0.5, 0.86), // G#
    (13.0, 79, 0.5, 0.84), // G (the #9, passing)
    (13.5, 80, 0.5, 0.88), // G#
    (14.0, 80, 1.5, 0.92), // G# held — leads up to A (home)
];

fn prov(part: Part) -> Provenance {
    Provenance::new(part.section_kind())
}

fn note(
    score: &mut Score,
    bar: u32,
    beat: f64,
    pitch: i32,
    dur: f32,
    vel: f32,
    role: Role,
    part: Part,
) {
    let start = place_note(bar as f64 * BPB + beat);
    score
        .notes
        .push(Note::new(start, dur, pitch, vel, role, prov(part)));
}

fn hit(score: &mut Score, bar: u32, beat: f64, voice: DrumVoice, vel: f32, part: Part) {
    let start = place_drum(bar as f64 * BPB + beat, voice);
    score.drums.push(DrumHit {
        start_beat: start,
        voice,
        velocity: vel,
        prov: prov(part),
    });
}

// =============================== SPARSE (Morty baseline) ================================

fn bass_sparse(score: &mut Score, bar: u32, ch: Ch, next_root: Option<i32>, part: Part) {
    note(score, bar, 0.0, ch.root(), 1.0, 0.82, Role::Bass, part);
    note(score, bar, 2.5, ch.fifth(), 0.75, 0.70, Role::Bass, part);
    if let Some(nr) = next_root {
        note(score, bar, 3.5, nr - 1, 0.5, 0.60, Role::Bass, part);
    }
}

fn keys_sparse(score: &mut Score, bar: u32, ch: Ch, part: Part) {
    if ch == Ch::Am {
        note(score, bar, 0.0, 60, 1.0, 0.46, Role::Keys, part);
        note(score, bar, 0.0, 65, 1.0, 0.56, Role::Keys, part);
        note(score, bar, 1.0, 64, 1.0, 0.44, Role::Keys, part);
        note(score, bar, 2.5, 60, 1.5, 0.44, Role::Keys, part);
        note(score, bar, 2.5, 67, 1.5, 0.50, Role::Keys, part);
    } else {
        note(score, bar, 0.0, ch.guide_low(), 1.5, 0.46, Role::Keys, part);
        note(score, bar, 0.0, ch.guide_hi(), 1.5, 0.52, Role::Keys, part);
        note(score, bar, 2.5, ch.guide_low(), 1.5, 0.42, Role::Keys, part);
        note(score, bar, 2.5, ch.guide_hi(), 1.5, 0.48, Role::Keys, part);
    }
}

fn drums_sparse(score: &mut Score, bar: u32, part: Part, fill: Option<bool>) {
    let chorus = part == Part::Chorus;
    hit(score, bar, 0.0, DrumVoice::Kick, 0.95, part);
    hit(score, bar, 2.0, DrumVoice::Kick, 0.80, part);
    if chorus {
        hit(score, bar, 2.5, DrumVoice::Kick, 0.68, part);
    }
    hit(score, bar, 1.0, DrumVoice::Snare, 0.85, part);
    if chorus {
        hit(score, bar, 1.0, DrumVoice::Clap, 0.58, part);
        hit(score, bar, 3.0, DrumVoice::Clap, 0.58, part);
    }
    let hat_end: f64 = if matches!(fill, Some(true)) { 2.0 } else { 4.0 };
    let mut b: f64 = 0.0;
    while b < hat_end - 1e-9 {
        let v = if b.fract() == 0.0 { 0.48 } else { 0.38 };
        hit(score, bar, b, DrumVoice::ClosedHat, v, part);
        b += 0.5;
    }
    if chorus && !matches!(fill, Some(true)) {
        hit(score, bar, 3.5, DrumVoice::OpenHat, 0.52, part);
    }
    match fill {
        None => hit(score, bar, 3.0, DrumVoice::Snare, 0.85, part),
        Some(false) => {
            hit(score, bar, 3.0, DrumVoice::Snare, 0.68, part);
            hit(score, bar, 3.5, DrumVoice::Snare, 0.78, part);
        }
        Some(true) => {
            for (i, &b) in [2.0, 2.5, 3.0, 3.25, 3.5, 3.75].iter().enumerate() {
                let vel = (0.60 + 0.055 * i as f32).min(0.95);
                hit(score, bar, b, DrumVoice::Snare, vel, part);
            }
        }
    }
}

// =============================== DENSE (Rick-direction) ================================

/// Active melodic bass: chord-tone arpeggiation (root / fifth / octave / third) with a two-note
/// chromatic approach into the next root. The line has contour and a destination — not a random walk.
fn bass_dense(score: &mut Score, bar: u32, ch: Ch, next_root: Option<i32>, part: Part) {
    // Outlines the full 7th chord: root - 3rd - 5th - 7th - 5th - root, then a chromatic approach.
    let (r, f, sev, t) = (ch.root(), ch.fifth(), ch.seventh(), ch.third());
    note(score, bar, 0.0, r, 0.4, 0.84, Role::Bass, part);
    note(score, bar, 1.0, t, 0.4, 0.66, Role::Bass, part);
    note(score, bar, 1.5, f, 0.4, 0.70, Role::Bass, part);
    note(score, bar, 2.0, sev, 0.4, 0.72, Role::Bass, part);
    note(score, bar, 2.5, f, 0.4, 0.64, Role::Bass, part);
    note(score, bar, 3.0, r, 0.4, 0.72, Role::Bass, part);
    if let Some(nr) = next_root {
        note(score, bar, 3.5, nr - 2, 0.25, 0.60, Role::Bass, part); // whole-step approach
        note(score, bar, 3.75, nr - 1, 0.25, 0.68, Role::Bass, part); // half-step leading tone
    } else {
        note(score, bar, 3.5, f, 0.5, 0.60, Role::Bass, part);
    }
}

/// Syncopated counter-comp: a guide-tone dyad states the harmony on the downbeat, then moving guide
/// tones stab the 16th holes the bass leaves (complementary placement — the interlock).
fn keys_dense(score: &mut Score, bar: u32, ch: Ch, part: Part) {
    let (gl, gh, col) = (ch.guide_low(), ch.guide_hi(), ch.color());
    // Extended voicing on the downbeat: 3rd + 7th + an upper-structure 9th/13th (the color).
    note(score, bar, 0.0, gl, 0.9, 0.46, Role::Keys, part);
    note(score, bar, 0.0, gh, 0.9, 0.50, Role::Keys, part);
    note(score, bar, 0.0, col, 0.9, 0.42, Role::Keys, part);
    // offbeat 16th comp in the holes between bass onsets (bass owns 0,1,1.5,2,2.5,3,3.5,3.75)
    note(score, bar, 1.25, gh, 0.2, 0.40, Role::Keys, part);
    note(score, bar, 1.75, col, 0.2, 0.38, Role::Keys, part);
    note(score, bar, 2.75, gh, 0.2, 0.42, Role::Keys, part);
    note(score, bar, 3.25, col, 0.2, 0.38, Role::Keys, part);
}

/// 16th-note drive: busy hats (accented on the beat, soft on the e/a), kick on 1 & 3 plus a syncopated
/// push, backbeat snare on 2 & 4 plus ghost snares. The anchor, but a driving one.
fn drums_dense(score: &mut Score, bar: u32, part: Part, fill: Option<bool>) {
    let chorus = part == Part::Chorus;
    // 16th hats
    let hat_end: f64 = if matches!(fill, Some(true)) { 2.0 } else { 4.0 };
    let mut b: f64 = 0.0;
    while b < hat_end - 1e-9 {
        let v = if b.fract() == 0.0 {
            0.46
        } else if (b * 2.0).fract() == 0.0 {
            0.34
        } else {
            0.26
        };
        hit(score, bar, b, DrumVoice::ClosedHat, v, part);
        b += 0.25;
    }
    // kick: 1 & 3 + a syncopated "and of 3"
    hit(score, bar, 0.0, DrumVoice::Kick, 0.95, part);
    hit(score, bar, 2.0, DrumVoice::Kick, 0.82, part);
    hit(score, bar, 2.75, DrumVoice::Kick, 0.62, part);
    if chorus {
        hit(score, bar, 1.5, DrumVoice::Kick, 0.58, part);
    }
    // backbeat + ghosts
    hit(score, bar, 1.0, DrumVoice::Snare, 0.85, part);
    hit(score, bar, 1.75, DrumVoice::Snare, 0.30, part); // ghost
    hit(score, bar, 3.75, DrumVoice::Snare, 0.32, part); // ghost
    if chorus {
        hit(score, bar, 1.0, DrumVoice::Clap, 0.56, part);
        hit(score, bar, 3.0, DrumVoice::Clap, 0.56, part);
        hit(score, bar, 3.5, DrumVoice::OpenHat, 0.50, part);
    }
    match fill {
        None => hit(score, bar, 3.0, DrumVoice::Snare, 0.85, part),
        Some(false) => {
            hit(score, bar, 3.0, DrumVoice::Snare, 0.70, part);
            hit(score, bar, 3.5, DrumVoice::Snare, 0.80, part);
        }
        Some(true) => {
            for (i, &b) in [2.0, 2.5, 3.0, 3.25, 3.5, 3.75].iter().enumerate() {
                let vel = (0.62 + 0.055 * i as f32).min(0.95);
                hit(score, bar, b, DrumVoice::Snare, vel, part);
            }
        }
    }
}

fn lay_bar(
    score: &mut Score,
    cfg: Cfg,
    bar: u32,
    ch: Ch,
    next_root: Option<i32>,
    part: Part,
    with_pad: bool,
) {
    if cfg.dense {
        bass_dense(score, bar, ch, next_root, part);
        keys_dense(score, bar, ch, part);
    } else {
        bass_sparse(score, bar, ch, next_root, part);
        keys_sparse(score, bar, ch, part);
    }
    if with_pad {
        for &p in &ch.pad_fifth() {
            note(score, bar, 0.0, p, 4.0, 0.40, Role::Pad, part);
        }
    }
}

fn drums(score: &mut Score, cfg: Cfg, bar: u32, part: Part, fill: Option<bool>) {
    if cfg.dense {
        drums_dense(score, bar, part, fill);
    } else {
        drums_sparse(score, bar, part, fill);
    }
}

/// Lay a melodic phrase with the accepted structure-coherent velocity: the per-note arc baked into the
/// phrase, scaled by the section terrace `gain`, with a small lift on the `varied` (second-pass) repeat.
/// The rhythm now lives in the phrase itself (syncopation, 8ths/16ths), so no grace-note crutch.
fn lead_phrase(
    score: &mut Score,
    start_bar: u32,
    phrase: &[(f64, i32, f32, f32)],
    part: Part,
    varied: bool,
    gain: f32,
) {
    for &(b, p, d, v) in phrase {
        let vel = (v * gain * if varied { 1.04 } else { 1.0 }).min(1.0);
        note(score, start_bar, b, p, d, vel, Role::Lead, part);
    }
}

fn build_song(cfg: Cfg) -> Score {
    let bars: u32 = 56;
    let mut score = Score::new(cfg.tempo, BPB, bars as f64 * BPB);

    // Sophisticated, coherent changes (extended voicings via Ch::color): the verse is a ii7-V7-Imaj7
    // then a V7/vi (E7#9) turnaround; the chorus is IV-V-ii-i, landing home on Am; the bridge departs.
    let verse_seq = [Ch::Dm, Ch::G, Ch::C, Ch::E]; //  ii7 - V7 - Imaj7 - V7/vi
    let chorus_seq = [Ch::F, Ch::G, Ch::Dm, Ch::Am]; // IV - V - ii - i (hook lands home)
                                                     // The bridge is the real travel: the Am pivots to A7 (its own dominant) and the music modulates
                                                     // DOWN to D minor, lives there (darker), then an E7 pivots home for the final chorus.
    let bridge_seq = [Ch::A7, Ch::Dm, Ch::C, Ch::F, Ch::Dm, Ch::A7, Ch::Dm, Ch::E];
    let last_bar = bars - 1;

    let chord_at = |bar: u32| -> Ch {
        match bar {
            0..=3 => Ch::Am, // intro vamp
            4..=11 | 20..=27 => verse_seq[(bar % 4) as usize],
            12..=19 | 28..=35 | 44..=51 => chorus_seq[(bar % 4) as usize],
            36..=43 => bridge_seq[(bar - 36) as usize],
            52 => Ch::Am,
            53 => Ch::F,
            54 => Ch::G,
            55 => Ch::Am,
            _ => Ch::Am,
        }
    };
    let part_at = |bar: u32| -> Part {
        match bar {
            0..=3 => Part::Intro,
            4..=11 | 20..=27 => Part::Verse,
            12..=19 | 28..=35 | 44..=51 => Part::Chorus,
            36..=43 => Part::Bridge,
            _ => Part::Outro,
        }
    };
    let fill_at = |bar: u32| -> Option<bool> {
        match bar {
            3 | 19 | 35 | 51 => Some(false),
            11 | 27 | 43 => Some(true),
            _ => None,
        }
    };

    if !cfg.lead_only {
        for bar in 0..bars {
            let part = part_at(bar);
            let ch = chord_at(bar);
            let next_root = if bar == last_bar {
                None
            } else {
                Some(chord_at(bar + 1).root())
            };
            let with_pad = part == Part::Chorus || (part == Part::Outro && ch == Ch::Am);
            lay_bar(&mut score, cfg, bar, ch, next_root, part, with_pad);
            drums(&mut score, cfg, bar, part, fill_at(bar));
        }
    }

    if !cfg.backing_only {
        let (v, c, b) = (Part::Verse, Part::Chorus, Part::Bridge);
        const G_VERSE: f32 = 0.90;
        const G_CHORUS: f32 = 1.00;
        const G_BRIDGE_1: f32 = 0.95;
        const G_BRIDGE_2: f32 = 1.05;
        const G_FINAL: f32 = 1.12;

        lead_phrase(&mut score, 4, VERSE, v, false, G_VERSE);
        lead_phrase(&mut score, 8, VERSE, v, true, G_VERSE);
        lead_phrase(&mut score, 12, HOOK, c, false, G_CHORUS);
        lead_phrase(&mut score, 16, HOOK, c, true, G_CHORUS);
        lead_phrase(&mut score, 20, VERSE, v, false, G_VERSE);
        lead_phrase(&mut score, 24, VERSE, v, true, G_VERSE);
        lead_phrase(&mut score, 28, HOOK, c, false, G_CHORUS);
        lead_phrase(&mut score, 32, HOOK, c, true, G_CHORUS);
        lead_phrase(&mut score, 36, BRIDGE1, b, false, G_BRIDGE_1);
        lead_phrase(&mut score, 40, BRIDGE2, b, true, G_BRIDGE_2);
        lead_phrase(&mut score, 44, HOOK, c, false, G_FINAL);
        lead_phrase(&mut score, 48, HOOK, c, true, G_FINAL);

        note(
            &mut score,
            last_bar,
            0.0,
            69,
            4.0,
            0.64,
            Role::Lead,
            Part::Outro,
        );
    }

    score
}

/// Onset diagnostics (machine evidence — may only REJECT; the ear decides). Reports onset count, the
/// rough events-per-second rate at the configured tempo, cross-role coincidences, and per-role activity.
fn onset_stats(label: &str, score: &Score, tempo: f32) {
    use std::collections::{BTreeSet, HashMap};
    let q = |t: f64| (t * 48.0).round() as i64;

    let mut roles_at: HashMap<i64, BTreeSet<&'static str>> = HashMap::new();
    let mut per_role: HashMap<&'static str, usize> = HashMap::new();
    for n in &score.notes {
        roles_at
            .entry(q(n.start_beat))
            .or_default()
            .insert(n.role.label());
        *per_role.entry(n.role.label()).or_default() += 1;
    }
    let mut drum_times: BTreeSet<i64> = BTreeSet::new();
    for d in &score.drums {
        drum_times.insert(q(d.start_beat));
    }
    let cross = roles_at.values().filter(|s| s.len() >= 2).count();
    let secs = score.total_beats * 60.0 / tempo as f64;
    let onsets = score.notes.len() + score.drums.len();
    println!(
        "  [{label}] pitched={:4} drums={:4} total={:4} | {:.1} events/sec | cross-role coincident instants={cross:3}",
        score.notes.len(),
        score.drums.len(),
        onsets,
        onsets as f64 / secs,
    );
    for r in ["bass", "keys", "lead", "pad"] {
        if let Some(c) = per_role.get(r) {
            println!("        {r:5}: {c:4} onsets");
        }
    }
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
    let out = PathBuf::from(
        arg("--out=").unwrap_or_else(|| "target/humanmusic-beefup/rick_probe".into()),
    );
    std::fs::create_dir_all(&out)?;

    let has = |f: &str| std::env::args().any(|a| a == f);
    let tempo: f32 = arg("--tempo=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(138.0);
    let cfg = Cfg {
        tempo,
        dense: !has("--sparse"),
        backing_only: has("--backing-only"),
        lead_only: has("--lead-only"),
    };

    println!("rick_probe — density calibration — onset diagnostics (full song):");
    onset_stats(
        "dense ",
        &build_song(Cfg {
            tempo,
            dense: true,
            backing_only: false,
            lead_only: false,
        }),
        tempo,
    );
    onset_stats(
        "sparse",
        &build_song(Cfg {
            tempo,
            dense: false,
            backing_only: false,
            lead_only: false,
        }),
        tempo,
    );

    let mut suffix = String::new();
    if !cfg.dense {
        suffix.push_str("_sparse");
    }
    if cfg.backing_only {
        suffix.push_str("_backing");
    }
    if cfg.lead_only {
        suffix.push_str("_lead");
    }
    suffix.push_str(&format!("_{}bpm", tempo as u32));

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
        "render [{}{}, {} bpm]: {} notes, {} drum hits, {:.0} beats ({:.1}s)",
        if cfg.dense { "dense" } else { "sparse" },
        if cfg.backing_only {
            ",backing-only"
        } else {
            ",full"
        },
        tempo as u32,
        score.notes.len(),
        score.drums.len(),
        score.total_beats,
        score.total_beats * 60.0 / tempo as f64,
    );

    for (id, name) in worlds {
        let world = MusicWorld::from_id(id);
        let mut synth = HumanMusicSynth::new(&score, &world, sr);
        let frames = synth.total_samples();
        let res = OfflineRenderer::new(sr, block).render(&mut synth, frames);
        let wav = out.join(format!("rick_{name}{suffix}.wav"));
        write_wav_i16(&wav, &res.audio, sr)?;
        println!(
            "  {name:12} -> {} ({} frames)",
            wav.display(),
            res.audio.frames()
        );
    }
    Ok(())
}
