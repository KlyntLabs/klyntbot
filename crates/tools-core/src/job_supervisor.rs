//! Background-job types shared across tools and the runtime.
//!
//! The concrete implementation lives in `feature-coding-bash`. Tools call into the
//! supervisor through the [`JobSupervisorHandle`] trait so they don't need a direct
//! dependency on the feature crate.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use jiff::Timestamp;
use rand::Rng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable string id of a background job. Format: "bash-{10 base32 chars}".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub String);

impl JobId {
    pub fn new() -> Self {
        const ALPHABET: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
        let mut bytes = [0u8; 7];
        rand::rng().fill(&mut bytes);
        let mut bits = 0u64;
        for b in &bytes {
            bits = (bits << 8) | (*b as u64);
        }
        let mut buf = [0u8; 10];
        for i in 0..10 {
            let shift = 51 - i * 5;
            let idx = ((bits >> shift) & 0x1f) as usize;
            buf[i] = ALPHABET[idx];
        }
        Self(format!("bash-{}", String::from_utf8(buf.to_vec()).unwrap()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_str(s: impl Into<String>) -> Result<Self, JobError> {
        let s: String = s.into();
        if !s.starts_with("bash-") || s.len() != "bash-".len() + 10 {
            return Err(JobError::InvalidJobId(s));
        }
        let suffix = &s["bash-".len()..];
        const ALPHABET: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
        if !suffix.bytes().all(|b| ALPHABET.contains(&b)) {
            return Err(JobError::InvalidJobId(s));
        }
        Ok(Self(s))
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum JobStatus {
    Starting,
    Running,
    Completed,
    Failed,
    Cancelled,
    Lost,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "Starting",
            Self::Running => "Running",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
            Self::Lost => "Lost",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Lost
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum FailureKind {
    CompileError,
    TestFailure,
    LintFailure,
    NetworkBindFailure,
    Timeout,
    Cancelled,
    Lost,
    Other(String),
}

impl FailureKind {
    pub fn as_db_str(&self) -> Cow<'static, str> {
        match self {
            Self::Other(s) => Cow::Owned(format!("Other:{s}")),
            Self::CompileError => Cow::Borrowed("CompileError"),
            Self::TestFailure => Cow::Borrowed("TestFailure"),
            Self::LintFailure => Cow::Borrowed("LintFailure"),
            Self::NetworkBindFailure => Cow::Borrowed("NetworkBindFailure"),
            Self::Timeout => Cow::Borrowed("Timeout"),
            Self::Cancelled => Cow::Borrowed("Cancelled"),
            Self::Lost => Cow::Borrowed("Lost"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GateResult {
    Passed,
    Failed {
        kind: FailureKind,
        detail: String,
        extracted: serde_json::Value,
    },
}

#[derive(Debug, Clone)]
pub struct JobSpec {
    pub session_id: String,
    pub agent_id: String,
    pub description: String,
    pub command: String,
    pub cwd: PathBuf,
    pub timeout_ms: u64,
    pub silent_completion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobView {
    pub id: JobId,
    pub session_id: String,
    pub agent_id: String,
    pub description: String,
    pub command: String,
    pub cwd: PathBuf,
    pub status: JobStatus,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
    pub exit_code: Option<i32>,
    pub gate_result: Option<GateResult>,
    pub failure_extracted: Option<serde_json::Value>,
    pub total_bytes_emitted: u64,
    pub bisect_generation: u64,
    pub last_polled_at: Option<Timestamp>,
    pub last_seen_offset: u64,
}

#[derive(Debug, Clone)]
pub struct RingRead {
    pub bytes: Vec<u8>,
    pub new_offset: u64,
    pub bisect_generation: u64,
    pub bisect_occurred_since: bool,
    pub total_bytes_emitted: u64,
}

#[derive(Debug, Error)]
pub enum JobError {
    #[error("invalid job id: {0}")]
    InvalidJobId(String),
    #[error("job not found: {0}")]
    NotFound(String),
    #[error("concurrency cap reached: {active} active in (session, agent_chain)")]
    CapReached { active: usize },
    #[error("missing description (required when run_in_background=true)")]
    MissingDescription,
    #[error("storage error: {0}")]
    Storage(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("spawn error: {0}")]
    Spawn(String),
    #[error("classification error: {0}")]
    Classification(String),
}

#[async_trait]
pub trait JobSupervisorHandle: Send + Sync + std::fmt::Debug {
    async fn spawn(&self, spec: JobSpec) -> Result<JobView, JobError>;
    async fn output_delta(
        &self,
        id: &JobId,
        since: u64,
        block: bool,
        timeout_ms: u64,
    ) -> Result<RingRead, JobError>;
    async fn stop(&self, id: &JobId, reason: &str) -> Result<JobView, JobError>;
    async fn list(
        &self,
        session_id: &str,
        agent_chain: &[String],
        active_only: bool,
    ) -> Vec<JobView>;
}

pub type DynJobSupervisor = Arc<dyn JobSupervisorHandle>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_id_format() {
        let id = JobId::new();
        assert!(id.as_str().starts_with("bash-"));
        assert_eq!(id.as_str().len(), "bash-".len() + 10);
    }

    #[test]
    fn job_id_parsing() {
        assert!(JobId::from_str("bash-0123456789").is_ok());
        assert!(JobId::from_str("notbash-0123456789").is_err());
        assert!(JobId::from_str("bash-short").is_err());
        assert!(JobId::from_str("bash-toolongchar").is_err());
        // 'i' and 'l' are excluded from the alphabet
        assert!(JobId::from_str("bash-iiiiiiiiii").is_err());
        assert!(JobId::from_str("bash-llllllllll").is_err());
    }

    #[test]
    fn job_status_terminal() {
        assert!(!JobStatus::Starting.is_terminal());
        assert!(!JobStatus::Running.is_terminal());
        assert!(JobStatus::Completed.is_terminal());
        assert!(JobStatus::Failed.is_terminal());
        assert!(JobStatus::Cancelled.is_terminal());
        assert!(JobStatus::Lost.is_terminal());
    }

    #[test]
    fn failure_kind_db_str() {
        assert_eq!(FailureKind::CompileError.as_db_str().as_ref(), "CompileError");
        assert_eq!(FailureKind::Other("oom".into()).as_db_str().as_ref(), "Other:oom");
    }
}
