use super::{card, meta, pick, sup};
use crate::flash::{Card, Generator, Hasard, Lang};

/// The standard derivatives, apart from powers and simple compositions.
const STANDARD: [(&str, &str); 7] = [
    ("1/x", "−1/x²"),
    ("√x", "1/(2√x)"),
    ("eˣ", "eˣ"),
    ("ln(x)", "1/x"),
    ("sin(x)", "cos(x)"),
    ("cos(x)", "−sin(x)"),
    ("tan(x)", "1 + tan²(x)"),
];

pub struct Derivees;

impl Generator for Derivees {
    meta! {
        key: "derivees-usuelles",
        subject: Maths,
        level: HighSchool,
        title: "Dérivées usuelles",
        description: "Dériver de mémoire : fonctions de référence, puissances de x, polynômes et composées simples.",
    }

    fn draw(&self, rng: &mut Hasard, _lang: Lang) -> Card {
        match rng.entier(1, 6) {
            1 | 2 => {
                let coefficient = rng.entier(1, 9);
                let power = rng.entier(2, 8);
                card(
                    format!("({})′", term(coefficient, power)),
                    term(coefficient * power, power - 1),
                )
            }
            3 => {
                let a = rng.entier(1, 9);
                let b = rng.entier(1, 9);
                let c = rng.entier(1, 9);
                card(
                    format!("({} + {} + {c})′", term(a, 2), term(b, 1)),
                    format!("{} + {b}", term(2 * a, 1)),
                )
            }
            4 => composed(rng),
            5 => inverse_and_root(rng),
            _ => {
                let (function, derivative) = pick(rng, &STANDARD);
                card(format!("({function})′"), derivative)
            }
        }
    }
}

/// The shortest compositions; « ln(ax) » is the trap card.
fn composed(rng: &mut Hasard) -> Card {
    let a = rng.entier(2, 9);

    match rng.entier(1, 4) {
        1 => card(format!("(e{}ˣ)′", sup(a)), format!("{a}e{}ˣ", sup(a))),
        2 => card(format!("(ln({a}x))′"), "1/x"),
        3 => card(format!("(sin({a}x))′"), format!("{a}cos({a}x)")),
        _ => card(format!("(cos({a}x))′"), format!("−{a}sin({a}x)")),
    }
}

/// The coefficient stays in front of the standard derivative.
fn inverse_and_root(rng: &mut Hasard) -> Card {
    let a = rng.entier(2, 9);

    if rng.entier(1, 2) == 1 {
        return card(format!("({a}/x)′"), format!("−{a}/x{}", sup(2)));
    }

    // a√x simplifies when a is even.
    let (numerator, denominator) = if a % 2 == 0 {
        (a / 2, "√x")
    } else {
        (a, "(2√x)")
    };

    card(format!("({a}√x)′"), format!("{numerator}/{denominator}"))
}

/// A monomial written by hand: « x », not « 1x¹ »; « 7 », not « 7x⁰ ».
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
