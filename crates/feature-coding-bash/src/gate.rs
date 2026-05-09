//! Gate classifier — turns command output + exit code into a structured
//! `GateResult` with extracted fields the LLM can use directly.

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::json;
use tools_core::{FailureKind, GateResult};

static RUST_COMPILE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"error\[E(\d{4})\]: (.+?)\n\s+--> ([^:\n]+):(\d+):(\d+)").unwrap()
});
static TS_COMPILE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([^\s:]+\.tsx?):(\d+):(\d+) - error TS(\d{4}): (.+)").unwrap()
});
static CARGO_COMPILE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"error: could not compile `([^`]+)`").unwrap()
});
static RUST_TEST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"test result: FAILED\. (\d+) passed; (\d+) failed").unwrap()
});
static VITEST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"Tests\s+(\d+) failed\s*\|\s*(\d+) passed").unwrap()
});
static FIRST_FAILED_TEST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"test ([\w:]+) \.\.\. FAILED").unwrap()
});
static CLIPPY_ABORT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"error: aborting due to (\d+) previous error").unwrap()
});
static ESLINT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"✖ (\d+) problems? \((\d+) errors?").unwrap()
});
static EADDRINUSE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"EADDRINUSE.*?:::?(\d+)").unwrap()
});

pub struct GateClassifier;

impl GateClassifier {
    pub fn classify(
        stdout: &str,
        stderr: &str,
        exit_code: i32,
        command: &str,
        was_timeout: bool,
        was_cancelled: bool,
        was_lost: bool,
        elapsed_ms: u64,
    ) -> GateResult {
        if was_lost {
            return GateResult::Failed {
                kind: FailureKind::Lost,
                detail: "klynt restarted while job was running".into(),
                extracted: json!({ "log_preserved": true }),
            };
        }
        if was_cancelled {
            return GateResult::Failed {
                kind: FailureKind::Cancelled,
                detail: "explicit stop or thread deletion".into(),
                extracted: json!({ "elapsed_ms": elapsed_ms }),
            };
        }
        if was_timeout {
            return GateResult::Failed {
                kind: FailureKind::Timeout,
                detail: "wall-clock exceeded timeout".to_string(),
                extracted: json!({ "elapsed_ms": elapsed_ms }),
            };
        }
        if exit_code == 0 {
            return GateResult::Passed;
        }

        // Try detectors in priority order.
        if let Some(r) = detect_compile_error(stderr, stdout) { return r; }
        if let Some(r) = detect_test_failure(stdout, stderr) { return r; }
        if let Some(r) = detect_lint_failure(stdout, stderr, command) { return r; }
        if let Some(r) = detect_port_in_use(stderr) { return r; }

        // Fallback.
        let signal_hint = if exit_code == 137 { Some("SIGKILL (likely OOM)") }
            else if exit_code == 143 { Some("SIGTERM") }
            else { None };
        GateResult::Failed {
            kind: FailureKind::Other(format!("exit code {exit_code}")),
            detail: format!("exit code {exit_code}"),
            extracted: json!({
                "exit_code": exit_code,
                "signal_hint": signal_hint,
            }),
        }
    }
}

fn detect_compile_error(stderr: &str, stdout: &str) -> Option<GateResult> {
    if let Some(c) = RUST_COMPILE_RE.captures(stderr).or_else(|| RUST_COMPILE_RE.captures(stdout)) {
        return Some(GateResult::Failed {
            kind: FailureKind::CompileError,
            detail: format!("error[E{}]: {}", &c[1], &c[2]),
            extracted: json!({
                "file": c[3].to_string(),
                "line": c[4].parse::<u32>().unwrap_or(0),
                "col": c[5].parse::<u32>().unwrap_or(0),
                "diagnostic_code": format!("E{}", &c[1]),
                "diagnostic_message": c[2].to_string(),
            }),
        });
    }
    if let Some(c) = TS_COMPILE_RE.captures(stderr).or_else(|| TS_COMPILE_RE.captures(stdout)) {
        return Some(GateResult::Failed {
            kind: FailureKind::CompileError,
            detail: format!("error TS{}: {}", &c[4], &c[5]),
            extracted: json!({
                "file": c[1].to_string(),
                "line": c[2].parse::<u32>().unwrap_or(0),
                "col": c[3].parse::<u32>().unwrap_or(0),
                "diagnostic_code": format!("TS{}", &c[4]),
                "diagnostic_message": c[5].to_string(),
            }),
        });
    }
    if let Some(c) = CARGO_COMPILE_RE.captures(stderr).or_else(|| CARGO_COMPILE_RE.captures(stdout)) {
        if !stderr.contains("clippy::") && !stdout.contains("clippy::") {
            return Some(GateResult::Failed {
                kind: FailureKind::CompileError,
                detail: format!("could not compile `{}`", &c[1]),
                extracted: json!({ "crate": c[1].to_string() }),
            });
        }
    }
    None
}

