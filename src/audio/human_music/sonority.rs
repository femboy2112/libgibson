//! **Sonority** — Round VIII's vertical theory: *why do THESE notes coexist right now?*
//!
//! Every earlier round asked the horizontal question — why is this note here? — and answered it per
//! player: each note carries a [`PitchFunction`] against the chord, and
//! [`super::diagnostics::RealizationDiagnostics`] counts the ones left unjustified (zero, for rounds).
//! Round VII/VIIb's listen then heard the band "harmonically out of tune despite the individual notes
//! being justified": a legal 9th in the lead, a legal 13th in the keys, lawful guide tones in the pad
//! and a chord tone in the bass can still SUM to a bad sonority. Zero unjustified notes does not imply
//! a coherent chord.
//!
//! This module is the ONE definition of vertical coherence. The diagnostics measure the realized
//! Score with it, and generation (the ensemble state, the joint voicing path, the coupled material
//! projection, the bass) consults the SAME functions — so "vertically garbage" cannot mean one thing
//! in the audit and another in the planner. It is organized as hard rules, soft costs and owned
//! exceptions, never one scalar "quality":
//!
//! - **Owned vs unowned dissonance.** A minor 2nd or minor 9th between two sounding notes is owned
//!   when something accounts for it — a short linear note that resolves by step (a passing or
//!   neighbour collision), a suspension that resolves, a bass pedal, an explicit altered-dominant
//!   package, or a one-player voicing cluster of chord tones. Anything else is an
//!   [`VerticalClass::UnownedCollision`]: garbage, however lawful each note is alone.
//! - **Available is not stable anywhere.** A licensed tension ([`TensionSpec`]) is colour ABOVE the
//!   harmonic floor, owned by one player, voiced so it does not sit a semitone under the chord tone
//!   it neighbours. In the bass it redefines the chord ([`BassFunction::Tension`]).
//! - **Register matters.** The conventional low-interval limits ([`low_interval_limit`]) apply to
//!   every pair, across roles — the bass and the lower pad/keys voices share one acoustic register.
//! - **Colour is a budget, not a menu.** [`ColorPolicy`] bounds the distinct colour tones, the
//!   sounding voices and the pitch classes per world and language: the band picks a small colour
//!   set; it does not sound the union of every legal tension each engine happened to pick. The
//!   budget counts [`is_selected_color`] tones only — the ones the players ADD beyond the written
//!   chord. A written 9th (the G of an Fmaj9) is the song, not a choice the band spent; it is still
//!   an extension for doubling, contradiction and register purposes.
//! - **One core sound.** Where a chordal player (pad/keys) sounds, the BAND must contain the guide
//!   tones (3rd and 7th/6th) — no single player has to.

use std::collections::{BTreeMap, BTreeSet};

use super::context::{context_at, guide_tones, HarmonicContext};
use super::instrument::Patch;
use super::language::{LanguageId, MusicalLanguage};
use super::score::{PitchFunction, Role, Score};
use super::theory::{note_name, pitch_class, Chord, Midi, Quality};
use super::world::{MusicWorld, WorldId};

/// A note shorter than this (beats) that moves by step to a stable tone can own a collision as a
/// linear event (passing, neighbour, approach, appoggiatura, anticipation, slide).
pub const LINEAR_MAX_BEATS: f64 = 0.5;
/// A linear or suspended note must reach its resolution within this many beats of its end.
pub const RESOLVE_WINDOW_BEATS: f64 = 1.0;
/// Two notes must overlap at least this long (beats) for their interval to count as a sounding
/// collision — a release brushing the next onset is contact, not a sonority.
pub const MIN_OVERLAP_BEATS: f64 = 0.125;
/// The chordal cluster exemption: a one-player semitone of chord tones is a voicing colour only at
/// or above this pitch (lower, it is mud).
pub const CLUSTER_FLOOR: Midi = 55;
/// Harmonic memory: a root stated (by anyone) within this many beats, inside the same harmony,
/// still names the chord — a two-feel's fifth on beat 3 is Dm7/A, not Fmaj7.
pub const ROOT_MEMORY_BEATS: f64 = 2.0;

/// A tension's degree over the chord root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Degree {
    Flat9,
    Nine,
    Sharp9,
    Eleven,
    Sharp11,
    Flat13,
    Thirteen,
}

impl Degree {
    /// The degree `pc` names over `chord`, if it is an extension there (a chord tone of the core —
    /// root, 3rd, 5th, 7th/6th — is not).
    pub fn of(chord: &Chord, pc: i32) -> Option<Degree> {
        let iv = (pc - chord.root_pc).rem_euclid(12);
        let core = core_intervals(chord.quality);
        if core.contains(&iv) {
            return None;
        }
        let has_p5 = core.contains(&7);
        match iv {
            1 => Some(Degree::Flat9),
            2 => Some(Degree::Nine),
            3 if core.contains(&4) => Some(Degree::Sharp9),
            5 => Some(Degree::Eleven),
            6 if has_p5 => Some(Degree::Sharp11),
            8 if has_p5 => Some(Degree::Flat13),
            9 => Some(Degree::Thirteen),
            _ => None,
        }
    }

    /// A short label (`b9`, `9`, `#9`, `11`, `#11`, `b13`, `13`).
    pub fn label(self) -> &'static str {
        match self {
            Degree::Flat9 => "b9",
            Degree::Nine => "9",
            Degree::Sharp9 => "#9",
            Degree::Eleven => "11",
            Degree::Sharp11 => "#11",
            Degree::Flat13 => "b13",
            Degree::Thirteen => "13",
        }
    }

    /// A dominant alteration (b9, #9, b13): a tendency tone, not a resting colour.
    pub fn is_altered(self) -> bool {
        matches!(self, Degree::Flat9 | Degree::Sharp9 | Degree::Flat13)
    }
}

/// The core intervals of a quality: root, 3rd (or its sus stand-in), 5th, 7th/6th — everything a
/// quality spells below the 9th. The written 9th of an add9/maj9/9/m9 is COLOUR, not core.
pub fn core_intervals(q: Quality) -> Vec<i32> {
    q.intervals().iter().copied().filter(|&i| i < 12).collect()
}

/// The core pitch classes of `chord` (see [`core_intervals`]).
pub fn core_pcs(chord: &Chord) -> Vec<i32> {
    core_intervals(chord.quality)
        .into_iter()
        .map(|i| (chord.root_pc + i).rem_euclid(12))
        .collect()
}

/// Whether `pc` is SELECTED colour over `chord`: a tone the chord symbol does not spell, so a player
/// chose to add it. The colour budget counts these. A written extension (the 9th of an add9/maj9)
/// is an extension — beyond the core — but not a selection: the harmony already asked for it.
pub fn is_selected_color(chord: &Chord, pc: i32) -> bool {
    !chord.contains_pc(pc)
}

/// The tones that DEFINE `chord`'s identity beyond its root: the guide tones (3rd or its sus
/// stand-in, 7th/6th) plus an altered 5th (the ♭5 of a m7♭5/dim, the ♯5 of an aug) — drop one and
/// the band states a different chord (Bm7♭5 without its F is an Em-ish sound over B).
pub fn identity_pcs(chord: &Chord) -> Vec<i32> {
    let mut v = guide_tones(chord);
    for i in core_intervals(chord.quality) {
        if matches!(i, 6 | 8) {
            let pc = (chord.root_pc + i).rem_euclid(12);
            if !v.contains(&pc) {
                v.push(pc);
            }
        }
    }
    v
}

/// How a tension behaves: a natural extension, a characteristic modal colour, or an alteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TensionClass {
    /// 9, 11 over minor, 13 over dominant/major: resting colour.
    Natural,
    /// A mode's signature colour that sits a semitone under a chord tone (#11 under the 5th over a
    /// Lydian major, 13 under the ♭7 over a Dorian minor): colour only when voiced ABOVE that tone.
    Characteristic,
    /// b9 / #9 / b13 over a dominant: a tendency tone that wants to move.
    Altered,
}

/// What an AVAILABLE tension may do in the band. Available is not "stable anywhere": the spec
/// carries the register, the floor, the owner count and the voicing constraint that make it colour
/// rather than a wrong note.
#[derive(Debug, Clone, PartialEq)]
pub struct TensionSpec {
    pub pc: i32,
    pub degree: Degree,
    pub class: TensionClass,
    /// Lowest pitch at which the tension sustains as colour (below it, it muddies the floor).
    pub min_register: Midi,
    /// Whether it may be the lowest sounding pitch (a slash bass). Never, unless planned.
    pub over_bass_ok: bool,
    /// How many players may own it at once (a planned unison is one owner in several roles).
    pub max_owners: u8,
    /// Whether it is a tendency that must resolve by step rather than rest.
    pub requires_resolution: bool,
    /// Chord tones a semitone ABOVE this tension: the tension must be voiced above them (a major
    /// 7th away) — voiced below, it forms a minor 2nd/9th with them.
    pub voice_above: Vec<i32>,
}

/// The tension specs of every AVAILABLE tension over `ctx` (the palette's licensed tensions and the
/// chord's own written extensions). What the band actually SELECTS is a [`SonorityPlan`]'s business.
pub fn tension_specs(ctx: &HarmonicContext) -> Vec<TensionSpec> {
    let core = core_pcs(&ctx.chord);
    let mut pcs: Vec<i32> = ctx.palette.tensions.clone();
    for pc in ctx.chord.pitch_classes() {
        if !core.contains(&pc) && !pcs.contains(&pc) {
            pcs.push(pc);
        }
    }
    pcs.sort_unstable();
    let dominant = is_dominant(&ctx.chord);
    pcs.into_iter()
        .filter_map(|pc| {
            let degree = Degree::of(&ctx.chord, pc)?;
            let voice_above: Vec<i32> = core
                .iter()
                .copied()
                .filter(|&c| (c - pc).rem_euclid(12) == 1)
                .collect();
            let class = if dominant && degree.is_altered() {
                TensionClass::Altered
            } else if !voice_above.is_empty() {
                TensionClass::Characteristic
            } else {
                TensionClass::Natural
            };
            Some(TensionSpec {
                pc,
                degree,
                class,
                min_register: if class == TensionClass::Altered {
                    60
                } else {
                    55
                },
                over_bass_ok: false,
                max_owners: 1,
                requires_resolution: class == TensionClass::Altered,
                voice_above,
            })
        })
        .collect()
}

/// Whether `chord` is a dominant-function quality (a major 3rd under a minor 7th).
pub fn is_dominant(chord: &Chord) -> bool {
    matches!(chord.quality, Quality::Dom7 | Quality::Dom9)
}

/// The two harsh semitone classes. A major 7th (the inversion) is chord identity, not a clash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Clash {
    /// Adjacent semitone (upper − lower = 1).
    MinorSecond,
    /// A semitone plus an octave (upper − lower = 13).
    MinorNinth,
}

impl Clash {
    /// The clash between two pitches, if their interval is a minor 2nd or minor 9th.
    pub fn of(a: Midi, b: Midi) -> Option<Clash> {
        match (a - b).abs() {
            1 => Some(Clash::MinorSecond),
            13 => Some(Clash::MinorNinth),
            _ => None,
        }
    }

    /// `m2` / `m9`.
    pub fn label(self) -> &'static str {
        match self {
            Clash::MinorSecond => "m2",
            Clash::MinorNinth => "m9",
        }
    }
}

/// The conventional arranging low-interval limits: the lowest pitch the LOWER note of a simple
/// interval (or a 9th) may have before the pair reads as mud. `None` for unisons, octaves and wider
/// compounds. (A register convention, not acoustics — stated as data so it can be argued with.)
pub fn low_interval_limit(interval: i32) -> Option<Midi> {
    match interval {
        1 => Some(52),  // m2: E3
        2 => Some(51),  // M2: E♭3
        3 => Some(48),  // m3: C3
        4 => Some(46),  // M3: B♭2
        5 => Some(46),  // P4: B♭2
        6 => Some(47),  // tritone: B2
        7 => Some(34),  // P5: B♭1
        8 => Some(43),  // m6: G2
        9 => Some(41),  // M6: F2
        10 => Some(41), // m7: F2
        11 => Some(41), // M7: F2
        13 => Some(40), // m9: E2
        14 => Some(39), // M9: E♭2
        _ => None,
    }
}

/// What a pitch does as the ensemble's FLOOR over `chord`. A note's harmonic function is not
/// invariant under register: a 9th in the lead is colour; the same pitch class under the band is a
/// different chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BassFunction {
    Root,
    Fifth,
    /// First inversion (the 3rd, or its sus stand-in, in the bass).
    Third,
    /// Third inversion (the 7th or 6th in the bass).
    Seventh,
    /// An extension in the bass — a slash chord nobody planned, unless a plan says so.
    Tension(Degree),
    /// Not in the chord at all (a passing/approach bass, legal only as a short resolving event).
    NonChord,
}

