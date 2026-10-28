//! One WebSocket connection (`game-protocol.md` §3).
//!
//! 1. The first frame must be `auth`, within 5 seconds: a game token
//!    (verified as in `auth-jwt.md` §5, its `jti` made single-use in Redis)
//!    or a guest.
//! 2. The hub registers the session and answers `auth.ok` (then
//!    `match.start` with `resumed: true` when a match is in progress).
//! 3. Frames are parsed and turned into hub commands; ratings and ghosts are
//!    read from the database here, so the hub never waits on I/O.
//!
//! The session writes what the hub and the match runners queue in its
//! [`Outbox`], pings the client every 30 seconds and closes a client that
//! sends more than 20 frames a second (`4029`) or stops reading (`1008`).

use std::collections::VecDeque;
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket};
use fred::prelude::KeysInterface as _;
use fred::types::{Expiration, SetOptions};
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::time::Instant;

use super::hub::{Command, Connected, HubCmd, Login};
use super::protocol::{self, ClientMsg, ErrorCode, ServerMsg, close};
use super::rules::QueueKey;
use super::{Game, Outbox, Outgoing, Player, glicko, store};
use crate::auth::{self, Expected, Failure};
use crate::flash::Lang;
use crate::state::AppState;
use crate::telemetry::names;

/// Prefix of the single-use `jti` keys in Redis.
const JTI_PREFIX: &str = "pulsars-api:game:jti";

/// How long the session waits for Redis to record a `jti`.
const REDIS_TIMEOUT: Duration = Duration::from_secs(2);

/// Why a session closes.
struct Closing {
    code: u16,
    reason: String,
}

impl Closing {
    fn new(code: u16, reason: impl Into<String>) -> Self {
        Self {
            code,
            reason: reason.into(),
        }
    }
}

/// Counts frames over the last second.
struct FrameLimit {
    max: usize,
    recent: VecDeque<Instant>,
}

impl FrameLimit {
    fn new(max: u32) -> Self {
        Self {
            max: usize::try_from(max).unwrap_or(20),
            recent: VecDeque::new(),
        }
    }

    /// False once more than `max` frames arrived within one second.
    fn allow(&mut self, now: Instant) -> bool {
        while self
            .recent
            .front()
            .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(1))
        {
            self.recent.pop_front();
        }
        self.recent.push_back(now);
        self.recent.len() <= self.max
    }
}

pub async fn run(mut socket: WebSocket, state: AppState) {
    let game = state.game.clone();
    let mut limit = FrameLimit::new(game.settings.max_frames_per_second);
    metrics::counter!(names::GAME_CONNECTIONS_TOTAL).increment(1);

    let login = match authenticate(&mut socket, &state, &mut limit).await {
        Ok(login) => login,
        Err(closing) => {
            metrics::counter!(names::GAME_AUTH_FAILURES_TOTAL, "code" => closing.code.to_string())
                .increment(1);
            close_socket(&mut socket, closing).await;
            return;
        }
    };

    let (outbox, mut frames, overflow) = Outbox::new(game.settings.outbox_capacity);
    let (reply, connected) = oneshot::channel();
    let registered = game
        .hub()
        .send(HubCmd::Connect {
            login,
            outbox,
            reply,
        })
        .await;
    let Ok(Connected { session, player }) = (match registered {
        Ok(()) => connected.await.map_err(|_| ()),
        Err(_) => Err(()),
    }) else {
        close_socket(
            &mut socket,
            Closing::new(close::GOING_AWAY, "game server unavailable"),
        )
        .await;
        return;
    };

    let closing = serve(
        &mut socket,
        &state,
        &game,
        session,
        &player,
        &mut frames,
        &overflow,
        &mut limit,
    )
    .await;
    let _ = game.hub().send(HubCmd::Disconnect { session }).await;
    match closing {
        Some(closing) => close_socket(&mut socket, closing).await,
        // The client closed: reading once more flushes the close reply
        // that the WebSocket layer queued, so the client sees a clean close.
        None => {
            let _ = tokio::time::timeout(Duration::from_secs(1), socket.recv()).await;
        }
    }
}

