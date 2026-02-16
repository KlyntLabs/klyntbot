//! Sprint 5 CLI integration tests — Semantic Search flags
//!
//! Tests the CLI-level integration for the new `--semantic`, `--hybrid`,
//! `--threshold`, and `--limit` flags on `klyntbot todo search`.
//!
//! These tests invoke the binary and verify stdout/stderr output format.

use std::process::Command;

/// Helper: Get the path to the klyntbot binary (debug build).
fn klyntbot_bin() -> String {
    // In tests, cargo puts the binary in target/debug/
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // remove test binary name
    path.pop(); // remove deps/
    path.push("klyntbot");
    path.to_str().unwrap().to_string()
}

/// Helper: Run klyntbot with args and return (stdout, stderr, exit_code).
#[allow(dead_code)]
fn run_klyntbot(args: &[&str]) -> (String, String, i32) {
    let output = Command::new(klyntbot_bin())
        .args(args)
        .env("NO_COLOR", "1") // Disable ANSI for easier assertions
        .output()
        .expect("Failed to execute klyntbot");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

// ═══════════════════════════════════════════════════════════════
// Flag Parsing Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_semantic_flag_accepted() {
    // The `--semantic` / `-s` flag should be recognized by the CLI parser
    //
    // TODO: Implement once CLI flags are added
    //
    // let (stdout, stderr, code) = run_klyntbot(&["todo", "search", "--semantic", "auth"]);
    // // Should not fail with "unknown flag" error
    // assert!(!stderr.contains("unexpected argument"), "CLI should accept --semantic flag");
}

#[test]
fn test_cli_semantic_short_flag() {
    // The `-s` short flag should work as alias for `--semantic`
    //
    // TODO: Implement once CLI flags are added
    //
    // let (stdout, stderr, code) = run_klyntbot(&["todo", "search", "-s", "auth"]);
    // assert!(!stderr.contains("unexpected argument"), "CLI should accept -s flag");
}

#[test]
fn test_cli_hybrid_flag_accepted() {
    // The `--hybrid` flag should be recognized
    //
    // TODO: Implement once CLI flags are added
    //
    // let (stdout, stderr, code) = run_klyntbot(&["todo", "search", "--hybrid", "login"]);
    // assert!(!stderr.contains("unexpected argument"), "CLI should accept --hybrid flag");
}

#[test]
fn test_cli_semantic_and_hybrid_mutually_exclusive() {
    // `--semantic` and `--hybrid` cannot be used together (conflicts_with)
    //
    // TODO: Implement once CLI flags are added
    //
    // let (stdout, stderr, code) = run_klyntbot(&["todo", "search", "--semantic", "--hybrid", "auth"]);
    // assert_ne!(code, 0, "Should fail when both flags are used");
    // assert!(stderr.contains("cannot be used with"), "Should explain the conflict");
}

#[test]
fn test_cli_threshold_flag() {
    // `--threshold` / `-t` should accept a float value
    //
    // TODO: Implement once CLI flags are added
    //
    // let (stdout, stderr, code) = run_klyntbot(&["todo", "search", "-s", "auth", "--threshold", "0.7"]);
    // assert!(!stderr.contains("unexpected argument"));
}

#[test]
fn test_cli_limit_flag() {
    // `--limit` / `-l` should accept an integer value
    //
    // TODO: Implement once CLI flags are added
    //
    // let (stdout, stderr, code) = run_klyntbot(&["todo", "search", "-s", "auth", "--limit", "5"]);
    // assert!(!stderr.contains("unexpected argument"));
}

// ═══════════════════════════════════════════════════════════════
// Output Format Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_semantic_output_shows_similarity_scores() {
    // Semantic search output should include `sim: 0.XX` for each result
    //
    // TODO: Implement once semantic search output formatting is done
    //
    // let (stdout, _stderr, _code) = run_klyntbot(&["todo", "search", "--semantic", "auth"]);
    // // Each result line should contain a similarity score
    // for line in stdout.lines().filter(|l| l.contains("P")) {
    //     assert!(line.contains("sim:"), "Result line should show similarity: {}", line);
    // }
}

#[test]
fn test_cli_hybrid_output_shows_match_sources() {
    // Hybrid search output should indicate match source (keyword/semantic/both)
    //
    // TODO: Implement once hybrid search output formatting is done
    //
    // let (stdout, _stderr, _code) = run_klyntbot(&["todo", "search", "--hybrid", "login"]);
    // // Should contain at least one of the source indicators
    // assert!(
    //     stdout.contains("keyword") || stdout.contains("semantic"),
    //     "Hybrid output should show match sources"
    // );
}

