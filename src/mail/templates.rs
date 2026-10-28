//! The two account mails, in the words of Laravel's stock `VerifyEmail` and
//! `ResetPassword` notifications as the site translates them
//! (`lang/{fr,en,ru}.json` of the site): same subject, same lines, same
//! button, same « trouble clicking » footer.

/// A mail's language: `fr`, `en` or `ru`. Anything else reads French.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Fr,
    En,
    Ru,
}

impl Lang {
    /// The first of `candidates` that is a supported language, else French.
    pub fn pick<'a>(candidates: impl IntoIterator<Item = Option<&'a str>>) -> Self {
        candidates
            .into_iter()
            .flatten()
            .find_map(|code| match code.get(..2) {
                Some("fr") => Some(Self::Fr),
                Some("en") => Some(Self::En),
                Some("ru") => Some(Self::Ru),
                _ => None,
            })
            .unwrap_or(Self::Fr)
    }
}

/// Subject, plain text and HTML of a mail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    pub subject: String,
    pub text: String,
    pub html: String,
}

struct Words {
    subject: &'static str,
    intro: &'static str,
    action: &'static str,
    outro: String,
}

struct Common {
    hello: &'static str,
    regards: &'static str,
    /// With `:actionText` in it.
    trouble: &'static str,
}

fn common(lang: Lang) -> Common {
    match lang {
        Lang::Fr => Common {
            hello: "Bonjour !",
            regards: "Cordialement,",
            trouble: "Si le bouton « :actionText » ne fonctionne pas, copie le lien ci-dessous\net colle-le dans ton navigateur :",
        },
        Lang::En => Common {
            hello: "Hello!",
            regards: "Regards,",
            trouble: "If you're having trouble clicking the \":actionText\" button, copy and paste the URL below\ninto your web browser:",
        },
        Lang::Ru => Common {
            hello: "Привет!",
            regards: "С уважением,",
            trouble: "Если кнопка «:actionText» не срабатывает, скопируй ссылку ниже и вставь её в адресную строку браузера:",
        },
    }
}

/// The mail with the link that verifies an address.
pub fn verification(lang: Lang, url: &str) -> Content {
    let words = match lang {
        Lang::Fr => Words {
            subject: "Vérifie ton adresse e-mail",
            intro: "Clique sur le bouton ci-dessous pour vérifier ton adresse e-mail.",
            action: "Vérifier mon adresse e-mail",
            outro: "Si tu n’as pas créé de compte, tu n’as rien à faire.".to_owned(),
        },
        Lang::En => Words {
            subject: "Verify your email address",
            intro: "Please click the button below to verify your email address.",
            action: "Verify Email Address",
            outro: "If you did not create an account, no further action is required.".to_owned(),
        },
        Lang::Ru => Words {
            subject: "Подтверди свой e-mail",
            intro: "Нажми кнопку ниже, чтобы подтвердить свой e-mail.",
            action: "Подтвердить e-mail",
            outro: "Если ты не создавал аккаунт, просто проигнорируй это письмо.".to_owned(),
        },
    };
    render(lang, &words, &[], url)
}

/// The mail with the link that resets a password, valid `minutes`.
pub fn reset(lang: Lang, url: &str, minutes: i64) -> Content {
    let (expiry, words) = match lang {
        Lang::Fr => (
            "Ce lien de réinitialisation expire dans :count minutes.",
            Words {
                subject: "Réinitialise ton mot de passe",
                intro: "Tu reçois cet e-mail parce qu’une réinitialisation du mot de passe a été demandée pour ton compte.",
                action: "Réinitialiser le mot de passe",
                outro: "Si tu n’as pas demandé de réinitialisation, tu n’as rien à faire."
                    .to_owned(),
            },
        ),
        Lang::En => (
            "This password reset link will expire in :count minutes.",
            Words {
                subject: "Reset your password",
                intro: "You are receiving this email because we received a password reset request for your account.",
                action: "Reset Password",
                outro: "If you did not request a password reset, no further action is required."
                    .to_owned(),
            },
        ),
        Lang::Ru => (
            "Ссылка для сброса пароля действительна :count мин.",
            Words {
                subject: "Сброс пароля",
                intro: "Ты получил это письмо, потому что для твоего аккаунта был запрошен сброс пароля.",
                action: "Сбросить пароль",
                outro: "Если ты не запрашивал сброс пароля, просто проигнорируй это письмо."
                    .to_owned(),
            },
        ),
    };
    let expiry = expiry.replace(":count", &minutes.to_string());
    render(lang, &words, &[expiry], url)
}

