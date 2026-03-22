/// Build system prompt for smart segmentation of a note into translation practice units.
pub fn segmentation_prompt(source_lang: &str, target_lang: &str) -> String {
    format!(
        r#"You are Klyntbot's patient language coach preparing a deliberate practice session.

Split the following {source_lang} note into the smallest meaningful translation units for practicing {target_lang}.
Rules (never break them):
- One full sentence = one unit
- Headings and titles = one unit
- Grammar patterns + their examples stay together as one unit
- Cultural explanations stay together as one unit
- Skip any line that is already in the target language or purely romanized
- Prioritise units that build on each other for natural flow

Return ONLY this exact JSON (no extra text):
[{{
  "index": 0,
  "text": "exact original text",
  "type": "heading | sentence | pattern | cultural",
  "suggested_focus": "vocabulary | grammar | naturalness | cultural"
}}]"#
    )
}

/// Build system prompt for evaluating a user's sentence-by-sentence translation attempt.
pub fn evaluation_prompt(source_lang: &str, target_lang: &str) -> String {
    format!(
        r#"You are Klyntbot's patient, encouraging language coach — supportive but honest,
like a kind tutor who celebrates progress.

The user is translating a {source_lang} note into {target_lang}, sentence by sentence.

Evaluate their translation with document-level awareness.
Be specific: explain WHY something is better, not just what.

Return ONLY this exact JSON:
{{
  "overall_grade": "A+ | A | A- | B+ | B | B- | C+ | C | C- | D+ | D | F",
  "scores": {{
    "meaning": "grade",
    "grammar": "grade",
    "naturalness": "grade",
    "wordChoice": "grade"
  }},
  "corrections": [
    {{
      "original": "exact phrase from user",
      "suggested": "better version",
      "explanation": "why this is more natural or correct in this context"
    }}
  ],
  "model_translation": "polished version of the sentence",
  "encouragement": "warm, short, personal comment referencing progress (max 15 words)",
  "improvement_hint": "one micro-tip for next time (optional, null if grade >= A)"
}}"#
    )
}

/// Check the last 3 results for a repeated weakness on the same dimension (B+ or lower).
///
/// Returns a coaching nudge message if any single score dimension appears at B+ or worse
/// across all of the last 3 results, or `None` otherwise.
pub fn coaching_nudge_check(results: &[serde_json::Value]) -> Option<String> {
    if results.len() < 3 {
        return None;
    }

    let last_three = &results[results.len() - 3..];
    let dimensions = ["meaning", "grammar", "naturalness", "wordChoice"];

    // Grades at B+ or worse (i.e. not A+, A, or A-)
    let is_weak = |grade: &str| -> bool {
        !matches!(grade, "A+" | "A" | "A-")
    };

    for dim in &dimensions {
        let all_weak = last_three.iter().all(|result| {
            result
                .get("scores")
                .and_then(|s| s.get(*dim))
                .and_then(|g| g.as_str())
                .is_some_and(is_weak)
        });

        if all_weak {
            return Some(format!(
                "You've hit {dim} B+ three times \u{2014} want a 20-second micro-tip?"
            ));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn segmentation_prompt_includes_languages() {
        let prompt = segmentation_prompt("Chinese", "English");
        assert!(prompt.contains("Chinese"));
        assert!(prompt.contains("English"));
        assert!(prompt.contains("translation units"));
    }

    #[test]
    fn evaluation_prompt_includes_languages() {
        let prompt = evaluation_prompt("Japanese", "English");
        assert!(prompt.contains("Japanese"));
        assert!(prompt.contains("English"));
        assert!(prompt.contains("overall_grade"));
    }

    #[test]
    fn nudge_returns_none_with_fewer_than_3_results() {
        let results = vec![
            json!({"scores": {"meaning": "B", "grammar": "B", "naturalness": "B", "wordChoice": "B"}}),
        ];
        assert!(coaching_nudge_check(&results).is_none());
    }

    #[test]
    fn nudge_detects_repeated_weakness() {
        let results = vec![
            json!({"scores": {"meaning": "A", "grammar": "A", "naturalness": "B+", "wordChoice": "A"}}),
            json!({"scores": {"meaning": "A", "grammar": "A-", "naturalness": "B", "wordChoice": "A"}}),
            json!({"scores": {"meaning": "A", "grammar": "A", "naturalness": "B-", "wordChoice": "A+"}}),
        ];
        let nudge = coaching_nudge_check(&results);
        assert!(nudge.is_some());
        assert!(nudge.unwrap().contains("naturalness"));
    }

    #[test]
    fn nudge_returns_none_when_no_repeated_weakness() {
        let results = vec![
            json!({"scores": {"meaning": "A", "grammar": "A", "naturalness": "A", "wordChoice": "A"}}),
            json!({"scores": {"meaning": "A-", "grammar": "A", "naturalness": "A+", "wordChoice": "A"}}),
            json!({"scores": {"meaning": "A", "grammar": "A-", "naturalness": "A", "wordChoice": "A-"}}),
        ];
        assert!(coaching_nudge_check(&results).is_none());
    }

    #[test]
    fn nudge_only_checks_last_three() {
        let results = vec![
            json!({"scores": {"meaning": "B", "grammar": "B", "naturalness": "B", "wordChoice": "B"}}),
            json!({"scores": {"meaning": "B", "grammar": "B", "naturalness": "B", "wordChoice": "B"}}),
            json!({"scores": {"meaning": "A", "grammar": "A", "naturalness": "A", "wordChoice": "A"}}),
            json!({"scores": {"meaning": "A", "grammar": "A", "naturalness": "A", "wordChoice": "A"}}),
            json!({"scores": {"meaning": "A", "grammar": "A", "naturalness": "A", "wordChoice": "A"}}),
        ];
        assert!(coaching_nudge_check(&results).is_none());
    }
}
