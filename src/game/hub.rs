//! The hub: one task that owns the sessions, the matchmaking queues, the
//! private rooms and the map of who plays which match. It never awaits I/O:
//! sessions do the database lookups before sending a command, and match
//! runners persist their own results.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::engine::{MatchSetup, SeatKind, SeatSetup};
use super::matchmaking::{Queue, Waiting};
use super::protocol::{AnswerFrame, ErrorCode, MatchFrame, RoomPlayer, ServerMsg, close};
use super::rules::{Mode, QueueKey};
use super::runner::{MatchCmd, Runner};
use super::store::GhostRun;
use super::{Clock, GameSettings, Outbox, Player, PlayerId, random_below, random_token};
use crate::flash::Lang;
use crate::telemetry::names;

pub type SessionId = u64;

/// How a session authenticated.
#[derive(Debug, Clone)]
pub enum Login {
    Account {
        user_id: i64,
        name: String,
        lang: Lang,
    },
    Guest {
        resume: Option<String>,
        lang: Lang,
    },
}

/// The reply to [`HubCmd::Connect`].
#[derive(Debug, Clone)]
pub struct Connected {
    pub session: SessionId,
    pub player: Arc<Player>,
}

/// A player's request, already parsed and checked by the session.
#[derive(Debug)]
pub enum Command {
    QueueJoin {
        key: QueueKey,
        /// Accounts: rating for the subject (1500 if none).
        rating: f64,
        /// A ghost exists for the key, to offer after a while.
        ghost_available: bool,
    },
    QueueLeave,
    GhostStart {
        key: QueueKey,
        ghost: GhostRun,
    },
    RoomCreate {
        key: QueueKey,
    },
    RoomJoin {
        code: String,
    },
    RoomLeave,
    RoomStart,
    Answer(AnswerFrame),
    MatchLeave(MatchFrame),
}

/// What a match runner reports.
#[derive(Debug)]
pub enum MatchEvent {
    /// The seat forfeited: its player is free again.
    Released {
        match_id: Uuid,
        seat: usize,
    },
    Ended {
        match_id: Uuid,
    },
}

#[derive(Debug)]
pub enum HubCmd {
    Connect {
        login: Login,
        outbox: Outbox,
        reply: oneshot::Sender<Connected>,
    },
    Disconnect {
        session: SessionId,
    },
    Client {
        session: SessionId,
        command: Command,
    },
    Match(MatchEvent),
    /// What is live right now, for the admin statistics.
    Live {
        reply: oneshot::Sender<LiveSnapshot>,
    },
    #[cfg(any(test, feature = "test-util"))]
    Seed {
        match_id: Uuid,
        reply: oneshot::Sender<Option<u32>>,
    },
}

struct Session {
    player: Arc<Player>,
    outbox: Outbox,
}

struct Queued {
    key: QueueKey,
    ranked: bool,
    joined_at: i64,
    ghost_available: bool,
    /// `queue.waiting` messages sent so far.
    reported: i64,
}

struct Room {
    key: QueueKey,
    /// In seat order; the first one is the host.
    members: Vec<Arc<Player>>,
    playing: Option<Uuid>,
}

struct LiveMatch {
    commands: mpsc::UnboundedSender<MatchCmd>,
    /// Read only by the tests (`Game::match_seed`, feature `test-util`).
    #[cfg_attr(not(feature = "test-util"), allow(dead_code))]
    seed: u32,
    mode: Mode,
    /// Unix ms of the first question (`game.matches.started_at`).
    starts_at: i64,
    room: Option<String>,
    /// Live players, by seat (`None` for ghosts).
    seats: Vec<Option<PlayerId>>,
}

