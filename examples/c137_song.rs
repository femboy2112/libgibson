//! Render the C137 long-form song. THE JERRY LAW: the ordinary, no-flag invocation renders C137
//! **Full** — the complete 16-bar verse→hook→bridge→return arc, the expressive lead, and the ensemble
//! breath response. Excellence is the default; a downgrade is the explicit experiment.
//!
//! `cargo run --release --example c137_song -- --out=/tmp/c137-song --seeds=2112,7,99 --tempo=144`
//!
//! A **Jerry switch** removes one capability (see `c137_song/preset.rs`); each is OFF by default and
//! each dials exactly its own capability, independently of the others:
//!   `--jerry=short`         renders the 8-bar verse→hook control (no bridge/return/rug).
//!   `--jerry=plain`         the COMPOUND archival control: robotic lead AND the whole band stripped
//!                           back (no keys comp, no bass anticipation, no drum translation, no answer).
//!   `--jerry=deaf`          keeps the expressive lead + keys/bass/drums, only the band's breath answer off.
//!   `--jerry=flat-lead`     the LEAD-ONLY downgrade: plain lead, but keys/bass/drums stay full.
//!   `--jerry=no-keys`       removes only the keyboard comp.
//!   `--jerry=straight-bass` removes only the bass's V→I destination anticipation.
//!   `--jerry=generic-drums` removes only the drum lead-rhythm translation (the lead-echo fill).
//! Switches combine (`--jerry=short,plain` is the frozen 8-bar plain control = the old default). The
//! narrow switches let any one capability be auditioned without downgrading the unrelated ones.
//!
//! The pre-preset flags still work for compatibility and fold into the same mechanism, so they are
//! reported honestly as the downgrades they are: `--bars=8` ⇒ `short`; `--phrasing=plain` ⇒ `plain`;
//! `--phrasing=expressive` ⇒ `deaf`. `--bridge-seed=N` swaps in a different LAWFUL bridge over the same
//! chart and returning hook (the earned-return counterfactual; only meaningful on the 16-bar arc).
//!
//! Each run prints and writes an EFFECTIVE-FEATURE MANIFEST (`manifest.txt`): the resolved preset, the
//! active capabilities with their witnessed effect, the Jerry switches in force, world/tempo/seed, and
//! the per-seed Score fingerprint — so running the wrong musical engine is immediately visible. If the
//! full C137 source cannot be admitted the run ERRORS; it never silently falls back to lesser music.
//!
//! WAVs are 48 kHz stereo PCM16 with the world's original gains and no normalization. These receipts
//! establish reproducibility and the Score fingerprint; they are NOT human-listening acceptance.

// The source fixtures and the preset are `#[path]`-shared with the integration tests. The example
// binary drives the sources through their `*_phrased` entry points, so the plain-delegating wrappers
// (`source`, `source_with_bridge_seed`) read as unused HERE while the tests still exercise them.
#[allow(dead_code)]
#[path = "c137_song/preset.rs"]
mod preset;
#[allow(dead_code)]
#[path = "c137_song/source.rs"]
mod source;
#[allow(dead_code)]
#[path = "c137_song/source_bridge.rs"]
mod source_bridge;
#[path = "c137_song/source_pop.rs"]
mod source_pop;
#[allow(dead_code)]
#[path = "c137_song/source_song.rs"]
mod source_song;

use gibson::audio::human_music::{
    argument::CompiledArgument,
    instrument::OscKind,
    kernel,
    score::{Role, Score},
    synth::{ProductionControl, StemMask},
    theme_family::Phrasing,
    theory::Mode,
    HumanMusicSynth, MusicWorld,
};
use gibson::audio::{buffer::StereoBlock, wav::write_wav_i16, OfflineRenderer, SampleRate};
use preset::{C137Preset, Jerry, Resolved};
use std::{error::Error, fmt::Write as _, fs, path::PathBuf};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The render palette for the smooth-pop arm. The HumanMusic engine is not chiptune — the "8-bit
/// beeps" the ear flagged are the BLACK_ICE palette (its keys voice is a 140 ms zero-sustain pulse,
/// a NES pluck). `Warm` renders the SAME composed Score through a warmer palette (glassy FM electric
/// piano, a filtered round bass, a soft machine kit, memory-bus chorus + a damped room) — a render
/// choice only, so every C137 render is byte-identical. `Beep` keeps the BLACK_ICE palette for the
/// A/B comparison. Only meaningful for the pop-groove arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopPalette {
    Warm,
    /// WARM, but a little less smooth (Operation Johnny Five R2): brighter, snappier FM keys and lead,
    /// a growlier plucky bass, a punchier kit and less chorus wash — more edge and bounce than WARM,
    /// without going back to the beepy BLACK_ICE voices. A point between WARM and BEEP.
    Punchy,
    Beep,
}

