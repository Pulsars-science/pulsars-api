//! WebSocket messages (`game-protocol.md` §3): JSON text frames
//! `{"type": "...", ...}`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ranking::Outcome;
use super::rules::{Mode, Rules, SubjectKey};

/// Close codes (`game-protocol.md` §3.1, `auth-jwt.md` §5).
pub mod close {
    /// No `auth` frame in time (or another frame first).
    pub const NO_AUTH: u16 = 4000;
    /// Invalid token; the close reason is the `reason` code.
    pub const INVALID_TOKEN: u16 = 4001;
    /// The token's `jti` already opened a session.
    pub const JTI_REUSED: u16 = 4003;
    /// The account opened another session.
    pub const REPLACED: u16 = 4009;
    /// More than 20 frames a second.
    pub const RATE_LIMITED: u16 = 4029;
    /// The client does not read its messages fast enough.
    pub const TOO_SLOW: u16 = 1008;
    /// A dependency (keys, Redis) is unavailable: try again later.
    pub const TRY_AGAIN_LATER: u16 = 1013;
    /// Server shutting down or internal error.
    pub const GOING_AWAY: u16 = 1001;
}

/// `error.code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    NotAllowed,
    RoomNotFound,
    RoomFull,
    RoomStarted,
    NoGhost,
    AlreadyPlaying,
}

/// The `auth` frame.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AuthFrame {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub guest: bool,
    #[serde(default)]
    pub resume: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KeyFrame {
    pub subject: SubjectKey,
    pub rules: Rules,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CodeFrame {
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AnswerFrame {
    pub match_id: String,
    pub index: usize,
    pub choice: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MatchFrame {
    pub match_id: String,
}

/// A frame from the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientMsg {
    Auth(AuthFrame),
    QueueJoin(KeyFrame),
    QueueLeave,
    GhostStart(KeyFrame),
    RoomCreate(KeyFrame),
    RoomJoin(CodeFrame),
    RoomLeave,
    RoomStart,
    Answer(AnswerFrame),
    MatchLeave(MatchFrame),
    Ping,
}

/// Why a frame was not understood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Not a JSON object with a string `type`.
    NotAMessage,
    /// A type this server does not know.
    UnknownType(String),
    /// A known type with missing or invalid fields.
    Invalid { kind: &'static str, detail: String },
}

impl ParseError {
    pub fn message(&self) -> String {
        match self {
            Self::NotAMessage => "Frames are JSON objects with a string `type`.".to_owned(),
            Self::UnknownType(kind) => {
                let kind: String = kind.chars().take(40).collect();
                format!("Unknown message type `{kind}`.")
            }
            Self::Invalid { kind, detail } => format!("Invalid `{kind}`: {detail}"),
        }
    }
}

pub fn parse(text: &str) -> Result<ClientMsg, ParseError> {
    let value: Value = serde_json::from_str(text).map_err(|_| ParseError::NotAMessage)?;
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or(ParseError::NotAMessage)?
        .to_owned();
    fn fields<T: for<'de> Deserialize<'de>>(
        kind: &'static str,
        value: Value,
    ) -> Result<T, ParseError> {
        serde_json::from_value(value).map_err(|e| ParseError::Invalid {
            kind,
            detail: e.to_string(),
        })
    }
    Ok(match kind.as_str() {
        "auth" => ClientMsg::Auth(fields("auth", value)?),
        "queue.join" => ClientMsg::QueueJoin(fields("queue.join", value)?),
        "queue.leave" => ClientMsg::QueueLeave,
        "ghost.start" => ClientMsg::GhostStart(fields("ghost.start", value)?),
        "room.create" => ClientMsg::RoomCreate(fields("room.create", value)?),
        "room.join" => ClientMsg::RoomJoin(fields("room.join", value)?),
        "room.leave" => ClientMsg::RoomLeave,
        "room.start" => ClientMsg::RoomStart,
        "answer" => {
            let frame: AnswerFrame = fields("answer", value)?;
            if frame.choice > 3 {
                return Err(ParseError::Invalid {
                    kind: "answer",
                    detail: "choice must be between 0 and 3".to_owned(),
                });
            }
            ClientMsg::Answer(frame)
        }
        "match.leave" => ClientMsg::MatchLeave(fields("match.leave", value)?),
        "ping" => ClientMsg::Ping,
        _ => return Err(ParseError::UnknownType(kind)),
    })
}

/// `auth.ok.player`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlayerInfo {
    pub user_id: Option<String>,
    pub name: String,
    pub guest: bool,
    pub locale: &'static str,
}

/// An entry of `room.state.players`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoomPlayer {
    pub seat: usize,
    pub name: String,
    pub user_id: Option<String>,
    pub guest: bool,
}