fn detect_test_failure(stdout: &str, stderr: &str) -> Option<GateResult> {
    if let Some(c) = RUST_TEST_RE.captures(stdout).or_else(|| RUST_TEST_RE.captures(stderr)) {
        let n_passed: u32 = c[1].parse().unwrap_or(0);
        let n_failed: u32 = c[2].parse().unwrap_or(0);
        let test_name = first_failed_rust_test(stdout)
            .or_else(|| first_failed_rust_test(stderr));
        return Some(GateResult::Failed {
            kind: FailureKind::TestFailure,
            detail: format!("{n_failed} failed; {n_passed} passed"),
            extracted: json!({
                "test_name": test_name,
                "n_failed": n_failed,
                "n_passed": n_passed,
                "n_ignored": 0,
            }),
        });
    }
    if let Some(c) = VITEST_RE.captures(stdout).or_else(|| VITEST_RE.captures(stderr)) {
        return Some(GateResult::Failed {
            kind: FailureKind::TestFailure,
            detail: format!("{} failed; {} passed", &c[1], &c[2]),
            extracted: json!({
                "n_failed": c[1].parse::<u32>().unwrap_or(0),
                "n_passed": c[2].parse::<u32>().unwrap_or(0),
            }),
        });
    }
    None
}

fn first_failed_rust_test(text: &str) -> Option<String> {
    FIRST_FAILED_TEST_RE.captures(text).map(|c| c[1].to_string())
}

fn detect_lint_failure(stdout: &str, stderr: &str, _command: &str) -> Option<GateResult> {
    if stderr.contains("clippy::") || stdout.contains("clippy::") {
        if let Some(c) = CLIPPY_ABORT_RE.captures(stderr).or_else(|| CLIPPY_ABORT_RE.captures(stdout)) {
            return Some(GateResult::Failed {
                kind: FailureKind::LintFailure,
                detail: format!("clippy: aborting due to {} previous error(s)", &c[1]),
                extracted: json!({
                    "tool": "clippy",
                    "n_errors": c[1].parse::<u32>().unwrap_or(0),
                }),
            });
        }
    }
    if let Some(c) = ESLINT_RE.captures(stdout).or_else(|| ESLINT_RE.captures(stderr)) {
        let n_errors: u32 = c[2].parse().unwrap_or(0);
        if n_errors > 0 {
            return Some(GateResult::Failed {
                kind: FailureKind::LintFailure,
                detail: format!("eslint: {} error(s)", n_errors),
                extracted: json!({ "tool": "eslint", "n_errors": n_errors }),
            });
        }
    }
    None
}

