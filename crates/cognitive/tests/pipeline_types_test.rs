use ai_core::RecallDomain;
use cognitive::pipeline::{CognitiveSignal, SignalContext, SignalSource};
use jiff::Timestamp;

#[test]
fn cognitive_signal_uses_typed_domain() {
    let s = CognitiveSignal {
        source: SignalSource::ChatTurn,
        content: "x".into(),
        domain: RecallDomain::General,
        confidence: 0.5,
        context: SignalContext::default(),
        timestamp: Timestamp::now(),
    };
    assert_eq!(s.domain, RecallDomain::General);
    assert_eq!(s.domain.as_str(), "general");
}
