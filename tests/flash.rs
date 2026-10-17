//! Parity of the flash questions with PHP and TypeScript.
//!
//! The fixtures under `tests/fixtures/flash/` come from `pulsars-game`
//! (`tests/fixtures/`, produced by the site's `outils/fixtures-flash.php`):
//! `hasard.json` (raw draws), `generators/<key>.json` (every generator, eight
//! seeds, both languages) and `defis.json` (the "défi" of `service.json`,
//! all levels). `duels.json` comes from the TypeScript port itself, through
//! `scripts/flash-fixtures.mjs`: the défi algorithm run on every subject.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashSet;
use std::path::PathBuf;

use pulsars_api::flash::{
    Card, Generator, Hasard, Lang, Level, Subject, catalogue, duel_questions,
};
use serde::Deserialize;
use serde_json::Value;

fn fixture(name: &str) -> String {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "fixtures",
        "flash",
        name,
    ]
    .iter()
    .collect();
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn langs() -> [(Lang, &'static str); 2] {
    [(Lang::Fr, "fr"), (Lang::En, "en")]
}

#[test]
fn hasard_matches_php() {
    let fixtures: Vec<Value> = serde_json::from_str(&fixture("hasard.json")).unwrap();
    assert_eq!(fixtures.len(), 8);
    for f in fixtures {
        let seed = u32::try_from(f["graine"].as_u64().unwrap()).unwrap();
        let mut rng = Hasard::new(seed);
        for (i, draw) in f["suite"].as_array().unwrap().iter().enumerate() {
            let d = draw.as_array().unwrap();
            match d[0].as_str().unwrap() {
                "entier" => {
                    let got = rng.entier(d[1].as_i64().unwrap(), d[2].as_i64().unwrap());
                    assert_eq!(got, d[3].as_i64().unwrap(), "seed {seed}, draw {i}");
                }
                "melanger" => {
                    let values: Vec<i64> = serde_json::from_value(d[1].clone()).unwrap();
                    let expected: Vec<i64> = serde_json::from_value(d[2].clone()).unwrap();
                    assert_eq!(rng.melanger(&values), expected, "seed {seed}, draw {i}");
                }
                "parmi" => {
                    let values: Vec<String> = serde_json::from_value(d[1].clone()).unwrap();
                    let got = rng.parmi(&values).unwrap();
                    assert_eq!(got, d[2].as_str().unwrap(), "seed {seed}, draw {i}");
                }
                other => panic!("unknown draw {other}"),
            }
        }
    }
}

#[derive(Deserialize)]
struct GeneratorFixture {
    cle: String,
    titre: String,
    #[serde(rename = "titreCourt")]
    titre_court: String,
    description: String,
    matiere: String,
    niveau: String,
    series: Vec<Series>,
}

#[derive(Deserialize)]
struct Series {
    graine: u32,
    cartes: Vec<NumberedCard>,
}

#[derive(Deserialize, Debug, PartialEq)]
struct NumberedCard {
    numero: usize,
    question: String,
    reponse: String,
}

/// The TypeScript `serie()`: a card drawn again (up to 12 times) while it
/// repeats a question already on the wall.
fn wall(generator: &dyn Generator, seed: u32, count: usize, lang: Lang) -> Vec<NumberedCard> {
    let mut rng = Hasard::new(seed);
    let mut seen = HashSet::new();
    (1..=count)
        .map(|numero| {
            let mut card: Card = generator.draw(&mut rng, lang);
            let mut tries = 0;
            while tries < 12 && seen.contains(&card.question) {
                card = generator.draw(&mut rng, lang);
                tries += 1;
            }
            seen.insert(card.question.clone());
            NumberedCard {
                numero,
                question: card.question,
                reponse: card.answer,
            }
        })
        .collect()
}

#[test]
fn every_generator_matches_php_in_both_languages() {
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "fixtures",
        "flash",
        "generators",
    ]
    .iter()
    .collect();
    let mut checked = HashSet::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let all: serde_json::Map<String, Value> =
            serde_json::from_str(&fixture(&format!("generators/{name}"))).unwrap();
        for (lang, code) in langs() {
            let f: GeneratorFixture = serde_json::from_value(all[code].clone()).unwrap();
            let g = catalogue()
                .iter()
                .find(|g| g.key() == f.cle)
                .unwrap_or_else(|| panic!("no generator {}", f.cle));
            assert_eq!(g.title(lang), f.titre, "{} {code}", f.cle);
            assert_eq!(g.short_title(lang), f.titre_court, "{} {code}", f.cle);
            assert_eq!(g.description(lang), f.description, "{} {code}", f.cle);
            assert_eq!(g.subject().label(lang), f.matiere, "{} {code}", f.cle);
            assert_eq!(g.level().label(lang), f.niveau, "{} {code}", f.cle);
            for s in &f.series {
                let got = wall(*g, s.graine, s.cartes.len(), lang);
                assert_eq!(got, s.cartes, "{} {code} seed {}", f.cle, s.graine);
            }
            checked.insert(f.cle);
        }
    }
    assert_eq!(checked.len(), 30);
}

