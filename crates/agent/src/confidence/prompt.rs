//! System prompt fragment for confidence evaluation instructions.

/// Build confidence evaluation instructions with the current adaptive threshold.
///
/// Instructs the LLM to output a `<confidence>` block before tool calls.
/// This block is parsed by the evaluator and stripped before reaching the user.
/// The threshold is dynamic so it stays in sync with the learning system.
pub fn confidence_prompt(threshold: f32) -> String {
    format!(
        r#"## Internal Decision Confidence

Before making tool calls, assess your confidence in understanding the user's intent.
Output a <confidence> block with your assessment (this is internal, never shown to user):

<confidence>
{{"score": 0.0-1.0, "intent_clarity": 0.0-1.0, "tool_fit": 0.0-1.0, "info_sufficiency": 0.0-1.0, "reasoning": "brief explanation"}}
</confidence>

Guidelines:
- score >= {threshold:.2}: Proceed with tool calls
- score < {threshold:.2}: Use ask_user to clarify before proceeding
- intent_clarity: How well you understand what the user wants
- tool_fit: How well the chosen tool matches the need
- info_sufficiency: Whether you have enough info to execute

Always include a <confidence> block before tool calls. After tool execution, include another
<confidence> block assessing the result quality."#
    )
}