/// The hub's live state, counted on demand (`GET /v1/game/admin/stats`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LiveSnapshot {
    /// Open authenticated sessions.
    pub sessions: usize,
    /// Of which guests.
    pub guests: usize,
    /// Players waiting in matchmaking, every queue key.
    pub queued: usize,
    /// Private rooms in their lobby (not started, or between matches).
    pub rooms_open: usize,
    /// Players still seated in a live match (forfeiters are released).
    pub players_in_matches: usize,
    pub matches: Vec<LiveMatchInfo>,
}

/// A match running now. Its seats are written to `game.match_players`
/// only when it ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveMatchInfo {
    pub id: Uuid,
    pub mode: Mode,
    /// Unix ms (`game.matches.started_at`).
    pub starts_at: i64,
    /// Account seats, forfeiters included.
    pub accounts: Vec<i64>,
    /// Guest seats, forfeiters included.
    pub guests: usize,
}

/// Room codes: no 0/O, 1/I/L.
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const CODE_LEN: usize = 6;

pub fn spawn(db: PgPool, settings: Arc<GameSettings>, clock: Clock) -> mpsc::Sender<HubCmd> {
    let (tx, rx) = mpsc::channel(4096);
    let hub = Hub {
        db,
        settings,
        clock,
        me: tx.clone(),
        next_session: 1,
        next_guest: 1,
        sessions: HashMap::new(),
        online: HashMap::new(),
        resume: HashMap::new(),
        resume_of: HashMap::new(),
        queues: HashMap::new(),
        queued: HashMap::new(),
        rooms: HashMap::new(),
        room_of: HashMap::new(),
        matches: HashMap::new(),
        playing: HashMap::new(),
    };
    tokio::spawn(hub.run(rx));
    tx
}

struct Hub {
    db: PgPool,
    settings: Arc<GameSettings>,
    clock: Clock,
    /// Given to match runners to report back.
    me: mpsc::Sender<HubCmd>,
    next_session: SessionId,
    next_guest: u64,
    sessions: HashMap<SessionId, Session>,
    online: HashMap<PlayerId, SessionId>,
    /// Guest resume token → guest.
    resume: HashMap<String, Arc<Player>>,
    resume_of: HashMap<PlayerId, String>,
    queues: HashMap<(QueueKey, bool), Queue<PlayerId>>,
    queued: HashMap<PlayerId, Queued>,
    rooms: HashMap<String, Room>,
    room_of: HashMap<PlayerId, String>,
    matches: HashMap<Uuid, LiveMatch>,
    /// Match and seat of every player in a live match.
    playing: HashMap<PlayerId, (Uuid, usize)>,
}