/// The main loop. Returns how to close, or `None` when the client is gone.
#[allow(clippy::too_many_arguments)]
async fn serve(
    socket: &mut WebSocket,
    state: &AppState,
    game: &Game,
    session: u64,
    player: &Player,
    frames: &mut mpsc::Receiver<Outgoing>,
    overflow: &Notify,
    limit: &mut FrameLimit,
) -> Option<Closing> {
    let mut ping = tokio::time::interval(game.settings.ping_interval);
    ping.reset();
    let mut last_seen = Instant::now();
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let message = match incoming {
                    Some(Ok(message)) => message,
                    // Closed or broken: nothing more to say.
                    Some(Err(_)) | None => return None,
                };
                last_seen = Instant::now();
                let text = match message {
                    Message::Text(text) => text,
                    Message::Binary(_) => {
                        if !limit.allow(Instant::now()) {
                            return Some(Closing::new(close::RATE_LIMITED, "too many frames"));
                        }
                        send(socket, &ServerMsg::error(ErrorCode::BadRequest, "Frames are JSON text.")).await?;
                        continue;
                    }
                    Message::Close(_) => return None,
                    Message::Ping(_) | Message::Pong(_) => continue,
                };
                if !limit.allow(Instant::now()) {
                    return Some(Closing::new(close::RATE_LIMITED, "too many frames"));
                }
                if let Some(reply) = handle(state, game, session, player, &text).await {
                    send(socket, &reply).await?;
                }
            }
            frame = frames.recv() => match frame {
                Some(Outgoing::Text(text)) => {
                    socket.send(Message::Text(Utf8Bytes::from(text.as_ref()))).await.ok()?;
                }
                Some(Outgoing::Close(code, reason)) => return Some(Closing::new(code, reason)),
                None => return Some(Closing::new(close::GOING_AWAY, "server stopping")),
            },
            () = overflow.notified() => {
                return Some(Closing::new(close::TOO_SLOW, "client too slow"));
            }
            _ = ping.tick() => {
                if last_seen.elapsed() >= game.settings.idle_timeout {
                    return Some(Closing::new(close::GOING_AWAY, "idle"));
                }
                socket.send(Message::Ping(Default::default())).await.ok()?;
            }
        }
    }
}

async fn send(socket: &mut WebSocket, msg: &ServerMsg) -> Option<()> {
    socket
        .send(Message::Text(Utf8Bytes::from(msg.to_text())))
        .await
        .ok()
}

/// Sends the close frame, then reads until the client's close frame (at
/// most 1 second). Dropping the connection with unread frames would reset
/// it, and the client could lose the close frame and its code.
async fn close_socket(socket: &mut WebSocket, closing: Closing) {
    let reason: String = closing.reason.chars().take(100).collect();
    let sent = socket
        .send(Message::Close(Some(CloseFrame {
            code: closing.code,
            reason: Utf8Bytes::from(reason),
        })))
        .await;
    if sent.is_err() {
        return;
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), async {
        while let Some(Ok(message)) = socket.recv().await {
            if matches!(message, Message::Close(_)) {
                break;
            }
        }
    })
    .await;
}

