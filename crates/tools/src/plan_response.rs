//! Daily plan response parsing.
//!
//! Parses user responses to daily planning notifications:
//! - "yes" / "y" / "ok" / "go" → Accept
//! - "swap 1 and 2" / "swap 1,2" → Swap { pos_a, pos_b }
//! - "skip 2" / "skip #2" → Skip { pos }
//! - "defer" / "defer all" / "not today" → DeferAll

use regex::Regex;
use serde::{Deserialize, Serialize};

/// User action in response to a daily plan
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanAction {
    /// Accept all suggested tasks
    Accept,
    /// Swap two tasks by position (1-indexed)
    Swap { pos_a: usize, pos_b: usize },
    /// Skip a task by position (1-indexed), promote next
    Skip { pos: usize },
    /// Defer/dismiss the entire plan
    DeferAll,
}

/// Parse a user response string into a PlanAction.
///
/// # Arguments
/// - `input`: Raw user input (trimmed, case-insensitive)
/// - `plan_size`: Number of tasks in the plan (for validation)
///
/// # Errors
/// Returns an error message if input is unrecognized or invalid.
pub fn parse_plan_response(input: &str, plan_size: usize) -> Result<PlanAction, String> {
    // Guard against unbounded input (DoS protection for regex)
    if input.len() > 100 {
        return Err("Response too long (max 100 characters)".to_string());
    }

    let trimmed = input.trim().to_lowercase();

    // Accept patterns
    let accept_patterns = [
        "yes",
        "y",
        "ok",
        "go",
        "looks good",
        "let's go",
        "confirm",
        "accept",
    ];

    if accept_patterns.contains(&trimmed.as_str()) {
        return Ok(PlanAction::Accept);
    }

    // Defer patterns
    let defer_patterns = [
        "defer",
        "defer all",
        "not today",
        "pass",
        "dismiss",
        "nah",
        "no",
    ];

    if defer_patterns.contains(&trimmed.as_str()) {
        return Ok(PlanAction::DeferAll);
    }

    // Skip pattern: "skip 2", "skip #2", "remove 1", "drop 3"
    let skip_re = Regex::new(r"^(skip|remove|drop)\s+#?(\d+)$").unwrap();
    if let Some(caps) = skip_re.captures(&trimmed) {
        let pos = caps[2].parse::<usize>().unwrap();

        // Validate position is within range (1-indexed)
        if pos < 1 || pos > plan_size {
            return Err(format!(
                "Position {} is out of range. There are only {} tasks in your plan.",
                pos, plan_size
            ));
        }

        return Ok(PlanAction::Skip { pos });
    }

    // Swap pattern: "swap 1 and 2", "swap 1,2", "swap 1 2", "reorder 1 and 3"
    let swap_re = Regex::new(r"^(swap|reorder)\s+(\d+)\s*(and|,|\s)\s*(\d+)$").unwrap();
    if let Some(caps) = swap_re.captures(&trimmed) {
        let pos_a = caps[2].parse::<usize>().unwrap();
        let pos_b = caps[4].parse::<usize>().unwrap();

        // Validate positions are different
        if pos_a == pos_b {
            return Err("Cannot swap the same position. Try swapping different tasks.".to_string());
        }

        // Validate positions are within range (1-indexed)
        if pos_a < 1 || pos_a > plan_size || pos_b < 1 || pos_b > plan_size {
            return Err(format!(
                "Position out of range. There are only {} tasks in your plan.",
                plan_size
            ));
        }

        return Ok(PlanAction::Swap { pos_a, pos_b });
    }

    Err(format!("Unrecognized response: '{}'", input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_accept_yes() {
        let action = parse_plan_response("yes", 3).unwrap();
        assert!(matches!(action, PlanAction::Accept));
    }

    #[test]
    fn test_parse_accept_variants() {
        let variants = [
            "y",
            "ok",
            "go",
            "looks good",
            "let's go",
            "confirm",
            "accept",
        ];
        for input in &variants {
            let action = parse_plan_response(input, 3).unwrap();
            assert!(
                matches!(action, PlanAction::Accept),
                "Failed for: {}",
                input
            );
        }
    }

    #[test]
    fn test_parse_accept_case_insensitive() {
        let variants = ["YES", "Y", "Ok", "GO", "LOOKS GOOD"];
        for input in &variants {
            let action = parse_plan_response(input, 3).unwrap();
            assert!(
                matches!(action, PlanAction::Accept),
                "Failed for: {}",
                input
            );
        }
    }

    #[test]
    fn test_parse_accept_with_whitespace() {
        let variants = ["  yes  ", " ok ", "\ty\t"];
        for input in &variants {
            let action = parse_plan_response(input, 3).unwrap();
            assert!(
                matches!(action, PlanAction::Accept),
                "Failed for: {}",
                input
            );
        }
    }

    #[test]
    fn test_parse_defer_variants() {
        let variants = [
            "defer",
            "defer all",
            "not today",
            "pass",
            "dismiss",
            "nah",
            "no",
        ];
        for input in &variants {
            let action = parse_plan_response(input, 3).unwrap();
            assert!(
                matches!(action, PlanAction::DeferAll),
                "Failed for: {}",
                input
            );
        }
    }

    #[test]
    fn test_parse_defer_case_insensitive() {
        let variants = ["DEFER", "Defer All", "NOT TODAY", "Pass"];
        for input in &variants {
            let action = parse_plan_response(input, 3).unwrap();
            assert!(
                matches!(action, PlanAction::DeferAll),
                "Failed for: {}",
                input
            );
        }
    }

    #[test]
    fn test_parse_skip_basic() {
        let action = parse_plan_response("skip 2", 3).unwrap();
        assert!(matches!(action, PlanAction::Skip { pos: 2 }));
    }

    #[test]
    fn test_parse_skip_variants() {
        let test_cases = [
            ("skip 1", 1),
            ("skip #2", 2),
            ("remove 3", 3),
            ("drop 1", 1),
        ];

        for (input, expected_pos) in &test_cases {
            let action = parse_plan_response(input, 3).unwrap();
            match action {
                PlanAction::Skip { pos } => {
                    assert_eq!(pos, *expected_pos, "Failed for: {}", input)
                }
                _ => panic!("Expected Skip for: {}", input),
            }
        }
    }

    #[test]
    fn test_parse_skip_case_insensitive() {
        let action = parse_plan_response("SKIP 2", 3).unwrap();
        assert!(matches!(action, PlanAction::Skip { pos: 2 }));
    }

    #[test]
    fn test_parse_swap_basic() {
        let action = parse_plan_response("swap 1 and 2", 3).unwrap();
        assert!(matches!(action, PlanAction::Swap { pos_a: 1, pos_b: 2 }));
    }

    #[test]
    fn test_parse_swap_variants() {
        let test_cases = [
            ("swap 1 and 2", 1, 2),
            ("swap 1,2", 1, 2),
            ("swap 1 2", 1, 2),
            ("reorder 1 and 3", 1, 3),
            ("swap 2,3", 2, 3),
        ];

        for (input, expected_a, expected_b) in &test_cases {
            let action = parse_plan_response(input, 3).unwrap();
            match action {
                PlanAction::Swap { pos_a, pos_b } => {
                    assert_eq!(pos_a, *expected_a, "Failed pos_a for: {}", input);
                    assert_eq!(pos_b, *expected_b, "Failed pos_b for: {}", input);
                }
                _ => panic!("Expected Swap for: {}", input),
            }
        }
    }

    #[test]
    fn test_parse_swap_case_insensitive() {
        let action = parse_plan_response("SWAP 1 AND 2", 3).unwrap();
        assert!(matches!(action, PlanAction::Swap { pos_a: 1, pos_b: 2 }));
    }

    // ─── Validation Error Tests ───────────────────────────────────

    #[test]
    fn test_skip_out_of_range() {
        let result = parse_plan_response("skip 5", 3);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("only 3 tasks"));
    }

    #[test]
    fn test_swap_same_position() {
        let result = parse_plan_response("swap 1 and 1", 3);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("same position"));
    }

    #[test]
    fn test_swap_out_of_range() {
        let result = parse_plan_response("swap 1 and 5", 3);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("only 3 tasks"));
    }

    #[test]
    fn test_input_too_long() {
        let long_input = "a".repeat(101); // 101 chars, exceeds 100 limit
        let result = parse_plan_response(&long_input, 3);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("too long"));
        assert!(err.contains("100"));
    }

    #[test]
    fn test_unrecognized_input() {
        let result = parse_plan_response("hello world", 3);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Unrecognized"));
    }
}
