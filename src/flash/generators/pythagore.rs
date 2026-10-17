use super::{card, int, meta, pick};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

/// The primitive Pythagorean triples with a hypotenuse under 100, scaled up
/// afterwards by an integer factor.
#[rustfmt::skip]
const PRIMITIVES: [(i64, i64, i64); 16] = [
    (3, 4, 5), (5, 12, 13), (8, 15, 17), (7, 24, 25), (20, 21, 29),
    (9, 40, 41), (12, 35, 37), (28, 45, 53), (11, 60, 61), (33, 56, 65),
    (16, 63, 65), (48, 55, 73), (13, 84, 85), (36, 77, 85), (39, 80, 89),
    (65, 72, 97),
];

/// Beyond it, the numbers stop being manageable in your head.
const MAX_HYPOTENUSE: i64 = 130;

pub struct Pythagore;

impl Generator for Pythagore {
    meta! {
        key: "theoreme-de-pythagore",
        subject: Maths,
        level: MiddleSchool,
        title: "Théorème de Pythagore",
        description: "Retrouver l’hypoténuse, le côté manquant ou le périmètre d’un triangle rectangle, sur les triplets d’entiers qui tombent juste.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let (a, b, c) = triple(rng);
        let (a_text, b_text, c_text) = (a.to_string(), b.to_string(), c.to_string());
        let missing = "Triangle rectangle : hypoténuse :c, côté :a, autre côté ?";

        match rng.entier(1, 4) {
            1 => card(
                t(
                    lang,
                    "Hypoténuse d’un triangle rectangle de côtés :a et :b",
                    &[("a", &a_text), ("b", &b_text)],
                ),
                int(lang, c),
            ),
            2 => card(
                t(lang, missing, &[("c", &c_text), ("a", &a_text)]),
                int(lang, b),
            ),
            3 => card(
                t(lang, missing, &[("c", &c_text), ("a", &b_text)]),
                int(lang, a),
            ),
            _ => card(
                t(
                    lang,
                    "Périmètre d’un triangle rectangle de côtés :a et :b",
                    &[("a", &a_text), ("b", &b_text)],
                ),
                int(lang, a + b + c),
            ),
        }
    }
}

/// A primitive triple, scaled by a factor that keeps the hypotenuse small.
fn triple(rng: &mut Hasard) -> (i64, i64, i64) {
    let (a, b, c) = pick(rng, &PRIMITIVES);
    let factor = rng.entier(1, (MAX_HYPOTENUSE / c).clamp(1, 8));
    (a * factor, b * factor, c * factor)
}
