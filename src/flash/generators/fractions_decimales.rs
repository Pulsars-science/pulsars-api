use super::{card, dec, meta, pick};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

/// Only irreducible fractions with a short, exact decimal form; irreducible
/// so that the answer is unique when the card is read backwards.
#[rustfmt::skip]
const FRACTIONS: [(i64, i64); 113] = [
    (1, 2), (3, 2), (5, 2), (1, 4), (3, 4), (5, 4), (7, 4), (9, 4),
    (11, 4), (1, 5), (2, 5), (3, 5), (4, 5), (6, 5), (7, 5), (8, 5),
    (9, 5), (1, 8), (3, 8), (5, 8), (7, 8), (9, 8), (11, 8), (13, 8),
    (15, 8), (1, 10), (3, 10), (7, 10), (9, 10), (11, 10), (13, 10), (17, 10),
    (19, 10), (1, 16), (3, 16), (5, 16), (7, 16), (9, 16), (11, 16), (13, 16),
    (15, 16), (1, 20), (3, 20), (7, 20), (9, 20), (11, 20), (13, 20), (17, 20),
    (19, 20), (1, 25), (2, 25), (3, 25), (4, 25), (6, 25), (7, 25), (8, 25),
    (9, 25), (1, 40), (3, 40), (7, 40), (9, 40), (11, 40), (13, 40), (17, 40),
    (19, 40), (1, 50), (3, 50), (7, 50), (9, 50), (11, 50), (13, 50), (17, 50),
    (19, 50), (1, 100), (3, 100), (7, 100), (9, 100), (11, 100), (13, 100), (17, 100),
    (19, 100), (1, 125), (2, 125), (3, 125), (4, 125), (6, 125), (7, 125), (8, 125),
    (9, 125), (1, 200), (3, 200), (7, 200), (9, 200), (11, 200), (13, 200), (17, 200),
    (19, 200), (1, 250), (3, 250), (7, 250), (9, 250), (11, 250), (13, 250), (17, 250),
    (19, 250), (1, 500), (3, 500), (7, 500), (9, 500), (11, 500), (13, 500), (17, 500),
    (19, 500),
];

pub struct FractionsDecimales;

impl Generator for FractionsDecimales {
    meta! {
        key: "fractions-decimales",
        subject: Maths,
        level: MiddleSchool,
        title: "Écriture décimale des fractions simples",
        short_title: "Écriture décimale des fractions",
        description: "Passer d’une fraction simple à son écriture décimale et revenir à la fraction, de tête, sans poser la division.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let (numerator, denominator) = pick(rng, &FRACTIONS);

        let fraction = format!("{numerator}/{denominator}");
        let decimal = dec(lang, numerator as f64 / denominator as f64, 4);

        if rng.entier(1, 2) == 1 {
            return card(fraction, decimal);
        }

        card(
            t(
                lang,
                "Quelle fraction irréductible vaut :nombre ?",
                &[("nombre", &decimal)],
            ),
            fraction,
        )
    }
}
