//! Final ranking of a match (`game-protocol.md` §4): by score, then by the
//! lower total answer time over correct answers, else equal rank. Leavers
//! rank after everyone else.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Win,
    Loss,
    Draw,
    Forfeit,
}

impl Outcome {
    pub fn key(self) -> &'static str {
        match self {
            Outcome::Win => "win",
            Outcome::Loss => "loss",
            Outcome::Draw => "draw",
            Outcome::Forfeit => "forfeit",
        }
    }

    /// Glicko score of a ranked game.
    pub fn score(self) -> f64 {
        match self {
            Outcome::Win => 1.0,
            Outcome::Draw => 0.5,
            Outcome::Loss | Outcome::Forfeit => 0.0,
        }
    }
}

/// What the ranking looks at for one seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    pub score: i32,
    /// Total time over correct answers, in milliseconds.
    pub answer_time_ms: i64,
    pub forfeited: bool,
}

/// Rank (1 = best, equal for ties, competition style "1, 1, 3") and outcome
/// of each standing, in the same order.
pub fn rank(standings: &[Standing]) -> Vec<(u16, Outcome)> {
    let mut order: Vec<usize> = (0..standings.len()).collect();
    let key = |i: usize| {
        let s = standings[i];
        (s.forfeited, -i64::from(s.score), s.answer_time_ms)
    };
    order.sort_by_key(|&i| key(i));

    let mut ranks = vec![0u16; standings.len()];
    for (position, &i) in order.iter().enumerate() {
        let tied_with_previous = position > 0 && key(order[position - 1]) == key(i);
        ranks[i] = if tied_with_previous {
            ranks[order[position - 1]]
        } else {
            u16::try_from(position + 1).unwrap_or(u16::MAX)
        };
    }

    let leaders = standings
        .iter()
        .zip(&ranks)
        .filter(|(s, r)| !s.forfeited && **r == 1)
        .count();
    standings
        .iter()
        .zip(ranks)
        .map(|(s, r)| {
            let outcome = if s.forfeited {
                Outcome::Forfeit
            } else if r == 1 && leaders == 1 {
                Outcome::Win
            } else if r == 1 {
                Outcome::Draw
            } else {
                Outcome::Loss
            };
            (r, outcome)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(score: i32, answer_time_ms: i64, forfeited: bool) -> Standing {
        Standing {
            score,
            answer_time_ms,
            forfeited,
        }
    }

    #[test]
    fn higher_score_wins() {
        assert_eq!(
            rank(&[s(3, 100, false), s(5, 900, false)]),
            vec![(2, Outcome::Loss), (1, Outcome::Win)]
        );
    }

    #[test]
    fn equal_scores_are_split_by_answer_time() {
        assert_eq!(
            rank(&[s(5, 4000, false), s(5, 3999, false)]),
            vec![(2, Outcome::Loss), (1, Outcome::Win)]
        );
    }

    #[test]
    fn same_score_and_time_is_a_draw() {
        assert_eq!(
            rank(&[s(5, 4000, false), s(5, 4000, false), s(2, 10, false)]),
            vec![(1, Outcome::Draw), (1, Outcome::Draw), (3, Outcome::Loss)]
        );
    }

    #[test]
    fn leavers_rank_last_whatever_their_score() {
        assert_eq!(
            rank(&[s(9, 100, true), s(-2, 0, false)]),
            vec![(2, Outcome::Forfeit), (1, Outcome::Win)]
        );
        assert_eq!(
            rank(&[s(1, 0, true), s(4, 0, false), s(3, 0, true), s(4, 0, false)]),
            vec![
                (4, Outcome::Forfeit),
                (1, Outcome::Draw),
                (3, Outcome::Forfeit),
                (1, Outcome::Draw)
            ]
        );
    }

    #[test]
    fn negative_scores_rank_below_zero() {
        assert_eq!(
            rank(&[s(-1, 0, false), s(0, 0, false), s(0, 0, false)]),
            vec![(3, Outcome::Loss), (1, Outcome::Draw), (1, Outcome::Draw)]
        );
    }
}
