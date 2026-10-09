//! **Band-story probe** (Rick-C137 semantic-story round) — does the BAND tell the story, or does a
//! lead just sing over backing? Built on the ear-accepted `rick_probe` foundation (same form, harmony,
//! germ, pocket, tempo, dense interlock) with ONE variable changed: **who carries which meaning when.**
//!
//! The song's germ identity (VERSE = the question, HOOK = the answer) is a melodic line. In the STATIC
//! cast the lead carries it every time and the band's responsibilities never move. In the NARRATIVE
//! cast the SAME identity migrates through the ensemble, mapped to the form's listener events:
//!
//! * Verse 1 (Learn)      — the lead teaches the germ; the band stays legible, restrained.
//! * Verse 2 (Reinforce)  — the germ enters the KEYS an octave down (it is no longer the lead's alone).
//! * Chorus 2 (Develop)   — the BASS quotes the hook's contour (same DNA, a new function).
//! * Bridge (Miss)        — the lead WITHHOLDS at the expected arrival; the keys carry the line in the
//!   dark D-minor (negative space is a narrative act, not a failure to play).
//! * Final chorus (Payoff)— the hook returns in an EARNED ensemble octave-tutti (lead + keys + bass):
//!   convergence, not more notes.
//!
//! Two negative controls make it falsifiable:
//! * SCRAMBLED — the narrative assignments are permuted across phrase positions. Each local phrase stays
//!   grammatical; the global story is wrong (the payoff shouted before anything is taught, the withhold
//!   landing on a chorus). If the ear can't tell scrambled from narrative, the band isn't telling a story.
//! * FLAT      — narrative's backing density, but the carrier never leaves the lead and nothing is
//!   withheld. If NARRATIVE is not audibly better than FLAT, the handoff abstraction is not worth building.
//!
//! Stem isolations (the carrier-handoff test, §13/§32): `--no-lead` on NARRATIVE should STILL carry
//! preparation / absence / return (the band holds the germ at the handoffs), where STATIC goes near-blank.
//!
//!   cargo run --release --example band_story_probe -- --out=target/humanmusic-beefup/band-story
//!
//! Flags:
//! * `--cast=static|narrative|scrambled|flat` — render one cast (default: all four).
//! * `--no-lead` / `--lead-only` / `--rhythm` (bass+drums) / `--support` (keys+pad) — stem isolations.
//! * `--tempo=NNN` (default 138, matches rick_probe), `--world=black_ice|vapor95|swiss_signal`
//!   (default: all three for full mixes; stems render black_ice only), `--out=`.
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

// ================================ HARMONY (verbatim from rick_probe) ================================

