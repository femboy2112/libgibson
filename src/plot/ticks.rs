//! Deterministic, size-independent tick generation.
//!
//! Tick *values* are semantic: they depend only on the scale and the visible
//! range, never on the terminal size (responsive rendering may drop *labels*,
//! but never moves a value). Linear uses a `1 / 2 / 5 × 10^k` nice-number
//! lattice; `Log10` uses powers of ten (majors) plus optional `2..9 × 10^k`
//! minors. See `docs/PLOT_OBSERVABLE_GEOMETRY.md` §7.

use super::scale::{AxisScale, FiniteRange};

/// A semantic tick: a data-space value and its rendered label (empty for an
/// unlabelled minor).
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    pub value: f64,
    pub label: String,
}

/// Snap a nearly-integer log exponent to the integer, so fp dust in
/// `(1e-3).log10() == -2.9999999…` does not drop the boundary decade.
fn snap(x: f64) -> f64 {
    let r = x.round();
    if (x - r).abs() < 1e-9 {
        r
    } else {
        x
    }
}

/// The nice step at or just above `raw`, drawn from `{1,2,5} × 10^k`.
fn nice_step(raw: f64) -> f64 {
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag; // in [1, 10)
    let nice = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * mag
}

fn decimals_for(step: f64) -> usize {
    let d = -(step.log10().floor());
    (d.max(0.0) as usize).min(15)
}

fn fmt_fixed(v: f64, decimals: usize) -> String {
    // Fixed-point has an unbounded integer side: `{:.0}` of 1e308 is 309 chars.
    // For extreme magnitudes fall back to scientific so a label stays short. 1e16
    // is far above any sane scientific axis and any value a log label routes here
    // (k ∈ -4..=5), so normal labels are unaffected.
    let v = if v == 0.0 { 0.0 } else { v }; // avoid "-0"
    if v.abs() >= 1e16 {
        return format!("{v:.1e}");
    }
    format!("{v:.decimals$}")
}

/// Major ticks for a linear axis: a nice-number lattice covering `range` with
/// roughly `target` divisions. Deterministic; all returned values lie in
/// `[min, max]` and are integer multiples of a single nice step.
pub fn linear_ticks(range: FiniteRange, target: usize) -> Vec<Tick> {
    let target = target.max(2);
    let raw = range.span() / target as f64;
    if !raw.is_finite() || raw <= 0.0 {
        return Vec::new();
    }
    let step = nice_step(raw);
    let decimals = decimals_for(step);
    let tol = step * 1e-9;
    let first_k = (range.min() / step).ceil();
    let mut out = Vec::new();
    let mut k = first_k;
    let mut guard = 0;
    loop {
        let v = k * step;
        if v > range.max() + tol {
            break;
        }
        if v >= range.min() - tol {
            out.push(Tick {
                value: v,
                label: fmt_fixed(v, decimals),
            });
        }
        k += 1.0;
        guard += 1;
        if guard > 10_000 {
            break;
        }
    }
    out
}

fn log_label(k: i64) -> String {
    if (-4..=5).contains(&k) {
        fmt_fixed(10f64.powi(k as i32), if k < 0 { (-k) as usize } else { 0 })
    } else {
        format!("1e{k}")
    }
}

/// Major ticks for a `Log10` axis: the powers of ten within `range`. Empty if
/// the range is not strictly positive (not a valid log domain).
pub fn log10_major_ticks(range: FiniteRange) -> Vec<Tick> {
    if range.min() <= 0.0 {
        return Vec::new();
    }
    let k0 = snap(range.min().log10()).ceil() as i64;
    let k1 = snap(range.max().log10()).floor() as i64;
    let mut out = Vec::new();
    for k in k0..=k1 {
        out.push(Tick {
            value: 10f64.powi(k as i32),
            label: log_label(k),
        });
    }
    out
}

/// Minor ticks for a `Log10` axis: `2..=9 × 10^k` within `range`, unlabelled.
/// A renderer draws these only when space allows.
pub fn log10_minor_ticks(range: FiniteRange) -> Vec<Tick> {
    if range.min() <= 0.0 {
        return Vec::new();
    }
    let k0 = snap(range.min().log10()).floor() as i64;
    let k1 = snap(range.max().log10()).floor() as i64;
    let tol_lo = range.min() * (1.0 - 1e-9);
    let tol_hi = range.max() * (1.0 + 1e-9);
    let mut out = Vec::new();
    for k in k0..=k1 {
        let decade = 10f64.powi(k as i32);
        for m in 2..=9 {
            let v = m as f64 * decade;
            if v >= tol_lo && v <= tol_hi {
                out.push(Tick {
                    value: v,
                    label: String::new(),
                });
            }
        }
    }
    out
}

