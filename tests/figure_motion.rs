//! C-1: continuous clips, inertialized transitions and the Look retarget.
//!
//! WHAT THESE TESTS ASSERT. Owner ruling 17 ("model continuous motion and not just jarring stop
//! motion"): motion is a pure function of explicit time and its velocity is at least C1. A
//! discontinuity is found by finite differences on a 1 ms grid. For a function p that is C1 with
//! piecewise-bounded acceleration, `|p(t+dt) - 2 p(t) + p(t-dt)| / dt <= A dt` where `A` is the
//! supremum of `|p''|`. A true velocity jump of size J shows up as about J. So every continuity
//! assertion is `max second difference / dt <= A dt + roundoff`, with `A` DERIVED FROM THE
//! FIXTURE'S KEY DATA (see "Declared bounds"), never from observed output.
//!
//! Mutation controls (`RUSTFLAGS='--cfg figure_mutant="..."'`) are caught by ordinary behavioural
//! assertions; no test inspects the cfg. The doc comment of each test lists what it is designed
//! to kill: S = `snap_transition`, F = `freeze_on_preempt`, L = `linear_keys`. The predicted kill
//! matrix is `docs/evidence/m5b/c1/KILL_MATRIX_PREDICTED.md` (a prediction until it is run under
//! heavy). The numeric model behind the bounds is `docs/evidence/m5b/c1/model_c1.py`.
//!
//! ## Declared bounds (derived before any comparison, never tuned)
//!
//! Hermite segment on `[t_i, t_i+1]`, `h` long, `s = (t - t_i) / h`, values `p_i, p_i+1`, tangents
//! `m_i, m_i+1` (units per second). Then
//!   `p_ss = (12 s - 6)(p_i - p_i+1) + (6 s - 4) h m_i + (6 s - 2) h m_i+1`
//! and `|12 s - 6| <= 6`, `|6 s - 4| <= 4`, `|6 s - 2| <= 4` on `[0, 1]`, so
//!   `|p''| = |p_ss| / h^2 <= (6 |dp| + 4 h (|m_i| + |m_i+1|)) / h^2`.
//! A Catmull-Rom tangent is `(p_i+1 - p_i-1) / (t_i+1 - t_i-1)`, a weighted mean of the two chord
//! slopes, or 0 at a clamped end, so `|m| <= S` where `S` is the largest chord slope
//! `|dp| / h` of the channel (including the wrap chord of a loop). Then `|dp| <= S h` and
//!   `|p''| <= 14 S / h_min`            (`A = 14 S / h_min`)
//! and likewise `|p'| <= 1.5 S + 2 S = 3.5 S` (`V = 3.5 S`), using `|6 s (1 - s)| <= 1.5`,
//! `|3 s^2 - 4 s + 1| <= 1` and `|3 s^2 - 2 s| <= 1`. Every channel component is bounded
//! separately and the largest is used.
//!
//! Rotation about Z by angle theta has key components `n = (sin(theta/2), cos(theta/2))`, each a
//! Hermite scalar, then `q = n / |n|`. The signed angle `2 atan2(z, w)` is unaffected by the
//! normalization, and with `rho = |n|`:
//!   `|theta'| <= 2 |n'| / rho`,
//!   `|theta''| <= 2 |n''| / rho + 4 |n'|^2 / rho^2`
//! (from `theta' = 2 (z' w - z w') / rho^2`, `|z' w - z w'| <= |n'| rho`,
//! `|z z' + w w'| <= |n'| rho`). `|n'| <= sqrt(2) V` and `|n''| <= sqrt(2) A` (two components).
//! `RHO_MIN = 0.75` is declared from the fixture: every key has `w = cos(theta/2) >= 0.9` and
//! adjacent keys differ by at most 80 degrees, so the Hermite combination keeps `w` above about
//! 0.8 and `rho >= w`. (The model prints the true minimum, 0.93, as a check on the declaration.)
//!
//! Quintic offset: `h0(x) = 1 - 10x^3 + 15x^4 - 6x^5`, `h1(x) = x - 6x^3 + 8x^4 - 3x^5`,
//! `offset(x) = x0 h0 + v0 T h1`, `x = (t - t0) / T`. `max |h0'| = 1.875`, `max |h1'| = 1`,
//! `max |h0''| = 10 / sqrt(3) = 5.78`, `max |h1''| <= 4` (3.94, the model prints it), so
//! `|offset'| <= 1.875 |x0| / T + |v0|`, `|offset''| <= 5.78 |x0| / T^2 + 4 |v0| / T`, and the
//! preempted motion has `A <= A_new + 5.78 |x0| / T^2 + 4 |v0| / T` with `|v0| <= V_old + V_new`.
//! Crossfade with weight `w = 6x^5 - 15x^4 + 10x^3`: `max w' = 1.875 / E`, `max |w''| = 5.78 / E^2`,
//! and `out'' = (1-w) A'' + w B'' + 2 w' (B' - A') + w'' (B - A)`.
//!
//! Roundoff: three f32 samples each carry at most `u |p|` (`u = 2^-24`), so the second
//! difference over `dt` carries at most `4 u P / dt` (`P` the channel's largest magnitude).
#![cfg(feature = "figure")]
#![allow(clippy::excessive_precision)]
#![allow(clippy::needless_range_loop)]

use std::f64::consts::PI;
use std::time::Duration;

use gibson::figure::clip::{Channel, ChannelTarget, Clip, Keys};
use gibson::figure::math::{Quat, Vec3};
use gibson::figure::motion::{
    Crossfade, Interpolation, Look, Motion, MotionClip, MotionError, MotionPose, Playing, Preempt,
    Repeat, Sample,
};

const DT: f64 = 1.0e-3;
/// f32 unit roundoff, `2^-24`.
const U: f64 = 5.960_464_477_539_063e-8;
/// Declared lower bound of `|n|` for the rotation fixtures (see the module docs).
const RHO_MIN: f64 = 0.75;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3 {
        x: a[0],
        y: a[1],
        z: a[2],
    }
}

fn zq(deg: f64) -> Quat {
    let h = deg.to_radians() * 0.5;
    Quat {
        x: 0.0,
        y: 0.0,
        z: h.sin() as f32,
        w: h.cos() as f32,
    }
}

fn angle(q: Quat) -> f64 {
    2.0 * f64::from(q.z).atan2(f64::from(q.w))
}

// ---------------------------------------------------------------------------------------------
// Fixtures. Synthetic, three channels each: translation of `arm`, Z rotation of `spine`, morph
// weight `smile`.
// ---------------------------------------------------------------------------------------------

struct Fx {
    dur_ms: u64,
    t_ms: &'static [u64],
    arm: &'static [[f32; 3]],
    spine_deg: &'static [f64],
    smile: &'static [f32],
}

