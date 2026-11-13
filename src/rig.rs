//! Experimental camera rigs for [`raster3d`](crate::raster3d): a dolly between two poses,
//! an orbit around a target, and a transition between two cameras.
//!
//! Every rig is a pure function of a caller-supplied time `t_ms` (milliseconds
//! since the rig started) and a `reduced_motion` flag. There is no wall clock and
//! no internal state, so the same inputs always give the same [`Camera`](crate::raster3d::Camera).
//!
//! Settling laws, shared by every rig that settles:
//! - at or after its settle time it returns its final pose **exactly** (the
//!   stored value, not an interpolation evaluated at 1);
//! - with `reduced_motion` it returns that same final pose at every `t_ms`, so
//!   the settled frame is bit-identical with motion on or off;
//! - [`Dolly::settled`](crate::rig::Dolly::settled) / [`Transition::settled`](crate::rig::Transition::settled) say when a caller can stop
//!   requesting frames.
//!
//! Lens values (`fov_y`, `near`, `far`) and `up` are passed through unchanged
//! from the rig's base camera ([`Camera::default`](crate::raster3d::Camera::default) unless set with `lens`).
//! Trigonometry uses the platform `f32` functions: results are deterministic on
//! one host, not guaranteed bit-identical across hosts.
use crate::geom::Vec3;
use crate::raster3d::Camera;

/// Easing curve applied to normalised progress `k ∈ [0, 1]` (clamped).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ease {
    Linear,
    /// `4k³` for `k < ½`, else `1 − (−2k + 2)³ / 2`.
    #[default]
    CubicInOut,
}

impl Ease {
    /// Evaluated in `f64`, then rounded to `f32`.
    pub fn apply(self, k: f32) -> f32 {
        self.apply_f64(f64::from(k)) as f32
    }
    fn apply_f64(self, k: f64) -> f64 {
        let k = if k.is_nan() { 0.0 } else { k.clamp(0.0, 1.0) };
        match self {
            Ease::Linear => k,
            Ease::CubicInOut => {
                if k < 0.5 {
                    4.0 * k * k * k
                } else {
                    let m = -2.0 * k + 2.0;
                    1.0 - m * m * m / 2.0
                }
            }
        }
    }
}

fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

fn lerp3(a: Vec3, b: Vec3, k: f32) -> Vec3 {
    Vec3::new(lerp(a.x, b.x, k), lerp(a.y, b.y, k), lerp(a.z, b.z, k))
}

/// Eased progress of `t_ms` through `duration_ms` — `ease(t / duration)` in
/// `f64`, rounded to `f32` — or `None` once complete (and for a zero duration),
/// so callers return the exact final value.
fn progress(t_ms: u64, duration_ms: u64, ease: Ease) -> Option<f32> {
    (t_ms < duration_ms).then(|| ease.apply_f64(t_ms as f64 / duration_ms as f64) as f32)
}

/// Moves the camera from `from` to `to` while it looks at a fixed `target`:
/// `position = from + (to − from)·k` per component in `f32`, with `k` from
/// [`Ease`] over `settle_ms`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dolly {
    from: Vec3,
    to: Vec3,
    target: Vec3,
    ease: Ease,
    settle_ms: u64,
    lens: Camera,
}

impl Dolly {
    /// Default ease [`Ease::CubicInOut`], default settle time 1000 ms.
    pub fn new(from: Vec3, to: Vec3, target: Vec3) -> Self {
        Self {
            from,
            to,
            target,
            ease: Ease::CubicInOut,
            settle_ms: 1000,
            lens: Camera::default(),
        }
    }
    pub fn ease(mut self, ease: Ease) -> Self {
        self.ease = ease;
        self
    }
    pub fn settle_ms(mut self, ms: u64) -> Self {
        self.settle_ms = ms;
        self
    }
    /// `up`, `fov_y`, `near` and `far` come from `base`; its position and target
    /// are ignored.
    pub fn lens(mut self, base: Camera) -> Self {
        self.lens = base;
        self
    }
    pub fn at(&self, t_ms: u64, reduced_motion: bool) -> Camera {
        let position = match progress(t_ms, self.settle_ms, self.ease) {
            Some(k) if !reduced_motion => lerp3(self.from, self.to, k),
            _ => self.to,
        };
        Camera {
            position,
            target: self.target,
            ..self.lens
        }
    }
    pub fn settled(&self, t_ms: u64, reduced_motion: bool) -> bool {
        reduced_motion || t_ms >= self.settle_ms
    }
}