/// Laravel's mail layout: greeting, intro, button, more lines, outro,
/// signature, then the raw link for mail clients that hide the button.
fn render(lang: Lang, words: &Words, after_action: &[String], url: &str) -> Content {
    let common = common(lang);
    let trouble = common.trouble.replace(":actionText", words.action);

    let mut text = format!(
        "{}\n\n{}\n\n{}: {url}\n\n",
        common.hello, words.intro, words.action
    );
    for line in after_action {
        text.push_str(line);
        text.push_str("\n\n");
    }
    text.push_str(&format!(
        "{}\n\n{}\nPulsars\n\n{}\n{url}\n",
        words.outro, common.regards, trouble
    ));

    let url_html = escape(url);
    let mut html = format!(
        "<!doctype html>\n<html lang=\"{lang}\"><body style=\"font-family:sans-serif;color:#1f2937;max-width:560px;margin:0 auto;padding:24px\">\n\
         <h1 style=\"font-size:20px\">{hello}</h1>\n<p>{intro}</p>\n\
         <p style=\"text-align:center;margin:28px 0\"><a href=\"{url_html}\" style=\"background:#4f7df0;color:#fff;padding:10px 18px;border-radius:6px;text-decoration:none\">{action}</a></p>\n",
        lang = match lang {
            Lang::Fr => "fr",
            Lang::En => "en",
            Lang::Ru => "ru",
        },
        hello = escape(common.hello),
        intro = escape(words.intro),
        action = escape(words.action),
    );
    for line in after_action {
        html.push_str(&format!("<p>{}</p>\n", escape(line)));
    }
    html.push_str(&format!(
        "<p>{}</p>\n<p>{}<br>Pulsars</p>\n<hr style=\"border:none;border-top:1px solid #e5e7eb\">\n\
         <p style=\"font-size:12px;color:#6b7280\">{}<br><a href=\"{url_html}\">{url_html}</a></p>\n</body></html>\n",
        escape(&words.outro),
        escape(common.regards),
        escape(&trouble).replace('\n', "<br>"),
    ));

    Content {
        subject: words.subject.to_owned(),
        text,
        html,
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_language_is_the_first_supported_one() {
        assert_eq!(Lang::pick([None, Some("ru")]), Lang::Ru);
        assert_eq!(Lang::pick([Some("en-GB"), Some("ru")]), Lang::En);
        assert_eq!(Lang::pick([Some("de"), None]), Lang::Fr);
        assert_eq!(Lang::pick([]), Lang::Fr);
    }

    #[test]
    fn mails_carry_laravels_words_and_the_link_twice() {
        let url = "https://pulsars.science/email/verifier/abc&def";
        let mail = verification(Lang::Fr, url);
        assert_eq!(mail.subject, "Vérifie ton adresse e-mail");
        assert!(mail.text.starts_with("Bonjour !\n\nClique sur le bouton"));
        assert_eq!(mail.text.matches(url).count(), 2);
        assert!(mail.html.contains("abc&amp;def"));
        assert!(!mail.html.contains("abc&def"));

        let mail = reset(Lang::En, "https://x/y", 60);
        assert_eq!(mail.subject, "Reset your password");
        assert!(
            mail.text
                .contains("This password reset link will expire in 60 minutes.")
        );
        assert!(mail.text.contains("clicking the \"Reset Password\" button"));
        let mail = reset(Lang::Ru, "https://x/y", 60);
        assert!(mail.text.contains("действительна 60 мин."));
    }
}
