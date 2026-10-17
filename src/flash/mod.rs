// TEMPORARY STUB: replaced by the real generators (feat/generateurs-rust).

//! Flash question generators (`docs/game-protocol.md` §2).
//!
//! This stand-in implements the public interface of the real port with one
//! trivial generator per subject, so the game server can be built and
//! tested before the 30 generators land. The game code uses only
//! [`catalogue`], [`duel_questions`], [`Lang`], [`Subject`],
//! [`DuelQuestion`] and [`Localized`]: swapping this module for the real one
//! changes nothing else.

mod hasard;

pub use hasard::Hasard;

/// Language of a question.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    /// `en` (or `en-*`) is English; anything else, absent included, French.
    pub fn from_locale(locale: Option<&str>) -> Lang {
        match locale {
            Some(l) if l == "en" || l.starts_with("en-") || l.starts_with("en_") => Lang::En,
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

/// Subject of a generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Subject {
    Maths,
    Physics,
    Chemistry,
    ComputerScience,
}

impl Subject {
    pub fn key(self) -> &'static str {
        match self {
            Subject::Maths => "maths",
            Subject::Physics => "physics",
            Subject::Chemistry => "chemistry",
            Subject::ComputerScience => "computer-science",
        }
    }

    pub fn from_key(key: &str) -> Option<Subject> {
        match key {
            "maths" => Some(Subject::Maths),
            "physics" => Some(Subject::Physics),
            "chemistry" => Some(Subject::Chemistry),
            "computer-science" => Some(Subject::ComputerScience),
            _ => None,
        }
    }
}

/// One drawn question and its answer.
pub struct Card {
    pub question: String,
    pub answer: String,
}

pub trait Generator: Send + Sync {
    fn key(&self) -> &'static str;
    fn subject(&self) -> Subject;
    fn title(&self, lang: Lang) -> String;
    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card;
}

struct Addition;
struct Multiplication;
struct Kilometres;
struct Symbols;
struct Binary;

impl Generator for Addition {
    fn key(&self) -> &'static str {
        "addition"
    }
    fn subject(&self) -> Subject {
        Subject::Maths
    }
    fn title(&self, lang: Lang) -> String {
        match lang {
            Lang::Fr => "Additions".to_owned(),
            Lang::En => "Additions".to_owned(),
        }
    }
    fn draw(&self, rng: &mut Hasard, _lang: Lang) -> Card {
        let a = rng.entier(2, 99);
        let b = rng.entier(2, 99);
        Card {
            question: format!("${a} + {b}$"),
            answer: format!("${}$", a + b),
        }
    }
}

impl Generator for Multiplication {
    fn key(&self) -> &'static str {
        "multiplication"
    }
    fn subject(&self) -> Subject {
        Subject::Maths
    }
    fn title(&self, lang: Lang) -> String {
        match lang {
            Lang::Fr => "Tables de multiplication".to_owned(),
            Lang::En => "Times tables".to_owned(),
        }
    }
    fn draw(&self, rng: &mut Hasard, _lang: Lang) -> Card {
        let a = rng.entier(2, 12);
        let b = rng.entier(2, 12);
        Card {
            question: format!("${a} \\times {b}$"),
            answer: format!("${}$", a * b),
        }
    }
}

impl Generator for Kilometres {
    fn key(&self) -> &'static str {
        "kilometres"
    }
    fn subject(&self) -> Subject {
        Subject::Physics
    }
    fn title(&self, lang: Lang) -> String {
        match lang {
            Lang::Fr => "Conversions de longueurs".to_owned(),
            Lang::En => "Length conversions".to_owned(),
        }
    }
    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let km = rng.entier(2, 90);
        let question = match lang {
            Lang::Fr => format!("${km}$ km en mètres"),
            Lang::En => format!("${km}$ km in metres"),
        };
        Card {
            question,
            answer: format!("${}$ m", km * 1000),
        }
    }
}

const ELEMENTS: &[(&str, &str, &str)] = &[
    ("H", "Hydrogène", "Hydrogen"),
    ("He", "Hélium", "Helium"),
    ("C", "Carbone", "Carbon"),
    ("N", "Azote", "Nitrogen"),
    ("O", "Oxygène", "Oxygen"),
    ("Na", "Sodium", "Sodium"),
    ("Fe", "Fer", "Iron"),
    ("Cu", "Cuivre", "Copper"),
    ("Ag", "Argent", "Silver"),
    ("Au", "Or", "Gold"),
];

impl Generator for Symbols {
    fn key(&self) -> &'static str {
        "symboles"
    }
    fn subject(&self) -> Subject {
        Subject::Chemistry
    }
    fn title(&self, lang: Lang) -> String {
        match lang {
            Lang::Fr => "Symboles chimiques".to_owned(),
            Lang::En => "Chemical symbols".to_owned(),
        }
    }
    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let last = ELEMENTS.len() as i64 - 1;
        let index = usize::try_from(rng.entier(0, last)).unwrap_or(0);
        let (symbol, fr, en) =
            ELEMENTS
                .get(index)
                .copied()
                .unwrap_or(("H", "Hydrogène", "Hydrogen"));
        let question = match lang {
            Lang::Fr => format!("Symbole de l'élément {fr}"),
            Lang::En => format!("Symbol of {en}"),
        };
        Card {
            question,
            answer: format!("$\\mathrm{{{symbol}}}$"),
        }
    }
}