fn detect_port_in_use(stderr: &str) -> Option<GateResult> {
    if let Some(c) = EADDRINUSE_RE.captures(stderr) {
        let port: u16 = c[1].parse().unwrap_or(0);
        return Some(GateResult::Failed {
            kind: FailureKind::NetworkBindFailure,
            detail: format!("port {port} already in use"),
            extracted: json!({ "port": port, "address": "127.0.0.1" }),
        });
    }
    if stderr.contains("address already in use") || stderr.contains("bind: address already in use") {
        return Some(GateResult::Failed {
            kind: FailureKind::NetworkBindFailure,
            detail: "address already in use".into(),
            extracted: json!({ "port": null, "address": null }),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(name: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap()
    }

    #[test]
    fn passed_when_exit_zero_and_no_signals() {
        let r = GateClassifier::classify("ok\n", "", 0, "echo ok", false, false, false, 100);
        assert!(matches!(r, GateResult::Passed));
    }

    #[test]
    fn cancelled_short_circuits_classification() {
        let r = GateClassifier::classify("", "", -15, "true", false, true, false, 5000);
        if let GateResult::Failed { kind, extracted, .. } = r {
            assert!(matches!(kind, FailureKind::Cancelled));
            assert_eq!(extracted["elapsed_ms"], 5000);
        } else { panic!("expected Failed") }
    }

    #[test]
    fn rust_compile_error_extracts_file_line_col() {
        let stderr = fix("cargo_compile_error.txt");
        let r = GateClassifier::classify("", &stderr, 101, "cargo build", false, false, false, 1000);
        if let GateResult::Failed { kind, extracted, .. } = r {
            assert!(matches!(kind, FailureKind::CompileError));
            assert_eq!(extracted["file"], "src/lib.rs");
            assert_eq!(extracted["line"], 42);
            assert_eq!(extracted["diagnostic_code"], "E0277");
        } else { panic!("expected Failed") }
    }

    #[test]
    fn rust_test_failure_extracts_test_name_and_counts() {
        let stdout = fix("cargo_test_failed.txt");
        let r = GateClassifier::classify(&stdout, "", 101, "cargo test", false, false, false, 200);
        if let GateResult::Failed { kind, extracted, .. } = r {
            assert!(matches!(kind, FailureKind::TestFailure));
            assert_eq!(extracted["n_failed"], 3);
            assert_eq!(extracted["n_passed"], 1);
            assert!(extracted["test_name"].is_string());
        } else { panic!("expected Failed") }
    }

    #[test]
    fn tsc_compile_error_extracts_file_line() {
        let stdout = fix("tsc_compile_error.txt");
        let r = GateClassifier::classify(&stdout, "", 1, "tsc", false, false, false, 1000);
        if let GateResult::Failed { kind, extracted, .. } = r {
            assert!(matches!(kind, FailureKind::CompileError));
            assert_eq!(extracted["diagnostic_code"], "TS2322");
        } else { panic!("expected Failed") }
    }

    #[test]
    fn vitest_failure_classifies_as_test_failure() {
        let stdout = fix("vitest_failure.txt");
        let r = GateClassifier::classify(&stdout, "", 1, "vitest", false, false, false, 200);
        if let GateResult::Failed { kind, extracted, .. } = r {
            assert!(matches!(kind, FailureKind::TestFailure));
            assert_eq!(extracted["n_failed"], 2);
            assert_eq!(extracted["n_passed"], 1);
        } else { panic!("expected Failed") }
    }

    #[test]
    fn clippy_aborting_classifies_as_lint_failure() {
        let stderr = fix("clippy_aborting.txt");
        let r = GateClassifier::classify("", &stderr, 101, "cargo clippy", false, false, false, 500);
        if let GateResult::Failed { kind, extracted, .. } = r {
            assert!(matches!(kind, FailureKind::LintFailure));
            assert_eq!(extracted["tool"], "clippy");
        } else { panic!("expected Failed") }
    }

    #[test]
    fn eslint_classifies_as_lint_failure() {
        let stdout = fix("eslint_errors.txt");
        let r = GateClassifier::classify(&stdout, "", 1, "eslint .", false, false, false, 200);
        if let GateResult::Failed { kind, extracted, .. } = r {
            assert!(matches!(kind, FailureKind::LintFailure));
            assert_eq!(extracted["tool"], "eslint");
            assert_eq!(extracted["n_errors"], 2);
        } else { panic!("expected Failed") }
    }

    #[test]
    fn eaddrinuse_extracts_port() {
        let stderr = fix("eaddrinuse.txt");
        let r = GateClassifier::classify("", &stderr, 1, "node server.js", false, false, false, 200);
        if let GateResult::Failed { kind, extracted, .. } = r {
            assert!(matches!(kind, FailureKind::NetworkBindFailure));
            assert_eq!(extracted["port"], 3000);
        } else { panic!("expected Failed") }
    }

    #[test]
    fn other_with_signal_hint_for_oom() {
        let r = GateClassifier::classify("", "", 137, "make", false, false, false, 500);
        if let GateResult::Failed { kind, extracted, .. } = r {
            assert!(matches!(kind, FailureKind::Other(_)));
            assert_eq!(extracted["signal_hint"], "SIGKILL (likely OOM)");
        } else { panic!("expected Failed") }
    }
}
