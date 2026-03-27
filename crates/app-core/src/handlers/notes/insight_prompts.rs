//! LLM prompt templates for the Insight Review tabs.
//!
//! All prompt functions accept an optional `response_lang` parameter (ISO 639-1 code).
//! When set to a non-English language, a language instruction is appended so the LLM
//! responds in the user's source language.

/// Resolve a language code to a display name, skipping English and unknown codes.
fn resolve_lang_name(lang: Option<&str>) -> Option<&str> {
    let code = match lang {
        Some(c) if !c.is_empty() && c != "en" => c,
        _ => return None,
    };
    lang_code_to_name(code)
}

/// Build a language instruction suffix for prompts.
/// Returns empty string for English or unrecognised codes.
fn language_instruction(lang: Option<&str>) -> String {
    match resolve_lang_name(lang) {
        Some(name) => format!(
            "\n\nIMPORTANT: Write your entire response in {name}. Do not default to English."
        ),
        None => String::new(),
    }
}

/// Like `language_instruction` but for JSON-producing prompts where structure must stay English.
fn language_instruction_json(lang: Option<&str>) -> String {
    match resolve_lang_name(lang) {
        Some(name) => format!(
            "\n\nIMPORTANT: Write all human-readable text (questions, answers, explanations, descriptions, titles) in {name}. Keep JSON keys in English."
        ),
        None => String::new(),
    }
}

fn lang_code_to_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "vi" => "Vietnamese",
        "zh" => "Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "th" => "Thai",
        "ar" => "Arabic",
        "hi" => "Hindi",
        "ru" => "Russian",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "pt" => "Portuguese",
        "it" => "Italian",
        "nl" => "Dutch",
        "pl" => "Polish",
        "tr" => "Turkish",
        "uk" => "Ukrainian",
        "id" => "Indonesian",
        "ms" => "Malay",
        "sv" => "Swedish",
        "da" => "Danish",
        "no" | "nb" | "nn" => "Norwegian",
        "fi" => "Finnish",
        "cs" => "Czech",
        "ro" => "Romanian",
        "hu" => "Hungarian",
        "el" => "Greek",
        "he" => "Hebrew",
        "fa" => "Persian",
        "bn" => "Bengali",
        "ta" => "Tamil",
        "te" => "Telugu",
        "mr" => "Marathi",
        "ur" => "Urdu",
        "sw" => "Swahili",
        "my" => "Burmese",
        "km" => "Khmer",
        "lo" => "Lao",
        "tl" | "fil" => "Filipino",
        _ => return None,
    })
}

/// Tab 1: Synthesis — streaming markdown response.
pub fn synthesis_prompt(context: &str, response_lang: Option<&str>) -> String {
    let lang = language_instruction(response_lang);
    format!(
        r#"You are a research synthesis assistant. Given the user's note and its related notes from their knowledge base, write a deep synthesis that:

1. Identifies the 3-5 key themes across these notes
2. Draws non-obvious connections between concepts
3. Highlights where ideas reinforce or build on each other
4. Surfaces insights the user may not have explicitly written

Format as clean Markdown with ## headings for each theme.
Keep it focused and insightful — not a summary, but a synthesis.
Do not repeat content verbatim from the notes.{lang}

--- BEGIN KNOWLEDGE CONTEXT ---
{context}
--- END KNOWLEDGE CONTEXT ---"#
    )
}

