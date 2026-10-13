//! THE JERRY LAW — C137 Full is the default; a Jerry switch is the explicit downgrade.
//!
//! The best music HumanMusic knows how to make must not hide behind a secret combination of
//! positive opt-in flags. The ordinary, no-flag `c137_song` invocation resolves to [`C137Preset::full`]:
//! the complete available song arc, the expressive lead, and the ensemble breath response — every
//! compatible, musically-intended C137 capability that is actually reachable on this render path.
//!
//! A **Jerry switch** ([`Jerry`]) deliberately REMOVES one of those capabilities, for diagnostics,
//! controlled A/B comparison, or stylistic austerity. Jerry switches default OFF. Each one maps to a
//! real, witnessed reduction on the render path — never a cosmetic label.
//!
//! [`Resolved`] is the SINGLE SOURCE OF TRUTH: the same value drives both the printed capability
//! manifest and the source build, so the engine can never report Full while quietly building less
//! (the no-hidden-downgrade law). The witness helpers ([`breath_answers`], [`pad_notes`],
//! [`lead_breaths`], [`lead_velocity_spread`]) count the realized effect from a rendered Score, so
//! the manifest never advertises a capability that left no trace (the no-fake-enablement law).
//!
//! This module is `#[path]`-shared with the long-form integration tests, which exercise the
//! default-correctness laws (D1–D7) directly against [`C137Preset`].

use gibson::audio::human_music::{
    argument::CompiledArgument,
    score::{Role, Score},
    theme_family::Phrasing,
};

/// One deliberate downgrade of the full C137 performance. Each variant removes exactly one capability
/// from the render path and is OFF by default. Compatibility flags fold into the same mechanism
/// (`--bars=8` ⇒ [`Jerry::Short`]; `--phrasing=plain` ⇒ [`Jerry::Plain`]; `--phrasing=expressive` ⇒
/// [`Jerry::DeafBand`]), so an old explicit downgrade is reported honestly as the Jerry switch it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jerry {
    /// Render the short two-episode verse→hook control (8 bars) instead of the full
    /// verse→hook→bridge→return arc (16 bars). Removes the bridge, the earned return, and the rug.
    Short,
    /// Play the lead PLAIN — legato re-tile + a flat rising ramp — instead of the authored
    /// articulation and tension-release contour. Because the ensemble breath answer rides the
    /// expressive lead's composed silences, the plain lead also silences it. The robotic-lead control.
    Plain,
    /// Keep the expressive lead but make the band DEAF to its breaths — the drummer no longer steps
    /// into the lead's silences. Isolates the ensemble-exchange capability from the lead performance.
    DeafBand,
    /// Play the lead PLAIN but keep the accepted band (keys comp, bass anticipation, drum
    /// translation, breath answer stay ON). The narrow, lead-ONLY downgrade — the robotic lead
    /// WITHOUT the compound band-stripping of [`Jerry::Plain`]. (The ensemble answer rides composed
    /// silences, so a plain lead legitimately yields it nothing to answer; the keys/bass capabilities,
    /// which read harmony and the chart, are untouched.)
    FlatLead,
    /// Remove ONLY the keyboard comp (the pianist's rootless chord voicings in the rhythmic holes).
    /// The lead, bass, drums and breath answer are untouched. Isolates the keys-comp capability.
    NoKeyComp,
    /// Remove ONLY the bass's destination anticipation (the V→I lean into the home arrivals) — the
    /// foundation reverts to its archived blind chromatic approach. Keys, drums and lead untouched.
    StraightBass,
    /// Remove ONLY the drum lead-rhythm translation (the lead-echo fill reverts to the archived
    /// generic rising run). Keys, bass, lead and breath answer untouched. Isolates the drum capability.
    GenericDrums,
}

impl Jerry {
    /// Parse one `--jerry=` token. `deaf` and `deaf-band` are accepted spellings of [`Jerry::DeafBand`].
    pub fn parse(token: &str) -> Result<Jerry, String> {
        match token {
            "short" => Ok(Jerry::Short),
            "plain" => Ok(Jerry::Plain),
            "deaf" | "deaf-band" => Ok(Jerry::DeafBand),
            "flat-lead" => Ok(Jerry::FlatLead),
            "no-keys" => Ok(Jerry::NoKeyComp),
            "straight-bass" => Ok(Jerry::StraightBass),
            "generic-drums" => Ok(Jerry::GenericDrums),
            other => Err(format!(
                "unknown Jerry switch `{other}` (known: short, plain, deaf, flat-lead, no-keys, straight-bass, generic-drums)"
            )),
        }
    }

