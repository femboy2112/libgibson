//! **Song-form listening probe** (the "prove it by ear first" experiment).
//!
//! This bypasses the HumanMusic *generator* entirely and hand-authors a minimal **song** directly
//! into the [`Score`] IR, then renders it through the engine's own synth/world — the *same voice*
//! the maintainer judged as "doesn't feel like a coherent song."
//!
//! The point is a controlled discriminator. The generator composes adaptive dramatic underscore:
//! form is a one-climax segmentation of the semantic story, the hook is re-pitched (never replayed),
//! harmony is through-composed, and section-recurrence labels (`A`/`A'`) are consumed by no realizer
//! (see `docs`/memory root-cause). A song's coherence instead comes from **literal, heard
//! repetition**. So this probe holds the engine's *sound* fixed and supplies what the generator does
//! not: a fixed 4-bar chord loop that cycles the whole way through, a verse melody that returns
//! **identically**, a chorus hook that returns **identically**, a steady backbeat groove, and fills
//! that announce each chorus. If *this* reads as "a song," the missing ingredient is song-form, not
//! the voice — and we know the pivot will land. If it still doesn't, the problem is deeper (feel /
//! timbre / voicing) and we redirect before rebuilding the generator.
//!
//!   cargo run --release --example song_form_probe -- --out=target/humanmusic-beefup/songform
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

/// One chord of the loop: a bass root and a 3-note pad/keys voicing (A minor family).
#[derive(Clone, Copy)]
struct Chord {
    bass: i32,
    triad: [i32; 3],
}
// Am – F – C – G, voiced for smooth motion (MIDI; 60 = middle C).
const AM: Chord = Chord { bass: 45, triad: [57, 60, 64] }; // A2 ; A3 C4 E4
const F: Chord = Chord { bass: 41, triad: [53, 57, 60] }; // F2 ; F3 A3 C4
const C: Chord = Chord { bass: 48, triad: [55, 60, 64] }; // C3 ; G3 C4 E4
const G: Chord = Chord { bass: 43, triad: [55, 59, 62] }; // G2 ; G3 B3 D4

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

/// The VERSE line — lower, conversational (octave 4). One 4-bar (16-beat) phrase; `(beat, pitch,
/// dur)` within the phrase. Stated identically in both verses.
const VERSE: &[(f64, i32, f32)] = &[
    (0.0, 60, 1.0), (1.0, 64, 1.0), (2.0, 69, 1.0), (3.0, 64, 1.0), // Am: C E A E
    (4.0, 60, 1.0), (5.0, 65, 1.0), (6.0, 69, 2.0), //                F:  C F A—
    (8.0, 67, 1.0), (9.0, 64, 1.0), (10.0, 60, 2.0), //              C:  G E C—
    (12.0, 62, 1.0), (13.0, 67, 1.0), (14.0, 71, 1.0), (15.0, 62, 1.0), // G: D G B D
];

/// The CHORUS HOOK — higher, a reach then a settle (octave 4–5). The thing that must return
/// recognizably. Stated identically in both choruses.
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

/// One bar of backing: a held pad chord, a root-pulse bass, a quarter-note keys arpeggio, and the
/// groove. Identical shape every bar — the body that loops, like a song's.
fn backing(score: &mut Score, bar: u32, ch: Chord, part: Part, fill: Option<bool>) {
    let chorus = part == Part::Chorus;
    // Pad: the chord, held. The chorus adds the octave for lift.
    let padv = if chorus { 0.48 } else { 0.40 };
    for &p in &ch.triad {
        note(score, bar, 0.0, p, 4.0, padv, Role::Pad, part);
    }
    if chorus {
        note(score, bar, 0.0, ch.triad[0] + 12, 4.0, 0.38, Role::Pad, part);
    }
    // Bass: root on 1 and 3.
    note(score, bar, 0.0, ch.bass, 1.6, 0.74, Role::Bass, part);
    note(score, bar, 2.0, ch.bass, 1.6, 0.70, Role::Bass, part);
    // Keys: a gentle quarter-note arpeggio (a b c b) for motion.
    let arp = [ch.triad[0], ch.triad[1], ch.triad[2], ch.triad[1]];
    let kv = if chorus { 0.36 } else { 0.30 };
    for (i, &p) in arp.iter().enumerate() {
        note(score, bar, i as f64, p, 0.9, kv, Role::Keys, part);
    }
    drums(score, bar, part, fill);
}

