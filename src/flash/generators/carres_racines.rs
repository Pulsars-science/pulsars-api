use super::{card, meta, sup};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

/// The largest integer whose square and root are asked.
const MAX_RANK: i64 = 30;

/// The largest number whose root is bracketed.
const MAX_BRACKET: i64 = 150;

pub struct CarresRacines;

impl Generator for CarresRacines {
    meta! {
        key: "carres-racines",
        subject: Maths,
        level: MiddleSchool,
        title: "Carrés et racines carrées",
        description: "Les carrés jusqu’à 30, les racines carrées exactes et l’encadrement des racines qui ne tombent pas juste.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        match rng.entier(1, 3) {
            1 => {
                let n = rng.entier(2, MAX_RANK);
                card(format!("{n}{}", sup(2)), (n * n).to_string())
            }
            2 => {
                // Only perfect squares: anything else would be an approximation.
                let n = rng.entier(2, MAX_RANK);
                card(format!("√{}", n * n), n.to_string())
            }
            _ => bracket(rng, lang),
        }
    }
}

/// √n between two consecutive integers, read off the table of squares.
fn bracket(rng: &mut Hasard, lang: Lang) -> Card {
    let (n, low) = loop {
        let n = rng.entier(2, MAX_BRACKET);
        // n <= 150: the square root of an exact square is exact.
        let low = (n as f64).sqrt().trunc() as i64;
        if low * low != n {
            break (n, low);
        }
    };

    card(
        t(
            lang,
            "Entre quels entiers consécutifs se trouve :racine ?",
            &[("racine", &format!("√{n}"))],
        ),
        t(
            lang,
            "entre :a et :b",
            &[("a", &low.to_string()), ("b", &(low + 1).to_string())],
        ),
    )
}