/// Swings around a fixed `target` on a circle of `radius` centred under it:
///
/// ```text
/// u = (t_ms mod yaw_period) / yaw_period      v = (t_ms mod lift_period) / lift_period
/// yaw  = yaw_amp  · sin(2π·u)                 lift = lift_amp · sin(2π·v)
/// position = (target.x + radius·sin(yaw), height + lift, target.z − radius·cos(yaw))
/// ```
///
/// computed in `f64` and rounded to `f32` per component; the integer modulo is
/// taken before the conversion, so phase never loses precision as `t_ms` grows.
/// So yaw 0 puts the camera on the −z side of the target. A zero period or
/// amplitude disables that motion. With reduced motion `t_ms` is taken as 0. An
/// orbit never settles unless motion is reduced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbit {
    target: Vec3,
    radius: f32,
    height: f32,
    yaw_amp: f32,
    yaw_period_ms: u64,
    lift_amp: f32,
    lift_period_ms: u64,
    lens: Camera,
}

impl Orbit {
    /// No swing and no lift; camera height defaults to `target.y`.
    pub fn new(target: Vec3, radius: f32) -> Self {
        Self {
            target,
            radius,
            height: target.y,
            yaw_amp: 0.0,
            yaw_period_ms: 0,
            lift_amp: 0.0,
            lift_period_ms: 0,
            lens: Camera::default(),
        }
    }
    /// Base camera height (world y) before lift.
    pub fn height(mut self, y: f32) -> Self {
        self.height = y;
        self
    }
    pub fn yaw_swing(mut self, amp_rad: f32, period_ms: u64) -> Self {
        self.yaw_amp = amp_rad;
        self.yaw_period_ms = period_ms;
        self
    }
    pub fn lift(mut self, amp: f32, period_ms: u64) -> Self {
        self.lift_amp = amp;
        self.lift_period_ms = period_ms;
        self
    }
    /// As for [`Dolly::lens`].
    pub fn lens(mut self, base: Camera) -> Self {
        self.lens = base;
        self
    }
    pub fn at(&self, t_ms: u64, reduced_motion: bool) -> Camera {
        let t = if reduced_motion { 0 } else { t_ms };
        let wave = |amp: f32, period: u64| -> f64 {
            if period == 0 {
                return 0.0;
            }
            let phase = (t % period) as f64 / period as f64;
            f64::from(amp) * (std::f64::consts::TAU * phase).sin()
        };
        let yaw = wave(self.yaw_amp, self.yaw_period_ms);
        let lift = wave(self.lift_amp, self.lift_period_ms);
        let r = f64::from(self.radius);
        let position = Vec3::new(
            (f64::from(self.target.x) + r * yaw.sin()) as f32,
            (f64::from(self.height) + lift) as f32,
            (f64::from(self.target.z) - r * yaw.cos()) as f32,
        );
        Camera {
            position,
            target: self.target,
            ..self.lens
        }
    }
    pub fn settled(&self, _t_ms: u64, reduced_motion: bool) -> bool {
        reduced_motion
    }
}

/// Flies from one camera to another over `duration_ms`: position, target, up
/// and lens all interpolate. With reduced motion it cuts straight to `to`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    from: Camera,
    to: Camera,
    duration_ms: u64,
    ease: Ease,
}

