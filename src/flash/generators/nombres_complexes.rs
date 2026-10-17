use super::{card, int, meta, pick, sup};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

/// Pairs with an integer modulus.
#[rustfmt::skip]
const MODULI: [(i64, i64, i64); 7] = [
    (3, 4, 5), (6, 8, 10), (5, 12, 13), (8, 15, 17), (7, 24, 25), (9, 12, 15), (20, 21, 29),
];

const POWERS_OF_I: [&str; 4] = ["1", "i", "−1", "−i"];

pub struct NombresComplexes;

impl Generator for NombresComplexes {
    meta! {
        key: "nombres-complexes",
        subject: Maths,
        level: HighSchool,
        title: "Nombres complexes",
        description: "Puissances de i, module et conjugué : les automatismes qui font gagner du temps sur un calcul complexe.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        match rng.entier(1, 3) {
            1 => {
                let power = rng.entier(2, 12);
                let answer = usize::try_from(power % 4)
                    .ok()
                    .and_then(|i| POWERS_OF_I.get(i));
                card(
                    format!("i{}", sup(power)),
                    answer.copied().unwrap_or_default(),
                )
            }
            2 => {
                let (a, b, modulus) = pick(rng, &MODULI);
                let sign = if rng.entier(1, 2) == 1 {
                    " + "
                } else {
                    " − "
                };
                card(format!("|{a}{sign}{b}i|"), int(lang, modulus))
            }
            _ => {
                let a = rng.entier(1, 9);
                let b = rng.entier(1, 9);
                let negative = rng.entier(1, 2) == 1;
                let (sign, conjugate) = if negative {
                    (" − ", " + ")
                } else {
                    (" + ", " − ")
                };
                card(
                    t(lang, "Conjugué de :z", &[("z", &format!("{a}{sign}{b}i"))]),
                    format!("{a}{conjugate}{b}i"),
                )
            }
        }
    }
}
