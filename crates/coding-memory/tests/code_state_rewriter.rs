use coding_memory::code_state::{CodeState, CodeStateSnapshot};

#[test]
fn code_state_roundtrips_through_snapshot() {
    let states = vec![
        CodeState::Idle,
        CodeState::StackTraceActive {
            error_type: "NullPointer".into(),
        },
        CodeState::RedTestsRunning,
        CodeState::GreenTestsRunning,
        CodeState::BuildFailing,
        CodeState::Refactoring,
        CodeState::Debugging,
    ];
    for s in states {
        let snap: CodeStateSnapshot = s.to_snapshot();
        let back = CodeState::from_snapshot(&snap).unwrap();
        assert_eq!(s, back);
    }
}
