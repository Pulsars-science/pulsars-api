use super::{card, dec, int, meta, pick};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

/// Rates that work out in your head.
const RATES: [i64; 11] = [5, 10, 20, 25, 30, 40, 50, 60, 75, 80, 90];

pub struct Pourcentages;

impl Generator for Pourcentages {
    meta! {
        key: "pourcentages",
        subject: Maths,
        level: MiddleSchool,
        title: "Pourcentages d’un nombre",
        description: "Prendre un pourcentage simple d’un nombre, et reconnaître une hausse ou une baisse.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let rate = pick(rng, &RATES);
        let number = rng.entier(1, 30) * 20;
        let rate_text = rate.to_string();
        let number_text = int(lang, number);
        let params = [
            ("taux", rate_text.as_str()),
            ("nombre", number_text.as_str()),
        ];
        let percent = |factor: i64| dec(lang, (number * factor) as f64 / 100.0, 4);

        match rng.entier(1, 3) {
            1 => card(t(lang, ":taux % de :nombre", &params), percent(rate)),
            2 => card(
                t(lang, "Augmenter :nombre de :taux %", &params),
                percent(100 + rate),
            ),
            _ => card(
                t(lang, "Diminuer :nombre de :taux %", &params),
                percent(100 - rate),
            ),
        }
    }
}
