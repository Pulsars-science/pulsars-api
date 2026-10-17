//! Glicko-2 (Mark Glickman, "Example of the Glicko-2 system", 2013), one
//! rating period per match (`game-protocol.md` §7).

/// Glicko-2 scale factor between the Glicko and Glicko-2 scales.
const SCALE: f64 = 173.7178;

/// System constant: how much volatility may change.
pub const TAU: f64 = 0.5;

/// Convergence tolerance of the volatility iteration.
const EPSILON: f64 = 0.000_001;

/// A new player's rating.
pub const DEFAULT: Rating = Rating {
    rating: 1500.0,
    rd: 350.0,
    volatility: 0.06,
};

/// A rating with RD above this is provisional.
pub const PROVISIONAL_RD: f64 = 110.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rating {
    pub rating: f64,
    pub rd: f64,
    pub volatility: f64,
}

impl Rating {
    pub fn provisional(&self) -> bool {
        self.rd > PROVISIONAL_RD
    }

    /// The rating as shown.
    pub fn rounded(&self) -> i32 {
        // Ratings stay far inside i32.
        self.rating.round() as i32
    }
}

/// The result of one game, for the player being rated: 1 win, 0.5 draw,
/// 0 loss.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Game {
    pub opponent: Rating,
    pub score: f64,
}

fn g(phi: f64) -> f64 {
    1.0 / (1.0 + 3.0 * phi * phi / (std::f64::consts::PI * std::f64::consts::PI)).sqrt()
}

fn expected(mu: f64, mu_j: f64, phi_j: f64) -> f64 {
    1.0 / (1.0 + (-g(phi_j) * (mu - mu_j)).exp())
}

/// The player's rating after one rating period with `games`. Without games
/// only the deviation grows (step 6 of the paper).
pub fn update(player: Rating, games: &[Game], tau: f64) -> Rating {
    // Step 2: to the Glicko-2 scale.
    let mu = (player.rating - 1500.0) / SCALE;
    let phi = player.rd / SCALE;
    let sigma = player.volatility;

    if games.is_empty() {
        let phi_star = (phi * phi + sigma * sigma).sqrt();
        return Rating {
            rating: player.rating,
            rd: (phi_star * SCALE).min(DEFAULT.rd),
            volatility: sigma,
        };
    }

    // Steps 3 and 4: estimated variance and improvement.
    let mut v_inv = 0.0;
    let mut sum = 0.0;
    for game in games {
        let mu_j = (game.opponent.rating - 1500.0) / SCALE;
        let phi_j = game.opponent.rd / SCALE;
        let e = expected(mu, mu_j, phi_j);
        let g_j = g(phi_j);
        v_inv += g_j * g_j * e * (1.0 - e);
        sum += g_j * (game.score - e);
    }
    let v = 1.0 / v_inv;
    let delta = v * sum;

    // Step 5: new volatility (Illinois algorithm).
    let a = (sigma * sigma).ln();
    let f = |x: f64| {
        let ex = x.exp();
        let d = phi * phi + v + ex;
        ex * (delta * delta - phi * phi - v - ex) / (2.0 * d * d) - (x - a) / (tau * tau)
    };
    let mut big_a = a;
    let mut big_b = if delta * delta > phi * phi + v {
        (delta * delta - phi * phi - v).ln()
    } else {
        let mut k = 1.0;
        while f(a - k * tau) < 0.0 && k < 1000.0 {
            k += 1.0;
        }
        a - k * tau
    };
    let mut f_a = f(big_a);
    let mut f_b = f(big_b);
    let mut iterations = 0;
    while (big_b - big_a).abs() > EPSILON && iterations < 100 {
        iterations += 1;
        let big_c = big_a + (big_a - big_b) * f_a / (f_b - f_a);
        let f_c = f(big_c);
        if f_c * f_b <= 0.0 {
            big_a = big_b;
            f_a = f_b;
        } else {
            f_a /= 2.0;
        }
        big_b = big_c;
        f_b = f_c;
    }
    let sigma_new = (big_a / 2.0).exp();

    // Steps 6 and 7: new deviation and rating.
    let phi_star = (phi * phi + sigma_new * sigma_new).sqrt();
    let phi_new = 1.0 / (1.0 / (phi_star * phi_star) + 1.0 / v).sqrt();
    let mu_new = mu + phi_new * phi_new * sum;

    // Step 8: back to the Glicko scale.
    Rating {
        rating: mu_new * SCALE + 1500.0,
        rd: (phi_new * SCALE).min(DEFAULT.rd),
        volatility: sigma_new,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    /// Glickman's worked example: 1500 / 200 / 0.06 against three opponents.
    #[test]
    fn matches_the_papers_example() {
        let player = Rating {
            rating: 1500.0,
            rd: 200.0,
            volatility: 0.06,
        };
        let opponent = |rating, rd| Rating {
            rating,
            rd,
            volatility: 0.06,
        };
        let games = [
            Game {
                opponent: opponent(1400.0, 30.0),
                score: 1.0,
            },
            Game {
                opponent: opponent(1550.0, 100.0),
                score: 0.0,
            },
            Game {
                opponent: opponent(1700.0, 300.0),
                score: 0.0,
            },
        ];
        let new = update(player, &games, TAU);
        assert!(close(new.rating, 1464.06, 0.01), "{new:?}");
        assert!(close(new.rd, 151.52, 0.01), "{new:?}");
        assert!(close(new.volatility, 0.05999, 0.00001), "{new:?}");
    }

    #[test]
    fn a_win_between_new_players_is_symmetric() {
        let winner = update(
            DEFAULT,
            &[Game {
                opponent: DEFAULT,
                score: 1.0,
            }],
            TAU,
        );
        let loser = update(
            DEFAULT,
            &[Game {
                opponent: DEFAULT,
                score: 0.0,
            }],
            TAU,
        );
        assert!(
            winner.rating > 1600.0 && winner.rating < 1700.0,
            "{winner:?}"
        );
        assert!(close(winner.rating - 1500.0, 1500.0 - loser.rating, 1e-9));
        assert!(winner.rd < DEFAULT.rd);
        assert!(winner.provisional());
    }

    #[test]
    fn a_draw_between_equals_changes_nothing_but_rd() {
        let player = Rating {
            rating: 1700.0,
            rd: 80.0,
            volatility: 0.06,
        };
        let new = update(
            player,
            &[Game {
                opponent: player,
                score: 0.5,
            }],
            TAU,
        );
        assert!(close(new.rating, 1700.0, 1e-9));
        assert!(new.rd < 80.0);
        assert!(!new.provisional());
        assert_eq!(new.rounded(), 1700);
    }

    #[test]
    fn idle_period_only_widens_rd() {
        let player = Rating {
            rating: 1600.0,
            rd: 50.0,
            volatility: 0.06,
        };
        let new = update(player, &[], TAU);
        assert_eq!(new.rating, 1600.0);
        assert!(new.rd > 50.0);
        let capped = update(DEFAULT, &[], TAU);
        assert_eq!(capped.rd, 350.0);
    }
}