/// Waits for the `auth` frame and checks it.
async fn authenticate(
    socket: &mut WebSocket,
    state: &AppState,
    limit: &mut FrameLimit,
) -> Result<Login, Closing> {
    let deadline = Instant::now() + state.game.settings.auth_timeout;
    let text = loop {
        let message = match tokio::time::timeout_at(deadline, socket.recv()).await {
            Err(_) => return Err(Closing::new(close::NO_AUTH, "no auth frame in time")),
            Ok(None | Some(Err(_))) => return Err(Closing::new(close::NO_AUTH, "closed")),
            Ok(Some(Ok(message))) => message,
        };
        if !limit.allow(Instant::now()) {
            return Err(Closing::new(close::RATE_LIMITED, "too many frames"));
        }
        match message {
            Message::Text(text) => break text,
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Binary(_) | Message::Close(_) => {
                return Err(Closing::new(close::NO_AUTH, "the first frame must be auth"));
            }
        }
    };
    let frame = match protocol::parse(&text) {
        Ok(ClientMsg::Auth(frame)) => frame,
        _ => return Err(Closing::new(close::NO_AUTH, "the first frame must be auth")),
    };

    if let Some(token) = frame.token {
        return account(state, &token).await;
    }
    if frame.guest {
        return Ok(Login::Guest {
            resume: frame.resume.filter(|r| r.len() <= 64),
            lang: Lang::from_locale(frame.locale.as_deref()),
        });
    }
    Err(Closing::new(close::INVALID_TOKEN, "missing"))
}

async fn account(state: &AppState, token: &str) -> Result<Login, Closing> {
    let header = format!("Bearer {token}");
    let expected = Expected {
        issuer: &state.config.jwt_issuer,
        audience: &state.config.jwt_audience,
    };
    let now = chrono::Utc::now().timestamp();
    let claims = match auth::authenticate(
        Some(header.as_bytes()),
        &state.jwks,
        expected,
        state.config.token_signer.as_ref(),
        now,
    )
    .await
    {
        Ok(claims) => claims,
        Err(Failure::Rejected { reason, .. }) => {
            metrics::counter!(names::AUTH_REJECTIONS_TOTAL, "reason" => reason.code()).increment(1);
            tracing::info!(reason = reason.code(), "game token rejected");
            return Err(Closing::new(close::INVALID_TOKEN, reason.code()));
        }
        Err(Failure::NoKeys) => {
            return Err(Closing::new(close::TRY_AGAIN_LATER, "no usable JWKS key"));
        }
    };
    let Some(user_id) = auth::user_id(&claims.sub) else {
        return Err(Closing::new(close::INVALID_TOKEN, "malformed"));
    };

    // Single use: the `jti` is recorded until the token expires. Without
    // Redis the token cannot be proven unused: refused.
    let ttl = (claims.exp - now + auth::token::LEEWAY_SECS).max(1);
    let key = format!("{JTI_PREFIX}:{}", claims.jti);
    let recorded = tokio::time::timeout(
        REDIS_TIMEOUT,
        state.redis.set::<Option<String>, _, _>(
            key,
            "1",
            Some(Expiration::EX(ttl)),
            Some(SetOptions::NX),
            false,
        ),
    )
    .await;
    match recorded {
        Ok(Ok(Some(_))) => {}
        Ok(Ok(None)) => return Err(Closing::new(close::JTI_REUSED, "token already used")),
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "cannot record the token jti in Redis");
            return Err(Closing::new(close::TRY_AGAIN_LATER, "try again later"));
        }
        Err(_) => {
            tracing::warn!("Redis did not answer in time to record the token jti");
            return Err(Closing::new(close::TRY_AGAIN_LATER, "try again later"));
        }
    }

    let name = claims
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("#{user_id}"));
    Ok(Login::Account {
        user_id,
        name: name.chars().take(64).collect(),
        lang: Lang::from_locale(claims.locale.as_deref()),
    })
}

