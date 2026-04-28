//! `EventDomain` — typed taxonomy for `DomainEvent::domain()`.
//!
//! Replaces the ~16 hardcoded strings returned by `DomainEvent::domain()` with
//! a closed enum. One `Custom(String)` variant handles the user-supplied tag
//! on `DomainEvent::UserStatedFact`, which is payload data rather than a
//! compile-time choice.
//!
//! Related but distinct:
//! - `ai_core::RecallDomain` — the AI-feature retrieval axis. Has fewer
//!   variants. Convert via `RecallDomain::from_event_domain`.
//! - `cognitive::USER_MODEL_DOMAINS` / `RULE_DOMAINS` — semantic-fact taxonomy
//!   (`identity`, `energy`, `work`, ...). Orthogonal to event categorization.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Event-category taxonomy for the `domain_event_log.domain` column and the
/// `DomainEvent::domain()` API.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventDomain {
    Work,
    Energy,
    Finance,
    Learning,
    Coaching,
    General,
    Notes,
    LanguageLearning,
    Productivity,
    Plugin,
    Agent,
    Fabric,
    Community,
    Lifecycle,
    Notifications,
    Scheduler,
    CodingMemory,
    Launcher,
    /// User-supplied tag from `DomainEvent::UserStatedFact.domain`. Not part of
    /// the fixed taxonomy — this carries payload data, not a call-site choice.
    Custom(String),
}

impl EventDomain {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Work => "work",
            Self::Energy => "energy",
            Self::Finance => "finance",
            Self::Learning => "learning",
            Self::Coaching => "coaching",
            Self::General => "general",
            Self::Notes => "notes",
            Self::LanguageLearning => "language_learning",
            Self::Productivity => "productivity",
            Self::Plugin => "plugin",
            Self::Agent => "agent",
            Self::Fabric => "fabric",
            Self::Community => "community",
            Self::Lifecycle => "lifecycle",
            Self::Notifications => "notifications",
            Self::Scheduler => "scheduler",
            Self::CodingMemory => "coding_memory",
            Self::Launcher => "launcher",
            Self::Custom(s) => s.as_str(),
        }
    }

    /// Parse a stored string back into an `EventDomain`. Unknown values become
    /// `Custom(s)` to preserve round-tripping; this is the inverse of
    /// `as_str`.
    pub fn parse(s: &str) -> Self {
        match s {
            "work" => Self::Work,
            "energy" => Self::Energy,
            "finance" => Self::Finance,
            "learning" => Self::Learning,
            "coaching" => Self::Coaching,
            "general" => Self::General,
            "notes" => Self::Notes,
            "language_learning" => Self::LanguageLearning,
            "productivity" => Self::Productivity,
            "plugin" => Self::Plugin,
            "agent" => Self::Agent,
            "fabric" => Self::Fabric,
            "community" => Self::Community,
            "lifecycle" => Self::Lifecycle,
            "notifications" => Self::Notifications,
            "scheduler" => Self::Scheduler,
            "coding_memory" => Self::CodingMemory,
            other => Self::Custom(other.to_string()),
        }
    }
}

impl fmt::Display for EventDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for EventDomain {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EventDomain {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::parse(&s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_variants_round_trip() {
        for d in [
            EventDomain::Work,
            EventDomain::Energy,
            EventDomain::Finance,
            EventDomain::Learning,
            EventDomain::Coaching,
            EventDomain::General,
            EventDomain::Notes,
            EventDomain::LanguageLearning,
            EventDomain::Productivity,
            EventDomain::Plugin,
            EventDomain::Agent,
            EventDomain::Fabric,
            EventDomain::Community,
            EventDomain::Lifecycle,
            EventDomain::Notifications,
            EventDomain::Scheduler,
            EventDomain::CodingMemory,
        ] {
            assert_eq!(EventDomain::parse(d.as_str()), d);
        }
    }

    #[test]
    fn custom_round_trips() {
        let d = EventDomain::Custom("my_fact_kind".to_string());
        assert_eq!(d.as_str(), "my_fact_kind");
        assert_eq!(EventDomain::parse("my_fact_kind"), d);
    }

    #[test]
    fn display_matches_as_str() {
        assert_eq!(format!("{}", EventDomain::Finance), "finance");
        assert_eq!(format!("{}", EventDomain::Custom("x".to_string())), "x");
    }

    #[test]
    fn serde_serializes_as_string() {
        let d = EventDomain::Finance;
        assert_eq!(serde_json::to_string(&d).unwrap(), "\"finance\"");
        let parsed: EventDomain = serde_json::from_str("\"finance\"").unwrap();
        assert_eq!(parsed, EventDomain::Finance);
        let parsed: EventDomain = serde_json::from_str("\"unknown\"").unwrap();
        assert_eq!(parsed, EventDomain::Custom("unknown".to_string()));
    }
}
