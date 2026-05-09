//! Cross-run diff for completed bash jobs.
//!
//! `diff_against_prior(prior, curr)` is pure: takes two `BashJobRow`s with
//! matching `command_key` and returns a `JobDiff` describing the transition.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use storage::repos::BashJobRow;
use tools_core::FailureKind;

#[derive(Debug, Clone, PartialEq)]
pub struct JobDiff {
    pub kind_transition: KindTransition,
    pub extracted_diff: ExtractedDiff,
    pub elapsed_delta_ms: i64, // signed: negative = faster than prior
}

#[derive(Debug, Clone, PartialEq)]
pub enum KindTransition {
    StillPassing,
    StillFailing { kind: String }, // kind name (FailureKind variant)
    Regressed { from: String, to: String }, // None or different failure
    Recovered { prior_kind: String },
    Changed { from: String, to: String }, // both failed, different kinds
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtractedDiff {
    None,
    TestSet {
        new_failures: Vec<String>,
        still_failing: Vec<String>,
        resolved: Vec<String>,
    },
    Compile {
        same_location: bool,
        prior_loc: Option<Location>,
        curr_loc: Option<Location>,
    },
    Bind {
        same_port: bool,
        prior_port: Option<u64>,
        curr_port: Option<u64>,
    },
    Lint {
        delta_n_errors: i64,
    },
    Timeout {
        prior_ms: u64,
        curr_ms: u64,
    },
    OtherExitTransition {
        from: Option<i32>,
        to: Option<i32>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub line: u64,
}

pub fn diff_against_prior(prior: &BashJobRow, curr: &BashJobRow) -> JobDiff {
    let kind_transition = classify_transition(prior, curr);
    let extracted_diff = match (
        parse_extracted(prior.failure_extracted.as_deref()),
        parse_extracted(curr.failure_extracted.as_deref()),
    ) {
        (Some(p), Some(c)) => diff_extracted(
            &p,
            &c,
            prior.failure_kind.as_deref(),
            curr.failure_kind.as_deref(),
        ),
        _ => ExtractedDiff::None,
    };
    let elapsed_delta_ms = elapsed_ms(curr) as i64 - elapsed_ms(prior) as i64;

    JobDiff {
        kind_transition,
        extracted_diff,
        elapsed_delta_ms,
    }
}

fn elapsed_ms(row: &BashJobRow) -> u64 {
    match row.finished_at {
        Some(end) => {
            let start_ms = row.started_at.as_millisecond() as i128;
            let end_ms = end.as_millisecond() as i128;
            (end_ms - start_ms).max(0) as u64
        }
        None => 0,
    }
}

fn parse_extracted(s: Option<&str>) -> Option<serde_json::Value> {
    s.and_then(|s| serde_json::from_str(s).ok())
}

fn classify_transition(prior: &BashJobRow, curr: &BashJobRow) -> KindTransition {
    let prior_failed = prior.failure_kind.is_some();
    let curr_failed = curr.failure_kind.is_some();
    match (prior_failed, curr_failed) {
        (false, false) => KindTransition::StillPassing,
        (true, false) => KindTransition::Recovered {
            prior_kind: prior.failure_kind.clone().unwrap_or_default(),
        },
        (false, true) => KindTransition::Regressed {
            from: "Passed".to_string(),
            to: curr.failure_kind.clone().unwrap_or_default(),
        },
        (true, true) => {
            let p = prior.failure_kind.clone().unwrap_or_default();
            let c = curr.failure_kind.clone().unwrap_or_default();
            if p == c {
                KindTransition::StillFailing { kind: p }
            } else {
                KindTransition::Changed { from: p, to: c }
            }
        }
    }
}

fn diff_extracted(
    prior: &serde_json::Value,
    curr: &serde_json::Value,
    prior_kind: Option<&str>,
    curr_kind: Option<&str>,
) -> ExtractedDiff {
    use ExtractedDiff::*;

    if prior_kind == Some("TestFailure") && curr_kind == Some("TestFailure") {
        let p_set = string_array_set(prior, "failed_test_names");
        let c_set = string_array_set(curr, "failed_test_names");
        let new_failures: Vec<String> = c_set.difference(&p_set).cloned().collect();
        let still_failing: Vec<String> = c_set.intersection(&p_set).cloned().collect();
        let resolved: Vec<String> = p_set.difference(&c_set).cloned().collect();
        return TestSet {
            new_failures,
            still_failing,
            resolved,
        };
    }

    if prior_kind == Some("CompileError") && curr_kind == Some("CompileError") {
        let pl = location_from(prior);
        let cl = location_from(curr);
        return Compile {
            same_location: pl.is_some() && pl == cl,
            prior_loc: pl,
            curr_loc: cl,
        };
    }

    if prior_kind == Some("NetworkBindFailure") && curr_kind == Some("NetworkBindFailure") {
        let pp = prior.get("port").and_then(|v| v.as_u64());
        let cp = curr.get("port").and_then(|v| v.as_u64());
        return Bind {
            same_port: pp.is_some() && pp == cp,
            prior_port: pp,
            curr_port: cp,
        };
    }

    if prior_kind == Some("LintFailure") && curr_kind == Some("LintFailure") {
        let pe = prior.get("n_errors").and_then(|v| v.as_i64()).unwrap_or(0);
        let ce = curr.get("n_errors").and_then(|v| v.as_i64()).unwrap_or(0);
        return Lint {
            delta_n_errors: ce - pe,
        };
    }

    if prior_kind == Some("Timeout") && curr_kind == Some("Timeout") {
        let pm = prior
            .get("elapsed_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cm = curr.get("elapsed_ms").and_then(|v| v.as_u64()).unwrap_or(0);
        return Timeout {
            prior_ms: pm,
            curr_ms: cm,
        };
    }

    if prior_kind.is_none() && curr_kind.is_none() {
        return None;
    }

    OtherExitTransition {
        from: prior
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        to: curr
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
    }
}

fn string_array_set(v: &serde_json::Value, key: &str) -> BTreeSet<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|el| el.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn location_from(v: &serde_json::Value) -> Option<Location> {
    let file = v.get("file").and_then(|x| x.as_str())?.to_string();
    let line = v.get("line").and_then(|x| x.as_u64())?;
    Some(Location { file, line })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Timestamp;

    fn row(
        kind: Option<&str>,
        extracted: Option<serde_json::Value>,
        started_ms: i64,
        finished_ms: i64,
    ) -> BashJobRow {
        let started_at = Timestamp::from_millisecond(started_ms).unwrap();
        BashJobRow {
            id: "x".into(),
            session_id: "s".into(),
            agent_id: "a".into(),
            description: "d".into(),
            command: "c".into(),
            command_key: "k".into(),
            cwd: "/".into(),
            timeout_ms: 60_000,
            silent_completion: false,
            status: if kind.is_some() {
                "Failed"
            } else {
                "Completed"
            }
            .into(),
            exit_code: Some(if kind.is_some() { 1 } else { 0 }),
            failure_kind: kind.map(String::from),
            failure_detail: None,
            failure_extracted: extracted.map(|v| v.to_string()),
            started_at,
            finished_at: Some(Timestamp::from_millisecond(finished_ms).unwrap()),
            total_bytes_emitted: 0,
            bisect_count: 0,
            log_path: "/tmp/x.log".into(),
            final_path: None,
            last_polled_at: None,
            last_seen_offset: 0,
        }
    }

    #[test]
    fn still_passing() {
        let p = row(None, None, 0, 1000);
        let c = row(None, None, 0, 1500);
        let d = diff_against_prior(&p, &c);
        assert_eq!(d.kind_transition, KindTransition::StillPassing);
        assert_eq!(d.extracted_diff, ExtractedDiff::None);
        assert_eq!(d.elapsed_delta_ms, 500);
    }

    #[test]
    fn recovered() {
        let p = row(
            Some("TestFailure"),
            Some(serde_json::json!({"failed_test_names":["a"]})),
            0,
            1000,
        );
        let c = row(None, None, 0, 800);
        let d = diff_against_prior(&p, &c);
        assert_eq!(
            d.kind_transition,
            KindTransition::Recovered {
                prior_kind: "TestFailure".into()
            }
        );
    }

    #[test]
    fn regressed_from_pass() {
        let p = row(None, None, 0, 1000);
        let c = row(
            Some("CompileError"),
            Some(serde_json::json!({"file":"src/lib.rs","line":42})),
            0,
            1500,
        );
        let d = diff_against_prior(&p, &c);
        assert_eq!(
            d.kind_transition,
            KindTransition::Regressed {
                from: "Passed".into(),
                to: "CompileError".into()
            }
        );
    }

    #[test]
    fn still_failing_same_kind() {
        let p = row(
            Some("TestFailure"),
            Some(serde_json::json!({"failed_test_names":["a"]})),
            0,
            1000,
        );
        let c = row(
            Some("TestFailure"),
            Some(serde_json::json!({"failed_test_names":["a"]})),
            0,
            1100,
        );
        let d = diff_against_prior(&p, &c);
        assert_eq!(
            d.kind_transition,
            KindTransition::StillFailing {
                kind: "TestFailure".into()
            }
        );
    }

    #[test]
    fn changed_kind() {
        let p = row(
            Some("TestFailure"),
            Some(serde_json::json!({"failed_test_names":["a"]})),
            0,
            1000,
        );
        let c = row(
            Some("CompileError"),
            Some(serde_json::json!({"file":"x","line":1})),
            0,
            1100,
        );
        let d = diff_against_prior(&p, &c);
        assert_eq!(
            d.kind_transition,
            KindTransition::Changed {
                from: "TestFailure".into(),
                to: "CompileError".into()
            }
        );
    }

    #[test]
    fn test_set_diff_overlapping() {
        let p = row(
            Some("TestFailure"),
            Some(serde_json::json!({"failed_test_names":["a","b","c"]})),
            0,
            1000,
        );
        let c = row(
            Some("TestFailure"),
            Some(serde_json::json!({"failed_test_names":["b","c","d"]})),
            0,
            1000,
        );
        let d = diff_against_prior(&p, &c);
        match d.extracted_diff {
            ExtractedDiff::TestSet {
                new_failures,
                still_failing,
                resolved,
            } => {
                assert_eq!(new_failures, vec!["d".to_string()]);
                let mut still = still_failing.clone();
                still.sort();
                assert_eq!(still, vec!["b".to_string(), "c".to_string()]);
                assert_eq!(resolved, vec!["a".to_string()]);
            }
            other => panic!("expected TestSet, got {other:?}"),
        }
    }

    #[test]
    fn test_set_diff_disjoint() {
        let p = row(
            Some("TestFailure"),
            Some(serde_json::json!({"failed_test_names":["a"]})),
            0,
            1000,
        );
        let c = row(
            Some("TestFailure"),
            Some(serde_json::json!({"failed_test_names":["b"]})),
            0,
            1000,
        );
        let d = diff_against_prior(&p, &c);
        match d.extracted_diff {
            ExtractedDiff::TestSet {
                new_failures,
                still_failing,
                resolved,
            } => {
                assert_eq!(new_failures, vec!["b".to_string()]);
                assert!(still_failing.is_empty());
                assert_eq!(resolved, vec!["a".to_string()]);
            }
            _ => panic!("expected TestSet"),
        }
    }

    #[test]
    fn compile_same_location() {
        let p = row(
            Some("CompileError"),
            Some(serde_json::json!({"file":"a.rs","line":10})),
            0,
            1000,
        );
        let c = row(
            Some("CompileError"),
            Some(serde_json::json!({"file":"a.rs","line":10})),
            0,
            1100,
        );
        let d = diff_against_prior(&p, &c);
        assert!(matches!(
            d.extracted_diff,
            ExtractedDiff::Compile {
                same_location: true,
                ..
            }
        ));
    }

    #[test]
    fn compile_different_location() {
        let p = row(
            Some("CompileError"),
            Some(serde_json::json!({"file":"a.rs","line":10})),
            0,
            1000,
        );
        let c = row(
            Some("CompileError"),
            Some(serde_json::json!({"file":"a.rs","line":11})),
            0,
            1100,
        );
        let d = diff_against_prior(&p, &c);
        assert!(matches!(
            d.extracted_diff,
            ExtractedDiff::Compile {
                same_location: false,
                ..
            }
        ));
    }

    #[test]
    fn bind_port_diff() {
        let p = row(
            Some("NetworkBindFailure"),
            Some(serde_json::json!({"port":3000})),
            0,
            1000,
        );
        let c = row(
            Some("NetworkBindFailure"),
            Some(serde_json::json!({"port":3000})),
            0,
            1000,
        );
        assert!(matches!(
            diff_against_prior(&p, &c).extracted_diff,
            ExtractedDiff::Bind {
                same_port: true,
                ..
            }
        ));
    }

    #[test]
    fn lint_delta() {
        let p = row(
            Some("LintFailure"),
            Some(serde_json::json!({"n_errors":5})),
            0,
            1000,
        );
        let c = row(
            Some("LintFailure"),
            Some(serde_json::json!({"n_errors":3})),
            0,
            1000,
        );
        assert_eq!(
            diff_against_prior(&p, &c).extracted_diff,
            ExtractedDiff::Lint { delta_n_errors: -2 },
        );
    }

    #[test]
    fn timeout_delta() {
        let p = row(
            Some("Timeout"),
            Some(serde_json::json!({"elapsed_ms":600_000})),
            0,
            1000,
        );
        let c = row(
            Some("Timeout"),
            Some(serde_json::json!({"elapsed_ms":700_000})),
            0,
            1000,
        );
        assert_eq!(
            diff_against_prior(&p, &c).extracted_diff,
            ExtractedDiff::Timeout {
                prior_ms: 600_000,
                curr_ms: 700_000
            },
        );
    }

    #[test]
    fn malformed_extracted_falls_back_to_none() {
        let mut p = row(Some("Other"), None, 0, 1000);
        p.failure_extracted = Some("not-valid-json".into());
        let c = row(Some("Other"), None, 0, 1000);
        let d = diff_against_prior(&p, &c);
        assert_eq!(d.extracted_diff, ExtractedDiff::None);
    }

    #[test]
    fn elapsed_delta_signed_negative_is_faster() {
        let p = row(None, None, 0, 1000);
        let c = row(None, None, 0, 600);
        assert_eq!(diff_against_prior(&p, &c).elapsed_delta_ms, -400);
    }
}
