//! Authoritative discriminator for assistant vs coding sessions.
//!
//! Set at session creation, never mutated. Stored as a NOT NULL `TEXT`
//! column on `sessions` and serialized as `"assistant"` / `"coding"`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    #[default]
    Assistant,
    Coding,
}

impl SessionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Assistant => "assistant",
            Self::Coding => "coding",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "assistant" => Some(Self::Assistant),
            "coding" => Some(Self::Coding),
            _ => None,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_via_str() {
        for m in [SessionMode::Assistant, SessionMode::Coding] {
            assert_eq!(SessionMode::parse(m.as_str()), Some(m));
        }
    }

    #[test]
    fn parse_unknown_is_none() {
        assert_eq!(SessionMode::parse("chat"), None);
        assert_eq!(SessionMode::parse(""), None);
    }

    #[test]
    fn serde_uses_snake_case() {
        let s = serde_json::to_string(&SessionMode::Assistant).unwrap();
        assert_eq!(s, "\"assistant\"");
        let parsed: SessionMode = serde_json::from_str("\"coding\"").unwrap();
        assert_eq!(parsed, SessionMode::Coding);
    }

    #[test]
    fn default_is_assistant() {
        assert_eq!(SessionMode::default(), SessionMode::Assistant);
    }
}
