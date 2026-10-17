//! The state machine of one match (`game-protocol.md` §4 and §5).
//!
//! Pure: no I/O, no clock. Every method takes the current time in Unix
//! milliseconds and returns the messages to send, so tests drive a whole
//! match through any timeline without sleeping. [`super::runner`] wraps it
//! in a task that feeds it commands and wakes it at [`Match::next_deadline`].
//!
//! ```text
//! created ──(starts_at, countdown)──▶ running ──▶ finished
//! ```

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Player;
use super::protocol::{EndReason, ErrorCode, MatchPlayer, MatchStart, SeatScore, ServerMsg};
use super::ranking::{self, Outcome, Standing};
use super::rules::{Mode, QueueKey};
use crate::flash::{DuelQuestion, Lang};

/// One answer of a run, as stored in `game.match_players.answers` and
/// replayed by ghosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerLog {
    /// Question index.
    pub i: usize,
    /// Choice, 0-3.
    pub c: u8,
    pub ok: bool,
    /// Milliseconds since `starts_at`.
    pub t: i64,
}

/// Who sits in a seat.
#[derive(Debug, Clone)]
pub enum SeatKind {
    Live(Arc<Player>),
    /// A recorded run, replayed at its recorded times.
    Ghost {
        log: Vec<AnswerLog>,
        /// Match whose run is replayed.
        source_match: Uuid,
    },
}

/// A seat as the match is created.
#[derive(Debug, Clone)]
pub struct SeatSetup {
    pub kind: SeatKind,
    pub name: String,
    /// Account id: live accounts only (null for guests and ghosts).
    pub user_id: Option<i64>,
    /// Rating shown in `match.start`, ranked matches only.
    pub rating: Option<i32>,
}

/// Timings and thresholds of a match.
#[derive(Debug, Clone, Copy)]
pub struct EngineSettings {
    /// Reconnection grace after a disconnection.
    pub grace_ms: i64,
    /// A correct answer faster than this is suspicious.
    pub suspicious_ms: i64,
    /// More suspicious answers than this from one player make the match
    /// unranked.
    pub suspicious_max: u32,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            grace_ms: 20_000,
            suspicious_ms: 300,
            suspicious_max: 3,
        }
    }
}

pub struct MatchSetup {
    pub id: Uuid,
    pub mode: Mode,
    pub ranked: bool,
    pub key: QueueKey,
    pub seed: u32,
    pub created_at: i64,
    pub starts_at: i64,
    pub seats: Vec<SeatSetup>,
}

/// A message to deliver.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    /// To one seat.
    To(usize, ServerMsg),
    /// To every seat but one.
    Others(usize, ServerMsg),
    /// To every seat.
    All(ServerMsg),
    /// The seat left the match (forfeit): its player is free again.
    Released(usize),
}

#[derive(Debug, Clone)]
pub struct Seat {
    pub kind: SeatKind,
    pub name: String,
    pub user_id: Option<i64>,
    pub rating: Option<i32>,
    pub score: i32,
    pub correct: u32,
    pub answered: u32,
    /// Index of the question the seat is on.
    pub index: usize,
    /// When the current question was sent.
    pub sent_at: i64,
    /// Total time over correct answers.
    pub answer_time_ms: i64,
    pub suspicious: u32,
    pub answers: Vec<AnswerLog>,
    pub connected: bool,
    pub grace_until: Option<i64>,
    pub forfeited: bool,
    /// Ghosts: next log entry to replay.
    next_replay: usize,
}

impl Seat {
    pub fn is_ghost(&self) -> bool {
        matches!(self.kind, SeatKind::Ghost { .. })
    }

    pub fn player(&self) -> Option<&Arc<Player>> {
        match &self.kind {
            SeatKind::Live(player) => Some(player),
            SeatKind::Ghost { .. } => None,
        }
    }

    fn lang(&self) -> Lang {
        self.player().map_or(Lang::Fr, |p| p.lang)
    }

    fn ghost_done(&self) -> bool {
        match &self.kind {
            SeatKind::Ghost { log, .. } => self.next_replay >= log.len(),
            SeatKind::Live(_) => true,
        }
    }
}

