//! Shared task infrastructure types: working hours, attachments, time entries,
//! and scope overrides.

use jiff::{civil::Time, Timestamp};
use serde::{Deserialize, Serialize};
use storage::rows::task::*;

use super::entity::EnergyLevel;

/// Working hours configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingHours {
    /// Start of working day.
    pub start: Time,
    /// End of working day.
    pub end: Time,
    /// Start of lunch break.
    pub lunch_start: Time,
}

impl Default for WorkingHours {
    fn default() -> Self {
        Self {
            start: Time::new(9, 0, 0, 0).unwrap(),
            end: Time::new(17, 0, 0, 0).unwrap(),
            lunch_start: Time::new(12, 0, 0, 0).unwrap(),
        }
    }
}

/// Per-scope overrides for task management settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScopeOverrides {
    pub wip_limit: Option<u32>,
    pub stale_task_days: Option<u32>,
}

// ── Attachment & Time Entry ─────────────────────────────────────────────────

/// Attachment to a task (file, URL, or note).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub attachment_type: AttachmentType,
    pub title: Option<String>,
    pub value: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: Timestamp,
}

/// Type of attachment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    File,
    Url,
    Note,
}

impl AttachmentType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "file" => Some(Self::File),
            "url" => Some(Self::Url),
            "note" => Some(Self::Note),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Url => "url",
            Self::Note => "note",
        }
    }
}

impl From<TaskAttachmentRow> for Attachment {
    fn from(row: TaskAttachmentRow) -> Self {
        Self {
            id: row.id.to_string(),
            attachment_type: match row.attachment_type.as_str() {
                "file" => AttachmentType::File,
                "url" => AttachmentType::Url,
                _ => AttachmentType::Note,
            },
            title: row.title,
            value: row.value,
            tags: row.tags,
            created_at: *row.created_at,
        }
    }
}

/// Time tracking entry for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub id: String,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub duration_secs: Option<i64>,
    pub note: Option<String>,
    pub energy_level: Option<EnergyLevel>,
    #[serde(default)]
    pub source: TimeEntrySource,
}

/// Source of a time entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TimeEntrySource {
    #[default]
    Focus,
    Manual,
}

impl From<TaskTimeEntryRow> for TimeEntry {
    fn from(row: TaskTimeEntryRow) -> Self {
        Self {
            id: row.id.to_string(),
            started_at: *row.started_at,
            ended_at: row.ended_at.map(|ts| *ts),
            duration_secs: row.duration_secs,
            note: row.note,
            energy_level: row
                .energy_level
                .as_deref()
                .and_then(|s| s.parse::<EnergyLevel>().ok()),
            source: match row.source.as_str() {
                "manual" => TimeEntrySource::Manual,
                _ => TimeEntrySource::Focus,
            },
        }
    }
}
