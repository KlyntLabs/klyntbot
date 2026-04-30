//! Long-memory benchmark accuracy. Replays each fixture, runs each query
//! through the assistant, scores against gold_answer.

use kca_e2e::fixtures::{load_jsonl, ConversationFixture};
use kca_e2e::replayer::ReplayContext;
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct LongMemBenchReport {
    pub total_queries: u32,
    pub correct: u32,
    pub by_hop_type: std::collections::HashMap<String, (u32, u32)>,
    pub p50_query_latency_ms: u64,
    pub p95_query_latency_ms: u64,
}

impl LongMemBenchReport {
    pub fn accuracy(&self) -> f64 {
        if self.total_queries == 0 {
            0.0
        } else {
            self.correct as f64 / self.total_queries as f64
        }
    }
}

pub async fn run_longmembench(path: &Path) -> common::Result<LongMemBenchReport> {
    let fixtures: Vec<ConversationFixture> = load_jsonl(path)?;
    let limit = std::env::var("KCA_BENCH_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(fixtures.len());
    let mut report = LongMemBenchReport::default();
    let mut latencies = Vec::new();

    for f in fixtures.iter().take(limit) {
        let mut ctx = ReplayContext::new().await?;
        ctx.replay(f).await?;
        for q in &f.queries {
            let started = std::time::Instant::now();
            let answer = ctx
                .chat_complete(
                    q.query.clone(),
                    common::SessionKey::from_parts("bench-lmb", &f.id).to_string(),
                )
                .await?;
            let elapsed = started.elapsed().as_millis() as u64;
            latencies.push(elapsed);
            let correct = scoring::is_answer_correct(&answer, &q.gold_answer);
            let entry = report
                .by_hop_type
                .entry(q.hop_type.clone())
                .or_insert((0, 0));
            entry.1 += 1;
            if correct {
                entry.0 += 1;
                report.correct += 1;
            }
            report.total_queries += 1;
        }
    }

    latencies.sort();
    if !latencies.is_empty() {
        report.p50_query_latency_ms = latencies[latencies.len() / 2];
        report.p95_query_latency_ms = latencies[(latencies.len() as f64 * 0.95) as usize];
    }
    Ok(report)
}

pub mod scoring {
    /// True if the predicted answer covers the gold answer.
    ///
    /// Two-pass:
    ///   1. **Substring match** (fast path). A single-token gold like
    ///      "Anthropic" or "SF" is correct iff it appears as a substring of
    ///      the normalized prediction.
    ///   2. **Token-overlap recall** (narrative path). For multi-token gold
    ///      answers, the prediction is correct iff at least 80% of gold's
    ///      content tokens (length ≥ 3, non-stopword) appear in the
    ///      prediction's token set. Catches cases where the LLM emits
    ///      "She lives in San Francisco" with gold "San Francisco, CA".
    pub fn is_answer_correct(predicted: &str, gold: &str) -> bool {
        let pn = normalize(predicted);
        let gn = normalize(gold);
        if gn.is_empty() {
            return false;
        }
        if pn.contains(&gn) {
            return true;
        }
        let pred_tokens: std::collections::HashSet<&str> = pn.split_whitespace().collect();
        let gold_tokens: Vec<&str> = gn
            .split_whitespace()
            .filter(|t| t.len() >= 3 && !is_stopword(t))
            .collect();
        if gold_tokens.is_empty() {
            return false;
        }
        let hits = gold_tokens
            .iter()
            .filter(|t| pred_tokens.contains(*t))
            .count();
        (hits as f64 / gold_tokens.len() as f64) >= 0.8
    }
    fn normalize(s: &str) -> String {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
    fn is_stopword(t: &str) -> bool {
        matches!(
            t,
            "the" | "and" | "for" | "with" | "from" | "that" | "this" | "are" | "was" | "but"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoring_basic_match() {
        assert!(scoring::is_answer_correct(
            "She works at Anthropic in SF",
            "Anthropic"
        ));
        assert!(!scoring::is_answer_correct(
            "She works at Google",
            "Anthropic"
        ));
    }
    #[test]
    fn scoring_token_overlap_narrative() {
        // Narrative reply, multi-token gold — should match via token overlap.
        assert!(scoring::is_answer_correct(
            "Alice moved to San Francisco last spring",
            "San Francisco"
        ));
    }
    #[test]
    fn scoring_rejects_low_overlap() {
        assert!(!scoring::is_answer_correct(
            "Alice mentioned moving cities",
            "San Francisco California"
        ));
    }
}