/// The chords, as roles-in-a-band. Home loop (Am/F/C/G); `Dm`/`E` are the bridge's contrasting
/// harmony — `E` carries the raised G# (the colored dominant that pulls home).
#[derive(Clone, Copy, PartialEq)]
enum Ch {
    Am,
    F,
    C,
    G,
    Dm,
    E,
    A7,
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
            Ch::A7 => 45,
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
            Ch::A7 => 52,
        }
    }
    fn third(self) -> i32 {
        match self {
            Ch::Am => 48,
            Ch::F => 45,
            Ch::C => 52,
            Ch::G => 47,
            Ch::Dm => 41,
            Ch::E => 44,
            Ch::A7 => 49,
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
            Ch::E => 56,
            Ch::A7 => 61,
        }
    }
    fn guide_hi(self) -> i32 {
        match self {
            Ch::Am => 67,
            Ch::F => 64,
            Ch::C => 64,
            Ch::G => 65,
            Ch::Dm => 60,
            Ch::E => 62,
            Ch::A7 => 67,
        }
    }
    fn seventh(self) -> i32 {
        self.root()
            + if matches!(self, Ch::C | Ch::F) {
                11
            } else {
                10
            }
    }
    fn color(self) -> i32 {
        match self {
            Ch::Am => 71,
            Ch::F => 67,
            Ch::C => 62,
            Ch::G => 64,
            Ch::Dm => 64,
            Ch::E => 66,
            Ch::A7 => 71,
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

// ================================ GERM (verbatim from rick_probe) ================================
// (beat-within-4-bar-phrase, pitch, dur, velocity)

const VERSE: &[(f64, i32, f32, f32)] = &[
    (0.0, 64, 0.5, 0.60),
    (0.5, 67, 0.5, 0.62),
    (1.0, 69, 0.5, 0.68),
    (1.5, 71, 1.0, 0.72),
    (3.0, 69, 0.5, 0.62),
    (3.5, 67, 0.5, 0.58),
    (4.0, 65, 0.5, 0.60),
    (4.5, 69, 0.5, 0.66),
    (5.0, 72, 0.5, 0.70),
    (5.5, 71, 0.25, 0.64),
    (5.75, 69, 0.25, 0.62),
    (6.0, 65, 1.0, 0.58),
    (8.0, 67, 0.5, 0.60),
    (8.5, 72, 0.5, 0.66),
    (9.0, 76, 0.25, 0.72),
    (9.25, 74, 0.25, 0.66),
    (9.5, 72, 0.25, 0.62),
    (9.75, 71, 0.25, 0.60),
    (10.0, 72, 1.0, 0.58),
    (12.0, 71, 0.5, 0.60),
    (12.5, 69, 0.5, 0.58),
    (13.0, 68, 0.5, 0.56),
    (13.5, 64, 0.5, 0.54),
    (14.0, 62, 1.5, 0.52),
];
const HOOK: &[(f64, i32, f32, f32)] = &[
    (0.0, 69, 0.5, 0.80),
    (0.5, 72, 0.5, 0.84),
    (1.0, 76, 0.75, 0.90),
    (2.0, 77, 0.5, 0.88),
    (2.5, 76, 0.5, 0.84),
    (3.0, 74, 0.5, 0.78),
    (3.5, 72, 0.5, 0.74),
    (4.0, 72, 0.5, 0.80),
    (4.5, 77, 0.5, 0.86),
    (5.0, 76, 0.5, 0.82),
    (5.5, 74, 0.25, 0.76),
    (5.75, 72, 0.25, 0.74),
    (6.0, 69, 1.0, 0.72),
    (8.0, 72, 0.5, 0.78),
    (8.5, 76, 0.5, 0.84),
    (9.0, 77, 0.25, 0.88),
    (9.25, 76, 0.25, 0.82),
    (9.5, 74, 0.25, 0.78),
    (9.75, 72, 0.25, 0.74),
    (10.0, 74, 1.0, 0.72),
    (12.0, 74, 0.5, 0.72),
    (12.5, 72, 0.5, 0.70),
    (13.0, 71, 0.5, 0.68),
    (13.5, 72, 0.5, 0.70),
    (14.0, 69, 1.5, 0.66),
];
const BRIDGE1: &[(f64, i32, f32, f32)] = &[
    (0.0, 69, 0.5, 0.66),
    (0.5, 73, 0.5, 0.68),
    (1.0, 76, 0.75, 0.72),
    (2.0, 74, 0.5, 0.68),
    (2.5, 73, 0.5, 0.66),
    (3.0, 69, 0.5, 0.64),
    (3.5, 74, 0.5, 0.68),
    (4.0, 74, 0.5, 0.70),
    (4.5, 77, 0.5, 0.72),
    (5.0, 76, 0.5, 0.70),
    (5.5, 74, 0.5, 0.68),
    (6.0, 72, 1.0, 0.66),
    (8.0, 72, 0.5, 0.68),
    (8.5, 76, 0.5, 0.72),
    (9.0, 79, 0.5, 0.74),
    (9.5, 76, 0.5, 0.70),
    (10.0, 74, 1.0, 0.68),
    (12.0, 77, 0.5, 0.72),
    (12.5, 76, 0.5, 0.70),
    (13.0, 74, 0.5, 0.68),
    (13.5, 72, 0.5, 0.66),
    (14.0, 69, 1.5, 0.64),
];
const BRIDGE2: &[(f64, i32, f32, f32)] = &[
    (0.0, 69, 0.5, 0.70),
    (0.5, 72, 0.5, 0.72),
    (1.0, 74, 0.75, 0.74),
    (2.0, 77, 0.5, 0.76),
    (2.5, 76, 0.5, 0.72),
    (3.0, 74, 0.5, 0.70),
    (3.5, 72, 0.5, 0.68),
    (4.0, 73, 0.5, 0.72),
    (4.5, 76, 0.5, 0.74),
    (5.0, 79, 0.5, 0.78),
    (5.5, 76, 0.5, 0.74),
    (6.0, 73, 1.0, 0.72),
    (8.0, 74, 0.5, 0.74),
    (8.5, 77, 0.5, 0.78),
    (9.0, 79, 0.5, 0.80),
    (9.5, 77, 0.5, 0.76),
    (10.0, 74, 1.0, 0.74),
    (12.0, 76, 0.5, 0.82),
    (12.5, 80, 0.5, 0.86),
    (13.0, 79, 0.5, 0.84),
    (13.5, 80, 0.5, 0.88),
    (14.0, 80, 1.5, 0.92),
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

// ================================ BACKING (convergence-graded) ================================
// convergence 0 = restrained/legible, 1 = the rick_probe dense interlock, 2 = tutti.

/// Melodic bass: chord-tone arpeggiation with a chromatic approach into the next root. At convergence 0
/// the line is thinned to root/fifth so the carried germ stays legible; at 1/2 it is the full arpeggio.
fn bass_backing(score: &mut Score, bar: u32, ch: Ch, next_root: Option<i32>, part: Part, conv: u8) {
    let (r, f, sev, t) = (ch.root(), ch.fifth(), ch.seventh(), ch.third());
    if conv == 0 {
        note(score, bar, 0.0, r, 1.0, 0.80, Role::Bass, part);
        note(score, bar, 2.5, f, 0.75, 0.66, Role::Bass, part);
        if let Some(nr) = next_root {
            note(score, bar, 3.5, nr - 1, 0.5, 0.58, Role::Bass, part);
        }
        return;
    }
    note(score, bar, 0.0, r, 0.4, 0.84, Role::Bass, part);
    note(score, bar, 1.0, t, 0.4, 0.66, Role::Bass, part);
    note(score, bar, 1.5, f, 0.4, 0.70, Role::Bass, part);
    note(score, bar, 2.0, sev, 0.4, 0.72, Role::Bass, part);
    note(score, bar, 2.5, f, 0.4, 0.64, Role::Bass, part);
    note(score, bar, 3.0, r, 0.4, 0.72, Role::Bass, part);
    if let Some(nr) = next_root {
        note(score, bar, 3.5, nr - 2, 0.25, 0.60, Role::Bass, part);
        note(score, bar, 3.75, nr - 1, 0.25, 0.68, Role::Bass, part);
    } else {
        note(score, bar, 3.5, f, 0.5, 0.60, Role::Bass, part);
    }
}

/// Syncopated counter-comp: a guide-tone/color voicing on the downbeat, then moving stabs in the holes.
/// convergence 0 holds only the downbeat voicing (legible); 1/2 add the offbeat 16th stabs.
fn keys_backing(score: &mut Score, bar: u32, ch: Ch, part: Part, conv: u8) {
    let (gl, gh, col) = (ch.guide_low(), ch.guide_hi(), ch.color());
    note(score, bar, 0.0, gl, 0.9, 0.46, Role::Keys, part);
    note(score, bar, 0.0, gh, 0.9, 0.50, Role::Keys, part);
    note(score, bar, 0.0, col, 0.9, 0.42, Role::Keys, part);
    if conv == 0 {
        return;
    }
    note(score, bar, 1.25, gh, 0.2, 0.40, Role::Keys, part);
    note(score, bar, 1.75, col, 0.2, 0.38, Role::Keys, part);
    note(score, bar, 2.75, gh, 0.2, 0.42, Role::Keys, part);
    note(score, bar, 3.25, col, 0.2, 0.38, Role::Keys, part);
}

fn drums_backing(score: &mut Score, bar: u32, part: Part, conv: u8, fill: Option<bool>) {
    let big = conv >= 2 || part == Part::Chorus;
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
    hit(score, bar, 0.0, DrumVoice::Kick, 0.95, part);
    hit(score, bar, 2.0, DrumVoice::Kick, 0.82, part);
    hit(score, bar, 2.75, DrumVoice::Kick, 0.62, part);
    if big {
        hit(score, bar, 1.5, DrumVoice::Kick, 0.58, part);
    }
    hit(score, bar, 1.0, DrumVoice::Snare, 0.85, part);
    hit(score, bar, 1.75, DrumVoice::Snare, 0.30, part);
    hit(score, bar, 3.75, DrumVoice::Snare, 0.32, part);
    if big {
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

// ================================ CAST / CARRIAGE ================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cast {
    Static,
    Narrative,
    Scrambled,
    Flat,
}

impl Cast {
    fn label(self) -> &'static str {
        match self {
            Cast::Static => "static",
            Cast::Narrative => "narrative",
            Cast::Scrambled => "scrambled",
            Cast::Flat => "flat",
        }
    }
}

/// How a single lead-seated phrase is cast: which agents voice the germ line (with a transposition in
/// semitones), and how convergent the band is underneath. The melodic CONTENT is fixed; only the
/// carriage and convergence move. `lead_withholds` = the lead is absent at this arrival (negative space).
#[derive(Clone, Copy)]
struct Carriage {
    carriers: &'static [(Role, i32)],
    conv: u8,
}

// Carrier sets, named for what they mean in the story.
const LEAD: &[(Role, i32)] = &[(Role::Lead, 0)];
const LEAD_KEYS: &[(Role, i32)] = &[(Role::Lead, 0), (Role::Keys, -12)];
const LEAD_BASS: &[(Role, i32)] = &[(Role::Lead, 0), (Role::Bass, -24)];
const KEYS_DARK: &[(Role, i32)] = &[(Role::Keys, -12)]; // the lead withholds; keys carry in the dark
const TUTTI: &[(Role, i32)] = &[(Role::Lead, 0), (Role::Keys, -12), (Role::Bass, -24)];

/// The twelve lead-seated phrases of the song, in order, each with its content, start bar, section,
/// whether it is a varied (second-pass) repeat, and its section terrace gain.
struct Phrase {
    start_bar: u32,
    notes: &'static [(f64, i32, f32, f32)],
    part: Part,
    varied: bool,
    gain: f32,
}

const G_VERSE: f32 = 0.90;
const G_CHORUS: f32 = 1.00;
const G_BRIDGE_1: f32 = 0.95;
const G_BRIDGE_2: f32 = 1.05;
const G_FINAL: f32 = 1.12;

fn phrases() -> Vec<Phrase> {
    let v = Part::Verse;
    let c = Part::Chorus;
    let b = Part::Bridge;
    vec![
        Phrase {
            start_bar: 4,
            notes: VERSE,
            part: v,
            varied: false,
            gain: G_VERSE,
        },
        Phrase {
            start_bar: 8,
            notes: VERSE,
            part: v,
            varied: true,
            gain: G_VERSE,
        },
        Phrase {
            start_bar: 12,
            notes: HOOK,
            part: c,
            varied: false,
            gain: G_CHORUS,
        },
        Phrase {
            start_bar: 16,
            notes: HOOK,
            part: c,
            varied: true,
            gain: G_CHORUS,
        },
        Phrase {
            start_bar: 20,
            notes: VERSE,
            part: v,
            varied: false,
            gain: G_VERSE,
        },
        Phrase {
            start_bar: 24,
            notes: VERSE,
            part: v,
            varied: true,
            gain: G_VERSE,
        },
        Phrase {
            start_bar: 28,
            notes: HOOK,
            part: c,
            varied: false,
            gain: G_CHORUS,
        },
        Phrase {
            start_bar: 32,
            notes: HOOK,
            part: c,
            varied: true,
            gain: G_CHORUS,
        },
        Phrase {
            start_bar: 36,
            notes: BRIDGE1,
            part: b,
            varied: false,
            gain: G_BRIDGE_1,
        },
        Phrase {
            start_bar: 40,
            notes: BRIDGE2,
            part: b,
            varied: true,
            gain: G_BRIDGE_2,
        },
        Phrase {
            start_bar: 44,
            notes: HOOK,
            part: c,
            varied: false,
            gain: G_FINAL,
        },
        Phrase {
            start_bar: 48,
            notes: HOOK,
            part: c,
            varied: true,
            gain: G_FINAL,
        },
    ]
}

/// The NARRATIVE carriage for each of the twelve phrases — the germ migrating through the band.
/// Index: 0,1 Learn · 2,3 Answer · 4,5 Reinforce(→keys) · 6,7 Develop(→bass) · 8 Miss(withhold,keys) ·
/// 9 Prepare · 10,11 Payoff(tutti).
const NARRATIVE: [Carriage; 12] = [
    Carriage {
        carriers: LEAD,
        conv: 0,
    },
    Carriage {
        carriers: LEAD,
        conv: 0,
    },
    Carriage {
        carriers: LEAD,
        conv: 1,
    },
    Carriage {
        carriers: LEAD,
        conv: 1,
    },
    Carriage {
        carriers: LEAD_KEYS,
        conv: 1,
    },
    Carriage {
        carriers: LEAD_KEYS,
        conv: 1,
    },
    Carriage {
        carriers: LEAD_BASS,
        conv: 1,
    },
    Carriage {
        carriers: LEAD_BASS,
        conv: 1,
    },
    Carriage {
        carriers: KEYS_DARK,
        conv: 0,
    }, // the Miss: lead withholds, keys carry the dark
    Carriage {
        carriers: LEAD_KEYS,
        conv: 1,
    }, // Prepare: building back
    Carriage {
        carriers: TUTTI,
        conv: 2,
    }, // Payoff: earned ensemble octave-tutti
    Carriage {
        carriers: TUTTI,
        conv: 2,
    },
];

/// The carriage for a phrase, for a given cast.
fn carriage_for(cast: Cast, ix: usize) -> Carriage {
    match cast {
        // Lead carries everything; band fixed at a middling convergence; no withhold.
        Cast::Static => Carriage {
            carriers: LEAD,
            conv: 1,
        },
        Cast::Narrative => NARRATIVE[ix],
        // Narrative's assignments permuted across positions (reversed): the payoff-tutti is shouted at
        // Verse 1 before anything is taught, the Learn-solo lands on the final chorus, the withhold
        // falls on a chorus. Every local phrase stays grammatical; the global story is wrong.
        Cast::Scrambled => NARRATIVE[11 - ix],
        // Narrative's backing density, but the carrier never leaves the lead and nothing is withheld.
        Cast::Flat => Carriage {
            carriers: LEAD,
            conv: NARRATIVE[ix].conv,
        },
    }
}

/// Emit one carrier's voicing of a phrase's germ line (transposed), with the phrase's terrace gain and a
/// small lift on the varied repeat. A non-lead carrier plays a touch softer so the lead, when present,
/// still leads.
fn carry(score: &mut Score, ph: &Phrase, role: Role, transpose: i32) {
    let role_gain = if role == Role::Lead { 1.0 } else { 0.82 };
    for &(beat, pitch, dur, vel) in ph.notes {
        let v = (vel * ph.gain * role_gain * if ph.varied { 1.04 } else { 1.0 }).min(1.0);
        note(
            score,
            ph.start_bar,
            beat,
            pitch + transpose,
            dur,
            v,
            role,
            ph.part,
        );
    }
}

fn chord_at(bar: u32) -> Ch {
    let verse_seq = [Ch::Dm, Ch::G, Ch::C, Ch::E];
    let chorus_seq = [Ch::F, Ch::G, Ch::Dm, Ch::Am];
    let bridge_seq = [Ch::A7, Ch::Dm, Ch::C, Ch::F, Ch::Dm, Ch::A7, Ch::Dm, Ch::E];
    match bar {
        0..=3 => Ch::Am,
        4..=11 | 20..=27 => verse_seq[(bar % 4) as usize],
        12..=19 | 28..=35 | 44..=51 => chorus_seq[(bar % 4) as usize],
        36..=43 => bridge_seq[(bar - 36) as usize],
        52 => Ch::Am,
        53 => Ch::F,
        54 => Ch::G,
        _ => Ch::Am,
    }
}

fn part_at(bar: u32) -> Part {
    match bar {
        0..=3 => Part::Intro,
        4..=11 | 20..=27 => Part::Verse,
        12..=19 | 28..=35 | 44..=51 => Part::Chorus,
        36..=43 => Part::Bridge,
        _ => Part::Outro,
    }
}

fn fill_at(bar: u32) -> Option<bool> {
    match bar {
        3 | 19 | 35 | 51 => Some(false),
        11 | 27 | 43 => Some(true),
        _ => None,
    }
}

/// The convergence and carrier-suppression in force during a given bar, derived from whichever lead-seated
/// phrase covers it (phrases are 4 bars, starting at their `start_bar`). Returns (conv, suppress_bass,
/// suppress_keys): a harmonic voice that is carrying the germ this phrase does NOT also play its backing.
fn bar_plan(cast: Cast, bar: u32) -> (u8, bool, bool) {
    for (ix, ph) in phrases().iter().enumerate() {
        if bar >= ph.start_bar && bar < ph.start_bar + 4 {
            let cg = carriage_for(cast, ix);
            let bass = cg.carriers.iter().any(|&(r, _)| r == Role::Bass);
            let keys = cg.carriers.iter().any(|&(r, _)| r == Role::Keys);
            return (cg.conv, bass, keys);
        }
    }
    // Intro / outro: no phrase, middling convergence, nothing suppressed.
    (1, false, false)
}

#[derive(Clone, Copy)]
struct Stem {
    no_lead: bool,
    lead_only: bool,
    rhythm: bool,  // bass + drums only
    support: bool, // keys + pad only
}

impl Stem {
    fn full() -> Stem {
        Stem {
            no_lead: false,
            lead_only: false,
            rhythm: false,
            support: false,
        }
    }
    fn wants(&self, role: Role) -> bool {
        if self.lead_only {
            return role == Role::Lead;
        }
        if self.rhythm {
            return matches!(role, Role::Bass);
        }
        if self.support {
            return matches!(role, Role::Keys | Role::Pad);
        }
        if self.no_lead && role == Role::Lead {
            return false;
        }
        true
    }
    fn wants_drums(&self) -> bool {
        !self.lead_only && !self.support
    }
    fn wants_backing(&self) -> bool {
        !self.lead_only
    }
}

fn build_song(cast: Cast, tempo: f32, stem: Stem) -> Score {
    let bars: u32 = 56;
    let mut score = Score::new(tempo, BPB, bars as f64 * BPB);
    let last_bar = bars - 1;

    // Backing: laid by the agents NOT carrying the germ this phrase; a pad covers whenever a harmonic
    // voice is pulled up to carry (or in choruses / at full convergence).
    if stem.wants_backing() {
        for bar in 0..bars {
            let part = part_at(bar);
            let ch = chord_at(bar);
            let (conv, suppress_bass, suppress_keys) = bar_plan(cast, bar);
            let next_root = if bar == last_bar {
                None
            } else {
                Some(chord_at(bar + 1).root())
            };
            if stem.wants(Role::Bass) && !suppress_bass {
                bass_backing(&mut score, bar, ch, next_root, part, conv);
            }
            if stem.wants(Role::Keys) && !suppress_keys {
                keys_backing(&mut score, bar, ch, part, conv);
            }
            let pad = part == Part::Chorus
                || conv >= 2
                || suppress_bass
                || suppress_keys
                || (part == Part::Outro && ch == Ch::Am);
            if pad && stem.wants(Role::Pad) {
                for &p in &ch.pad_fifth() {
                    note(&mut score, bar, 0.0, p, 4.0, 0.40, Role::Pad, part);
                }
            }
            if stem.wants_drums() {
                drums_backing(&mut score, bar, part, conv, fill_at(bar));
            }
        }
    }

    // The germ line, carried per the cast's carriage for each phrase.
    for (ix, ph) in phrases().iter().enumerate() {
        let cg = carriage_for(cast, ix);
        for &(role, transpose) in cg.carriers {
            if stem.wants(role) {
                carry(&mut score, ph, role, transpose);
            }
        }
    }

    // Outro tail: a lone held tonic in the lead (unless excluded).
    if stem.wants(Role::Lead) {
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

// ================================ DIAGNOSTICS (machine evidence — may only REJECT) ================================

fn onset_stats(cast: Cast, score: &Score, tempo: f32) {
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
    let cross = roles_at.values().filter(|s| s.len() >= 2).count();
    let secs = score.total_beats * 60.0 / tempo as f64;
    let onsets = score.notes.len() + score.drums.len();
    let mut per = String::new();
    for r in ["bass", "keys", "lead", "pad"] {
        per.push_str(&format!(" {r}={}", per_role.get(r).copied().unwrap_or(0)));
    }
    println!(
        "  [{:<9}] pitched={:4} drums={:4} total={:4} | {:.1} ev/s | cross-role instants={cross:3} |{per}",
        cast.label(),
        score.notes.len(),
        score.drums.len(),
        onsets,
        onsets as f64 / secs,
    );
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

fn render(
    out: &std::path::Path,
    cast: Cast,
    tempo: f32,
    stem: Stem,
    suffix: &str,
    worlds: &[(WorldId, &'static str)],
) -> std::io::Result<()> {
    let sr = SampleRate::STUDIO;
    let score = build_song(cast, tempo, stem);
    for &(id, name) in worlds {
        let world = MusicWorld::from_id(id);
        let mut synth = HumanMusicSynth::new(&score, &world, sr);
        let frames = synth.total_samples();
        let res = OfflineRenderer::new(sr, 1024).render(&mut synth, frames);
        let wav = out.join(format!("{}_{}{}.wav", name, cast.label(), suffix));
        write_wav_i16(&wav, &res.audio, sr)?;
        println!(
            "  {name:12} {:<9}{suffix:10} -> {} ({} notes, {} drums, peak {:.3})",
            cast.label(),
            wav.display(),
            score.notes.len(),
            score.drums.len(),
            res.peak,
        );
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let out = PathBuf::from(
        arg("--out=").unwrap_or_else(|| "target/humanmusic-beefup/band-story".into()),
    );
    std::fs::create_dir_all(&out)?;

    let has = |f: &str| std::env::args().any(|a| a == f);
    let tempo: f32 = arg("--tempo=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(138.0);

    let stem = Stem {
        no_lead: has("--no-lead"),
        lead_only: has("--lead-only"),
        rhythm: has("--rhythm"),
        support: has("--support"),
    };
    let mut suffix = String::new();
    if stem.no_lead {
        suffix.push_str("_nolead");
    }
    if stem.lead_only {
        suffix.push_str("_leadonly");
    }
    if stem.rhythm {
        suffix.push_str("_rhythm");
    }
    if stem.support {
        suffix.push_str("_support");
    }

    // Diagnostics on the FULL mix of every cast (machine evidence — the ear decides).
    println!("band_story_probe — cast onset diagnostics ({tempo:.0} bpm, full mix):");
    for cast in [Cast::Static, Cast::Narrative, Cast::Scrambled, Cast::Flat] {
        onset_stats(cast, &build_song(cast, tempo, Stem::full()), tempo);
    }

    let casts: Vec<Cast> = match arg("--cast=").as_deref() {
        Some("static") => vec![Cast::Static],
        Some("narrative") => vec![Cast::Narrative],
        Some("scrambled") => vec![Cast::Scrambled],
        Some("flat") => vec![Cast::Flat],
        _ => vec![Cast::Static, Cast::Narrative, Cast::Scrambled, Cast::Flat],
    };

    // Full mixes render in all three worlds by default (black_ice is the judged voice); a named world or
    // any stem isolation renders black_ice only (the stems are a structural, not a timbral, test).
    let all = [
        (WorldId::BlackIce, "black_ice"),
        (WorldId::Vapor95, "vapor95"),
        (WorldId::SwissSignal, "swiss_signal"),
    ];
    let a_stem = stem.no_lead || stem.lead_only || stem.rhythm || stem.support;
    let worlds: Vec<(WorldId, &'static str)> = match arg("--world=") {
        Some(name) => vec![world_from(&name).unwrap_or((WorldId::BlackIce, "black_ice"))],
        None if a_stem => vec![(WorldId::BlackIce, "black_ice")],
        None => all.to_vec(),
    };

    println!(
        "\nrender [{} bpm{}]:",
        tempo as u32,
        if suffix.is_empty() {
            "".into()
        } else {
            format!(", stem{suffix}")
        }
    );
    for cast in casts {
        render(&out, cast, tempo, stem, &suffix, &worlds)?;
    }
    Ok(())
}
