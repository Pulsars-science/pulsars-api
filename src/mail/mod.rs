//! Account mails: the address verification link and the password reset link
//! (`docs/contract/auth-sessions.md`).
//!
//! Sent off the request path: a request queues its mail on a task and
//! answers at once; a failure (SMTP down, refused address) is logged, never
//! returned. Without `MAIL_SMTP_URL`, a mail is only logged, without its
//! link, so a link token never reaches the logs.

pub mod templates;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use lettre::message::{Mailbox, MultiPart};
use lettre::{AsyncSmtpTransport, AsyncTransport as _, Message, Tokio1Executor};

pub use templates::{Content, Lang};

use crate::config::MailConfig;

/// How long one mail may take to be handed to the SMTP server.
const SEND_TIMEOUT: Duration = Duration::from_secs(20);

/// What a mail is, for logs and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Verification,
    Reset,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Verification => "verification",
            Self::Reset => "reset",
        }
    }
}

/// One mail to one member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    pub kind: Kind,
    pub user_id: i64,
    pub to: String,
    pub content: Content,
}

#[derive(Clone)]
pub struct Mailer(Arc<Inner>);

struct Smtp {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

enum Inner {
    Smtp(Box<Smtp>),
    /// No SMTP server configured.
    Log,
    /// Tests: mails are kept, in order.
    Record(Mutex<Vec<Mail>>),
}

impl Mailer {
    /// The mailer of `MAIL_SMTP_URL`, or one that only logs.
    pub fn from_config(config: Option<&MailConfig>) -> Result<Self, String> {
        let Some(config) = config else {
            return Ok(Self::log_only());
        };
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(config.smtp_url.expose())
            .map_err(|e| format!("MAIL_SMTP_URL: {e}"))?
            .timeout(Some(SEND_TIMEOUT))
            .build();
        let from = config.from.parse().map_err(|e| format!("MAIL_FROM: {e}"))?;
        Ok(Self(Arc::new(Inner::Smtp(Box::new(Smtp {
            transport,
            from,
        })))))
    }

    pub fn log_only() -> Self {
        Self(Arc::new(Inner::Log))
    }

    /// A mailer that keeps what it is asked to send (tests).
    pub fn recording() -> Self {
        Self(Arc::new(Inner::Record(Mutex::new(Vec::new()))))
    }

    /// The mails kept by a [`Self::recording`] mailer.
    pub fn sent(&self) -> Vec<Mail> {
        match &*self.0 {
            Inner::Record(mails) => mails.lock().map(|m| m.clone()).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// Queues `mail` and returns at once.
    pub fn send(&self, mail: Mail) {
        match &*self.0 {
            Inner::Record(mails) => {
                if let Ok(mut mails) = mails.lock() {
                    mails.push(mail);
                }
            }
            Inner::Log => tracing::info!(
                kind = mail.kind.name(),
                user_id = mail.user_id,
                "account mail not sent: MAIL_SMTP_URL is not set"
            ),
            Inner::Smtp(smtp) => {
                let Smtp { transport, from } = smtp.as_ref();
                let message = match message(from, &mail) {
                    Ok(message) => message,
                    Err(error) => {
                        tracing::warn!(kind = mail.kind.name(), user_id = mail.user_id, %error, "account mail not built");
                        return;
                    }
                };
                let transport = transport.clone();
                tokio::spawn(async move {
                    match tokio::time::timeout(SEND_TIMEOUT, transport.send(message)).await {
                        Ok(Ok(_)) => tracing::info!(
                            kind = mail.kind.name(),
                            user_id = mail.user_id,
                            "account mail sent"
                        ),
                        Ok(Err(error)) => tracing::warn!(
                            kind = mail.kind.name(),
                            user_id = mail.user_id,
                            %error,
                            "account mail failed"
                        ),
                        Err(_) => tracing::warn!(
                            kind = mail.kind.name(),
                            user_id = mail.user_id,
                            "account mail timed out"
                        ),
                    }
                });
            }
        }
    }
}

/// The MIME message of `mail`: plain text and HTML alternatives.
pub fn message(from: &Mailbox, mail: &Mail) -> Result<Message, String> {
    let to: Mailbox = mail.to.parse().map_err(|e| format!("recipient: {e}"))?;
    Message::builder()
        .from(from.clone())
        .to(to)
        .subject(mail.content.subject.clone())
        .multipart(MultiPart::alternative_plain_html(
            mail.content.text.clone(),
            mail.content.html.clone(),
        ))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mail_is_text_and_html_from_pulsars() {
        let from: Mailbox = crate::config::MAIL_FROM.parse().unwrap();
        let mail = Mail {
            kind: Kind::Verification,
            user_id: 42,
            to: "ada@example.invalid".to_owned(),
            content: templates::verification(Lang::Fr, "https://pulsars.science/email/verifier/t"),
        };
        let raw = String::from_utf8(message(&from, &mail).unwrap().formatted()).unwrap();
        assert!(
            raw.contains("From: Pulsars <noreply@blackenium.com>"),
            "{raw}"
        );
        assert!(raw.contains("To: ada@example.invalid"));
        // Non-ASCII subjects are encoded (RFC 2047).
        assert!(raw.contains("Subject: =?utf-8?"), "{raw}");
        assert!(raw.contains("multipart/alternative"));
        assert!(raw.contains("text/plain"));
        assert!(raw.contains("text/html"));

        let bad = Mail {
            to: "not an address".to_owned(),
            ..mail
        };
        assert!(message(&from, &bad).is_err());
    }
}
