//! Cron job types and structures.

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Schedule definition for a cron job
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CronSchedule {
    /// One-time execution at a specific timestamp
    #[serde(rename = "at")]
    At {
        #[serde(rename = "atMs")]
        at_ms: i64,
    },

    /// Recurring execution with fixed interval
    #[serde(rename = "every")]
    Every {
        #[serde(rename = "everyMs")]
        every_ms: u64,
    },

    /// Cron expression-based scheduling
    #[serde(rename = "cron")]
    Cron {
        expr: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tz: Option<String>,
    },
}

/// What to do when the job runs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronPayload {
    #[serde(default = "default_payload_kind")]
    pub kind: String,

    #[serde(default)]
    pub message: String,

    /// Deliver response to channel
    #[serde(default)]
    pub deliver: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

fn default_payload_kind() -> String {
    "agent_turn".to_string()
}

impl Default for CronPayload {
    fn default() -> Self {
        Self {
            kind: default_payload_kind(),
            message: String::new(),
            deliver: false,
            channel: None,
            to: None,
        }
    }
}

/// Runtime state of a job
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CronJobState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run_at_ms: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at_ms: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// A scheduled job
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJob {
    pub id: String,
    pub name: String,

    #[serde(default = "default_enabled")]
    pub enabled: bool,

    pub schedule: CronSchedule,

    #[serde(default)]
    pub payload: CronPayload,

    #[serde(default)]
    pub state: CronJobState,

    #[serde(default)]
    pub created_at_ms: i64,

    #[serde(default)]
    pub updated_at_ms: i64,

    #[serde(default)]
    pub delete_after_run: bool,
}

fn default_enabled() -> bool {
    true
}

/// Persistent store for cron jobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronStore {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default)]
    pub jobs: Vec<CronJob>,
}

fn default_version() -> u32 {
    1
}

impl Default for CronStore {
    fn default() -> Self {
        Self {
            version: default_version(),
            jobs: Vec::new(),
        }
    }
}

impl CronJob {
    /// Create a new cron job
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        schedule: CronSchedule,
        message: impl Into<String>,
    ) -> Self {
        let now_ms = Utc::now().timestamp_millis();
        Self {
            id: id.into(),
            name: name.into(),
            enabled: true,
            schedule,
            payload: CronPayload {
                kind: "agent_turn".to_string(),
                message: message.into(),
                deliver: false,
                channel: None,
                to: None,
            },
            state: CronJobState::default(),
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            delete_after_run: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cron_schedule_serde() {
        // At variant
        let schedule = CronSchedule::At { at_ms: 1234567890 };
        let json = serde_json::to_value(&schedule).unwrap();
        assert_eq!(json["kind"], "at");
        assert_eq!(json["atMs"], 1234567890);
        match serde_json::from_value::<CronSchedule>(json).unwrap() {
            CronSchedule::At { at_ms } => assert_eq!(at_ms, 1234567890),
            _ => panic!("Wrong schedule type"),
        }

        // Every variant
        let schedule = CronSchedule::Every { every_ms: 60000 };
        let json = serde_json::to_value(&schedule).unwrap();
        assert_eq!(json["kind"], "every");
        assert_eq!(json["everyMs"], 60000);
        match serde_json::from_value::<CronSchedule>(json).unwrap() {
            CronSchedule::Every { every_ms } => assert_eq!(every_ms, 60000),
            _ => panic!("Wrong schedule type"),
        }

        // Cron variant
        let schedule = CronSchedule::Cron {
            expr: "0 0 * * *".to_string(),
            tz: Some("UTC".to_string()),
        };
        let json = serde_json::to_value(&schedule).unwrap();
        assert_eq!(json["kind"], "cron");
        assert_eq!(json["expr"], "0 0 * * *");
        assert_eq!(json["tz"], "UTC");
        match serde_json::from_value::<CronSchedule>(json).unwrap() {
            CronSchedule::Cron { expr, tz } => {
                assert_eq!(expr, "0 0 * * *");
                assert_eq!(tz, Some("UTC".to_string()));
            }
            _ => panic!("Wrong schedule type"),
        }
    }

    #[test]
    fn test_cron_payload_serialization() {
        let payload = CronPayload {
            kind: "agent_turn".to_string(),
            message: "Test message".to_string(),
            deliver: true,
            channel: Some("telegram".to_string()),
            to: Some("chat123".to_string()),
        };

        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["kind"], "agent_turn");
        assert_eq!(json["message"], "Test message");
        assert_eq!(json["deliver"], true);
        assert_eq!(json["channel"], "telegram");
        assert_eq!(json["to"], "chat123");
    }

    #[test]
    fn test_cron_job_serialization() {
        let schedule = CronSchedule::At { at_ms: 1234567890 };
        let job = CronJob::new("job1", "Test Job", schedule, "Test message");

        let json = serde_json::to_value(&job).unwrap();
        assert_eq!(json["id"], "job1");
        assert_eq!(json["name"], "Test Job");
        assert_eq!(json["enabled"], true);
        assert_eq!(json["schedule"]["kind"], "at");

        // Test round-trip
        let deserialized: CronJob = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.id, "job1");
        assert_eq!(deserialized.name, "Test Job");
    }

    #[test]
    fn test_cron_job_state() {
        let state = CronJobState {
            next_run_at_ms: Some(1234567890),
            last_run_at_ms: Some(1234567800),
            last_status: Some("success".to_string()),
            last_error: None,
        };

        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["nextRunAtMs"], 1234567890);
        assert_eq!(json["lastRunAtMs"], 1234567800);
        assert_eq!(json["lastStatus"], "success");
    }

    #[test]
    fn test_cron_store_serialization() {
        let schedule = CronSchedule::Every { every_ms: 60000 };
        let job = CronJob::new("job1", "Test Job", schedule, "Test");

        let store = CronStore {
            version: 1,
            jobs: vec![job],
        };

        let json = serde_json::to_value(&store).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["jobs"].as_array().unwrap().len(), 1);

        // Test round-trip
        let deserialized: CronStore = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.jobs.len(), 1);
        assert_eq!(deserialized.jobs[0].id, "job1");
    }

    #[test]
    fn test_cron_job_camel_case_serialization() {
        let schedule = CronSchedule::Every { every_ms: 60000 };
        let mut job = CronJob::new("job1", "Test", schedule, "Test");
        job.delete_after_run = true;

        let json = serde_json::to_string(&job).unwrap();
        // Verify camelCase in JSON
        assert!(json.contains("deleteAfterRun"));
        assert!(json.contains("createdAtMs"));
        assert!(json.contains("updatedAtMs"));
    }
}
