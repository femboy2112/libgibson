//! A tiny deterministic PRNG (xorshift64*) shared by the composition engines. Determinism
//! is load-bearing: given a seed, the whole score is reproducible for tests and replay.

/// A seeded pseudo-random generator.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator seeded by `seed` (0 remapped so it never sticks).
    pub fn new(seed: u64) -> Rng {
        Rng {
            state: if seed == 0 { 0x1234_5678_9abc_def1 } else { seed },
        }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A float in `[0, 1)`.
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// A float in `[lo, hi)`.
    #[inline]
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// An index in `[0, n)` (0 if `n == 0`).
    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// True with probability `p`.
    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }

    /// A reference to a random element of `s` (or `None` if empty).
    pub fn pick<'a, T>(&mut self, s: &'a [T]) -> Option<&'a T> {
        if s.is_empty() {
            None
        } else {
            let i = self.below(s.len());
            Some(&s[i])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_a_seed() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn floats_in_range_and_below_bounded() {
        let mut r = Rng::new(1);
        for _ in 0..10_000 {
            let f = r.next_f32();
            assert!((0.0..1.0).contains(&f));
            assert!(r.below(5) < 5);
            let g = r.range_f32(2.0, 3.0);
            assert!((2.0..3.0).contains(&g));
        }
    }

    #[test]
    fn chance_roughly_matches_probability() {
        let mut r = Rng::new(99);
        let mut hits = 0;
        for _ in 0..10_000 {
            if r.chance(0.3) {
                hits += 1;
            }
        }
        let frac = hits as f32 / 10_000.0;
        assert!((frac - 0.3).abs() < 0.03, "chance frac {frac}");
    }
}
