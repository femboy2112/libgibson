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
//!   set; it does not sound the union of every legal tension each engine happened to pick.
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
    /// Colour pitch classes selected for this harmony, each with its single owner.
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
    /// Whether it doubles a planned ensemble unison line (one owner in several roles).
    pub unison: bool,
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
            VerticalClass::UnownedCollision => "UNOWNED",
        }
    }
}

/// Who owns the semitone-class clash between `a` and `b` over `ctx` — the vertical question. The
/// order of the exemptions is the order of the argument: motion first (a resolving linear note or
/// suspension owns its clash), then structure (a bass pedal, a planned alteration), then the
/// one-player chordal cluster; otherwise nobody does.
pub fn classify_clash(
    ctx: &HarmonicContext,
    a: &Voice,
    b: &Voice,
    plan: Option<&SonorityPlan>,
) -> VerticalClass {
    let (lo, hi) = if a.pitch <= b.pitch { (a, b) } else { (b, a) };
    let short_linear =
        |v: &Voice| v.is_linear() && v.dur() <= LINEAR_MAX_BEATS + 1e-9 && v.resolves;
    if short_linear(lo) || short_linear(hi) {
        return VerticalClass::LinearCollision;
    }
    let susp = |v: &Voice| v.function.is_some_and(is_suspension);
    if susp(lo) || susp(hi) {
        return if (susp(lo) && lo.resolves) || (susp(hi) && hi.resolves) {
            VerticalClass::Suspension
        } else {
            VerticalClass::UnownedCollision
        };
    }
    if lo.role == Role::Bass
        && (lo.function == Some(PitchFunction::PedalTone) || lo.tag == "pedal")
        && hi.pitch - lo.pitch >= 12
    {
        return VerticalClass::OwnedTension;
    }
    if let Some(p) = plan {
        let (lpc, hpc) = (pitch_class(lo.pitch), pitch_class(hi.pitch));
        if !p.altered.is_empty()
            && (p.altered.contains(&hpc) || p.altered.contains(&lpc))
            && (lpc == ctx.chord.root_pc || hpc == ctx.chord.root_pc || p.altered.contains(&lpc))
        {
            return VerticalClass::AlteredColor;
        }
    }
    if lo.role == hi.role
        && hi.pitch - lo.pitch == 1
        && lo.pitch >= CLUSTER_FLOOR
        && ctx.chord.contains_pc(pitch_class(lo.pitch))
        && ctx.chord.contains_pc(pitch_class(hi.pitch))
    {
        return VerticalClass::StableColor;
    }
    VerticalClass::UnownedCollision
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
    /// The bass rests on a non-root while nobody sounds the root: the rootless upper voices plus
    /// that floor spell a DIFFERENT chord (Dm7's shell over an A bass reads Fmaj7). Hard when the
    /// bass holds it a beat or more.
    IdentityFlip { bass: usize, held: bool },
    /// More voices sounding than the policy's density limit.
    Crowded { voices: usize, limit: usize },
    /// More distinct pitch classes than the policy's limit.
    TooManyPcs { pcs: usize, limit: usize },
    /// A linear/suspended note that never reaches its resolution.
    Unresolved { voice: usize },
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

/// Every [`Voice`] of a realized Score, sorted by (start, role, pitch), with the resolution flag of
/// each linear or suspended note computed from what the same player actually plays next.
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
            unison: n.prov.motif_xform == Some("unison") || n.prov.role_note == "unison",
        })
        .collect();
    v.sort_by(|a, b| {
        a.start
            .total_cmp(&b.start)
            .then(a.role.label().cmp(b.role.label()))
            .then(a.pitch.cmp(&b.pitch))
    });
    let resolved: Vec<bool> = v
        .iter()
        .map(|x| {
            let needs = x.function.is_some_and(|f| is_linear(f) || is_suspension(f));
            !needs || resolves_in(&v, x, contexts)
        })
        .collect();
    for (x, r) in v.iter_mut().zip(resolved) {
        x.resolves = r;
    }
    v
}

