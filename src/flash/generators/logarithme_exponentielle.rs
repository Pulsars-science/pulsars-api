use super::{card, int, meta, pick, sup};
use crate::flash::{Card, Generator, Hasard, Lang};

/// The values that are known rather than computed.
const VALUES: [(&str, &str); 5] = [
    ("ln(1)", "0"),
    ("ln(e)", "1"),
    ("e⁰", "1"),
    ("e¹", "e"),
    ("ln(e) + ln(1)", "1"),
];

pub struct LogarithmeExponentielle;

impl Generator for LogarithmeExponentielle {
    meta! {
        key: "logarithme-et-exponentielle",
        subject: Maths,
        level: HighSchool,
        title: "Logarithme et exponentielle",
        description: "Les valeurs de référence de ln et de exp, et les règles du produit, du quotient et de la puissance.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        match rng.entier(1, 4) {
            1 => {
                let power = rng.entier(2, 9);
                card(format!("ln(e{})", sup(power)), int(lang, power))
            }
            2 => {
                let a = rng.entier(2, 9);
                let b = rng.entier(2, 9);
                card(
                    format!("ln({a}) + ln({b})"),
                    format!("ln({})", int(lang, a * b)),
                )
            }
            3 => {
                let divisor = rng.entier(2, 9);
                let quotient = rng.entier(2, 9);
                card(
                    format!("ln({}) − ln({divisor})", int(lang, divisor * quotient)),
                    format!("ln({quotient})"),
                )
            }
            _ => {
                let (question, answer) = pick(rng, &VALUES);
                card(question, answer)
            }
        }
    }
}
