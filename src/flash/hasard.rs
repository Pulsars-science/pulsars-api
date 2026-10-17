//! The randomness of the flash questions, identical to PHP's.
//!
//! The site drew its series in PHP with `new Randomizer(new Mt19937($seed))`;
//! the game (`pulsars-game/src/flash/hasard.ts`) ported it, and this is the
//! same port: ext/random of php-src, Mt19937 engine in `MT_RAND_MT19937`
//! mode, `getInt`, `shuffleArray` and `pickArrayKeys`. A seed gives the same
//! series as on the site. `tests/flash.rs` checks it against PHP's own output.

use std::fmt;

const N: usize = 624;
const M: usize = 397;

fn twist(m: u32, u: u32, v: u32) -> u32 {
    let mix = (u & 0x8000_0000) | (v & 0x7fff_ffff);
    m ^ (mix >> 1) ^ if v & 1 == 1 { 0x9908_b0df } else { 0 }
}

/// A Mt19937 generator seeded like PHP's `new Mt19937($seed)`.
///
/// Two generators compare equal when their next draws are the same.
#[derive(Clone, PartialEq, Eq)]
pub struct Hasard {
    state: [u32; N],
    index: usize,
}

impl fmt::Debug for Hasard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hasard")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl Hasard {
    pub fn new(seed: u32) -> Self {
        let mut state = [0u32; N];
        state[0] = seed;
        for i in 1..N {
            let prev = state[i - 1];
            // `i < 624`: the cast cannot truncate.
            state[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        let mut hasard = Self { state, index: 0 };
        hasard.reload();
        hasard
    }

    fn reload(&mut self) {
        let s = &mut self.state;
        for p in 0..N - M {
            s[p] = twist(s[p + M], s[p], s[p + 1]);
        }
        for p in N - M..N - 1 {
            s[p] = twist(s[p + M - N], s[p], s[p + 1]);
        }
        s[N - 1] = twist(s[M - 1], s[N - 1], s[0]);
        self.index = 0;
    }

    /// The next 32-bit output of the engine.
    pub fn suivant(&mut self) -> u32 {
        if self.index >= N {
            self.reload();
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// An integer in `0..=max` without bias (`php_random_range32`): draws
    /// above the last full multiple are rejected.
    fn range32(&mut self, max: u32) -> u32 {
        let mut result = self.suivant();
        if max == u32::MAX {
            return result;
        }
        let size = max + 1;
        if size & (size - 1) == 0 {
            return result & (size - 1);
        }
        let limit = u32::MAX - (u32::MAX % size) - 1;
        while result > limit {
            result = self.suivant();
        }
        result % size
    }

    /// `php_random_range64`, for spans wider than 32 bits: two engine
    /// outputs per draw, high word first. No flash generator needs it.
    fn range64(&mut self, max: u64) -> u64 {
        let draw = |h: &mut Self| (u64::from(h.suivant()) << 32) | u64::from(h.suivant());
        let mut result = draw(self);
        if max == u64::MAX {
            return result;
        }
        let size = max + 1;
        if size & (size - 1) == 0 {
            return result & (size - 1);
        }
        let limit = u64::MAX - (u64::MAX % size) - 1;
        while result > limit {
            result = draw(self);
        }
        result % size
    }

    /// `Randomizer::getInt($min, $max)`, both bounds included. Returns `min`
    /// without drawing when `max < min` (PHP throws).
    pub fn entier(&mut self, min: i64, max: i64) -> i64 {
        if max < min {
            return min;
        }
        let span = max.abs_diff(min);
        match u32::try_from(span) {
            Ok(span) => min.wrapping_add(i64::from(self.range32(span))),
            Err(_) => min.wrapping_add_unsigned(self.range64(span)),
        }
    }

    /// `Randomizer::shuffleArray()`: Fisher-Yates from the end, on a copy.
    pub fn melanger<T: Clone>(&mut self, values: &[T]) -> Vec<T> {
        let mut copy = values.to_vec();
        for rest in (1..copy.len()).rev() {
            let j = self.index_below(rest + 1);
            copy.swap(rest, j);
        }
        copy
    }

    /// A value picked from a list (`pickArrayKeys($list, 1)`); `None`
    /// without drawing when the list is empty.
    pub fn parmi<'a, T>(&mut self, values: &'a [T]) -> Option<&'a T> {
        if values.is_empty() {
            return None;
        }
        let i = self.index_below(values.len());
        values.get(i)
    }

    /// `entier(0, len - 1)` as an index, for `len >= 1`.
    pub(crate) fn index_below(&mut self, len: usize) -> usize {
        let max = i64::try_from(len.saturating_sub(1)).unwrap_or(i64::MAX);
        usize::try_from(self.entier(0, max)).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_span_still_draws() {
        let mut a = Hasard::new(5);
        let mut b = Hasard::new(5);
        assert_eq!(a.entier(3, 3), 3);
        b.suivant();
        assert_eq!(a, b);
    }

    #[test]
    fn empty_list_draws_nothing() {
        let mut a = Hasard::new(5);
        assert_eq!(a.parmi::<u8>(&[]), None);
        assert_eq!(a, Hasard::new(5));
    }
}
