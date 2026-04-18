//! Common signal type produced by all collectors and consumed by the consolidator.

use jiff::Timestamp;

#[derive(Debug, Clone)]
pub struct CognitiveSignal {
    pub source: SignalSource,
    pub content: String,
    pub domain: String,
    pub confidence: f64,
    pub context: SignalContext,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalSource {
    ChatTurn,
    SessionEnd,
    AtomReinforcement,
    CoachingPattern,
    ConversationRecall,
    UserStatedFact,
}

#[derive(Debug, Clone, Default)]
pub struct SignalContext {
    pub session_key: Option<String>,
    pub related_fact_ids: Vec<String>,
    pub related_atom_ids: Vec<String>,
    pub source_count: u32,
    pub raw_observations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_source_equality() {
        assert_eq!(SignalSource::ChatTurn, SignalSource::ChatTurn);
        assert_ne!(SignalSource::ChatTurn, SignalSource::SessionEnd);
    }

    #[test]
    fn test_signal_construction() {
        let signal = CognitiveSignal {
            source: SignalSource::ChatTurn,
            content: "User is a software engineer".into(),
            domain: "identity".into(),
            confidence: 0.8,
            context: SignalContext {
                session_key: Some("sess_1".into()),
                source_count: 1,
                ..Default::default()
            },
            timestamp: Timestamp::now(),
        };
        assert_eq!(signal.confidence, 0.8);
        assert_eq!(signal.context.source_count, 1);
    }
}
