//! A tiny deterministic RNG. Self-play needs reproducible games, so every
//! stochastic decision (only luck rolls, today) draws from here rather than
//! from thread-local randomness.

/// xorshift64*, seeded so that seed 0 still produces a usable stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `0..=max`.
    #[inline]
    pub fn roll_inclusive(&mut self, max: u32) -> u32 {
        if max == 0 {
            return 0;
        }
        // Modulo bias is negligible at these ranges (max is 9 for luck).
        (self.next_u64() % (max as u64 + 1)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_a_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn luck_rolls_cover_the_range() {
        let mut rng = Rng::new(7);
        let mut seen = [false; 10];
        for _ in 0..1000 {
            let r = rng.roll_inclusive(9);
            assert!(r <= 9);
            seen[r as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }
}
