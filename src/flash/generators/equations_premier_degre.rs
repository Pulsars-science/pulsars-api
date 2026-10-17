use super::{card, int, meta};
use crate::flash::{Card, Generator, Hasard, Lang};

pub struct EquationsPremierDegre;

impl Generator for EquationsPremierDegre {
    meta! {
        key: "equations-premier-degre",
        subject: Maths,
        level: MiddleSchool,
        title: "Équations du premier degré",
        description: "Résoudre de tête une équation de la forme ax + b = c, à solution entière.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let a = rng.entier(2, 9);
        let x = rng.entier(-8, 10);
        let b = rng.entier(-15, 15);
        let c = a * x + b;

        let side = match b {
            b if b < 0 => format!("{a}x − {}", b.abs()),
            0 => format!("{a}x"),
            b => format!("{a}x + {b}"),
        };

        card(
            format!("{side} = {}", int(lang, c)),
            format!("x = {}", int(lang, x)),
        )
    }
}