impl Transition {
    pub fn between(from: Camera, to: Camera, duration_ms: u64, ease: Ease) -> Self {
        Self {
            from,
            to,
            duration_ms,
            ease,
        }
    }
    pub fn at(&self, t_ms: u64, reduced_motion: bool) -> Camera {
        match progress(t_ms, self.duration_ms, self.ease) {
            Some(k) if !reduced_motion => Camera {
                position: lerp3(self.from.position, self.to.position, k),
                target: lerp3(self.from.target, self.to.target, k),
                up: lerp3(self.from.up, self.to.up, k),
                fov_y: lerp(self.from.fov_y, self.to.fov_y, k),
                near: lerp(self.from.near, self.to.near, k),
                far: lerp(self.from.far, self.to.far, k),
            },
            _ => self.to,
        }
    }
    pub fn settled(&self, t_ms: u64, reduced_motion: bool) -> bool {
        reduced_motion || t_ms >= self.duration_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_in_out_matches_its_formula_and_endpoints() {
        let e = Ease::CubicInOut;
        assert_eq!(e.apply(0.0), 0.0);
        assert_eq!(e.apply(1.0), 1.0);
        assert_eq!(e.apply(0.5), 0.5);
        assert_eq!(e.apply(0.25), 4.0 * 0.25f32.powi(3));
        assert_eq!(e.apply(-3.0), 0.0);
        assert_eq!(e.apply(f32::NAN), 0.0);
        for i in 0..100 {
            assert!(e.apply(i as f32 / 100.0) <= e.apply((i + 1) as f32 / 100.0));
        }
    }

    fn dolly() -> Dolly {
        Dolly::new(
            Vec3::new(0.0, 7.5, -12.0),
            Vec3::new(2.6, 3.4, -8.2),
            Vec3::new(0.0, 1.5, 0.0),
        )
        .settle_ms(2400)
    }

    #[test]
    fn dolly_starts_at_from_and_settles_exactly_on_to() {
        let d = dolly();
        assert_eq!(d.at(0, false).position, Vec3::new(0.0, 7.5, -12.0));
        for t in [2400, 2401, 99_999] {
            assert_eq!(d.at(t, false).position, Vec3::new(2.6, 3.4, -8.2), "{t}");
            assert!(d.settled(t, false));
        }
        assert!(!d.settled(2399, false));
        let mid = d.at(1200, false).position;
        assert_eq!(
            mid,
            lerp3(Vec3::new(0.0, 7.5, -12.0), Vec3::new(2.6, 3.4, -8.2), 0.5)
        );
    }

    #[test]
    fn reduced_motion_is_the_settled_pose_at_every_time() {
        let d = dolly();
        let settled = d.at(2400, false);
        for t in [0, 1, 1200, 2399, 5000] {
            assert_eq!(d.at(t, true), settled, "{t}");
            assert!(d.settled(t, true));
        }
    }

    #[test]
    fn transition_interpolates_everything_and_cuts_when_reduced() {
        let a = Camera::default();
        let b = Camera {
            position: Vec3::new(4.0, 2.0, -3.0),
            target: Vec3::new(1.0, 1.0, 1.0),
            fov_y: 1.2,
            ..Camera::default()
        };
        let tr = Transition::between(a, b, 600, Ease::Linear);
        assert_eq!(tr.at(0, false), a);
        assert_eq!(tr.at(600, false), b);
        assert_eq!(tr.at(0, true), b);
        let mid = tr.at(300, false);
        assert_eq!(mid.position, Vec3::new(2.0, 1.0, -1.5));
        assert_eq!(mid.fov_y, lerp(a.fov_y, 1.2, 0.5));
        assert!(!tr.settled(599, false) && tr.settled(600, false) && tr.settled(0, true));
    }

    /// The consumer's hand-written camera (ProjectGHOST Citadel), restated from
    /// its published formulas, must match the rig to 1e-6.
    fn citadel_lens() -> Camera {
        Camera {
            far: 60.0,
            ..Camera::default()
        }
    }

    #[test]
    fn dolly_reproduces_the_citadel_camera() {
        let (a, b) = (Vec3::new(0.0, 7.5, -12.0), Vec3::new(2.6, 3.4, -8.2));
        let d = Dolly::new(a, b, Vec3::new(0.0, 1.5, 0.0))
            .settle_ms(700)
            .lens(citadel_lens());
        for t in [0u64, 350, 699] {
            let k = Ease::CubicInOut.apply_f64((t as f64 / 700.0).clamp(0.0, 1.0)) as f32;
            let want = a.plus(b.minus(a).scale(k));
            let got = d.at(t, false);
            assert!(got.position.minus(want).length() <= 1e-6, "{t}: {got:?}");
            assert_eq!((got.far, got.up), (60.0, Vec3::new(0.0, 1.0, 0.0)));
        }
        assert_eq!(d.at(700, false).position, b);
    }

    #[test]
    fn orbit_reproduces_the_citadel_camera() {
        let o = Orbit::new(Vec3::new(0.0, 1.2, 0.0), 5.0)
            .height(1.7)
            .yaw_swing(0.5, 16_000)
            .lift(0.3, 9_000)
            .lens(citadel_lens());
        for t in [0u64, 4_000, 8_000, 123_456_789] {
            let u = (t % 16_000) as f64 / 16_000.0;
            let v = (t % 9_000) as f64 / 9_000.0;
            let yaw = 0.5 * (std::f64::consts::TAU * u).sin();
            let lift = 0.3 * (std::f64::consts::TAU * v).sin();
            let want = Vec3::new(
                (5.0 * yaw.sin()) as f32,
                (1.7 + lift) as f32,
                (-5.0 * yaw.cos()) as f32,
            );
            let got = o.at(t, false);
            assert!(got.position.minus(want).length() <= 1e-6, "{t}: {got:?}");
            assert_eq!(got.target, Vec3::new(0.0, 1.2, 0.0));
            assert!(got.position.z < 0.0, "camera stays on the −z side");
        }
        assert_eq!(o.at(5_555, true).position, Vec3::new(0.0, 1.7, -5.0));
        assert!(o.settled(0, true) && !o.settled(0, false));
    }

    #[test]
    fn orbit_without_periods_is_static() {
        let o = Orbit::new(Vec3::new(1.0, 2.0, 3.0), 4.0);
        assert_eq!(o.at(0, false), o.at(77_777, false));
        assert_eq!(o.at(0, false).position, Vec3::new(1.0, 2.0, -1.0));
    }

    #[test]
    fn zero_duration_is_already_settled() {
        let d = dolly().settle_ms(0);
        assert_eq!(d.at(0, false).position, Vec3::new(2.6, 3.4, -8.2));
        assert!(d.settled(0, false));
    }
}
