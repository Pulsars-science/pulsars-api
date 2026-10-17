// TEMPORARY STUB: replaced by the real generators (feat/generateurs-rust).

//! Seeded randomness. The real module ports PHP's
//! `Random\Randomizer(new Mt19937($seed))`; this stand-in is a small
//! deterministic generator (SplitMix64) with the same shape of API.

/// A deterministic random number generator.
#[derive(Debug, Clone)]
pub struct Hasard {
    state: u64,
}

impl Hasard {
    pub fn new(seed: u32) -> Self {
        Self {
            state: u64::from(seed) ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// An integer in `min..=max` (`min` when the range is empty).
    pub fn entier(&mut self, min: i64, max: i64) -> i64 {
        if max <= min {
            return min;
        }
        let span = max.abs_diff(min).saturating_add(1);
        let offset = self.next_u64() % span;
        min.saturating_add_unsigned(offset)
    }

    /// Shuffles `items` in place (Fisher-Yates).
    pub fn melanger<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = usize::try_from(self.entier(0, i as i64)).unwrap_or(0);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_series() {
        let mut a = Hasard::new(42);
        let mut b = Hasard::new(42);
        for _ in 0..100 {
            let x = a.entier(1, 6);
            assert_eq!(x, b.entier(1, 6));
            assert!((1..=6).contains(&x));
        }
    }
}