/// 2000 ms, keys every 500 ms, first and last key equal (a closed loop).
const FX_A: Fx = Fx {
    dur_ms: 2000,
    t_ms: &[0, 500, 1000, 1500, 2000],
    arm: &[
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 0.0],
        [0.2, -1.0, 0.5],
        [1.5, 0.5, -1.0],
        [0.0, 0.0, 0.0],
    ],
    spine_deg: &[0.0, 40.0, -30.0, 50.0, 0.0],
    smile: &[0.0, 1.0, 0.2, 0.8, 0.0],
};

/// 1000 ms, keys at 100, 400, 700 ms: a loop whose wrap segment has positive length.
const FX_B: Fx = Fx {
    dur_ms: 1000,
    t_ms: &[100, 400, 700],
    arm: &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
    spine_deg: &[0.0, 30.0, -20.0],
    smile: &[0.0, 1.0, 0.3],
};

/// 1000 ms one-shot, key at the start: the "new" motion of the transition tests.
const FX_N: Fx = Fx {
    dur_ms: 1000,
    t_ms: &[0, 600, 1000],
    arm: &[[3.0, 3.0, 0.0], [-2.0, 1.0, 1.0], [0.0, 0.0, 0.0]],
    spine_deg: &[10.0, -40.0, 0.0],
    smile: &[0.5, 0.0, 0.5],
};

/// 1000 ms one-shot: the second preemptor of the chained test.
const FX_C: Fx = Fx {
    dur_ms: 1000,
    t_ms: &[0, 500, 1000],
    arm: &[[-1.0, 2.0, 1.0], [2.0, -2.0, 0.0], [0.0, 0.0, 0.0]],
    spine_deg: &[-20.0, 30.0, 0.0],
    smile: &[0.2, 0.9, 0.0],
};

#[derive(Clone, Copy, Debug)]
enum Ch {
    Arm,
    Spine,
    Smile,
}
const CHS: [Ch; 3] = [Ch::Arm, Ch::Spine, Ch::Smile];

#[derive(Clone, Copy, Debug)]
struct Kin {
    /// Bound of the first derivative.
    v: f64,
    /// Bound of the second derivative.
    a: f64,
}

/// `(S, h_min)`: the largest chord slope over all components and the shortest chord.
fn chord_stats(times: &[f64], comps: &[Vec<f64>], period: Option<f64>) -> (f64, f64) {
    let n = times.len();
    let mut s = 0.0f64;
    let mut hmin = f64::INFINITY;
    for i in 1..n {
        let h = times[i] - times[i - 1];
        hmin = hmin.min(h);
        for c in comps {
            s = s.max((c[i] - c[i - 1]).abs() / h);
        }
    }
    if let Some(p) = period {
        let h = times[0] + p - times[n - 1];
        if h > 1.0e-9 {
            hmin = hmin.min(h);
            for c in comps {
                s = s.max((c[0] - c[n - 1]).abs() / h);
            }
        }
    }
    (s, hmin)
}

fn kin_from(times: &[f64], comps: &[Vec<f64>], period: Option<f64>) -> Kin {
    let (s, hmin) = chord_stats(times, comps, period);
    Kin {
        v: 3.5 * s,
        a: 14.0 * s / hmin,
    }
}

/// Bound of the signed angle of a Z-rotation channel from its `(sin, cos)` component bound.
fn kin_angle(k: Kin) -> Kin {
    let vn = 2.0f64.sqrt() * k.v;
    let an = 2.0f64.sqrt() * k.a;
    Kin {
        v: 2.0 * vn / RHO_MIN,
        a: 2.0 * an / RHO_MIN + 4.0 * vn * vn / (RHO_MIN * RHO_MIN),
    }
}

/// Bound of a preempted channel after `at`: `new` plus the quintic offset.
fn preempt_kin(new: Kin, x0: f64, v0: f64, t: f64) -> Kin {
    Kin {
        v: new.v + 1.875 * x0 / t + v0,
        a: new.a + 5.78 * x0 / (t * t) + 4.0 * v0 / t,
    }
}

fn kin_max(a: Kin, b: Kin) -> Kin {
    Kin {
        v: a.v.max(b.v),
        a: a.a.max(b.a),
    }
}

fn round(p: f64) -> f64 {
    4.0 * U * p / DT
}

impl Fx {
    fn clip(&self) -> Clip {
        let arm: Vec<(Duration, Vec3)> = self
            .t_ms
            .iter()
            .zip(self.arm.iter())
            .map(|(t, a)| (ms(*t), v3(*a)))
            .collect();
        let spine: Vec<(Duration, Quat)> = self
            .t_ms
            .iter()
            .zip(self.spine_deg.iter())
            .map(|(t, d)| (ms(*t), zq(*d)))
            .collect();
        let smile: Vec<(Duration, f32)> = self
            .t_ms
            .iter()
            .zip(self.smile.iter())
            .map(|(t, w)| (ms(*t), *w))
            .collect();
        Clip::new(
            "fixture",
            ms(self.dur_ms),
            vec![
                Channel {
                    target: ChannelTarget::JointTranslation("arm".into()),
                    keys: Keys::Translation(arm),
                },
                Channel {
                    target: ChannelTarget::JointRotation("spine".into()),
                    keys: Keys::Rotation(spine),
                },
                Channel {
                    target: ChannelTarget::MorphWeight("smile".into()),
                    keys: Keys::Weight(smile),
                },
            ],
        )
        .expect("fixture clip is valid")
    }

    fn motion(&self, repeat: Repeat) -> MotionClip {
        MotionClip::new(self.clip()).with_repeat(repeat)
    }

    fn playing(&self, repeat: Repeat, start_ms: u64) -> Playing {
        Playing::new(self.motion(repeat), ms(start_ms))
    }

    fn times_s(&self) -> Vec<f64> {
        self.t_ms.iter().map(|t| *t as f64 / 1000.0).collect()
    }

    fn kin(&self, ch: Ch, looped: bool) -> Kin {
        let per = if looped {
            Some(self.dur_ms as f64 / 1000.0)
        } else {
            None
        };
        let times = self.times_s();
        match ch {
            Ch::Arm => {
                let comps: Vec<Vec<f64>> = (0..3)
                    .map(|c| self.arm.iter().map(|a| f64::from(a[c])).collect())
                    .collect();
                kin_from(&times, &comps, per)
            }
            Ch::Smile => {
                let comps: Vec<Vec<f64>> = vec![self.smile.iter().map(|w| f64::from(*w)).collect()];
                kin_from(&times, &comps, per)
            }
            Ch::Spine => {
                let sin: Vec<f64> = self
                    .spine_deg
                    .iter()
                    .map(|d| (d.to_radians() * 0.5).sin())
                    .collect();
                let cos: Vec<f64> = self
                    .spine_deg
                    .iter()
                    .map(|d| (d.to_radians() * 0.5).cos())
                    .collect();
                kin_angle(kin_from(&times, &[sin, cos], per))
            }
        }
    }