/// Whether the same player, within [`RESOLVE_WINDOW_BEATS`] of `x`'s end, moves by a step (1–2
/// semitones) to a pitch stable over the harmony it lands in (a chord tone for the bass).
fn resolves_in(all: &[Voice], x: &Voice, contexts: &[HarmonicContext]) -> bool {
    all.iter().any(|y| {
        y.role == x.role
            && y.start > x.start + 1e-6
            && y.start >= x.end - 0.26
            && y.start <= x.end + RESOLVE_WINDOW_BEATS
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
}

/// The approximate AUDIBLE end (beats) of a note on `patch` at `tempo_bpm` — its ADSR heard down to
/// −30 dB of its peak. A percussive patch (sustain ≈ 0) falls silent after its decay whatever its
/// written length; a sustaining one rings through its release after the note-off. Diagnostics only:
/// the planner never simulates samples.
pub fn audible_end(start: f64, dur: f64, patch: &Patch, tempo_bpm: f32) -> f64 {
    let spb = 60.0 / tempo_bpm.max(1.0) as f64;
    let (a, d, s, r) = patch.adsr;
    let (a, d, s, r) = (a as f64, d as f64, s as f64, r as f64);
    // The envelope moves 40 dB (to 1%) in its decay/release time (dsp::env time_to_coef).
    let nominal = dur * spb;
    let secs = if s <= 0.02 {
        // Falls to −30 dB after attack + ¾ of the decay (or the release, if gated off first).
        (a + 0.75 * d).min(nominal + 0.75 * r)
    } else {
        let tail = r * ((30.0 + 20.0 * s.log10()) / 40.0).max(0.0);
        nominal + tail
    };
    start + secs / spb
}

/// One analysed slice of the Score: an interval with a constant set of sounding notes.
#[derive(Debug, Clone, PartialEq)]
pub struct Slice {
    pub start: f64,
    pub end: f64,
    /// Indices into the voice list.
    pub sounding: Vec<usize>,
    pub distinct_pcs: usize,
    pub colors: Vec<i32>,
    pub problems: Vec<Problem>,
    pub class: VerticalClass,
}

/// Evaluate one set of simultaneously sounding voices over `ctx` — the SAME function the planner
/// calls on a candidate. `sounding` indexes `voices`; `dur` is how long this set sounds together.
pub fn evaluate(
    ctx: &HarmonicContext,
    voices: &[Voice],
    sounding: &[usize],
    policy: &ColorPolicy,
    plan: Option<&SonorityPlan>,
) -> (Vec<Problem>, VerticalClass) {
    let mut problems = Vec::new();
    let mut class = VerticalClass::StructuralChord;
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
                let c = classify_clash(ctx, va, vb, plan);
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
    // The floor.
    if let Some(&bass) = sounding
        .iter()
        .filter(|&&i| voices[i].role == Role::Bass)
        .min_by_key(|&&i| voices[i].pitch)
    {
        let v = &voices[bass];
        let f = BassFunction::of(&ctx.chord, v.pitch);
        let owned = (v.is_linear() && v.dur() <= LINEAR_MAX_BEATS + 1e-9 && v.resolves)
            || v.function == Some(PitchFunction::PedalTone)
            || v.tag == "pedal"
            || plan.is_some_and(|p| p.bass_pc == pitch_class(v.pitch));
        if matches!(f, BassFunction::Tension(_) | BassFunction::NonChord) && !owned {
            problems.push(Problem::BassFunction { bass, function: f });
        }
        // The identity: a non-root floor under a band that has no root anywhere.
        let root_sounds = sounding
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
                    held: v.dur() >= 1.0 - 1e-9,
                });
            }
        }
    }
    // Colour: the resting (non-linear) tones beyond the core.
    let resting: Vec<usize> = sounding
        .iter()
        .copied()
        .filter(|&i| !voices[i].is_linear() && !voices[i].function.is_some_and(is_suspension))
        .collect();
    let mut colors: Vec<i32> = resting
        .iter()
        .map(|&i| pitch_class(voices[i].pitch))
        .filter(|pc| !core.contains(pc))
        .collect();
    colors.sort_unstable();
    colors.dedup();
    if !colors.is_empty() {
        raise(&mut class, VerticalClass::StableColor);
    }
    for &pc in &colors {
        let mut owners: Vec<Role> = Vec::new();
        let mut unison_only = true;
        for &i in &resting {
            if pitch_class(voices[i].pitch) == pc && !owners.contains(&voices[i].role) {
                owners.push(voices[i].role);
                unison_only &= voices[i].unison;
            }
        }
        if owners.len() > 1 && !unison_only {
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
    let altered_ok = |x: i32| plan.is_some_and(|p| p.altered.contains(&x));
    for (i, &x) in colors.iter().enumerate() {
        for &y in &colors[i + 1..] {
            let d = (y - x).rem_euclid(12);
            if (d == 1 || d == 11) && !(altered_ok(x) && altered_ok(y)) {
                problems.push(Problem::Contradiction { a: x, b: y });
            }
        }
    }
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
        let (problems, class) = evaluate(ctx, voices, &sounding, policy, plan);
        let pcs: BTreeSet<i32> = sounding
            .iter()
            .map(|&i| pitch_class(voices[i].pitch))
            .collect();
        let core = core_pcs(&ctx.chord);
        let mut colors: Vec<i32> = pcs.iter().copied().filter(|p| !core.contains(p)).collect();
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

fn pair_key(a: Role, b: Role) -> String {
    let (x, y) = if a.label() <= b.label() {
        (a, b)
    } else {
        (b, a)
    };
    format!("{}/{}", x.label(), y.label())
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
        let mut voices = voices_of(score, contexts);
        for v in &mut voices {
            let patch = match v.role {
                Role::Pad => &world.pad,
                Role::Keys => &world.keys,
                Role::Bass => &world.bass,
                Role::Lead => &world.lead,
            };
            v.end = audible_end(v.start, v.end - v.start, patch, score.tempo_bpm);
        }
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
                            let c = classify_clash(ctx, &voices[a], &voices[b], plan);
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
                                    .entry(pair_key(voices[*a].role, voices[*b].role))
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
            "  duplicate_tension_slices={} over_colour_slices={} contradiction_slices={} expensive_sustained={} missing_core_slices={} ({:.2} beats) crowded={} too_many_pcs={}",
            self.duplicate_tension_slices,
            self.over_color_slices,
            self.contradiction_slices,
            self.expensive_sustained,
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
            ["bass/bass", "keys/keys", "lead/lead", "pad/pad"]
                .iter()
                .filter_map(|k| self.role_pairs.get(*k).map(|n| format!("{k}={n}"))),
        )
        .collect();
        let _ = writeln!(s, "  unowned by role pair: {}", pairs.join(" "));
        let _ = writeln!(s, "  non-root bass events: {}", self.non_root_bass.len());
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
    for role in [Role::Bass, Role::Pad, Role::Keys, Role::Lead] {
        let names: Vec<String> = s
            .sounding
            .iter()
            .filter(|&&i| voices[i].role == role)
            .map(|&i| {
                let v = &voices[i];
                format!(
                    "{}({})",
                    note_name(v.pitch),
                    v.function.map(|f| f.label()).unwrap_or("NONE")
                )
            })
            .collect();
        if !names.is_empty() {
            let _ = writeln!(
                out,
                "  {:5} {}",
                format!("{}:", role.label()),
                names.join(" ")
            );
        }
    }
    let nm = |i: usize| format!("{} {}", voices[i].role.label(), note_name(voices[i].pitch));
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
            unison: false,
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
        // Every tension that sits a semitone under a core tone names that tone in `voice_above`
        // (it is colour only voiced a major 7th above it).
        for s in &specs {
            for &c in &s.voice_above {
                assert_eq!((c - s.pc).rem_euclid(12), 1);
            }
        }
    }

    #[test]
    fn individually_legal_collectively_garbage_fails_the_union() {
        // Cmaj7, every note individually lawful: bass root, pad a valid E4-G4-B4 voicing, keys a
        // chord tone plus two licensed tensions (C5 D5 A5), lead a chord tone (B5). The per-note
        // audit (function != None) passes every one; the union holds the pad's B4 against the keys'
        // C5 — a cross-role minor 2nd nobody owns.
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
            v(Role::Lead, 83, 0.0, 2.0, PitchFunction::ChordTone, "melody"),
        ];
        assert!(
            voices.iter().all(|x| x.function.is_some()),
            "per-note audit passes"
        );
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
    fn a_resolving_suspension_passes_and_a_broken_one_fails() {
        // Over G7 the keys hold C5, a 4-3 suspension a minor 2nd over the pad's B4. Resolved: the
        // keys step down to B4 at beat 2. Broken: the keys never move.
        let g7 = Chord::new(7, Quality::Dom7);
        let c = ctx_of(&[(0.0, 4.0, g7)]);
        let mk = |resolve: bool| {
            let mut vs = vec![
                v(Role::Bass, 43, 0.0, 4.0, PitchFunction::ChordTone, "root"),
                v(Role::Pad, 71, 0.0, 4.0, PitchFunction::ChordTone, "pad"),
                v(Role::Keys, 72, 0.0, 2.0, PitchFunction::Suspension, "hold"),
            ];
            if resolve {
                vs.push(v(
                    Role::Keys,
                    71,
                    2.0,
                    4.0,
                    PitchFunction::ChordTone,
                    "hold",
                ));
            }
            voices_of_with_resolution(&mut vs, &c)
        };
        let ok = measure(mk(true), &c);
        assert_eq!(ok.hard_total(), 0, "{}", ok.report());
        assert!(ok.owned.get("suspension").copied().unwrap_or(0) >= 1);
        let broken = measure(mk(false), &c);
        assert!(broken.unowned_m2 >= 1, "{}", broken.report());
        assert!(broken.unresolved >= 1);
    }

    #[test]
    fn a_planned_altered_dominant_is_coherent_and_the_same_notes_unplanned_are_not() {
        // G7(b9): bass G2, keys G3 B3 F4 A♭4 — the A♭4 a minor 9th over the keys' G3. Unplanned it is
        // an unowned m9; with an explicit alteration package {A♭} it is the chord's planned colour.
        let g7 = Chord::new(7, Quality::Dom7);
        let c = ctx_of(&[(0.0, 4.0, g7)]);
        let voices = vec![
            v(Role::Bass, 43, 0.0, 4.0, PitchFunction::ChordTone, "root"),
            v(Role::Keys, 55, 0.0, 4.0, PitchFunction::ChordTone, "comp"),
            v(Role::Keys, 59, 0.0, 4.0, PitchFunction::ChordTone, "comp"),
            v(Role::Keys, 65, 0.0, 4.0, PitchFunction::ChordTone, "comp"),
            v(Role::Keys, 68, 0.0, 4.0, PitchFunction::ModalColor, "comp"),
        ];
        let unplanned = measure(voices.clone(), &c);
        assert!(unplanned.unowned_m9 >= 1, "{}", unplanned.report());
        let plan = SonorityPlan {
            context: 0,
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
        let planned = EnsembleSonorityDiagnostics::measure_voices(
            &voices,
            &c,
            &ColorPolicy::lenient(),
            &[plan],
        );
        assert_eq!(planned.hard_total(), 0, "{}", planned.report());
        assert!(planned.owned.get("altered").copied().unwrap_or(0) >= 1);
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

    /// Recompute the `resolves` flags of hand-built voices the way [`voices_of`] does.
    fn voices_of_with_resolution(vs: &mut [Voice], ctxs: &[HarmonicContext]) -> Vec<Voice> {
        vs.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
        let all = vs.to_vec();
        all.iter()
            .map(|x| {
                let needs = x.function.is_some_and(|f| is_linear(f) || is_suspension(f));
                Voice {
                    resolves: !needs || resolves_in(&all, x, ctxs),
                    ..*x
                }
            })
            .collect()
    }
}