struct Options {
    out: PathBuf,
    seeds: Vec<u64>,
    tempo: f32,
    /// The resolved C137 preset (Full by default; Jerry switches downgrade it).
    preset: C137Preset,
    /// The bridge motif's seed, for the earned-return counterfactual (only used by the 16-bar arc).
    /// `None` ties the bridge to the song seed (the shipped arc); `Some(bs)` swaps in a different
    /// LAWFUL bridge over the same chart and the same returning hook.
    bridge_seed: Option<u64>,
    /// Experimental UPGRADE audition (`--source-answer`): make the drummer's breath answer genuinely
    /// answer the lead's question (content-derived + credited to the lead's call) instead of the
    /// accepted fixed-slot ghost. Off by default — it changes the audio, so it is an opt-in A/B arm,
    /// never the default, until the maintainer's ear accepts it.
    source_answer: bool,
    /// Also render a lead-muted mix (`full.nolead.wav`) alongside the normal mix (`--no-lead`): the
    /// band carries the song with the lead bus dropped. The survival test for the narrative handoff —
    /// the keys-carried bridge still sounds the theme; a lead-carried bridge falls to backing.
    no_lead: bool,
    /// Which smooth-pop harmonic flavor the `--form=pop-groove*` arm renders (ear-gated A/B; only
    /// meaningful when the pop-groove preset is selected). Defaults to `Circle`.
    pop_flavor: source_pop::PopFlavor,
    /// Which render palette the smooth-pop arm uses (`--pop-palette=warm|beep`; only meaningful for
    /// the pop-groove arm). Defaults to `Warm`; `Beep` keeps the BLACK_ICE palette for the A/B.
    pop_palette: PopPalette,
}

impl Options {
    fn parse() -> Result<Self> {
        let mut out = PathBuf::from("/tmp/c137-song");
        let mut seeds = vec![2112u64, 7, 99];
        let mut tempo = 144.0f32;
        let mut bridge_seed = None;
        let mut source_answer = false;
        let mut no_lead = false;
        let mut pop_flavor = source_pop::PopFlavor::Circle;
        let mut pop_palette = PopPalette::Warm;
        // The no-flag command is C137 FULL. Downgrades accumulate here, from `--jerry=` and from the
        // compatibility flags, which fold into the same mechanism so they manifest as the Jerry
        // switches they are.
        let mut preset = C137Preset::full();
        for arg in std::env::args().skip(1) {
            if let Some(value) = arg.strip_prefix("--out=") {
                if value.is_empty() {
                    return Err("--out requires a directory".into());
                }
                out = value.into();
            } else if let Some(value) = arg.strip_prefix("--seeds=") {
                seeds = value
                    .split(',')
                    .map(str::parse)
                    .collect::<std::result::Result<_, _>>()?;
            } else if let Some(value) = arg.strip_prefix("--tempo=") {
                tempo = value.parse()?;
            } else if let Some(value) = arg.strip_prefix("--jerry=") {
                for token in value.split(',').filter(|t| !t.is_empty()) {
                    preset = preset.jerry(Jerry::parse(token)?);
                }
            } else if let Some(value) = arg.strip_prefix("--bars=") {
                // Compatibility: an explicit bar count folds into the Jerry mechanism.
                match value.parse::<u32>()? {
                    16 => {} // the Full arc — no downgrade
                    8 => preset = preset.jerry(Jerry::Short),
                    _ => {
                        return Err("--bars must be 8 (verse→hook control) or 16 (full arc)".into())
                    }
                }
            } else if let Some(value) = arg.strip_prefix("--form=") {
                // Opt-in zoom-out: the full 32-bar song form (eight typed sections) vs the default
                // 16-bar vertical slice. Audio-changing and NOT a default — an ear-gated A/B arm.
                match value {
                    "slice" => {} // the default 16-bar arc — no change
                    "song" => preset = preset.song(),
                    "song-intro" => preset = preset.song_intro(),
                    "song-narrative" => preset = preset.song_narrative(),
                    "song-intro-narrative" => preset = preset.song_intro_narrative(),
                    "pop-groove" => {
                        preset = preset.pop_groove();
                        pop_flavor = source_pop::PopFlavor::Circle;
                    }
                    "pop-groove-lift" => {
                        preset = preset.pop_groove();
                        pop_flavor = source_pop::PopFlavor::Lift;
                    }
                    "pop-song" => {
                        preset = preset.pop_song();
                        pop_flavor = source_pop::PopFlavor::Circle;
                    }
                    "pop-song-lift" => {
                        preset = preset.pop_song();
                        pop_flavor = source_pop::PopFlavor::Lift;
                    }
                    "pop-band" => {
                        preset = preset.pop_band();
                        pop_flavor = source_pop::PopFlavor::Circle;
                    }
                    "pop-band-lift" => {
                        preset = preset.pop_band();
                        pop_flavor = source_pop::PopFlavor::Lift;
                    }
                    _ => {
                        return Err(
                            "--form must be slice (16-bar), song (32-bar), song-intro (32-bar + intro), song-narrative (32-bar, keys-carried bridge), song-intro-narrative (32-bar, keys-foreshadow intro), pop-groove (8-bar lead-free smooth-pop pocket, circle-of-4ths), pop-groove-lift (the same groove, leading-tone-pull progression), pop-song (8-bar Lead-carried smooth-pop verse->hook), pop-song-lift (the same song, leading-tone-pull progression), pop-band (8-bar lead-free band-only mini-song: verse backing -> chorus arrival), or pop-band-lift (the same backing, leading-tone-pull progression)"
                                .into(),
                        )
                    }
                }
            } else if let Some(value) = arg.strip_prefix("--bridge-seed=") {
                bridge_seed = Some(value.parse()?);
            } else if let Some(value) = arg.strip_prefix("--phrasing=") {
                // Compatibility: explicit phrasing folds into the Jerry mechanism (Full is `ensemble`).
                match value {
                    "ensemble" => {} // the Full lead — no downgrade
                    "expressive" => preset = preset.jerry(Jerry::DeafBand),
                    "plain" => preset = preset.jerry(Jerry::Plain),
                    _ => return Err("--phrasing must be plain, expressive, or ensemble".into()),
                }
            } else if let Some(value) = arg.strip_prefix("--pop-palette=") {
                // The smooth-pop render palette A/B (render choice only; C137 is byte-identical either way).
                match value {
                    "warm" => pop_palette = PopPalette::Warm,
                    "punchy" => pop_palette = PopPalette::Punchy,
                    "beep" => pop_palette = PopPalette::Beep,
                    _ => return Err("--pop-palette must be warm (default), punchy, or beep".into()),
                }
            } else if arg == "--source-answer" {
                // Experimental UPGRADE audition: the drummer truly answers the lead's question.
                source_answer = true;
            } else if arg == "--no-lead" {
                // Also emit a lead-muted mix — the survival test for the narrative bridge handoff.
                no_lead = true;
            } else {
                return Err(format!("unknown argument: {arg}").into());
            }
        }
        if !tempo.is_finite() || !(30.0..=300.0).contains(&tempo) {
            return Err("--tempo must be finite and between 30 and 300 BPM".into());
        }
        Ok(Self {
            out,
            seeds,
            tempo,
            preset,
            bridge_seed,
            source_answer,
            no_lead,
            pop_flavor,
            pop_palette,
        })
    }
}