/// How a match ended.
#[derive(Debug, Clone, PartialEq)]
pub struct Final {
    pub reason: EndReason,
    pub ranked: bool,
    pub ended_at: i64,
    /// Rank and outcome per seat, in seat order.
    pub places: Vec<(u16, Outcome)>,
}

pub struct Match {
    pub id: Uuid,
    pub mode: Mode,
    /// Ranked at creation; the final value is in [`Final::ranked`].
    pub ranked: bool,
    pub key: QueueKey,
    pub seed: u32,
    pub created_at: i64,
    pub starts_at: i64,
    pub ends_at: i64,
    questions: Vec<DuelQuestion>,
    seats: Vec<Seat>,
    started: bool,
    settings: EngineSettings,
    end: Option<(EndReason, i64)>,
}

impl Match {
    pub fn new(setup: MatchSetup, questions: Vec<DuelQuestion>, settings: EngineSettings) -> Self {
        let seats = setup
            .seats
            .into_iter()
            .map(|s| Seat {
                kind: s.kind,
                name: s.name,
                user_id: s.user_id,
                rating: s.rating,
                score: 0,
                correct: 0,
                answered: 0,
                index: 0,
                sent_at: setup.starts_at,
                answer_time_ms: 0,
                suspicious: 0,
                answers: Vec::new(),
                connected: true,
                grace_until: None,
                forfeited: false,
                next_replay: 0,
            })
            .collect();
        Self {
            id: setup.id,
            mode: setup.mode,
            ranked: setup.ranked,
            key: setup.key,
            seed: setup.seed,
            created_at: setup.created_at,
            starts_at: setup.starts_at,
            ends_at: setup.starts_at + setup.key.rules.length_ms(),
            questions,
            seats,
            started: false,
            settings,
            end: None,
        }
    }

    pub fn seats(&self) -> &[Seat] {
        &self.seats
    }

    pub fn is_finished(&self) -> bool {
        self.end.is_some()
    }

    fn id_string(&self) -> String {
        self.id.to_string()
    }

    /// `match.start` for `seat`.
    pub fn start_message(&self, seat: usize, resumed: bool, now: i64) -> ServerMsg {
        ServerMsg::MatchStart(MatchStart {
            match_id: self.id_string(),
            mode: self.mode,
            ranked: self.ranked,
            subject: self.key.subject,
            rules: self.key.rules,
            seat,
            players: self
                .seats
                .iter()
                .enumerate()
                .map(|(i, s)| MatchPlayer {
                    seat: i,
                    name: s.name.clone(),
                    user_id: s.user_id.map(|id| id.to_string()),
                    guest: s.player().is_some_and(|p| p.is_guest()),
                    ghost: s.is_ghost(),
                    rating: s.rating,
                })
                .collect(),
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            server_time: now,
            resumed,
        })
    }

    pub fn scores_message(&self) -> ServerMsg {
        ServerMsg::MatchScores {
            match_id: self.id_string(),
            scores: self
                .seats
                .iter()
                .enumerate()
                .map(|(i, s)| SeatScore {
                    seat: i,
                    score: s.score,
                    answered: s.answered,
                    correct: s.correct,
                    connected: s.connected,
                })
                .collect(),
        }
    }

    /// The question `seat` is on, if it has one to answer now.
    fn question_message(&self, seat: usize) -> Option<ServerMsg> {
        let s = self.seats.get(seat)?;
        if !self.started || s.forfeited || self.is_finished() {
            return None;
        }
        let question = self.questions.get(s.index)?;
        let text = question.text(s.lang());
        Some(ServerMsg::Question {
            match_id: self.id_string(),
            index: s.index,
            question: text.question.clone(),
            series: text.series.clone(),
            choices: text.choices.clone(),
        })
    }

    /// The next time [`Self::tick`] has something to do.
    pub fn next_deadline(&self) -> Option<i64> {
        if self.is_finished() {
            return None;
        }
        let mut next = self.ends_at;
        if !self.started {
            next = next.min(self.starts_at);
        }
        for seat in &self.seats {
            if seat.forfeited {
                continue;
            }
            if let Some(grace) = seat.grace_until {
                next = next.min(grace);
            }
            if let SeatKind::Ghost { log, .. } = &seat.kind
                && let Some(entry) = log.get(seat.next_replay)
            {
                next = next.min(self.starts_at + entry.t);
            }
        }
        Some(next)
    }

