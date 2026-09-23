//! A deliberately tiny deterministic particle system.
//!
//! No ECS, no physics engine, no threads, no `thread_rng`. Randomness comes from
//! a small seeded PRNG so `--deterministic` runs reproduce exactly. Particles can
//! be rendered as Braille dots or as terminal-cell sprites.

use crate::canvas::BrailleCanvas;
use crate::cell::Style;
use crate::surface::{Rect, Surface};

/// Small, fast, deterministic PRNG (xorshift64*).
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Avoid the fixed point at zero.
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        // 24 high-quality bits.
        ((self.next_u64() >> 40) as f32) / ((1u32 << 24) as f32)
    }

    /// Uniform in `[min, max)`.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }

    /// `-1.0` or `1.0`.
    pub fn sign(&mut self) -> f32 {
        if self.next_u64() & 1 == 0 {
            -1.0
        } else {
            1.0
        }
    }
}

/// A single particle in pixel space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub max_life: f32,
    pub intensity: f32,
}

impl Particle {
    pub fn alive(&self) -> bool {
        self.life > 0.0
    }
}

/// Deterministic particle collection.
#[derive(Debug, Clone)]
pub struct ParticleSystem {
    pub particles: Vec<Particle>,
    pub rng: Rng,
}

impl ParticleSystem {
    pub fn new(seed: u64) -> Self {
        Self {
            particles: Vec::new(),
            rng: Rng::new(seed),
        }
    }

    pub fn len(&self) -> usize {
        self.particles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.particles.is_empty()
    }

    pub fn spawn(&mut self, p: Particle) -> usize {
        self.particles.push(p);
        self.particles.len() - 1
    }

    /// Emits `n` particles from `(x, y)` with deterministic randomness.
    pub fn burst(&mut self, n: usize, x: f32, y: f32, speed: f32, life: f32, spread: f32) {
        for _ in 0..n {
            let angle = self.rng.range(0.0, std::f32::consts::TAU);
            let v = speed * self.rng.range(0.35, 1.0);
            let life = life * self.rng.range(0.5, 1.0) * spread.max(0.01);
            self.particles.push(Particle {
                x,
                y,
                vx: angle.cos() * v,
                vy: angle.sin() * v,
                life,
                max_life: life,
                intensity: self.rng.range(0.4, 1.0),
            });
        }
    }

    /// Integrates physics and drops expired particles.
    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt;
        }
        self.particles.retain(|p| p.alive());
    }

    /// Plots every live particle as a Braille dot (clipped by the canvas).
    pub fn render_braille(&self, canvas: &mut BrailleCanvas) {
        for p in &self.particles {
            if p.x.is_finite() && p.y.is_finite() {
                canvas.set(p.x.round() as i32, p.y.round() as i32);
            }
        }
    }

    /// Plots particles as single-cell sprites on a surface, clipped to `rect`.
    pub fn render_cells(&self, surface: &mut Surface, rect: Rect, glyph: &str, style: Style) {
        for p in &self.particles {
            if !p.x.is_finite() || !p.y.is_finite() {
                continue;
            }
            let x = p.x.round() as i32;
            let y = p.y.round() as i32;
            if x < 0 || y < 0 {
                continue;
            }
            let (x, y) = (x as u16, y as u16);
            if rect.contains(x, y) {
                surface.set_cell(
                    x,
                    y,
                    crate::cell::Cell::new(crate::cell::Glyph::new(glyph), style),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_for_a_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..16 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn rng_range_is_bounded() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let v = r.range(-2.0, 5.0);
            assert!((-2.0..5.0).contains(&v));
        }
    }

    #[test]
    fn burst_is_reproducible_and_expires() {
        let mut a = ParticleSystem::new(123);
        a.burst(50, 10.0, 10.0, 4.0, 1.0, 1.0);
        let mut b = ParticleSystem::new(123);
        b.burst(50, 10.0, 10.0, 4.0, 1.0, 1.0);
        assert_eq!(a.particles, b.particles);
        assert_eq!(a.len(), 50);

        for _ in 0..120 {
            a.update(1.0 / 60.0);
        }
        assert!(a.is_empty(), "particles should expire");
    }

    #[test]
    fn particle_render_is_bounded_by_canvas() {
        let mut ps = ParticleSystem::new(1);
        ps.burst(200, 0.0, 0.0, 30.0, 5.0, 1.0);
        ps.update(0.5);
        let mut canvas = BrailleCanvas::new(10, 5); // 10 x 20 dots
        ps.render_braille(&mut canvas); // must not panic
        assert!(!canvas.is_empty());
    }

    #[test]
    fn render_cells_respects_clip_rect() {
        let mut ps = ParticleSystem::new(2);
        ps.particles.push(Particle {
            x: 1.0,
            y: 1.0,
            vx: 0.0,
            vy: 0.0,
            life: 1.0,
            max_life: 1.0,
            intensity: 1.0,
        });
        ps.particles.push(Particle {
            x: 50.0,
            y: 50.0,
            vx: 0.0,
            vy: 0.0,
            life: 1.0,
            max_life: 1.0,
            intensity: 1.0,
        });
        let mut s = Surface::new(10, 5);
        ps.render_cells(&mut s, Rect::new(0, 0, 10, 5), "*", Style::default());
        assert_eq!(s.get(1, 1).unwrap().glyph.grapheme.as_str(), "*");
        assert_eq!(s.get(5, 0).unwrap().glyph.grapheme.as_str(), " ");
    }
}
