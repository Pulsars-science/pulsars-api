//! Translations and number formatting of the flash questions.
//!
//! French is the source language: the keys are the French sentences, and
//! `en.json` is a copy of `pulsars-game/src/i18n/en.json` (extracted from the
//! site's `lang/en.json`). Parameters are replaced like Laravel's `__()`.

use std::collections::HashMap;
use std::sync::LazyLock;

use super::Lang;

static EN: LazyLock<HashMap<String, String>> =
    LazyLock::new(|| serde_json::from_str(include_str!("en.json")).unwrap_or_default());

/// The sentence in `lang`, or the key itself when it has no translation.
pub fn tr(lang: Lang, key: &str) -> &str {
    match lang {
        Lang::Fr => key,
        Lang::En => EN.get(key).map_or(key, String::as_str),
    }
}

/// Translates `key` and replaces its parameters: `:name`, `:Name` (first
/// letter upper-cased) and `:NAME`, in a single pass that always takes the
/// longest match (PHP's `strtr`). A `:n` never eats a `:nombre`, and an
/// inserted value is never read again.
pub fn t(lang: Lang, key: &str, params: &[(&str, &str)]) -> String {
    strtr(tr(lang, key), &replacements(params))
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The replacement table, in insertion order; a key set twice keeps its
/// first position and its last value (a JavaScript `Map`).
fn replacements(params: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut table: Vec<(String, String)> = Vec::with_capacity(params.len() * 3);
    let mut set = |key: String, value: String| match table.iter_mut().find(|(k, _)| *k == key) {
        Some(entry) => entry.1 = value,
        None => table.push((key, value)),
    };
    for (name, value) in params {
        set(format!(":{}", capitalize(name)), capitalize(value));
        set(format!(":{}", name.to_uppercase()), value.to_uppercase());
        set(format!(":{name}"), (*value).to_owned());
    }
    table
}

/// PHP's `strtr($text, $table)`: the longest key wins, in one pass.
fn strtr(text: &str, table: &[(String, String)]) -> String {
    let mut keys: Vec<&(String, String)> = table.iter().filter(|(k, _)| !k.is_empty()).collect();
    if keys.is_empty() {
        return text.to_owned();
    }
    // Stable: among keys of the same length, the first inserted wins.
    keys.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        match keys.iter().find(|(k, _)| rest.starts_with(k.as_str())) {
            Some((key, value)) => {
                out.push_str(value);
                rest = &rest[key.len()..];
            }
            None => {
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

/// A number written in `lang` like `Intl.NumberFormat(locale, {
/// maximumFractionDigits, roundingMode: 'halfEven' })` in V8: the shortest
/// decimal that round-trips the double, rounded half to even to at most
/// `decimals` digits, trailing zeros dropped, thousands grouped (U+202F in
/// French, `,` in English), decimal comma in French. The minus sign is the
/// real one (U+2212), and a negative value rounded to zero keeps it, as
/// `Intl` does.
pub fn number(lang: Lang, value: f64, decimals: usize) -> String {
    let negative = value.is_sign_negative();
    let shortest = format!("{}", value.abs());
    let (int, frac) = shortest.split_once('.').unwrap_or((&shortest, ""));
    let mut int: Vec<u8> = int.bytes().collect();
    let mut frac: Vec<u8> = frac.bytes().collect();

    if frac.len() > decimals {
        let dropped = &frac[decimals..];
        let first = dropped.first().copied().unwrap_or(b'0');
        let tail_nonzero = dropped.iter().skip(1).any(|&d| d != b'0');
        let last_kept = if decimals == 0 {
            int.last().copied()
        } else {
            frac.get(decimals - 1).copied()
        }
        .unwrap_or(b'0');
        let up = first > b'5' || (first == b'5' && (tail_nonzero || (last_kept - b'0') % 2 == 1));
        frac.truncate(decimals);
        if up {
            increment(&mut int, &mut frac);
        }
    }
    while frac.last() == Some(&b'0') {
        frac.pop();
    }

    let (group, point) = match lang {
        Lang::Fr => ("\u{202f}", ","),
        Lang::En => (",", "."),
    };
    let mut out = String::with_capacity(int.len() + frac.len() + 8);
    if negative {
        out.push('−');
    }
    for (i, digit) in int.iter().enumerate() {
        if i > 0 && (int.len() - i).is_multiple_of(3) {
            out.push_str(group);
        }
        out.push(char::from(*digit));
    }
    if !frac.is_empty() {
        out.push_str(point);
        out.extend(frac.iter().map(|&d| char::from(d)));
    }
    out
}

/// Adds one unit in the last place of `int.frac`, carrying leftwards.
fn increment(int: &mut Vec<u8>, frac: &mut [u8]) {
    for digit in frac.iter_mut().rev() {
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            return;
        }
    }
    for digit in int.iter_mut().rev() {
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            return;
        }
    }
    int.insert(0, b'1');
}

const SUPERSCRIPTS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
const SUBSCRIPTS: [char; 10] = ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'];

fn script(value: i64, digits: &[char; 10], minus: char) -> String {
    value
        .to_string()
        .chars()
        .map(|c| match c.to_digit(10) {
            Some(d) => digits.get(d as usize).copied().unwrap_or(c),
            None if c == '-' => minus,
            None => c,
        })
        .collect()
}

/// `-12` → `⁻¹²`.
pub fn superscript(value: i64) -> String {
    script(value, &SUPERSCRIPTS, '⁻')
}

/// `-12` → `₋₁₂`.
pub fn subscript(value: i64) -> String {
    script(value, &SUBSCRIPTS, '₋')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_dictionary_loads() {
        assert!(EN.len() > 300);
        assert_eq!(tr(Lang::En, "Mathématiques"), "Mathematics");
        assert_eq!(tr(Lang::En, "pas traduit"), "pas traduit");
    }

    #[test]
    fn replaces_longest_key_once() {
        let out = t(
            Lang::Fr,
            ":n et :nombre :N :NOMBRE :Nombre",
            &[("n", "x"), ("nombre", "abc")],
        );
        assert_eq!(out, "x et abc X ABC Abc");
        assert_eq!(t(Lang::Fr, ":a", &[("a", ":b"), ("b", "no")]), ":b");
    }

    #[test]
    fn formats_like_intl() {
        // Expected values printed by node 25 (V8 Intl, ICU).
        let cases: [(f64, usize, &str, &str); 14] = [
            (1000.0, 4, "1\u{202f}000", "1,000"),
            (1_234_567.5, 4, "1\u{202f}234\u{202f}567,5", "1,234,567.5"),
            (-0.0, 4, "−0", "−0"),
            (-0.00001, 4, "−0", "−0"),
            (0.00011, 4, "0,0001", "0.0001"),
            (0.00011, 8, "0,00011", "0.00011"),
            (0.125, 2, "0,12", "0.12"),
            (
                1e21,
                4,
                "1\u{202f}000\u{202f}000\u{202f}000\u{202f}000\u{202f}000\u{202f}000\u{202f}000",
                "1,000,000,000,000,000,000,000",
            ),
            (-1234.0, 4, "−1\u{202f}234", "−1,234"),
            (0.000015, 4, "0", "0"),
            (1.00005, 4, "1", "1"),
            (12345.6789, 2, "12\u{202f}345,68", "12,345.68"),
            (1e-9, 8, "0", "0"),
            (0.375, 2, "0,38", "0.38"),
        ];
        for (value, decimals, fr, en) in cases {
            assert_eq!(number(Lang::Fr, value, decimals), fr, "{value}");
            assert_eq!(number(Lang::En, value, decimals), en, "{value}");
        }
    }

    #[test]
    fn scripts() {
        assert_eq!(superscript(-12), "⁻¹²");
        assert_eq!(subscript(30), "₃₀");
    }
}
