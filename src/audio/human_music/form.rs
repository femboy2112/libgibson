//! Global form: the long-range plan that keeps local reactivity from destroying
//! structure. A warning at beat 3 may perturb a phrase; it must not modulate the whole
//! song. [`Form`] is derived from the **semantic trace alone** (not the world), so the same
//! trace yields the same section skeleton and the same energy/tension/density target curves
//! under every dialect — the invariant the skin natural-transformations preserve.

use super::semantic::SemanticTrace;

/// The fixed meter for Round I (4/4). Form bars are counted in these beats.
pub const BEATS_PER_BAR: f64 = 4.0;

/// The number of bars a piece of `total_beats` occupies — **the partial-bar rule**, the one bar
/// count every layer uses (form, plan, summary, diagnostics). A final partial bar is a bar: 9 beats
/// is 3 bars (two whole bars and one of a single beat), never 2 (which would drop the last beat) or
/// a rounded 2 or 3 depending on where the fraction falls. Never 0: even an empty request occupies
/// one bar. A hair of float slack keeps an exact multiple (120 beats) at exactly 30 bars.
pub fn bars_spanning(total_beats: f64, beats_per_bar: f64) -> u32 {
    let bpb = if beats_per_bar > 0.0 {
        beats_per_bar
    } else {
        BEATS_PER_BAR
    };
    if !total_beats.is_finite() || total_beats <= 0.0 {
        return 1;
    }
    ((total_beats / bpb - 1e-9).ceil().max(1.0)) as u32
}

/// A structural section kind (provenance / diagnostics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Intro,
    A,
    Development,
    Climax,
    Contrast,
    Coda,
}

impl SectionKind {
    pub fn label(self) -> &'static str {
        match self {
            SectionKind::Intro => "intro",
            SectionKind::A => "A",
            SectionKind::Development => "dev",
            SectionKind::Climax => "climax",
            SectionKind::Contrast => "contrast",
            SectionKind::Coda => "coda",
        }
    }
}

/// One section: a bar span with energy/tension/density *targets*.
#[derive(Debug, Clone, Copy)]
pub struct Section {
    pub kind: SectionKind,
    pub start_bar: u32,
    pub bars: u32,
    pub energy: f32,
    pub tension: f32,
    pub density: f32,
}

impl Section {
    /// One past the last bar of this section.
    pub fn end_bar(&self) -> u32 {
        self.start_bar + self.bars
    }
}

/// A planned form.
#[derive(Debug, Clone)]
pub struct Form {
    pub sections: Vec<Section>,
    pub total_bars: u32,
}

impl Form {
    /// Derive a form from a semantic trace. Each region between consecutive semantic events
    /// becomes a section; the highest-pressure region is the Climax, the first is the
    /// Intro, the last is the Coda.
    pub fn from_trace(trace: &SemanticTrace) -> Form {
        let evs = &trace.events;
        // The partial-bar rule: the form spans every bar the trace touches, the last one partial.
        let piece_bars = bars_spanning(trace.total_beats, BEATS_PER_BAR);
        if evs.is_empty() {
            return Form {
                sections: vec![Section {
                    kind: SectionKind::A,
                    start_bar: 0,
                    bars: piece_bars,
                    energy: 0.5,
                    tension: 0.3,
                    density: 0.5,
                }],
                total_bars: piece_bars,
            };
        }

        // Region boundaries in beats -> bars.
        let mut anchors: Vec<(f64, usize)> = evs
            .iter()
            .enumerate()
            .map(|(i, e)| (e.at_beat, i))
            .collect();
        anchors.push((trace.total_beats, evs.len()));

        // Which region has the greatest semantic pressure -> Climax.
        let climax_idx = evs
            .iter()
            .enumerate()
            .max_by(|a, b| {
                a.1.state
                    .pressure()
                    .partial_cmp(&b.1.state.pressure())
                    .unwrap()
            })
            .map(|(i, _)| i)
            .unwrap_or(0);

        let n = evs.len();
        let mut sections = Vec::with_capacity(n);
        for i in 0..n {
            let start_beat = anchors[i].0;
            let end_beat = anchors[i + 1].0;
            let start_bar = (start_beat / BEATS_PER_BAR).round() as u32;
            // Interior boundaries snap to the nearest bar; the final one is the piece's own end
            // under the partial-bar rule (so a 9-beat trace keeps its ninth beat).
            let end_bar = if i + 1 == n {
                piece_bars as f64
            } else {
                (end_beat / BEATS_PER_BAR).round()
            }
            .max((start_bar + 1) as f64) as u32;
            let st = evs[i].state;
            let density = match st.density {
                super::semantic::Density::Compact => 0.8,
                super::semantic::Density::Normal => 0.5,
                super::semantic::Density::Spacious => 0.3,
            };
            let kind = if i == 0 {
                SectionKind::Intro
            } else if i == climax_idx {
                SectionKind::Climax
            } else if i == n - 1 {
                SectionKind::Coda
            } else if i < climax_idx {
                if i % 2 == 1 {
                    SectionKind::A
                } else {
                    SectionKind::Development
                }
            } else {
                SectionKind::Contrast
            };
            sections.push(Section {
                kind,
                start_bar,
                bars: end_bar - start_bar,
                energy: (st.dynamic() * 0.6 + st.pressure() * 0.4).clamp(0.0, 1.0),
                tension: st.pressure(),
                density,
            });
        }
        let total_bars = sections.last().map(|s| s.end_bar()).unwrap_or(1);
        Form {
            sections,
            total_bars,
        }
    }