/// An entry of `match.start.players`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchPlayer {
    pub seat: usize,
    pub name: String,
    pub user_id: Option<String>,
    pub guest: bool,
    pub ghost: bool,
    pub rating: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchStart {
    pub match_id: String,
    pub mode: Mode,
    pub ranked: bool,
    pub subject: SubjectKey,
    pub rules: Rules,
    pub seat: usize,
    pub players: Vec<MatchPlayer>,
    pub starts_at: i64,
    pub ends_at: i64,
    pub server_time: i64,
    pub resumed: bool,
}

/// An entry of `match.scores.scores`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeatScore {
    pub seat: usize,
    pub score: i32,
    pub answered: u32,
    pub correct: u32,
    pub connected: bool,
}

/// An entry of `match.end.results`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeatResult {
    pub seat: usize,
    pub name: String,
    pub user_id: Option<String>,
    pub score: i32,
    pub correct: u32,
    pub answered: u32,
    pub rank: u16,
    pub outcome: Outcome,
    pub rating_before: Option<i32>,
    pub rating_after: Option<i32>,
}

/// `match.end.end_reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EndReason {
    Completed,
    Timeout,
    Forfeit,
    Void,
}

impl EndReason {
    pub fn key(self) -> &'static str {
        match self {
            EndReason::Completed => "completed",
            EndReason::Timeout => "timeout",
            EndReason::Forfeit => "forfeit",
            EndReason::Void => "void",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchEnd {
    pub match_id: String,
    pub end_reason: EndReason,
    pub ranked: bool,
    pub results: Vec<SeatResult>,
}

/// A frame to the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum ServerMsg {
    #[serde(rename = "error")]
    Error { code: ErrorCode, message: String },
    #[serde(rename = "auth.ok")]
    AuthOk {
        player: PlayerInfo,
        resume: Option<String>,
    },
    #[serde(rename = "queue.joined")]
    QueueJoined {
        subject: SubjectKey,
        rules: Rules,
        ranked: bool,
    },
    #[serde(rename = "queue.waiting")]
    QueueWaiting { seconds: u64, ghost_available: bool },
    #[serde(rename = "queue.left")]
    QueueLeft,
    #[serde(rename = "room.state")]
    RoomState {
        code: String,
        host_seat: usize,
        players: Vec<RoomPlayer>,
        subject: SubjectKey,
        rules: Rules,
    },
    #[serde(rename = "room.closed")]
    RoomClosed { reason: &'static str },
    #[serde(rename = "match.start")]
    MatchStart(MatchStart),
    #[serde(rename = "question")]
    Question {
        match_id: String,
        index: usize,
        question: String,
        series: String,
        choices: [String; 4],
    },
    #[serde(rename = "answer.result")]
    AnswerResult {
        match_id: String,
        index: usize,
        correct: bool,
        correct_choice: usize,
        score: i32,
    },
    #[serde(rename = "match.scores")]
    MatchScores {
        match_id: String,
        scores: Vec<SeatScore>,
    },
    #[serde(rename = "player.disconnected")]
    PlayerDisconnected {
        match_id: String,
        seat: usize,
        grace_until: i64,
    },
    #[serde(rename = "player.reconnected")]
    PlayerReconnected { match_id: String, seat: usize },
    #[serde(rename = "match.end")]
    MatchEnd(MatchEnd),
    #[serde(rename = "pong")]
    Pong { server_time: i64 },
}

impl ServerMsg {
    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Error {
            code,
            message: message.into(),
        }
    }

    /// The JSON text of the frame.
    pub fn to_text(&self) -> String {
        // Plain data with string keys: serialization cannot fail.
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"type":"error","code":"bad_request","message":"internal"}"#.to_owned()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::rules::Limit;
    use serde_json::json;

    #[test]
    fn parses_every_client_message() {
        assert_eq!(
            parse(r#"{"type":"auth","token":"abc"}"#),
            Ok(ClientMsg::Auth(AuthFrame {
                token: Some("abc".to_owned()),
                guest: false,
                resume: None,
                locale: None
            }))
        );
        assert_eq!(
            parse(
                r#"{"type":"queue.join","subject":"maths","rules":{"duration":60,"target":null,"penalty":true}}"#
            ),
            Ok(ClientMsg::QueueJoin(KeyFrame {
                subject: SubjectKey(Some(crate::flash::Subject::Maths)),
                rules: Rules {
                    limit: Limit::Duration(60),
                    penalty: true
                }
            }))
        );
        assert_eq!(parse(r#"{"type":"ping"}"#), Ok(ClientMsg::Ping));
        assert_eq!(
            parse(r#"{"type":"answer","match_id":"m","index":3,"choice":2}"#),
            Ok(ClientMsg::Answer(AnswerFrame {
                match_id: "m".to_owned(),
                index: 3,
                choice: 2
            }))
        );
    }

    #[test]
    fn rejects_bad_frames() {
        assert_eq!(parse("not json"), Err(ParseError::NotAMessage));
        assert_eq!(parse(r#"{"no":"type"}"#), Err(ParseError::NotAMessage));
        assert_eq!(
            parse(r#"{"type":"dance"}"#),
            Err(ParseError::UnknownType("dance".to_owned()))
        );
        assert!(matches!(
            parse(r#"{"type":"answer","match_id":"m","index":0,"choice":4}"#),
            Err(ParseError::Invalid { kind: "answer", .. })
        ));
        assert!(matches!(
            parse(
                r#"{"type":"queue.join","subject":"biology","rules":{"duration":60,"penalty":true}}"#
            ),
            Err(ParseError::Invalid { .. })
        ));
    }

    #[test]
    fn server_messages_have_the_documented_shape() {
        let msg = ServerMsg::error(ErrorCode::NoGhost, "none");
        assert_eq!(
            serde_json::to_value(&msg).unwrap_or_default(),
            json!({"type": "error", "code": "no_ghost", "message": "none"})
        );
        assert_eq!(ServerMsg::QueueLeft.to_text(), r#"{"type":"queue.left"}"#);
        let end = ServerMsg::MatchEnd(MatchEnd {
            match_id: "m".to_owned(),
            end_reason: EndReason::Forfeit,
            ranked: true,
            results: vec![SeatResult {
                seat: 0,
                name: "ada".to_owned(),
                user_id: Some("2".to_owned()),
                score: 3,
                correct: 3,
                answered: 4,
                rank: 1,
                outcome: Outcome::Win,
                rating_before: Some(1500),
                rating_after: Some(1662),
            }],
        });
        assert_eq!(
            serde_json::to_value(&end).unwrap_or_default(),
            json!({"type": "match.end", "match_id": "m", "end_reason": "forfeit", "ranked": true,
                   "results": [{"seat": 0, "name": "ada", "user_id": "2", "score": 3, "correct": 3,
                                "answered": 4, "rank": 1, "outcome": "win",
                                "rating_before": 1500, "rating_after": 1662}]})
        );
    }
}