impl Hub {
    async fn run(mut self, mut rx: mpsc::Receiver<HubCmd>) {
        let mut tick = tokio::time::interval(self.settings.queue_tick);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                command = rx.recv() => match command {
                    Some(command) => self.handle(command),
                    None => break,
                },
                _ = tick.tick() => self.on_tick(),
            }
        }
    }

    fn handle(&mut self, command: HubCmd) {
        match command {
            HubCmd::Connect {
                login,
                outbox,
                reply,
            } => self.connect(login, outbox, reply),
            HubCmd::Disconnect { session } => self.disconnect(session),
            HubCmd::Client { session, command } => {
                if let Some(s) = self.sessions.get(&session) {
                    let player = s.player.clone();
                    let outbox = s.outbox.clone();
                    self.client(&player, &outbox, command);
                }
            }
            HubCmd::Match(MatchEvent::Released { match_id, seat }) => {
                let player = self
                    .matches
                    .get(&match_id)
                    .and_then(|m| m.seats.get(seat).copied().flatten());
                if let Some(player) = player {
                    self.release(player, match_id);
                }
            }
            HubCmd::Match(MatchEvent::Ended { match_id }) => self.ended(match_id),
            HubCmd::Live { reply } => {
                let _ = reply.send(self.snapshot());
            }
            #[cfg(any(test, feature = "test-util"))]
            HubCmd::Seed { match_id, reply } => {
                let _ = reply.send(self.matches.get(&match_id).map(|m| m.seed));
            }
        }
    }

    /// Counts what is live: one pass over the sessions and the matches,
    /// no I/O.
    fn snapshot(&self) -> LiveSnapshot {
        LiveSnapshot {
            sessions: self.sessions.len(),
            guests: self
                .sessions
                .values()
                .filter(|s| s.player.is_guest())
                .count(),
            queued: self.queued.len(),
            rooms_open: self.rooms.values().filter(|r| r.playing.is_none()).count(),
            players_in_matches: self.playing.len(),
            matches: self
                .matches
                .iter()
                .map(|(id, m)| LiveMatchInfo {
                    id: *id,
                    mode: m.mode,
                    starts_at: m.starts_at,
                    accounts: m
                        .seats
                        .iter()
                        .filter_map(|p| match p {
                            Some(PlayerId::Account(id)) => Some(*id),
                            _ => None,
                        })
                        .collect(),
                    guests: m
                        .seats
                        .iter()
                        .filter(|p| matches!(p, Some(PlayerId::Guest(_))))
                        .count(),
                })
                .collect(),
        }
    }

    fn connect(&mut self, login: Login, outbox: Outbox, reply: oneshot::Sender<Connected>) {
        let (player, resume) = match login {
            Login::Account {
                user_id,
                name,
                lang,
            } => (
                Arc::new(Player {
                    id: PlayerId::Account(user_id),
                    name,
                    lang,
                }),
                None,
            ),
            Login::Guest { resume, lang } => {
                let known = resume
                    .as_deref()
                    .and_then(|token| self.resume.get(token).map(|p| (token, p.clone())))
                    .filter(|(_, p)| self.playing.contains_key(&p.id));
                match known {
                    Some((token, player)) => (player, Some(token.to_owned())),
                    None => {
                        let player = Arc::new(self.new_guest(lang));
                        let token = random_token();
                        self.resume.insert(token.clone(), player.clone());
                        self.resume_of.insert(player.id, token.clone());
                        (player, Some(token))
                    }
                }
            }
        };

        let session = self.next_session;
        self.next_session += 1;
        if let Some(old) = self.online.insert(player.id, session)
            && let Some(old) = self.sessions.remove(&old)
        {
            old.outbox
                .close(close::REPLACED, "another session opened for this player");
        }
        outbox.send(&ServerMsg::AuthOk {
            player: player.info(),
            resume,
        });
        self.sessions.insert(
            session,
            Session {
                player: player.clone(),
                outbox: outbox.clone(),
            },
        );
        metrics::gauge!(names::GAME_SESSIONS).set(self.sessions.len() as f64);
        let _ = reply.send(Connected {
            session,
            player: player.clone(),
        });

        // A session that replaced another one picks up where it was.
        if let Some(q) = self.queued.get(&player.id) {
            outbox.send(&ServerMsg::QueueJoined {
                subject: q.key.subject,
                rules: q.key.rules,
                ranked: q.ranked,
            });
        }
        if let Some(code) = self.room_of.get(&player.id).cloned() {
            self.broadcast_room(&code);
        }
        if let Some((match_id, seat)) = self.playing.get(&player.id).copied()
            && let Some(m) = self.matches.get(&match_id)
        {
            let _ = m.commands.send(MatchCmd::Reconnect { seat, outbox });
        }
    }

    fn new_guest(&mut self, lang: Lang) -> Player {
        let number = self.next_guest;
        self.next_guest += 1;
        let digits = 1000 + random_below(9000);
        let name = match lang {
            Lang::Fr => format!("Invité {digits}"),
            Lang::En => format!("Guest {digits}"),
            Lang::Ru => format!("Гость {digits}"),
        };
        Player {
            id: PlayerId::Guest(number),
            name,
            lang,
        }
    }

    fn disconnect(&mut self, session: SessionId) {
        let Some(s) = self.sessions.remove(&session) else {
            // Already replaced by a newer session.
            return;
        };
        metrics::gauge!(names::GAME_SESSIONS).set(self.sessions.len() as f64);
        let player = s.player;
        if self.online.get(&player.id) == Some(&session) {
            self.online.remove(&player.id);
        }
        self.leave_queue(player.id);
        if let Some(code) = self.room_of.get(&player.id).cloned()
            && self.rooms.get(&code).is_some_and(|r| r.playing.is_none())
        {
            self.leave_room(player.id, false);
        }
        if let Some((match_id, seat)) = self.playing.get(&player.id).copied()
            && let Some(m) = self.matches.get(&match_id)
        {
            let _ = m.commands.send(MatchCmd::Disconnected { seat });
        }
        self.forget_guest(player.id);
    }

    /// A guest's resume token lives while they are connected or playing.
    fn forget_guest(&mut self, player: PlayerId) {
        if matches!(player, PlayerId::Guest(_))
            && !self.online.contains_key(&player)
            && !self.playing.contains_key(&player)
            && let Some(token) = self.resume_of.remove(&player)
        {
            self.resume.remove(&token);
        }
    }

    fn client(&mut self, player: &Arc<Player>, outbox: &Outbox, command: Command) {
        let error = |code, message: &str| outbox.send(&ServerMsg::error(code, message));
        match command {
            Command::QueueJoin {
                key,
                rating,
                ghost_available,
            } => {
                if self.playing.contains_key(&player.id) || self.room_of.contains_key(&player.id) {
                    return error(ErrorCode::AlreadyPlaying, "Leave your match or room first.");
                }
                let ranked = !player.is_guest();
                if let Some(q) = self.queued.get(&player.id)
                    && q.key == key
                {
                    // Already waiting there: keep the place.
                    return outbox.send(&ServerMsg::QueueJoined {
                        subject: key.subject,
                        rules: key.rules,
                        ranked,
                    });
                }
                self.leave_queue(player.id);
                let now = self.clock.now_ms();
                self.queues.entry((key, ranked)).or_default().join(Waiting {
                    player: player.id,
                    // Guests are paired by arrival order.
                    rating: if ranked { rating } else { 1500.0 },
                    joined_at: now,
                });
                self.queued.insert(
                    player.id,
                    Queued {
                        key,
                        ranked,
                        joined_at: now,
                        ghost_available,
                        reported: 0,
                    },
                );
                outbox.send(&ServerMsg::QueueJoined {
                    subject: key.subject,
                    rules: key.rules,
                    ranked,
                });
                self.pair((key, ranked), now);
                self.queue_gauge();
            }
            Command::QueueLeave => {
                self.leave_queue(player.id);
                outbox.send(&ServerMsg::QueueLeft);
            }
            Command::GhostStart { key, ghost } => {
                if self.playing.contains_key(&player.id) || self.room_of.contains_key(&player.id) {
                    return error(ErrorCode::AlreadyPlaying, "Leave your match or room first.");
                }
                if self.leave_queue(player.id) {
                    outbox.send(&ServerMsg::QueueLeft);
                }
                let seed = ghost.seed;
                let seats = vec![
                    live_seat(player, None),
                    SeatSetup {
                        name: ghost.name,
                        user_id: None,
                        rating: None,
                        kind: SeatKind::Ghost {
                            log: ghost.log,
                            source_match: ghost.source_match,
                        },
                    },
                ];
                self.create_match(Mode::Ghost, false, key, seed, seats, None);
            }
            Command::RoomCreate { key } => {
                if self.playing.contains_key(&player.id) || self.room_of.contains_key(&player.id) {
                    return error(ErrorCode::AlreadyPlaying, "Leave your match or room first.");
                }
                if self.leave_queue(player.id) {
                    outbox.send(&ServerMsg::QueueLeft);
                }
                let code = self.new_code();
                self.rooms.insert(
                    code.clone(),
                    Room {
                        key,
                        members: vec![player.clone()],
                        playing: None,
                    },
                );
                self.room_of.insert(player.id, code.clone());
                self.broadcast_room(&code);
            }
            Command::RoomJoin { code } => {
                let code = code.trim().to_ascii_uppercase();
                if self.playing.contains_key(&player.id) {
                    return error(ErrorCode::AlreadyPlaying, "Leave your match first.");
                }
                if let Some(current) = self.room_of.get(&player.id) {
                    if *current == code {
                        return self.broadcast_room(&code);
                    }
                    return error(ErrorCode::AlreadyPlaying, "Leave your room first.");
                }
                let max = self.settings.room_max;
                let Some(room) = self.rooms.get_mut(&code) else {
                    return error(ErrorCode::RoomNotFound, "No room has this code.");
                };
                if room.playing.is_some() {
                    return error(ErrorCode::RoomStarted, "This room is playing.");
                }
                if room.members.len() >= max {
                    return error(ErrorCode::RoomFull, "This room is full.");
                }
                room.members.push(player.clone());
                self.room_of.insert(player.id, code.clone());
                if self.leave_queue(player.id) {
                    outbox.send(&ServerMsg::QueueLeft);
                }
                self.broadcast_room(&code);
            }
            Command::RoomLeave => {
                if self.room_of.contains_key(&player.id) {
                    self.leave_room(player.id, true);
                }
                outbox.send(&ServerMsg::RoomClosed { reason: "left" });
            }
            Command::RoomStart => {
                let Some(code) = self.room_of.get(&player.id).cloned() else {
                    return error(ErrorCode::NotAllowed, "You are not in a room.");
                };
                let Some(room) = self.rooms.get(&code) else {
                    return error(ErrorCode::NotAllowed, "You are not in a room.");
                };
                if room.members.first().map(|p| p.id) != Some(player.id) {
                    return error(ErrorCode::NotAllowed, "Only the host starts the match.");
                }
                if room.playing.is_some() {
                    return error(ErrorCode::RoomStarted, "This room is playing.");
                }
                if room.members.len() < 2 {
                    return error(ErrorCode::NotAllowed, "A match needs at least 2 players.");
                }
                let key = room.key;
                let seats = room.members.iter().map(|p| live_seat(p, None)).collect();
                let seed = new_seed();
                let id = self.create_match(Mode::Room, false, key, seed, seats, Some(code.clone()));
                if let Some(room) = self.rooms.get_mut(&code) {
                    room.playing = Some(id);
                }
            }
            Command::Answer(frame) => match self.playing.get(&player.id).copied() {
                Some((match_id, seat)) => {
                    if let Some(m) = self.matches.get(&match_id) {
                        let _ = m.commands.send(MatchCmd::Answer { seat, frame });
                    }
                }
                None => error(ErrorCode::NotAllowed, "You are not in a match."),
            },
            Command::MatchLeave(frame) => match self.playing.get(&player.id).copied() {
                Some((match_id, seat)) if match_id.to_string() == frame.match_id => {
                    if let Some(m) = self.matches.get(&match_id) {
                        let _ = m.commands.send(MatchCmd::Leave { seat });
                    }
                    self.release(player.id, match_id);
                    if self.room_of.contains_key(&player.id) {
                        self.leave_room(player.id, true);
                    }
                }
                _ => error(ErrorCode::NotAllowed, "You are not in this match."),
            },
        }
    }

    /// Removes the player from any queue. True if they were waiting.
    fn leave_queue(&mut self, player: PlayerId) -> bool {
        let Some(q) = self.queued.remove(&player) else {
            return false;
        };
        if let Some(queue) = self.queues.get_mut(&(q.key, q.ranked)) {
            queue.leave(&player);
            if queue.is_empty() {
                self.queues.remove(&(q.key, q.ranked));
            }
        }
        self.queue_gauge();
        true
    }

    fn queue_gauge(&self) {
        metrics::gauge!(names::GAME_QUEUED).set(self.queued.len() as f64);
    }

    fn pair(&mut self, queue: (QueueKey, bool), now: i64) {
        let pairs = match self.queues.get_mut(&queue) {
            Some(q) => q.pair(now),
            None => return,
        };
        let (key, ranked) = queue;
        for (a, b) in pairs {
            self.queued.remove(&a.player);
            self.queued.remove(&b.player);
            let players = [a, b].map(|w| {
                let player = self
                    .online
                    .get(&w.player)
                    .and_then(|s| self.sessions.get(s))
                    .map(|s| s.player.clone());
                (player, w.rating)
            });
            let [(Some(pa), ra), (Some(pb), rb)] = players else {
                // A player without a session left the queue on disconnect;
                // nothing to pair.
                continue;
            };
            let display = |rating: f64| ranked.then(|| rating.round() as i32);
            let seats = vec![live_seat(&pa, display(ra)), live_seat(&pb, display(rb))];
            self.create_match(Mode::Duel, ranked, key, new_seed(), seats, None);
        }
        if self.queues.get(&queue).is_some_and(Queue::is_empty) {
            self.queues.remove(&queue);
        }
        self.queue_gauge();
    }

    fn on_tick(&mut self) {
        let now = self.clock.now_ms();
        let keys: Vec<(QueueKey, bool)> = self.queues.keys().copied().collect();
        for key in keys {
            self.pair(key, now);
        }
        let every = i64::try_from(self.settings.waiting_every.as_millis()).unwrap_or(5_000);
        let ghost_after = i64::try_from(self.settings.ghost_offer_after.as_millis()).unwrap_or(0);
        for (player, q) in &mut self.queued {
            let waited = now - q.joined_at;
            if every > 0 && waited / every > q.reported {
                q.reported = waited / every;
                let msg = ServerMsg::QueueWaiting {
                    seconds: u64::try_from(waited / 1000).unwrap_or(0),
                    ghost_available: q.ghost_available && waited >= ghost_after,
                };
                if let Some(s) = self.online.get(player).and_then(|s| self.sessions.get(s)) {
                    s.outbox.send(&msg);
                }
            }
        }
    }

    fn new_code(&self) -> String {
        loop {
            let code: String = (0..CODE_LEN)
                .map(|_| {
                    let i = usize::try_from(random_below(CODE_ALPHABET.len() as u64)).unwrap_or(0);
                    char::from(CODE_ALPHABET.get(i).copied().unwrap_or(b'A'))
                })
                .collect();
            if !self.rooms.contains_key(&code) {
                return code;
            }
        }
    }

    fn broadcast_room(&self, code: &str) {
        let Some(room) = self.rooms.get(code) else {
            return;
        };
        let msg = ServerMsg::RoomState {
            code: code.to_owned(),
            host_seat: 0,
            players: room
                .members
                .iter()
                .enumerate()
                .map(|(seat, p)| RoomPlayer {
                    seat,
                    name: p.name.clone(),
                    user_id: p.user_id().map(|id| id.to_string()),
                    guest: p.is_guest(),
                })
                .collect(),
            subject: room.key.subject,
            rules: room.key.rules,
        };
        let text: Arc<str> = msg.to_text().into();
        for member in &room.members {
            if let Some(s) = self
                .online
                .get(&member.id)
                .and_then(|s| self.sessions.get(s))
            {
                s.outbox.send_text(text.clone());
            }
        }
    }

    /// The player leaves their room; the next one becomes host, an empty
    /// room closes. A player leaving a room whose match is running forfeits
    /// it when `forfeit` is set.
    fn leave_room(&mut self, player: PlayerId, forfeit: bool) {
        let Some(code) = self.room_of.remove(&player) else {
            return;
        };
        let Some(room) = self.rooms.get_mut(&code) else {
            return;
        };
        room.members.retain(|p| p.id != player);
        let empty = room.members.is_empty();
        let playing = room.playing;
        if forfeit
            && let Some(match_id) = playing
            && let Some((m, seat)) = self.playing.get(&player).copied()
            && m == match_id
        {
            if let Some(live) = self.matches.get(&match_id) {
                let _ = live.commands.send(MatchCmd::Leave { seat });
            }
            self.release(player, match_id);
        }
        if empty && playing.is_none() {
            self.rooms.remove(&code);
        } else {
            self.broadcast_room(&code);
        }
    }

    /// The player no longer plays `match_id`.
    fn release(&mut self, player: PlayerId, match_id: Uuid) {
        if self.playing.get(&player).map(|(m, _)| *m) == Some(match_id) {
            self.playing.remove(&player);
        }
        self.forget_guest(player);
    }

    fn ended(&mut self, match_id: Uuid) {
        let Some(m) = self.matches.remove(&match_id) else {
            return;
        };
        for player in m.seats.iter().flatten() {
            self.release(*player, match_id);
        }
        if let Some(code) = m.room
            && let Some(room) = self.rooms.get_mut(&code)
        {
            room.playing = None;
            // Back to the lobby with whoever is still connected.
            let online = &self.online;
            let gone: Vec<PlayerId> = room
                .members
                .iter()
                .filter(|p| !online.contains_key(&p.id))
                .map(|p| p.id)
                .collect();
            room.members.retain(|p| online.contains_key(&p.id));
            for player in gone {
                self.room_of.remove(&player);
            }
            if self.rooms.get(&code).is_some_and(|r| r.members.is_empty()) {
                self.rooms.remove(&code);
            } else {
                self.broadcast_room(&code);
            }
        }
    }

    fn create_match(
        &mut self,
        mode: Mode,
        ranked: bool,
        key: QueueKey,
        seed: u32,
        seats: Vec<SeatSetup>,
        room: Option<String>,
    ) -> Uuid {
        let id = Uuid::now_v7();
        let now = self.clock.now_ms();
        let countdown = i64::try_from(self.settings.countdown.as_millis()).unwrap_or(3_000);
        let starts_at = now + countdown;
        let mut outboxes = Vec::with_capacity(seats.len());
        let mut players = Vec::with_capacity(seats.len());
        for (seat, setup) in seats.iter().enumerate() {
            match &setup.kind {
                SeatKind::Live(player) => {
                    self.playing.insert(player.id, (id, seat));
                    outboxes.push(
                        self.online
                            .get(&player.id)
                            .and_then(|s| self.sessions.get(s))
                            .map(|s| s.outbox.clone()),
                    );
                    players.push(Some(player.id));
                }
                SeatKind::Ghost { .. } => {
                    outboxes.push(None);
                    players.push(None);
                }
            }
        }
        let (commands, rx) = mpsc::unbounded_channel();
        let runner = Runner {
            setup: MatchSetup {
                id,
                mode,
                ranked,
                key,
                seed,
                created_at: now,
                starts_at,
                seats,
            },
            outboxes,
            commands: rx,
            hub: self.me.clone(),
            db: self.db.clone(),
            clock: self.clock,
            settings: self.settings.clone(),
        };
        tokio::spawn(runner.run());
        self.matches.insert(
            id,
            LiveMatch {
                commands,
                seed,
                mode,
                starts_at,
                room,
                seats: players,
            },
        );
        id
    }
}

fn live_seat(player: &Arc<Player>, rating: Option<i32>) -> SeatSetup {
    SeatSetup {
        name: player.name.clone(),
        user_id: player.user_id(),
        rating,
        kind: SeatKind::Live(player.clone()),
    }
}

/// A match seed, 1..=999999.
fn new_seed() -> u32 {
    u32::try_from(1 + random_below(999_999)).unwrap_or(1)
}