/// Handles one text frame. Returns a reply to send directly, if any.
async fn handle(
    state: &AppState,
    game: &Game,
    session: u64,
    player: &Player,
    text: &str,
) -> Option<ServerMsg> {
    let message = match protocol::parse(text) {
        Ok(message) => message,
        Err(error) => return Some(ServerMsg::error(ErrorCode::BadRequest, error.message())),
    };
    let command = match message {
        ClientMsg::Ping => {
            return Some(ServerMsg::Pong {
                server_time: game.clock.now_ms(),
            });
        }
        ClientMsg::Auth(_) => {
            return Some(ServerMsg::error(
                ErrorCode::BadRequest,
                "Already authenticated.",
            ));
        }
        ClientMsg::QueueJoin(frame) => {
            let key = QueueKey {
                subject: frame.subject,
                rules: frame.rules,
            };
            let rating = my_rating(state, player, key).await;
            let ghost_available = match store::ghost_exists(&state.db, key, player.user_id()).await
            {
                Ok(exists) => exists,
                Err(error) => {
                    tracing::warn!(error = %error, "cannot look for ghosts");
                    false
                }
            };
            Command::QueueJoin {
                key,
                rating: rating.rating,
                ghost_available,
            }
        }
        ClientMsg::QueueLeave => Command::QueueLeave,
        ClientMsg::GhostStart(frame) => {
            let key = QueueKey {
                subject: frame.subject,
                rules: frame.rules,
            };
            let rating = my_rating(state, player, key).await;
            match store::pick_ghost(&state.db, key, player.user_id(), rating.rating).await {
                Ok(Some(ghost)) => Command::GhostStart { key, ghost },
                Ok(None) => {
                    return Some(ServerMsg::error(
                        ErrorCode::NoGhost,
                        "No recorded run for this subject and these rules.",
                    ));
                }
                Err(error) => {
                    tracing::warn!(error = %error, "cannot pick a ghost");
                    return Some(ServerMsg::error(
                        ErrorCode::NoGhost,
                        "Recorded runs are unavailable right now.",
                    ));
                }
            }
        }
        ClientMsg::RoomCreate(frame) => Command::RoomCreate {
            key: QueueKey {
                subject: frame.subject,
                rules: frame.rules,
            },
        },
        ClientMsg::RoomJoin(frame) => {
            if frame.code.len() > 16 {
                return Some(ServerMsg::error(
                    ErrorCode::RoomNotFound,
                    "No room has this code.",
                ));
            }
            Command::RoomJoin { code: frame.code }
        }
        ClientMsg::RoomLeave => Command::RoomLeave,
        ClientMsg::RoomStart => Command::RoomStart,
        ClientMsg::Answer(frame) => Command::Answer(frame),
        ClientMsg::MatchLeave(frame) => Command::MatchLeave(frame),
    };
    let _ = game.hub().send(HubCmd::Client { session, command }).await;
    None
}

/// The player's rating for the key's subject; the default for guests, new
/// players, or when the database cannot answer.
async fn my_rating(state: &AppState, player: &Player, key: QueueKey) -> glicko::Rating {
    let Some(user_id) = player.user_id() else {
        return glicko::DEFAULT;
    };
    match store::rating(&state.db, user_id, key.subject).await {
        Ok(rating) => rating.unwrap_or(glicko::DEFAULT),
        Err(error) => {
            tracing::warn!(error = %error, "cannot read a rating, using the default");
            glicko::DEFAULT
        }
    }
}

/// Shared by the WebSocket route: is this `Origin` allowed?
pub fn origin_allowed(
    origin: Option<&axum::http::HeaderValue>,
    allowed: &[axum::http::HeaderValue],
) -> bool {
    origin.is_some_and(|o| allowed.iter().any(|a| a == o))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_limited_per_second() {
        let mut limit = FrameLimit::new(20);
        let start = Instant::now();
        for _ in 0..20 {
            assert!(limit.allow(start));
        }
        assert!(!limit.allow(start + Duration::from_millis(999)));
        let mut limit = FrameLimit::new(20);
        for i in 0..40 {
            assert!(limit.allow(start + Duration::from_millis(60 * i)));
        }
    }

    #[test]
    fn origins_must_match_exactly() {
        let allowed = vec![axum::http::HeaderValue::from_static(
            "https://game.pulsars.science",
        )];
        let ok = axum::http::HeaderValue::from_static("https://game.pulsars.science");
        let bad = axum::http::HeaderValue::from_static("https://evil.example");
        assert!(origin_allowed(Some(&ok), &allowed));
        assert!(!origin_allowed(Some(&bad), &allowed));
        assert!(!origin_allowed(None, &allowed));
    }
}
