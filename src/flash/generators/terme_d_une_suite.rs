use super::{card, int, meta};
use crate::flash::i18n::{subscript, t};
use crate::flash::{Card, Generator, Hasard, Lang};

pub struct TermeDUneSuite;

impl Generator for TermeDUneSuite {
    meta! {
        key: "terme-d-une-suite",
        subject: Maths,
        level: HighSchool,
        title: "Terme d’une suite",
        description: "Calculer un terme d’une suite arithmétique ou géométrique à partir du premier terme et de la raison.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let first = rng.entier(1, 9);
        let rank = rng.entier(3, 8);
        let zero = subscript(0);
        let first_text = int(lang, first);

        if rng.entier(1, 2) == 1 {
            let magnitude = rng.entier(2, 9);
            let ratio = magnitude * if rng.entier(1, 4) == 1 { -1 } else { 1 };
            return card(
                t(
                    lang,
                    "u:zero = :premier et r = :raison : u:rang ?",
                    &[
                        ("zero", &zero),
                        ("premier", &first_text),
                        ("raison", &int(lang, ratio)),
                        ("rang", &subscript(rank)),
                    ],
                ),
                int(lang, first + rank * ratio),
            );
        }

        // A small geometric ratio, and a small rank with it.
        let ratio = rng.entier(2, 3);
        let rank = rng.entier(3, 5);

        card(
            t(
                lang,
                "u:zero = :premier et q = :raison : u:rang ?",
                &[
                    ("zero", &zero),
                    ("premier", &first_text),
                    ("raison", &int(lang, ratio)),
                    ("rang", &subscript(rank)),
                ],
            ),
            int(lang, first * ratio.pow(u32::try_from(rank).unwrap_or(0))),
        )
    }
}
