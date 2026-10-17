use super::{card, int, meta};
use crate::flash::{Card, Generator, Hasard, Lang};

pub struct NombresRelatifs;

impl Generator for NombresRelatifs {
    meta! {
        key: "nombres-relatifs",
        subject: Maths,
        level: MiddleSchool,
        title: "Calcul avec les nombres relatifs",
        description: "Additions, soustractions et produits de nombres relatifs, là où les signes se jouent des automatismes.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let mut a = rng.entier(-12, 12);
        let mut b = rng.entier(-12, 12);

        // Zero exercises no sign rule.
        if a == 0 {
            a = 3;
        }
        if b == 0 {
            b = -4;
        }

        // Nor do two positives.
        if a > 0 && b > 0 {
            b = -b;
        }

        // A negative number is written in parentheses: « 5 − (−8) ».
        let left = parenthesized(a);
        let right = parenthesized(b);

        match rng.entier(1, 3) {
            1 => card(format!("{left} + {right}"), int(lang, a + b)),
            2 => card(format!("{left} − {right}"), int(lang, a - b)),
            _ => card(format!("{left} × {right}"), int(lang, a * b)),
        }
    }
}

fn parenthesized(value: i64) -> String {
    if value < 0 {
        format!("(−{})", value.abs())
    } else {
        value.to_string()
    }
}
