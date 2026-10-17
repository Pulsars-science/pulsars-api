use super::{card, meta, pick, sup};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

/// The table of derivatives read backwards; the constant is implied.
const STANDARD: [(&str, &str); 8] = [
    ("cos(x)", "sin(x)"),
    ("sin(x)", "−cos(x)"),
    ("eˣ", "eˣ"),
    ("1/x", "ln(x)"),
    ("1/x²", "−1/x"),
    ("1", "x"),
    ("1/√x", "2√x"),
    ("1 + tan²(x)", "tan(x)"),
];

pub struct PrimitivesUsuelles;

impl Generator for PrimitivesUsuelles {
    meta! {
        key: "primitives-usuelles",
        subject: Maths,
        level: HighSchool,
        title: "Primitives usuelles",
        description: "Remonter aux primitives à la constante près : fonctions de référence, puissances, polynômes et composées simples.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let of = |f: &str| t(lang, "Primitive de :f", &[("f", f)]);

        match rng.entier(1, 6) {
            1 | 2 => {
                // The coefficient is a multiple of the new rank: « 6x² » → « 2x³ ».
                let power = rng.entier(2, 7);
                let quotient = rng.entier(1, 9);
                card(
                    of(&term(quotient * (power + 1), power)),
                    term(quotient, power + 1),
                )
            }
            3 => {
                let a = rng.entier(1, 6);
                let b = rng.entier(1, 6);
                card(
                    of(&format!("{} + {}", term(3 * a, 2), term(2 * b, 1))),
                    format!("{} + {}", term(a, 3), term(b, 2)),
                )
            }
            4 => {
                let a = rng.entier(2, 9);
                let exponent = sup(a);
                match rng.entier(1, 3) {
                    1 => card(of(&format!("e{exponent}ˣ")), format!("e{exponent}ˣ/{a}")),
                    2 => card(of(&format!("cos({a}x)")), format!("sin({a}x)/{a}")),
                    _ => card(of(&format!("sin({a}x)")), format!("−cos({a}x)/{a}")),
                }
            }
            5 => {
                let a = rng.entier(2, 9);
                match rng.entier(1, 3) {
                    1 => card(of(&a.to_string()), term(a, 1)),
                    2 => card(of(&format!("{a}/x")), format!("{a}ln(x)")),
                    _ => card(of(&format!("{a}eˣ")), format!("{a}eˣ")),
                }
            }
            _ => {
                let (function, primitive) = pick(rng, &STANDARD);
                card(of(function), primitive)
            }
        }
    }
}

/// A monomial written by hand: « x », « 5x³ », never « 1x¹ ».
fn term(coefficient: i64, power: i64) -> String {
    if power == 0 {
        return coefficient.to_string();
    }
    let factor = if coefficient == 1 {
        String::new()
    } else {
        coefficient.to_string()
    };
    let exponent = if power == 1 {
        String::new()
    } else {
        sup(power)
    };
    format!("{factor}x{exponent}")
}
