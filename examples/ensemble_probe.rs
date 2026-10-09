//! **Ensemble-interdependence listening probe** (the controlled twin of `song_form_probe`).
//!
//! The song-form probe crossed one bar ("much better") but left a deeper one standing: it still
//! felt hollow, because *every voice was a complete, self-sufficient, redundant statement of the
//! same harmony* — the pad stated the whole triad, the keys arpeggiated the same triad, the bass
//! rooted it, the melody sat inside it. They all AGREED, vertically, simultaneously, but nothing was
//! incomplete, so nothing needed anything else, so the whole never exceeded the parts. That is the
//! maintainer's verdict in her words: "a group of musicians who happen to be semantically agreeing
//! simultaneously," not an ensemble whose meaning is "functionally completed by the ensemble, more
//! than the sum of its individual musicians." (It is also the *same* flaw as the generator, dressed
//! better: the generator's voices all derive from one shared semantic trace; the song-form probe's
//! all derive from one stated chord. Both are common-mode agreement.)
//!
//! So this is a clean A/B against `song_form_probe`. Held FIXED: tempo (104), the 40-bar
//! Intro·V·C·V·C·Outro form, the Am–F–C–G roots, the **byte-identical VERSE and HOOK melodies**, and
//! the **identical groove/fills**. Changed — the ONE variable under test — redundant vertical
//! agreement becomes horizontal interdependence:
//!   * No voice ever states a full triad. The bass plays roots + a late chromatic approach pickup;
//!     an inner two-note voice (the 3rd + 7th) carries the harmony BY MOVING; the chorus pad enters
//!     as a bare open fifth (no 3rd). The major/minor QUALITY of each chord lives only in the inner
//!     voice — mute it and the pad+bass go modal and ambiguous. The chord exists only in the union.
//!   * A suspension that crosses voices: the inner voice hangs G7's F over the Am downbeat (a b6) and
//!     resolves it to E *while the bass has already moved to A* — tension made by one part, resolved
//!     by another, a sigh no single voice contains.
//!   * Space + interlock: the bass sings two beats, leaves a hole, answers late; the inner voice
//!     moves early. They converse instead of stacking.
//!   * Arrival by ensemble: the pad is withheld the whole verse, so the chorus arrives because the
//!     texture fills in TOGETHER, not because one voice got louder.
//!
//!   cargo run --release --example ensemble_probe -- --out=target/humanmusic-beefup/ensemble
//!
//! Flags: `--world=black_ice|vapor95|swiss_signal` (default: all three), `--out=`.
//! Not shipped; a hand-authored listening control on the beef-up branch, NOT the generator.

use std::path::PathBuf;

use gibson::audio::human_music::form::SectionKind;
use gibson::audio::human_music::score::{DrumHit, DrumVoice, Note, Provenance, Role, Score};
use gibson::audio::human_music::{HumanMusicSynth, MusicWorld, WorldId};
use gibson::audio::wav::write_wav_i16;
use gibson::audio::{OfflineRenderer, SampleRate};

const TEMPO: f32 = 104.0;
const BPB: f64 = 4.0;

/// The chords of the loop, as roles-in-a-band rather than block triads. Each chord knows only its
/// bass root, its inner guide tones (the 3rd + 7th — the harmony's meaning), the open fifth the pad
/// contributes at an arrival, and the note that leads into the NEXT root.
#[derive(Clone, Copy, PartialEq)]
enum Ch {
    Am,
    F,
    C,
    G,
}