    /// Largest magnitude of the channel (`P`).
    fn mag(&self, ch: Ch) -> f64 {
        match ch {
            Ch::Arm => self
                .arm
                .iter()
                .flat_map(|a| a.iter())
                .fold(0.0f64, |m, v| m.max(f64::from(v.abs()))),
            Ch::Smile => self
                .smile
                .iter()
                .fold(0.0f64, |m, v| m.max(f64::from(v.abs()))),
            Ch::Spine => PI,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Probes.
// ---------------------------------------------------------------------------------------------

fn tr(p: &MotionPose, joint: &str) -> (Vec3, Vec3) {
    match p.get(&ChannelTarget::JointTranslation(joint.into())) {
        Some(Sample::Vec3 { value, rate }) => (*value, *rate),
        other => panic!("no translation track {joint}: {other:?}"),
    }
}

fn rot(p: &MotionPose, joint: &str) -> (Quat, Vec3) {
    match p.get(&ChannelTarget::JointRotation(joint.into())) {
        Some(Sample::Rotation { value, omega }) => (*value, *omega),
        other => panic!("no rotation track {joint}: {other:?}"),
    }
}

fn wt(p: &MotionPose, morph: &str) -> (f32, f32) {
    match p.get(&ChannelTarget::MorphWeight(morph.into())) {
        Some(Sample::Scalar { value, rate }) => (*value, *rate),
        other => panic!("no weight track {morph}: {other:?}"),
    }
}

fn linf3(a: Vec3, b: Vec3) -> f64 {
    f64::from((a.x - b.x).abs())
        .max(f64::from((a.y - b.y).abs()))
        .max(f64::from((a.z - b.z).abs()))
}

fn probe(m: &dyn Motion, ch: Ch, t: Duration) -> Vec<f64> {
    let p = m.sample(t);
    match ch {
        Ch::Arm => {
            let (v, _) = tr(&p, "arm");
            vec![f64::from(v.x), f64::from(v.y), f64::from(v.z)]
        }
        Ch::Spine => vec![angle(rot(&p, "spine").0)],
        Ch::Smile => vec![f64::from(wt(&p, "smile").0)],
    }
}

/// Largest finite-difference velocity jump over the 1 ms grid `[from_ms, to_ms]`, and where.
fn max_velocity_jump(m: &dyn Motion, ch: Ch, from_ms: u64, to_ms: u64) -> (f64, u64) {
    let pts: Vec<Vec<f64>> = (from_ms..=to_ms).map(|k| probe(m, ch, ms(k))).collect();
    let mut best = (0.0f64, from_ms);
    for k in 1..pts.len() - 1 {
        let mut j = 0.0f64;
        for c in 0..pts[k].len() {
            let v0 = (pts[k][c] - pts[k - 1][c]) / DT;
            let v1 = (pts[k + 1][c] - pts[k][c]) / DT;
            j = j.max((v1 - v0).abs());
        }
        if j > best.0 {
            best = (j, from_ms + k as u64);
        }
    }
    best
}

fn assert_c1(m: &dyn Motion, ch: Ch, from_ms: u64, to_ms: u64, a: f64, p: f64, what: &str) {
    let (jump, at) = max_velocity_jump(m, ch, from_ms, to_ms);
    let bound = a * DT + round(p);
    assert!(
        jump <= bound,
        "{what} {ch:?}: velocity jump {jump} at {at} ms exceeds the derived bound {bound}"
    );
}

// ---------------------------------------------------------------------------------------------
// 1. Continuous key interpolation.
// ---------------------------------------------------------------------------------------------

/// Hand-computed Catmull-Rom values and rates. Keys `(0, 0), (500, 1), (1000, 3)`, clamped ends
/// (tangent 0), middle tangent `(3 - 0) / 1.0 s = 3`. Segment 0 (h = 0.5 s):
/// `p(s) = -0.5 s^3 + 1.5 s^2`, `p'(t) = (-1.5 s^2 + 3 s) / 0.5`. Segment 1:
/// `p(0.5) = 0.5 + 0.125 * 0.5 * 3 + 0.5 * 3 = 2.1875`, `p' = 2.625 / 0.5 = 5.25`.
/// Every value is dyadic, so f32 evaluation is exact up to 1e-6.
/// Kills: L (linear would give 0.5 and rate 2.0 at t = 250 ms).
#[test]
fn smooth_matches_hand_computed_catmull_rom() {
    let clip = Clip::new(
        "hand",
        ms(1000),
        vec![Channel {
            target: ChannelTarget::MorphWeight("smile".into()),
            keys: Keys::Weight(vec![(ms(0), 0.0), (ms(500), 1.0), (ms(1000), 3.0)]),
        }],
    )
    .unwrap();
    let m = MotionClip::new(clip);
    let at = |t: u64| wt(&m.sample_at(ms(t)), "smile");
    let close = |a: f32, b: f32| (a - b).abs() <= 1.0e-5;
    let (v, r) = at(125);
    assert!(close(v, 0.0859375) && close(r, 1.3125), "t=125: {v} {r}");
    let (v, r) = at(250);
    assert!(close(v, 0.3125) && close(r, 2.25), "t=250: {v} {r}");
    let (v, r) = at(500);
    assert!(close(v, 1.0) && close(r, 3.0), "t=500: {v} {r}");
    let (v, r) = at(750);
    assert!(close(v, 2.1875) && close(r, 5.25), "t=750: {v} {r}");
    // C1 at the interior key: the rate just before equals the rate at the key.
    let (_, before) = wt(&m.sample_at(Duration::from_micros(499_999)), "smile");
    assert!(
        (before - 3.0).abs() <= 1.0e-3,
        "rate just before the key {before}"
    );
}

/// Values at keys are exactly the authored values and the one-shot clip holds with zero rate
/// before the first and after the last key (clamped end tangents).
#[test]
fn smooth_is_exact_at_keys_and_holds_with_zero_rate() {
    let fx = &FX_B;
    let m = fx.motion(Repeat::Once);
    for (i, t) in fx.t_ms.iter().enumerate() {
        let p = m.sample_at(ms(*t));
        assert_eq!(tr(&p, "arm").0, v3(fx.arm[i]), "arm at key {i}");
        assert_eq!(rot(&p, "spine").0, zq(fx.spine_deg[i]), "spine at key {i}");
        assert_eq!(wt(&p, "smile").0, fx.smile[i], "smile at key {i}");
    }
    // Before the first key (100 ms) and from the last key (700 ms) on: hold, zero rate.
    for t in [0u64, 50, 700, 900, 1000, 5000] {
        let p = m.sample_at(ms(t));
        let held = if t < 700 { fx.arm[0] } else { fx.arm[2] };
        assert_eq!(tr(&p, "arm").0, v3(held), "arm held at {t}");
        assert_eq!(
            tr(&p, "arm").1,
            Vec3::default(),
            "arm rate while holding at {t}"
        );
        assert_eq!(wt(&p, "smile").1, 0.0, "smile rate while holding at {t}");
        assert_eq!(
            rot(&p, "spine").1,
            Vec3::default(),
            "spine omega while holding"
        );
    }
}

/// Smooth keys: no velocity jump anywhere, across every key boundary and into the end hold.
/// Kills: L (linear keys jump by the change of chord slope at each key).
#[test]
fn smooth_one_shot_velocity_is_continuous() {
    let m = FX_A.playing(Repeat::Once, 0);
    for ch in CHS {
        assert_c1(
            &m,
            ch,
            0,
            2500,
            FX_A.kin(ch, false).a,
            FX_A.mag(ch),
            "one-shot",
        );
    }
}

/// Looping: velocity is continuous across every loop seam (three periods), including a loop
/// whose wrap segment has positive length (fixture B) and one whose first and last key coincide
/// (fixture A, wrap tangents).
/// Kills: L.
#[test]
fn loop_seam_velocity_is_continuous() {
    for (fx, name, end) in [(&FX_A, "A", 6000u64), (&FX_B, "B", 3000u64)] {
        let m = fx.playing(Repeat::Loop, 0);
        for ch in CHS {
            assert_c1(&m, ch, 0, end, fx.kin(ch, true).a, fx.mag(ch), name);
        }
    }
}

/// The reported analytic rate matches a central difference of the reported value, on every
/// channel kind, one-shot and looped. Central difference error is at most `A dt / 2` (the
/// velocity is `A`-Lipschitz); f32 evaluation of the rate adds `RATE_ROUND`.
/// `RATE_ROUND = 1e-3` is declared: the rate is about 64 f32 operations on magnitudes up to
/// `V <= 25`, `64 * u * 25 = 1e-4`, ten times margin. The FD value roundoff is `u P / dt`.
/// Kills: none of the three mutants by itself (a linear segment's central difference equals its
/// slope); it pins the analytic rate to the value, which the preemption tests rely on.
#[test]
fn reported_rate_matches_finite_difference() {
    const RATE_ROUND: f64 = 1.0e-3;
    for (fx, repeat, looped) in [
        (&FX_A, Repeat::Once, false),
        (&FX_A, Repeat::Loop, true),
        (&FX_B, Repeat::Loop, true),
    ] {
        let m = fx.playing(repeat, 0);
        let mut t = 40u64;
        while t < 1900 {
            let p = m.sample(ms(t));
            for ch in CHS {
                let a = ms(t + 1);
                let b = ms(t - 1);
                let fd: Vec<f64> = probe(&m, ch, a)
                    .iter()
                    .zip(probe(&m, ch, b).iter())
                    .map(|(x, y)| (x - y) / (2.0 * DT))
                    .collect();
                let reported: Vec<f64> = match ch {
                    Ch::Arm => {
                        let r = tr(&p, "arm").1;
                        vec![f64::from(r.x), f64::from(r.y), f64::from(r.z)]
                    }
                    Ch::Spine => vec![f64::from(rot(&p, "spine").1.z)],
                    Ch::Smile => vec![f64::from(wt(&p, "smile").1)],
                };
                let tol = fx.kin(ch, looped).a * DT / 2.0 + RATE_ROUND + U * fx.mag(ch) / DT;
                for (r, f) in reported.iter().zip(fd.iter()) {
                    assert!(
                        (r - f).abs() <= tol,
                        "{ch:?} at {t} ms: reported {r}, central difference {f}, tol {tol}"
                    );
                }
            }
            t += 37;
        }
    }
}

/// The looped clip is exactly periodic in presentation time (integer-nanosecond wrap).
#[test]
fn looped_clip_is_exactly_periodic() {
    for fx in [&FX_A, &FX_B] {
        let m = fx.playing(Repeat::Loop, 0);
        for t in [0u64, 50, 99, 100, 333, 700, 999] {
            let base = m.sample(ms(t));
            for k in 1..4u64 {
                assert_eq!(base, m.sample(ms(t + k * fx.dur_ms)), "t={t} k={k}");
            }
        }
    }
}

/// The reported rate agrees across the seam (1 microsecond either side), for a loop whose first
/// and last key coincide (A: the seam is a key) and one with a positive-length wrap segment (B).
/// Kills: L (at the seam of A the linear rate flips from the last chord slope to the first).
#[test]
fn loop_seam_reported_rate_is_continuous() {
    for fx in [&FX_A, &FX_B] {
        let m = fx.playing(Repeat::Loop, 0);
        let before = m.sample(Duration::from_micros(fx.dur_ms * 1000 - 1));
        let after = m.sample(ms(fx.dur_ms));
        let tol = fx.kin(Ch::Arm, true).a * 1.0e-6 + 1.0e-3;
        assert!(linf3(tr(&before, "arm").1, tr(&after, "arm").1) <= tol);
        assert!(f64::from((wt(&before, "smile").1 - wt(&after, "smile").1).abs()) <= tol);
        let dz = f64::from((rot(&before, "spine").1.z - rot(&after, "spine").1.z).abs());
        assert!(dz <= tol * 10.0, "spine omega across the seam: {dz}");
    }
}

/// Linear channels reproduce the existing `Clip::sample` (step 1 behaviour unchanged), and
/// `Clip::sample` itself is untouched.
#[test]
fn linear_mode_matches_existing_clip_sample() {
    let clip = FX_A.clip();
    let m = MotionClip::new(clip.clone())
        .with_interpolation(0, Interpolation::Linear)
        .unwrap()
        .with_interpolation(1, Interpolation::Linear)
        .unwrap()
        .with_interpolation(2, Interpolation::Linear)
        .unwrap();
    for t in [0u64, 123, 500, 777, 1000, 1999, 2000, 2400] {
        let old = clip.sample(ms(t));
        let new = m.sample_at(ms(t));
        let (v, _) = tr(&new, "arm");
        assert!(linf3(v, old.translations[0].1) <= 1.0e-6, "arm at {t}");
        let (q, _) = rot(&new, "spine");
        let o = old.rotations[0].1;
        let dot = f64::from(q.x * o.x + q.y * o.y + q.z * o.z + q.w * o.w);
        assert!(dot.abs() >= 1.0 - 1.0e-6, "spine at {t}: dot {dot}");
        assert!(
            (wt(&new, "smile").0 - old.weights[0].1).abs() <= 1.0e-6,
            "smile at {t}"
        );
    }
}

#[test]
fn motion_clip_rejects_an_unknown_channel() {
    let r = MotionClip::new(FX_A.clip()).with_interpolation(3, Interpolation::Linear);
    assert_eq!(r.unwrap_err(), MotionError::UnknownChannel { channel: 3 });
}

// ---------------------------------------------------------------------------------------------
// 2a. Crossfade.
// ---------------------------------------------------------------------------------------------

fn crossfade() -> Crossfade<Playing, Playing> {
    Crossfade::new(
        FX_A.playing(Repeat::Once, 0),
        FX_N.playing(Repeat::Once, 600),
        ms(600),
        ms(500),
    )
}

/// A crossfade over `ease` has continuous velocity everywhere, including at the start and end of
/// the blend. `D = |B - A| <= P_A + P_B` (angles: `2 pi`).
/// Kills: S (the snap makes the value, hence the finite-difference velocity, jump at 600 ms).
#[test]
fn crossfade_velocity_is_continuous() {
    let cf = crossfade();
    let e = 0.5;
    for ch in CHS {
        let (ka, kb) = (FX_A.kin(ch, false), FX_N.kin(ch, false));
        let d = match ch {
            Ch::Spine => 2.0 * PI,
            _ => FX_A.mag(ch) + FX_N.mag(ch),
        };
        let a = ka.a.max(kb.a) + 2.0 * 1.875 / e * (ka.v + kb.v) + 5.78 / (e * e) * d;
        assert_c1(
            &cf,
            ch,
            0,
            1800,
            a,
            FX_A.mag(ch).max(FX_N.mag(ch)),
            "crossfade",
        );
    }
}

/// Exact blend facts: before the start it is `from`; at the start it is `from` and its rate
/// matches (`w'(0) = 0`); at `x = 1/2` the smootherstep weight is exactly 1/2 so the value is the
/// mean; at the end it is `to` bit for bit, with matching rate.
/// Kills: S.
#[test]
fn crossfade_blends_exactly_over_ease() {
    let cf = crossfade();
    let from = || FX_A.playing(Repeat::Once, 0);
    let to = || FX_N.playing(Repeat::Once, 600);
    assert_eq!(cf.sample(ms(300)), from().sample(ms(300)));
    let at0 = cf.sample(ms(600));
    let f0 = from().sample(ms(600));
    assert!(linf3(tr(&at0, "arm").0, tr(&f0, "arm").0) <= 1.0e-5);
    assert!(linf3(tr(&at0, "arm").1, tr(&f0, "arm").1) <= 1.0e-3);
    let mid = cf.sample(ms(850));
    let (a, b) = (from().sample(ms(850)), to().sample(ms(850)));
    let (va, vb) = (tr(&a, "arm").0, tr(&b, "arm").0);
    let mean = Vec3 {
        x: 0.5 * (va.x + vb.x),
        y: 0.5 * (va.y + vb.y),
        z: 0.5 * (va.z + vb.z),
    };
    assert!(
        linf3(tr(&mid, "arm").0, mean) <= 1.0e-5,
        "midpoint is the mean"
    );
    let end = cf.sample(ms(1100));
    assert_eq!(end, to().sample(ms(1100)));
    assert_eq!(cf.sample(ms(1400)), to().sample(ms(1400)));
}

// ---------------------------------------------------------------------------------------------
// 2b. Preemption by inertialization.
// ---------------------------------------------------------------------------------------------

const T0: u64 = 750;
const EASE: u64 = 400;

fn old_motion() -> Playing {
    FX_A.playing(Repeat::Once, 0)
}
fn new_motion(start: u64) -> Playing {
    FX_N.playing(Repeat::Once, start)
}
fn preempt(new_start: u64) -> Preempt<Playing, Playing> {
    Preempt::new(old_motion(), new_motion(new_start), ms(T0), ms(EASE))
}

fn max_comp3(v: Vec3) -> f64 {
    f64::from(v.x.abs())
        .max(f64::from(v.y.abs()))
        .max(f64::from(v.z.abs()))
}

/// Preempting WHILE MOVING preserves position and velocity at the preemption instant. The old
/// arm is moving at `T0` (speed above four times the bound, asserted, so a velocity loss is
/// detectable at all). Three bearings: the reported rate equals the old rate; the finite
/// difference just before equals the one just after within the derived bound; the position is
/// continuous.
/// Kills: F (rate drops to 0), S (the position jumps to the new clip).
#[test]
fn preempt_while_moving_preserves_position_and_velocity() {
    let pre = preempt(T0);
    let po = old_motion().sample(ms(T0));
    let pp = pre.sample(ms(T0));
    let (vo, ro) = tr(&po, "arm");
    let (vp, rp) = tr(&pp, "arm");
    // value: new + (old - new) is within three roundings of 3.5, 3 * u * 3.5 = 6e-7.
    assert!(linf3(vo, vp) <= 1.0e-5, "position at t0: {vo:?} vs {vp:?}");
    // rate: same arithmetic on rates up to 10; declared 1e-3 is far above 3 * u * 10.
    assert!(
        linf3(ro, rp) <= 1.0e-3,
        "reported rate at t0: {ro:?} vs {rp:?}"
    );

    let (ko, kn) = (FX_A.kin(Ch::Arm, false), FX_N.kin(Ch::Arm, false));
    let x0 = linf3(vo, tr(&new_motion(T0).sample(ms(T0)), "arm").0);
    let comb = preempt_kin(kn, x0, ko.v + kn.v, EASE as f64 / 1000.0);
    let bound = ko.a.max(comb.a) * DT + round(FX_A.mag(Ch::Arm));
    let f = |k: u64| probe(&pre, Ch::Arm, ms(k));
    let (p0, p1, p2) = (f(T0 - 1), f(T0), f(T0 + 1));
    let mut jump = 0.0f64;
    let mut speed = 0.0f64;
    for c in 0..3 {
        let before = (p1[c] - p0[c]) / DT;
        let after = (p2[c] - p1[c]) / DT;
        jump = jump.max((after - before).abs());
        speed = speed.max(before.abs());
    }
    assert!(
        speed > 4.0 * bound,
        "premise: the old arm must be moving (speed {speed}, bound {bound})"
    );
    assert!(
        jump <= bound,
        "velocity jump {jump} across t0 exceeds {bound}"
    );
}

/// The same property for the rotation and morph channels, by the reported rate and value.
/// Kills: F, S.
#[test]
fn preempt_preserves_rotation_and_weight_rate_at_t0() {
    let pre = preempt(T0);
    let po = old_motion().sample(ms(T0));
    let pp = pre.sample(ms(T0));
    let (qo, wo) = rot(&po, "spine");
    let (qp, wp) = rot(&pp, "spine");
    assert!((angle(qo) - angle(qp)).abs() <= 1.0e-5, "spine angle");
    assert!(f64::from((wo.z - wp.z).abs()) <= 1.0e-3, "spine omega");
    assert!(f64::from(wo.z.abs()) > 0.5, "premise: the spine is turning");
    let (so, ro) = wt(&po, "smile");
    let (sp, rp) = wt(&pp, "smile");
    assert!((so - sp).abs() <= 1.0e-5, "smile value");
    assert!((ro - rp).abs() <= 1.0e-3, "smile rate");
    assert!(ro.abs() > 0.5, "premise: the smile weight is changing");
}

/// The offset reaches exactly zero at `t0 + ease`: the pose is then bit for bit the new motion,
/// and stays so. One millisecond before, the offset and its rate are already negligible
/// (`h0, h1 ~ (1 - x)^3`). Before `t0` the pose is the old motion bit for bit.
/// Kills: S (the early blend values differ), F (the freeze leaves a residual only if the weight
/// were wrong; the exact-equality assertions pin the end state).
#[test]
fn preempt_offset_reaches_exactly_zero_and_poses_follow_the_new_clip() {
    let pre = preempt(T0);
    for t in [T0 + EASE, T0 + EASE + 1, T0 + EASE + 250, 2000, 9000] {
        assert_eq!(pre.sample(ms(t)), new_motion(T0).sample(ms(t)), "t={t}");
    }
    let near = pre.sample(ms(T0 + EASE - 1));
    let target = new_motion(T0).sample(ms(T0 + EASE - 1));
    // At x = 1 - 1/400 = 0.9975, e = 1 - x = 2.5e-3: `|h0| <= 10 e^3 = 1.6e-7`,
    // `|h0'| <= 30 e^2 = 1.9e-4` and `h1, h1' = O(e^3)`. With `|x0| <= 5` and `T = 0.4`
    // the value offset is under 1e-6 and the rate offset under `1.9e-4 * 5 / 0.4 = 2.3e-3`.
    assert!(linf3(tr(&near, "arm").0, tr(&target, "arm").0) <= 1.0e-5);
    assert!(linf3(tr(&near, "arm").1, tr(&target, "arm").1) <= 5.0e-3);
    for t in [0u64, 100, 400, 749] {
        assert_eq!(pre.sample(ms(t)), old_motion().sample(ms(t)), "t={t}");
    }
}

/// The offset is the quintic `x0 h0 + v0 T h1`, checked at `x = 1/2` against independent central
/// differences of the old and new motions: `h0(1/2) = 1/2`, `h1(1/2) = 5/32`, so
/// `offset = x0 / 2 + (5/32) v0 T`. The new clip started at 600 ms so both rates are non-zero at
/// `T0`. Tolerance `TOL_CF = 2e-4`: finite-difference truncation `(dt^2 / 6) (P3_old + P3_new)`
/// with `P3 = 24 S / h^2` (576 and 1250) is 3e-4 on `v0`, times `T h1 = 0.0625` is 1.9e-5;
/// roundoff of the two central differences `u P / dt = 2.4e-4` each, times 0.0625, is 3e-5;
/// total under 5e-5, so 2e-4 is four times above it and 3700 times below the freeze error (0.74).
/// Kills: F, S.
#[test]
fn preempt_matches_the_quintic_closed_form_midway() {
    const TOL_CF: f64 = 2.0e-4;
    let new_start = 600;
    let pre = preempt(new_start);
    let tm = T0 + EASE / 2;
    let tt = EASE as f64 / 1000.0;
    let got = pre.sample(ms(tm));
    let base = new_motion(new_start).sample(ms(tm));
    let (old, new) = (old_motion(), new_motion(new_start));
    let cd = |m: &Playing, ch: Ch| -> (Vec<f64>, Vec<f64>) {
        let at = probe(m, ch, ms(T0));
        let v: Vec<f64> = probe(m, ch, ms(T0 + 1))
            .iter()
            .zip(probe(m, ch, ms(T0 - 1)).iter())
            .map(|(a, b)| (a - b) / (2.0 * DT))
            .collect();
        (at, v)
    };
    for ch in [Ch::Arm, Ch::Smile] {
        let (xo, vo) = cd(&old, ch);
        let (xn, vn) = cd(&new, ch);
        let g = probe(&pre, ch, ms(tm));
        let b = probe(&new_motion(new_start), ch, ms(tm));
        for c in 0..g.len() {
            let x0 = xo[c] - xn[c];
            let v0 = vo[c] - vn[c];
            let predicted = 0.5 * x0 + (5.0 / 32.0) * v0 * tt;
            let offset = g[c] - b[c];
            assert!(
                (offset - predicted).abs() <= TOL_CF,
                "{ch:?}[{c}]: offset {offset}, quintic {predicted}"
            );
        }
    }
    assert_ne!(got, base, "the offset must be non-trivial midway");
}

/// Velocity is C1 over the whole preempted timeline: before, across `t0`, through the blend, and
/// at its end.
/// Kills: S, F (the freeze drops the old velocity at `t0`).
#[test]
fn preempt_timeline_is_c1() {
    let pre = preempt(T0);
    let t = EASE as f64 / 1000.0;
    for ch in CHS {
        let (ko, kn) = (FX_A.kin(ch, false), FX_N.kin(ch, false));
        let x0 = match ch {
            Ch::Spine => PI,
            _ => FX_A.mag(ch) + FX_N.mag(ch),
        };
        let comb = preempt_kin(kn, x0, ko.v + kn.v, t);
        assert_c1(
            &pre,
            ch,
            0,
            2000,
            ko.a.max(comb.a),
            FX_A.mag(ch).max(FX_N.mag(ch)),
            "preempt",
        );
    }
}

/// Preempting a preemption that is still blending stays C1 (the old motion is itself a
/// transition). The second preemptor starts at 900 ms, inside the first blend `[750, 1150)`.
/// Kills: S, F.
#[test]
fn chained_preempt_is_c1_and_keeps_velocity() {
    let first = || Preempt::new(old_motion(), new_motion(T0), ms(T0), ms(EASE));
    let second = Preempt::new(first(), FX_C.playing(Repeat::Once, 900), ms(900), ms(EASE));
    let t = EASE as f64 / 1000.0;
    let ch = Ch::Arm;
    let (ka, kn, kc) = (
        FX_A.kin(ch, false),
        FX_N.kin(ch, false),
        FX_C.kin(ch, false),
    );
    let x0a = linf3(
        tr(&old_motion().sample(ms(T0)), "arm").0,
        tr(&new_motion(T0).sample(ms(T0)), "arm").0,
    );
    let kfirst = kin_max(ka, preempt_kin(kn, x0a, ka.v + kn.v, t));
    let x0b = linf3(
        tr(&first().sample(ms(900)), "arm").0,
        tr(&FX_C.playing(Repeat::Once, 900).sample(ms(900)), "arm").0,
    );
    let ktotal = kin_max(kfirst, preempt_kin(kc, x0b, kfirst.v + kc.v, t));
    assert_c1(&second, ch, 0, 2200, ktotal.a, 4.0, "chained preempt");
    // Reported rate continuity at the second preemption instant.
    let before = first().sample(ms(900));
    let after = second.sample(ms(900));
    assert!(linf3(tr(&before, "arm").0, tr(&after, "arm").0) <= 1.0e-5);
    assert!(linf3(tr(&before, "arm").1, tr(&after, "arm").1) <= 1.0e-3);
}

/// Preemption of a rotation whose offset is a large rotation about a tilted axis: the pose and
/// the reported angular velocity are continuous at `t0` (this exercises the SO(3) Jacobian, not
/// only a single axis). Tolerance 2e-3: about 64 f32 operations on `|omega| <= 10` amplified by
/// `|J^-1| <= pi / 2`, `64 * u * 10 * 1.6 = 6e-5`, thirty times margin.
/// Kills: F (omega drops to 0), S (the rotation jumps).
#[test]
fn preempt_rotation_about_a_tilted_axis_keeps_pose_and_angular_velocity() {
    let axis_a = Vec3 {
        x: 1.0,
        y: 2.0,
        z: 2.0,
    };
    let axis_b = Vec3 {
        x: -2.0,
        y: 1.0,
        z: 2.0,
    };
    let rot_clip = |axis: Vec3, degs: [f64; 3]| {
        let keys: Vec<(Duration, Quat)> = [0u64, 500, 1000]
            .iter()
            .zip(degs.iter())
            .map(|(t, d)| (ms(*t), Quat::from_axis_angle(axis, d.to_radians() as f32)))
            .collect();
        MotionClip::new(
            Clip::new(
                "tilt",
                ms(1000),
                vec![Channel {
                    target: ChannelTarget::JointRotation("head".into()),
                    keys: Keys::Rotation(keys),
                }],
            )
            .unwrap(),
        )
    };
    let old = Playing::new(rot_clip(axis_a, [0.0, 100.0, 60.0]), ms(0));
    let new = Playing::new(rot_clip(axis_b, [-80.0, 120.0, 10.0]), ms(250));
    let pre = Preempt::new(
        Playing::new(rot_clip(axis_a, [0.0, 100.0, 60.0]), ms(0)),
        Playing::new(rot_clip(axis_b, [-80.0, 120.0, 10.0]), ms(250)),
        ms(250),
        ms(400),
    );
    let (qo, wo) = rot(&old.sample(ms(250)), "head");
    let (qn, _) = rot(&new.sample(ms(250)), "head");
    let (qp, wp) = rot(&pre.sample(ms(250)), "head");
    let dot = |a: Quat, b: Quat| f64::from(a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w).abs();
    assert!(
        dot(qo, qn) < 0.9,
        "premise: the two rotations must differ by a large offset"
    );
    assert!(
        dot(qo, qp) >= 1.0 - 1.0e-5,
        "rotation at t0: dot {}",
        dot(qo, qp)
    );
    let wd = Vec3 {
        x: wo.x - wp.x,
        y: wo.y - wp.y,
        z: wo.z - wp.z,
    };
    assert!(max_comp3(wd) <= 2.0e-3, "omega at t0: {wo:?} vs {wp:?}");
    assert!(wo.length() > 0.5, "premise: the old rotation is turning");
    // And it ends exactly on the new clip.
    assert_eq!(pre.sample(ms(650)), new.sample(ms(650)));
}

/// `ease = 0` is the documented static (reduced-motion) realization: an immediate switch.
#[test]
fn preempt_with_zero_ease_switches_immediately() {
    let pre = Preempt::new(old_motion(), new_motion(T0), ms(T0), Duration::ZERO);
    assert_eq!(pre.sample(ms(T0)), new_motion(T0).sample(ms(T0)));
    assert_eq!(pre.sample(ms(T0 - 1)), old_motion().sample(ms(T0 - 1)));
}

/// A target only the old motion animates blends to the supplied rest value and then leaves the
/// pose; a target only the new one animates blends in from rest. No snap either way.
/// Kills: S.
#[test]
fn preempt_blends_targets_present_on_one_side_through_rest() {
    let only_smile = MotionClip::new(
        Clip::new(
            "smile_only",
            ms(1000),
            vec![Channel {
                target: ChannelTarget::MorphWeight("smile".into()),
                keys: Keys::Weight(vec![(ms(0), 0.5)]),
            }],
        )
        .unwrap(),
    );
    let rest = MotionPose::new(vec![(
        ChannelTarget::JointTranslation("arm".into()),
        Sample::Vec3 {
            value: Vec3 {
                x: 0.5,
                y: 0.5,
                z: 0.5,
            },
            rate: Vec3::default(),
        },
    )]);
    let pre = Preempt::new(
        old_motion(),
        Playing::new(only_smile, ms(T0)),
        ms(T0),
        ms(EASE),
    )
    .with_rest(rest);
    let (vo, _) = tr(&old_motion().sample(ms(T0)), "arm");
    let (v0, _) = tr(&pre.sample(ms(T0)), "arm");
    assert!(linf3(vo, v0) <= 1.0e-5, "arm continuous at t0");
    let (vend, _) = tr(&pre.sample(ms(T0 + EASE - 1)), "arm");
    let rest_v = Vec3 {
        x: 0.5,
        y: 0.5,
        z: 0.5,
    };
    assert!(
        linf3(vend, rest_v) <= 1.0e-4,
        "arm has decayed to rest: {vend:?}"
    );
    assert!(
        pre.sample(ms(T0 + EASE))
            .get(&ChannelTarget::JointTranslation("arm".into()))
            .is_none(),
        "after the blend only the new clip's targets remain"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Look retarget.
// ---------------------------------------------------------------------------------------------

const FORWARD: Vec3 = Vec3 {
    x: 0.0,
    y: 0.0,
    z: 1.0,
};
const LOOK_EASE: u64 = 400;

fn look() -> Look {
    Look::new("head", FORWARD, FORWARD, ms(LOOK_EASE))
        .unwrap()
        .retarget(
            ms(500),
            Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
        )
        .unwrap()
        .retarget(
            ms(700),
            Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        )
        .unwrap()
}

fn aim(l: &Look, t: Duration) -> Vec3 {
    let (q, _) = rot(&l.sample(t), "head");
    q.to_mat().transform_point(FORWARD)
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    f64::from(a.sub(b).length())
}

/// No jump at the first retarget instant: just before (1 ns) and at the instant the aim is the
/// same within f32 rounding; and one millisecond later it has moved by at most the angular speed
/// bound times dt: `|r'| <= pi * 1.875 / T` (the offset starts at most `pi` away, from rest).
/// Kills: S (the aim jumps by `sqrt 2`).
#[test]
fn look_retarget_has_no_jump_at_the_retarget_instant() {
    let l = look();
    let before = aim(&l, Duration::from_nanos(499_999_999));
    let at = aim(&l, ms(500));
    assert!(
        dist(before, at) <= 1.0e-5,
        "jump at the retarget: {before:?} -> {at:?}"
    );
    let r1 = PI * 1.875 / (LOOK_EASE as f64 / 1000.0);
    let next = aim(&l, ms(501));
    assert!(
        dist(at, next) <= r1 * DT + 1.0e-5,
        "moved {} in 1 ms",
        dist(at, next)
    );
    assert!(dist(at, next) > 0.0, "premise: it does move afterwards");
}

/// A retarget in the middle of an earlier retarget's blend is inertialized too: the aim
/// direction has no velocity jump anywhere in `[0, 2000]` ms, and the reported angular velocity
/// is continuous across the second retarget instant (1 microsecond either side; tolerance 2e-3 =
/// `alpha_max * 1 us` with `alpha <= 345 rad/s^2` is 3.5e-4, plus f32 rounding of `|omega| <= 38`
/// through `J^-1`, `64 * u * 38 * 1.6 = 2.3e-4`).
///
/// Bound. `d = R f` gives `|d''| <= |omega'| + |omega|^2`, `omega = J(r) r'` with `|J| <= 1` and
/// `|dJ/dt| <= |r'| / 2` (the model checks both), so `|d''| <= R2 + 1.5 R1^2` where `R1, R2` bound
/// `|r'|, |r''|` of the offset rotation vector. First blend from rest: `R1a = pi 1.875 / T`.
/// Second blend: `|x0| <= pi`, `|w0| <= (pi/2) |omega_old| <= (pi/2) R1a`
/// (`|J^-1| <= (theta/2) / sin(theta/2) <= pi/2`), so
/// `R1b = R1a + (pi/2) R1a` and `R2b = pi 5.78 / T^2 + (pi/2) R1a 4 / T`.
/// Kills: S (the aim jumps), F (the second retarget drops the first blend's angular velocity).
#[test]
fn look_retarget_mid_motion_is_c1() {
    let l = look();
    let t = LOOK_EASE as f64 / 1000.0;
    let r1a = PI * 1.875 / t;
    let r1b = r1a + (PI / 2.0) * r1a;
    let r2b = PI * 5.78 / (t * t) + (PI / 2.0) * r1a * 4.0 / t;
    let bound = (r2b + 1.5 * r1b * r1b) * DT + round(1.0);
    let pts: Vec<Vec3> = (0..=2000u64).map(|k| aim(&l, ms(k))).collect();
    let mut jump = 0.0f64;
    let mut at = 0usize;
    for k in 1..pts.len() - 1 {
        let v0 = pts[k].sub(pts[k - 1]).scale((1.0 / DT) as f32);
        let v1 = pts[k + 1].sub(pts[k]).scale((1.0 / DT) as f32);
        let j = f64::from(v1.sub(v0).length());
        if j > jump {
            jump = j;
            at = k;
        }
    }
    assert!(
        jump <= bound,
        "aim velocity jump {jump} at {at} ms exceeds {bound}"
    );

    let (_, w_before) = rot(&l.sample(Duration::from_micros(699_999)), "head");
    let (_, w_after) = rot(&l.sample(ms(700)), "head");
    assert!(
        w_before.length() > 1.0,
        "premise: the aim is turning at the second retarget"
    );
    assert!(
        max_comp3(w_before.sub(w_after)) <= 2.0e-3,
        "angular velocity across the retarget: {w_before:?} -> {w_after:?}"
    );
}

/// The aim converges on the commanded direction, and from `last retarget + ease` on it is
/// constant, bit for bit.
#[test]
fn look_converges_to_the_aimed_direction() {
    let l = look();
    let target = Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    for t in [1100u64, 1500, 3000, 60_000] {
        assert!(dist(aim(&l, ms(t)), target) <= 1.0e-5, "aim at {t} ms");
    }
    assert_eq!(l.sample(ms(1100)), l.sample(ms(60_000)));
    // Part way, it is strictly between the two commanded directions' endpoints.
    let mid = aim(&l, ms(900));
    assert!(dist(mid, target) > 1.0e-3);
    assert!(
        (mid.length() - 1.0).abs() <= 1.0e-5,
        "aim stays a unit vector"
    );
}

/// An antipodal command has no unique shortest arc; the choice is deterministic and the aim
/// still converges.
#[test]
fn look_handles_an_antipodal_target() {
    let l = Look::new("head", FORWARD, FORWARD, ms(300))
        .unwrap()
        .retarget(
            ms(100),
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
        )
        .unwrap();
    let end = aim(&l, ms(1000));
    assert!(
        dist(
            end,
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: -1.0
            }
        ) <= 1.0e-5
    );
    assert_eq!(l.sample(ms(250)), l.sample(ms(250)));
    let (q, _) = rot(&l.sample(ms(250)), "head");
    assert!((f64::from(q.length_squared()) - 1.0).abs() <= 1.0e-5);
}

#[test]
fn look_rejects_bad_directions_and_unordered_retargets() {
    let zero = Vec3::default();
    let nan = Vec3 {
        x: f32::NAN,
        y: 0.0,
        z: 1.0,
    };
    assert_eq!(
        Look::new("h", zero, FORWARD, ms(100)).unwrap_err(),
        MotionError::BadDirection
    );
    assert_eq!(
        Look::new("h", FORWARD, nan, ms(100)).unwrap_err(),
        MotionError::BadDirection
    );
    let l = Look::new("h", FORWARD, FORWARD, ms(100)).unwrap();
    assert_eq!(
        l.clone().retarget(ms(10), zero).unwrap_err(),
        MotionError::BadDirection
    );
    let l = l.retarget(ms(200), FORWARD).unwrap();
    assert_eq!(
        l.clone().retarget(ms(200), FORWARD).unwrap_err(),
        MotionError::RetargetNotIncreasing
    );
    assert_eq!(
        l.retarget(ms(100), FORWARD).unwrap_err(),
        MotionError::RetargetNotIncreasing
    );
}

/// The look layer replaces only the aimed joint of a body pose.
#[test]
fn look_overlays_only_its_joint() {
    let body = FX_A.playing(Repeat::Loop, 0).sample(ms(300));
    let head = look().sample(ms(900));
    let both = body.overlay(&head);
    assert_eq!(tr(&both, "arm"), tr(&body, "arm"));
    assert_eq!(wt(&both, "smile"), wt(&body, "smile"));
    assert_eq!(rot(&both, "spine"), rot(&body, "spine"));
    assert_eq!(rot(&both, "head"), rot(&head, "head"));
    // Replacing an existing target keeps one track for it.
    let again = both.overlay(&head);
    assert_eq!(again.tracks().len(), both.tracks().len());
}

// ---------------------------------------------------------------------------------------------
// 4. Determinism: a pure function of explicit time.
// ---------------------------------------------------------------------------------------------

/// Same inputs, bit-identical output, in either order of calls: nothing advances between them.
#[test]
fn motion_is_a_pure_function_of_time() {
    let cf = crossfade();
    let pre = preempt(T0);
    let l = look();
    let lp = FX_B.playing(Repeat::Loop, 0);
    let times = [0u64, 1, 333, 749, 750, 751, 900, 1149, 1150, 1700, 5000];
    let run = |order: &[u64]| -> Vec<Vec<MotionPose>> {
        order
            .iter()
            .map(|t| {
                vec![
                    cf.sample(ms(*t)),
                    pre.sample(ms(*t)),
                    l.sample(ms(*t)),
                    lp.sample(ms(*t)),
                ]
            })
            .collect()
    };
    let forward = run(&times);
    let again = run(&times);
    assert_eq!(forward, again);
    let mut rev: Vec<u64> = times.to_vec();
    rev.reverse();
    let mut backward = run(&rev);
    backward.reverse();
    assert_eq!(forward, backward, "call order must not matter");
}