    /// Applies everything due at `now`: the start, ghost answers, expired
    /// grace periods and the time limit.
    pub fn tick(&mut self, now: i64) -> Vec<Out> {
        let mut out = Vec::new();
        if self.is_finished() {
            return out;
        }
        if !self.started && now >= self.starts_at {
            self.started = true;
            for seat in 0..self.seats.len() {
                self.seats[seat].sent_at = self.starts_at;
                if self.seats[seat].connected
                    && !self.seats[seat].is_ghost()
                    && let Some(question) = self.question_message(seat)
                {
                    out.push(Out::To(seat, question));
                }
            }
        }
        if self.started {
            self.replay_ghosts(now, &mut out);
        }
        for seat in 0..self.seats.len() {
            let s = &self.seats[seat];
            if !s.forfeited && s.grace_until.is_some_and(|g| now >= g) {
                self.forfeit(seat, &mut out);
            }
        }
        self.check_end(now);
        out
    }

    fn replay_ghosts(&mut self, now: i64, out: &mut Vec<Out>) {
        let mut changed = false;
        // Apply in time order across ghosts (rooms never have several, but
        // nothing forbids it).
        loop {
            if self.is_finished() {
                break;
            }
            let due = self
                .seats
                .iter()
                .enumerate()
                .filter(|(_, s)| !s.forfeited)
                .filter_map(|(i, s)| match &s.kind {
                    SeatKind::Ghost { log, .. } => log
                        .get(s.next_replay)
                        .filter(|e| self.starts_at + e.t <= now)
                        .map(|e| (self.starts_at + e.t, i, *e)),
                    SeatKind::Live(_) => None,
                })
                .min_by_key(|(at, i, _)| (*at, *i));
            let Some((at, seat, entry)) = due else {
                break;
            };
            self.seats[seat].next_replay += 1;
            if self.seats[seat].index < self.questions.len() {
                self.record(seat, entry.c, entry.ok, at);
                changed = true;
                self.check_end(at);
            }
        }
        if changed {
            out.push(Out::All(self.scores_message()));
        }
    }

    /// Scores an answer of `seat` to its current question at `at`.
    fn record(&mut self, seat: usize, choice: u8, ok: bool, at: i64) {
        let rules = self.key.rules;
        let starts_at = self.starts_at;
        let suspicious_ms = self.settings.suspicious_ms;
        let s = &mut self.seats[seat];
        let elapsed = (at - s.sent_at).max(0);
        s.answers.push(AnswerLog {
            i: s.index,
            c: choice,
            ok,
            t: at - starts_at,
        });
        s.answered += 1;
        s.score += rules.points(ok);
        if ok {
            s.correct += 1;
            s.answer_time_ms += elapsed;
            if elapsed < suspicious_ms && !s.is_ghost() {
                s.suspicious += 1;
            }
        }
        s.index += 1;
        s.sent_at = at;
    }

    /// The player in `seat` answers question `index` with `choice`.
    pub fn answer(
        &mut self,
        seat: usize,
        match_id: &str,
        index: usize,
        choice: u8,
        now: i64,
    ) -> Result<Vec<Out>, (ErrorCode, &'static str)> {
        let mut out = self.tick(now);
        if match_id != self.id_string() {
            return Err((ErrorCode::BadRequest, "Not your match."));
        }
        if self.is_finished() {
            return Err((ErrorCode::NotAllowed, "The match is over."));
        }
        if !self.started {
            return Err((ErrorCode::NotAllowed, "The match has not started."));
        }
        let Some(s) = self.seats.get(seat) else {
            return Err((ErrorCode::BadRequest, "Not your match."));
        };
        if s.forfeited || s.is_ghost() {
            return Err((ErrorCode::NotAllowed, "You left this match."));
        }
        if index != s.index || index >= self.questions.len() {
            return Err((ErrorCode::BadRequest, "Not the current question."));
        }
        let correct_choice = self.questions.get(index).map_or(0, |q| q.correct);
        let ok = usize::from(choice) == correct_choice;
        self.record(seat, choice, ok, now);
        out.push(Out::To(
            seat,
            ServerMsg::AnswerResult {
                match_id: self.id_string(),
                index,
                correct: ok,
                correct_choice,
                score: self.seats[seat].score,
            },
        ));
        out.push(Out::All(self.scores_message()));
        self.check_end(now);
        if let Some(question) = self.question_message(seat) {
            out.push(Out::To(seat, question));
        }
        Ok(out)
    }