impl Ch {
    /// Bass fundamental (MIDI; 60 = middle C). A2 / F2 / C3 / G2.
    fn root(self) -> i32 {
        match self {
            Ch::Am => 45,
            Ch::F => 41,
            Ch::C => 48,
            Ch::G => 43,
        }
    }
    /// The pad's contribution at an arrival: a bare open fifth (root + 5th), deliberately with NO
    /// third. Alone it is modal, ambiguous; the inner voice supplies the quality.
    fn pad_fifth(self) -> [i32; 2] {
        match self {
            Ch::Am => [57, 64], // A3 + E4
            Ch::F => [53, 60],  // F3 + C4
            Ch::C => [60, 67],  // C4 + G4
            Ch::G => [55, 62],  // G3 + D4
        }
    }
    /// The lower inner voice — a smooth, mostly-stepwise support line (held per bar).
    fn guide_low(self) -> i32 {
        match self {
            Ch::Am => 60, // C  (minor 3rd)
            Ch::F => 57,  // A  (major 3rd)
            Ch::C => 59,  // B  (major 7th)
            Ch::G => 59,  // B  (3rd) — common tone across the wrap
        }
    }
    /// The upper inner voice's SETTLED pitch for the bar (non-Am bars hold this; the Am bar instead
    /// plays the suspension figure, see `lay_bar`). F / C / G hold their 7th-or-color; the G7's F is
    /// what hangs over into the next Am as a suspension.
    fn guide_hi(self) -> i32 {
        match self {
            Ch::Am => 67, // G  (minor 7th) — reached after the suspension resolves
            Ch::F => 64,  // E  (major 7th colour)
            Ch::C => 64,  // E  (common tone, now the 3rd)
            Ch::G => 65,  // F  (dom 7th) — hangs over -> next suspension
        }
    }
    /// The pickup note on the "and of 3" that leads into the next root in the loop (Am->F->C->G->Am).
    /// Diatonic step or chromatic leading tone; the bass's late answer to the inner voice's early move.
    fn approach_to_next(self) -> i32 {
        match self {
            Ch::Am => 43, // G  -> F (step down)
            Ch::F => 47,  // B  -> C (leading tone)
            Ch::C => 42,  // F# -> G (leading tone)
            Ch::G => 44,  // G# -> A (leading tone home)
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

/// The VERSE line — IDENTICAL to `song_form_probe` (the controlled constant). Lower, conversational.
const VERSE: &[(f64, i32, f32)] = &[
    (0.0, 60, 1.0), (1.0, 64, 1.0), (2.0, 69, 1.0), (3.0, 64, 1.0), // Am: C E A E
    (4.0, 60, 1.0), (5.0, 65, 1.0), (6.0, 69, 2.0), //                F:  C F A—
    (8.0, 67, 1.0), (9.0, 64, 1.0), (10.0, 60, 2.0), //              C:  G E C—
    (12.0, 62, 1.0), (13.0, 67, 1.0), (14.0, 71, 1.0), (15.0, 62, 1.0), // G: D G B D
];

/// The CHORUS HOOK — IDENTICAL to `song_form_probe` (the controlled constant). The thing that must
/// return recognizably.
const HOOK: &[(f64, i32, f32)] = &[
    (0.0, 69, 1.0), (1.0, 72, 1.0), (2.0, 76, 2.0), //  Am: A C E——  (the reach)
    (4.0, 77, 1.0), (5.0, 76, 1.0), (6.0, 72, 2.0), //  F:  F E C——  (peak then settle)
    (8.0, 76, 1.0), (9.0, 74, 1.0), (10.0, 72, 2.0), // C:  E D C——
    (12.0, 74, 1.0), (13.0, 71, 1.0), (14.0, 74, 2.0), // G: D B D—— (open, loops home)
];

fn prov(part: Part) -> Provenance {
    Provenance::new(part.section_kind())
}

fn note(score: &mut Score, bar: u32, beat: f64, pitch: i32, dur: f32, vel: f32, role: Role, part: Part) {
    let start = bar as f64 * BPB + beat;
    score
        .notes
        .push(Note::new(start, dur, pitch, vel, role, prov(part)));
}

fn hit(score: &mut Score, bar: u32, beat: f64, voice: DrumVoice, vel: f32, part: Part) {
    let start = bar as f64 * BPB + beat;
    score.drums.push(DrumHit {
        start_beat: start,
        voice,
        velocity: vel,
        prov: prov(part),
    });
}

/// The groove — IDENTICAL to `song_form_probe` (the controlled constant): kick 1&3, snare backbeat,
/// eighth hats, chorus thickening, and the same fills. Only the pitched voices differ between probes.
fn drums(score: &mut Score, bar: u32, part: Part, fill: Option<bool>) {
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
        None => {
            hit(score, bar, 3.0, DrumVoice::Snare, 0.85, part);
        }
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

/// Lay ONE bar of the interdependent backing. No block triads anywhere:
///   * bass  — root sung for two beats, then a hole, then a late approach pickup into the next root;
///   * inner — a two-note guide voice (3rd+7th) that MOVES; Am carries the b6->5 suspension figure;
///   * pad   — an open fifth (no 3rd), only at an arrival (`with_pad`), withheld otherwise.
fn lay_bar(score: &mut Score, bar: u32, ch: Ch, part: Part, with_pad: bool, last: bool) {
    // Bass: root (sings), then a hole, then the late chromatic/diatonic pickup — unless this is the
    // final home bar, where the root is left to ring.
    note(score, bar, 0.0, ch.root(), 2.0, 0.82, Role::Bass, part);
    if !last {
        note(score, bar, 3.5, ch.approach_to_next(), 0.5, 0.60, Role::Bass, part);
    }

    // Inner guide voice — the harmony's meaning, carried by motion.
    note(score, bar, 0.0, ch.guide_low(), 4.0, 0.46, Role::Keys, part); // lower support line
    if ch == Ch::Am {
        // The suspension: F hung over from the prior G7 (a b6 over A) resolves down to E (the 5th)
        // while the bass has already moved to A, then rises to G (the min7). A sigh across voices.
        note(score, bar, 0.0, 65, 1.0, 0.56, Role::Keys, part); // F  (suspension)
        note(score, bar, 1.0, 64, 1.0, 0.44, Role::Keys, part); // E  (resolution)
        note(score, bar, 2.0, 67, 2.0, 0.50, Role::Keys, part); // G  (settle)
    } else {
        note(score, bar, 0.0, ch.guide_hi(), 4.0, 0.52, Role::Keys, part);
    }

    // Pad: an open fifth, no third — ambiguous alone, completed by the inner voice. Arrivals only.
    if with_pad {
        for &p in &ch.pad_fifth() {
            note(score, bar, 0.0, p, 4.0, 0.40, Role::Pad, part);
        }
    }
}

/// Lay a melodic phrase — the SAME array every call (literal return), identical to `song_form_probe`.
fn lead_phrase(score: &mut Score, start_bar: u32, phrase: &[(f64, i32, f32)], vel: f32, part: Part) {
    for &(b, p, d) in phrase {
        let start = start_bar as f64 * BPB + b;
        score
            .notes
            .push(Note::new(start, d, p, vel, Role::Lead, prov(part)));
    }
}

/// Build the whole song. Same 40-bar form and Am–F–C–G roots as `song_form_probe`; the outro
/// cadences home (…G→Am). Backing is interdependent, melodies are the unchanged controlled constants.
fn build_song() -> Score {
    let total_beats = 40.0 * BPB;
    let mut score = Score::new(TEMPO, BPB, total_beats);

    let seq = [Ch::Am, Ch::F, Ch::C, Ch::G];
    let chord_at = |bar: u32| -> Ch {
        match bar {
            // Outro resolves home: Am · F · G · Am.
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
    // Fills — IDENTICAL to `song_form_probe`: a tag out of the intro, a big build into each chorus,
    // a tag at each chorus/verse boundary.
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
        // Pad only arrives at a chorus, plus the two outro home-chord bars so the ending lands full.
        let with_pad = part == Part::Chorus || (part == Part::Outro && ch == Ch::Am);
        let last = bar == 39;
        lay_bar(&mut score, bar, ch, part, with_pad, last);
        drums(&mut score, bar, part, fill_at(bar));
    }

    // The melodies — stated LITERALLY, the same arrays each time, identical to `song_form_probe`.
    lead_phrase(&mut score, 4, VERSE, 0.70, Part::Verse); // Verse 1
    lead_phrase(&mut score, 8, VERSE, 0.70, Part::Verse);
    lead_phrase(&mut score, 12, HOOK, 0.82, Part::Chorus); // Chorus 1
    lead_phrase(&mut score, 16, HOOK, 0.82, Part::Chorus);
    lead_phrase(&mut score, 20, VERSE, 0.70, Part::Verse); // Verse 2 — identical
    lead_phrase(&mut score, 24, VERSE, 0.70, Part::Verse);
    lead_phrase(&mut score, 28, HOOK, 0.82, Part::Chorus); // Chorus 2 — identical
    lead_phrase(&mut score, 32, HOOK, 0.82, Part::Chorus);
    note(&mut score, 39, 0.0, 69, 4.0, 0.68, Role::Lead, Part::Outro); // A4 home

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
    let out = PathBuf::from(arg("--out=").unwrap_or_else(|| "target/humanmusic-beefup/ensemble".into()));
    std::fs::create_dir_all(&out)?;

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
    let score = build_song();

    println!(
        "interdependent ensemble: {} notes, {} drum hits, {:.0} beats @ {} bpm ({:.1}s)",
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
        let wav = out.join(format!("ensemble_{name}.wav"));
        write_wav_i16(&wav, &res.audio, sr)?;
        println!("  {name:12} -> {} ({} frames)", wav.display(), res.audio.frames());
    }
    Ok(())
}
