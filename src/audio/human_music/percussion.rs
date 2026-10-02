//! One percussion surface. Every drum stroke the drummer might play is first a typed candidate —
//! **required** (the pocket the groove stands on, a stroke an action witness needs, a pinned
//! cover stroke) or **optional** (an ornament: a ghost, an interlocking kick, an open hat, a fill
//! flourish past its minimal witness, an answering echo, a unison accent) — and all optional
//! candidates of a bar compete for ONE allowance decided from the band around the drummer.
//!
//! The allowance is an ordered decision, never a weighted score: the first rule that holds sets
//! the bar's [`OrnamentBand`], and the [`DrumRestraint`] shifts that band. Within a band, ornaments
//! are spent in a fixed [`Ornament`] priority. Required strokes are never arbitrated, so a
//! restrained drummer still performs every action it is stamped for.
//!
//! The historical drummer ([`PercussionPolicy::Unarbitrated`]) appends every producer's strokes
//! and stays byte-exact; arbitration is a separate realization law.

/// How much optional information the drummer adds around the required pocket. A dial in the
/// product surface; here it only shifts the band each bar's ordered decision chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DrumRestraint {
    /// The pocket and the required strokes; an ornament only where the drummer has the floor.
    Foundation,
    /// Speaks at chosen moments: the phrase end with space, the floor, otherwise one touch.
    Balanced,
    /// One band more generous everywhere, still yielding to a busy band.
    Expressive,
    /// Every candidate every producer offers (the historical producer set, arbitrated timing).
    Busy,
}

impl DrumRestraint {
    pub const ALL: [DrumRestraint; 4] = [
        DrumRestraint::Foundation,
        DrumRestraint::Balanced,
        DrumRestraint::Expressive,
        DrumRestraint::Busy,
    ];
    pub fn label(self) -> &'static str {
        match self {
            DrumRestraint::Foundation => "foundation",
            DrumRestraint::Balanced => "balanced",
            DrumRestraint::Expressive => "expressive",
            DrumRestraint::Busy => "busy",
        }
    }
}

/// Whether drum strokes pass through the arbitration boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercussionPolicy {
    /// Historical: every producer appends its strokes (the archived drummer, byte-exact).
    Unarbitrated,
    /// One source-level arbitration of required and optional strokes at this restraint.
    Arbitrated(DrumRestraint),
}

/// Why a stroke must sound. Never subject to the ornament allowance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Required {
    /// The groove's own anchors: the bar's downbeat kick, its mid-bar kick, the backbeat.
    PocketAnchor,
    /// The hat time-line at the bar's subdivision.
    Timekeeping,
    /// A stroke carrying a planned action the audit needs (a hit, a push, the minimal fill, a
    /// re-entry, a pickup, the answer's first stroke).
    ActionWitness,
    /// An optional stroke the rate guard re-admitted: a surface verb (pull back / accelerate) is
    /// judged by the drum onset rate in its window against the preceding one, and arbitration
    /// may not reverse that comparison.
    SurfaceWindow,
}

/// What an optional stroke adds, in the order a drummer spends on them (first is spent first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ornament {
    /// Joining a shared landing: a unison figure's first/last stroke, the clap on a big hit.
    Landing,
    /// An extra kick doubling the bass's syncopation, or an accent kick the grid invites.
    Interlock,
    /// Opening the hat on a pickup or push (declined, the hat stays closed; it is timekeeping).
    OpenHat,
    /// Fill strokes past the minimal witness, kept nearest the landing first.
    FillFlourish,
    /// Answering echo strokes past the answer's first.
    AnswerEcho,
    /// A ghost note on a weak off-beat.
    Ghost,
    /// A unison figure's inner strokes.
    UnisonInner,
}

