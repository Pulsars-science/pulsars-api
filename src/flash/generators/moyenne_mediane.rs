use super::{card, dec, int, meta};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

pub struct MoyenneMediane;

impl Generator for MoyenneMediane {
    meta! {
        key: "moyenne-et-mediane",
        subject: Maths,
        level: MiddleSchool,
        title: "Moyenne et médiane",
        description: "Lire une petite série de nombres et en donner la moyenne, la médiane ou l’étendue sans poser d’opération.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        // Five values: the median is a value of the series.
        let values: [i64; 5] = std::array::from_fn(|_| rng.entier(1, 20));
        let mut sorted = values;
        sorted.sort_unstable();

        let series = values.map(|v| v.to_string()).join(" ; ");
        let params = [("serie", series.as_str())];

        match rng.entier(1, 3) {
            1 => card(t(lang, "Médiane de :serie", &params), int(lang, sorted[2])),
            2 => card(
                t(lang, "Étendue de :serie", &params),
                int(lang, sorted[4] - sorted[0]),
            ),
            _ => card(
                t(lang, "Moyenne de :serie", &params),
                dec(lang, values.iter().sum::<i64>() as f64 / 5.0, 2),
            ),
        }
    }
}
