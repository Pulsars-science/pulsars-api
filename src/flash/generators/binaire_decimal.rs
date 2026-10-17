use super::{card, meta};
use crate::flash::i18n::t;
use crate::flash::{Card, Generator, Hasard, Lang};

pub struct BinaireDecimal;

impl Generator for BinaireDecimal {
    meta! {
        key: "binaire-decimal",
        subject: ComputerScience,
        level: HighSchool,
        title: "Conversions binaire et hexadécimal",
        short_title: "Binaire et hexadécimal",
        description: "Convertir un octet entre base 2, base 10 et base 16, comme le fait la machine.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let value = rng.entier(1, 255);

        // Binary on 8 bits: that is how a byte reads.
        let binary = format!("{value:08b}");

        match rng.entier(1, 3) {
            1 => card(format!("{binary} (base 2)"), value.to_string()),
            2 => card(
                t(
                    lang,
                    "Écrire :valeur en binaire",
                    &[("valeur", &value.to_string())],
                ),
                binary,
            ),
            _ => card(format!("0x{value:X}"), value.to_string()),
        }
    }
}
