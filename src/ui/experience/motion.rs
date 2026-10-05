//! The one motion primitive shared by the settling and continuous grammars.
//!
//! MEDIA_SHELF (carousel position), ORBITAL (ring phase) and BLADES (blade-stack
//! offset) all glide a single scalar toward an integer target with the *same*
//! semi-implicit-Euler damped spring. Before this module each kept its own
//! character-identical copy; they differ only in their ω/ζ constants and in whether
//! the caller clamps the result, so the integrator and the settle test live here
//! once and each grammar passes its own feel in per step.

/// A one-dimensional damped spring integrated with semi-implicit (symplectic)
/// Euler. `x` is the current position, `v` the current velocity. The caller
/// supplies the stiffness `ω` and damping ratio `ζ` on every [`DampedSpring::step`]
/// so each grammar keeps its own motion feel while sharing one integrator and one
/// [`DampedSpring::settled`] test.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct DampedSpring {
    pub x: f32,
    pub v: f32,
}

impl DampedSpring {
    /// A spring parked at `x` with zero velocity.
    pub fn at(x: f32) -> Self {
        Self { x, v: 0.0 }
    }

    /// Advance one sub-step of `dt` seconds toward `target`, with stiffness `omega`
    /// and damping ratio `zeta`. The acceleration is the standard damped-spring law
    /// `a = -2ζω·v - ω²(x - target)`; velocity is integrated first, then position
    /// (semi-implicit Euler), which stays stable for the small sub-steps the
    /// grammars use.
    pub fn step(&mut self, target: f32, dt: f32, omega: f32, zeta: f32) {
        let accel = -2.0 * zeta * omega * self.v - omega * omega * (self.x - target);
        self.v += accel * dt;
        self.x += self.v * dt;
    }

    /// Settled when within `1e-3` of `target` with negligible velocity — the point
    /// past which further frames would not change the rendered position.
    pub fn settled(&self, target: f32) -> bool {
        (self.x - target).abs() < 1e-3 && self.v.abs() < 1e-3
    }
}
