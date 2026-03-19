/// Build system prompt for translation + sentence breakdown.
pub fn translate_breakdown_prompt(source_lang: &str, target_lang: &str) -> String {
    format!(
        r#"You are a language learning assistant. Translate the given text from {source_lang} to {target_lang} and provide a detailed breakdown.

Respond ONLY with a JSON object (no markdown fences, no explanation). The format:
{{
  "translation": "full translation",
  "words": [
    {{
      "word": "original word",
      "reading": "pronunciation (pinyin for Chinese, IPA for English, null if same script)",
      "meaning": "translation of this word",
      "part_of_speech": "noun/verb/adj/adv/etc",
      "proficiency_level": "HSK 1-6 for Chinese, CEFR A1-C2 for English, null if unknown",
      "example_sentence": "a short example sentence using this word"
    }}
  ],
  "grammar_patterns": [
    {{
      "pattern": "[Subject] + verb + [Object] + 来 + [Purpose]",
      "explanation": "plain language explanation of this grammar pattern",
      "pattern_type": "purpose clause / passive / conditional / etc"
    }}
  ]
}}

Rules:
- Extract ALL meaningful words (skip particles/punctuation unless pedagogically important)
- For Chinese: always include pinyin with tone marks
- For English: include IPA only for words with non-obvious pronunciation
- Identify 1-3 grammar patterns (0 if none are notable)
- proficiency_level: use HSK 1-6 for Chinese, CEFR A1-C2 for European languages
- Keep explanations concise (1-2 sentences)"#
    )
}

/// Build system prompt for evaluating a user's translation attempt.
pub fn evaluate_translation_prompt(source_lang: &str, target_lang: &str) -> String {
    format!(
        r#"You are a language learning evaluator. A student is translating from {source_lang} to {target_lang}. Evaluate their translation across 4 dimensions.

You will receive the source text and the student's translation attempt.

Respond ONLY with a JSON object:
{{
  "grades": {{
    "meaning": "A+/A/A-/B+/B/B-/C+/C/C-/D+/D/F",
    "grammar": "same scale",
    "naturalness": "same scale",
    "word_choice": "same scale"
  }},
  "corrections": [
    {{
      "original": "what the student wrote",
      "suggested": "better version",
      "explanation": "why this is better (1-2 sentences, include linguistic reason)",
      "category": "grammar/vocabulary/register/naturalness"
    }}
  ],
  "model_translation": "your ideal translation of the source text"
}}

Grading guide:
- A: native-level quality
- B: clearly understood, minor issues
- C: meaning conveyed but notable errors
- D: significant errors affecting comprehension
- F: incomprehensible or wrong meaning

Rules:
- Be encouraging but honest
- Focus on the most impactful corrections (max 5)
- Explain WHY each correction matters for learning
- model_translation should be natural, not literal"#
    )
}

/// Build system prompt for detecting confusable words.
pub fn detect_confusables_prompt(source_lang: &str) -> String {
    format!(
        r#"You are a vocabulary specialist for {source_lang}. Given two similar words, explain the key difference between them for a language learner.

Respond ONLY with a JSON object:
{{
  "explanation": "clear explanation of the difference (2-3 sentences)",
  "word1_usage": "when to use word 1",
  "word2_usage": "when to use word 2",
  "example_word1": "example sentence using word 1",
  "example_word2": "example sentence using word 2"
}}"#
    )
}

/// Build system prompt for annotation language enrichment.
pub fn enrich_annotation_prompt(source_lang: &str, target_lang: &str) -> String {
    format!(
        r#"You are a language learning assistant. Translate the given text from {source_lang} to {target_lang} and extract key vocabulary.

Respond ONLY with a JSON object:
{{
  "translation": "full translation",
  "words": [
    {{
      "word": "original word",
      "reading": "pronunciation (pinyin/IPA/null)",
      "meaning": "translation",
      "part_of_speech": "noun/verb/adj/etc",
      "proficiency_level": "HSK 1-6 / CEFR A1-C2 / null"
    }}
  ]
}}

Keep it concise — this is for a small annotation card, not a full breakdown."#
    )
}
