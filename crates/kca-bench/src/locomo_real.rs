//! Real LoCoMo benchmark — bit-for-bit compatible with Letta's
//! `letta-leaderboard/leaderboard/locomo/locomo_benchmark.py`.
//!
//! Dataset: `locomo10.json` (10 multi-session conversations, ~199 QA each).
//! Source: <https://github.com/letta-ai/letta-leaderboard/blob/main/leaderboard/locomo/locomo10.json>
//!
//! Scoring methodology (per Letta `utils.py::grade_sample`):
//! - LLM-judge using OpenAI gpt-4.1 with the SimpleQA-style A/B/C grader prompt.
//! - "A" = CORRECT (gold target fully contained, no contradictions)
//! - "B" = INCORRECT
//! - "C" = NOT_ATTEMPTED (no info given, but no contradiction)
//! - Accuracy = fraction of "A".
//!
//! What this DOES NOT do (deliberately): copy Letta's filesystem-tool agent
//! harness. That tests "agent uses search_files on transcripts" — we want
//! to test "our memory pipeline extracts and recalls". So we replay each
//! session as a real conversation through `AppCore::chat_complete`, letting
//! `IngestionConsumer` extract semantic facts naturally, then ask each QA
//! against the populated memory.
//!
//! Run with `OPENAI_API_KEY` set (grader). Optional env:
//! - `KCA_LOCOMO_LIMIT=N` — only first N conversations (default: all 10)
//! - `KCA_LOCOMO_QA_LIMIT=M` — only first M QA per conversation (default: all)
//! - `KCA_LOCOMO_GRADER_MODEL=gpt-4.1` (default; matches Letta)

use kca_e2e::replayer::ReplayContext;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Deserialize, Debug, Clone)]
pub struct LocoMoTurn {
    pub speaker: String,
    pub dia_id: String,
    pub text: String,
    #[serde(default)]
    pub img_url: Option<Vec<String>>,
    #[serde(default)]
    pub blip_caption: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LocoMoQA {
    pub question: String,
    /// LoCoMo answers can be string OR number — capture as raw JSON value.
    /// Category 5 (adversarial) questions have NO `answer` field — they
    /// instead carry `adversarial_answer`. Letta skips these entirely
    /// (`locomo_benchmark.py` line 113: `continue`), so we do the same to
    /// keep the score comparable.
    #[serde(default)]
    pub answer: Option<serde_json::Value>,
    #[serde(default)]
    pub evidence: Vec<String>,
    /// 1 = single-hop, 2 = multi-hop / temporal, 3 = open-domain,
    /// 4 = adversarial, 5 = abstention (no answer in conversation).
    #[serde(default)]
    pub category: Option<u32>,
    /// Some questions have an `adversarial_answer` field; we ignore for now.
    #[serde(default, rename = "adversarial_answer")]
    pub adversarial_answer: Option<serde_json::Value>,
}

impl LocoMoQA {
    pub fn answer_string(&self) -> Option<String> {
        self.answer.as_ref().map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string().trim_matches('"').to_string(),
        })
    }
}

/// One LoCoMo "sample" — a multi-session dialogue between two speakers
/// with QA pairs covering the whole conversation.
#[derive(Deserialize, Debug, Clone)]
pub struct LocoMoConversation {
    pub sample_id: String,
    /// Keys like `session_1`, `session_1_date_time`, …, `session_19`,
    /// `session_19_date_time`. Sessions are lists of `LocoMoTurn`.
    pub conversation: HashMap<String, serde_json::Value>,
    pub qa: Vec<LocoMoQA>,
    /// Auxiliary fields we don't need for scoring; keep them out of the
    /// public surface but allow the JSON to round-trip cleanly.
    #[serde(default)]
    pub event_summary: serde_json::Value,
    #[serde(default)]
    pub observation: serde_json::Value,
    #[serde(default)]
    pub session_summary: serde_json::Value,
}

impl LocoMoConversation {
    /// Iterate sessions in numeric order. Each item is `(session_index,
    /// date_time, turns)`. Skips `_date_time` keys (paired with sessions).
    pub fn ordered_sessions(&self) -> Vec<(u32, Option<String>, Vec<LocoMoTurn>)> {
        let mut out: Vec<(u32, Option<String>, Vec<LocoMoTurn>)> = Vec::new();
        for (key, val) in &self.conversation {
            if key.contains("_date_time") {
                continue;
            }
            let Some(idx_str) = key.strip_prefix("session_") else { continue };
            let Ok(idx) = idx_str.parse::<u32>() else { continue };
            let Ok(turns) = serde_json::from_value::<Vec<LocoMoTurn>>(val.clone()) else { continue };
            let dt = self
                .conversation
                .get(&format!("session_{idx}_date_time"))
                .and_then(|v| v.as_str().map(|s| s.to_string()));
            out.push((idx, dt, turns));
        }
        out.sort_by_key(|(idx, _, _)| *idx);
        out
    }
}

#[derive(Debug, Default, Clone)]
pub struct LocoMoRealReport {
    pub total: u32,
    pub correct: u32,
    pub incorrect: u32,
    pub not_attempted: u32,
    pub by_category: HashMap<u32, (u32, u32)>, // (correct, total) per LoCoMo category
    pub p50_query_latency_ms: u64,
    pub p95_query_latency_ms: u64,
    pub conversations_replayed: u32,
}

impl LocoMoRealReport {
    pub fn accuracy(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.correct as f64 / self.total as f64 }
    }
    /// Letta's "attempted accuracy" — fraction CORRECT among attempted (A or B,
    /// excluding C). Useful to disentangle "answered wrong" from "refused".
    pub fn attempted_accuracy(&self) -> f64 {
        let attempted = self.correct + self.incorrect;
        if attempted == 0 { 0.0 } else { self.correct as f64 / attempted as f64 }
    }
}