/// Label a bare log value (a minor promoted to a labelled tick), with just
/// enough decimals for its magnitude and no `-0`.
fn log_value_label(v: f64) -> String {
    if v <= 0.0 {
        return String::new();
    }
    let decimals = (-(v.log10().floor())).max(0.0) as usize;
    fmt_fixed(v, decimals.min(15))
}

/// Ticks for a `Log10` axis (POST-CANARY fix for PULSAR-2 defect 4). Normally the
/// powers of ten. But when the view spans **less than two decades** — e.g. `2..8`
/// (no power of ten at all) or `2..60` (a single major) — powers of ten alone give
/// zero or one tick and the axis is unreadable, although `log10_minor_ticks` was
/// already available and simply never called. In that case promote the `2..9×10^k`
/// minors to labelled ticks (merged with any lone major), thinned toward `target`
/// so they do not collide. Doc §7: "minors … only if space supports them."
pub fn log10_ticks(range: FiniteRange, target: usize) -> Vec<Tick> {
    let majors = log10_major_ticks(range);
    if majors.len() >= 2 {
        return majors;
    }
    // Keep every major (a power of ten is the most meaningful tick), and thin the
    // promoted minors into the remaining budget so labels have room to breathe.
    let target = target.max(2);
    let budget = target.saturating_sub(majors.len()).max(1);
    let mut minors: Vec<Tick> = log10_minor_ticks(range)
        .into_iter()
        .map(|mut m| {
            m.label = log_value_label(m.value);
            m
        })
        .collect();
    if minors.len() > budget {
        let stride = minors.len().div_ceil(budget);
        minors = minors
            .into_iter()
            .enumerate()
            .filter(|(i, _)| *i % stride == 0)
            .map(|(_, t)| t)
            .collect();
    }
    let mut all = majors;
    all.append(&mut minors);
    all.sort_by(|a, b| {
        a.value
            .partial_cmp(&b.value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    all.dedup_by(|a, b| (a.value - b.value).abs() <= b.value.abs() * 1e-12);
    all
}

/// Major ticks dispatched by scale.
pub fn major_ticks(scale: AxisScale, range: FiniteRange, target: usize) -> Vec<Tick> {
    match scale {
        AxisScale::Linear => linear_ticks(range, target),
        AxisScale::Log10 => log10_ticks(range, target),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(a: f64, b: f64) -> FiniteRange {
        FiniteRange::new(a, b).unwrap()
    }

    #[test]
    fn linear_lattice_is_nice_and_in_range() {
        let t = linear_ticks(r(0.0, 10.0), 5);
        let vals: Vec<f64> = t.iter().map(|x| x.value).collect();
        assert_eq!(vals, vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        // all in range, strictly increasing
        for w in t.windows(2) {
            assert!(w[1].value > w[0].value);
        }
        assert!(t.first().unwrap().value >= 0.0);
        assert!(t.last().unwrap().value <= 10.0);
    }

    #[test]
    fn linear_step_drawn_from_1_2_5_decades() {
        for (span, target) in [(1.0, 5), (10.0, 5), (37.0, 6), (0.01, 4), (1234.0, 7)] {
            let t = linear_ticks(r(0.0, span), target);
            assert!(t.len() >= 2, "span {span}");
            let step = t[1].value - t[0].value;
            // step / 10^k ∈ {1,2,5}
            let mag = 10f64.powf(step.log10().floor());
            let norm = (step / mag).round();
            assert!(
                [1.0, 2.0, 5.0, 10.0].contains(&norm),
                "span {span}: step {step} norm {norm}"
            );
        }
    }

    #[test]
    fn linear_fractional_labels_have_precision() {
        let t = linear_ticks(r(0.0, 1.0), 5);
        assert!(t.iter().any(|x| x.label == "0.2"));
        assert!(t.iter().any(|x| x.label == "1.0"));
        // never a "-0"
        assert!(t.iter().all(|x| x.label != "-0" && x.label != "-0.0"));
    }

    #[test]
    fn linear_negative_range() {
        let t = linear_ticks(r(-3.0, 7.0), 5);
        assert!(t.iter().all(|x| x.value >= -3.0 && x.value <= 7.0));
        assert!(t.iter().any(|x| x.value == 0.0));
    }

    #[test]
    fn linear_deterministic() {
        let a = linear_ticks(r(-3.3, 7.7), 6);
        let b = linear_ticks(r(-3.3, 7.7), 6);
        assert_eq!(a, b);
    }

    #[test]
    fn log_majors_are_powers_of_ten() {
        let t = log10_major_ticks(r(1e-3, 1e5));
        let vals: Vec<f64> = t.iter().map(|x| x.value).collect();
        assert_eq!(vals, vec![1e-3, 1e-2, 1e-1, 1e0, 1e1, 1e2, 1e3, 1e4, 1e5]);
        assert!(t.iter().any(|x| x.label == "1"));
        assert!(t.iter().any(|x| x.label == "0.001"));
    }

    #[test]
    fn log_majors_boundary_decades_not_dropped_by_fp() {
        // (1e-3).log10() is -2.9999999… in fp; snap must keep the -3 decade.
        let t = log10_major_ticks(r(1e-3, 1e3));
        assert!(t.iter().any(|x| (x.value - 1e-3).abs() < 1e-12));
        assert!(t.iter().any(|x| (x.value - 1e3).abs() < 1e-9));
    }

    #[test]
    fn log_majors_empty_on_nonpositive() {
        assert!(log10_major_ticks(r(-1.0, 10.0)).is_empty());
    }

    #[test]
    fn log_minors_within_decades_unlabelled() {
        let t = log10_minor_ticks(r(1.0, 100.0));
        assert!(!t.is_empty());
        assert!(t.iter().all(|x| x.label.is_empty()));
        assert!(t.iter().any(|x| (x.value - 20.0).abs() < 1e-9));
        assert!(t.iter().all(|x| x.value >= 1.0 && x.value <= 100.0));
    }

    #[test]
    fn linear_labels_bounded_length_break4() {
        // Astronomically large (but finite-span) ranges must not emit absurd
        // fixed-point labels (~300 digits); the integer side is otherwise
        // unbounded (dalembert break #4). Values stay finite & in range.
        for (lo, hi) in [(0.0, 1e308), (-1e300, 1e300)] {
            let t = linear_ticks(r(lo, hi), 6);
            for tk in &t {
                assert!(tk.value.is_finite(), "value finite for {lo}..{hi}");
                assert!(
                    tk.label.chars().count() <= 24,
                    "label {:?} is {} chars for {lo}..{hi}",
                    tk.label,
                    tk.label.chars().count()
                );
            }
        }
    }

    #[test]
    fn log_sub_decade_view_gets_labelled_minor_ticks_postcanary() {
        // POST-CANARY (PULSAR-2 defect 4): a log view spanning no power of ten
        // must not be tick-less. `major_ticks` now promotes labelled minors.
        let t = major_ticks(AxisScale::Log10, r(2.0, 8.0), 6);
        assert!(
            t.len() >= 2,
            "a 2..8 log axis must have readable ticks: {t:?}"
        );
        assert!(
            t.iter().all(|x| !x.label.is_empty()),
            "the fallback ticks are labelled"
        );
        assert!(t.iter().all(|x| x.value >= 2.0 && x.value <= 8.0));
        // 2..60: a single major (10) alone is unreadable; minors fill it in.
        let t = major_ticks(AxisScale::Log10, r(2.0, 60.0), 6);
        assert!(t.len() > 1, "more than the lone '10' major: {t:?}");
        assert!(t.iter().any(|x| x.label == "10"));
        // thinned toward the target, not a wall of 14 labels
        assert!(t.len() <= 8, "thinned, got {}", t.len());
        // a normal multi-decade view is unchanged (pure powers of ten)
        let t = major_ticks(AxisScale::Log10, r(1.0, 1e4), 6);
        assert!(t.iter().all(|x| (x.value.log10().fract()).abs() < 1e-9));
    }

    #[test]
    fn hostile_tiny_and_huge_no_panic() {
        // tiny span — must not panic, labels stay finite, values in range
        let t = linear_ticks(r(0.0, 1e-9), 4);
        assert!(t.iter().all(|x| x.value.is_finite()));
        // huge span
        let t2 = linear_ticks(r(-1e12, 1e12), 6);
        assert!(t2.len() >= 2 && t2.iter().all(|x| x.value.is_finite()));
        // single sub-decade log range
        let t3 = log10_major_ticks(r(1.0, 5.0));
        assert_eq!(t3.iter().map(|x| x.value).collect::<Vec<_>>(), vec![1.0]);
    }
}