impl BassFunction {
    /// The floor function of `pitch` over `chord`.
    pub fn of(chord: &Chord, pitch: Midi) -> BassFunction {
        let iv = (pitch_class(pitch) - chord.root_pc).rem_euclid(12);
        let core = core_intervals(chord.quality);
        match iv {
            0 => BassFunction::Root,
            6..=8 if core.contains(&iv) => BassFunction::Fifth,
            2..=5 if core.contains(&iv) => BassFunction::Third,
            9..=11 if core.contains(&iv) => BassFunction::Seventh,
            _ => match Degree::of(chord, pitch_class(pitch)) {
                Some(d) => BassFunction::Tension(d),
                None => BassFunction::NonChord,
            },
        }
    }

    /// Root or fifth: the floor the upper voices assume unless told otherwise.
    pub fn is_foundation(self) -> bool {
        matches!(self, BassFunction::Root | BassFunction::Fifth)
    }

    /// A short label.
    pub fn label(self) -> String {
        match self {
            BassFunction::Root => "root".into(),
            BassFunction::Fifth => "5th".into(),
            BassFunction::Third => "3rd (inversion)".into(),
            BassFunction::Seventh => "7th/6th (inversion)".into(),
            BassFunction::Tension(d) => format!("{} in the bass", d.label()),
            BassFunction::NonChord => "non-chord bass".into(),
        }
    }
}

/// How much colour and density the band may sound at once — per world and language. The palette
/// exposes what is POSSIBLE; this bounds what is SELECTED.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorPolicy {
    /// Most distinct colour pitch classes (extensions beyond the core) sounding at once.
    pub color_budget: usize,
    /// Most pitched voices sounding at once.
    pub max_voices: usize,
    /// Most distinct pitch classes sounding at once.
    pub max_pcs: usize,
    /// Whether an upper-structure triad may be the band's colour.
    pub upper_structures: bool,
    /// Whether a plain triad / add9 must carry colour in its voicing (the old `required_pcs`
    /// forced 3rd + 9th + 5th on every single-guide chord in every world).
    pub triad_color: bool,
}

impl ColorPolicy {
    /// The policy for `world` speaking `language`. SWISS_SIGNAL is restrained, BLACK_ICE is dark
    /// and sparse, VAPOR95 is lush; the plain-speech language halves the colour everywhere.
    pub fn for_world(world: WorldId, language: &MusicalLanguage) -> ColorPolicy {
        let (color, voices, pcs, upper, triad) = match world {
            WorldId::BlackIce => (2, 7, 6, true, false),
            WorldId::Vapor95 => (3, 8, 7, true, true),
            WorldId::SwissSignal => (1, 6, 5, false, false),
        };
        match language.id {
            LanguageId::FusionConversation => ColorPolicy {
                color_budget: color,
                max_voices: voices,
                max_pcs: pcs,
                upper_structures: upper,
                triad_color: triad,
            },
            LanguageId::Simple => ColorPolicy {
                color_budget: color.min(1),
                max_voices: voices - 1,
                max_pcs: pcs - 1,
                upper_structures: false,
                triad_color: false,
            },
        }
    }

    /// A permissive policy for measurement when no world applies (every colour budget unbounded
    /// except the hard rules).
    pub const fn lenient() -> ColorPolicy {
        ColorPolicy {
            color_budget: 12,
            max_voices: 64,
            max_pcs: 12,
            upper_structures: true,
            triad_color: true,
        }
    }
}

/// The band's allocation of one harmony: who holds the floor, who supplies the core, which colours
/// sound and who owns them. Built before the harmony is voiced; read by the joint support path and
/// the realizers; carried into the diagnostics so an explicit alteration package is visible there.
#[derive(Debug, Clone, PartialEq)]
pub struct SonorityPlan {
    /// The context index this plan allocates.
    pub context: usize,
    pub start_beat: f64,
    pub end_beat: f64,
    /// The floor: the pitch class the bass states and what it is over the chord.
    pub bass_pc: i32,
    pub bass_function: BassFunction,
    /// The core guide tones (3rd, 7th/6th) the band must contain.
    pub core: Vec<i32>,
    /// Colour pitch classes selected for this harmony ([`is_selected_color`]), each with its single
    /// owner.
    pub colors: Vec<(i32, Role)>,
    /// Pitch classes the accompaniment deliberately omits (the root when the bass has it, the 5th
    /// under a #11, a colour the lead already sustains).
    pub omit: Vec<i32>,
    /// An explicit alteration package (pitch classes) — the only licence for a b9-over-root m9.
    pub altered: Vec<i32>,
    /// The upper-structure triad the band shares, if the plan chose one (one owner, one triad).
    pub upper_structure: Option<[i32; 3]>,
    pub policy: ColorPolicy,
}

impl SonorityPlan {
    /// The colour room the accompaniment has left: the policy's budget minus the colours the lead
    /// already owns here.
    pub fn color_room(&self) -> usize {
        self.policy.color_budget.saturating_sub(self.colors.len())
    }

    /// A one-line dump.
    pub fn dump(&self) -> String {
        let colors: Vec<String> = self
            .colors
            .iter()
            .map(|(pc, r)| format!("{pc}:{}", r.label()))
            .collect();
        format!(
            "  sonority ctx{} @{:.2}-{:.2}: floor pc{} {}  core {:?}  colours [{}] room {}",
            self.context,
            self.start_beat,
            self.end_beat,
            self.bass_pc,
            self.bass_function.label(),
            self.core,
            colors.join(" "),
            self.color_room()
        )
    }
}

/// Allocate every harmony BEFORE the support players voice it: the floor (the root — no inversion
/// is planned until one is authored), the identity tones the band must contain, and the colours the
/// LEAD already owns there (its resting extensions of a half beat or more — the lead is the first
/// mover and is never re-pitched), under `policy`. A pure function of the contexts and the lead line,
/// computed at realize time (a probe that mutates the performance and re-realizes gets a fresh plan).
pub fn plan_sonority(
    contexts: &[HarmonicContext],
    lead: &[super::score::Note],
    policy: &ColorPolicy,
) -> Vec<SonorityPlan> {
    contexts
        .iter()
        .enumerate()
        .map(|(i, ctx)| {
            let start = ctx.start_beat;
            let end = start + ctx.dur_beats as f64;
            let mut colors: Vec<(i32, Role)> = Vec::new();
            for n in lead.iter().filter(|n| {
                let e = n.start_beat + n.dur_beats as f64;
                n.start_beat >= start - 1e-6
                    && n.start_beat < end - 1e-6
                    && e.min(end) - n.start_beat >= 0.5 - 1e-9
                    && !n.function.is_some_and(|f| is_linear(f) || is_suspension(f))
            }) {
                let pc = pitch_class(n.pitch);
                if is_selected_color(&ctx.chord, pc) && !colors.iter().any(|c| c.0 == pc) {
                    colors.push((pc, Role::Lead));
                }
            }
            colors.sort_by_key(|c| c.0);
            SonorityPlan {
                context: i,
                start_beat: start,
                end_beat: end,
                bass_pc: ctx.chord.root_pc,
                bass_function: BassFunction::Root,
                core: identity_pcs(&ctx.chord),
                colors,
                omit: Vec::new(),
                altered: Vec::new(),
                upper_structure: None,
                policy: *policy,
            }
        })
        .collect()
}

/// A sounding note, as the vertical theory sees it: who, what pitch, when, and why.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Voice {
    pub role: Role,
    pub pitch: Midi,
    pub start: f64,
    pub end: f64,
    pub function: Option<PitchFunction>,
    /// The realizer's tag (`comp`, `pad`, `unison`, `answer`, …).
    pub tag: &'static str,
    /// Whether this note, if linear or suspended, reaches its resolution (the same player moves by
    /// step to a stable tone within [`RESOLVE_WINDOW_BEATS`]). Always true for a stable note.
    pub resolves: bool,
    /// The pitch the same player steps to next (observed), if it does — what a suspension resolves
    /// INTO (it may not sound against that tone), what an alteration owes.
    pub resolves_to: Option<Midi>,
    /// Whether it doubles a planned ensemble unison line (one owner in several roles).
    pub unison: bool,
    /// Where the note is WRITTEN to end (the Score's end), when `end` is an audible end instead — so
    /// a same-player release tail crossfading under its own successor reads as voice-leading.
    pub written_end: f64,
    /// A pitched SFX gesture (it joins the union; its owned dissonance is its owner's).
    pub sfx: bool,
    /// An SFX dissonance owned by a planned action.
    pub owned: bool,
}

impl Voice {
    /// Whether the note is a linear event (passing, neighbour, approach, enclosure, appoggiatura,
    /// anticipation, slide) rather than a resting pitch.
    pub fn is_linear(&self) -> bool {
        self.function.is_some_and(is_linear)
    }

    /// Duration in beats.
    pub fn dur(&self) -> f64 {
        self.end - self.start
    }
}

/// Whether `f` is a linear function (it owns a dissonance by moving, not by resting).
pub fn is_linear(f: PitchFunction) -> bool {
    matches!(
        f,
        PitchFunction::DiatonicPassing
            | PitchFunction::ChromaticPassing
            | PitchFunction::Neighbor
            | PitchFunction::ChromaticApproach
            | PitchFunction::Enclosure
            | PitchFunction::Appoggiatura
            | PitchFunction::Anticipation
            | PitchFunction::SlidePath
    )
}

/// Whether `f` is a suspension-type function (a held tone resolving by step).
pub fn is_suspension(f: PitchFunction) -> bool {
    matches!(f, PitchFunction::Suspension | PitchFunction::Retardation)
}

/// The ensemble-level classification of a vertical relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VerticalClass {
    /// Chord tones only.
    StructuralChord,
    /// Chord tones plus colour within budget; or a one-player voicing cluster of chord tones.
    StableColor,
    /// A dissonance a structure owns (a bass pedal under the harmony).
    OwnedTension,
    /// A short linear note that resolves by step: a passing/neighbour collision.
    LinearCollision,
    /// A held tone resolving by step.
    Suspension,
    /// An explicitly planned alteration package.
    AlteredColor,
    /// One player's release tail crossfading a step under its own successor (voice-leading, heard
    /// only through the envelope — never in the written Score).
    VoiceStep,
    /// Nothing accounts for it.
    UnownedCollision,
}

impl VerticalClass {
    /// A short label.
    pub fn label(self) -> &'static str {
        match self {
            VerticalClass::StructuralChord => "structural",
            VerticalClass::StableColor => "stable-color",
            VerticalClass::OwnedTension => "owned-tension",
            VerticalClass::LinearCollision => "linear",
            VerticalClass::Suspension => "suspension",
            VerticalClass::AlteredColor => "altered",
            VerticalClass::VoiceStep => "voice-step",
            VerticalClass::UnownedCollision => "UNOWNED",
        }
    }
}

/// Who owns the semitone-class clash between `a` and `b` over `ctx` — the vertical question. The
/// order of the exemptions is the order of the argument: motion first (a resolving linear note owns
/// its clash; a resolving suspension owns one — but never against the very tone it resolves into,
/// save the 9-8 over a bass that holds it an octave or more below), then structure (a bass pedal, an
/// SFX gesture's owned dissonance, a planned alteration against the dominant's ROOT that resolves),
/// then the one-player chordal cluster and the one-player release crossfade; otherwise nobody does.
pub fn classify_clash(
    ctx: &HarmonicContext,
    a: &Voice,
    b: &Voice,
    plan: Option<&SonorityPlan>,
) -> VerticalClass {
    let (lo, hi) = if a.pitch <= b.pitch { (a, b) } else { (b, a) };
    let short_linear =
        // Linearity is a property of the WRITTEN note: an audible tail does not make a passing
        // tone long.
        |v: &Voice| v.is_linear() && v.written_end - v.start <= LINEAR_MAX_BEATS + 1e-9 && v.resolves;
    if short_linear(lo) || short_linear(hi) {
        return VerticalClass::LinearCollision;
    }
    let susp = |v: &Voice| v.function.is_some_and(is_suspension);
    if susp(lo) || susp(hi) {
        let (s, other) = if susp(hi) { (hi, lo) } else { (lo, hi) };
        if !s.resolves {
            return VerticalClass::UnownedCollision;
        }
        let against_resolution = s.resolves_to.map(pitch_class) == Some(pitch_class(other.pitch));
        let nine_eight = other.role == Role::Bass && !other.sfx && s.pitch - other.pitch >= 12;
        return if against_resolution && !nine_eight {
            VerticalClass::UnownedCollision
        } else {
            VerticalClass::Suspension
        };
    }
    if lo.role == Role::Bass
        && !lo.sfx
        && (lo.function == Some(PitchFunction::PedalTone) || lo.tag == "pedal")
        && hi.pitch - lo.pitch >= 12
    {
        return VerticalClass::OwnedTension;
    }
    if (lo.sfx && lo.owned) || (hi.sfx && hi.owned) {
        return VerticalClass::OwnedTension;
    }
    if let Some(p) = plan {
        if is_dominant(&ctx.chord) && !p.altered.is_empty() {
            let root = ctx.chord.root_pc;
            let (lpc, hpc) = (pitch_class(lo.pitch), pitch_class(hi.pitch));
            let owned_by = |alt: &Voice, other_pc: i32| {
                p.altered.contains(&pitch_class(alt.pitch))
                    && other_pc == root
                    && alt.resolves_to.is_some()
            };
            if owned_by(hi, lpc) || owned_by(lo, hpc) {
                return VerticalClass::AlteredColor;
            }
        }
    }
    if lo.role == hi.role
        && !lo.sfx
        && !hi.sfx
        && hi.pitch - lo.pitch == 1
        && lo.pitch >= CLUSTER_FLOOR
        && ctx.chord.contains_pc(pitch_class(lo.pitch))
        && ctx.chord.contains_pc(pitch_class(hi.pitch))
    {
        return VerticalClass::StableColor;
    }
    let crossfade = |x: &Voice, y: &Voice| x.written_end <= y.start + 1e-6;
    if lo.role == hi.role
        && !lo.sfx
        && !hi.sfx
        && (hi.pitch - lo.pitch) <= 2
        && (crossfade(lo, hi) || crossfade(hi, lo))
    {
        return VerticalClass::VoiceStep;
    }
    VerticalClass::UnownedCollision
}

