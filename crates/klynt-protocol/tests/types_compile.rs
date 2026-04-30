use klynt_protocol::{CodingTraceEvent, Op, ProtocolError, Submission, SubmissionResult};

#[test]
fn types_are_constructible() {
    let _ = Op::ToolCall {
        tool: "bash".into(),
        args: serde_json::json!({}),
    };
    let _ = Submission {
        id: "s1".into(),
        op: Op::NoOp,
    };
    let _ = SubmissionResult::Ok { id: "s1".into() };
    let _ = CodingTraceEvent::IterationStart { iteration: 0 };
    let _: ProtocolError = ProtocolError::InvalidOp("x".into());
}
