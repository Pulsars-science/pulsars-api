//! The 30 flash generators, one per file, each a line-by-line port of
//! `pulsars-game/src/flash/generateurs/<Name>.ts`. Every draw happens in the
//! same order as in TypeScript and PHP, and never depends on the language.

use super::i18n;
use super::{Card, Generator, Hasard, Lang};

mod aires_perimetres;
mod binaire_decimal;
mod carres_racines;
mod conversions_unites;
mod derivees;
mod discriminant_et_racines;
mod equations_premier_degre;
mod formules_physique;
mod fractions_decimales;
mod fractions_irreductibles;
mod identites_remarquables;
mod limites_usuelles;
mod logarithme_exponentielle;
mod moyenne_mediane;
mod nombres_complexes;
mod nombres_relatifs;
mod pgcd_ppcm;
mod pourcentages;
mod primitives_usuelles;
mod priorites_operatoires;
mod puissances_de_deux;
mod puissances_de_dix;
mod pythagore;
mod regles_des_exposants;
mod sommes_fractions;
mod symboles_chimiques;
mod tables_multiplication;
mod terme_d_une_suite;
mod unites_physique;
mod valeurs_trigonometriques;

/// The 30 generators, in the order of the site (`catalogue.ts`): it fixes
/// the order of the levels, and the "défi" picks a generator by its rank.
static CATALOGUE: [&dyn Generator; 30] = [
    &tables_multiplication::TablesMultiplication,
    &priorites_operatoires::PrioritesOperatoires,
    &fractions_decimales::FractionsDecimales,
    &fractions_irreductibles::FractionsIrreductibles,
    &sommes_fractions::SommesFractions,
    &pourcentages::Pourcentages,
    &nombres_relatifs::NombresRelatifs,
    &carres_racines::CarresRacines,
    &pgcd_ppcm::PgcdPpcm,
    &puissances_de_dix::PuissancesDeDix,
    &conversions_unites::ConversionsUnites,
    &pythagore::Pythagore,
    &aires_perimetres::AiresPerimetres,
    &moyenne_mediane::MoyenneMediane,
    &equations_premier_degre::EquationsPremierDegre,
    &identites_remarquables::IdentitesRemarquables,
    &regles_des_exposants::ReglesDesExposants,
    &discriminant_et_racines::DiscriminantEtRacines,
    &valeurs_trigonometriques::ValeursTrigonometriques,
    &logarithme_exponentielle::LogarithmeExponentielle,
    &limites_usuelles::LimitesUsuelles,
    &derivees::Derivees,
    &primitives_usuelles::PrimitivesUsuelles,
    &terme_d_une_suite::TermeDUneSuite,
    &nombres_complexes::NombresComplexes,
    &unites_physique::UnitesPhysique,
    &formules_physique::FormulesPhysique,
    &symboles_chimiques::SymbolesChimiques,
    &binaire_decimal::BinaireDecimal,
    &puissances_de_deux::PuissancesDeDeux,
];

/// The 30 generators, in the PHP catalogue order.
pub fn catalogue() -> &'static [&'static dyn Generator] {
    &CATALOGUE
}

/// The generator with this key.
pub fn find(key: &str) -> Option<&'static dyn Generator> {
    CATALOGUE.iter().copied().find(|g| g.key() == key)
}

/// The metadata methods of a generator, from its French strings.
macro_rules! meta {
    (
        key: $key:literal,
        subject: $subject:ident,
        level: $level:ident,
        title: $title:literal,
        $(short_title: $short:literal,)?
        description: $description:literal $(,)?
    ) => {
        fn key(&self) -> &'static str {
            $key
        }
        fn subject(&self) -> $crate::flash::Subject {
            $crate::flash::Subject::$subject
        }
        fn level(&self) -> $crate::flash::Level {
            $crate::flash::Level::$level
        }
        fn title(&self, lang: $crate::flash::Lang) -> String {
            $crate::flash::i18n::tr(lang, $title).to_owned()
        }
        $(
            fn short_title(&self, lang: $crate::flash::Lang) -> String {
                $crate::flash::i18n::tr(lang, $short).to_owned()
            }
        )?
        fn description(&self, lang: $crate::flash::Lang) -> String {
            $crate::flash::i18n::tr(lang, $description).to_owned()
        }
    };
}
use meta;

/// A card.
fn card(question: impl Into<String>, answer: impl Into<String>) -> Card {
    Card {
        question: question.into(),
        answer: answer.into(),
    }
}

/// `nombre()` of an integer, at most four decimals.
fn int(lang: Lang, value: i64) -> String {
    // Every value is far below 2^53: the conversion is exact.
    i18n::number(lang, value as f64, 4)
}

/// `nombre()` of a decimal value.
fn dec(lang: Lang, value: f64, decimals: usize) -> String {
    i18n::number(lang, value, decimals)
}

fn sup(value: i64) -> String {
    i18n::superscript(value)
}

/// A value picked from a constant table (`parmi`).
fn pick<T: Copy, const N: usize>(rng: &mut Hasard, values: &[T; N]) -> T {
    const { assert!(N > 0) };
    values[rng.index_below(N) % N]
}

/// PHP's `ucfirst` (only a leading ASCII `a`-`z` changes), extended to
/// any lower-case first letter for Russian, like the TypeScript port. The
/// French and English texts it is given all start with an ASCII letter, so
/// their output is unchanged.
fn ucfirst(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a } else { gcd(b, a % b) }
}
