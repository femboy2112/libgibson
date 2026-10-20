//! Independent, source-grounded SECTION-FORM witness for the smooth-pop backing.
//!
//! The pop closure law ([`super::kernel`]'s `pop_route_law`) checks only that the route is a whole
//! number of `loop_bars` cycles repeated exactly. When a source sets `loop_bars == route.len()` (the
//! rehearsal backing does), that check is a TAUTOLOGY — `route[i] == route[i % n]` for `i < n` is
//! vacuously true — so it certifies nothing about the song's actual verse -> chorus form. This witness
//! is the independent law for that form, reconstructed from the source route and groove.
//!
//! It recognises a two-section pop form and REFUSES a false certificate of development: the verse must
//! be a genuine repeating harmonic camp (distinct bars that actually recur), the camp must have motion
//! (not a one-chord drone), the chorus must genuinely transform the verse (not merely repeat it), the
//! form must reach a cadential close distinct from the camp (the turnaround dominant), and the bass
//! must mark the section boundary with a real transition. A flat drone, a chorus equal to the verse, a
//! missing turnaround, a verse that does not recur, or a missing bass transition each break it. Inert
//! (valid) unless the source is `SmoothPop` (C137 uses the literal-tonic law instead).

use super::argument::{CompiledArgument, SourceDomain};
use super::theory::Quality;

/// A section-form failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionViolation {
    pub reason: &'static str,
}

/// The song's section form, reported independently of the loop-closure law and of listening.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionWitness {
    violations: Vec<SectionViolation>,
}

impl SectionWitness {
    pub fn valid(&self) -> bool {
        self.violations.is_empty()
    }
    pub fn violations(&self) -> &[SectionViolation] {
        &self.violations
    }
}

fn is_dominant(q: Quality) -> bool {
    matches!(q, Quality::Dom7 | Quality::Dom9)
}

/// Observe the two-section (verse -> chorus) form from the source route and groove. Valid (inert) for
/// a non-`SmoothPop` source.
pub fn observe_section_form(compiled: &CompiledArgument) -> SectionWitness {
    let mut violations = Vec::new();
    let mut fail = |reason: &'static str| violations.push(SectionViolation { reason });

    if !matches!(compiled.domain, SourceDomain::SmoothPop(_)) {
        return SectionWitness { violations };
    }

    let route = &compiled.route;
    let n = route.len();
    // A two-section form with an even-bar verse camp.
    if n < 4 || n % 2 != 0 || (n / 2) % 2 != 0 {
        fail("route is not a two-section form with an even-bar verse camp");
        return SectionWitness { violations };
    }
    let half = n / 2; // verse = route[0..half], chorus = route[half..n]
    let period = half / 2; // the camp's repeating unit within the verse
    let verse = &route[0..half];
    let chorus = &route[half..n];

    // The verse is a GENUINE repeating camp: its second statement recurs its first, bar for bar —
    // distinct bars that actually agree (not the i < n tautology).
    if (0..period).any(|i| verse[i] != verse[i + period]) {
        fail("verse is not a genuine repeating harmonic camp (its statements do not recur)");
    }
    // The camp has MOTION: it is not a single chord droned across the bars.
    if verse.iter().all(|c| *c == verse[0]) {
        fail("verse camp has no harmonic motion (a one-chord drone, not a progression)");
    }
    // The chorus genuinely TRANSFORMS the verse: it is not the verse repeated bar for bar.
    if chorus == verse {
        fail(
            "chorus does not transform the verse (it is the verse repeated — a false certificate)",
        );
    }
    // A cadential close DISTINCT from the camp: the final bar is a dominant turnaround, and the camp
    // itself contains no dominant — so the close is a real harmonic event, not part of the vamp.
    if !is_dominant(route[n - 1].1) {
        fail("the form does not reach a cadential dominant close (the turnaround is missing)");
    }
    if verse.iter().any(|c| is_dominant(c.1)) {
        fail(
            "the verse camp already contains a dominant — the close is not distinct from the vamp",
        );
    }

    // The bass marks the section with a real TRANSITION: the last verse bar's cell differs from the
    // opening verse cell (the guitarist's/bassist's source-derived walk into the chorus).
    match &compiled.groove {
        Some(groove) if groove.bass_cells.len() >= half => {
            if groove.bass_cells[half - 1] == groove.bass_cells[0] {
                fail("the last verse bar has no bass transition into the chorus");
            }
        }
        _ => fail("a smooth-pop section form needs a groove with a bass cell per verse bar"),
    }

    SectionWitness { violations }
}