impl Generator for Binary {
    fn key(&self) -> &'static str {
        "binaire"
    }
    fn subject(&self) -> Subject {
        Subject::ComputerScience
    }
    fn title(&self, lang: Lang) -> String {
        match lang {
            Lang::Fr => "Binaire vers décimal".to_owned(),
            Lang::En => "Binary to decimal".to_owned(),
        }
    }
    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let n = rng.entier(2, 63);
        let question = match lang {
            Lang::Fr => format!("${n:b}_2$ en décimal"),
            Lang::En => format!("${n:b}_2$ in decimal"),
        };
        Card {
            question,
            answer: format!("${n}$"),
        }
    }
}

static CATALOGUE: [&dyn Generator; 5] =
    [&Addition, &Multiplication, &Kilometres, &Symbols, &Binary];

/// The generators, in catalogue order.
pub fn catalogue() -> &'static [&'static dyn Generator] {
    &CATALOGUE
}

/// A question in one language.
pub struct Localized {
    pub question: String,
    /// Title of the generator.
    pub series: String,
    pub choices: [String; 4],
}

/// A question of a duel, in both languages, the right answer at the same
/// index in both.
pub struct DuelQuestion {
    pub generator: &'static str,
    pub fr: Localized,
    pub en: Localized,
    pub correct: usize,
}

impl DuelQuestion {
    pub fn text(&self, lang: Lang) -> &Localized {
        match lang {
            Lang::Fr => &self.fr,
            Lang::En => &self.en,
        }
    }
}

/// `count` questions drawn from `seed`, among the generators of `subject`
/// (`None`: every subject). Decoys are other draws of the same generator;
/// when a generator cannot give four distinct answers in 50 draws, the
/// remaining choices are padded with numbered placeholders.
pub fn duel_questions(subject: Option<Subject>, seed: u32, count: usize) -> Vec<DuelQuestion> {
    let pool: Vec<&'static dyn Generator> = catalogue()
        .iter()
        .copied()
        .filter(|g| subject.is_none_or(|s| g.subject() == s))
        .collect();
    let mut rng = Hasard::new(seed);
    let mut questions = Vec::with_capacity(count);
    if pool.is_empty() {
        return questions;
    }
    for _ in 0..count {
        let last = pool.len() as i64 - 1;
        let index = usize::try_from(rng.entier(0, last)).unwrap_or(0);
        let Some(generator) = pool.get(index).copied() else {
            continue;
        };
        // The same draw in both languages: replay the RNG state.
        let state = rng.clone();
        let fr = generator.draw(&mut rng, Lang::Fr);
        let mut replay = state;
        let en = generator.draw(&mut replay, Lang::En);

        let mut fr_choices = vec![fr.answer.clone()];
        let mut en_choices = vec![en.answer.clone()];
        let mut tries = 0;
        while fr_choices.len() < 4 && tries < 50 {
            tries += 1;
            let state = rng.clone();
            let decoy_fr = generator.draw(&mut rng, Lang::Fr);
            let mut replay = state;
            let decoy_en = generator.draw(&mut replay, Lang::En);
            if !fr_choices.contains(&decoy_fr.answer) {
                fr_choices.push(decoy_fr.answer);
                en_choices.push(decoy_en.answer);
            }
        }
        while fr_choices.len() < 4 {
            let n = fr_choices.len();
            fr_choices.push(format!("({n})"));
            en_choices.push(format!("({n})"));
        }
        let mut order = [0usize, 1, 2, 3];
        rng.melanger(&mut order);
        let pick = |choices: &[String]| -> [String; 4] {
            order.map(|i| choices.get(i).cloned().unwrap_or_default())
        };
        let correct = order.iter().position(|&i| i == 0).unwrap_or(0);
        questions.push(DuelQuestion {
            generator: generator.key(),
            fr: Localized {
                question: fr.question,
                series: generator.title(Lang::Fr),
                choices: pick(&fr_choices),
            },
            en: Localized {
                question: en.question,
                series: generator.title(Lang::En),
                choices: pick(&en_choices),
            },
            correct,
        });
    }
    questions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn questions_are_deterministic_and_well_formed() {
        for subject in [
            None,
            Some(Subject::Maths),
            Some(Subject::Physics),
            Some(Subject::Chemistry),
            Some(Subject::ComputerScience),
        ] {
            let a = duel_questions(subject, 1234, 50);
            let b = duel_questions(subject, 1234, 50);
            assert_eq!(a.len(), 50);
            for (x, y) in a.iter().zip(&b) {
                assert_eq!(x.fr.question, y.fr.question);
                assert_eq!(x.correct, y.correct);
                assert!(x.correct < 4);
                let mut distinct = x.fr.choices.to_vec();
                distinct.sort();
                distinct.dedup();
                assert_eq!(distinct.len(), 4);
                if let Some(s) = subject {
                    let g = catalogue().iter().find(|g| g.key() == x.generator);
                    assert_eq!(g.map(|g| g.subject()), Some(s));
                }
            }
        }
    }

    #[test]
    fn subjects_round_trip() {
        for s in [
            Subject::Maths,
            Subject::Physics,
            Subject::Chemistry,
            Subject::ComputerScience,
        ] {
            assert_eq!(Subject::from_key(s.key()), Some(s));
        }
        assert_eq!(Lang::from_locale(Some("en")), Lang::En);
        assert_eq!(Lang::from_locale(None), Lang::Fr);
    }
}