    pub fn disconnect(&mut self, seat: usize, now: i64) -> Vec<Out> {
        let mut out = self.tick(now);
        let grace_ms = self.settings.grace_ms;
        let Some(s) = self.seats.get_mut(seat) else {
            return out;
        };
        if self.end.is_some() || s.forfeited || !s.connected || s.is_ghost() {
            return out;
        }
        s.connected = false;
        let grace_until = now + grace_ms;
        s.grace_until = Some(grace_until);
        out.push(Out::Others(
            seat,
            ServerMsg::PlayerDisconnected {
                match_id: self.id.to_string(),
                seat,
                grace_until,
            },
        ));
        out
    }

    /// The player of `seat` is back: they get `match.start` (resumed), the
    /// scores and their current question.
    pub fn reconnect(&mut self, seat: usize, now: i64) -> Vec<Out> {
        let mut out = self.tick(now);
        let Some(s) = self.seats.get_mut(seat) else {
            return out;
        };
        if self.end.is_some() || s.forfeited || s.is_ghost() {
            return out;
        }
        let was_connected = s.connected;
        s.connected = true;
        s.grace_until = None;
        if !was_connected {
            out.push(Out::Others(
                seat,
                ServerMsg::PlayerReconnected {
                    match_id: self.id_string(),
                    seat,
                },
            ));
        }
        out.push(Out::To(seat, self.start_message(seat, true, now)));
        out.push(Out::To(seat, self.scores_message()));
        if let Some(question) = self.question_message(seat) {
            out.push(Out::To(seat, question));
        }
        out
    }

    /// `match.leave`: the player forfeits.
    pub fn leave(&mut self, seat: usize, now: i64) -> Vec<Out> {
        let mut out = self.tick(now);
        if self.end.is_none() && self.seats.get(seat).is_some_and(|s| !s.forfeited) {
            self.forfeit(seat, &mut out);
            self.check_end(now);
        }
        out
    }

    fn forfeit(&mut self, seat: usize, out: &mut Vec<Out>) {
        let Some(s) = self.seats.get_mut(seat) else {
            return;
        };
        s.forfeited = true;
        s.grace_until = None;
        out.push(Out::Released(seat));
        if self.end.is_none() {
            out.push(Out::All(self.scores_message()));
        }
    }

    fn check_end(&mut self, now: i64) {
        if self.end.is_some() {
            return;
        }
        let remaining: Vec<&Seat> = self.seats.iter().filter(|s| !s.forfeited).collect();
        let end = if remaining.len() < 2 && remaining.len() < self.seats.len() {
            Some((EndReason::Forfeit, now))
        } else if let Some(target) = self.key.rules.target()
            && remaining.iter().any(|s| s.score >= target)
        {
            Some((EndReason::Completed, now))
        } else if now >= self.ends_at {
            // A late wake-up still ends at the limit.
            let reason = match self.key.rules.target() {
                Some(_) => EndReason::Timeout,
                None => EndReason::Completed,
            };
            Some((reason, self.ends_at))
        } else if self.started && self.all_done(&remaining) {
            Some((EndReason::Completed, now))
        } else {
            None
        };
        if end.is_some() {
            self.end = end;
        }
    }

    /// Every connected live player answered every question, and every ghost
    /// replayed its whole run.
    fn all_done(&self, remaining: &[&Seat]) -> bool {
        let total = self.questions.len();
        let mut connected_live = 0;
        for seat in remaining {
            if seat.is_ghost() {
                if !seat.ghost_done() {
                    return false;
                }
            } else if seat.connected {
                connected_live += 1;
                if seat.index < total {
                    return false;
                }
            }
        }
        connected_live > 0
    }

    /// Ends the match now (server shutdown): whoever leads wins.
    pub fn abort(&mut self, now: i64) {
        if self.end.is_none() {
            self.end = Some((EndReason::Completed, now));
        }
    }