impl Ornament {
    pub const ALL: [Ornament; 7] = [
        Ornament::Landing,
        Ornament::Interlock,
        Ornament::OpenHat,
        Ornament::FillFlourish,
        Ornament::AnswerEcho,
        Ornament::Ghost,
        Ornament::UnisonInner,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Ornament::Landing => "landing",
            Ornament::Interlock => "interlock",
            Ornament::OpenHat => "open-hat",
            Ornament::FillFlourish => "fill-flourish",
            Ornament::AnswerEcho => "answer-echo",
            Ornament::Ghost => "ghost",
            Ornament::UnisonInner => "unison-inner",
        }
    }
}

/// How many optional strokes a bar may spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OrnamentBand {
    Silent,
    Sparse,
    Conversational,
    Open,
}

impl OrnamentBand {
    /// The bar's ornament allowance, in strokes.
    pub fn allowance(self) -> usize {
        match self {
            OrnamentBand::Silent => 0,
            OrnamentBand::Sparse => 1,
            OrnamentBand::Conversational => 3,
            OrnamentBand::Open => usize::MAX,
        }
    }
    fn shift(self, up: bool) -> Self {
        use OrnamentBand::*;
        match (self, up) {
            (Silent, true) => Sparse,
            (Sparse, true) => Conversational,
            (Conversational, true) | (Open, _) => Open,
            (Silent, false) | (Sparse, false) => Silent,
            (Conversational, false) => Sparse,
        }
    }
}

/// A lead bar with at least this many onsets is a busy lead.
pub const BUSY_LEAD_ONSETS: usize = 5;
/// Lead, bass and keys together with at least this many onsets in a bar is a busy band.
pub const BUSY_BAND_ONSETS: usize = 12;
/// A drummer who spent at least this many ornaments in the previous bar has just spoken.
pub const SPOKE_ORNAMENTS: usize = 2;

/// The evidence one bar's decision reads — all of it measured from the plan and the band's
/// already-realized notes, before any drum stroke exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BarEvidence {
    /// The drummer is the bar's foreground, or performs its own figure/answer here.
    pub drummer_floor: bool,
    /// A surface verb's rate comparison covers this bar (reported; the rate guard, not the
    /// band, protects its witness).
    pub surface_window: bool,
    /// The phrase's last bar, the lead silent in its second half, and the drummer has not yet
    /// spoken at length in this phrase.
    pub phrase_end_space: bool,
    pub lead_onsets: usize,
    /// Lead + bass + keys onsets.
    pub band_onsets: usize,
    /// Ornaments admitted in the previous bar.
    pub previous_ornaments: usize,
}

/// The ordered decision for one bar: the first rule that holds sets the band; the restraint
/// shifts it. Returns the band and the rule that chose it.
pub fn decide(restraint: DrumRestraint, e: &BarEvidence) -> (OrnamentBand, &'static str) {
    use OrnamentBand::*;
    if restraint == DrumRestraint::Busy {
        return (Open, "busy restraint: every candidate");
    }
    let (band, why) = if e.drummer_floor {
        (Conversational, "the drummer has the floor")
    } else if e.phrase_end_space {
        (Conversational, "phrase end with space: a moment to speak")
    } else if e.lead_onsets >= BUSY_LEAD_ONSETS || e.band_onsets >= BUSY_BAND_ONSETS {
        (Silent, "the band is already speaking")
    } else if e.previous_ornaments >= SPOKE_ORNAMENTS {
        (Silent, "spoke last bar: leaves room")
    } else {
        (Sparse, "supports the pocket")
    };
    let band = match restraint {
        DrumRestraint::Foundation if e.drummer_floor => band.shift(false),
        DrumRestraint::Foundation => Silent,
        DrumRestraint::Balanced => band,
        DrumRestraint::Expressive => band.shift(true),
        DrumRestraint::Busy => Open,
    };
    (band, why)
}

/// One bar's arbitration, decomposed.
#[derive(Debug, Clone, PartialEq)]
pub struct BarDecision {
    pub bar: u32,
    pub band: OrnamentBand,
    pub reason: &'static str,
    pub evidence: BarEvidence,
    pub required: usize,
    pub offered: usize,
    pub admitted: usize,
}

