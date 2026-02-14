//! System prompt fragment for confidence evaluation instructions.

/// Confidence evaluation instructions injected into the system prompt.
///
/// Instructs the LLM to output a `<confidence>` block before tool calls.
/// This block is parsed by the evaluator and stripped before reaching the user.
pub const CONFIDENCE_PROMPT: &str = r#"## Internal Decision Confidence

Before making tool calls, assess your confidence in understanding the user's intent.
Output a <confidence> block with your assessment (this is internal, never shown to user):

<confidence>
{"score": 0.0-1.0, "intent_clarity": 0.0-1.0, "tool_fit": 0.0-1.0, "info_sufficiency": 0.0-1.0, "reasoning": "brief explanation"}
</confidence>

Guidelines:
- score >= 0.7: Proceed with tool calls
- score < 0.7: Use ask_user to clarify before proceeding
- intent_clarity: How well you understand what the user wants
- tool_fit: How well the chosen tool matches the need
- info_sufficiency: Whether you have enough info to execute

Always include a <confidence> block before tool calls. After tool execution, include another
<confidence> block assessing the result quality."#;
