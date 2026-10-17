use super::{card, gcd, meta, pick, sup};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

/// The reference limits: function, where, limit.
const LIMITS: [(&str, &str, &str); 12] = [
    ("1/x", "+∞", "0"),
    ("1/x", "0⁺", "+∞"),
    ("1/x²", "0⁺", "+∞"),
    ("x²", "+∞", "+∞"),
    ("x²", "−∞", "+∞"),
    ("x³", "−∞", "−∞"),
    ("eˣ", "+∞", "+∞"),
    ("eˣ", "−∞", "0"),
    ("ln(x)", "+∞", "+∞"),
    ("ln(x)", "0⁺", "−∞"),
    ("√x", "+∞", "+∞"),
    ("x·ln(x)", "0⁺", "0"),
];

pub struct LimitesUsuelles;

impl Generator for LimitesUsuelles {
    meta! {
        key: "limites-usuelles",
        subject: Maths,
        level: HighSchool,
        title: "Limites usuelles",
        description: "Les limites de référence, le terme de plus haut degré des polynômes et des quotients, et les croissances comparées.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        match rng.entier(1, 4) {
            1 => polynomial(rng, lang),
            2 => quotient(rng, lang),
            3 => compared_growth(rng, lang),
            _ => {
                let (function, at, limit) = pick(rng, &LIMITS);
                limit_card(lang, function, at, limit)
            }
        }
    }
}

/// At infinity, a polynomial behaves like its leading term.
fn polynomial(rng: &mut Hasard, lang: Lang) -> Card {
    let degree = rng.entier(2, 5);
    let lead = rng.entier(2, 6);
    let negative = rng.entier(1, 2) == 1;
    let at = if rng.entier(1, 2) == 1 {
        "+∞"
    } else {
        "−∞"
    };

    let mut sign = if negative { -1 } else { 1 };

    // At −∞, an odd degree flips the sign.
    if at == "−∞" && degree % 2 == 1 {
        sign = -sign;
    }

    // The next two draws happen in writing order.
    let linear = term(rng.entier(1, 9), 1);
    let constant = rng.entier(1, 9);
    let function = format!(
        "{}{} + {linear} + {constant}",
        if negative { "−" } else { "" },
        term(lead, degree)
    );

    limit_card(lang, &function, at, if sign > 0 { "+∞" } else { "−∞" })
}

/// A quotient of polynomials at infinity: compare the degrees.
fn quotient(rng: &mut Hasard, lang: Lang) -> Card {
    let top_degree = rng.entier(1, 4);
    let bottom_degree = rng.entier(1, 4);
    let top = rng.entier(1, 9);
    let bottom = rng.entier(2, 9);

    let top_constant = rng.entier(1, 9);
    let bottom_constant = rng.entier(1, 9);
    let function = format!(
        "({} + {top_constant})/({} + {bottom_constant})",
        term(top, top_degree),
        term(bottom, bottom_degree)
    );

    let answer = if top_degree < bottom_degree {
        "0".to_owned()
    } else if top_degree > bottom_degree {
        "+∞".to_owned()
    } else {
        fraction(top, bottom)
    };

    limit_card(lang, &function, "+∞", &answer)
}

/// Exponential beats any power, which beats the logarithm.
fn compared_growth(rng: &mut Hasard, lang: Lang) -> Card {
    let n = rng.entier(1, 5);
    let power = if n == 1 {
        "x".to_owned()
    } else {
        format!("x{}", sup(n))
    };

    match rng.entier(1, 4) {
        1 => limit_card(lang, &format!("eˣ/{power}"), "+∞", "+∞"),
        2 => limit_card(lang, &format!("ln(x)/{power}"), "+∞", "0"),
        3 => limit_card(lang, &format!("{power}·eˣ"), "−∞", "0"),
        _ => limit_card(lang, &format!("{power}/eˣ"), "+∞", "0"),
    }
}

fn limit_card(lang: Lang, function: &str, at: &str, limit: &str) -> Card {
    card(
        t(lang, "Limite de :f en :en", &[("f", function), ("en", at)]),
        limit,
    )
}

/// The ratio of the leading coefficients, reduced.
fn fraction(top: i64, bottom: i64) -> String {
    let divisor = gcd(top, bottom);
    let (top, bottom) = (top / divisor, bottom / divisor);
    if bottom == 1 {
        top.to_string()
    } else {
        format!("{top}/{bottom}")
    }
}

/// A monomial written by hand: « x », « 5x³ », never « 1x¹ ».
fn term(coefficient: i64, power: i64) -> String {
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
