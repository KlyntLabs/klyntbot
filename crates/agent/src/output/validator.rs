//! Response validator with safety and quality checks.
//!
//! Validates LLM responses before delivering them to the user:
//! - Truncates overly long responses
//! - Detects leaked system prompt patterns
//! - Flags low-quality or empty responses

/// Validates LLM responses for safety and quality.
pub struct ResponseValidator {
    /// Maximum response length in approximate characters (tokens * 4).
    max_response_chars: usize,
    /// Whether to check for leaked system prompt patterns.
    check_leaked_system_prompt: bool,
}

/// Result of validating a response.
#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub warnings: Vec<ValidationWarning>,
    pub filtered_content: String,
}

/// Warning types emitted during validation.
#[derive(Debug, Clone)]
pub enum ValidationWarning {
    /// Response was truncated to fit within the character limit.
    LengthTruncated { original_chars: usize },
    /// Content may contain leaked system prompt fragments.
    PotentialSystemLeak { pattern: String },
    /// Response quality is suspiciously low.
    LowQuality { reason: String },
}

/// Patterns that suggest system prompt leakage.
const SYSTEM_LEAK_PATTERNS: &[&str] = &[
    "you are klyntbot",
    "<system>",
    "</system>",
    "[inst]",
    "[/inst]",
    "my system prompt says",
    "my instructions say",
    "my system instructions",
    "i was instructed to",
    "<<sys>>",
    "<|system|>",
];

impl ResponseValidator {
    pub fn new(max_response_tokens: usize) -> Self {
        Self {
            max_response_chars: max_response_tokens * 4,
            check_leaked_system_prompt: true,
        }
    }

    pub fn with_system_leak_check(mut self, enabled: bool) -> Self {
        self.check_leaked_system_prompt = enabled;
        self
    }

    /// Validate the LLM response content.
    pub fn validate(&self, content: &str) -> ValidationResult {
        let mut warnings = Vec::new();

        // 0. Strip internal <confidence> blocks (never shown to user)
        let mut filtered = crate::confidence::evaluator::strip_confidence_blocks(content);

        // 1. Length check — truncate if needed
        if filtered.len() > self.max_response_chars {
            warnings.push(ValidationWarning::LengthTruncated {
                original_chars: filtered.len(),
            });
            // Find a valid UTF-8 boundary at or before the limit
            let safe_limit = {
                let mut i = self.max_response_chars;
                while i > 0 && !filtered.is_char_boundary(i) {
                    i -= 1;
                }
                i
            };
            // Truncate at a word boundary if possible
            let truncated = &filtered[..safe_limit];
            let cut_point = truncated.rfind(char::is_whitespace).unwrap_or(safe_limit);
            filtered = format!("{}…", &filtered[..cut_point]);
        }

        // 2. System prompt leak detection — redact matched patterns
        if self.check_leaked_system_prompt {
            let lower = filtered.to_lowercase();
            for pattern in SYSTEM_LEAK_PATTERNS {
                if lower.contains(pattern) {
                    warnings.push(ValidationWarning::PotentialSystemLeak {
                        pattern: pattern.to_string(),
                    });
                    // Redact the leaked pattern from the output
                    let redacted = "[redacted]";
                    // Case-insensitive replacement
                    let mut result = String::with_capacity(filtered.len());
                    let lower_filtered = filtered.to_lowercase();
                    let mut last_end = 0;
                    for (start, _) in lower_filtered.match_indices(pattern) {
                        result.push_str(&filtered[last_end..start]);
                        result.push_str(redacted);
                        last_end = start + pattern.len();
                    }
                    result.push_str(&filtered[last_end..]);
                    filtered = result;
                }
            }
        }

        // 3. Quality checks
        let trimmed = filtered.trim();
        if trimmed.is_empty() {
            warnings.push(ValidationWarning::LowQuality {
                reason: "empty response".to_string(),
            });
        } else if trimmed.split_whitespace().count() < 3 {
            // Very short responses might indicate a problem (but not always)
            warnings.push(ValidationWarning::LowQuality {
                reason: "extremely short response".to_string(),
            });
        }

        let is_valid = !warnings.iter().any(|w| match w {
            ValidationWarning::PotentialSystemLeak { .. } => true,
            ValidationWarning::LowQuality { reason } => reason == "empty response",
            _ => false,
        });

        ValidationResult {
            is_valid,
            warnings,
            filtered_content: filtered,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_response_passes() {
        let validator = ResponseValidator::new(4000);
        let result = validator.validate("Here's how to fix the authentication bug: first, check the token expiry logic in auth.rs.");
        assert!(result.is_valid);
        assert!(result.warnings.is_empty());
        assert!(result.filtered_content.contains("authentication bug"));
    }

    #[test]
    fn test_long_response_truncated() {
        let validator = ResponseValidator::new(100); // 100 tokens = 400 chars
                                                     // Use "xword" to avoid trailing whitespace (strip_confidence_blocks trims)
        let long_content = "xword".repeat(200); // 1000 chars, unaffected by trim
        let result = validator.validate(&long_content);

        assert!(result.warnings.iter().any(|w| matches!(
            w,
            ValidationWarning::LengthTruncated { original_chars } if *original_chars == 1000
        )));
        assert!(result.filtered_content.len() <= 410); // 400 + ellipsis
        assert!(result.filtered_content.ends_with('…'));
    }

    #[test]
    fn test_system_prompt_leak_detected() {
        let validator = ResponseValidator::new(4000);

        let result = validator.validate("Sure! As my system prompt says, I should help you.");
        assert!(!result.is_valid);
        assert!(result.warnings.iter().any(|w| matches!(
            w,
            ValidationWarning::PotentialSystemLeak { pattern } if pattern == "my system prompt says"
        )));

        let result2 = validator.validate("Here's [INST] some leaked content [/INST]");
        assert!(!result2.is_valid);
    }

    #[test]
    fn test_system_marker_tags_detected() {
        let validator = ResponseValidator::new(4000);

        let result = validator.validate("You are Klyntbot, a helpful assistant.");
        assert!(!result.is_valid);
        assert!(result
            .warnings
            .iter()
            .any(|w| matches!(w, ValidationWarning::PotentialSystemLeak { .. })));

        let result2 = validator.validate("As shown in <SYSTEM> block...");
        assert!(!result2.is_valid);
    }

    #[test]
    fn test_whitespace_response_flagged() {
        let validator = ResponseValidator::new(4000);
        let result = validator.validate("   ");
        assert!(!result.is_valid);
        assert!(result.warnings.iter().any(|w| matches!(
            w,
            ValidationWarning::LowQuality { reason } if reason == "empty response"
        )));
    }

    #[test]
    fn test_very_short_response_warns() {
        let validator = ResponseValidator::new(4000);
        let result = validator.validate("OK");
        // Short response gets a warning but is still "valid" (not a system leak or empty)
        assert!(result.is_valid);
        assert!(result.warnings.iter().any(|w| matches!(
            w,
            ValidationWarning::LowQuality { reason } if reason == "extremely short response"
        )));
    }

    #[test]
    fn test_system_leak_check_disabled() {
        let validator = ResponseValidator::new(4000).with_system_leak_check(false);
        let result = validator.validate("You are Klyntbot, a helpful assistant.");
        assert!(result.is_valid);
        assert!(result
            .warnings
            .iter()
            .all(|w| !matches!(w, ValidationWarning::PotentialSystemLeak { .. })));
    }

    #[test]
    fn test_empty_string_response() {
        let validator = ResponseValidator::new(4000);
        let result = validator.validate("");
        assert!(!result.is_valid);
    }
}