#[test]
fn test_cli_semantic_header_shows_mode_and_threshold() {
    // Semantic search header: "N task(s) matching 'query' (semantic, threshold: 0.5):"
    //
    // TODO: Implement once output formatting is done
    //
    // let (stdout, _stderr, _code) = run_klyntbot(&["todo", "search", "-s", "auth"]);
    // assert!(stdout.contains("semantic"), "Header should mention search mode");
    // assert!(stdout.contains("threshold"), "Header should show threshold");
}

#[test]
fn test_cli_hybrid_header_shows_mode() {
    // Hybrid search header: "N task(s) matching 'query' (hybrid: keyword + semantic):"
    //
    // TODO: Implement once output formatting is done
    //
    // let (stdout, _stderr, _code) = run_klyntbot(&["todo", "search", "--hybrid", "login"]);
    // assert!(stdout.contains("hybrid"), "Header should mention hybrid mode");
}

// ═══════════════════════════════════════════════════════════════
// Error State Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_semantic_empty_query() {
    // Empty query with --semantic → "Search query cannot be empty"
    //
    // TODO: Implement once validation is added
    //
    // let (_stdout, stderr, code) = run_klyntbot(&["todo", "search", "-s", ""]);
    // assert_ne!(code, 0);
    // assert!(stderr.contains("cannot be empty") || stderr.contains("required"));
}

#[test]
fn test_cli_invalid_threshold_value() {
    // Invalid threshold (e.g., 1.5) → error message
    //
    // TODO: Implement once validation is added
    //
    // let (_stdout, stderr, code) = run_klyntbot(&["todo", "search", "-s", "auth", "-t", "1.5"]);
    // assert_ne!(code, 0);
    // assert!(stderr.contains("0.0") && stderr.contains("1.0"),
    //     "Should explain valid threshold range");
}

#[test]
fn test_cli_invalid_limit_value() {
    // Invalid limit (e.g., 0 or 101) → error message
    //
    // TODO: Implement once validation is added
    //
    // let (_stdout, stderr, code) = run_klyntbot(&["todo", "search", "-s", "auth", "-l", "0"]);
    // assert_ne!(code, 0);
}

// ═══════════════════════════════════════════════════════════════
// Help Text Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_help_includes_semantic_examples() {
    // `klyntbot todo search --help` should include --semantic examples
    //
    // TODO: Implement once CLI flags are added
    //
    // let (stdout, _stderr, _code) = run_klyntbot(&["todo", "search", "--help"]);
    // assert!(stdout.contains("--semantic"), "Help should mention --semantic");
    // assert!(stdout.contains("--hybrid"), "Help should mention --hybrid");
    // assert!(stdout.contains("--threshold"), "Help should mention --threshold");
    // assert!(stdout.contains("--limit"), "Help should mention --limit");
    // assert!(stdout.contains("Examples"), "Help should include examples section");
}

// ═══════════════════════════════════════════════════════════════
// Graceful Degradation Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_semantic_no_model_shows_actionable_error() {
    // When model is not available, --semantic should show helpful error
    //
    // TODO: Implement once graceful degradation is wired
    //
    // let (stdout, stderr, code) = run_klyntbot(&["todo", "search", "-s", "auth"]);
    // // Should suggest using keyword search instead
    // let combined = format!("{}{}", stdout, stderr);
    // assert!(combined.contains("unavailable") || combined.contains("keyword search"));
}

#[test]
fn test_cli_keyword_search_unaffected_by_missing_model() {
    // When model is not available, plain `todo search` should work as before
    //
    // TODO: Implement once embedding handler is optional
    //
    // let (stdout, stderr, code) = run_klyntbot(&["todo", "search", "auth"]);
    // // Should NOT contain any error about embeddings
    // assert!(!stderr.contains("embedding"), "Keyword search should not mention embeddings");
}

// ═══════════════════════════════════════════════════════════════
// Partial Embedding Coverage Test
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_semantic_partial_coverage_note() {
    // When some tasks lack embeddings, output should note it (per UX spec §3.4)
    //
    // TODO: Implement once partial coverage detection is done
    //
    // Output should contain: "Note: X of Y tasks have no embeddings"
}
