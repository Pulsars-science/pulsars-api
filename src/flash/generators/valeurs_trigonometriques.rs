use super::{card, meta, pick};
use crate::flash::{Card, Generator, Hasard, Lang};

/// Radians, degrees, cosine, sine, tangent (empty where undefined), over
/// the whole circle.
const ANGLES: [(&str, i64, &str, &str, &str); 17] = [
    ("0", 0, "1", "0", "0"),
    ("π/6", 30, "√3/2", "1/2", "√3/3"),
    ("π/4", 45, "√2/2", "√2/2", "1"),
    ("π/3", 60, "1/2", "√3/2", "√3"),
    ("π/2", 90, "0", "1", ""),
    ("2π/3", 120, "−1/2", "√3/2", "−√3"),
    ("3π/4", 135, "−√2/2", "√2/2", "−1"),
    ("5π/6", 150, "−√3/2", "1/2", "−√3/3"),
    ("π", 180, "−1", "0", "0"),
    ("7π/6", 210, "−√3/2", "−1/2", "√3/3"),
    ("5π/4", 225, "−√2/2", "−√2/2", "1"),
    ("4π/3", 240, "−1/2", "−√3/2", "√3"),
    ("3π/2", 270, "0", "−1", ""),
    ("5π/3", 300, "1/2", "−√3/2", "−√3"),
    ("7π/4", 315, "√2/2", "−√2/2", "−1"),
    ("11π/6", 330, "√3/2", "−1/2", "−√3/3"),
    ("2π", 360, "1", "0", "0"),
];

pub struct ValeursTrigonometriques;

impl Generator for ValeursTrigonometriques {
    meta! {
        key: "valeurs-trigonometriques",
        subject: Maths,
        level: HighSchool,
        title: "Valeurs remarquables du cercle trigonométrique",
        short_title: "Cercle trigonométrique",
        description: "Les cosinus, sinus et tangentes des angles remarquables du tour complet, en radians comme en degrés.",
    }

    fn draw(&self, rng: &mut Hasard, _lang: Lang) -> Card {
        let (radians, degrees, cosine, sine, tangent) = pick(rng, &ANGLES);

        // The tangent is only asked where it exists.
        let (name, value) = if tangent.is_empty() {
            pick(rng, &[("cos", cosine), ("sin", sine)])
        } else {
            pick(rng, &[("cos", cosine), ("sin", sine), ("tan", tangent)])
        };

        let angle = if rng.entier(1, 2) == 1 {
            radians.to_owned()
        } else {
            format!("{degrees}°")
        };

        // One time in three, the opposite angle (down to −π): cosine is
        // even, sine and tangent are odd. Drawn only in that range.
        if (30..=180).contains(&degrees) && rng.entier(1, 3) == 1 {
            let answer = if name == "cos" {
                value.to_owned()
            } else {
                opposite(value)
            };
            return card(format!("{name}(−{angle})"), answer);
        }

        card(format!("{name}({angle})"), value)
    }
}

/// The opposite of a table value, zero excepted: « √3/2 » → « −√3/2 ».
fn opposite(value: &str) -> String {
    if value == "0" {
        return "0".to_owned();
    }
    match value.strip_prefix('−') {
        Some(rest) => rest.to_owned(),
        None => format!("−{value}"),
    }
}
