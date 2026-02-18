//! Behavioral signal detection — keyword-based sentiment from user messages.

/// Detected behavioral signal from user message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehavioralSignal {
    Positive,
    Negative,
    Neutral,
}

const POSITIVE_KEYWORDS: &[&str] = &[
    "thanks",
    "thank you",
    "perfect",
    "exactly",
    "great",
    "awesome",
    "nice",
    "well done",
    "good job",
    "that works",
    "correct",
    "yes",
];

const NEGATIVE_KEYWORDS: &[&str] = &[
    "no",
    "wrong",
    "incorrect",
    "that's not",
    "try again",
    "not what i",
    "bad",
    "fail",
    "broken",
    "doesn't work",
    "didn't work",
    "stop",
    "undo",
    "revert",
];

/// Detect behavioral signal from a user message using word-boundary-aware matching.
pub fn detect_signal(message: &str) -> BehavioralSignal {
    let msg = message.trim().to_lowercase();

    // Check negative first (more actionable)
    if NEGATIVE_KEYWORDS
        .iter()
        .any(|k| contains_word_boundary(&msg, k))
    {
        return BehavioralSignal::Negative;
    }

    if POSITIVE_KEYWORDS
        .iter()
        .any(|k| contains_word_boundary(&msg, k))
    {
        return BehavioralSignal::Positive;
    }

    BehavioralSignal::Neutral
}

/// Check if `text` contains `keyword` at a word boundary.
/// Prevents false positives like "I know" matching "no" or "bypass" matching "yes".
fn contains_word_boundary(text: &str, keyword: &str) -> bool {
    for (start, _) in text.match_indices(keyword) {
        let end = start + keyword.len();
        let before_ok = start == 0 || !text.as_bytes()[start - 1].is_ascii_alphanumeric();
        let after_ok = end >= text.len() || !text.as_bytes()[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive_signals() {
        assert_eq!(detect_signal("thanks!"), BehavioralSignal::Positive);
        assert_eq!(detect_signal("That's perfect"), BehavioralSignal::Positive);
        assert_eq!(
            detect_signal("Yes, exactly what I needed"),
            BehavioralSignal::Positive
        );
    }

    #[test]
    fn test_negative_signals() {
        assert_eq!(
            detect_signal("No, that's wrong"),
            BehavioralSignal::Negative
        );
        assert_eq!(
            detect_signal("Try again please"),
            BehavioralSignal::Negative
        );
        assert_eq!(
            detect_signal("That doesn't work"),
            BehavioralSignal::Negative
        );
    }

    #[test]
    fn test_neutral_signals() {
        assert_eq!(detect_signal("What is Rust?"), BehavioralSignal::Neutral);
        assert_eq!(detect_signal("Show me the code"), BehavioralSignal::Neutral);
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(detect_signal("THANKS"), BehavioralSignal::Positive);
        assert_eq!(detect_signal("WRONG"), BehavioralSignal::Negative);
    }

    #[test]
    fn test_negative_takes_precedence() {
        // "no thanks" has both positive and negative keywords
        assert_eq!(detect_signal("no thanks"), BehavioralSignal::Negative);
    }
}
