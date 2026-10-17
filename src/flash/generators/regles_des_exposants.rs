use super::{card, meta, sup};
use crate::flash::{Card, Generator, Hasard, Lang};

pub struct ReglesDesExposants;

impl Generator for ReglesDesExposants {
    meta! {
        key: "regles-des-exposants",
        subject: Maths,
        level: HighSchool,
        title: "Règles des exposants",
        description: "Multiplier, diviser et élever des puissances, avec ou sans coefficient devant : les règles d’exposants qui doivent devenir un réflexe.",
    }

    fn draw(&self, rng: &mut Hasard, _lang: Lang) -> Card {
        match rng.entier(1, 6) {
            1 => {
                let a = rng.entier(2, 8);
                let b = rng.entier(2, 8);
                card(format!("{} × {}", x(a), x(b)), x(a + b))
            }
            2 => {
                let a = rng.entier(2, 7);
                let b = rng.entier(2, 5);
                card(format!("({}){}", x(a), sup(b)), x(a * b))
            }
            3 => {
                // The exponents differ by at least two: « x¹ » is not written.
                let a = rng.entier(4, 12);
                let b = rng.entier(2, a - 2);
                card(format!("{} ÷ {}", x(a), x(b)), x(a - b))
            }
            4 => {
                let a = rng.entier(2, 8);
                card(format!("x{}", sup(-a)), format!("1/{}", x(a)))
            }
            5 => {
                // The coefficient multiplies, the exponent adds.
                let coefficient = rng.entier(2, 9);
                let other = rng.entier(2, 9);
                let a = rng.entier(2, 6);
                let b = rng.entier(2, 6);
                card(
                    format!("({coefficient}{})({other}{})", x(a), x(b)),
                    format!("{}{}", coefficient * other, x(a + b)),
                )
            }
            _ => {
                // Coefficients chosen to divide exactly.
                let divisor = rng.entier(2, 9);
                let quotient = rng.entier(2, 9);
                let a = rng.entier(4, 10);
                let b = rng.entier(2, a - 2);
                card(
                    format!("({}{}) ÷ ({divisor}{})", divisor * quotient, x(a), x(b)),
                    format!("{quotient}{}", x(a - b)),
                )
            }
        }
    }
}

fn x(power: i64) -> String {
    format!("x{}", sup(power))
}