/// The audit's verdict on a semitone-class pair: [`classify_clash`], plus the one exemption a
/// synthetic witness proved it wrong about (Round VIIIb) — a minor chord's licensed 9th a semitone
/// under its minor 3rd, in the mid register, sounded by the chordal support (Dm7 voiced C E F A, the
/// rootless "B-form", or the pad's E under the keys' F): that E–F is the voicing's colour, not a
/// collision — whether or not the chord symbol happens to spell the 9 (the same notes over "Dm9"
/// were already stable) and whichever of the pad and the keys holds which half (the ear hears one
/// composite voicing). Not exempt: the 9th directly under the MELODY, the same pair a minor 9th apart,
/// and every other semitone between chord tones (root over major 7th is the classic avoid).
///
/// The Round VIII coupled generator is frozen as the rejected negative control and keeps calling
/// [`classify_clash`] directly; the ruler (every slice this module evaluates) calls this.
pub fn classify_heard(
    ctx: &HarmonicContext,
    a: &Voice,
    b: &Voice,
    plan: Option<&SonorityPlan>,
) -> VerticalClass {
    let (lo, hi) = if a.pitch <= b.pitch { (a, b) } else { (b, a) };
    let above_root = |v: &Voice| (pitch_class(v.pitch) - ctx.chord.root_pc).rem_euclid(12);
    let support = |v: &Voice| matches!(v.role, Role::Pad | Role::Keys);
    if support(lo)
        && support(hi)
        && !lo.sfx
        && !hi.sfx
        && hi.pitch - lo.pitch == 1
        && lo.pitch >= CLUSTER_FLOOR
        && above_root(lo) == 2
        && above_root(hi) == 3
        && ctx.chord.contains_pc(pitch_class(hi.pitch))
        && ctx.palette.is_stable(pitch_class(lo.pitch))
    {
        return VerticalClass::StableColor;
    }
    classify_clash(ctx, a, b, plan)
}

/// One problem in one slice — every one names the actual notes.
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    /// An unowned minor 2nd / minor 9th between two sounding notes (indices into the voice list).
    Unowned { clash: Clash, a: usize, b: usize },
    /// A pair below its low-interval limit.
    LowRegister {
        interval: i32,
        lower: usize,
        upper: usize,
    },
    /// The bass sounds an extension or non-chord tone that nothing owns.
    BassFunction { bass: usize, function: BassFunction },
    /// One colour tone sounded by several players at once (not one planned unison).
    DuplicateTension { pc: i32, owners: Vec<Role> },
    /// More distinct colour tones than the policy's budget.
    OverColor { colors: Vec<i32>, budget: usize },
    /// Two colour tones a semitone apart (9 & b9, 13 & b13, 11 & #11 …).
    Contradiction { a: i32, b: i32 },
    /// An expensive ("avoid") tone resting as if it were stable.
    ExpensiveSustained { voice: usize },
    /// A chordal player sounds, but the band lacks an identity tone (a guide tone, or an altered 5th).
    MissingCore { missing: Vec<i32> },
    /// The bass rests on a non-root while nobody sounds the root — nor has, inside this harmony,
    /// within [`ROOT_MEMORY_BEATS`]: the rootless upper voices plus that floor spell a DIFFERENT
    /// chord (Dm7's shell over an A bass that never stated D reads Fmaj7). Hard when the bass holds
    /// it a beat or more.
    IdentityFlip { bass: usize, held: bool },
    /// More voices sounding than the policy's density limit.
    Crowded { voices: usize, limit: usize },
    /// More distinct pitch classes than the policy's limit.
    TooManyPcs { pcs: usize, limit: usize },
    /// A linear/suspended note (or a planned alteration) that never reaches its resolution.
    Unresolved { voice: usize },
    /// A resting colour tone voiced below its [`TensionSpec::min_register`].
    TensionLow { voice: usize },
}

impl Problem {
    /// Whether this problem is a HARD violation (garbage), as opposed to a soft cost.
    pub fn is_hard(&self) -> bool {
        matches!(
            self,
            Problem::Unowned { .. }
                | Problem::BassFunction { .. }
                | Problem::Unresolved { .. }
                | Problem::IdentityFlip { held: true, .. }
        )
    }
}

/// Every [`Voice`] of a realized Score — its notes and its pitched SFX gestures (for their gated
/// life: attack + decay + hold) — sorted by (start, role, pitch), with each note's observed
/// resolution (what the same player steps to next) and the resolution flag of each linear or
/// suspended note.
pub fn voices_of(score: &Score, contexts: &[HarmonicContext]) -> Vec<Voice> {
    let mut v: Vec<Voice> = score
        .notes
        .iter()
        .map(|n| Voice {
            role: n.role,
            pitch: n.pitch,
            start: n.start_beat,
            end: n.start_beat + n.dur_beats as f64,
            function: n.function,
            tag: n.prov.role_note,
            resolves: true,
            resolves_to: None,
            unison: n.prov.motif_xform == Some("unison") || n.prov.role_note == "unison",
            written_end: n.start_beat + n.dur_beats as f64,
            sfx: false,
            owned: false,
        })
        .collect();
    let bps = score.tempo_bpm.max(1.0) as f64 / 60.0;
    for e in score.sfx.iter().filter(|e| e.is_pitched()) {
        let (a, d, _, _) = e.kind.envelope();
        let end = e.start_beat + (a + d + e.kind.hold_secs()) as f64 * bps;
        for (k, &p) in e.pitches.iter().enumerate() {
            v.push(sfx_voice(
                p,
                e.function[k],
                e.start_beat,
                end,
                e.owned_by.is_some(),
            ));
        }
    }
    v.sort_by(|a, b| {
        a.start
            .total_cmp(&b.start)
            .then(a.role.label().cmp(b.role.label()))
            .then(a.pitch.cmp(&b.pitch))
    });
    settle_resolutions(&mut v, contexts);
    v
}

/// One pitch of a pitched SFX gesture as a vertical voice, sounding `[start, end)` (its gated
/// lifetime). A planned action owns the gesture's DISSONANCE — the pitch that carries no function
/// over the harmony (a Warning's tritone) — never its chord tone: the sting's root against another
/// player's semitone neighbour is an ordinary clash like any other.
pub fn sfx_voice(
    pitch: Midi,
    function: Option<PitchFunction>,
    start: f64,
    end: f64,
    owned_by_action: bool,
) -> Voice {
    Voice {
        role: Role::Lead,
        pitch,
        start,
        end,
        function,
        tag: "sfx",
        resolves: true,
        resolves_to: None,
        unison: false,
        written_end: end,
        sfx: true,
        owned: owned_by_action && function.is_none(),
    }
}

/// The voices of `score` with their AUDIBLE lifetimes at `floor_db` below peak on `world`'s
/// envelopes: a note on its role's patch, a pitched SFX on its own kind's envelope (released after
/// its hold). Linearity and resolution stay judged on the written durations. The ONE audible voice
/// list: the audible measure and the lab's detail listing both read it.
pub fn audible_voices(
    score: &Score,
    contexts: &[HarmonicContext],
    world: &MusicWorld,
    floor_db: f64,
) -> Vec<Voice> {
    let mut v = voices_of(score, contexts);
    // voices_of sorts stably. Match each projected voice to one source note once, preserving
    // duplicate multiplicity without guessing a role-wide voice/string assignment.
    let mut consumed = vec![false; score.notes.len()];
    for x in &mut v {
        let patch = if x.sfx {
            // The SFX voice's own envelope, found by its onset (gestures never share a beat and a
            // pitch across kinds).
            let kind = score
                .sfx
                .iter()
                .find(|e| {
                    e.is_pitched()
                        && (e.start_beat - x.start).abs() < 1e-9
                        && e.pitches.contains(&x.pitch)
                })
                .map(|e| e.kind);
            match kind {
                Some(k) => Patch {
                    adsr: k.envelope(),
                    ..world.lead
                },
                None => continue,
            }
        } else {
            *match x.role {
                Role::Pad => &world.pad,
                Role::Keys => &world.keys,
                Role::Bass => &world.bass,
                Role::Lead => &world.lead,
            }
        };
        if (score.mono_voice || !score.voice_continuity.is_empty()) && !x.sfx {
            let source = score.notes.iter().enumerate().find(|(i, n)| {
                !consumed[*i]
                    && n.role == x.role
                    && n.pitch == x.pitch
                    && n.start_beat == x.start
                    && n.start_beat + f64::from(n.dur_beats) == x.written_end
                    && n.function == x.function
                    && n.prov.role_note == x.tag
            });
            if let Some((i, note)) = source {
                consumed[i] = true;
                x.end = super::voice::effective_audible_end_at(
                    note,
                    &patch,
                    score.tempo_bpm,
                    floor_db,
                    &score.voice_continuity,
                );
                continue;
            }
        }
        x.end = audible_end_at(
            x.start,
            x.written_end - x.start,
            &patch,
            score.tempo_bpm,
            floor_db,
        );
    }
    v
}

/// Fill each voice's observed resolution and the resolution flag of its linear/suspended notes.
pub fn settle_resolutions(v: &mut [Voice], contexts: &[HarmonicContext]) {
    let resolved: Vec<Option<Midi>> = v.iter().map(|x| resolution_of(v, x, contexts)).collect();
    for (x, r) in v.iter_mut().zip(resolved) {
        let needs = x.function.is_some_and(|f| is_linear(f) || is_suspension(f));
        x.resolves = !needs || r.is_some();
        x.resolves_to = r;
    }
}

/// The pitch the same player steps to (1–2 semitones), within [`RESOLVE_WINDOW_BEATS`] of `x`'s
/// written end, if it is stable over the harmony it lands in (a chord tone for the bass) — the
/// observed resolution. SFX gestures never resolve (they are gestures, not lines).
pub fn resolution_of(all: &[Voice], x: &Voice, contexts: &[HarmonicContext]) -> Option<Midi> {
    if x.sfx {
        return None;
    }
    all.iter()
        .find(|y| {
            !y.sfx
                && y.role == x.role
                && y.start > x.start + 1e-6
                && y.start >= x.written_end - 0.26
                && y.start <= x.written_end + RESOLVE_WINDOW_BEATS
                && (1..=2).contains(&(y.pitch - x.pitch).abs())
                && context_at(contexts, y.start).is_some_and(|c| {
                    let pc = pitch_class(y.pitch);
                    if y.role == Role::Bass {
                        c.chord.contains_pc(pc)
                    } else {
                        c.palette.is_stable(pc)
                    }
                })
        })
        .map(|y| y.pitch)
}

/// The default audibility floor (dB below a note's peak) of the audible measure.
pub const AUDIBLE_FLOOR_DB: f64 = 30.0;
/// The masking floor: a release tail this far below its peak is covered by the next harmony's
/// attack. The coupled pad releases early by its tail to this level.
pub const MASKING_FLOOR_DB: f64 = 20.0;

/// The approximate AUDIBLE end (beats) of a note on `patch` at `tempo_bpm` — its ADSR heard down to
/// `floor_db` below its peak. A percussive patch (sustain ≈ 0) falls silent after its decay whatever
/// its written length; a sustaining one rings through its release after the note-off. The
/// envelope moves 40 dB in its decay/release time (`dsp::env::time_to_coef`). Diagnostics and the
/// pad's early release only: the planner never simulates samples.
pub fn audible_end_at(start: f64, dur: f64, patch: &Patch, tempo_bpm: f32, floor_db: f64) -> f64 {
    let spb = 60.0 / tempo_bpm.max(1.0) as f64;
    let (a, d, s, r) = patch.adsr;
    let (a, d, s, r) = (a as f64, d as f64, s as f64, r as f64);
    let nominal = dur * spb;
    let frac = floor_db / 40.0;
    let secs = if s <= 0.02 {
        // Falls below the floor during its decay (or its release, if gated off first).
        (a + frac * d).min(nominal + frac * r)
    } else {
        nominal + release_tail_secs(patch, floor_db)
    };
    start + secs / spb
}

/// [`audible_end_at`] at the default [`AUDIBLE_FLOOR_DB`].
pub fn audible_end(start: f64, dur: f64, patch: &Patch, tempo_bpm: f32) -> f64 {
    audible_end_at(start, dur, patch, tempo_bpm, AUDIBLE_FLOOR_DB)
}