    /// The final ranking, once [`Self::is_finished`].
    pub fn result(&self) -> Option<Final> {
        let (reason, ended_at) = self.end?;
        let live_answered = self.seats.iter().any(|s| !s.is_ghost() && s.answered > 0);
        let reason = if live_answered {
            reason
        } else {
            EndReason::Void
        };
        let suspicious = self
            .seats
            .iter()
            .any(|s| s.suspicious > self.settings.suspicious_max);
        let ranked = self.ranked && reason != EndReason::Void && !suspicious;
        let standings: Vec<Standing> = self
            .seats
            .iter()
            .map(|s| Standing {
                score: s.score,
                answer_time_ms: s.answer_time_ms,
                forfeited: s.forfeited,
            })
            .collect();
        Some(Final {
            reason,
            ranked,
            ended_at,
            places: ranking::rank(&standings),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flash::{Subject, duel_questions};
    use crate::game::PlayerId;
    use crate::game::rules::{Limit, Rules, SubjectKey};

    const T0: i64 = 1_800_000_000_000;
    const START: i64 = T0 + 3_000;

    fn player(id: i64, name: &str) -> Arc<Player> {
        Arc::new(Player {
            id: PlayerId::Account(id),
            name: name.to_owned(),
            lang: Lang::Fr,
        })
    }

    fn live(id: i64, name: &str) -> SeatSetup {
        SeatSetup {
            kind: SeatKind::Live(player(id, name)),
            name: name.to_owned(),
            user_id: Some(id),
            rating: Some(1500),
        }
    }

    fn duel(limit: Limit, penalty: bool) -> Match {
        build(
            Mode::Duel,
            true,
            limit,
            penalty,
            vec![live(1, "a"), live(2, "b")],
            50,
        )
    }

    fn build(
        mode: Mode,
        ranked: bool,
        limit: Limit,
        penalty: bool,
        seats: Vec<SeatSetup>,
        count: usize,
    ) -> Match {
        let key = QueueKey {
            subject: SubjectKey(Some(Subject::Maths)),
            rules: Rules { limit, penalty },
        };
        Match::new(
            MatchSetup {
                id: Uuid::now_v7(),
                mode,
                ranked,
                key,
                seed: 77,
                created_at: T0,
                starts_at: START,
                seats,
            },
            duel_questions(key.subject.0, 77, count),
            EngineSettings::default(),
        )
    }

    fn correct(m: &Match, seat: usize) -> u8 {
        let index = m.seats()[seat].index;
        let q = duel_questions(m.key.subject.0, m.seed, index + 1);
        q.get(index).map_or(0, |q| q.correct as u8)
    }

    fn wrong(m: &Match, seat: usize) -> u8 {
        (correct(m, seat) + 1) % 4
    }

    fn answer(m: &mut Match, seat: usize, ok: bool, now: i64) -> Vec<Out> {
        let choice = if ok { correct(m, seat) } else { wrong(m, seat) };
        let index = m.seats()[seat].index;
        let id = m.id.to_string();
        m.answer(seat, &id, index, choice, now).expect("accepted")
    }

    fn questions_to(out: &[Out], seat: usize) -> Vec<usize> {
        out.iter()
            .filter_map(|o| match o {
                Out::To(s, ServerMsg::Question { index, .. }) if *s == seat => Some(*index),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn questions_start_after_the_countdown() {
        let mut m = duel(Limit::Duration(60), false);
        assert_eq!(m.next_deadline(), Some(START));
        assert!(m.tick(START - 1).is_empty());
        let id = m.id.to_string();
        assert_eq!(
            m.answer(0, &id, 0, 0, START - 1).map(|_| ()),
            Err((ErrorCode::NotAllowed, "The match has not started."))
        );
        let out = m.tick(START);
        assert_eq!(questions_to(&out, 0), vec![0]);
        assert_eq!(questions_to(&out, 1), vec![0]);
        assert_eq!(m.next_deadline(), Some(START + 60_000));
    }

    #[test]
    fn scoring_with_and_without_penalty() {
        let mut m = duel(Limit::Duration(60), true);
        m.tick(START);
        let out = answer(&mut m, 0, true, START + 1_000);
        assert_eq!(questions_to(&out, 0), vec![1], "next question follows");
        assert!(
            out.iter()
                .any(|o| matches!(o, Out::All(ServerMsg::MatchScores { .. })))
        );
        answer(&mut m, 0, false, START + 2_000);
        answer(&mut m, 1, false, START + 2_500);
        assert_eq!(m.seats()[0].score, 0);
        assert_eq!(m.seats()[1].score, -1);
        assert_eq!((m.seats()[0].answered, m.seats()[0].correct), (2, 1));

        let mut m = duel(Limit::Duration(60), false);
        m.tick(START);
        answer(&mut m, 1, false, START + 2_500);
        assert_eq!(m.seats()[1].score, 0);
    }

    #[test]
    fn only_the_current_question_is_accepted() {
        let mut m = duel(Limit::Duration(60), false);
        m.tick(START);
        let id = m.id.to_string();
        assert_eq!(
            m.answer(0, &id, 1, 0, START + 500).map(|_| ()),
            Err((ErrorCode::BadRequest, "Not the current question."))
        );
        assert_eq!(
            m.answer(0, "other", 0, 0, START + 500).map(|_| ()),
            Err((ErrorCode::BadRequest, "Not your match."))
        );
        answer(&mut m, 0, true, START + 500);
        assert_eq!(
            m.answer(0, &id, 0, 0, START + 600).map(|_| ()),
            Err((ErrorCode::BadRequest, "Not the current question."))
        );
    }

    #[test]
    fn duration_mode_ends_at_the_time_limit() {
        let mut m = duel(Limit::Duration(60), false);
        m.tick(START);
        answer(&mut m, 0, true, START + 1_000);
        answer(&mut m, 1, true, START + 2_000);
        answer(&mut m, 1, true, START + 3_000);
        m.tick(START + 59_999);
        assert!(!m.is_finished());
        m.tick(START + 60_000);
        let result = m.result().expect("finished");
        assert_eq!(result.reason, EndReason::Completed);
        assert!(result.ranked);
        assert_eq!(result.places, vec![(2, Outcome::Loss), (1, Outcome::Win)]);
        assert_eq!(result.ended_at, START + 60_000);
    }

    #[test]
    fn target_mode_ends_when_a_player_reaches_the_target() {
        let mut m = duel(Limit::Target(10), false);
        m.tick(START);
        for k in 0..9 {
            answer(&mut m, 0, true, START + 1_000 * (k + 1));
        }
        assert!(!m.is_finished());
        let out = answer(&mut m, 0, true, START + 10_000);
        assert!(m.is_finished());
        assert!(
            questions_to(&out, 0).is_empty(),
            "no question after the end"
        );
        let result = m.result().expect("finished");
        assert_eq!(result.reason, EndReason::Completed);
        assert_eq!(result.places[0], (1, Outcome::Win));
        assert_eq!(m.ends_at, START + 300_000);
    }

    #[test]
    fn target_mode_times_out_after_300_seconds() {
        let mut m = duel(Limit::Target(10), false);
        m.tick(START);
        answer(&mut m, 0, true, START + 1_000);
        answer(&mut m, 1, true, START + 1_500);
        m.tick(START + 300_000);
        let result = m.result().expect("finished");
        assert_eq!(result.reason, EndReason::Timeout);
        // Same score: player 0 was faster.
        assert_eq!(result.places, vec![(1, Outcome::Win), (2, Outcome::Loss)]);
    }

    #[test]
    fn answering_every_question_ends_the_match() {
        let mut m = build(
            Mode::Duel,
            true,
            Limit::Duration(300),
            false,
            vec![live(1, "a"), live(2, "b")],
            3,
        );
        m.tick(START);
        for k in 0..3 {
            answer(&mut m, 0, true, START + 1_000 * (k + 1));
        }
        assert!(!m.is_finished(), "the other player is still answering");
        for k in 0..3 {
            answer(&mut m, 1, k != 0, START + 5_000 * (k + 1));
        }
        let result = m.result().expect("finished");
        assert_eq!(result.reason, EndReason::Completed);
        assert_eq!(result.places, vec![(1, Outcome::Win), (2, Outcome::Loss)]);
    }

    #[test]
    fn a_disconnected_player_forfeits_after_the_grace_period() {
        let mut m = duel(Limit::Duration(60), false);
        m.tick(START);
        answer(&mut m, 0, true, START + 1_000);
        let out = m.disconnect(1, START + 2_000);
        assert!(out.iter().any(|o| matches!(
            o,
            Out::Others(1, ServerMsg::PlayerDisconnected { grace_until, .. }) if *grace_until == START + 22_000
        )));
        assert_eq!(m.next_deadline(), Some(START + 22_000));
        m.tick(START + 21_999);
        assert!(!m.is_finished());
        let out = m.tick(START + 22_000);
        assert!(out.contains(&Out::Released(1)));
        let result = m.result().expect("finished");
        assert_eq!(result.reason, EndReason::Forfeit);
        assert_eq!(
            result.places,
            vec![(1, Outcome::Win), (2, Outcome::Forfeit)]
        );
        assert!(result.ranked, "a ranked forfeit still counts");
    }

    #[test]
    fn reconnecting_in_time_resumes_the_match() {
        let mut m = duel(Limit::Duration(60), false);
        m.tick(START);
        answer(&mut m, 1, true, START + 1_000);
        m.disconnect(1, START + 2_000);
        let out = m.reconnect(1, START + 10_000);
        assert!(matches!(
            &out[0],
            Out::Others(1, ServerMsg::PlayerReconnected { seat: 1, .. })
        ));
        assert!(matches!(
            &out[1],
            Out::To(
                1,
                ServerMsg::MatchStart(MatchStart {
                    resumed: true,
                    seat: 1,
                    ..
                })
            )
        ));
        assert_eq!(questions_to(&out, 1), vec![1]);
        m.tick(START + 30_000);
        assert!(!m.is_finished());
        // The clock kept running: answer time counts from the first send.
        answer(&mut m, 1, true, START + 11_000);
        assert_eq!(m.seats()[1].answer_time_ms, 1_000 + 10_000);
    }

    #[test]
    fn leaving_a_duel_is_a_forfeit() {
        let mut m = duel(Limit::Duration(60), false);
        m.tick(START);
        answer(&mut m, 1, true, START + 1_000);
        let out = m.leave(1, START + 2_000);
        assert!(out.contains(&Out::Released(1)));
        let result = m.result().expect("finished");
        assert_eq!(result.reason, EndReason::Forfeit);
        assert_eq!(
            result.places,
            vec![(1, Outcome::Win), (2, Outcome::Forfeit)]
        );
    }

    #[test]
    fn a_room_goes_on_while_two_players_remain() {
        let mut m = build(
            Mode::Room,
            false,
            Limit::Duration(60),
            false,
            vec![live(1, "a"), live(2, "b"), live(3, "c")],
            50,
        );
        m.tick(START);
        answer(&mut m, 0, true, START + 1_000);
        m.leave(0, START + 2_000);
        assert!(!m.is_finished());
        answer(&mut m, 1, true, START + 3_000);
        m.leave(2, START + 4_000);
        let result = m.result().expect("finished");
        assert_eq!(result.reason, EndReason::Forfeit);
        assert!(!result.ranked);
        assert_eq!(
            result.places,
            vec![
                (2, Outcome::Forfeit),
                (1, Outcome::Win),
                (3, Outcome::Forfeit)
            ]
        );
    }

    #[test]
    fn a_match_without_answers_is_void() {
        let mut m = duel(Limit::Duration(60), false);
        m.tick(START);
        m.leave(0, START + 5_000);
        let result = m.result().expect("finished");
        assert_eq!(result.reason, EndReason::Void);
        assert!(!result.ranked);
    }

    #[test]
    fn too_many_fast_correct_answers_make_the_match_unranked() {
        let mut m = duel(Limit::Duration(60), false);
        m.tick(START);
        let mut now = START;
        // Three fast answers are tolerated.
        for _ in 0..3 {
            now += 299;
            answer(&mut m, 0, true, now);
        }
        // Fast wrong answers are not suspicious.
        now += 100;
        answer(&mut m, 0, false, now);
        now += 300;
        answer(&mut m, 0, true, now);
        m.tick(START + 60_000);
        assert!(m.result().expect("finished").ranked);

        let mut m = duel(Limit::Duration(60), false);
        m.tick(START);
        let mut now = START;
        for _ in 0..4 {
            now += 250;
            answer(&mut m, 1, true, now);
        }
        m.tick(START + 60_000);
        let result = m.result().expect("finished");
        assert!(!result.ranked);
        assert_eq!(result.reason, EndReason::Completed);
    }

    fn ghost(log: Vec<AnswerLog>) -> SeatSetup {
        SeatSetup {
            kind: SeatKind::Ghost {
                log,
                source_match: Uuid::now_v7(),
            },
            name: "ghost".to_owned(),
            user_id: None,
            rating: None,
        }
    }

    #[test]
    fn ghosts_answer_at_their_recorded_times() {
        let log = vec![
            AnswerLog {
                i: 0,
                c: 1,
                ok: true,
                t: 1_200,
            },
            AnswerLog {
                i: 1,
                c: 0,
                ok: false,
                t: 2_000,
            },
            AnswerLog {
                i: 2,
                c: 3,
                ok: true,
                t: 4_500,
            },
        ];
        let mut m = build(
            Mode::Ghost,
            false,
            Limit::Duration(60),
            true,
            vec![live(1, "a"), ghost(log.clone())],
            50,
        );
        let out = m.tick(START);
        assert_eq!(
            questions_to(&out, 1),
            Vec::<usize>::new(),
            "ghosts get no question"
        );
        assert_eq!(m.next_deadline(), Some(START + 1_200));
        assert!(m.tick(START + 1_199).is_empty());
        let out = m.tick(START + 1_200);
        assert!(matches!(
            &out[..],
            [Out::All(ServerMsg::MatchScores { .. })]
        ));
        assert_eq!(m.seats()[1].score, 1);
        assert_eq!(m.next_deadline(), Some(START + 2_000));
        // Late tick: both remaining entries are applied in order.
        m.tick(START + 10_000);
        assert_eq!(m.seats()[1].score, 1);
        assert_eq!((m.seats()[1].answered, m.seats()[1].correct), (3, 2));
        assert_eq!(m.seats()[1].answers, log);
        // Answer time over correct answers: 1200 + (4500 - 2000).
        assert_eq!(m.seats()[1].answer_time_ms, 1_200 + 2_500);
        assert_eq!(m.next_deadline(), Some(START + 60_000));
    }

    #[test]
    fn a_ghost_match_needs_a_live_answer_and_ends_on_forfeit() {
        let log = vec![AnswerLog {
            i: 0,
            c: 0,
            ok: true,
            t: 500,
        }];
        let mut m = build(
            Mode::Ghost,
            false,
            Limit::Duration(60),
            false,
            vec![live(1, "a"), ghost(log.clone())],
            50,
        );
        m.tick(START + 60_000);
        assert_eq!(m.result().map(|r| r.reason), Some(EndReason::Void));

        let mut m = build(
            Mode::Ghost,
            false,
            Limit::Duration(60),
            false,
            vec![live(1, "a"), ghost(log)],
            50,
        );
        m.tick(START);
        answer(&mut m, 0, true, START + 400);
        m.leave(0, START + 600);
        let result = m.result().expect("finished");
        assert_eq!(result.reason, EndReason::Forfeit);
        assert_eq!(
            result.places,
            vec![(2, Outcome::Forfeit), (1, Outcome::Win)]
        );
    }

    #[test]
    fn a_live_player_finishing_waits_for_the_ghost() {
        let log = (0..3)
            .map(|i| AnswerLog {
                i,
                c: 0,
                ok: false,
                t: 10_000 * (i as i64 + 1),
            })
            .collect();
        let mut m = build(
            Mode::Ghost,
            false,
            Limit::Duration(60),
            false,
            vec![live(1, "a"), ghost(log)],
            3,
        );
        m.tick(START);
        for k in 0..3 {
            answer(&mut m, 0, true, START + 1_000 * (k + 1));
        }
        assert!(!m.is_finished());
        m.tick(START + 30_000);
        let result = m.result().expect("finished");
        assert_eq!(result.reason, EndReason::Completed);
        assert_eq!(result.ended_at, START + 30_000);
    }
}
