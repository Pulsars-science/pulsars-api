use super::{card, int, meta, sup};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

/// Up to 2²⁴ both ways.
const MAX_EXPONENT: i64 = 24;

/// Word sizes for the coding questions; from two bits, or the question
/// would say « on 1 bits ».
const MIN_BITS: i64 = 2;
const MAX_BITS: i64 = 20;

pub struct PuissancesDeDeux;

impl Generator for PuissancesDeDeux {
    meta! {
        key: "puissances-de-deux",
        subject: ComputerScience,
        level: HighSchool,
        title: "Puissances de deux",
        description: "De 2¹ à 2²⁴, dans les deux sens, et ce qu’elles disent du codage : nombre de valeurs, nombre de bits, plus grand entier.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        match rng.entier(1, 5) {
            1 => {
                let exponent = rng.entier(1, MAX_EXPONENT);
                card(format!("2{}", sup(exponent)), int(lang, pow2(exponent)))
            }
            2 => {
                let exponent = rng.entier(1, MAX_EXPONENT);
                card(
                    t(
                        lang,
                        "Quelle puissance de 2 vaut :valeur ?",
                        &[("valeur", &int(lang, pow2(exponent)))],
                    ),
                    format!("2{}", sup(exponent)),
                )
            }
            3 => {
                let bits = rng.entier(MIN_BITS, MAX_BITS);
                card(
                    t(
                        lang,
                        "Combien de valeurs différentes code-t-on sur :bits bits ?",
                        &[("bits", &bits.to_string())],
                    ),
                    int(lang, pow2(bits)),
                )
            }
            4 => {
                let bits = rng.entier(MIN_BITS, MAX_BITS);
                card(
                    t(
                        lang,
                        "Combien de bits pour coder :valeur valeurs ?",
                        &[("valeur", &int(lang, pow2(bits)))],
                    ),
                    int(lang, bits),
                )
            }
            _ => {
                // 8 bits, 256 values, but 255 at most: counting starts at zero.
                let bits = rng.entier(MIN_BITS, MAX_BITS);
                card(
                    t(
                        lang,
                        "Quel est le plus grand entier non signé codable sur :bits bits ?",
                        &[("bits", &bits.to_string())],
                    ),
                    int(lang, pow2(bits) - 1),
                )
            }
        }
    }
}

fn pow2(exponent: i64) -> i64 {
    1i64 << exponent.clamp(0, 62)
}