    /// The section containing `bar` (clamped to the last section).
    pub fn section_at_bar(&self, bar: u32) -> &Section {
        self.sections
            .iter()
            .find(|s| bar >= s.start_bar && bar < s.end_bar())
            .unwrap_or_else(|| self.sections.last().unwrap())
    }

    fn curve(&self, bar_f: f64, pick: impl Fn(&Section) -> f32) -> f32 {
        // Piecewise-linear through (section start, target) anchors + a final anchor.
        if self.sections.is_empty() {
            return 0.0;
        }
        let anchors: Vec<(f64, f32)> = self
            .sections
            .iter()
            .map(|s| (s.start_bar as f64, pick(s)))
            .chain(std::iter::once((
                self.total_bars as f64,
                pick(self.sections.last().unwrap()),
            )))
            .collect();
        for w in anchors.windows(2) {
            let (b0, v0) = w[0];
            let (b1, v1) = w[1];
            if bar_f >= b0 && bar_f <= b1 {
                let t = if b1 > b0 {
                    (bar_f - b0) / (b1 - b0)
                } else {
                    0.0
                };
                return v0 + (v1 - v0) * t as f32;
            }
        }
        anchors.last().unwrap().1
    }

    /// Interpolated energy target at a fractional bar.
    pub fn energy_at(&self, bar_f: f64) -> f32 {
        self.curve(bar_f, |s| s.energy)
    }
    /// Interpolated tension target at a fractional bar.
    pub fn tension_at(&self, bar_f: f64) -> f32 {
        self.curve(bar_f, |s| s.tension)
    }
    /// Interpolated density target at a fractional bar.
    pub fn density_at(&self, bar_f: f64) -> f32 {
        self.curve(bar_f, |s| s.density)
    }
}

#[cfg(test)]
mod tests {
    use super::super::semantic::demo_trace;
    use super::*;

    #[test]
    fn the_partial_bar_rule_counts_every_bar_a_piece_touches() {
        // 9 beats is two whole bars and one of a single beat — three bars, not a rounded two that
        // drops the ninth beat; 10.5 and 17 likewise round UP; an exact multiple stays exact.
        for (beats, bars) in [
            (9.0, 3),
            (10.5, 3),
            (17.0, 5),
            (120.0, 30),
            (8.0, 2),
            (12.0, 3),
            (0.5, 1),
        ] {
            assert_eq!(bars_spanning(beats, BEATS_PER_BAR), bars, "{beats} beats");
        }
        // Never zero bars, even for a degenerate request.
        assert_eq!(bars_spanning(0.0, BEATS_PER_BAR), 1);
        assert_eq!(bars_spanning(f64::NAN, BEATS_PER_BAR), 1);
        // The legacy trace form obeys the same rule at its final boundary.
        assert_eq!(Form::from_trace(&demo_trace(9.0)).total_bars, 3);
        assert_eq!(Form::from_trace(&demo_trace(120.0)).total_bars, 30);
    }

    #[test]
    fn form_covers_the_trace_and_has_a_climax() {
        let form = Form::from_trace(&demo_trace(120.0));
        assert!(!form.sections.is_empty());
        assert_eq!(form.sections[0].kind, SectionKind::Intro);
        assert!(form.sections.iter().any(|s| s.kind == SectionKind::Climax));
        assert_eq!(form.sections.last().unwrap().kind, SectionKind::Coda);
        // Sections tile without gaps.
        for w in form.sections.windows(2) {
            assert_eq!(w[0].end_bar(), w[1].start_bar);
        }
    }

    #[test]
    fn climax_has_the_highest_tension() {
        let form = Form::from_trace(&demo_trace(120.0));
        let climax = form
            .sections
            .iter()
            .find(|s| s.kind == SectionKind::Climax)
            .unwrap();
        let max_tension = form
            .sections
            .iter()
            .map(|s| s.tension)
            .fold(0.0f32, f32::max);
        assert!((climax.tension - max_tension).abs() < 1e-6);
    }

    #[test]
    fn energy_curve_is_bounded_and_continuous() {
        let form = Form::from_trace(&demo_trace(120.0));
        let mut prev = form.energy_at(0.0);
        for i in 0..=(form.total_bars * 4) {
            let b = i as f64 / 4.0;
            let e = form.energy_at(b);
            assert!((0.0..=1.0).contains(&e));
            // Continuity: no jump larger than a section's worth in a quarter-bar step.
            assert!((e - prev).abs() < 0.5);
            prev = e;
        }
    }
}
