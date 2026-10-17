//! One task per match: feeds the pure [`Match`] with the players' commands
//! and wakes it at its deadlines, delivers its messages, then persists the
//! result and tells the hub.

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::mpsc;

use super::engine::{Final, Match, MatchSetup, Out};
use super::hub::{HubCmd, MatchEvent};
use super::protocol::{AnswerFrame, MatchEnd, SeatResult, ServerMsg};
use super::store::{self, RatingChange};
use super::{Clock, GameSettings, Outbox};
use crate::flash;
use crate::telemetry::names;

/// What the hub sends a match.
#[derive(Debug)]
pub enum MatchCmd {
    Answer { seat: usize, frame: AnswerFrame },
    Leave { seat: usize },
    Disconnected { seat: usize },
    Reconnect { seat: usize, outbox: Outbox },
}

pub struct Runner {
    pub setup: MatchSetup,
    /// One per seat; `None` for ghosts and disconnected players.
    pub outboxes: Vec<Option<Outbox>>,
    pub commands: mpsc::UnboundedReceiver<MatchCmd>,
    pub hub: mpsc::Sender<HubCmd>,
    pub db: PgPool,
    pub clock: Clock,
    pub settings: Arc<GameSettings>,
}

/// Seats that left the match keep their outbox here, for `match.end` only.
struct Delivery {
    active: Vec<Option<Outbox>>,
    released: Vec<Option<Outbox>>,
}

impl Delivery {
    fn send(&self, out: Vec<Out>) -> Vec<usize> {
        let mut released = Vec::new();
        for item in out {
            match item {
                Out::To(seat, msg) => {
                    if let Some(Some(outbox)) = self.active.get(seat) {
                        outbox.send(&msg);
                    }
                }
                Out::Others(except, msg) => {
                    let text: Arc<str> = msg.to_text().into();
                    for (seat, outbox) in self.active.iter().enumerate() {
                        if seat != except
                            && let Some(outbox) = outbox
                        {
                            outbox.send_text(text.clone());
                        }
                    }
                }
                Out::All(msg) => {
                    let text: Arc<str> = msg.to_text().into();
                    for outbox in self.active.iter().flatten() {
                        outbox.send_text(text.clone());
                    }
                }
                Out::Released(seat) => released.push(seat),
            }
        }
        released
    }
}

