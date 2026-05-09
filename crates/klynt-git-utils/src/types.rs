//! Wire-format types for the git Tauri commands. These travel directly to
//! the desktop UI as IPC payloads, so they live here in L0 with `serde` +
//! `specta::Type` derives. The `desktop-shared::git` module re-exports them.
//!
//! Field shape mirrors the frontend's `desktop-ui/src/types.ts`. Optional
//! image/binary fields use `skip_serializing_if = "Option::is_none"` so the
//! JSON matches the optional-field contract on the TS side.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitFileStatus {
    pub path: String,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusSummary {
    pub branch_name: String,
    pub files: Vec<GitFileStatus>,
    pub staged_files: Vec<GitFileStatus>,
    pub unstaged_files: Vec<GitFileStatus>,
    pub total_additions: u32,
    pub total_deletions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitFileDiff {
    pub path: String,
    pub diff: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub old_lines: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub new_lines: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub is_binary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub is_image: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub old_image_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub new_image_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub old_image_mime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub new_image_mime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitDiff {
    pub path: String,
    pub status: String,
    pub diff: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub old_lines: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub new_lines: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub is_binary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub is_image: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub old_image_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub new_image_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub old_image_mime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub new_image_mime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitLogEntry {
    pub sha: String,
    pub summary: String,
    pub author: String,
    /// Author timestamp in seconds since the Unix epoch.
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitLogResponse {
    pub total: u32,
    pub entries: Vec<GitLogEntry>,
    pub ahead: u32,
    pub behind: u32,
    pub ahead_entries: Vec<GitLogEntry>,
    pub behind_entries: Vec<GitLogEntry>,
    pub upstream: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    /// Last commit timestamp on this branch (seconds since Unix epoch).
    pub last_commit: i64,
}
