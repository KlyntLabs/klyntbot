//! Cron job types and structures.

use jiff::Timestamp;
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

/// Origin of a cron job — who created it
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CronOrigin {
    #[default]
    System,
    User,
    Ai,
    Plugin,
}

/// Optional intent window — controls when a job actually fires relative to its cron schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentWindow {
    pub trigger: IntentTrigger,
    #[serde(rename = "toleranceSecs", with = "duration_secs")]
    pub tolerance: std::time::Duration,
    #[serde(rename = "catchUp")]
    pub catch_up: CatchUpPriority,
}

/// What must be true for an intent-windowed job to fire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IntentTrigger {
    UserPresent,
    FirstActivityAfter {
        #[serde(rename = "afterLocal")]
        after_local: jiff::civil::Time,
    },
    MinActiveMinutes {
        minutes: u32,
    },
    UserIdle {
        #[serde(rename = "minIdleSecs")]
        min_idle_secs: u64,
    },
}

/// Priority for catch-up after sleep/idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatchUpPriority {
    Immediate,
    WhenPresent,
    WhenIdle,
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(d.as_secs())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::from_secs(u64::deserialize(d)?))
    }
}

/// A scheduled job
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJob {
    pub id: String,
    pub name: String,

    #[serde(default = "default_enabled")]
    pub enabled: bool,

    #[serde(default)]
    pub origin: CronOrigin,

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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_window: Option<IntentWindow>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_pending_since_ms: Option<i64>,
}

fn default_enabled() -> bool {
    true
}

impl CronJob {
    /// Create a new cron job
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        schedule: CronSchedule,
        message: impl Into<String>,
        origin: CronOrigin,
    ) -> Self {
        let now_ms = Timestamp::now().as_millisecond();
        Self {
            id: id.into(),
            name: name.into(),
            enabled: true,
            origin,
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
            intent_window: None,
            intent_pending_since_ms: None,
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
        let job = CronJob::new(
            "job1",
            "Test Job",
            schedule,
            "Test message",
            CronOrigin::System,
        );

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
    fn test_cron_job_camel_case_serialization() {
        let schedule = CronSchedule::Every { every_ms: 60000 };
        let mut job = CronJob::new("job1", "Test", schedule, "Test", CronOrigin::System);
        job.delete_after_run = true;

        let json = serde_json::to_string(&job).unwrap();
        // Verify camelCase in JSON
        assert!(json.contains("deleteAfterRun"));
        assert!(json.contains("createdAtMs"));
        assert!(json.contains("updatedAtMs"));
    }

    #[test]
    fn test_cron_origin_serde() {
        let origin = CronOrigin::Ai;
        let json = serde_json::to_value(&origin).unwrap();
        assert_eq!(json, "ai");
        let deserialized: CronOrigin = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, CronOrigin::Ai);
    }

    #[test]
    fn test_intent_window_serde() {
        let window = IntentWindow {
            trigger: IntentTrigger::UserPresent,
            tolerance: std::time::Duration::from_secs(7200),
            catch_up: CatchUpPriority::WhenPresent,
        };
        let json = serde_json::to_value(&window).unwrap();
        assert_eq!(json["trigger"]["kind"], "user_present");
        assert_eq!(json["toleranceSecs"], 7200);
        assert_eq!(json["catchUp"], "when_present");

        let roundtrip: IntentWindow = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.tolerance.as_secs(), 7200);
    }

    #[test]
    fn test_intent_trigger_first_activity_after() {
        let trigger = IntentTrigger::FirstActivityAfter {
            after_local: jiff::civil::Time::constant(8, 0, 0, 0),
        };
        let json = serde_json::to_value(&trigger).unwrap();
        assert_eq!(json["kind"], "first_activity_after");
        assert_eq!(json["afterLocal"], "08:00:00");
        let roundtrip: IntentTrigger = serde_json::from_value(json).unwrap();
        assert!(matches!(
            roundtrip,
            IntentTrigger::FirstActivityAfter { after_local } if after_local == jiff::civil::Time::constant(8, 0, 0, 0)
        ));
    }

    #[test]
    fn test_cron_job_with_intent_window() {
        let schedule = CronSchedule::Cron {
            expr: "0 0 9 * * 1".to_string(),
            tz: None,
        };
        let mut job = CronJob::new("j1", "Weekly reflection", schedule, "", CronOrigin::System);
        job.intent_window = Some(IntentWindow {
            trigger: IntentTrigger::FirstActivityAfter {
                after_local: jiff::civil::Time::constant(8, 0, 0, 0),
            },
            tolerance: std::time::Duration::from_secs(7200),
            catch_up: CatchUpPriority::WhenPresent,
        });

        let json = serde_json::to_value(&job).unwrap();
        assert!(json["intentWindow"].is_object());

        let roundtrip: CronJob = serde_json::from_value(json).unwrap();
        assert!(roundtrip.intent_window.is_some());
    }
}
