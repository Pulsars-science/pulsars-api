//! The questions of a duel: the TypeScript `defi()` (four choices per
//! question, decoys drawn from the same generator), in both languages at
//! once and for any subject.

use super::{Generator, Hasard, Lang, Subject, catalogue};

/// Choices per question.
const CHOICES: usize = 4;
/// Draws to find decoys different from the answer, per pass.
const DECOY_TRIES: usize = 60;
/// Draws of the padding (see [`question`]).
const PADDING_TRIES: usize = 240;

/// A question in one language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Localized {
    pub question: String,
    /// The generator's title.
    pub series: String,
    pub choices: [String; CHOICES],
}

/// A question in both languages; the right answer is at the same index in
/// both.
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// The questions of a match. `None` = every subject. Same algorithm as the
/// "défi" (pick a generator with `entier` among the candidates, in catalogue
/// order, draw, decoys of the same shape first, shuffle), restricted to the
/// subject instead of maths only: `Some(Subject::Maths)` reproduces the
/// TypeScript `defi()` exactly, in both languages.
///
/// Each question is drawn in French, then again in English from a copy of
/// the generator taken before it. Both runs consume the same draws (checked
/// for every subject in the tests), so the right answer lands at the same
/// index. Should a translation ever make the English run diverge, the
/// English choices follow the French decisions instead (see
/// [`question`]), which keeps the two languages aligned.
pub fn duel_questions(subject: Option<Subject>, seed: u32, count: usize) -> Vec<DuelQuestion> {
    let candidates = candidates(subject);
    if candidates.is_empty() {
        return Vec::new();
    }
    let mut rng = Hasard::new(seed);
    (0..count)
        .map(|_| {
            let start = rng.clone();
            let fr = question(&mut rng, &candidates, Lang::Fr, None);
            let mut en_rng = start.clone();
            let mut en = question(&mut en_rng, &candidates, Lang::En, None);
            if en_rng != rng || en.correct != fr.correct || en.accepted != fr.accepted {
                tracing::warn!(
                    seed,
                    generator = fr.generator,
                    "flash question diverges between languages"
                );
                en = question(
                    &mut start.clone(),
                    &candidates,
                    Lang::En,
                    Some(&fr.accepted),
                );
            }
            DuelQuestion {
                generator: fr.generator,
                fr: fr.text,
                en: en.text,
                correct: fr.correct,
            }
        })
        .collect()
}

/// The generators of a subject (all of them for `None`), in catalogue order.
fn candidates(subject: Option<Subject>) -> Vec<&'static dyn Generator> {
    catalogue()
        .iter()
        .copied()
        .filter(|g| subject.is_none_or(|s| g.subject() == s))
        .collect()
}

/// One question drawn in one language.
struct Drawn {
    generator: &'static str,
    text: Localized,
    correct: usize,
    /// For each decoy draw, whether it was kept.
    accepted: Vec<bool>,
}

