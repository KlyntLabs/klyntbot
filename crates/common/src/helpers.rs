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
