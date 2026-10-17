use super::{card, meta, pick};
use crate::flash::i18n::tr;
use crate::flash::{Card, Generator, Hasard, Lang};

/// Element name, symbol: the first fifty-five useful boxes of the table.
#[rustfmt::skip]
const ELEMENTS: [(&str, &str); 55] = [
    ("Hydrogène", "H"), ("Hélium", "He"), ("Lithium", "Li"),
    ("Béryllium", "Be"), ("Bore", "B"), ("Carbone", "C"),
    ("Azote", "N"), ("Oxygène", "O"), ("Fluor", "F"),
    ("Néon", "Ne"), ("Sodium", "Na"), ("Magnésium", "Mg"),
    ("Aluminium", "Al"), ("Silicium", "Si"), ("Phosphore", "P"),
    ("Soufre", "S"), ("Chlore", "Cl"), ("Argon", "Ar"),
    ("Potassium", "K"), ("Calcium", "Ca"), ("Scandium", "Sc"),
    ("Titane", "Ti"), ("Vanadium", "V"), ("Chrome", "Cr"),
    ("Manganèse", "Mn"), ("Fer", "Fe"), ("Cobalt", "Co"),
    ("Nickel", "Ni"), ("Cuivre", "Cu"), ("Zinc", "Zn"),
    ("Gallium", "Ga"), ("Germanium", "Ge"), ("Arsenic", "As"),
    ("Sélénium", "Se"), ("Brome", "Br"), ("Krypton", "Kr"),
    ("Rubidium", "Rb"), ("Strontium", "Sr"), ("Zirconium", "Zr"),
    ("Argent", "Ag"), ("Cadmium", "Cd"), ("Étain", "Sn"),
    ("Antimoine", "Sb"), ("Iode", "I"), ("Xénon", "Xe"),
    ("Baryum", "Ba"), ("Tungstène", "W"), ("Platine", "Pt"),
    ("Or", "Au"), ("Mercure", "Hg"), ("Plomb", "Pb"),
    ("Bismuth", "Bi"), ("Radium", "Ra"), ("Uranium", "U"),
    ("Plutonium", "Pu"),
];

pub struct SymbolesChimiques;

impl Generator for SymbolesChimiques {
    meta! {
        key: "symboles-chimiques",
        subject: Chemistry,
        level: MiddleSchool,
        title: "Symboles des éléments chimiques",
        description: "Passer du nom d’un élément chimique à son symbole, et du symbole au nom, sur les cinquante-cinq éléments les plus courants.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let (name, symbol) = pick(rng, &ELEMENTS);

        // The symbol is universal, the name is not.
        if rng.entier(1, 2) == 1 {
            return card(tr(lang, name), symbol);
        }

        card(symbol, tr(lang, name))
    }
}