    /// The short token for diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Jerry::Short => "short",
            Jerry::Plain => "plain",
            Jerry::DeafBand => "deaf",
            Jerry::FlatLead => "flat-lead",
            Jerry::NoKeyComp => "no-keys",
            Jerry::StraightBass => "straight-bass",
            Jerry::GenericDrums => "generic-drums",
        }
    }

    /// The one capability this switch removes, for the manifest.
    pub fn removes(self) -> &'static str {
        match self {
            Jerry::Short => {
                "the full arc (bridge + earned return + rug) — renders the 8-bar verse→hook control"
            }
            Jerry::Plain => {
                "expressive articulation + dynamic contour + the ensemble breath answer + keys comp + bass anticipation + drum translation (the compound robotic-lead control)"
            }
            Jerry::DeafBand => "the ensemble breath answer (the band goes deaf to the lead's silences)",
            Jerry::FlatLead => {
                "ONLY the lead's expressive articulation + dynamic contour (keys/bass/drums stay full)"
            }
            Jerry::NoKeyComp => "ONLY the keyboard comp (the pianist's chord voicings in the holes)",
            Jerry::StraightBass => "ONLY the bass's destination anticipation (the V->I lean home)",
            Jerry::GenericDrums => "ONLY the drum lead-rhythm translation (the lead-echo fill)",
        }
    }
}

/// The resolved render configuration — the single source of truth for a run. `bars` selects the song
/// arc (16 = full verse→hook→bridge→return, 8 = the verse→hook control); `phrasing` selects the lead
/// performance. The four capability booleans are resolved INDEPENDENTLY of the lead phrasing, so a
/// lead-only downgrade (`flat-lead`) no longer silences the keys/bass/drums the way the compound
/// `plain` control does. `jerry` records the downgrades in effect. [`apply_caps`](Resolved::apply_caps)
/// stamps the four booleans onto the built `CompiledArgument`, overriding the source builder's
/// phrasing-derived defaults, so each capability is switchable on its own promised causal domain.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub bars: u32,
    pub phrasing: Phrasing,
    /// The keyboard comps the holes (pianist chord voicings). Reads harmony — independent of the lead.
    pub comp_keys: bool,
    /// The bass leans on the dominant into the home arrivals. Reads the chart — independent of the lead.
    pub anticipate_bass: bool,
    /// The drum phrase-end fill quotes the lead's rhythm (the lead-echo). Reads the lead.
    pub translate_lead_rhythm: bool,
    /// The band answers the lead's composed breaths (the `Ensemble` capability). Rides the lead's silences.
    pub answer_breaths: bool,
    pub jerry: Vec<Jerry>,
}

impl Resolved {
    /// Whether this is the full C137 default (no downgrades).
    pub fn is_full(&self) -> bool {
        self.jerry.is_empty() && self.bars == 16 && self.phrasing == Phrasing::Ensemble
    }

    /// The total song length in beats (four beats per bar).
    pub fn total_beats(&self) -> f64 {
        f64::from(self.bars) * 4.0
    }

    /// Stamp the resolved capability booleans onto a freshly-built source, OVERRIDING the source
    /// builder's phrasing-derived defaults. This is where the Jerry decoupling takes physical effect:
    /// the kernel gates each capability on exactly these four flags (`comp_keys` @kernel.rs:1446,
    /// `anticipate_bass` @1598, `answer_breaths` @1793, `translate_lead_rhythm` @1838), so a narrow
    /// switch changes only its own promised causal domain. The same method drives both the example's
    /// render and the integration tests, so the manifest can never diverge from what is built.
    pub fn apply_caps(&self, compiled: &mut CompiledArgument) {
        compiled.comp_keys = self.comp_keys;
        compiled.anticipate_bass = self.anticipate_bass;
        compiled.translate_lead_rhythm = self.translate_lead_rhythm;
        compiled.answer_breaths = self.answer_breaths;
    }
}

