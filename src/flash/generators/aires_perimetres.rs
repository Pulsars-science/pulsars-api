use super::{card, int, meta};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

pub struct AiresPerimetres;

impl Generator for AiresPerimetres {
    meta! {
        key: "aires-et-perimetres",
        subject: Maths,
        level: MiddleSchool,
        title: "Aires et périmètres",
        description: "Calculer l’aire ou le périmètre des figures usuelles — carré, rectangle, triangle, disque — sans ouvrir le formulaire.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let a = rng.entier(2, 12);
        let b = rng.entier(2, 12);
        let (a_text, b_text) = (a.to_string(), b.to_string());

        match rng.entier(1, 6) {
            1 => card(
                t(
                    lang,
                    "Aire d’un rectangle :a × :b",
                    &[("a", &a_text), ("b", &b_text)],
                ),
                int(lang, a * b),
            ),
            2 => card(
                t(
                    lang,
                    "Périmètre d’un rectangle :a × :b",
                    &[("a", &a_text), ("b", &b_text)],
                ),
                int(lang, 2 * (a + b)),
            ),
            3 => card(
                t(lang, "Aire d’un carré de côté :a", &[("a", &a_text)]),
                int(lang, a * a),
            ),
            // The base is doubled so that the area is exact.
            4 => card(
                t(
                    lang,
                    "Aire d’un triangle de base :b et de hauteur :h",
                    &[("b", &(2 * a).to_string()), ("h", &b_text)],
                ),
                int(lang, a * b),
            ),
            // Answers stay in multiples of π.
            5 => card(
                t(lang, "Aire d’un disque de rayon :r", &[("r", &a_text)]),
                format!("{}π", int(lang, a * a)),
            ),
            _ => card(
                t(lang, "Périmètre d’un cercle de rayon :r", &[("r", &a_text)]),
                format!("{}π", int(lang, 2 * a)),
            ),
        }
    }
}
