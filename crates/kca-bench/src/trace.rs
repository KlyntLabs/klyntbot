//! Per-QA trace infrastructure for the LoCoMo benchmark.
//!
//! Phase 0 of the four-phase memory improvement ladder. Emits one JSONL
//! event per graded QA so we can attribute lift to specific mechanisms
//! when we A/B subsequent phases. Output gated by `KCA_TRACE=1`.
//!
//! Hit counts (vector_hits, fts_hits, episodic_hits) are sourced from
//! `cognitive::bench_hooks` — a thread-local cell that the cognitive
//! retrieval path writes to and the bench reads once per QA. See plan
//! at ~/.claude/plans/rosy-booping-hejlsberg.md.

use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use cognitive::bench_hooks::{
    read_entities, read_hit_counts, record_hits, reset_hit_counts, HitCounts,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct PhaseFlags {
    pub p1: bool,
    pub p2: bool,
    pub p3: bool,
    pub p4: bool,
}

impl PhaseFlags {
    pub fn from_env() -> Self {
        Self {
            p1: env_truthy("KCA_PHASE_1"),
            p2: env_truthy("KCA_PHASE_2"),
            p3: env_truthy("KCA_PHASE_3"),
            p4: env_truthy("KCA_PHASE_4"),
        }
    }

    /// Bit 0 = P1, …, bit 3 = P4. Stamped on every event so concatenated
    /// JSONL files can be partitioned without an external metadata file.
    pub fn bitmask(&self) -> u8 {
        (self.p1 as u8) | ((self.p2 as u8) << 1) | ((self.p3 as u8) << 2) | ((self.p4 as u8) << 3)
    }
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("yes")
    )
}

/// One row of `./benchmark-out/trace-<run_id>.jsonl`.
///
/// Append-only schema: future phases add fields with `#[serde(default)]`
/// so old JSONL files remain readable by new analyzers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaTraceEvent {
    pub run_id: String,
    pub conv_id: String,
    pub qa_index: u32,
    pub category: u8,
    pub question: String,
    pub gold: String,
    pub predicted: String,
    pub grade: char, // 'A' | 'B' | 'C'

    // Phase 1 — extraction subject correctness
    #[serde(default)]
    pub entities_extracted: Vec<String>,
    #[serde(default)]
    pub subject_was_speaker: bool,

    // Phase 2 — retrieval mechanism
    #[serde(default)]
    pub fts_query: String,
    #[serde(default)]
    pub vector_hits: u32,
    #[serde(default)]
    pub fts_hits: u32,
    #[serde(default)]
    pub episodic_hits: u32,
    #[serde(default)]
    pub top_subjects: Vec<String>,
    #[serde(default)]
    pub top_predicates: Vec<String>,

    // Phase 3 — consolidation
    #[serde(default)]
    pub reorganize_fired: bool,

    // Phase 4 — refusal-detection retry
    #[serde(default)]
    pub retry_fired: bool,
    #[serde(default)]
    pub predicted_was_refusal: bool,

    pub qa_latency_ms: u64,
    pub phases_enabled: u8,
}

/// JSONL writer. No-op when `KCA_TRACE` is unset.
pub struct TraceWriter {
    file: Option<BufWriter<File>>,
    pub run_id: String,
    pub phases_enabled: u8,
}

impl TraceWriter {
    pub fn open_if_enabled(run_id: String, phases_enabled: u8) -> std::io::Result<Self> {
        if !env_truthy("KCA_TRACE") {
            return Ok(Self { file: None, run_id, phases_enabled });
        }
        let dir = PathBuf::from("benchmark-out");
        create_dir_all(&dir)?;
        let path = dir.join(format!("trace-{run_id}.jsonl"));
        let file = File::create(&path)?;
        eprintln!("[trace] writing to {}", path.display());
        Ok(Self {
            file: Some(BufWriter::new(file)),
            run_id,
            phases_enabled,
        })
    }

    /// Append one event. Failures log to stderr but never abort the run.
    pub fn emit(&mut self, event: &QaTraceEvent) {
        let Some(file) = self.file.as_mut() else { return };
        match serde_json::to_string(event) {
            Ok(line) => {
                if let Err(e) = writeln!(file, "{line}") {
                    eprintln!("[trace] write failed: {e}");
                }
            }
            Err(e) => eprintln!("[trace] serialize failed: {e}"),
        }
    }
}

impl Drop for TraceWriter {
    fn drop(&mut self) {
        if let Some(f) = self.file.as_mut() {
            let _ = f.flush();
        }
    }
}

/// Generate a short slug `locomo-real-<YYYYMMDD>-<6-hex>`. Override via
/// `KCA_RUN_ID` to make a run reproducible by name.
pub fn make_run_id() -> String {
    if let Ok(explicit) = std::env::var("KCA_RUN_ID") {
        if !explicit.is_empty() {
            return explicit;
        }
    }
    let zoned = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::system());
    let date = zoned.strftime("%Y%m%d").to_string();
    let nanos = jiff::Timestamp::now().as_nanosecond() as u64;
    let suffix = format!("{:06x}", (nanos as u32) & 0x00FF_FFFF);
    format!("locomo-real-{date}-{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_flags_bitmask() {
        let f = PhaseFlags { p1: true, p3: true, ..Default::default() };
        assert_eq!(f.bitmask(), 0b0101);
    }
}