/// One question of the "défi": a generator, its card, three decoys from the
/// same generator (same shape first, then any shape), all shuffled.
///
/// Padding: when the two passes find fewer than four distinct choices (the
/// "défi" then shows three; it happens, e.g. on limits whose answers are
/// `0`, `+∞` and `−∞`), the missing decoys are drawn from a copy of the
/// generator taken after the shuffle, each inserted at a random position.
/// The main stream is left untouched, so the following questions stay those
/// of the "défi".
///
/// With `follow`, each decoy is kept or rejected as in that record instead
/// of by comparing texts: the draws are then exactly those of the recorded
/// run.
fn question(
    rng: &mut Hasard,
    candidates: &[&'static dyn Generator],
    lang: Lang,
    follow: Option<&[bool]>,
) -> Drawn {
    let index = rng.index_below(candidates.len());
    let generator = candidates.get(index).copied().unwrap_or(catalogue()[0]);
    let card = generator.draw(rng, lang);

    // `true` tags the right answer, to find it after the shuffle.
    let mut list = vec![(true, card.answer.clone())];
    let mut accepted = Vec::new();
    let shape = silhouette(&card.answer);
    let mut decide = |list: &[(bool, String)], decoy: &str, same_shape: bool| {
        let keep = match follow {
            Some(record) => record.get(accepted.len()).copied().unwrap_or(false),
            None => {
                !list.iter().any(|(_, c)| c == decoy) && (!same_shape || silhouette(decoy) == shape)
            }
        };
        accepted.push(keep);
        keep
    };

    for same_shape in [true, false] {
        let mut tries = 0;
        while tries < DECOY_TRIES && list.len() < CHOICES {
            tries += 1;
            let decoy = generator.draw(rng, lang).answer;
            if decide(&list, &decoy, same_shape) {
                list.push((false, decoy));
            }
        }
    }

    let mut list = rng.melanger(&list);

    if list.len() < CHOICES {
        let mut side = rng.clone();
        let mut tries = 0;
        while tries < PADDING_TRIES && list.len() < CHOICES {
            tries += 1;
            let decoy = generator.draw(&mut side, lang).answer;
            if decide(&list, &decoy, false) {
                let at = side.index_below(list.len() + 1);
                list.insert(at, (false, decoy));
            }
        }
    }

    let correct = list.iter().position(|(right, _)| *right).unwrap_or(0);
    let mut choices = list.into_iter().map(|(_, c)| c);

    Drawn {
        generator: generator.key(),
        text: Localized {
            question: card.question,
            series: generator.title(lang),
            // Four distinct choices unless the generator has fewer than
            // four answers in its padding draws (none has).
            choices: std::array::from_fn(|_| choices.next().unwrap_or_default()),
        },
        correct,
        accepted,
    }
}

/// The shape of an answer: digits, white space, separators and signs
/// removed. PHP's pattern has the `u` flag, under which `\d` is `\p{Nd}` and
/// `\s` matches the narrow no-break space of the thousands. The only `Nd`
/// characters in answers are ASCII digits (subscripts and `½` are `No`, and
/// stay); a test checks it over every generator.
pub(crate) fn silhouette(answer: &str) -> String {
    answer
        .chars()
        .filter(|&c| {
            !(c.is_ascii_digit()
                || is_js_space(c)
                || matches!(
                    c,
                    '.' | ','
                        | '-'
                        | '−'
                        | '⁰'
                        | '¹'
                        | '²'
                        | '³'
                        | '⁴'
                        | '⁵'
                        | '⁶'
                        | '⁷'
                        | '⁸'
                        | '⁹'
                        | '⁻'
                ))
        })
        .collect()
}

/// JavaScript's `\s` (with `u`): white space and line terminators.
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subjects() -> [Option<Subject>; 5] {
        [
            None,
            Some(Subject::Maths),
            Some(Subject::Physics),
            Some(Subject::Chemistry),
            Some(Subject::ComputerScience),
        ]
    }

    /// The French and English runs of every question consume the same
    /// draws and put the right answer at the same index, without the
    /// fallback.
    #[test]
    fn languages_consume_the_same_draws() {
        for subject in subjects() {
            let candidates = candidates(subject);
            for seed in (1..=999_999).step_by(19_997).chain([2, 3, 42, 999_999]) {
                let mut rng = Hasard::new(seed);
                for _ in 0..240 {
                    let mut en_rng = rng.clone();
                    let fr = question(&mut rng, &candidates, Lang::Fr, None);
                    let en = question(&mut en_rng, &candidates, Lang::En, None);
                    assert_eq!(en_rng, rng, "{subject:?} seed {seed} {}", fr.generator);
                    assert_eq!(en.correct, fr.correct, "{subject:?} seed {seed}");
                    assert_eq!(en.accepted, fr.accepted, "{subject:?} seed {seed}");
                }
            }
        }
    }

    /// The fallback replays the French decisions exactly.
    #[test]
    fn following_a_record_draws_the_same() {
        let candidates = candidates(None);
        let mut rng = Hasard::new(77);
        for _ in 0..100 {
            let mut copy = rng.clone();
            let fr = question(&mut rng, &candidates, Lang::Fr, None);
            let en = question(&mut copy, &candidates, Lang::En, Some(&fr.accepted));
            assert_eq!(copy, rng);
            assert_eq!(en.correct, fr.correct);
        }
    }

    /// `\p{Nd}` reduces to ASCII digits: no other decimal digit occurs in
    /// an answer (the other numeric characters met are `No`).
    #[test]
    fn answers_have_only_ascii_decimal_digits() {
        let other_numeric = [
            '½', '¹', '²', '³', '⁰', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹', '₀', '₁', '₂', '₃', '₄', '₅',
            '₆', '₇', '₈', '₉',
        ];
        for g in catalogue() {
            for lang in [Lang::Fr, Lang::En] {
                let mut rng = Hasard::new(12_345);
                for _ in 0..2_000 {
                    let card = g.draw(&mut rng, lang);
                    for c in card.answer.chars() {
                        assert!(
                            !c.is_numeric() || c.is_ascii_digit() || other_numeric.contains(&c),
                            "{} {c:?}",
                            g.key()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn silhouette_strips_numbers() {
        assert_eq!(silhouette("−1\u{202f}234,5"), "");
        assert_eq!(silhouette("3/4"), "/");
        assert_eq!(silhouette("x² + 2x"), "x+x");
        assert_eq!(silhouette("n₁"), "n₁");
    }
}