/// The warm smooth-pop RENDER palette: the same composed Score, played through a warmer world than
/// the beepy BLACK_ICE one it was composed under. Reuses the existing VAPOR95 palette (a glassy FM
/// electric-piano keys voice, a filtered round bass, a soft drum-machine pocket, memory-bus chorus,
/// tempo echoes and a damped room — built for "glassy 80s pop/R&B"), retuned to the pop arm's key,
/// mode and tempo. This is a render-world only: composition still happens under `base`, and no C137
/// render path ever sees this world, so every accepted artifact stays byte-identical.
fn pop_world(base: &MusicWorld) -> MusicWorld {
    let mut w = MusicWorld::vapor95();
    w.tonic_pc = base.tonic_pc;
    w.mode = base.mode;
    w.tempo_bpm = base.tempo_bpm;
    // VAPOR95's tempo echo is a dotted-eighth/quarter feedback delay on the keys — it smeared the comp
    // off the beat (the "out of sync" the ear flagged). Drop it: the groove wants a tight pocket, not
    // cross-rhythmic repeats. Keep the memory-bus chorus (width), but pull the room back from a wash
    // (0.28) to a more present ambience (0.20) so the pocket reads as higher-energy, not distant.
    w.production.echo = None;
    w.production.space.mix = 0.20;
    w
}