impl Runner {
    pub async fn run(self) {
        let Runner {
            setup,
            outboxes,
            mut commands,
            hub,
            db,
            clock,
            settings,
        } = self;
        let id = setup.id;
        let questions =
            flash::duel_questions(setup.key.subject.0, setup.seed, settings.question_count);
        let mut engine = Match::new(setup, questions, settings.engine);
        let mut delivery = Delivery {
            released: vec![None; outboxes.len()],
            active: outboxes,
        };
        metrics::counter!(names::GAME_MATCHES_STARTED_TOTAL, "mode" => engine.mode.key())
            .increment(1);
        metrics::gauge!(names::GAME_MATCHES_LIVE).increment(1.0);

        let now = clock.now_ms();
        for (seat, outbox) in delivery.active.iter().enumerate() {
            if let Some(outbox) = outbox {
                outbox.send(&engine.start_message(seat, false, now));
            }
        }
        {
            let db = db.clone();
            let row = store::StartRow::of(&engine);
            tokio::spawn(async move {
                if let Err(error) = store::insert_start(&db, &row).await {
                    tracing::warn!(match_id = %row.id, error = %error, "cannot record the match start");
                }
            });
        }

        while !engine.is_finished() {
            let deadline = engine.next_deadline().map(|ms| clock.instant_at(ms));
            let out = tokio::select! {
                command = commands.recv() => {
                    let now = clock.now_ms();
                    match command {
                        Some(MatchCmd::Answer { seat, frame }) => {
                            match engine.answer(seat, &frame.match_id, frame.index, frame.choice, now) {
                                Ok(out) => out,
                                Err((code, message)) => {
                                    let mut out = engine.tick(now);
                                    out.push(Out::To(seat, ServerMsg::error(code, message)));
                                    out
                                }
                            }
                        }
                        Some(MatchCmd::Leave { seat }) => engine.leave(seat, now),
                        Some(MatchCmd::Disconnected { seat }) => {
                            if let Some(slot) = delivery.active.get_mut(seat) {
                                *slot = None;
                            }
                            engine.disconnect(seat, now)
                        }
                        Some(MatchCmd::Reconnect { seat, outbox }) => {
                            if let Some(slot) = delivery.active.get_mut(seat) {
                                *slot = Some(outbox);
                            }
                            engine.reconnect(seat, now)
                        }
                        // The hub is gone: the process is stopping.
                        None => {
                            engine.abort(now);
                            Vec::new()
                        }
                    }
                }
                () = sleep_until(deadline) => engine.tick(clock.now_ms()),
            };
            for seat in delivery.send(out) {
                if let Some(slot) = delivery.active.get_mut(seat) {
                    delivery.released[seat] = slot.take();
                }
                let _ = hub
                    .send(HubCmd::Match(MatchEvent::Released { match_id: id, seat }))
                    .await;
            }
        }

        let Some(result) = engine.result() else {
            return;
        };
        let changes = persist(&db, &engine, &result).await;
        let end = end_message(&engine, &result, &changes);
        let text: Arc<str> = end.to_text().into();
        for outbox in delivery
            .active
            .iter()
            .flatten()
            .chain(delivery.released.iter().flatten())
        {
            outbox.send_text(text.clone());
        }
        metrics::gauge!(names::GAME_MATCHES_LIVE).decrement(1.0);
        metrics::counter!(names::GAME_MATCHES_ENDED_TOTAL,
            "mode" => engine.mode.key(), "end_reason" => result.reason.key())
        .increment(1);
        tracing::info!(
            match_id = %id,
            mode = engine.mode.key(),
            end_reason = result.reason.key(),
            ranked = result.ranked,
            "match ended"
        );
        let _ = hub
            .send(HubCmd::Match(MatchEvent::Ended { match_id: id }))
            .await;
    }
}

async fn sleep_until(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

/// Writes the end of the match, retrying a few times. On failure the match
/// still ends for the players, without rating change.
async fn persist(db: &PgPool, engine: &Match, result: &Final) -> Vec<Option<RatingChange>> {
    let mut delay = Duration::from_millis(500);
    for attempt in 1..=3 {
        match store::finish(db, engine, result).await {
            Ok(changes) => return changes,
            Err(error) if attempt < 3 => {
                tracing::warn!(match_id = %engine.id, attempt, error = %error,
                    "cannot record the match end, retrying");
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            Err(error) => {
                metrics::counter!(names::GAME_PERSIST_ERRORS_TOTAL).increment(1);
                tracing::error!(match_id = %engine.id, error = %error,
                    "cannot record the match end: result and ratings are lost");
            }
        }
    }
    vec![None; engine.seats().len()]
}

pub fn end_message(engine: &Match, result: &Final, changes: &[Option<RatingChange>]) -> ServerMsg {
    ServerMsg::MatchEnd(MatchEnd {
        match_id: engine.id.to_string(),
        end_reason: result.reason,
        ranked: result.ranked,
        results: engine
            .seats()
            .iter()
            .enumerate()
            .map(|(seat, s)| {
                let (rank, outcome) = result
                    .places
                    .get(seat)
                    .copied()
                    .unwrap_or((1, super::ranking::Outcome::Draw));
                let change = changes.get(seat).copied().flatten();
                SeatResult {
                    seat,
                    name: s.name.clone(),
                    user_id: s.user_id.map(|id| id.to_string()),
                    score: s.score,
                    correct: s.correct,
                    answered: s.answered,
                    rank,
                    outcome,
                    rating_before: change.map(|c| c.before),
                    rating_after: change.map(|c| c.after),
                }
            })
            .collect(),
    })
}
