//! Schema for events emitted by plugins via the `agent_emit_event` host function.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginEmittedEvent {
    pub kind: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginEventValidationError {
    #[error("plugin event kind must be non-empty")]
    EmptyKind,
    #[error("plugin event kind must be ASCII alphanumeric or underscore (got {0:?})")]
    InvalidKindChars(String),
    #[error("plugin event kind exceeds 64 chars")]
    KindTooLong,
    #[error("plugin event payload exceeds 4 KiB JSON")]
    PayloadTooLarge,
}

impl PluginEmittedEvent {
    pub fn validate(&self) -> Result<(), PluginEventValidationError> {
        if self.kind.is_empty() {
            return Err(PluginEventValidationError::EmptyKind);
        }
        if self.kind.len() > 64 {
            return Err(PluginEventValidationError::KindTooLong);
        }
        if !self
            .kind
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(PluginEventValidationError::InvalidKindChars(
                self.kind.clone(),
            ));
        }
        let payload_size = serde_json::to_vec(&self.payload)
            .map(|v| v.len())
            .unwrap_or(0);
        if payload_size > 4096 {
            return Err(PluginEventValidationError::PayloadTooLarge);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_json() {
        let s = r#"{"kind":"my_event","payload":{"x":1}}"#;
        let e: PluginEmittedEvent = serde_json::from_str(s).unwrap();
        assert_eq!(e.kind, "my_event");
        assert!(e.validate().is_ok());
    }

    #[test]
    fn rejects_empty_kind() {
        let e = PluginEmittedEvent {
            kind: "".to_string(),
            payload: serde_json::Value::Null,
        };
        assert!(matches!(
            e.validate(),
            Err(PluginEventValidationError::EmptyKind)
        ));
    }

    #[test]
    fn rejects_invalid_chars_in_kind() {
        let e = PluginEmittedEvent {
            kind: "bad-kind!".to_string(),
            payload: serde_json::Value::Null,
        };
        assert!(matches!(
            e.validate(),
            Err(PluginEventValidationError::InvalidKindChars(_))
        ));
    }

    #[test]
    fn rejects_oversized_payload() {
        let huge = serde_json::Value::String("x".repeat(5000));
        let e = PluginEmittedEvent {
            kind: "k".to_string(),
            payload: huge,
        };
        assert!(matches!(
            e.validate(),
            Err(PluginEventValidationError::PayloadTooLarge)
        ));
    }
}
