//! Flash questions: the 30 generators of the site, ported from
//! `pulsars-game/src/flash/` (itself a port of the Laravel site's
//! `app/QuestionsFlash/`), and the question list of a duel.
//!
//! Everything is drawn from a seed with [`Hasard`], PHP's Mt19937, so a seed
//! gives exactly the cards of the site and of the game, in French and in
//! English. `tests/flash.rs` checks it against fixtures produced by PHP and
//! by the TypeScript port.

mod duel;
pub mod generators;
mod hasard;
pub mod i18n;

pub use duel::{DuelQuestion, Localized, duel_questions};
pub use generators::catalogue;
pub use hasard::Hasard;

use serde::{Deserialize, Serialize};

/// The two languages of the game. French is the source language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    /// English for a locale starting with `en` (`en`, `en-GB`, `en_US`),
    /// French otherwise, as on the site.
    pub fn from_locale(locale: Option<&str>) -> Lang {
        match locale {
            Some(l) if l.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("en")) => Lang::En,
            _ => Lang::Fr,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }
}

/// The subject of a generator (the TypeScript `matiere()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Subject {
    Maths,
    Physics,
    Chemistry,
    ComputerScience,
}

impl Subject {
    pub const ALL: [Subject; 4] = [
        Subject::Maths,
        Subject::Physics,
        Subject::Chemistry,
        Subject::ComputerScience,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Subject::Maths => "maths",
            Subject::Physics => "physics",
            Subject::Chemistry => "chemistry",
            Subject::ComputerScience => "computer-science",
        }
    }

    pub fn from_key(key: &str) -> Option<Subject> {
        Subject::ALL.into_iter().find(|s| s.key() == key)
    }

    /// The subject's name on the site (`Mathématiques`, `Mathematics`…).
    pub fn label(self, lang: Lang) -> String {
        let fr = match self {
            Subject::Maths => "Mathématiques",
            Subject::Physics => "Physique",
            Subject::Chemistry => "Chimie",
            Subject::ComputerScience => "Informatique",
        };
        i18n::tr(lang, fr).to_owned()
    }
}

/// The school level of a generator (the TypeScript `niveau()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// Collège.
    MiddleSchool,
    /// Lycée.
    HighSchool,
}

impl Level {
    pub fn label(self, lang: Lang) -> String {
        let fr = match self {
            Level::MiddleSchool => "Collège",
            Level::HighSchool => "Lycée",
        };
        i18n::tr(lang, fr).to_owned()
    }
}

/// A question and its answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub question: String,
    pub answer: String,
}

/// A series of flash questions. `draw` consumes the same draws as the PHP
/// and TypeScript generators, in the same order, whatever the language.
pub trait Generator: Send + Sync {
    /// The series' slug on the site (`tables-multiplication`).
    fn key(&self) -> &'static str;
    fn subject(&self) -> Subject;
    fn title(&self, lang: Lang) -> String;
    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card;

    /// The title on a card: shorter when the full title is long.
    fn short_title(&self, lang: Lang) -> String {
        self.title(lang)
    }

    fn description(&self, lang: Lang) -> String;

    fn level(&self) -> Level;
}