/// Seconds a sustained note on `patch` stays within `floor_db` of its peak after its note-off
/// (released from its sustain level).
pub fn release_tail_secs(patch: &Patch, floor_db: f64) -> f64 {
    let (_, _, s, r) = patch.adsr;
    let (s, r) = (s as f64, r as f64);
    if s <= 0.02 {
        return 0.0;
    }
    r * ((floor_db + 20.0 * s.log10()) / 40.0).max(0.0)
}

/// One analysed slice of the Score: an interval with a constant set of sounding notes.
#[derive(Debug, Clone, PartialEq)]
pub struct Slice {
    pub start: f64,
    pub end: f64,
    /// Indices into the voice list.
    pub sounding: Vec<usize>,
    pub distinct_pcs: usize,
    /// The selected colour sounding ([`is_selected_color`]).
    pub colors: Vec<i32>,
    pub problems: Vec<Problem>,
    pub class: VerticalClass,
}

/// Evaluate one set of simultaneously sounding voices over `ctx`, as heard from beat `at` (the
/// slice's start: the moment the ear's root memory is judged at). `sounding` indexes `voices`.
pub fn evaluate(
    ctx: &HarmonicContext,
    voices: &[Voice],
    sounding: &[usize],
    policy: &ColorPolicy,
    plan: Option<&SonorityPlan>,
    at: f64,
) -> (Vec<Problem>, VerticalClass) {
    let mut problems = Vec::new();
    let mut class = VerticalClass::StructuralChord;
    let specs = tension_specs(ctx);
    let spec_of = |pc: i32| specs.iter().find(|t| t.pc == pc);
    let ctx_end = ctx.start_beat + ctx.dur_beats as f64;
    // How long a voice sounds under THIS harmony (a release tail crossing into it is contact, not
    // a floor, below MIN_OVERLAP_BEATS — the same rule the pair count applies).
    let under = |v: &Voice| v.end.min(ctx_end) - v.start.max(ctx.start_beat);
    let core = core_pcs(&ctx.chord);
    let raise = |c: &mut VerticalClass, to: VerticalClass| {
        if to > *c {
            *c = to;
        }
    };
    // Pairs: clashes and low-register mud.
    for (i, &a) in sounding.iter().enumerate() {
        for &b in &sounding[i + 1..] {
            let (va, vb) = (&voices[a], &voices[b]);
            if va.pitch == vb.pitch {
                continue;
            }
            let (lo, hi) = if va.pitch < vb.pitch { (a, b) } else { (b, a) };
            let iv = voices[hi].pitch - voices[lo].pitch;
            if let Some(clash) = Clash::of(va.pitch, vb.pitch) {
                let c = classify_heard(ctx, va, vb, plan);
                raise(&mut class, c);
                if c == VerticalClass::UnownedCollision {
                    problems.push(Problem::Unowned {
                        clash,
                        a: lo,
                        b: hi,
                    });
                }
            }
            if let Some(limit) = low_interval_limit(iv) {
                let linear = voices[lo].is_linear() || voices[hi].is_linear();
                if voices[lo].pitch < limit && !linear {
                    problems.push(Problem::LowRegister {
                        interval: iv,
                        lower: lo,
                        upper: hi,
                    });
                }
            }
        }
    }
    // The floor: the lowest bass voice that sounds under this harmony for more than contact.
    if let Some(&bass) = sounding
        .iter()
        .filter(|&&i| voices[i].role == Role::Bass && under(&voices[i]) >= MIN_OVERLAP_BEATS - 1e-9)
        .min_by_key(|&&i| voices[i].pitch)
    {
        let v = &voices[bass];
        let f = BassFunction::of(&ctx.chord, v.pitch);
        let owned =
            (v.is_linear() && v.written_end - v.start <= LINEAR_MAX_BEATS + 1e-9 && v.resolves)
                || v.function == Some(PitchFunction::PedalTone)
                || v.tag == "pedal"
                || plan.is_some_and(|p| p.bass_pc == pitch_class(v.pitch))
                || spec_of(pitch_class(v.pitch)).is_some_and(|t| t.over_bass_ok);
        if matches!(f, BassFunction::Tension(_) | BassFunction::NonChord) && !owned {
            problems.push(Problem::BassFunction { bass, function: f });
        }
        // The identity: a non-root floor under a band that has no root anywhere — and has not had
        // one, in this harmony, within the ear's memory.
        let now = at.max(ctx.start_beat);
        let root_sounds = voices.iter().any(|x| {
            pitch_class(x.pitch) == ctx.chord.root_pc
                && !x.is_linear()
                && x.start < now + 1e-6
                && x.end > (now - ROOT_MEMORY_BEATS).max(ctx.start_beat) + 1e-6
        }) || sounding
            .iter()
            .any(|&i| pitch_class(voices[i].pitch) == ctx.chord.root_pc && !voices[i].is_linear());
        let chordal = sounding
            .iter()
            .any(|&i| matches!(voices[i].role, Role::Pad | Role::Keys));
        if f != BassFunction::Root && !root_sounds && chordal && !v.is_linear() {
            let planned = plan.is_some_and(|p| p.bass_pc == pitch_class(v.pitch));
            let pedal = v.function == Some(PitchFunction::PedalTone) || v.tag == "pedal";
            if !planned && !pedal {
                problems.push(Problem::IdentityFlip {
                    bass,
                    held: under(v) >= 1.0 - 1e-9,
                });
            }
        }
    }
    // Extensions: the resting (non-linear) tones beyond the core. SFX gestures are not the band's
    // colour owners (they join the clash and density checks only). Doubling, contradiction and
    // expense are judged over every extension; the BUDGET over selected colour only.
    let resting: Vec<usize> = sounding
        .iter()
        .copied()
        .filter(|&i| {
            !voices[i].sfx
                && !voices[i].is_linear()
                && !voices[i].function.is_some_and(is_suspension)
        })
        .collect();
    let mut extensions: Vec<i32> = resting
        .iter()
        .map(|&i| pitch_class(voices[i].pitch))
        .filter(|pc| !core.contains(pc))
        .collect();
    extensions.sort_unstable();
    extensions.dedup();
    if !extensions.is_empty() {
        raise(&mut class, VerticalClass::StableColor);
    }
    for &pc in &extensions {
        let mut owners: Vec<Role> = Vec::new();
        // A planned unison is one owner in several roles: every non-lead owner doubles the line
        // (the lead is the unison's source; its own notes carry no unison tag).
        let mut unison_only = true;
        let mut any_unison = false;
        for &i in &resting {
            if pitch_class(voices[i].pitch) == pc && !owners.contains(&voices[i].role) {
                owners.push(voices[i].role);
                unison_only &= voices[i].unison || voices[i].role == Role::Lead;
                any_unison |= voices[i].unison;
            }
        }
        let limit = spec_of(pc).map_or(1, |t| t.max_owners as usize);
        if owners.len() > limit && !(unison_only && any_unison) {
            problems.push(Problem::DuplicateTension { pc, owners });
        }
        if ctx.palette.expensive.contains(&pc) {
            for &i in resting
                .iter()
                .filter(|&&i| pitch_class(voices[i].pitch) == pc)
            {
                problems.push(Problem::ExpensiveSustained { voice: i });
            }
        }
    }
    // Available is not stable anywhere: a resting colour below its spec's register muddies the floor.
    for &i in &resting {
        let pc = pitch_class(voices[i].pitch);
        if voices[i].role != Role::Bass
            && specs
                .iter()
                .any(|t| t.pc == pc && voices[i].pitch < t.min_register)
        {
            problems.push(Problem::TensionLow { voice: i });
        }
    }
    // A planned alteration is a tendency: sounded, it owes its step.
    if let Some(p) = plan {
        for &i in sounding {
            if !voices[i].sfx
                && p.altered.contains(&pitch_class(voices[i].pitch))
                && voices[i].resolves_to.is_none()
            {
                problems.push(Problem::Unresolved { voice: i });
            }
        }
    }
    let altered_ok = |x: i32| plan.is_some_and(|p| p.altered.contains(&x));
    for (i, &x) in extensions.iter().enumerate() {
        for &y in &extensions[i + 1..] {
            let d = (y - x).rem_euclid(12);
            if (d == 1 || d == 11) && !(altered_ok(x) && altered_ok(y)) {
                problems.push(Problem::Contradiction { a: x, b: y });
            }
        }
    }
    let colors: Vec<i32> = extensions
        .iter()
        .copied()
        .filter(|&pc| is_selected_color(&ctx.chord, pc))
        .collect();
    if colors.len() > policy.color_budget {
        problems.push(Problem::OverColor {
            colors: colors.clone(),
            budget: policy.color_budget,
        });
    }
    // The core sound: where a chordal player sounds, the band carries the guide tones.
    if sounding
        .iter()
        .any(|&i| matches!(voices[i].role, Role::Pad | Role::Keys))
    {
        let have: BTreeSet<i32> = resting
            .iter()
            .map(|&i| pitch_class(voices[i].pitch))
            .collect();
        let missing: Vec<i32> = identity_pcs(&ctx.chord)
            .into_iter()
            .filter(|g| !have.contains(g))
            .collect();
        if !missing.is_empty() {
            problems.push(Problem::MissingCore { missing });
        }
    }
    // Density.
    if sounding.len() > policy.max_voices {
        problems.push(Problem::Crowded {
            voices: sounding.len(),
            limit: policy.max_voices,
        });
    }
    let pcs: BTreeSet<i32> = sounding
        .iter()
        .map(|&i| pitch_class(voices[i].pitch))
        .collect();
    if pcs.len() > policy.max_pcs {
        problems.push(Problem::TooManyPcs {
            pcs: pcs.len(),
            limit: policy.max_pcs,
        });
    }
    (problems, class)
}

/// The Score cut into slices: every note onset, note end and chord boundary starts a new one.
pub fn slices(
    voices: &[Voice],
    contexts: &[HarmonicContext],
    policy: &ColorPolicy,
    plans: &[SonorityPlan],
) -> Vec<Slice> {
    let mut cuts: Vec<f64> = voices
        .iter()
        .flat_map(|v| [v.start, v.end])
        .chain(contexts.iter().map(|c| c.start_beat))
        .collect();
    // Where the ear's memory of the root runs out under a non-root floor that is still held (and
    // no root sounds), cut: the flip starts there, not at the next note boundary.
    for x in voices.iter().filter(|x| !x.is_linear()) {
        let t = x.end + ROOT_MEMORY_BEATS;
        let Some(c) = context_at(contexts, x.end - 1e-6) else {
            continue;
        };
        if pitch_class(x.pitch) != c.chord.root_pc || t >= c.start_beat + c.dur_beats as f64 - 1e-6
        {
            continue;
        }
        let across = |y: &Voice| y.start < t - 1e-6 && y.end > t + 1e-6;
        let floor_held = voices.iter().any(|b| {
            b.role == Role::Bass
                && across(b)
                && BassFunction::of(&c.chord, b.pitch) != BassFunction::Root
        });
        let root_on = voices.iter().any(|y| {
            pitch_class(y.pitch) == c.chord.root_pc
                && !y.is_linear()
                && y.start <= t + 1e-6
                && y.end > t + 1e-6
        });
        if floor_held && !root_on {
            cuts.push(t);
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let mut out = Vec::new();
    for w in cuts.windows(2) {
        let (s, e) = (w[0], w[1]);
        if e - s < 1e-6 {
            continue;
        }
        let sounding: Vec<usize> = voices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.start <= s + 1e-6 && v.end > s + 1e-6)
            .map(|(i, _)| i)
            .collect();
        if sounding.is_empty() {
            continue;
        }
        let Some(ctx) = context_at(contexts, s) else {
            continue;
        };
        let ci = contexts
            .iter()
            .rposition(|c| c.start_beat <= s + 1e-6)
            .unwrap_or(0);
        let plan = plans.iter().find(|p| p.context == ci);
        let (problems, class) = evaluate(ctx, voices, &sounding, policy, plan, s);
        let pcs: BTreeSet<i32> = sounding
            .iter()
            .map(|&i| pitch_class(voices[i].pitch))
            .collect();
        let mut colors: Vec<i32> = pcs
            .iter()
            .copied()
            .filter(|&p| is_selected_color(&ctx.chord, p))
            .collect();
        colors.sort_unstable();
        out.push(Slice {
            start: s,
            end: e,
            sounding,
            distinct_pcs: pcs.len(),
            colors,
            problems,
            class,
        });
    }
    out
}

/// Measured vertical coherence of a realized Score — the union the band actually sounds, slice by
/// slice. Counts of note PAIRS (a collision that lasts three slices is one collision) and of
/// affected slices, weighted by duration where it says "beats".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnsembleSonorityDiagnostics {
    /// Slices with at least one sounding pitched note.
    pub slices: usize,
    pub max_active: usize,
    pub max_distinct_pcs: usize,
    /// Duration-weighted mean distinct pitch classes.
    pub mean_distinct_pcs: f32,
    /// Unique unowned minor-2nd / minor-9th note pairs.
    pub unowned_m2: usize,
    pub unowned_m9: usize,
    /// Beats during which at least one unowned collision sounds.
    pub unowned_beats: f64,
    /// Owned clashes by class (unique pairs).
    pub owned: BTreeMap<&'static str, usize>,
    /// Unique low-register pairs.
    pub low_register: usize,
    /// Unique bass notes whose floor function nothing owns (a tension or non-chord tone).
    pub bass_function_violations: usize,
    /// Every bass note that is not a root: (beat, pitch, chord, floor function, its tag/label).
    pub non_root_bass: Vec<(f64, Midi, String, String, String)>,
    /// Slices where one colour tone is owned by several players.
    pub duplicate_tension_slices: usize,
    /// Slices over the colour budget.
    pub over_color_slices: usize,
    /// Slices with two colours a semitone apart.
    pub contradiction_slices: usize,
    /// Unique notes resting on an expensive tone.
    pub expensive_sustained: usize,
    /// Slices (a chordal player sounding) missing a guide tone; and their beats.
    pub missing_core_slices: usize,
    pub missing_core_beats: f64,
    /// Linear/suspended notes that never resolve (unique).
    pub unresolved: usize,
    /// Unique resting colour notes voiced below their tension's register floor.
    pub tension_low: usize,
    /// Unique cross-role MAJOR 2nds / 9ths among resting upper voices — soft: colour, but a smear
    /// of them is what a fix that merely turns every m2 into an M2 would leave behind.
    pub cross_role_seconds: usize,
    /// Unique bass notes whose floor flips the chord's identity (rootless band, non-root bass);
    /// `identity_flips_held` of them held a beat or more (hard). And the beats it lasts.
    pub identity_flips: usize,
    pub identity_flips_held: usize,
    pub identity_flip_beats: f64,
    /// Slices over the voice / pitch-class density limits.
    pub crowded_slices: usize,
    pub too_many_pcs_slices: usize,
    /// Unique unowned collision pairs by role pair (`bass/keys`, `keys/lead`, …).
    pub role_pairs: BTreeMap<String, usize>,
    /// The worst slices, fully spelled out.
    pub worst: Vec<String>,
    /// The policy measured against.
    pub policy: Option<ColorPolicy>,
}