/// The groove: kick on 1 & 3, snare backbeat on 2 & 4, eighth-note hats. The chorus thickens it
/// (clap-doubled backbeat, a push kick, an open hat). `fill` turns the bar into a pickup into the
/// next section (a small tag, or a big snare build into a chorus).
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
    // Hats across the bar (eighths), unless a big fill takes the second half.
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
            // Small tag into the next section.
            hit(score, bar, 3.0, DrumVoice::Snare, 0.68, part);
            hit(score, bar, 3.5, DrumVoice::Snare, 0.78, part);
        }
        Some(true) => {
            // Big snare build announcing the chorus downbeat.
            for (i, &b) in [2.0, 2.5, 3.0, 3.25, 3.5, 3.75].iter().enumerate() {
                let vel = (0.60 + 0.055 * i as f32).min(0.95);
                hit(score, bar, b, DrumVoice::Snare, vel, part);
            }
        }
    }
}

/// Lay a melodic phrase down starting at `start_bar`. The SAME array every call — literal return.
fn lead_phrase(score: &mut Score, start_bar: u32, phrase: &[(f64, i32, f32)], vel: f32, part: Part) {
    for &(b, p, d) in phrase {
        let start = start_bar as f64 * BPB + b;
        score
            .notes
            .push(Note::new(start, d, p, vel, Role::Lead, prov(part)));
    }
}

/// Build the whole song into one Score. 40 bars: Intro(4) · Verse1(8) · Chorus1(8) · Verse2(8) ·
/// Chorus2(8) · Outro(4), on a 4-bar Am–F–C–G loop throughout; the outro cadences home (…G→Am).
fn build_song() -> Score {
    let total_beats = 40.0 * BPB;
    let mut score = Score::new(TEMPO, BPB, total_beats);

    let loop4 = [AM, F, C, G];
    let chord_at = |bar: u32| -> Chord {
        match bar {
            // Outro resolves home: Am · F · G · Am (a i–VI–VII–i button).
            36 => AM,
            37 => F,
            38 => G,
            39 => AM,
            _ => loop4[(bar % 4) as usize],
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
    // Fills: a tag out of the intro, a big build into each chorus, a tag at each chorus end.
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
        backing(&mut score, bar, chord_at(bar), part_at(bar), fill_at(bar));
    }

    // The melodies — stated LITERALLY, the same arrays each time.
    lead_phrase(&mut score, 4, VERSE, 0.70, Part::Verse); // Verse 1, bars 4–7
    lead_phrase(&mut score, 8, VERSE, 0.70, Part::Verse); //          bars 8–11
    lead_phrase(&mut score, 12, HOOK, 0.82, Part::Chorus); // Chorus 1, bars 12–15
    lead_phrase(&mut score, 16, HOOK, 0.82, Part::Chorus); //           bars 16–19
    lead_phrase(&mut score, 20, VERSE, 0.70, Part::Verse); // Verse 2 — identical to Verse 1
    lead_phrase(&mut score, 24, VERSE, 0.70, Part::Verse);
    lead_phrase(&mut score, 28, HOOK, 0.82, Part::Chorus); // Chorus 2 — identical to Chorus 1
    lead_phrase(&mut score, 32, HOOK, 0.82, Part::Chorus);
    // Outro button: land on the tonic and let it ring.
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
    let out = PathBuf::from(arg("--out=").unwrap_or_else(|| "target/humanmusic-beefup/songform".into()));
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
        "hand-authored song: {} notes, {} drum hits, {:.0} beats @ {} bpm ({:.1}s)",
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
        let wav = out.join(format!("songform_{name}.wav"));
        write_wav_i16(&wav, &res.audio, sr)?;
        println!("  {name:12} -> {} ({} frames)", wav.display(), res.audio.frames());
    }
    Ok(())
}
