use super::{card, meta, pick, ucfirst};
use crate::flash::i18n::{t, tr};
use crate::flash::{Card, Generator, Hasard, Lang};

/// Quantity, unit, unit symbol: the seven base units and the derived ones
/// met in class.
const QUANTITIES: [(&str, &str, &str); 36] = [
    ("une longueur", "le mètre", "m"),
    ("une masse", "le kilogramme", "kg"),
    ("une durée", "la seconde", "s"),
    ("une intensité électrique", "l’ampère", "A"),
    ("une température", "le kelvin", "K"),
    ("une quantité de matière", "la mole", "mol"),
    ("une intensité lumineuse", "la candela", "cd"),
    ("une aire", "le mètre carré", "m²"),
    ("un volume", "le mètre cube", "m³"),
    ("une vitesse", "le mètre par seconde", "m/s"),
    ("une accélération", "le mètre par seconde carrée", "m/s²"),
    (
        "une masse volumique",
        "le kilogramme par mètre cube",
        "kg/m³",
    ),
    ("un débit volumique", "le mètre cube par seconde", "m³/s"),
    ("une force", "le newton", "N"),
    ("un moment de force", "le newton-mètre", "N·m"),
    (
        "une quantité de mouvement",
        "le kilogramme-mètre par seconde",
        "kg·m/s",
    ),
    ("une énergie", "le joule", "J"),
    ("une puissance", "le watt", "W"),
    ("une pression", "le pascal", "Pa"),
    ("une fréquence", "le hertz", "Hz"),
    ("une tension électrique", "le volt", "V"),
    ("une résistance électrique", "l’ohm", "Ω"),
    ("une conductance électrique", "le siemens", "S"),
    ("une charge électrique", "le coulomb", "C"),
    ("une capacité électrique", "le farad", "F"),
    ("une inductance", "le henry", "H"),
    ("un champ magnétique", "le tesla", "T"),
    ("un flux magnétique", "le weber", "Wb"),
    ("une activité radioactive", "le becquerel", "Bq"),
    ("une dose absorbée", "le gray", "Gy"),
    ("un équivalent de dose", "le sievert", "Sv"),
    ("un flux lumineux", "le lumen", "lm"),
    ("un éclairement lumineux", "le lux", "lx"),
    ("un angle plan", "le radian", "rad"),
    ("un angle solide", "le stéradian", "sr"),
    (
        "une concentration en quantité de matière",
        "la mole par mètre cube",
        "mol/m³",
    ),
];

pub struct UnitesPhysique;

impl Generator for UnitesPhysique {
    meta! {
        key: "unites-physique",
        subject: Physics,
        level: HighSchool,
        title: "Unités du système international",
        description: "Associer chaque grandeur physique à son unité SI, reconnaître le symbole et retrouver le nom qui se cache derrière.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let (quantity, unit, symbol) = pick(rng, &QUANTITIES);

        // Quantities, units and symbols are translated too: a card read in
        // English or Russian must be entirely so (Russian writes the symbols
        // in Cyrillic, `м`, `кг`, `Дж`; `tr` leaves them as they are in
        // French and English).
        match rng.entier(1, 3) {
            1 => card(
                t(
                    lang,
                    "En quelle unité SI mesure-t-on :grandeur ?",
                    &[("grandeur", tr(lang, quantity))],
                ),
                format!("{} ({})", tr(lang, unit), tr(lang, symbol)),
            ),
            2 => card(
                t(
                    lang,
                    "Que mesure-t-on en :symbole ?",
                    &[("symbole", tr(lang, symbol))],
                ),
                ucfirst(tr(lang, quantity)),
            ),
            _ => card(
                t(
                    lang,
                    "Comment s’appelle l’unité notée :symbole ?",
                    &[("symbole", tr(lang, symbol))],
                ),
                ucfirst(tr(lang, unit)),
            ),
        }
    }
}
