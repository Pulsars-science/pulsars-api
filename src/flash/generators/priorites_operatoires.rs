use super::{card, int, meta};
use crate::flash::{Card, Generator, Hasard, Lang};

pub struct PrioritesOperatoires;

impl Generator for PrioritesOperatoires {
    meta! {
        key: "priorites-operatoires",
        subject: Maths,
        level: MiddleSchool,
        title: "Priorités opératoires",
        description: "Enchaîner additions, multiplications et parenthèses dans le bon ordre, sans se laisser piéger par l’écriture du calcul.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let a = rng.entier(2, 9);
        let b = rng.entier(2, 9);
        let c = rng.entier(2, 9);

        match rng.entier(1, 4) {
            1 => card(format!("{a} + {b} × {c}"), int(lang, a + b * c)),
            2 => card(format!("({a} + {b}) × {c}"), int(lang, (a + b) * c)),
            3 => card(format!("{a} × {b} − {c}"), int(lang, a * b - c)),
            _ => card(format!("{a} × ({b} + {c})"), int(lang, a * (b + c))),
        }
    }
}