/// WARM, but a little less smooth (Operation Johnny Five R2). The SAME VAPOR95-derived render world as
/// [`pop_world`] — composition is unchanged and C137 stays byte-identical — retuned toward more edge,
/// bounce and forward motion WITHOUT returning to the beepy BLACK_ICE voices: brighter, snappier FM
/// keys and lead (more harmonic bite, shorter tails), a growlier plucky bass (higher cutoff +
/// resonance, less sustain so it pops instead of holding), a punchier kit (more kick click, a snappier
/// snare, crisper hats, livelier ghosts), and less chorus wash with a touch more drive. A render-only
/// A/B against WARM — the maintainer's ear decides whether "less smooth" landed; nothing is promoted.
fn pop_world_punchy(base: &MusicWorld) -> MusicWorld {
    let mut w = pop_world(base); // start from the accepted warm render world, then de-smooth it
                                 // KEYS: de-glass the FM electric piano — more harmonic bite (index), a brighter filter, and a
                                 // shorter tail so each note speaks and clears (percussive-present, not a glassy pad).
    if let OscKind::Fm { index, .. } = &mut w.keys.osc {
        *index = 0.55;
    }
    w.keys.cutoff_hz = 2900.0;
    w.keys.resonance = 0.18;
    w.keys.adsr.1 = 0.32; // decay
    w.keys.adsr.3 = 0.28; // release
                          // LEAD: give the voice body and articulation so it cuts the mix instead of hazing into it.
    if let OscKind::Fm { index, .. } = &mut w.lead.osc {
        *index = 0.65;
    }
    w.lead.cutoff_hz = 3200.0;
    w.lead.resonance = 0.16;
    w.lead.adsr.0 = 0.010; // attack (crisper)
    w.lead.adsr.3 = 0.40; // release (a touch tighter)
                          // BASS: the biggest bounce lever — a growlier, plucky, defined bass that pops rather than sustains.
    w.bass.cutoff_hz = 760.0;
    w.bass.resonance = 0.30;
    w.bass.adsr.1 = 0.17; // decay
    w.bass.adsr.2 = 0.52; // sustain (lower = plucky bounce, not a smooth hold)
    w.bass.adsr.3 = 0.12; // release
                          // KIT: punch it up — more kick click + a tighter body, a snappier snare, crisper hats, livelier ghosts.
    w.kick.click = 0.5;
    w.kick.amp_tau = 0.10;
    w.snare.noise_hp_hz = 2000.0;
    w.snare.noise_tau = 0.09;
    w.snare.noise_mix = 0.78;
    w.hat_cutoff = 9500.0;
    w.ghost_amount = 0.32;
    // PRODUCTION: less wash, a touch more drive/presence.
    w.production.saturation = 1.6;
    w.production.space.mix = 0.16;
    if let Some(ch) = &mut w.production.chorus {
        ch.mix = 0.22;
        ch.depth_ms = 2.5;
    }
    w
}

/// Build the committed source for one seed under the resolved config. The SAME `Resolved` that drives
/// the manifest chooses the arc and the lead performance here, so the manifest can never over-report.
/// Any compile/admission failure propagates — the run never silently substitutes lesser music.
fn build_source(
    resolved: &Resolved,
    seed: u64,
    bridge_seed: Option<u64>,
    pop_flavor: source_pop::PopFlavor,
) -> CompiledArgument {
    // The smooth-pop groove is its own lead-free arm with its own `SmoothPop` domain and groove — it
    // does not go through the C137 arc builders or the capability caps (which are C137-specific), so
    // it returns here before `apply_caps`.
    if resolved.pop_band {
        // Operation The Homosapians R1: the lead-free band-only mini-song (verse backing → chorus
        // arrival). Checked BEFORE pop_song/pop_groove because it also sets `pop_groove` (for the pop
        // render path/palette). Its own `SmoothPop` domain; returns before `apply_caps` like the others.
        return source_pop::source_homosapians_backing(seed, pop_flavor);
    }
    if resolved.pop_song {
        // Operation Johnny Five: the Lead-carried verse→hook pop song. Like the groove, it is its own
        // `SmoothPop` domain and carries its OWN expressive capability flags, so it also returns here
        // before `apply_caps` (which resolves C137-arc capabilities the pop song sets for itself).
        return source_pop::source_pop_song(seed, pop_flavor);
    }
    if resolved.pop_groove {
        return source_pop::source_pop_groove(seed, pop_flavor);
    }
    let mut compiled = match resolved.bars {
        32 if resolved.song_narrative => {
            source_song::source_song_narrative_phrased(seed, resolved.phrasing)
        }
        32 if resolved.song_intro_narrative => {
            source_song::source_song_intro_narrative_phrased(seed, resolved.phrasing)
        }
        32 => match (resolved.song_intro, bridge_seed) {
            (true, Some(bs)) => {
                source_song::source_song_intro_with_bridge_seed_phrased(seed, bs, resolved.phrasing)
            }
            (true, None) => source_song::source_song_intro_phrased(seed, resolved.phrasing),
            (false, Some(bs)) => {
                source_song::source_song_with_bridge_seed_phrased(seed, bs, resolved.phrasing)
            }
            (false, None) => source_song::source_song_phrased(seed, resolved.phrasing),
        },
        16 => match bridge_seed {
            Some(bs) => source_bridge::source_with_bridge_seed_phrased(seed, bs, resolved.phrasing),
            None => source_bridge::source_phrased(seed, resolved.phrasing),
        },
        _ => source::source_phrased(seed, resolved.phrasing),
    };
    // Override the source builder's phrasing-derived capability defaults with the INDEPENDENTLY
    // resolved booleans, so a narrow Jerry switch (no-keys / straight-bass / generic-drums / flat-lead)
    // dials exactly its own capability and the rest of the band stays full.
    resolved.apply_caps(&mut compiled);
    compiled
}

fn render(score: &Score, world: &MusicWorld, block: usize, mask: StemMask) -> StereoBlock {
    let rate = SampleRate::STUDIO;
    let mut synth = HumanMusicSynth::with_production(score, world, rate, ProductionControl::NORMAL);
    synth.set_stem_mask(mask);
    let frames = synth.total_samples();
    OfflineRenderer::new(rate, block)
        .render(&mut synth, frames)
        .audio
}

