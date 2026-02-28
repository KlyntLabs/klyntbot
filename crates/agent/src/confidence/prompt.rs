//! System prompt fragment for confidence evaluation instructions.

/// Build confidence evaluation instructions with the current adaptive threshold.
///
/// Instructs the LLM to output a `<confidence>` block before tool calls.
/// This block is parsed by the evaluator and stripped before reaching the user.
/// The threshold is dynamic so it stays in sync with the learning system.
pub fn confidence_prompt(threshold: f32) -> String {
    format!(
        r#"## Internal Decision Confidence

IMPORTANT: "confidence" is NOT a tool. Do NOT generate a tool call named "confidence".

Before making tool calls, briefly assess your confidence in your text response (not as a tool call).
Include a short XML tag in your text output like this:

<confidence score="0.85" clarity="high" reasoning="clear intent" />

Guidelines:
- score >= {threshold:.2}: Proceed with tool calls
- score < {threshold:.2}: Use ask_user tool to clarify before proceeding

This is purely internal text markup, not a function/tool call."#
    )
}
