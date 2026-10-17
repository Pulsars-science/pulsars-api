//! The flash-duel game server (`docs/game-protocol.md`).
//!
//! ```text
//! WebSocket ──▶ session ──commands──▶ hub ──▶ match runner (one task per match)
//!    ▲             │                   │            │
//!    └── outbox ◀──┴───────────────────┴────────────┘
//! ```
//!
//! - [`session`]: one task per WebSocket: authentication, frame rate limit,
//!   parsing, database lookups (ratings, ghosts), and writing the frames
//!   queued in its [`Outbox`].
//! - [`hub`]: one task for the process: sessions, matchmaking queues,
//!   private rooms, and which player is in which match. It never awaits I/O.
//! - [`runner`]: one task per match around the pure [`engine::Match`],
//!   which persists the result, the ratings and the webhook outbox row in
//!   one transaction at the end.
//! - [`webhook`]: the outbox delivery worker (`docs/contract/webhooks.md`).
//! - [`season`]: the monthly season close (`docs/contract/brains.md` §3):
//!   standings, `season.ended` event and rating decay.
//!
//! Everything lives in this process: one API instance serves the game
//! (`game-protocol.md` §6).

pub mod engine;
pub mod glicko;
pub mod hub;
pub mod matchmaking;
pub mod protocol;
pub mod ranking;
pub mod rules;
pub mod runner;
pub mod season;
pub mod session;
pub mod store;
pub mod webhook;

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::{Notify, mpsc};

use crate::flash::Lang;
use engine::EngineSettings;
use protocol::{PlayerInfo, ServerMsg};

/// Who a player is for the length of their session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerId {
    /// Laravel `users.id`.
    Account(i64),
    /// A guest, numbered by this process.
    Guest(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub lang: Lang,
}

impl Player {
    pub fn is_guest(&self) -> bool {
        matches!(self.id, PlayerId::Guest(_))
    }

    pub fn user_id(&self) -> Option<i64> {
        match self.id {
            PlayerId::Account(id) => Some(id),
            PlayerId::Guest(_) => None,
        }
    }

    pub fn info(&self) -> PlayerInfo {
        PlayerInfo {
            user_id: self.user_id().map(|id| id.to_string()),
            name: self.name.clone(),
            guest: self.is_guest(),
            locale: self.lang.code(),
        }
    }
}

/// Timings and limits of the game server. [`GameSettings::default`] is the
/// protocol; tests shorten the timings.
#[derive(Debug, Clone)]
pub struct GameSettings {
    /// Between `match.start` and the first question.
    pub countdown: Duration,
    /// The first frame must be `auth` within this delay.
    pub auth_timeout: Duration,
    pub engine: EngineSettings,
    /// Questions drawn per match (the flash "défi" cap).
    pub question_count: usize,
    /// Frames a client may send per second.
    pub max_frames_per_second: u32,
    /// WebSocket pings, well under nginx's 300 s read timeout.
    pub ping_interval: Duration,
    /// A connection silent for this long (no frame, no pong) is closed.
    pub idle_timeout: Duration,
    /// How often the hub pairs waiting players.
    pub queue_tick: Duration,
    /// `queue.waiting` period.
    pub waiting_every: Duration,
    /// Wait after which `queue.waiting` offers a ghost.
    pub ghost_offer_after: Duration,
    /// Players in a private room.
    pub room_max: usize,
    /// Frames queued for a client before it is closed as too slow.
    pub outbox_capacity: usize,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            countdown: Duration::from_secs(3),
            auth_timeout: Duration::from_secs(5),
            engine: EngineSettings::default(),
            question_count: 240,
            max_frames_per_second: 20,
            ping_interval: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(90),
            queue_tick: Duration::from_secs(1),
            waiting_every: Duration::from_secs(5),
            ghost_offer_after: Duration::from_secs(20),
            room_max: 8,
            outbox_capacity: 256,
        }
    }
}

/// Wall-clock milliseconds derived from tokio's clock, so that a paused
/// tokio clock (`tokio::time::pause`) drives timers and timestamps alike.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    origin: tokio::time::Instant,
    origin_ms: i64,
}

impl Clock {
    pub fn new() -> Self {
        Self {
            origin: tokio::time::Instant::now(),
            origin_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Unix milliseconds.
    pub fn now_ms(&self) -> i64 {
        let elapsed = i64::try_from(self.origin.elapsed().as_millis()).unwrap_or(i64::MAX / 2);
        self.origin_ms + elapsed
    }

    /// The tokio instant of Unix time `ms` (now, if `ms` is in the past).
    pub fn instant_at(&self, ms: i64) -> tokio::time::Instant {
        let offset = u64::try_from(ms - self.origin_ms).unwrap_or(0);
        self.origin + Duration::from_millis(offset)
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

/// A frame for a session to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outgoing {
    Text(Arc<str>),
    Close(u16, &'static str),
}

/// The write side of a session, held by the hub and the match runners.
/// Never blocks: a client that does not read is closed.
#[derive(Debug, Clone)]
pub struct Outbox {
    tx: mpsc::Sender<Outgoing>,
    overflow: Arc<Notify>,
}

impl Outbox {
    /// The outbox, the receiver the session writes from, and the signal
    /// raised when the queue overflows.
    pub fn new(capacity: usize) -> (Self, mpsc::Receiver<Outgoing>, Arc<Notify>) {
        let (tx, rx) = mpsc::channel(capacity.max(1));
        let overflow = Arc::new(Notify::new());
        (
            Self {
                tx,
                overflow: overflow.clone(),
            },
            rx,
            overflow,
        )
    }

    pub fn send(&self, msg: &ServerMsg) {
        self.push(Outgoing::Text(msg.to_text().into()));
    }

    pub fn send_text(&self, text: Arc<str>) {
        self.push(Outgoing::Text(text));
    }

    pub fn close(&self, code: u16, reason: &'static str) {
        self.push(Outgoing::Close(code, reason));
    }

    fn push(&self, frame: Outgoing) {
        if let Err(mpsc::error::TrySendError::Full(_)) = self.tx.try_send(frame) {
            self.overflow.notify_one();
        }
    }
}

/// Handle on the game server, shared by every WebSocket. Cheap to clone.
#[derive(Clone)]
pub struct Game {
    hub: mpsc::Sender<hub::HubCmd>,
    pub settings: Arc<GameSettings>,
    pub clock: Clock,
}

impl Game {
    /// Starts the hub task.
    pub fn start(db: PgPool, settings: GameSettings) -> Self {
        let settings = Arc::new(settings);
        let clock = Clock::new();
        let hub = hub::spawn(db, settings.clone(), clock);
        Self {
            hub,
            settings,
            clock,
        }
    }

    pub(crate) fn hub(&self) -> &mpsc::Sender<hub::HubCmd> {
        &self.hub
    }

    /// The seed of a live match, so tests can compute the right answers.
    #[cfg(any(test, feature = "test-util"))]
    pub async fn match_seed(&self, match_id: uuid::Uuid) -> Option<u32> {
        let (reply, rx) = tokio::sync::oneshot::channel();
        self.hub
            .send(hub::HubCmd::Seed { match_id, reply })
            .await
            .ok()?;
        rx.await.ok().flatten()
    }
}

/// A random integer in `0..n` (`n > 0`), from the OS RNG through UUIDv4 (no
/// direct dependency on `rand`). Not for secrets.
pub(crate) fn random_below(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    let value = uuid::Uuid::new_v4().as_u128() as u64;
    value % n
}

/// 32 random bytes, base64url without padding: guest resume tokens.
pub(crate) fn random_token() -> String {
    use base64::Engine as _;
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