/// Tab 2: Gap Analysis — markdown + trailing JSON block.
pub fn gap_analysis_prompt(context: &str, response_lang: Option<&str>) -> String {
    let lang = language_instruction(response_lang);
    format!(
        r#"You are a knowledge gap analyst. Given the user's note cluster, identify:

1. **Missing concepts** — important topics referenced but never explored in depth
2. **Contradictions** — places where notes disagree or present conflicting info
3. **Shallow coverage** — topics mentioned briefly that deserve deeper treatment
4. **Research suggestions** — specific papers, books, or topics to explore next
5. **Notes to create** — suggest 2-3 new note titles that would strengthen the network

Format as Markdown with clear sections. Be specific and actionable.
For each gap, reference which note(s) it relates to.

ALSO return a machine-readable JSON block at the end, wrapped in ```json fences:
[{{"topic": "short title", "description": "1-2 sentence description", "suggestedTitle": "New Note: ..."}}]{lang}

--- BEGIN KNOWLEDGE CONTEXT ---
{context}
--- END KNOWLEDGE CONTEXT ---"#
    )
}

/// Tab 3: Self-Assessment — pure JSON response.
pub fn self_assessment_prompt(context: &str, response_lang: Option<&str>) -> String {
    let lang = language_instruction_json(response_lang);
    format!(
        r#"You are an educational assessment designer. Generate a self-assessment quiz based on the user's knowledge network.

Generate exactly 8 questions:
- 4 multiple choice (4 options each, one correct)
- 4 short answer (expecting 1-2 sentence responses)

For each question, include:
- A unique short id (e.g. "q1", "q2")
- The question text
- The correct answer
- A brief explanation of why
- Which note(s) the question draws from
- Difficulty: "easy", "medium", or "hard"
- Difficulty score: 0.0-1.0

Questions should test understanding, not memorization. Include questions that require connecting ideas across multiple notes.

Respond ONLY with a JSON array (no markdown fences, no explanation):
[{{"id": "q1", "type": "multiple_choice", "question": "...", "choices": ["A", "B", "C", "D"], "correctAnswer": "...", "explanation": "...", "sourceNotes": ["note title"], "difficulty": "medium", "difficultyScore": 0.5}}]{lang}

--- BEGIN KNOWLEDGE CONTEXT ---
{context}
--- END KNOWLEDGE CONTEXT ---"#
    )
}

/// Tab 4: Concept Map — mermaid mindmap syntax.
pub fn concept_map_prompt(context: &str, root_title: &str, response_lang: Option<&str>) -> String {
    let lang = language_instruction(response_lang);
    format!(
        r#"You are a concept mapping specialist. Create a Mermaid mindmap diagram showing how concepts connect across the user's note cluster.

Rules:
- Use Mermaid mindmap syntax exactly
- Root node = root(({root_title}))
- Branch into major themes/concepts
- Show connections to related notes by name
- Max 4 levels deep, max 5-6 branches per node
- Max 6 words per node label
- Use clean, short labels (no full sentences)

If you cannot generate valid Mermaid syntax, return a clean indented text outline instead, prefixed with "FALLBACK:" on the first line.

Example format:
mindmap
  root((Machine Learning Notes))
    Supervised Learning
      Regression
      Classification
    Neural Networks
      Deep Learning
      Transformers
{lang}
--- BEGIN KNOWLEDGE CONTEXT ---
{context}
--- END KNOWLEDGE CONTEXT ---"#
    )
}

/// Generate a brief "What's Changed" summary comparing previous insight with current context.
pub fn changes_summary_prompt(
    previous_synthesis: &str,
    current_context: &str,
    response_lang: Option<&str>,
) -> String {
    let lang = language_instruction(response_lang);
    format!(
        r#"You are comparing a previous insight analysis with updated knowledge context to identify what's new or different.

Previous synthesis:
{previous_synthesis}

Current knowledge context (may have new notes, updated content, or addressed gaps):
{current_context}

Respond with a single concise sentence (max 80 words) summarizing what has changed. Focus on:
- New related notes discovered
- Previous gaps that have been addressed
- Significant content changes
- New connections not seen before

If nothing meaningful has changed, respond with exactly: "No significant changes since last review."

Do NOT repeat the synthesis. Just describe what's different.{lang}"#
    )
}

/// Generate an applied scenario challenge based on the knowledge context.
pub fn scenario_challenge_prompt(context: &str, response_lang: Option<&str>) -> String {
    let lang = language_instruction_json(response_lang);
    format!(
        r#"You are a scenario designer for applied learning. Create a realistic scenario that tests the user's ability to apply concepts from their knowledge network.

Requirements:
- Concrete situation (not abstract)
- Requires applying concepts from 2+ notes
- 2-3 decision points where knowledge matters
- The best approach should NOT be immediately obvious

Respond ONLY with JSON (no markdown, no explanation):
{{"title": "Short scenario title (5-10 words)", "situation": "2-3 paragraph setup describing the scenario in detail", "questions": ["Decision question 1", "Decision question 2", "Decision question 3"], "modelAnswer": "3-4 paragraph ideal response explaining the reasoning and how concepts from the notes apply", "sourceNotes": ["note title 1", "note title 2"], "difficultyScore": 0.7}}{lang}

--- BEGIN KNOWLEDGE CONTEXT ---
{context}
--- END KNOWLEDGE CONTEXT ---"#
    )
}