/// The C137 generation preset. Build [`C137Preset::full`] (the default) and apply [`Jerry`] switches;
/// [`resolve`](C137Preset::resolve) folds them into a [`Resolved`] config. Full is 16-bar `Ensemble`;
/// each Jerry switch dials exactly its capability down and nothing else.
#[derive(Debug, Clone, Default)]
pub struct C137Preset {
    jerry: Vec<Jerry>,
    /// The full 32-bar SONG form (opt-in zoom-out from the 16-bar vertical slice). Off by default;
    /// `--form=song` turns it on. It is not a Jerry downgrade (it adds form, it never removes a
    /// capability) and it is not "Full" — it is its own opt-in arm, ear-gated before any default.
    song: bool,
}

impl C137Preset {
    /// C137 Full — the default: the complete available arc, the expressive lead, the ensemble response.
    pub fn full() -> Self {
        Self::default()
    }

    /// Add a Jerry downgrade (idempotent — repeats are de-duplicated at resolve time).
    pub fn jerry(mut self, switch: Jerry) -> Self {
        if !self.jerry.contains(&switch) {
            self.jerry.push(switch);
        }
        self
    }

    /// Select the full 32-bar song form. The song sets its own length (it overrides the archival
    /// `--bars`/`short` bar count), so the resolved arc is the eight-section pop skeleton.
    pub fn song(mut self) -> Self {
        self.song = true;
        self
    }

    /// Fold the preset and its switches into the resolved render configuration. Resolution starts
    /// from Full (16-bar `Ensemble`, every capability ON) and applies each downgrade. A switch only
    /// ever turns something OFF (the lead to plain, or one capability boolean to false) — it never
    /// UPGRADES — so the result is order-independent and each narrow switch composes cleanly with the
    /// compound ones (`plain` then `no-keys` is still just `plain`).
    ///
    /// The four capability booleans are resolved HERE, independently of the lead phrasing, which is
    /// the whole point of the decoupling: `flat-lead` drops the lead to plain while leaving keys, bass
    /// and drums full, whereas the archival `plain` control strips all of them together (its pinned,
    /// byte-frozen historical meaning). `deaf` removes exactly the breath answer.
    pub fn resolve(&self) -> Resolved {
        let mut bars = 16u32;
        let mut phrasing = Phrasing::Ensemble;
        // Every accepted capability is ON in Full; a Jerry switch dials exactly its own off.
        let mut comp_keys = true;
        let mut anticipate_bass = true;
        let mut translate_lead_rhythm = true;
        let mut answer_breaths = true;
        for switch in &self.jerry {
            match switch {
                Jerry::Short => bars = 8,
                // The compound archival control: plain lead AND the whole band stripped back to the
                // historical baseline. Its output is byte-frozen (D3); do not narrow its meaning.
                Jerry::Plain => {
                    phrasing = Phrasing::Plain;
                    comp_keys = false;
                    anticipate_bass = false;
                    translate_lead_rhythm = false;
                    answer_breaths = false;
                }
                // Deaf drops Ensemble→Expressive only (never revives a plain lead) and removes the
                // breath answer; the keys/bass/drums capabilities stay on (the accepted expressive band).
                Jerry::DeafBand => {
                    if phrasing == Phrasing::Ensemble {
                        phrasing = Phrasing::Expressive;
                    }
                    answer_breaths = false;
                }
                // The narrow, lead-ONLY downgrade: plain lead, band capabilities untouched. (A plain
                // lead opens no breaths, so `answer_breaths` stays requested-but-idle — honest, and the
                // render is identical to flipping it off: a legato lead gives the band nothing to answer.)
                Jerry::FlatLead => {
                    if phrasing == Phrasing::Ensemble {
                        phrasing = Phrasing::Plain;
                    }
                }
                Jerry::NoKeyComp => comp_keys = false,
                Jerry::StraightBass => anticipate_bass = false,
                Jerry::GenericDrums => translate_lead_rhythm = false,
            }
        }
        // The song form sets its own length, overriding any archival bar count. It adds form, never
        // removes a capability, so the accepted capabilities resolved above are untouched.
        if self.song {
            bars = 32;
        }
        Resolved {
            bars,
            phrasing,
            comp_keys,
            anticipate_bass,
            translate_lead_rhythm,
            answer_breaths,
            jerry: self.jerry.clone(),
        }
    }
}

