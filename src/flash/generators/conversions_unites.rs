use super::{card, dec, meta, pick};
use crate::flash::{Card, Generator, Hasard, Lang};

/// From unit, to unit, and the factor from one to the other.
const FAMILIES: [(&str, &str, f64); 15] = [
    ("km", "m", 1000.0),
    ("m", "cm", 100.0),
    ("cm", "mm", 10.0),
    ("m", "km", 0.001),
    ("cm", "m", 0.01),
    ("kg", "g", 1000.0),
    ("g", "mg", 1000.0),
    ("g", "kg", 0.001),
    ("t", "kg", 1000.0),
    ("L", "mL", 1000.0),
    ("L", "cL", 100.0),
    ("mL", "L", 0.001),
    ("h", "min", 60.0),
    ("min", "s", 60.0),
    ("h", "s", 3600.0),
];

pub struct ConversionsUnites;

impl Generator for ConversionsUnites {
    meta! {
        key: "conversions-unites",
        subject: Maths,
        level: MiddleSchool,
        title: "Conversions d’unités",
        description: "Longueurs, masses, contenances et durées : changer d’unité sans tableau de conversion.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let (from, to, factor) = pick(rng, &FAMILIES);

        // An integer, or a number with one decimal.
        let value = if rng.entier(1, 2) == 1 {
            rng.entier(2, 95) as f64
        } else {
            rng.entier(11, 95) as f64 / 10.0
        };

        card(
            format!("{} {from} = ? {to}", dec(lang, value, 4)),
            format!("{} {to}", dec(lang, value * factor, 6)),
        )
    }
}
