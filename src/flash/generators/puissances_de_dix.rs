use super::{card, dec, meta, pick, sup};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

const EXPONENTS: [i64; 9] = [-4, -3, -2, -1, 1, 2, 3, 4, 5];

pub struct PuissancesDeDix;

impl Generator for PuissancesDeDix {
    meta! {
        key: "puissances-de-dix",
        subject: Maths,
        level: HighSchool,
        title: "Puissances de dix et écriture scientifique",
        short_title: "Puissances de dix",
        description: "Passer de l’écriture scientifique à l’écriture décimale, et inversement, sans calculatrice.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let mantissa = rng.entier(11, 99) as f64 / 10.0;
        let exponent = pick(rng, &EXPONENTS);
        // JavaScript's `10 ** e`: exact for e >= 0, and the nearest double
        // of 10^e (like the literal `1e-4`) for these negative ones.
        let value = mantissa * power_of_ten(exponent);

        let scientific = format!("{} × 10{}", dec(lang, mantissa, 4), sup(exponent));
        // Eight decimals: 1.1 × 10⁻⁴ is 0.00011.
        let decimal = dec(lang, value, 8);

        if rng.entier(1, 2) == 1 {
            return card(scientific, decimal);
        }

        card(
            t(
                lang,
                "Écriture scientifique de :nombre",
                &[("nombre", &decimal)],
            ),
            scientific,
        )
    }
}

fn power_of_ten(exponent: i64) -> f64 {
    match exponent {
        -4 => 1e-4,
        -3 => 1e-3,
        -2 => 1e-2,
        -1 => 1e-1,
        e => 10f64.powi(i32::try_from(e).unwrap_or(0)),
    }
}
