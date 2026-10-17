//! In-memory matchmaking queue for one queue key (`game-protocol.md` §6).
//!
//! The oldest waiting player is paired with the waiting player of closest
//! rating within a window of ±100, widened by 50 every 5 seconds of its
//! wait. Pure: the caller passes the time.

/// Half-width of the rating window when a player starts waiting.
pub const BASE_WINDOW: f64 = 100.0;
/// Added to the window every [`WIDEN_EVERY_MS`].
pub const WIDEN_BY: f64 = 50.0;
pub const WIDEN_EVERY_MS: i64 = 5_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Waiting<P> {
    pub player: P,
    pub rating: f64,
    /// Unix milliseconds.
    pub joined_at: i64,
}

/// Rating window of a player who has waited `waited_ms`.
pub fn window(waited_ms: i64) -> f64 {
    let steps = waited_ms.max(0) / WIDEN_EVERY_MS;
    BASE_WINDOW + WIDEN_BY * steps as f64
}

#[derive(Debug, Clone)]
pub struct Queue<P> {
    waiting: Vec<Waiting<P>>,
}

impl<P> Default for Queue<P> {
    fn default() -> Self {
        Self {
            waiting: Vec::new(),
        }
    }
}

impl<P: PartialEq + Clone> Queue<P> {
    /// Adds a player; a player already waiting keeps their place.
    pub fn join(&mut self, entry: Waiting<P>) {
        if !self.contains(&entry.player) {
            self.waiting.push(entry);
            // Stable: equal times keep their arrival order.
            self.waiting.sort_by_key(|w| w.joined_at);
        }
    }

    pub fn leave(&mut self, player: &P) -> bool {
        let before = self.waiting.len();
        self.waiting.retain(|w| &w.player != player);
        before != self.waiting.len()
    }

    pub fn contains(&self, player: &P) -> bool {
        self.waiting.iter().any(|w| &w.player == player)
    }

    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    /// Pairs whoever can be paired at `now`, oldest first, and removes them.
    pub fn pair(&mut self, now: i64) -> Vec<(Waiting<P>, Waiting<P>)> {
        let mut pairs = Vec::new();
        let mut i = 0;
        while i < self.waiting.len() {
            let oldest = &self.waiting[i];
            let limit = window(now - oldest.joined_at);
            let best = self
                .waiting
                .iter()
                .enumerate()
                .filter(|(j, w)| *j != i && (w.rating - oldest.rating).abs() <= limit)
                // Closest rating; the older one on a tie (the list is by age).
                .min_by(|(_, a), (_, b)| {
                    let da = (a.rating - oldest.rating).abs();
                    let db = (b.rating - oldest.rating).abs();
                    da.total_cmp(&db)
                })
                .map(|(j, _)| j);
            match best {
                Some(j) => {
                    // Remove the later index first so the earlier stays valid.
                    let (first, second) = if i < j { (i, j) } else { (j, i) };
                    let b = self.waiting.remove(second);
                    let a = self.waiting.remove(first);
                    // The older player first.
                    pairs.push(if first == i { (a, b) } else { (b, a) });
                    // Everyone before `first` already failed against a
                    // larger set of players: resume from there.
                    i = first;
                }
                None => i += 1,
            }
        }
        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(player: &'static str, rating: f64, joined_at: i64) -> Waiting<&'static str> {
        Waiting {
            player,
            rating,
            joined_at,
        }
    }

    fn names(
        pairs: &[(Waiting<&'static str>, Waiting<&'static str>)],
    ) -> Vec<(&'static str, &'static str)> {
        pairs.iter().map(|(a, b)| (a.player, b.player)).collect()
    }

    #[test]
    fn the_window_widens_every_five_seconds() {
        assert_eq!(window(0), 100.0);
        assert_eq!(window(4_999), 100.0);
        assert_eq!(window(5_000), 150.0);
        assert_eq!(window(12_000), 200.0);
        assert_eq!(window(-5), 100.0);
    }

    #[test]
    fn pairs_within_the_window_only() {
        let mut q = Queue::default();
        q.join(w("a", 1500.0, 0));
        q.join(w("b", 1650.0, 1_000));
        assert!(q.pair(1_000).is_empty(), "150 apart, window 100");
        assert_eq!(q.len(), 2);
        // After 5 s of waiting for `a`, the window is 150.
        assert_eq!(names(&q.pair(5_000)), vec![("a", "b")]);
        assert!(q.is_empty());
    }

    #[test]
    fn the_oldest_gets_the_closest_rating() {
        let mut q = Queue::default();
        q.join(w("old", 1500.0, 0));
        q.join(w("far", 1590.0, 100));
        q.join(w("near", 1480.0, 200));
        q.join(w("other", 1585.0, 300));
        let pairs = q.pair(500);
        assert_eq!(names(&pairs), vec![("old", "near"), ("far", "other")]);
    }

    #[test]
    fn an_isolated_oldest_does_not_block_the_others() {
        let mut q = Queue::default();
        q.join(w("lonely", 2400.0, 0));
        q.join(w("x", 1500.0, 10));
        q.join(w("y", 1520.0, 20));
        assert_eq!(names(&q.pair(100)), vec![("x", "y")]);
        assert!(q.contains(&"lonely"));
    }

    #[test]
    fn joining_twice_keeps_the_first_place_and_leave_removes() {
        let mut q = Queue::default();
        q.join(w("a", 1500.0, 0));
        q.join(w("a", 1500.0, 9_000));
        assert_eq!(q.len(), 1);
        assert!(q.leave(&"a"));
        assert!(!q.leave(&"a"));
        assert!(q.pair(0).is_empty());
    }

    #[test]
    fn a_single_player_waits() {
        let mut q = Queue::default();
        q.join(w("a", 1500.0, 0));
        assert!(q.pair(600_000).is_empty());
    }
}