/// T/N/D/C columns, mirroring the conversation example's score dump.
fn score_tsv(score: &Score) -> String {
    let mut text = format!(
        "T\t{}\t{}\t{}\n",
        score.tempo_bpm, score.beats_per_bar, score.total_beats
    );
    for n in &score.notes {
        writeln!(
            text,
            "N\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{:?}\t{}\t{:?}\t{:?}",
            n.role.label(),
            n.start_beat,
            n.dur_beats,
            n.pitch,
            n.velocity,
            n.prov.section.label(),
            n.prov.phrase,
            n.function,
            n.prov.role_note,
            n.prov.material,
            n.prov.actions
        )
        .unwrap();
    }
    for d in &score.drums {
        writeln!(
            text,
            "D\t{:?}\t{}\t{}\t{}\t{}\t{:?}",
            d.voice,
            d.start_beat,
            d.velocity,
            d.prov.section.label(),
            d.prov.role_note,
            d.prov.actions
        )
        .unwrap();
    }
    for c in &score.chords {
        writeln!(
            text,
            "C\t{}\t{}\t{}\t{:?}\t{:?}\t{}",
            c.start_beat, c.dur_beats, c.chord.root_pc, c.chord.quality, c.function, c.degree
        )
        .unwrap();
    }
    text
}

