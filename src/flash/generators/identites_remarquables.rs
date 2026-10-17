use super::{card, meta, sup};
use crate::flash::{Card, Generator, Hasard, Lang};

pub struct IdentitesRemarquables;

impl Generator for IdentitesRemarquables {
    meta! {
        key: "identites-remarquables",
        subject: Maths,
        level: HighSchool,
        title: "Identités remarquables",
        description: "Développer (a + b)², (a − b)² et (a + b)(a − b), coefficient devant x compris, et les reconnaître pour factoriser.",
    }

    fn draw(&self, rng: &mut Hasard, _lang: Lang) -> Card {
        // Three expansions for one factorisation.
        if rng.entier(1, 4) == 1 {
            factorisation(rng)
        } else {
            expansion(rng)
        }
    }
}

fn expansion(rng: &mut Hasard) -> Card {
    let a = rng.entier(1, 6);
    let b = rng.entier(1, 12);
    let square = sup(2);

    let term = if a == 1 {
        "x".to_owned()
    } else {
        format!("{a}x")
    };
    let lead = format!(
        "{}x{square}",
        if a * a == 1 {
            String::new()
        } else {
            (a * a).to_string()
        }
    );

    match rng.entier(1, 3) {
        1 => card(
            format!("({term} + {b}){square}"),
            format!("{lead} + {}x + {}", 2 * a * b, b * b),
        ),
        2 => card(
            format!("({term} − {b}){square}"),
            format!("{lead} − {}x + {}", 2 * a * b, b * b),
        ),
        _ => card(
            format!("({term} + {b})({term} − {b})"),
            format!("{lead} − {}", b * b),
        ),
    }
}

/// Recognising the expanded form to close it again; the x² coefficient
/// stays one.
fn factorisation(rng: &mut Hasard) -> Card {
    let b = rng.entier(2, 12);
    let square = sup(2);

    match rng.entier(1, 3) {
        1 => card(
            format!("x{square} + {}x + {}", 2 * b, b * b),
            format!("(x + {b}){square}"),
        ),
        2 => card(
            format!("x{square} − {}x + {}", 2 * b, b * b),
            format!("(x − {b}){square}"),
        ),
        _ => card(
            format!("x{square} − {}", b * b),
            format!("(x + {b})(x − {b})"),
        ),
    }
}
