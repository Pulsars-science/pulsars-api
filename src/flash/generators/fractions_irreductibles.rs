use super::{card, meta, pick};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

/// The irreducible fractions, amplified afterwards: the expected answer is
/// always one of these.
#[rustfmt::skip]
const REDUCED: [(i64, i64); 19] = [
    (1, 2), (1, 3), (2, 3), (1, 4), (3, 4), (1, 5), (2, 5), (3, 5), (4, 5),
    (1, 6), (5, 6), (2, 7), (3, 7), (5, 7), (3, 8), (5, 8), (7, 8), (4, 9),
    (5, 9),
];

pub struct FractionsIrreductibles;

impl Generator for FractionsIrreductibles {
    meta! {
        key: "fractions-irreductibles",
        subject: Maths,
        level: MiddleSchool,
        title: "Fractions irréductibles",
        description: "Simplifier une fraction jusqu’à sa forme irréductible, en repérant le diviseur commun d’un seul coup d’œil.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let (p, q) = pick(rng, &REDUCED);
        let factor = rng.entier(2, 9);

        card(
            t(
                lang,
                "Simplifier :fraction",
                &[("fraction", &format!("{}/{}", p * factor, q * factor))],
            ),
            format!("{p}/{q}"),
        )
    }
}
