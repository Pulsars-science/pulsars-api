use super::{card, int, meta, pick};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

/// Coprime pairs: times a common factor, their GCD is exactly that factor.
#[rustfmt::skip]
const PAIRS: [(i64, i64); 15] = [
    (2, 3), (3, 4), (2, 5), (3, 5), (4, 5), (5, 6),
    (2, 7), (3, 7), (4, 7), (5, 7), (3, 8), (5, 8),
    (7, 8), (4, 9), (5, 9),
];

pub struct PgcdPpcm;

impl Generator for PgcdPpcm {
    meta! {
        key: "pgcd-et-ppcm",
        subject: Maths,
        level: MiddleSchool,
        title: "PGCD et PPCM",
        description: "Trouver le plus grand diviseur commun et le plus petit multiple commun de deux entiers, sans poser la décomposition.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let (m, n) = pick(rng, &PAIRS);
        let factor = rng.entier(2, 12);

        let a = (factor * m).to_string();
        let b = (factor * n).to_string();
        let params = [("a", a.as_str()), ("b", b.as_str())];

        if rng.entier(1, 2) == 1 {
            return card(t(lang, "PGCD(:a ; :b)", &params), int(lang, factor));
        }

        card(t(lang, "PPCM(:a ; :b)", &params), int(lang, factor * m * n))
    }
}
