use super::{card, int, meta};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

pub struct DiscriminantEtRacines;

impl Generator for DiscriminantEtRacines {
    meta! {
        key: "discriminant-et-racines",
        subject: Maths,
        level: HighSchool,
        title: "Discriminant et racines",
        description: "Discriminant, somme et produit des racines d’un trinôme du second degré, à lire directement sur les coefficients.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        // The trinomial is built from its roots: the discriminant is a
        // perfect square and the roots are integers.
        let r1 = rng.entier(-6, 6);
        let mut r2 = rng.entier(-6, 6);

        if r1 == r2 {
            r2 = r1 + 1;
        }

        let sum = r1 + r2;
        let product = r1 * r2;
        let trinomial = trinomial(lang, -sum, product);
        let params = [("trinome", trinomial.as_str())];

        match rng.entier(1, 4) {
            1 => card(
                t(lang, "Δ de :trinome", &params),
                int(lang, (r1 - r2).pow(2)),
            ),
            2 => card(
                t(lang, "Somme des racines de :trinome", &params),
                int(lang, sum),
            ),
            3 => card(
                t(lang, "Produit des racines de :trinome", &params),
                int(lang, product),
            ),
            _ => card(
                t(lang, "Racines de :trinome", &params),
                t(
                    lang,
                    ":a et :b",
                    &[("a", &int(lang, r1.min(r2))), ("b", &int(lang, r1.max(r2)))],
                ),
            ),
        }
    }
}

/// x² + bx + c, with the right signs.
fn trinomial(lang: Lang, b: i64, c: i64) -> String {
    let term = |value: i64, suffix: &str| {
        let sign = if value < 0 { " − " } else { " + " };
        let coefficient = if value.abs() == 1 && !suffix.is_empty() {
            String::new()
        } else {
            int(lang, value.abs())
        };
        format!("{sign}{coefficient}{suffix}")
    };

    let mut out = format!("x{}", super::sup(2));
    if b != 0 {
        out.push_str(&term(b, "x"));
    }
    if c != 0 {
        out.push_str(&term(c, ""));
    }
    out
}