/// The global capability block of the effective-feature manifest (everything independent of seed).
fn manifest_header(
    resolved: &Resolved,
    world: &MusicWorld,
    bridge_seed: Option<u64>,
    pop_flavor: source_pop::PopFlavor,
) -> String {
    // The smooth-pop SONG (Operation Johnny Five) is a Lead-carried verse→hook, NOT the lead-free
    // groove — it gets its own honest header so the manifest never claims it is lead-free.
    if resolved.pop_song {
        let mut m = String::new();
        let _ = writeln!(
            m,
            "SMOOTH-POP SONG  (Operation Johnny Five — opt-in Lead-carried verse->hook; ear-gated, nothing promoted)"
        );
        let _ = writeln!(
            m,
            "  domain                : SmoothPop (groove-cycle closure, 4-bar loop)"
        );
        let _ = writeln!(
            m,
            "  form                  : 8 bars — Lead-carried VERSE (Establish, open) -> HOOK (altered-consequent, lands home)"
        );
        let _ = writeln!(
            m,
            "  harmony               : {}, looped x2",
            pop_flavor.song_harmony_label()
        );
        let _ = writeln!(
            m,
            "  lead                  : source-authored vocal-proxy topline (leap germ shared verse->hook; funk-syncopated; hook lifts, resolves home)"
        );
        let _ = writeln!(
            m,
            "  keys                  : comps guide-tone stabs under the hook (comp_keys); freed from the melody"
        );
        let _ = writeln!(
            m,
            "  bass                  : four breathing per-bar cells; approach leans toward the destination (anticipate_bass)"
        );
        let _ = writeln!(
            m,
            "  drums                 : steady pocket; answers the lead's breaths (answer_breaths) + translates its phrase-end rhythm"
        );
        let _ = writeln!(
            m,
            "  pad                   : rug enters on the hook (the earned arrival)"
        );
        let _ = writeln!(
            m,
            "  world                 : tonic_pc={}, {:?}, tempo={} BPM (render --tempo=104 for the pop feel)",
            world.tonic_pc, world.mode, world.tempo_bpm
        );
        let _ = writeln!(
            m,
            "  listening             : PENDING — the maintainer's ear gates this arm; nothing is promoted"
        );
        return m;
    }
    // The Homosapians backing (R1) is a lead-free band-only mini-song — checked before the groove
    // branch because it also sets `pop_groove`. Its own honest header: verse backing → chorus arrival.
    if resolved.pop_band {
        let mut m = String::new();
        let _ = writeln!(
            m,
            "HOMOSAPIANS BACKING  (Operation The Homosapians — opt-in lead-free band-only mini-song; woven harmony + pad bed; ear-gated, nothing promoted)"
        );
        let _ = writeln!(
            m,
            "  domain                : SmoothPop (groove-cycle closure, 8-bar woven realized route)"
        );
        let _ = writeln!(
            m,
            "  form                  : 8 bars — the keys COMP the chord carrier on every bar; a sparse ornamental lead; chorus hotter"
        );
        let _ = writeln!(
            m,
            "  harmony               : global {} — but WOVEN: verse CAMPS on a 2-chord fragment, chorus HOPS to a relative-major conjugate (ear-probe; realization law is Step 2)",
            pop_flavor.song_harmony_label()
        );
        let _ = writeln!(
            m,
            "  keys                  : the chordal CARRIER (comp_all_sections) — a rootless voicing on the 'and of 2' + a guide-tone shell/colour/grace on the 'and of 4', every bar; chords, not a line"
        );
        let _ = writeln!(
            m,
            "  lead                  : a SPARSE ornamental line (a few soft chord-tones, mostly the comp's bare bar) — the expression over the chords, not a topline"
        );
        let _ = writeln!(
            m,
            "  bass                  : EIGHT per-bar cells (four verse, four driven chorus) walking/funking in the pocket; dark ♭7 at each E7 cadence"
        );
        let _ = writeln!(
            m,
            "  drums                 : funk/disco pocket (pop_funk_kit); pad: a soft continuous BED (pad_all_sections) smoothing the vibe under the comp"
        );
        let _ = writeln!(
            m,
            "  world                 : tonic_pc={}, {:?}, tempo={} BPM (render --tempo=104 for the pop feel)",
            world.tonic_pc, world.mode, world.tempo_bpm
        );
        let _ = writeln!(
            m,
            "  listening             : PENDING — the maintainer's ear gates this arm; nothing is promoted"
        );
        return m;
    }
    // The smooth-pop groove is NOT a C137 arc, so it gets its own honest header rather than the C137
    // capability manifest (which would claim arc/lead features this arm does not have).
    if resolved.pop_groove {
        let mut m = String::new();
        let _ = writeln!(
            m,
            "SMOOTH-POP GROOVE  (opt-in R2 arm — lead-free, ear-gated A/B flavor: {})",
            pop_flavor.form_token()
        );
        let _ = writeln!(
            m,
            "  domain                : SmoothPop (groove-cycle closure, 4-bar loop)"
        );
        let _ = writeln!(
            m,
            "  form                  : 8 bars — two Keys-carried Establish episodes, NO foreground lead"
        );
        let _ = writeln!(
            m,
            "  harmony               : {}, looped x2",
            pop_flavor.harmony_label()
        );
        let _ = writeln!(
            m,
            "  bass                  : four breathing per-bar cells (root downbeat anchor + syncopated octave/fifth/ghost off-beats + turnaround push)"
        );
        let _ = writeln!(
            m,
            "  keys                  : guide-tone comp voiced to the progression (carried); drums: steady pocket; pad: silent"
        );
        let _ = writeln!(
            m,
            "  world                 : tonic_pc={}, {:?}, tempo={} BPM (render --tempo=104 for the pop feel)",
            world.tonic_pc, world.mode, world.tempo_bpm
        );
        let _ = writeln!(
            m,
            "  listening             : PENDING — the maintainer's ear gates this arm; nothing is promoted"
        );
        return m;
    }
    let mut m = String::new();
    let title = if resolved.is_full() {
        "C137 Full  (default — excellence is not opt-in)".to_string()
    } else if resolved.jerry.is_empty() && resolved.bars == 32 {
        if resolved.song_intro {
            "C137 Song+Intro  (32-bar full-song form with front intro — opt-in, ear-gated)"
                .to_string()
        } else {
            "C137 Song  (32-bar full-song form — opt-in zoom-out, ear-gated)".to_string()
        }
    } else {
        let names: Vec<_> = resolved.jerry.iter().map(|j| j.label()).collect();
        format!("C137 Jerry[{}]  — DOWNGRADED", names.join(","))
    };
    let _ = writeln!(m, "C137 EFFECTIVE FEATURE MANIFEST");
    let _ = writeln!(m, "  preset                : {title}");
    // Capability lines — resolved from the config, each tagged FULL or the Jerry that reduced it.
    let arc = if resolved.bars == 32 {
        if resolved.song_intro {
            "32 bars — intro→verse→chorus→verse→chorus→bridge→chorus→chorus (SONG+intro, opt-in)"
        } else {
            "32 bars — verse→chorus→verse→chorus→bridge→chorus×3 (full SONG form, opt-in)"
        }
    } else if resolved.bars == 16 {
        "16 bars — verse→hook→bridge→return (FULL)"
    } else {
        "8 bars — verse→hook control (Jerry: short)"
    };
    let _ = writeln!(m, "  song arc              : {arc}");
    let lead = match resolved.phrasing {
        Phrasing::Ensemble => {
            "expressive articulation + tension-release contour + ensemble breath answer (FULL)"
        }
        Phrasing::Expressive => {
            "expressive articulation + tension-release contour; band DEAF (Jerry: deaf)"
        }
        Phrasing::Plain => "plain legato re-tile + flat ramp (Jerry: plain)",
    };
    let _ = writeln!(m, "  lead performance      : {lead}");
    let earned = if resolved.bars >= 16 {
        "active — the return accompaniment depends on the bridge travelled (counterfactual: --bridge-seed)"
    } else {
        "n/a — the 8-bar control has no bridge"
    };
    let _ = writeln!(m, "  earned bridge→return  : {earned}");
    let rug = if resolved.bars >= 16 {
        "active — the pad frames the home arrivals (verse/bridge stay dry)"
    } else {
        "n/a — the 8-bar control stages the pad silent"
    };
    let _ = writeln!(m, "  selective pad (rug)   : {rug}");
    // The three independently-switchable band capabilities (decoupled from the lead phrasing): each
    // reports active (FULL) or off, so a narrow Jerry downgrade can never masquerade as Full.
    let keys_cap = if resolved.comp_keys {
        "active — pianist rootless chord voicings comp the holes under the home arrivals (FULL)"
    } else {
        "off — the keyboard keeps its sparse per-episode answer (Jerry: no-keys, or compound plain)"
    };
    let _ = writeln!(m, "  keyboard comp         : {keys_cap}");
    let bass_cap = if resolved.anticipate_bass {
        "active — the foundation leans on the dominant (V->I) into the home arrivals (FULL)"
    } else {
        "off — the bass steps a blind chromatic approach (Jerry: straight-bass, or compound plain)"
    };
    let _ = writeln!(m, "  bass anticipation     : {bass_cap}");
    let drum_cap = if resolved.translate_lead_rhythm {
        "active — the phrase-end fill quotes the lead's rhythm, the lead-echo (FULL)"
    } else {
        "off — the drummer plays the archived generic rising run (Jerry: generic-drums, or compound plain)"
    };
    let _ = writeln!(m, "  drum translation      : {drum_cap}");
    let _ = writeln!(
        m,
        "  shared pocket/hearing : active — all voices lock one pocket; drums read lead+harmony only (keys-independent pulse)"
    );
    let jerry = if resolved.jerry.is_empty() {
        "none".to_string()
    } else {
        resolved
            .jerry
            .iter()
            .map(|j| format!("{} (removes {})", j.label(), j.removes()))
            .collect::<Vec<_>>()
            .join("\n                          ")
    };
    let _ = writeln!(m, "  jerry switches        : {jerry}");
    let _ = writeln!(
        m,
        "  negative controls     : CoupledR8 and low-register BASS_WEAVE are ear-REJECTED — never in Full, reachable only by their own explicit pathways"
    );
    let _ = writeln!(
        m,
        "  world                 : tonic_pc={}, {:?}, sevenths={}, modal_mixture={}, tempo={} BPM",
        world.tonic_pc, world.mode, world.use_sevenths, world.allow_modal_mixture, world.tempo_bpm
    );
    if let Some(bs) = bridge_seed {
        let note = if resolved.bars >= 16 {
            format!("bridge-seed={bs} (earned-return counterfactual arm)")
        } else {
            format!("bridge-seed={bs} IGNORED (no bridge on the 8-bar control)")
        };
        let _ = writeln!(m, "  counterfactual        : {note}");
    }
    let git = option_env!("GIT_SHA")
        .unwrap_or("not embedded — the per-seed Score fingerprint is the content-integrity anchor");
    let _ = writeln!(m, "  source                : {git}");
    m
}

