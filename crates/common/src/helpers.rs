//! Shared utility functions.

/// Extract a JSON array substring from a string that may contain prose or markdown.
///
/// Finds the first `[` and last `]` and returns the slice between them (inclusive).
/// Falls back to the full input if no matching pair is found.
pub fn extract_json_array(s: &str) -> &str {
    if let (Some(start), Some(end)) = (s.find('['), s.rfind(']')) {
        if start < end {
            return &s[start..=end];
        }
    }
    s
}

/// Extract the LAST balanced top-level JSON array from a string.
///
/// Reasoning-model output (Mimo, DeepSeek-R1, o1) interleaves chain-of-thought
/// prose with the final structured answer. Stray brackets in prose
/// (e.g. `"[option 1, option 2]"`) break the naive `find('[')..rfind(']')`
/// approach because the slice spans both reasoning and answer, producing
/// invalid JSON. This function scans from the end for `]`, then walks
/// backwards counting brackets to find the matching `[`, returning the
/// last balanced top-level array. Falls back to `None` if none found.
pub fn extract_last_json_array(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let end = bytes.iter().rposition(|&b| b == b']')?;
    let mut depth: i32 = 0;
    let mut i = end;
    loop {
        match bytes[i] {
            b']' => depth += 1,
            b'[' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[i..=end]);
                }
            }
            _ => {}
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

/// Extract a JSON object substring from a string that may contain prose or markdown.
///
/// Finds the first `{` and last `}` and returns the slice between them (inclusive).
/// Returns `None` if no matching pair is found.
pub fn extract_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end > start {
        Some(&s[start..=end])
    } else {
        None
    }
}

/// Strip markdown code fences from LLM output.
///
/// Handles ` ```json ` and ` ``` ` prefixes. Returns a `&str` slice into the
/// original string — no allocation needed.
pub fn strip_llm_fences(s: &str) -> &str {
    let trimmed = s.trim();
    let stripped = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    stripped
        .trim_start_matches('\n')
        .trim_end_matches("```")
        .trim()
}

/// Truncate a `&str` at a UTF-8 char boundary so it fits within `max_bytes`.
///
/// Returns the original slice unchanged if it is already short enough.
pub fn truncate_at_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let end = s
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max_bytes)
        .last()
        .unwrap_or(0);
    &s[..end]
}

/// Truncate to at most `max_chars` Unicode scalar values, appending `suffix` if cut.
pub fn truncate_chars(s: &str, max_chars: usize, suffix: &str) -> String {
    let mut count = 0;
    for (i, _) in s.char_indices() {
        count += 1;
        if count > max_chars {
            return s[..i].to_string() + suffix;
        }
    }
    s.to_string()
}

/// Extract the function name from an OpenAI-style tool definition JSON value.
///
/// Handles the `{"type":"function","function":{"name":"..."}}` format.
/// Returns `None` if the value does not match the expected structure.
pub fn tool_def_name(def: &serde_json::Value) -> Option<&str> {
    def.get("function")
        .and_then(|f| f.get("name"))
        .and_then(|n| n.as_str())
}

/// Cosine similarity between two equal-length f32 vectors.
///
/// Returns 0.0 for empty, mismatched-length, or zero-norm vectors.
/// NaN values are treated as 0.0.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f64;
    let mut norm_a = 0.0f64;
    let mut norm_b = 0.0f64;
    for (va, vb) in a.iter().zip(b.iter()) {
        let fa = if va.is_nan() { 0.0f64 } else { *va as f64 };
        let fb = if vb.is_nan() { 0.0f64 } else { *vb as f64 };
        dot += fa * fb;
        norm_a += fa * fa;
        norm_b += fb * fb;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_last_json_array_skips_reasoning_prose_brackets() {
        // Mimo-style: reasoning emits stray brackets, then final answer.
        let input = r#"First, I think about [option 1, option 2] and decide.
Then I produce the final answer:
["summary one", "summary two"]"#;
        let got = extract_last_json_array(input).unwrap();
        assert_eq!(got, r#"["summary one", "summary two"]"#);
    }

    #[test]
    fn extract_last_json_array_handles_clean_input() {
        let input = r#"["a", "b"]"#;
        assert_eq!(extract_last_json_array(input).unwrap(), input);
    }

    #[test]
    fn extract_last_json_array_returns_none_when_unbalanced() {
        assert!(extract_last_json_array("no array here").is_none());
        assert!(extract_last_json_array("only ] no opener").is_none());
    }

    #[test]
    fn extract_last_json_array_picks_last_when_multiple() {
        let input = r#"first [1, 2] then [3, 4]"#;
        assert_eq!(extract_last_json_array(input).unwrap(), "[3, 4]");
    }

    #[test]
    fn extract_last_json_array_handles_nested() {
        let input = r#"prose [[1, 2], [3, 4]]"#;
        assert_eq!(extract_last_json_array(input).unwrap(), "[[1, 2], [3, 4]]");
    }
}
