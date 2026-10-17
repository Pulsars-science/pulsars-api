use super::{card, meta, pick, ucfirst};
use crate::flash::i18n::{t, tr};
use crate::flash::{Card, Generator, Hasard, Lang};

/// What the formula computes, then the formula, under its isolated forms.
const FORMULAS: [(&str, &str); 53] = [
    ("la vitesse moyenne", "v = d / t"),
    ("la distance parcourue", "d = v × t"),
    ("la durée d’un parcours", "t = d / v"),
    ("l’accélération moyenne", "a = Δv / Δt"),
    ("le poids d’un corps", "P = m × g"),
    ("la masse volumique", "ρ = m / V"),
    ("la masse, connaissant la masse volumique", "m = ρ × V"),
    ("le volume, connaissant la masse volumique", "V = m / ρ"),
    ("l’énergie cinétique", "Ec = ½ × m × v²"),
    ("l’énergie potentielle de pesanteur", "Epp = m × g × h"),
    ("l’énergie mécanique", "Em = Ec + Epp"),
    ("l’énergie potentielle élastique", "Epe = ½ × k × x²"),
    ("la force de rappel d’un ressort", "F = k × x"),
    (
        "la force, connaissant la masse et l’accélération",
        "F = m × a",
    ),
    ("la quantité de mouvement", "p = m × v"),
    ("le travail d’une force constante", "W = F × d"),
    ("la puissance mécanique moyenne", "P = W / t"),
    ("la puissance électrique", "P = U × I"),
    ("la puissance dissipée par effet Joule", "P = R × I²"),
    ("la tension aux bornes d’un conducteur ohmique", "U = R × I"),
    (
        "l’intensité, connaissant la tension et la résistance",
        "I = U / R",
    ),
    (
        "la résistance, connaissant la tension et l’intensité",
        "R = U / I",
    ),
    ("la résistance d’un fil conducteur", "R = ρ × L / S"),
    (
        "la tension aux bornes d’un générateur réel",
        "U = E − r × I",
    ),
    ("l’énergie électrique consommée", "E = P × t"),
    ("la charge électrique transportée", "Q = I × t"),
    ("la période, connaissant la fréquence", "T = 1 / f"),
    ("la fréquence, connaissant la période", "f = 1 / T"),
    ("la période d’un pendule simple", "T = 2π√(l / g)"),
    ("la vitesse angulaire", "ω = 2π / T"),
    ("la longueur d’onde", "λ = v / f"),
    ("la fréquence d’une onde", "f = v / λ"),
    ("l’énergie d’un photon", "E = h × ν"),
    ("l’énergie de masse", "E = m × c²"),
    ("la pression", "p = F / S"),
    ("la force pressante", "F = p × S"),
    ("la pression au fond d’un liquide", "p = ρ × g × h"),
    ("la poussée d’Archimède", "F = ρ × V × g"),
    ("l’énergie thermique échangée", "Q = m × c × ΔT"),
    ("l’état d’un gaz parfait", "P × V = n × R × T"),
    ("la quantité de matière, connaissant la masse", "n = m / M"),
    ("la masse, connaissant la quantité de matière", "m = n × M"),
    ("la concentration en masse", "Cm = m / V"),
    ("la concentration en quantité de matière", "c = n / V"),
    (
        "la quantité de matière, connaissant la concentration",
        "n = c × V",
    ),
    ("le nombre d’entités d’un échantillon", "N = n × NA"),
    (
        "la force d’interaction gravitationnelle",
        "F = G × mA × mB / d²",
    ),
    (
        "la force d’interaction électrostatique",
        "F = k × |qA × qB| / d²",
    ),
    ("l’indice de réfraction d’un milieu", "n = c / v"),
    (
        "la réfraction à la traversée d’un dioptre",
        "n₁ × sin(i₁) = n₂ × sin(i₂)",
    ),
    ("la vergence d’une lentille", "C = 1 / f′"),
    ("le grandissement d’une lentille", "γ = A′B′ / AB"),
    ("le débit volumique", "Qv = V / t"),
];

pub struct FormulesPhysique;

impl Generator for FormulesPhysique {
    meta! {
        key: "formules-physique",
        subject: Physics,
        level: HighSchool,
        title: "Formules de physique-chimie",
        description: "Les formules du programme, dans les deux sens : retrouver la formule d’une grandeur, et dire ce que calcule une formule.",
    }

    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card {
        let (object, formula) = pick(rng, &FORMULAS);

        if rng.entier(1, 2) == 1 {
            return card(
                t(
                    lang,
                    "Quelle formule donne :objet ?",
                    &[("objet", tr(lang, object))],
                ),
                formula,
            );
        }

        // Backwards: read a formula and say what it gives.
        card(
            t(lang, "Que calcule :formule ?", &[("formule", formula)]),
            ucfirst(tr(lang, object)),
        )
    }
}