/// One seed's witness line: the Score fingerprint plus the realized effect of each advertised
/// capability (D5: no capability is reported active unless it left a trace; a requested-but-idle
/// capability is reported as such rather than silently claimed).
fn manifest_seed_line(
    resolved: &Resolved,
    seed: u64,
    score: &Score,
    audio: &StereoBlock,
    tonic: i32,
) -> String {
    let count = |r: Role| score.role_notes(r).count();
    let breaths = preset::lead_breaths(score);
    let answers = preset::breath_answers(score);
    let pad = preset::pad_notes(score);
    let spread = preset::lead_velocity_spread(score);
    let comps = preset::keys_comps(score);
    let echoes = preset::lead_echoes(score);
    let antics = preset::bass_anticipations(score, tonic);
    // Honest status for the expressive lead (its own phrasing switch).
    let expr_status = if resolved.phrasing == Phrasing::Plain {
        "off(plain)".to_string()
    } else if breaths > 0 {
        format!("ACTIVE(breaths={breaths}, vspread={spread:.2})")
    } else {
        format!("requested, 0 breaths this seed (vspread={spread:.2})")
    };
    // Each independently-switchable band capability: ACTIVE(trace) / off (switched out) /
    // requested-but-idle (capability on, but this seed's material left it nothing to do).
    let witness = |on: bool, n: usize| {
        if !on {
            "off".to_string()
        } else if n > 0 {
            format!("ACTIVE({n})")
        } else {
            "requested, 0 this seed".to_string()
        }
    };
    let ans_status = witness(resolved.answer_breaths, answers);
    let comp_status = witness(resolved.comp_keys, comps);
    let bass_status = witness(resolved.anticipate_bass, antics);
    let drum_status = witness(resolved.translate_lead_rhythm, echoes);
    format!(
        "  seed {seed:<5}: fp={:016x}  lead={} keys={} bass={} pad={} drums={} frames={}  | expressive={expr_status}  breath-answer={ans_status}  keys-comp={comp_status}  bass-antic={bass_status}  drum-echo={drum_status}  rug={pad}",
        score.fingerprint(),
        count(Role::Lead),
        count(Role::Keys),
        count(Role::Bass),
        count(Role::Pad),
        score.drums.len(),
        audio.frames(),
    )
}