fn pair_key(a: &Voice, b: &Voice) -> String {
    let name = |v: &Voice| if v.sfx { "sfx" } else { v.role.label() };
    let (x, y) = if name(a) <= name(b) {
        (name(a), name(b))
    } else {
        (name(b), name(a))
    };
    format!("{x}/{y}")
}

impl EnsembleSonorityDiagnostics {
    /// Measure `score` over `contexts` under `policy` (nominal Score durations).
    pub fn measure(
        score: &Score,
        contexts: &[HarmonicContext],
        policy: &ColorPolicy,
        plans: &[SonorityPlan],
    ) -> EnsembleSonorityDiagnostics {
        let voices = voices_of(score, contexts);
        EnsembleSonorityDiagnostics::measure_voices(&voices, contexts, policy, plans)
    }

    /// Measure with each note's AUDIBLE lifetime on `world`'s patches ([`audible_end`]) instead of
    /// its written one — the pad tail ringing under the next harmony, the percussive stab that is
    /// already silent.
    pub fn measure_audible(
        score: &Score,
        contexts: &[HarmonicContext],
        world: &MusicWorld,
        policy: &ColorPolicy,
        plans: &[SonorityPlan],
    ) -> EnsembleSonorityDiagnostics {
        EnsembleSonorityDiagnostics::measure_audible_at(
            score,
            contexts,
            world,
            policy,
            plans,
            AUDIBLE_FLOOR_DB,
        )
    }

    /// [`Self::measure_audible`] with an explicit audibility floor (dB below each note's peak).
    pub fn measure_audible_at(
        score: &Score,
        contexts: &[HarmonicContext],
        world: &MusicWorld,
        policy: &ColorPolicy,
        plans: &[SonorityPlan],
        floor_db: f64,
    ) -> EnsembleSonorityDiagnostics {
        let voices = audible_voices(score, contexts, world, floor_db);
        EnsembleSonorityDiagnostics::measure_voices(&voices, contexts, policy, plans)
    }

    /// Measure an explicit voice list (the synthetic controls build one by hand).
    pub fn measure_voices(
        voices: &[Voice],
        contexts: &[HarmonicContext],
        policy: &ColorPolicy,
        plans: &[SonorityPlan],
    ) -> EnsembleSonorityDiagnostics {
        let sl = slices(voices, contexts, policy, plans);
        let mut d = EnsembleSonorityDiagnostics {
            slices: sl.len(),
            policy: Some(*policy),
            ..Default::default()
        };
        let mut total = 0.0f64;
        let mut weighted = 0.0f64;
        let mut unowned: BTreeSet<(usize, usize, Clash)> = BTreeSet::new();
        let mut owned: BTreeSet<(usize, usize, VerticalClass)> = BTreeSet::new();
        let mut low: BTreeSet<(usize, usize)> = BTreeSet::new();
        let mut bassv: BTreeSet<usize> = BTreeSet::new();
        let mut expensive: BTreeSet<usize> = BTreeSet::new();
        let mut unresolved: BTreeSet<usize> = BTreeSet::new();
        let mut flips: BTreeSet<(usize, bool)> = BTreeSet::new();
        let mut low_tension: BTreeSet<usize> = BTreeSet::new();
        for s in &sl {
            let dur = s.end - s.start;
            total += dur;
            weighted += dur * s.distinct_pcs as f64;
            d.max_active = d.max_active.max(s.sounding.len());
            d.max_distinct_pcs = d.max_distinct_pcs.max(s.distinct_pcs);
            let mut any_unowned = false;
            let mut any_flip = false;
            let mut flags = [false; 6];
            // Owned clashes, recomputed for their class tally.
            if let Some(ctx) = context_at(contexts, s.start) {
                let ci = contexts
                    .iter()
                    .rposition(|c| c.start_beat <= s.start + 1e-6)
                    .unwrap_or(0);
                let plan = plans.iter().find(|p| p.context == ci);
                for (i, &a) in s.sounding.iter().enumerate() {
                    for &b in &s.sounding[i + 1..] {
                        if Clash::of(voices[a].pitch, voices[b].pitch).is_some() {
                            let c = classify_heard(ctx, &voices[a], &voices[b], plan);
                            if c != VerticalClass::UnownedCollision {
                                owned.insert((a.min(b), a.max(b), c));
                            }
                        }
                    }
                }
            }
            for p in &s.problems {
                match p {
                    Problem::Unowned { clash, a, b } => {
                        let overlap = voices[*a].end.min(voices[*b].end)
                            - voices[*a].start.max(voices[*b].start);
                        if overlap >= MIN_OVERLAP_BEATS - 1e-9 {
                            any_unowned = true;
                            if unowned.insert((*a, *b, *clash)) {
                                *d.role_pairs
                                    .entry(pair_key(&voices[*a], &voices[*b]))
                                    .or_default() += 1;
                            }
                        }
                    }
                    Problem::LowRegister { lower, upper, .. } => {
                        low.insert((*lower, *upper));
                    }
                    Problem::BassFunction { bass, .. } => {
                        bassv.insert(*bass);
                    }
                    Problem::DuplicateTension { .. } => flags[0] = true,
                    Problem::OverColor { .. } => flags[1] = true,
                    Problem::Contradiction { .. } => flags[2] = true,
                    Problem::ExpensiveSustained { voice } => {
                        expensive.insert(*voice);
                    }
                    Problem::MissingCore { .. } => flags[3] = true,
                    Problem::Crowded { .. } => flags[4] = true,
                    Problem::TooManyPcs { .. } => flags[5] = true,
                    Problem::Unresolved { voice } => {
                        unresolved.insert(*voice);
                    }
                    Problem::IdentityFlip { bass, held } => {
                        flips.insert((*bass, *held));
                        any_flip = true;
                    }
                    Problem::TensionLow { voice } => {
                        low_tension.insert(*voice);
                    }
                }
            }
            if any_flip {
                d.identity_flip_beats += dur;
            }
            if any_unowned {
                d.unowned_beats += dur;
            }
            d.duplicate_tension_slices += flags[0] as usize;
            d.over_color_slices += flags[1] as usize;
            d.contradiction_slices += flags[2] as usize;
            if flags[3] && dur >= 0.25 - 1e-9 {
                d.missing_core_slices += 1;
                d.missing_core_beats += dur;
            }
            d.crowded_slices += flags[4] as usize;
            d.too_many_pcs_slices += flags[5] as usize;
        }
        for (i, v) in voices.iter().enumerate() {
            let needs = v.function.is_some_and(|f| is_linear(f) || is_suspension(f));
            if needs && !v.resolves {
                unresolved.insert(i);
            }
        }
        d.mean_distinct_pcs = if total > 0.0 {
            (weighted / total) as f32
        } else {
            0.0
        };
        d.unowned_m2 = unowned.iter().filter(|x| x.2 == Clash::MinorSecond).count();
        d.unowned_m9 = unowned.iter().filter(|x| x.2 == Clash::MinorNinth).count();
        for (_, _, c) in &owned {
            *d.owned.entry(c.label()).or_default() += 1;
        }
        d.low_register = low.len();
        d.bass_function_violations = bassv.len();
        d.expensive_sustained = expensive.len();
        d.unresolved = unresolved.len();
        d.identity_flips = flips.len();
        d.tension_low = low_tension.len();
        let mut seconds: BTreeSet<(usize, usize)> = BTreeSet::new();
        for s in &sl {
            for (i, &a) in s.sounding.iter().enumerate() {
                for &b in &s.sounding[i + 1..] {
                    let (va, vb) = (&voices[a], &voices[b]);
                    let resting = |v: &Voice| {
                        !v.sfx
                            && !v.is_linear()
                            && v.role != Role::Bass
                            && !v.function.is_some_and(is_suspension)
                    };
                    let overlap = va.end.min(vb.end) - va.start.max(vb.start);
                    if va.role != vb.role
                        && resting(va)
                        && resting(vb)
                        && matches!((va.pitch - vb.pitch).abs(), 2 | 14)
                        && overlap >= MIN_OVERLAP_BEATS - 1e-9
                    {
                        seconds.insert((a.min(b), a.max(b)));
                    }
                }
            }
        }
        d.cross_role_seconds = seconds.len();
        d.identity_flips_held = flips.iter().filter(|x| x.1).count();
        for v in voices.iter().filter(|v| v.role == Role::Bass) {
            let Some(ctx) = context_at(contexts, v.start) else {
                continue;
            };
            let f = BassFunction::of(&ctx.chord, v.pitch);
            if f != BassFunction::Root {
                d.non_root_bass.push((
                    v.start,
                    v.pitch,
                    ctx.chord.label(),
                    f.label(),
                    format!(
                        "{}/{}",
                        v.tag,
                        v.function.map(|x| x.label()).unwrap_or("NONE")
                    ),
                ));
            }
        }
        // The worst slices: most hard problems, then most problems, then the longest.
        let mut ranked: Vec<&Slice> = sl.iter().filter(|s| !s.problems.is_empty()).collect();
        ranked.sort_by(|a, b| {
            let hard = |s: &Slice| s.problems.iter().filter(|p| p.is_hard()).count();
            hard(b)
                .cmp(&hard(a))
                .then(b.problems.len().cmp(&a.problems.len()))
                .then((b.end - b.start).total_cmp(&(a.end - a.start)))
                .then(a.start.total_cmp(&b.start))
        });
        d.worst = ranked
            .iter()
            .take(8)
            .map(|s| describe(s, voices, contexts))
            .collect();
        d
    }

    /// Unique hard problems (unowned collisions + unowned bass functions + unresolved notes + held
    /// identity flips).
    pub fn hard_total(&self) -> usize {
        self.unowned_m2
            + self.unowned_m9
            + self.bass_function_violations
            + self.unresolved
            + self.identity_flips_held
    }