#[test]
fn catalogue_order_and_subjects() {
    let keys: Vec<&str> = catalogue().iter().map(|g| g.key()).collect();
    assert_eq!(keys.len(), 30);
    assert_eq!(keys[0], "tables-multiplication");
    assert_eq!(keys[29], "puissances-de-deux");
    let count = |s| catalogue().iter().filter(|g| g.subject() == s).count();
    assert_eq!(count(Subject::Maths), 25);
    assert_eq!(count(Subject::Physics), 2);
    assert_eq!(count(Subject::Chemistry), 1);
    assert_eq!(count(Subject::ComputerScience), 2);
    let levels = catalogue()
        .iter()
        .filter(|g| g.level() == Level::MiddleSchool)
        .count();
    assert!(levels > 0 && levels < 30);
}

#[test]
fn subject_and_lang_keys() {
    for s in Subject::ALL {
        assert_eq!(Subject::from_key(s.key()), Some(s));
        assert_eq!(serde_json::to_value(s).unwrap(), s.key());
    }
    assert_eq!(Subject::from_key("biology"), None);
    assert_eq!(Lang::from_locale(Some("en")), Lang::En);
    assert_eq!(Lang::from_locale(Some("en-GB")), Lang::En);
    assert_eq!(Lang::from_locale(Some("fr")), Lang::Fr);
    assert_eq!(Lang::from_locale(Some("de")), Lang::Fr);
    assert_eq!(Lang::from_locale(Some("e")), Lang::Fr);
    assert_eq!(Lang::from_locale(None), Lang::Fr);
    assert_eq!(Lang::En.code(), "en");
}

#[derive(Deserialize)]
struct Defi {
    graine: u32,
    questions: Vec<DefiQuestion>,
}

#[derive(Deserialize)]
struct DefiQuestion {
    question: String,
    reponse: String,
    choix: Vec<String>,
    serie: String,
}

/// Compares duel questions with a list of "défi" questions in one language.
fn assert_same(
    got: &[pulsars_api::flash::DuelQuestion],
    expected: &[DefiQuestion],
    lang: Lang,
    what: &str,
    padded: &mut usize,
) {
    assert_eq!(got.len(), expected.len(), "{what}");
    for (i, (q, e)) in got.iter().zip(expected).enumerate() {
        let text = q.text(lang);
        assert_eq!(text.question, e.question, "{what} #{i}");
        assert_eq!(text.series, e.serie, "{what} #{i}");
        if e.choix.len() == 4 {
            assert_eq!(text.choices.as_slice(), e.choix.as_slice(), "{what} #{i}");
        } else {
            // Padded: the défi's choices in the same order, plus new ones.
            let kept: Vec<&String> = text
                .choices
                .iter()
                .filter(|c| e.choix.contains(c))
                .collect();
            assert_eq!(kept, e.choix.iter().collect::<Vec<_>>(), "{what} #{i}");
            *padded += 1;
        }
        assert_eq!(text.choices[q.correct], e.reponse, "{what} #{i}");
    }
}

#[test]
fn maths_duel_matches_php_defi() {
    let all: serde_json::Map<String, Value> = serde_json::from_str(&fixture("defis.json")).unwrap();
    let (mut checked, mut padded) = (0, 0);
    for (lang, code) in langs() {
        let defis: Vec<Defi> = serde_json::from_value(all[code].clone()).unwrap();
        for d in defis {
            let got = duel_questions(Some(Subject::Maths), d.graine, d.questions.len());
            assert_same(
                &got,
                &d.questions,
                lang,
                &format!("{code} seed {}", d.graine),
                &mut padded,
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 32);
    // The défi shows three choices on a few questions; the duel pads them.
    assert!(padded > 0);
}

#[derive(Deserialize)]
struct Duel {
    subject: Option<Subject>,
    graine: u32,
    fr: Vec<DefiQuestion>,
    en: Vec<DefiQuestion>,
}

#[test]
fn duel_matches_typescript_on_every_subject() {
    let duels: Vec<Duel> = serde_json::from_str(&fixture("duels.json")).unwrap();
    assert!(!duels.is_empty());
    for d in &duels {
        let got = duel_questions(d.subject, d.graine, d.fr.len());
        let what = format!("{:?} seed {}", d.subject, d.graine);
        let mut padded = 0;
        assert_same(&got, &d.fr, Lang::Fr, &what, &mut padded);
        assert_same(&got, &d.en, Lang::En, &what, &mut padded);
    }
}

#[test]
fn duel_has_four_distinct_choices() {
    for subject in [
        None,
        Some(Subject::Maths),
        Some(Subject::Physics),
        Some(Subject::Chemistry),
        Some(Subject::ComputerScience),
    ] {
        for seed in [1, 2, 3, 500_000, 999_999] {
            for q in duel_questions(subject, seed, 240) {
                for lang in [Lang::Fr, Lang::En] {
                    let choices: HashSet<&String> = q.text(lang).choices.iter().collect();
                    assert_eq!(choices.len(), 4, "{subject:?} {seed} {}", q.generator);
                }
                assert!(q.correct < 4);
            }
        }
    }
    assert!(duel_questions(Some(Subject::Maths), 7, 0).is_empty());
}