/// The drummer's arbitration receipt: per bar, the band, the rule and the counts; per ornament
/// kind, how many were offered and admitted. No total "quality".
#[derive(Debug, Clone, PartialEq)]
pub struct PercussionReport {
    pub restraint: DrumRestraint,
    pub bars: Vec<BarDecision>,
    /// `(kind, offered, admitted)` in [`Ornament::ALL`] order.
    pub ornaments: Vec<(Ornament, usize, usize)>,
    pub required: usize,
    /// Optional strokes the rate guard re-admitted to keep a surface verb's witness.
    pub rate_guard: usize,
}

impl PercussionReport {
    pub fn admitted_ornaments(&self) -> usize {
        self.ornaments.iter().map(|o| o.2).sum()
    }
    pub fn offered_ornaments(&self) -> usize {
        self.ornaments.iter().map(|o| o.1).sum()
    }
    pub fn report(&self) -> String {
        let mut text = format!(
            "PercussionReport restraint={} required={} ornaments admitted {}/{} offered (rate guard {})\n",
            self.restraint.label(),
            self.required,
            self.admitted_ornaments(),
            self.offered_ornaments(),
            self.rate_guard
        );
        for (kind, offered, admitted) in &self.ornaments {
            text.push_str(&format!("  {}: {admitted}/{offered}\n", kind.label()));
        }
        for b in &self.bars {
            text.push_str(&format!(
                "  bar {:>3} {:?} required={} ornaments {}/{} — {} (lead {}, band {}, prev {})\n",
                b.bar,
                b.band,
                b.required,
                b.admitted,
                b.offered,
                b.reason,
                b.evidence.lead_onsets,
                b.evidence.band_onsets,
                b.evidence.previous_ornaments
            ));
        }
        text
    }
}

/// A stable per-candidate draw in `[0, 1)`: dropping one candidate never shifts another's timing
/// or chance (the historical drummer's sequential stream does).
pub(crate) fn unit_draw(seed: u64, bar: u32, step: usize, salt: u64) -> f64 {
    let mut x = seed
        ^ (u64::from(bar) << 20)
        ^ ((step as u64) << 8)
        ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    // splitmix64 finalizer
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restraint_orders_the_band_without_a_score() {
        let quiet = BarEvidence::default();
        let busy = BarEvidence {
            lead_onsets: BUSY_LEAD_ONSETS,
            ..BarEvidence::default()
        };
        let floor = BarEvidence {
            drummer_floor: true,
            lead_onsets: 9,
            ..BarEvidence::default()
        };
        let bands = |e: &BarEvidence| DrumRestraint::ALL.map(|r| decide(r, e).0);
        assert_eq!(
            bands(&quiet),
            [
                OrnamentBand::Silent,
                OrnamentBand::Sparse,
                OrnamentBand::Conversational,
                OrnamentBand::Open
            ]
        );
        // A busy band silences every restrained drummer except the one asked to be busy...
        assert_eq!(
            decide(DrumRestraint::Balanced, &busy).0,
            OrnamentBand::Silent
        );
        // ...unless the drummer has the floor, which outranks a busy band.
        assert_eq!(
            decide(DrumRestraint::Balanced, &floor),
            (OrnamentBand::Conversational, "the drummer has the floor")
        );
        // A surface verb's window follows the same ordered decision; its witness is the rate
        // guard's job, not a licence to open the bar.
        let surface = BarEvidence {
            surface_window: true,
            ..busy
        };
        assert_eq!(
            decide(DrumRestraint::Balanced, &surface).0,
            OrnamentBand::Silent
        );
    }

    #[test]
    fn draws_are_stable_per_candidate() {
        let a = unit_draw(7, 3, 5, 1);
        assert_eq!(a, unit_draw(7, 3, 5, 1));
        assert_ne!(a, unit_draw(7, 3, 6, 1));
        assert!((0.0..1.0).contains(&a));
    }
}