// --- D5 witnesses: realized effects counted from a rendered Score ---------------------------------
// Each capability the manifest advertises as ACTIVE must leave one of these traces on a Full render,
// and must drop to its downgraded value when the matching Jerry switch is thrown. The tests pin that.

/// Breath answers: full-voiced drum strokes the band placed into the lead's composed silences (the
/// `Ensemble` capability). Zero on every non-ensemble arm.
pub fn breath_answers(score: &Score) -> usize {
    score
        .drums
        .iter()
        .filter(|d| d.prov.role_note == "answer")
        .count()
}

/// Pad (rug) notes: the sustained chordal bed that frames the home arrivals. Zero on the 8-bar
/// control (its pad is staged Silent).
pub fn pad_notes(score: &Score) -> usize {
    score.role_notes(Role::Pad).count()
}

/// Composed breaths in the lead: inter-onset gaps where the previous note's gate has closed before
/// the next attack (a silence the expressive articulation opens; a legato plain lead leaves none).
pub fn lead_breaths(score: &Score) -> usize {
    let mut ns: Vec<_> = score.role_notes(Role::Lead).collect();
    ns.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    ns.windows(2)
        .filter(|w| {
            let end = w[0].start_beat + f64::from(w[0].dur_beats);
            w[1].start_beat - end > 0.25
        })
        .count()
}

/// The lead's dynamic range (max − min velocity): wide under an expressive contour, narrow under the
/// plain flat ramp.
pub fn lead_velocity_spread(score: &Score) -> f32 {
    let vs: Vec<f32> = score.role_notes(Role::Lead).map(|n| n.velocity).collect();
    if vs.is_empty() {
        return 0.0;
    }
    let hi = vs.iter().copied().fold(f32::MIN, f32::max);
    let lo = vs.iter().copied().fold(f32::MAX, f32::min);
    hi - lo
}

/// Keyboard comp notes: the pianist's rootless chord voicings laid in the rhythmic holes (the
/// `comp_keys` capability). Zero when the comp is switched off (`no-keys`, or the compound `plain`).
pub fn keys_comps(score: &Score) -> usize {
    score
        .role_notes(Role::Keys)
        .filter(|n| n.prov.role_note == "comp")
        .count()
}

/// Drum lead-echo strokes: the phrase-end fill that quotes the lead's rhythm (the
/// `translate_lead_rhythm` capability). Zero when the drummer plays the archived generic rising run
/// (`generic-drums`, or the compound `plain`).
pub fn lead_echoes(score: &Score) -> usize {
    score
        .drums
        .iter()
        .filter(|d| d.prov.role_note == "lead-echo")
        .count()
}

/// Bass destination anticipations: beat-four approaches that lean on the dominant (a fifth above the
/// tonic) into a tonic downbeat — the V the ear hears resolving home (the `anticipate_bass`
/// capability). The archived straight bass steps a blind chromatic leading tone there instead, so this
/// is zero under `straight-bass` (and the compound `plain`). Mirrors the kernel's own V→I construction
/// read independently from the committed Score; `tonic` is the world's tonic pitch-class.
pub fn bass_anticipations(score: &Score, tonic: i32) -> usize {
    let dominant = (tonic + 7).rem_euclid(12);
    let mut bass: Vec<(f64, i32)> = score
        .role_notes(Role::Bass)
        .map(|n| (n.start_beat, n.pitch.rem_euclid(12)))
        .collect();
    bass.sort_by(|a, b| a.0.total_cmp(&b.0));
    // The pocket leans attacks a hair off the exact grid, so match the note nearest a target beat.
    let near = |target: f64| {
        bass.iter()
            .find(|(b, _)| (b - target).abs() < 0.3)
            .map(|(_, pc)| *pc)
    };
    let bars = (score.total_beats / 4.0).round() as i64;
    let mut count = 0;
    for bar in 1..bars {
        let home_arrival = near(bar as f64 * 4.0) == Some(tonic.rem_euclid(12));
        let leans_dominant = near((bar - 1) as f64 * 4.0 + 3.0) == Some(dominant);
        if home_arrival && leans_dominant {
            count += 1;
        }
    }
    count
}
