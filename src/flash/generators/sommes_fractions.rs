use super::{card, gcd, int, meta, pick};
use crate::flash::{Card, Generator, Hasard, Lang};

/// Denominators where one is a multiple of the other: a single reduction.
#[rustfmt::skip]
const PAIRS: [(i64, i64); 18] = [
    (2, 4), (2, 6), (2, 8), (2, 10), (3, 6), (3, 9), (3, 12),
    (4, 8), (4, 12), (5, 10), (5, 15), (6, 12), (2, 3), (3, 4),
    (4, 5), (3, 5), (2, 5), (5, 6),
];

pub struct SommesFractions;

impl Generator for SommesFractions {
    meta! {
        key: "sommes-fractions",
        subject: Maths,
        level: MiddleSchool,
        title: "Sommes de fractions",
        description: "Additionner ou soustraire deux fractions et donner le résultat sous forme irréductible.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let (d1, d2) = pick(rng, &PAIRS);

        let n1 = numerator(rng, d1);
        let n2 = numerator(rng, d2);

        let addition = rng.entier(1, 2) == 1;
        let sign = if addition { '+' } else { '−' };

        let numerator = if addition {
            n1 * d2 + n2 * d1
        } else {
            n1 * d2 - n2 * d1
        };

        card(
            format!("{n1}/{d1} {sign} {n2}/{d2}"),
            fraction(lang, numerator, d1 * d2),
        )
    }
}

/// A numerator coprime with the denominator, so not reducible.
fn numerator(rng: &mut Hasard, denominator: i64) -> i64 {
    let mut numerator = rng.entier(1, denominator - 1);

    // The denominator is at most 15: a few tries are enough, and 1 always
    // works as a last resort.
    let mut tries = 0;
    while tries < 8 && gcd(numerator, denominator) != 1 {
        numerator = rng.entier(1, denominator - 1);
        tries += 1;
    }

    if gcd(numerator, denominator) == 1 {
        numerator
    } else {
        1
    }
}

/// The reduced fraction, or the integer when the quotient is exact.
fn fraction(lang: Lang, numerator: i64, denominator: i64) -> String {
    let divisor = match gcd(numerator.abs(), denominator) {
        0 => 1,
        d => d,
    };
    let numerator = numerator / divisor;
    let denominator = denominator / divisor;

    if denominator == 1 {
        return int(lang, numerator);
    }

    format!("{}/{denominator}", int(lang, numerator))
}
