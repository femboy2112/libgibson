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
}

impl Jerry {
    /// Parse one `--jerry=` token. `deaf` and `deaf-band` are accepted spellings of [`Jerry::DeafBand`].
    pub fn parse(token: &str) -> Result<Jerry, String> {
        match token {
            "short" => Ok(Jerry::Short),
            "plain" => Ok(Jerry::Plain),
            "deaf" | "deaf-band" => Ok(Jerry::DeafBand),
            other => Err(format!(
                "unknown Jerry switch `{other}` (known: short, plain, deaf)"
            )),
        }
    }

    /// The short token for diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Jerry::Short => "short",
            Jerry::Plain => "plain",
            Jerry::DeafBand => "deaf",
        }
    }

    /// The one capability this switch removes, for the manifest.
    pub fn removes(self) -> &'static str {
        match self {
            Jerry::Short => {
                "the full arc (bridge + earned return + rug) — renders the 8-bar verse→hook control"
            }
            Jerry::Plain => {
                "expressive articulation + dynamic contour + the ensemble breath answer (robotic lead)"
            }
            Jerry::DeafBand => "the ensemble breath answer (the band goes deaf to the lead's silences)",
        }
    }
}

/// The resolved render configuration — the single source of truth for a run. `bars` selects the song
/// arc (16 = full verse→hook→bridge→return, 8 = the verse→hook control); `phrasing` selects the lead
/// performance and, at `Ensemble`, the band breath answer. `jerry` records the downgrades in effect.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub bars: u32,
    pub phrasing: Phrasing,
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
}

/// The C137 generation preset. Build [`C137Preset::full`] (the default) and apply [`Jerry`] switches;
/// [`resolve`](C137Preset::resolve) folds them into a [`Resolved`] config. Full is 16-bar `Ensemble`;
/// each Jerry switch dials exactly its capability down and nothing else.
#[derive(Debug, Clone, Default)]
pub struct C137Preset {
    jerry: Vec<Jerry>,
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

    /// Fold the preset and its switches into the resolved render configuration. Resolution starts
    /// from Full (16-bar `Ensemble`) and applies each downgrade; it never UPGRADES (so `plain` then
    /// `deaf` stays plain), which makes the result order-independent.
    pub fn resolve(&self) -> Resolved {
        let mut bars = 16u32;
        let mut phrasing = Phrasing::Ensemble;
        for switch in &self.jerry {
            match switch {
                Jerry::Short => bars = 8,
                Jerry::Plain => phrasing = Phrasing::Plain,
                // Deaf drops Ensemble→Expressive only; it must never revive a plain lead.
                Jerry::DeafBand => {
                    if phrasing == Phrasing::Ensemble {
                        phrasing = Phrasing::Expressive;
                    }
                }
            }
        }
        Resolved {
            bars,
            phrasing,
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
