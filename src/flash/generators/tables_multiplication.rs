use super::{card, meta};
use crate::flash::{Card, Generator, Hasard, Lang};

pub struct TablesMultiplication;

impl Generator for TablesMultiplication {
    meta! {
        key: "tables-multiplication",
        subject: Maths,
        level: MiddleSchool,
        title: "Tables de multiplication",
        description: "Les tables de 2 à 12, dans les deux sens : le produit demandé, ou le facteur manquant.",
    }

    fn draw(&self, rng: &mut Hasard, _lang: Lang) -> Card {
        let a = rng.entier(2, 12);
        let b = rng.entier(2, 12);

        // One time in three, the missing factor: the same table, no longer
        // recited in order.
        if rng.entier(1, 3) == 1 {
            return card(format!("{a} × ? = {}", a * b), b.to_string());
        }

        card(format!("{a} × {b}"), (a * b).to_string())
    }
}