fn main() -> Result<()> {
    let options = Options::parse()?;
    let resolved = options.preset.resolve();

    let mut world = MusicWorld::black_ice();
    world.tempo_bpm = options.tempo;
    world.tonic_pc = 9;
    world.mode = Mode::Aeolian;
    world.use_sevenths = true;
    world.allow_modal_mixture = true;

    if options.out.exists() && fs::read_dir(&options.out)?.next().is_some() {
        return Err(format!("output directory is not empty: {}", options.out.display()).into());
    }
    fs::create_dir_all(&options.out)?;

    let mut manifest = manifest_header(&resolved, &world, options.bridge_seed, options.pop_flavor);
    if options.source_answer {
        let note = "  source answer         : ON (experimental) — the drummer's breath answer quotes the lead's question + credits its call (--source-answer audition; default OFF)";
        let _ = writeln!(manifest, "{note}");
    }
    if resolved.pop_groove {
        let note = match options.pop_palette {
            PopPalette::Warm => "  palette               : WARM (render-only) — VAPOR95-style: glassy FM electric piano, filtered round bass, soft machine kit, chorus + damped room. C137 byte-identical.",
            PopPalette::Punchy => "  palette               : PUNCHY (render-only) — WARM de-smoothed (R2): brighter/snappier FM keys + lead, a growlier plucky bass, a punchier kit, less chorus wash. Between WARM and BEEP. C137 byte-identical.",
            PopPalette::Beep => "  palette               : BEEP (render-only) — the original BLACK_ICE voices (the 8-bit-ish pulse keys), kept for the A/B.",
        };
        let _ = writeln!(manifest, "{note}");
    }
    print!("{manifest}");

    for seed in &options.seeds {
        // Build under the resolved config. A failure here propagates — no silent fallback to Jerry.
        let mut candidate_source =
            build_source(&resolved, *seed, options.bridge_seed, options.pop_flavor);
        // The experimental source-answer upgrade is applied here (never by a source builder), so it
        // is strictly an opt-in audition arm and the default render stays byte-identical.
        candidate_source.source_answer = options.source_answer;
        let take = kernel::perform_song(&candidate_source, &world)?;
        // Re-derive the committed candidate before rendering: the exported Score is a candidate that
        // must still satisfy the event/source contract it was committed under.
        take.validate_candidate(take.score())?;
        let score = take.score();
        // The smooth-pop arm may RENDER through a warmer palette than it composed under — the beepy
        // BLACK_ICE keys were the "8-bit" sound the ear flagged. Render-world only: composition stayed
        // under `world` above, and no C137 path is pop_groove, so `rw` is `&world` for every C137
        // render and those artifacts are byte-identical.
        // Render-world palette: WARM and PUNCHY both play under a pop render world (composition stays
        // under `world`); BEEP keeps the BLACK_ICE compose voices. No C137 path is pop_groove, so `rw`
        // is `&world` for every C137 render and those artifacts stay byte-identical.
        let render_pop_world = if resolved.pop_groove {
            match options.pop_palette {
                PopPalette::Warm => Some(pop_world(&world)),
                PopPalette::Punchy => Some(pop_world_punchy(&world)),
                PopPalette::Beep => None,
            }
        } else {
            None
        };
        let rw = render_pop_world.as_ref().unwrap_or(&world);
        let dir = options.out.join(format!("seed-{seed}"));
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("score.tsv"), score_tsv(score))?;
        let audio = render(score, rw, 256, StemMask::full());
        if !audio.left.iter().chain(&audio.right).all(|s| s.is_finite()) {
            return Err(format!("non-finite rendered PCM: seed {seed}").into());
        }
        write_wav_i16(dir.join("full.normal.wav"), &audio, SampleRate::STUDIO)?;
        // The lead-mute SURVIVAL render (`--no-lead`): drop the lead bus and let the band carry the
        // song. In the narrative arm the keys-carried bridge still sounds the theme; in a lead-carried
        // arm the bridge falls to backing — the ear test the 2026-10-09 band-story study turned on.
        if options.no_lead {
            let muted = render(
                score,
                rw,
                256,
                StemMask {
                    lead: false,
                    ..StemMask::full()
                },
            );
            if !muted.left.iter().chain(&muted.right).all(|s| s.is_finite()) {
                return Err(format!("non-finite rendered PCM (no-lead): seed {seed}").into());
            }
            write_wav_i16(dir.join("full.nolead.wav"), &muted, SampleRate::STUDIO)?;
        }
        let line = manifest_seed_line(&resolved, *seed, score, &audio, world.tonic_pc);
        println!("{line}");
        let _ = writeln!(manifest, "{line}");
    }

    fs::write(options.out.join("manifest.txt"), &manifest)?;
    println!(
        "Artifacts: {} (manifest.txt written). Human listening remains pending.",
        options.out.display()
    );
    Ok(())
}
