//! What a match is played on: subject, rules, mode (`game-protocol.md` §1).

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::flash::Subject;

/// Every subject key a client may send, `all` included.
pub const SUBJECT_KEYS: &[&str] = &["maths", "physics", "chemistry", "computer-science", "all"];

/// A subject, or `all` (`None`): selects the generators of a match and the
/// rating it counts for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubjectKey(pub Option<Subject>);

impl SubjectKey {
    pub fn parse(key: &str) -> Option<Self> {
        match key {
            "all" => Some(Self(None)),
            other => Subject::from_key(other).map(|s| Self(Some(s))),
        }
    }

    pub fn key(self) -> &'static str {
        self.0.map_or("all", Subject::key)
    }
}

impl Serialize for SubjectKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}

impl<'de> Deserialize<'de> for SubjectKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "subject must be one of: {}",
                SUBJECT_KEYS.join(", ")
            ))
        })
    }
}

/// How a match ends: after a fixed time, or at a target score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Limit {
    /// Seconds: 60 or 300.
    Duration(u32),
    /// Points: 10.
    Target(i32),
}

/// `{"duration": 60 | 300 | null, "target": 10 | null, "penalty": bool}`,
/// exactly one of `duration` and `target` set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rules {
    pub limit: Limit,
    /// A wrong answer costs a point.
    pub penalty: bool,
}

/// Hard end of a match in target mode, in seconds.
pub const TARGET_MODE_MAX_SECS: u32 = 300;

impl Rules {
    pub const DURATIONS: [u32; 2] = [60, 300];
    pub const TARGETS: [i32; 1] = [10];

    /// Points for an answer.
    pub fn points(self, correct: bool) -> i32 {
        match (correct, self.penalty) {
            (true, _) => 1,
            (false, true) => -1,
            (false, false) => 0,
        }
    }

    /// Length of the match from its start, in milliseconds.
    pub fn length_ms(self) -> i64 {
        match self.limit {
            Limit::Duration(secs) => i64::from(secs) * 1000,
            Limit::Target(_) => i64::from(TARGET_MODE_MAX_SECS) * 1000,
        }
    }

    pub fn target(self) -> Option<i32> {
        match self.limit {
            Limit::Target(points) => Some(points),
            Limit::Duration(_) => None,
        }
    }

    /// The canonical JSON, as stored in `game.matches.rules`.
    pub fn to_json(self) -> serde_json::Value {
        serde_json::to_value(RulesWire::from(self)).unwrap_or(serde_json::Value::Null)
    }
}

#[derive(Serialize, Deserialize)]
struct RulesWire {
    #[serde(default)]
    duration: Option<u32>,
    #[serde(default)]
    target: Option<i32>,
    penalty: bool,
}

impl From<Rules> for RulesWire {
    fn from(rules: Rules) -> Self {
        let (duration, target) = match rules.limit {
            Limit::Duration(secs) => (Some(secs), None),
            Limit::Target(points) => (None, Some(points)),
        };
        Self {
            duration,
            target,
            penalty: rules.penalty,
        }
    }
}

impl TryFrom<RulesWire> for Rules {
    type Error = String;

    fn try_from(wire: RulesWire) -> Result<Self, String> {
        let limit = match (wire.duration, wire.target) {
            (Some(secs), None) if Self::DURATIONS.contains(&secs) => Limit::Duration(secs),
            (None, Some(points)) if Self::TARGETS.contains(&points) => Limit::Target(points),
            (Some(_), None) => return Err("duration must be 60 or 300".to_owned()),
            (None, Some(_)) => return Err("target must be 10".to_owned()),
            _ => return Err("exactly one of duration and target must be set".to_owned()),
        };
        Ok(Self {
            limit,
            penalty: wire.penalty,
        })
    }
}

impl Serialize for Rules {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        RulesWire::from(*self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Rules {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = RulesWire::deserialize(deserializer)?;
        Rules::try_from(wire).map_err(serde::de::Error::custom)
    }
}

/// Matchmaking only pairs players with the same key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueueKey {
    pub subject: SubjectKey,
    pub rules: Rules,
}

/// Kind of match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// 1 vs 1 from matchmaking.
    Duel,
    /// Private room, 2-8 players.
    Room,
    /// Against a recorded run.
    Ghost,
}

impl Mode {
    /// Value of `game.matches.mode` and of the WebSocket messages.
    pub fn key(self) -> &'static str {
        match self {
            Mode::Duel => "duel",
            Mode::Room => "room",
            Mode::Ghost => "ghost",
        }
    }

    /// Value of the webhook's `data.mode`.
    pub fn webhook_key(self) -> &'static str {
        match self {
            Mode::Duel => "flash-duel",
            Mode::Room => "flash-room",
            Mode::Ghost => "flash-ghost",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "duel" => Some(Mode::Duel),
            "room" => Some(Mode::Room),
            "ghost" => Some(Mode::Ghost),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rules(value: serde_json::Value) -> Result<Rules, String> {
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    #[test]
    fn exactly_one_limit() {
        assert_eq!(
            rules(json!({"duration": 60, "target": null, "penalty": true})),
            Ok(Rules {
                limit: Limit::Duration(60),
                penalty: true
            })
        );
        assert_eq!(
            rules(json!({"target": 10, "penalty": false})).map(|r| r.limit),
            Ok(Limit::Target(10))
        );
        for bad in [
            json!({"duration": 60, "target": 10, "penalty": true}),
            json!({"duration": null, "target": null, "penalty": true}),
            json!({"duration": 30, "penalty": true}),
            json!({"target": 5, "penalty": true}),
            json!({"duration": 60}),
        ] {
            assert!(rules(bad.clone()).is_err(), "{bad}");
        }
    }

    #[test]
    fn canonical_json() {
        let r = Rules {
            limit: Limit::Target(10),
            penalty: true,
        };
        assert_eq!(
            r.to_json(),
            json!({"duration": null, "target": 10, "penalty": true})
        );
        assert_eq!(r.length_ms(), 300_000);
        assert_eq!(r.points(false), -1);
        assert_eq!(
            Rules {
                limit: Limit::Duration(60),
                penalty: false
            }
            .points(false),
            0
        );
    }

    #[test]
    fn subject_keys() {
        for key in SUBJECT_KEYS {
            assert_eq!(SubjectKey::parse(key).map(SubjectKey::key), Some(*key));
        }
        assert_eq!(SubjectKey::parse("biology"), None);
    }
}
