//! Spawn a synthetic `cargo`-style failure with ANSI colours, verify the gate
//! still classifies it as TestFailure (i.e. strip_ansi ran pre-regex).

use feature_coding_bash::gate::GateClassifier;
use tools_core::{FailureKind, GateResult};

#[test]
fn cargo_colored_test_failure_classifies() {
    let coloured = "\x1b[31mtest some::test ... FAILED\x1b[0m\n\
                    \x1b[31mtest result: FAILED. 0 passed; 1 failed\x1b[0m";
    let r = GateClassifier::classify(coloured, "", 101, "cargo nextest run", false, false, false, 0);
    match r {
        GateResult::Failed { kind, extracted, .. } => {
            assert!(matches!(kind, FailureKind::TestFailure));
            let names = extracted
                .get("failed_test_names")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            assert!(names >= 1, "expected at least one failed test name");
        }
        other => panic!("expected Failed/TestFailure, got: {other:?}"),
    }
}