    /// A human-readable report. Measurements of the realized Score, with the offending notes.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "ensemble sonority (the UNION the band sounds, slice by slice — not per-note legality):"
        );
        if let Some(p) = self.policy {
            let _ = writeln!(
                s,
                "  policy: colour_budget={} max_voices={} max_pcs={} upper_structures={} triad_colour={}",
                p.color_budget, p.max_voices, p.max_pcs, p.upper_structures, p.triad_color
            );
        }
        let _ = writeln!(
            s,
            "  slices={} max_active={} max_pcs={} mean_pcs={:.2}",
            self.slices, self.max_active, self.max_distinct_pcs, self.mean_distinct_pcs
        );
        let owned: Vec<String> = self.owned.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let _ = writeln!(
            s,
            "  UNOWNED m2={} m9={} ({:.2} beats)  owned[{}]  low_register={}  bass_function={}  unresolved={}  identity_flips={} (held {}, {:.2} beats)",
            self.unowned_m2,
            self.unowned_m9,
            self.unowned_beats,
            owned.join(" "),
            self.low_register,
            self.bass_function_violations,
            self.unresolved,
            self.identity_flips,
            self.identity_flips_held,
            self.identity_flip_beats,
        );
        let _ = writeln!(
            s,
            "  duplicate_tension_slices={} over_colour_slices={} contradiction_slices={} expensive_sustained={} tension_low={} cross_role_seconds={} missing_core_slices={} ({:.2} beats) crowded={} too_many_pcs={}",
            self.duplicate_tension_slices,
            self.over_color_slices,
            self.contradiction_slices,
            self.expensive_sustained,
            self.tension_low,
            self.cross_role_seconds,
            self.missing_core_slices,
            self.missing_core_beats,
            self.crowded_slices,
            self.too_many_pcs_slices,
        );
        let pairs: Vec<String> = [
            "bass/keys",
            "bass/pad",
            "bass/lead",
            "keys/pad",
            "keys/lead",
            "lead/pad",
        ]
        .iter()
        .map(|k| format!("{k}={}", self.role_pairs.get(*k).copied().unwrap_or(0)))
        .chain(
            [
                "bass/bass",
                "keys/keys",
                "lead/lead",
                "pad/pad",
                "bass/sfx",
                "keys/sfx",
                "lead/sfx",
                "pad/sfx",
            ]
            .iter()
            .filter_map(|k| self.role_pairs.get(*k).map(|n| format!("{k}={n}"))),
        )
        .collect();
        let _ = writeln!(s, "  unowned by role pair: {}", pairs.join(" "));
        let _ = writeln!(s, "  non-root bass events: {}", self.non_root_bass.len());
        if !self.non_root_bass.is_empty() {
            // Why each non-root floor is there: its function over the chord, and the bass line's
            // own reason (tag/pitch function) — the justification, not just the count.
            let mut by_function: BTreeMap<&str, usize> = BTreeMap::new();
            let mut by_reason: BTreeMap<&str, usize> = BTreeMap::new();
            for (_, _, _, f, why) in &self.non_root_bass {
                *by_function.entry(f.as_str()).or_default() += 1;
                *by_reason.entry(why.as_str()).or_default() += 1;
            }
            let join = |m: &BTreeMap<&str, usize>| {
                m.iter()
                    .map(|(k, n)| format!("{k}={n}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let _ = writeln!(s, "    by floor function: {}", join(&by_function));
            let _ = writeln!(s, "    by reason (tag/function): {}", join(&by_reason));
        }
        for w in &self.worst {
            for line in w.lines() {
                let _ = writeln!(s, "    {line}");
            }
        }
        s
    }
}

/// A slice spelled out: beat, chord, each role's pitches, the problems with the actual notes.
pub fn describe(s: &Slice, voices: &[Voice], contexts: &[HarmonicContext]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let chord = context_at(contexts, s.start)
        .map(|c| c.chord.label())
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "beat {:.2}-{:.2} / {chord}  [{}; {} voices, {} pcs]",
        s.start,
        s.end,
        s.class.label(),
        s.sounding.len(),
        s.distinct_pcs
    );
    for (label, role, sfx) in [
        ("bass", Role::Bass, false),
        ("pad", Role::Pad, false),
        ("keys", Role::Keys, false),
        ("lead", Role::Lead, false),
        ("sfx", Role::Lead, true),
    ] {
        let names: Vec<String> = s
            .sounding
            .iter()
            .filter(|&&i| voices[i].role == role && voices[i].sfx == sfx)
            .map(|&i| {
                let v = &voices[i];
                format!(
                    "{}({},{})",
                    note_name(v.pitch),
                    v.function.map(|f| f.label()).unwrap_or("NONE"),
                    v.tag
                )
            })
            .collect();
        if !names.is_empty() {
            let _ = writeln!(out, "  {:5} {}", format!("{label}:"), names.join(" "));
        }
    }
    let nm = |i: usize| {
        format!(
            "{} {}",
            if voices[i].sfx {
                "sfx"
            } else {
                voices[i].role.label()
            },
            note_name(voices[i].pitch)
        )
    };
    for p in &s.problems {
        let line = match p {
            Problem::Unowned { clash, a, b } => {
                format!("{} against {} = unowned {}", nm(*a), nm(*b), clash.label())
            }
            Problem::LowRegister {
                interval,
                lower,
                upper,
            } => format!(
                "{} under {} = {interval} semitones below the low-interval limit",
                nm(*lower),
                nm(*upper)
            ),
            Problem::BassFunction { bass, function } => {
                format!("{} = {} (unowned floor)", nm(*bass), function.label())
            }
            Problem::DuplicateTension { pc, owners } => format!(
                "colour pc {pc} owned by {} players ({})",
                owners.len(),
                owners
                    .iter()
                    .map(|r| r.label())
                    .collect::<Vec<_>>()
                    .join("+")
            ),
            Problem::OverColor { colors, budget } => {
                format!("{} colour tones {colors:?} > budget {budget}", colors.len())
            }
            Problem::Contradiction { a, b } => {
                format!("colours pc {a} and pc {b} a semitone apart")
            }
            Problem::ExpensiveSustained { voice } => {
                format!("{} rests on an expensive (avoid) tone", nm(*voice))
            }
            Problem::MissingCore { missing } => format!("band lacks guide tone(s) pc {missing:?}"),
            Problem::Crowded { voices: n, limit } => format!("{n} sounding voices > {limit}"),
            Problem::TooManyPcs { pcs, limit } => format!("{pcs} distinct pcs > {limit}"),
            Problem::Unresolved { voice } => format!("{} never resolves", nm(*voice)),
            Problem::TensionLow { voice } => format!(
                "{} is colour below its register floor (muddies the foundation)",
                nm(*voice)
            ),
            Problem::IdentityFlip { bass, held } => format!(
                "{} under a rootless band = a different chord{}",
                nm(*bass),
                if *held { " (held)" } else { "" }
            ),
        };
        let _ = writeln!(out, "  problem: {line}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::context::analyze;
    use super::super::harmony::ChordSpan;
    use super::super::theory::{Mode, Scale};
    use super::*;

    const CMAJ7: Chord = Chord {
        root_pc: 0,
        quality: Quality::Maj7,
    };

    fn ctx_of(chords: &[(f64, f32, Chord)]) -> Vec<HarmonicContext> {
        let spans: Vec<ChordSpan> = chords
            .iter()
            .map(|&(s, d, c)| ChordSpan::test(s, d, c))
            .collect();
        analyze(&spans, &Scale::new(0, Mode::Ionian))
    }

    fn v(
        role: Role,
        pitch: Midi,
        start: f64,
        end: f64,
        f: PitchFunction,
        tag: &'static str,
    ) -> Voice {
        Voice {
            role,
            pitch,
            start,
            end,
            function: Some(f),
            tag,
            resolves: true,
            resolves_to: None,
            unison: false,
            written_end: end,
            sfx: false,
            owned: false,
        }
    }

    fn measure(voices: Vec<Voice>, ctxs: &[HarmonicContext]) -> EnsembleSonorityDiagnostics {
        EnsembleSonorityDiagnostics::measure_voices(&voices, ctxs, &ColorPolicy::lenient(), &[])
    }

    #[test]
    fn degrees_and_core() {
        assert_eq!(Degree::of(&CMAJ7, 2), Some(Degree::Nine));
        assert_eq!(Degree::of(&CMAJ7, 6), Some(Degree::Sharp11));
        assert_eq!(Degree::of(&CMAJ7, 9), Some(Degree::Thirteen));
        assert_eq!(Degree::of(&CMAJ7, 4), None, "the 3rd is core");
        let add9 = Chord::new(0, Quality::Add9);
        assert_eq!(
            core_pcs(&add9),
            vec![0, 4, 7],
            "the written 9th is colour, not core"
        );
        assert_eq!(Degree::of(&add9, 2), Some(Degree::Nine));
        assert_eq!(
            BassFunction::of(&CMAJ7, 38),
            BassFunction::Tension(Degree::Nine),
            "D under Cmaj7 is a 9th in the bass, not a licensed extension"
        );
        assert_eq!(BassFunction::of(&CMAJ7, 40), BassFunction::Third);
        assert_eq!(BassFunction::of(&CMAJ7, 43), BassFunction::Fifth);
        assert_eq!(BassFunction::of(&CMAJ7, 47), BassFunction::Seventh);
    }

    #[test]
    fn tension_specs_know_where_a_tension_may_sit() {
        let c = ctx_of(&[(0.0, 4.0, CMAJ7)]);
        let specs = tension_specs(&c[0]);
        let nine = specs
            .iter()
            .find(|s| s.degree == Degree::Nine)
            .expect("9 available");
        assert!(!nine.over_bass_ok && nine.max_owners == 1);
        assert_eq!((nine.class, nine.min_register), (TensionClass::Natural, 55));
        assert!(nine.voice_above.is_empty() && !nine.requires_resolution);
        // Dm7 (Dorian in C): the 13th (B) sits a semitone under the b7 (C) — the mode's
        // characteristic colour, only colour when voiced ABOVE the C.
        let dm7 = ctx_of(&[(0.0, 4.0, Chord::new(2, Quality::Min7))]);
        let dspecs = tension_specs(&dm7[0]);
        let thirteen = dspecs
            .iter()
            .find(|s| s.degree == Degree::Thirteen)
            .expect("the Dorian 13 is available over Dm7");
        assert_eq!(thirteen.pc, 11);
        assert_eq!(thirteen.class, TensionClass::Characteristic);
        assert_eq!(thirteen.voice_above, vec![0]);
        // And the 9th (E) sits a semitone under the minor 3rd (F): voiced below it, E-F is a m2.
        let dnine = dspecs.iter().find(|s| s.degree == Degree::Nine).unwrap();
        assert_eq!(dnine.voice_above, vec![5]);
    }

    #[test]
    fn individually_legal_collectively_garbage_fails_the_union() {
        // Cmaj7, every note individually lawful: bass root, pad a valid E4-G4-B4 voicing, keys a
        // chord tone plus two licensed tensions (C5 D5 A5), lead another licensed tension (the 9th,
        // D6). The per-note audit (function != None) passes every one; the union holds the pad's B4
        // against the keys' C5 — a cross-role minor 2nd nobody owns.
        let c = ctx_of(&[(0.0, 4.0, CMAJ7)]);
        let voices = vec![
            v(Role::Bass, 36, 0.0, 4.0, PitchFunction::ChordTone, "root"),
            v(Role::Pad, 64, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(Role::Pad, 67, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(Role::Pad, 71, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(Role::Keys, 72, 0.0, 2.0, PitchFunction::ChordTone, "comp"),
            v(
                Role::Keys,
                74,
                0.0,
                2.0,
                PitchFunction::LicensedExtension,
                "comp",
            ),
            v(
                Role::Keys,
                81,
                0.0,
                2.0,
                PitchFunction::LicensedExtension,
                "comp",
            ),
            // The lead takes another licensed tension (the 9th, D6).
            v(
                Role::Lead,
                86,
                0.0,
                2.0,
                PitchFunction::LicensedExtension,
                "melody",
            ),
        ];
        // The per-note audit passes: every note carries a function (R4-R7's `unjustified == 0`)
        // and is lawful over the chord — a chord tone or a licensed tension of the palette.
        assert!(
            voices.iter().all(|x| x.function.is_some()),
            "per-note audit passes"
        );
        assert!(voices.iter().all(|x| {
            let pc = pitch_class(x.pitch);
            c[0].chord.contains_pc(pc) || c[0].palette.tensions.contains(&pc)
        }));
        let d = measure(voices, &c);
        assert!(
            d.unowned_m2 >= 1,
            "pad B4 vs keys C5 is an unowned m2: {d:?}"
        );
        assert!(d.role_pairs.get("keys/pad").copied().unwrap_or(0) >= 1);
        assert!(d.hard_total() > 0);
    }

    #[test]
    fn a_lead_ninth_copied_into_the_bass_is_a_bass_function_violation() {
        let c = ctx_of(&[(0.0, 4.0, CMAJ7)]);
        // The lead's lawful 9th (D5) doubled two octaves down in the bass (D2), labelled as the
        // lead labelled it.
        let voices = vec![
            v(
                Role::Lead,
                74,
                0.0,
                1.0,
                PitchFunction::LicensedExtension,
                "melody",
            ),
            v(
                Role::Bass,
                38,
                0.0,
                1.0,
                PitchFunction::LicensedExtension,
                "unison",
            ),
            v(Role::Keys, 64, 0.0, 1.0, PitchFunction::ChordTone, "comp"),
            v(Role::Keys, 71, 0.0, 1.0, PitchFunction::ChordTone, "comp"),
        ];
        let d = measure(voices, &c);
        assert_eq!(d.bass_function_violations, 1, "{d:?}");
        // A planned slash bass on the 9th owns it.
        let plan = SonorityPlan {
            context: 0,
            start_beat: 0.0,
            end_beat: 4.0,
            bass_pc: 2,
            bass_function: BassFunction::Tension(Degree::Nine),
            core: vec![4, 11],
            colors: vec![],
            omit: vec![],
            altered: vec![],
            upper_structure: None,
            policy: ColorPolicy::lenient(),
        };
        let voices = vec![
            v(Role::Bass, 38, 0.0, 1.0, PitchFunction::ChordTone, "slash"),
            v(Role::Keys, 64, 0.0, 1.0, PitchFunction::ChordTone, "comp"),
            v(Role::Keys, 71, 0.0, 1.0, PitchFunction::ChordTone, "comp"),
        ];
        let d = EnsembleSonorityDiagnostics::measure_voices(
            &voices,
            &c,
            &ColorPolicy::lenient(),
            &[plan],
        );
        assert_eq!(
            d.bass_function_violations, 0,
            "a planned slash bass is owned"
        );
    }

    #[test]
    fn a_sting_owns_its_dissonance_not_its_chord_tone() {
        // A Warning over Fmaj7: F4 (its chord tone) + B4 (the owned tritone). The keys hold E4. The
        // action owns the tritone; the sting's F4 against the keys' E4 is an ordinary m2 nobody owns.
        let fmaj7 = Chord::new(5, Quality::Maj7);
        let c = ctx_of(&[(0.0, 4.0, fmaj7)]);
        let voices = vec![
            v(Role::Bass, 41, 0.0, 4.0, PitchFunction::ChordTone, "root"),
            v(Role::Keys, 64, 0.0, 2.0, PitchFunction::ChordTone, "hold"),
            sfx_voice(65, Some(PitchFunction::ChordTone), 0.0, 0.6, true),
            sfx_voice(71, None, 0.0, 0.6, true),
        ];
        assert!(!voices[2].owned && voices[3].owned);
        let d = measure(voices, &c);
        assert_eq!((d.unowned_m2, d.unowned_m9), (1, 0), "{d:?}");
        assert_eq!(d.role_pairs.get("keys/sfx").copied(), Some(1));
    }

    #[test]
    fn a_release_tail_is_contact_not_the_next_floor() {
        // Dm7 then Cmaj7. The bass D2 rings 0.06 beats into Cmaj7 (a release tail) before the C2:
        // contact, not a 9th in the bass. Held 0.3 beats into Cmaj7 it IS the floor there.
        let dm7 = Chord::new(2, Quality::Min7);
        let c = ctx_of(&[(0.0, 4.0, dm7), (4.0, 4.0, CMAJ7)]);
        let band = |tail_end: f64| {
            vec![
                v(
                    Role::Bass,
                    38,
                    3.0,
                    tail_end,
                    PitchFunction::ChordTone,
                    "root",
                ),
                v(Role::Bass, 36, 4.3, 8.0, PitchFunction::ChordTone, "root"),
                v(Role::Pad, 64, 4.0, 8.0, PitchFunction::ChordTone, "pad"),
                v(Role::Pad, 71, 4.0, 8.0, PitchFunction::ChordTone, "pad"),
            ]
        };
        let tail = measure(band(4.06), &c);
        assert_eq!(tail.bass_function_violations, 0, "{tail:?}");
        assert_eq!(tail.identity_flips_held, 0);
        let held = measure(band(4.3), &c);
        assert_eq!(held.bass_function_violations, 1, "{held:?}");
    }

    #[test]
    fn the_roots_memory_runs_out_under_a_held_non_root_floor() {
        // Dm7 for 8 beats: bass D2 (0-2) then A2 held to 8 under a rootless F3 C4 E4. Two beats
        // after the D stops the ear has lost the root: F-A-C-E is heard as Fmaj7/A.
        let dm7 = Chord::new(2, Quality::Min7);
        let c = ctx_of(&[(0.0, 8.0, dm7)]);
        let band = |a_end: f64| {
            vec![
                v(Role::Bass, 38, 0.0, 2.0, PitchFunction::ChordTone, "root"),
                v(
                    Role::Bass,
                    45,
                    2.0,
                    a_end,
                    PitchFunction::ChordTone,
                    "fifth",
                ),
                v(Role::Pad, 53, 0.0, 8.0, PitchFunction::ChordTone, "pad"),
                v(Role::Pad, 60, 0.0, 8.0, PitchFunction::ChordTone, "pad"),
                v(Role::Pad, 64, 0.0, 8.0, PitchFunction::ChordTone, "pad"),
            ]
        };
        let long = measure(band(8.0), &c);
        assert_eq!(long.identity_flips, 1, "{long:?}");
        assert!((long.identity_flip_beats - 4.0).abs() < 1e-6, "{long:?}");
        // The fifth let go inside the memory: the root is still heard.
        let short = measure(band(3.5), &c);
        assert_eq!(short.identity_flips, 0, "{short:?}");
    }

    #[test]
    fn a_unison_doubling_the_leads_colour_is_one_owner() {
        // The lead sings the 9th (D6); the keys double it in unison (D5, tagged). One owner in two
        // roles. Untagged, the same keys D5 is a second owner of the colour.
        let c = ctx_of(&[(0.0, 4.0, CMAJ7)]);
        let band = |unison: bool| {
            let mut keys = v(
                Role::Keys,
                74,
                0.0,
                4.0,
                PitchFunction::LicensedExtension,
                if unison { "unison" } else { "comp" },
            );
            keys.unison = unison;
            vec![
                v(Role::Bass, 36, 0.0, 4.0, PitchFunction::ChordTone, "root"),
                v(Role::Pad, 64, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
                v(Role::Pad, 71, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
                keys,
                v(
                    Role::Lead,
                    86,
                    0.0,
                    4.0,
                    PitchFunction::LicensedExtension,
                    "melody",
                ),
            ]
        };
        assert_eq!(measure(band(true), &c).duplicate_tension_slices, 0);
        assert_eq!(measure(band(false), &c).duplicate_tension_slices, 1);
    }

    #[test]
    fn the_colour_budget_counts_what_the_band_adds_not_what_the_chord_spells() {
        // Fmaj9 WRITES its 9th (G). SWISS_SIGNAL's budget is one selected colour: the written G plus
        // one added 13th (D) is within it; add a #11 (B) too and the band has chosen two colours.
        let fmaj9 = Chord::new(5, Quality::Maj9);
        let c = ctx_of(&[(0.0, 4.0, fmaj9)]);
        let policy = ColorPolicy::for_world(WorldId::SwissSignal, &MusicalLanguage::default());
        assert_eq!(policy.color_budget, 1);
        assert!(
            !is_selected_color(&fmaj9, 7),
            "the written 9th is not a selection"
        );
        assert!(is_selected_color(&fmaj9, 2), "the 13th is");
        let ext = PitchFunction::LicensedExtension;
        let mut band = vec![
            v(Role::Bass, 41, 0.0, 4.0, PitchFunction::ChordTone, "root"),
            v(Role::Pad, 64, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(Role::Pad, 69, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(Role::Pad, 79, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(Role::Keys, 74, 0.0, 4.0, ext, "comp"),
        ];
        let d = EnsembleSonorityDiagnostics::measure_voices(&band, &c, &policy, &[]);
        assert_eq!(d.over_color_slices, 0, "written 9 + one 13: {d:?}");
        band.push(v(Role::Keys, 83, 0.0, 4.0, ext, "comp"));
        let d = EnsembleSonorityDiagnostics::measure_voices(&band, &c, &policy, &[]);
        assert_eq!(d.over_color_slices, 1, "13 AND #11 are two choices: {d:?}");
        // A written extension is still an extension: two players owning the written 9th double it.
        let doubled = vec![
            v(Role::Bass, 41, 0.0, 4.0, PitchFunction::ChordTone, "root"),
            v(Role::Pad, 64, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(Role::Pad, 69, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(Role::Pad, 79, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(Role::Keys, 67, 0.0, 4.0, PitchFunction::ChordTone, "comp"),
        ];
        let d = EnsembleSonorityDiagnostics::measure_voices(&doubled, &c, &policy, &[]);
        assert_eq!(d.over_color_slices, 0);
        assert_eq!(d.duplicate_tension_slices, 1, "{d:?}");
    }

    #[test]
    fn a_tension_pile_is_caught_and_the_coordinated_version_passes() {
        let c = ctx_of(&[(0.0, 4.0, CMAJ7)]);
        let policy = ColorPolicy::for_world(WorldId::BlackIce, &MusicalLanguage::default());
        // Every role independently takes an available tension: the pad the 13th (A4), the keys the
        // 9th and 13th (D5, A5), the lead the 9th again (D6) — eight voices, one colour owned twice.
        let pile = vec![
            v(Role::Bass, 36, 0.0, 4.0, PitchFunction::ChordTone, "root"),
            v(Role::Pad, 64, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(
                Role::Pad,
                69,
                0.0,
                4.0,
                PitchFunction::LicensedExtension,
                "pad",
            ),
            v(Role::Pad, 71, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(
                Role::Keys,
                74,
                0.0,
                4.0,
                PitchFunction::LicensedExtension,
                "comp",
            ),
            v(Role::Keys, 76, 0.0, 4.0, PitchFunction::ChordTone, "comp"),
            v(
                Role::Keys,
                81,
                0.0,
                4.0,
                PitchFunction::LicensedExtension,
                "comp",
            ),
            v(
                Role::Lead,
                86,
                0.0,
                4.0,
                PitchFunction::LicensedExtension,
                "melody",
            ),
        ];
        let d = EnsembleSonorityDiagnostics::measure_voices(&pile, &c, &policy, &[]);
        assert!(
            d.duplicate_tension_slices >= 1,
            "9 owned by keys AND lead: {d:?}"
        );
        assert!(d.crowded_slices >= 1, "8 voices > BLACK_ICE's 7");
        // Coordinated: bass owns the floor, keys the guide tones, the lead owns the one tension,
        // the pad carries a common tone.
        let coordinated = vec![
            v(Role::Bass, 36, 0.0, 4.0, PitchFunction::ChordTone, "root"),
            v(Role::Keys, 64, 0.0, 4.0, PitchFunction::ChordTone, "comp"),
            v(Role::Keys, 71, 0.0, 4.0, PitchFunction::ChordTone, "comp"),
            v(Role::Pad, 67, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(
                Role::Lead,
                74,
                0.0,
                4.0,
                PitchFunction::LicensedExtension,
                "melody",
            ),
        ];
        let d = EnsembleSonorityDiagnostics::measure_voices(&coordinated, &c, &policy, &[]);
        assert_eq!(d.hard_total(), 0, "{}", d.report());
        assert_eq!(
            d.duplicate_tension_slices + d.over_color_slices + d.crowded_slices,
            0
        );
        assert_eq!(d.missing_core_slices, 0);
    }

    #[test]
    fn a_resolving_passing_collision_is_owned_linear_dissonance() {
        let c = ctx_of(&[(0.0, 4.0, CMAJ7)]);
        // Keys hold E4 and C5; the lead walks D5 (the 9th) -> C#5 (a quarter-beat chromatic passing
        // tone, a minor 2nd over the keys' C5) -> C5: short, stepwise, resolved — owned.
        let mut voices = vec![
            v(Role::Keys, 64, 0.0, 2.0, PitchFunction::ChordTone, "comp"),
            v(Role::Keys, 72, 0.0, 2.0, PitchFunction::ChordTone, "comp"),
            v(
                Role::Lead,
                74,
                0.0,
                0.5,
                PitchFunction::LicensedExtension,
                "melody",
            ),
            v(
                Role::Lead,
                73,
                0.5,
                0.75,
                PitchFunction::ChromaticPassing,
                "melody",
            ),
            v(
                Role::Lead,
                72,
                0.75,
                2.0,
                PitchFunction::ChordTone,
                "melody",
            ),
        ];
        let resolved = voices_of_with_resolution(&mut voices, &c);
        let d = measure(resolved, &c);
        assert_eq!(d.unowned_m2, 0, "{}", d.report());
        assert!(d.owned.get("linear").copied().unwrap_or(0) >= 1);
        assert_eq!(d.unresolved, 0);
    }

    #[test]
    fn a_suspension_owns_its_clash_only_off_its_own_resolution_tone() {
        // Over C, the keys suspend D♭4 into C4 (a b9-8 against the bass). The clash is D♭4 against
        // whatever else sounds a semitone class away from it.
        let c_maj = Chord::new(0, Quality::Maj);
        let c = ctx_of(&[(0.0, 4.0, c_maj)]);
        let mk = |partner: Voice, resolve: bool| {
            let mut vs = vec![
                partner,
                v(Role::Pad, 64, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
                v(Role::Pad, 67, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
                v(Role::Keys, 61, 0.0, 2.0, PitchFunction::Suspension, "hold"),
            ];
            if resolve {
                vs.push(v(
                    Role::Keys,
                    60,
                    2.0,
                    4.0,
                    PitchFunction::ChordTone,
                    "hold",
                ));
            }
            voices_of_with_resolution(&mut vs, &c)
        };
        // The 9-8: the BASS holds the resolution pitch class an octave and more below — owned.
        let bass_c3 = v(Role::Bass, 48, 0.0, 4.0, PitchFunction::ChordTone, "root");
        let ok = measure(mk(bass_c3, true), &c);
        assert_eq!(ok.hard_total(), 0, "{}", ok.report());
        assert!(ok.owned.get("suspension").copied().unwrap_or(0) >= 1);
        // The same suspension against its resolution tone in ANOTHER UPPER voice (the pad holding
        // C4 while the keys' D♭4 resolves into it) is the forbidden case — unowned.
        let pad_c4 = v(Role::Pad, 60, 0.0, 4.0, PitchFunction::ChordTone, "pad");
        let against = measure(mk(pad_c4, true), &c);
        assert!(against.unowned_m2 >= 1, "{}", against.report());
        // Broken: never resolves.
        let bass_c3 = v(Role::Bass, 48, 0.0, 4.0, PitchFunction::ChordTone, "root");
        let broken = measure(mk(bass_c3, false), &c);
        assert!(broken.unowned_m9 >= 1, "{}", broken.report());
        assert!(broken.unresolved >= 1);
    }

    #[test]
    fn a_planned_altered_dominant_is_coherent_and_its_abuses_are_not() {
        // G7(b9) -> Cmaj7: bass G2; keys G3 B3 F4 and A♭4 — the A♭4 a minor 9th over the keys' G3
        // (the root) — then A♭4 steps down to G4 on the Cmaj7. The package {A♭} owns the b9 against
        // the ROOT, over a DOMINANT, when it RESOLVES; nothing else.
        let g7 = Chord::new(7, Quality::Dom7);
        let cmaj7 = Chord::new(0, Quality::Maj7);
        let c = ctx_of(&[(0.0, 4.0, g7), (4.0, 4.0, cmaj7)]);
        let base = |resolve: bool| {
            let mut vs = vec![
                v(Role::Bass, 43, 0.0, 4.0, PitchFunction::ChordTone, "root"),
                v(Role::Keys, 55, 0.0, 4.0, PitchFunction::ChordTone, "comp"),
                v(Role::Keys, 59, 0.0, 4.0, PitchFunction::ChordTone, "comp"),
                v(Role::Keys, 65, 0.0, 4.0, PitchFunction::ChordTone, "comp"),
                v(Role::Keys, 68, 0.0, 4.0, PitchFunction::ModalColor, "comp"),
            ];
            if resolve {
                vs.push(v(
                    Role::Keys,
                    67,
                    4.0,
                    8.0,
                    PitchFunction::ChordTone,
                    "comp",
                ));
            }
            voices_of_with_resolution(&mut vs, &c)
        };
        let plan = |ctx: usize| SonorityPlan {
            context: ctx,
            start_beat: 0.0,
            end_beat: 4.0,
            bass_pc: 7,
            bass_function: BassFunction::Root,
            core: vec![11, 5],
            colors: vec![(8, Role::Keys)],
            omit: vec![],
            altered: vec![8],
            upper_structure: None,
            policy: ColorPolicy::lenient(),
        };
        let lenient = ColorPolicy::lenient();
        let unplanned = measure(base(true), &c);
        assert!(unplanned.unowned_m9 >= 1, "{}", unplanned.report());
        let planned =
            EnsembleSonorityDiagnostics::measure_voices(&base(true), &c, &lenient, &[plan(0)]);
        assert_eq!(planned.hard_total(), 0, "{}", planned.report());
        assert!(planned.owned.get("altered").copied().unwrap_or(0) >= 1);
        // The package does not excuse an unresolved alteration.
        let held =
            EnsembleSonorityDiagnostics::measure_voices(&base(false), &c, &lenient, &[plan(0)]);
        assert!(
            held.unowned_m9 >= 1 && held.unresolved >= 1,
            "{}",
            held.report()
        );
        // Nor a b9 against the natural 9 (A♭4 against a lead A4): not a clash with the root.
        let mut with_nine = base(true);
        with_nine.push(v(
            Role::Lead,
            69,
            0.0,
            4.0,
            PitchFunction::LicensedExtension,
            "melody",
        ));
        let with_nine = voices_of_with_resolution(&mut with_nine, &c);
        let d = EnsembleSonorityDiagnostics::measure_voices(&with_nine, &c, &lenient, &[plan(0)]);
        assert!(
            d.unowned_m2 >= 1 && d.contradiction_slices >= 1,
            "{}",
            d.report()
        );
        // Nor a package on a chord that is not a dominant (the same notes planned over Cmaj7).
        let cm = ctx_of(&[(0.0, 4.0, cmaj7), (4.0, 4.0, cmaj7)]);
        let mut over_major = base(true);
        let over_major = voices_of_with_resolution(&mut over_major, &cm);
        let d = EnsembleSonorityDiagnostics::measure_voices(&over_major, &cm, &lenient, &[plan(0)]);
        assert!(d.unowned_m9 >= 1, "{}", d.report());
    }

    #[test]
    fn a_fifth_in_the_bass_under_rootless_shells_flips_the_chord() {
        // Dm7 (D F A C): keys shell F4 C5 E5 (3, 7, 9), pad shell F3 C4 E4, bass holds A2 for two
        // beats. Every note a chord tone or licensed tension; the union {A C E F} is Fmaj7.
        let dm7 = Chord::new(2, Quality::Min7);
        let c = ctx_of(&[(0.0, 4.0, dm7)]);
        let shells = |bass: Midi| {
            vec![
                v(
                    Role::Bass,
                    bass,
                    0.0,
                    2.0,
                    PitchFunction::ChordTone,
                    "fifth",
                ),
                v(Role::Pad, 53, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
                v(Role::Pad, 60, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
                v(Role::Keys, 65, 0.0, 2.0, PitchFunction::ChordTone, "comp"),
                v(Role::Keys, 72, 0.0, 2.0, PitchFunction::ChordTone, "comp"),
            ]
        };
        let flipped = measure(shells(45), &c);
        assert_eq!(flipped.identity_flips_held, 1, "{}", flipped.report());
        let rooted = measure(shells(38), &c);
        assert_eq!(rooted.identity_flips, 0, "{}", rooted.report());
        // The two-feel: root D2 on beat 1, fifth A2 on beat 3 — the ear still has the D.
        let mut two_feel = shells(45);
        two_feel[0].start = 2.0;
        two_feel[0].end = 4.0;
        two_feel.push(v(
            Role::Bass,
            38,
            0.0,
            2.0,
            PitchFunction::ChordTone,
            "root",
        ));
        let d = measure(two_feel, &c);
        assert_eq!(d.identity_flips, 0, "{}", d.report());
    }

    #[test]
    fn low_register_mud_is_measured_across_roles() {
        let c = ctx_of(&[(0.0, 4.0, CMAJ7)]);
        // Bass C2 (36) under a pad E2 (40): a major 3rd whose lower note is below the M3 limit
        // (B♭2 = 46) — mud, however lawful both pitches are.
        let voices = vec![
            v(Role::Bass, 36, 0.0, 4.0, PitchFunction::ChordTone, "root"),
            v(Role::Pad, 40, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
            v(Role::Pad, 55, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
        ];
        let d = measure(voices, &c);
        assert_eq!(d.low_register, 1, "{}", d.report());
    }

    #[test]
    fn a_voicing_cluster_of_chord_tones_is_colour_not_garbage() {
        // Cmaj7's B/C is its identity: one player's B4 C5 cluster is a colour; the SAME pair split
        // across two players is an unowned m2 (two interpretations of one chord).
        let c = ctx_of(&[(0.0, 4.0, CMAJ7)]);
        let one = vec![
            v(Role::Keys, 64, 0.0, 2.0, PitchFunction::ChordTone, "comp"),
            v(Role::Keys, 71, 0.0, 2.0, PitchFunction::ChordTone, "comp"),
            v(Role::Keys, 72, 0.0, 2.0, PitchFunction::ChordTone, "comp"),
        ];
        assert_eq!(measure(one, &c).unowned_m2, 0);
        let two = vec![
            v(Role::Keys, 64, 0.0, 2.0, PitchFunction::ChordTone, "comp"),
            v(Role::Keys, 71, 0.0, 2.0, PitchFunction::ChordTone, "comp"),
            v(Role::Pad, 72, 0.0, 2.0, PitchFunction::ChordTone, "pad"),
        ];
        assert_eq!(measure(two, &c).unowned_m2, 1);
    }

    #[test]
    fn audible_ends_follow_the_envelope() {
        let world = MusicWorld::vapor95();
        // A sustaining pad rings past its note-off; a zero-sustain keys stab falls silent early.
        let pad_end = audible_end(0.0, 4.0, &world.pad, world.tempo_bpm);
        assert!(
            pad_end > 4.0,
            "a sustaining pad rings after its note-off: {pad_end}"
        );
        let bi = MusicWorld::black_ice();
        let stab = audible_end(0.0, 0.9, &bi.keys, bi.tempo_bpm);
        assert!(
            stab < 0.9,
            "a zero-sustain stab is silent before its written end: {stab}"
        );
    }

    /// Settle the resolutions of hand-built voices the way [`voices_of`] does.
    fn voices_of_with_resolution(vs: &mut [Voice], ctxs: &[HarmonicContext]) -> Vec<Voice> {
        vs.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
        let mut all = vs.to_vec();
        settle_resolutions(&mut all, ctxs);
        all
    }

    /// Round VIIIb's classifier controls. The ONE class the ruler was provably wrong about — a minor
    /// chord's 9th a semitone under its minor 3rd in the chordal support, the rootless "B-form" — is
    /// colour whether one player or two hold it; everything around it that is a real avoid stays
    /// unowned; the owned relations (a written alteration, a suspension, a short linear note) stay
    /// owned. The raw `classify_clash` (the frozen R8 generator's) still says what it said.
    #[test]
    fn the_b_form_is_colour_and_the_real_avoids_are_not() {
        let dm7 = Chord {
            root_pc: 2,
            quality: Quality::Min7,
        };
        let c = ctx_of(&[(0.0, 4.0, dm7)]);
        let ctx = &c[0];
        let ext = PitchFunction::LicensedExtension;
        let ct = PitchFunction::ChordTone;
        // Dm7 voiced C5 E5 F5 A5 by the pad alone.
        let (e5, f5) = (
            v(Role::Pad, 76, 0.0, 4.0, ext, "pad"),
            v(Role::Pad, 77, 0.0, 4.0, ct, "pad"),
        );
        assert_eq!(
            classify_clash(ctx, &e5, &f5, None),
            VerticalClass::UnownedCollision
        );
        assert_eq!(
            classify_heard(ctx, &e5, &f5, None),
            VerticalClass::StableColor
        );
        // The same two notes split between the pad and the keys: one composite voicing.
        let f5_keys = v(Role::Keys, 77, 0.0, 0.5, ct, "comp");
        assert_eq!(
            classify_heard(ctx, &e5, &f5_keys, None),
            VerticalClass::StableColor
        );
        // Not exempt: the 9th directly under the MELODY's 3rd, the pair a minor 9th apart, a sting.
        let f5_lead = v(Role::Lead, 77, 0.0, 1.0, ct, "melody");
        assert_eq!(
            classify_heard(ctx, &e5, &f5_lead, None),
            VerticalClass::UnownedCollision
        );
        let e4 = v(Role::Pad, 64, 0.0, 4.0, ext, "pad");
        assert_eq!(
            classify_heard(ctx, &e4, &f5, None),
            VerticalClass::UnownedCollision
        );
        let f5_sfx = Voice {
            sfx: true,
            ..f5_keys
        };
        assert_eq!(
            classify_heard(ctx, &e5, &f5_sfx, None),
            VerticalClass::UnownedCollision
        );
        // Root over major 7th — the classic avoid — stays unowned between the support players and
        // against the melody.
        let c7 = ctx_of(&[(0.0, 4.0, CMAJ7)]);
        let b4 = v(Role::Pad, 71, 0.0, 4.0, ct, "pad");
        for other in [
            v(Role::Keys, 72, 0.0, 0.5, ct, "comp"),
            v(Role::Lead, 72, 0.0, 1.0, ct, "melody"),
        ] {
            assert_eq!(
                classify_heard(&c7[0], &b4, &other, None),
                VerticalClass::UnownedCollision
            );
        }
        // A written b9 on the dominant, planned and resolving, against the root: owned.
        let g7 = ctx_of(&[(
            0.0,
            4.0,
            Chord {
                root_pc: 7,
                quality: Quality::Dom7,
            },
        )]);
        let plan = SonorityPlan {
            context: 0,
            start_beat: 0.0,
            end_beat: 4.0,
            bass_pc: 7,
            bass_function: BassFunction::Root,
            core: vec![11, 5],
            colors: vec![],
            omit: vec![],
            altered: vec![8],
            upper_structure: None,
            policy: ColorPolicy::lenient(),
        };
        let g4 = v(Role::Bass, 55, 0.0, 4.0, ct, "root");
        let ab5 = Voice {
            resolves_to: Some(79),
            ..v(Role::Pad, 68, 0.0, 1.0, ext, "pad")
        };
        assert_eq!(
            classify_heard(&g7[0], &g4, &ab5, Some(&plan)),
            VerticalClass::AlteredColor
        );
        // A short chromatic passing tone brushing a pad tone, and a resolving suspension: owned.
        let pass = Voice {
            resolves: true,
            ..v(
                Role::Lead,
                75,
                2.0,
                2.25,
                PitchFunction::ChromaticPassing,
                "melody",
            )
        };
        let e5c = v(Role::Pad, 76, 0.0, 4.0, ct, "pad");
        assert_eq!(
            classify_heard(&c7[0], &pass, &e5c, None),
            VerticalClass::LinearCollision
        );
        let sus = Voice {
            resolves_to: Some(76),
            ..v(Role::Pad, 77, 0.0, 1.0, PitchFunction::Suspension, "pad")
        };
        let b4c = v(Role::Keys, 71, 0.0, 1.0, ct, "comp");
        assert_ne!(
            classify_heard(&c7[0], &sus, &Voice { pitch: 78, ..b4c }, None),
            VerticalClass::UnownedCollision
        );
    }
}