/// LoCoMo grader prompt — copied from Letta `utils.py::GRADER_TEMPLATE`
/// (SimpleQA style). Keeping this verbatim is a deliberate
/// reproducibility choice: any deviation makes our number incomparable.
const GRADER_TEMPLATE: &str = include_str!("locomo_grader_template.txt");

/// Grade a single (question, gold, predicted) triple via OpenAI gpt-4.1.
/// Returns "A" / "B" / "C" matching Letta's mapping.
pub async fn grade_sample(
    question: &str,
    target: &str,
    predicted: &str,
) -> common::Result<char> {
    let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
        common::KlyntbotError::Storage("OPENAI_API_KEY not set — required for LoCoMo grader".into())
    })?;
    let model = std::env::var("KCA_LOCOMO_GRADER_MODEL").unwrap_or_else(|_| "gpt-4.1".into());

    let prompt = GRADER_TEMPLATE
        .replace("{question}", question)
        .replace("{target}", target)
        .replace("{predicted_answer}", predicted);

    let body = serde_json::json!({
        "model": model,
        "temperature": 0,
        "max_tokens": 16,
        "messages": [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": prompt}
        ]
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| common::KlyntbotError::Storage(format!("grader client: {e}")))?;
    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(&api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("grader request: {e}")))?;
    let status = resp.status();
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("grader parse: {e}")))?;
    if !status.is_success() {
        return Err(common::KlyntbotError::Storage(format!(
            "grader HTTP {status}: {json}"
        )));
    }
    let text = json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("C");
    let ch = text
        .chars()
        .find(|c| matches!(*c, 'A' | 'B' | 'C'))
        .unwrap_or('C');
    Ok(ch)
}

/// Run the real LoCoMo benchmark.
///
/// Steps per conversation:
/// 1. Fresh `ReplayContext` (ephemeral DB).
/// 2. Replay each session in order: feed each speaker's turn as a chat
///    message tagged with `[<Speaker>]: <text>`. After each session,
///    `await_cognitive_idle` so extraction lands.
/// 3. For each QA, ask the question via `chat_complete`, capture the
///    predicted answer, grade with `grade_sample`.
pub async fn run_locomo_real(path: &Path) -> common::Result<LocoMoRealReport> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| common::KlyntbotError::Storage(format!("locomo read: {e}")))?;
    let data: Vec<LocoMoConversation> = serde_json::from_str(&raw)
        .map_err(|e| common::KlyntbotError::Storage(format!("locomo parse: {e}")))?;

    let conv_limit = std::env::var("KCA_LOCOMO_LIMIT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(data.len());
    let qa_limit = std::env::var("KCA_LOCOMO_QA_LIMIT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());

    let mut report = LocoMoRealReport::default();
    let mut latencies: Vec<u64> = Vec::new();

    for conv in data.iter().take(conv_limit) {
        eprintln!(
            "[locomo-real] === {} ({} sessions, {} QA) ===",
            conv.sample_id,
            conv.ordered_sessions().len(),
            conv.qa.len()
        );
        let ctx = ReplayContext::new().await?;

        // Replay every session as conversation history.
        for (idx, dt, turns) in conv.ordered_sessions() {
            let session_key = common::SessionKey::from_parts("locomo-real", &conv.sample_id);
            // One bulk message per session: prefix with date_time + speaker
            // labels so the agent can attribute facts. Replaying every turn
            // individually would multiply LLM cost ~30× for marginal gain.
            let mut blob = String::new();
            if let Some(dt) = &dt {
                blob.push_str(&format!("[Session {idx} — {dt}]\n"));
            } else {
                blob.push_str(&format!("[Session {idx}]\n"));
            }
            for t in &turns {
                blob.push_str(&format!("{}: {}\n", t.speaker, t.text));
            }
            let _ = ctx
                .chat_complete(blob, session_key.to_string())
                .await?;
            ctx.await_cognitive_idle().await;
        }

        let qa_take = qa_limit.unwrap_or(conv.qa.len());
        for qa in conv.qa.iter().take(qa_take) {
            // Skip category 5 (adversarial) — Letta excludes them from scoring.
            if qa.category == Some(5) || qa.answer.is_none() {
                continue;
            }
            let started = std::time::Instant::now();
            let session_key = common::SessionKey::from_parts("locomo-real", &conv.sample_id);
            let predicted = ctx
                .chat_complete(qa.question.clone(), session_key.to_string())
                .await?;
            let elapsed = started.elapsed().as_millis() as u64;
            latencies.push(elapsed);

            let Some(target) = qa.answer_string() else { continue };
            let grade = grade_sample(&qa.question, &target, &predicted).await?;
            report.total += 1;
            let cat = qa.category.unwrap_or(0);
            let entry = report.by_category.entry(cat).or_insert((0, 0));
            entry.1 += 1;
            match grade {
                'A' => {
                    report.correct += 1;
                    entry.0 += 1;
                }
                'B' => report.incorrect += 1,
                _ => report.not_attempted += 1,
            }
            eprintln!(
                "[locomo-real] q={:?} gold={:?} pred={:?} grade={}",
                qa.question.chars().take(70).collect::<String>(),
                target,
                predicted.chars().take(70).collect::<String>(),
                grade
            );
        }
        report.conversations_replayed += 1;
    }

    latencies.sort();
    if !latencies.is_empty() {
        report.p50_query_latency_ms = latencies[latencies.len() / 2];
        let p95_idx = ((latencies.len() as f64) * 0.95) as usize;
        report.p95_query_latency_ms = latencies[p95_idx.min(latencies.len() - 1)];
    }
    Ok(report)
}
