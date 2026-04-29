//! Fixture loaders for KCA E2E tests + benchmarks.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationFixture {
    pub id: String,
    pub turns: Vec<TurnFixture>,
    pub queries: Vec<QueryFixture>,
    pub source: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TurnFixture {
    pub user: String,
    pub assistant: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCallFixture>,
    #[serde(default)]
    pub ground_truth_facts: Vec<GroundTruthFact>,
    #[serde(default)]
    pub cli_source: Option<String>,
    #[serde(default)]
    pub recorded_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFixture {
    pub name: String,
    pub args_json: serde_json::Value,
    pub result_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruthFact {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFixture {
    pub query: String,
    pub gold_answer: String,
    pub hop_type: String,
    #[serde(default)]
    pub required_fact_subjects: Vec<String>,
}

pub fn load_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> common::Result<Vec<T>> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| common::KlyntbotError::Storage(format!("read fixture {path:?}: {e}")))?;
    let mut out = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let item: T = serde_json::from_str(line)
            .map_err(|e| common::KlyntbotError::Storage(format!("parse line {i}: {e}")))?;
        out.push(item);
    }
    Ok(out)
}

pub fn fixtures_root() -> std::path::PathBuf {
    let workspace = std::env::var("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .expect("CARGO_MANIFEST_DIR set");
    workspace
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/kca")
}
