//! Wire-format types for the git Tauri commands. The data-shape types
//! (`GitFileStatus`, `GitFileDiff`, …) live in `klynt-git-utils::types` so
//! the L0 git operations can produce them directly with no mapping layer;
//! we just re-export. The two discriminated-union response enums below are
//! API-only: they describe lifecycle outcomes, not git data, so they stay
//! here.

use serde::{Deserialize, Serialize};

pub use klynt_git_utils::types::{
    BranchInfo, GitCommitDiff, GitFileDiff, GitFileStatus, GitLogEntry, GitLogResponse,
    GitStatusSummary,
};

/// Tagged-union response for `init_git_repo`. The `tag = "status"` + per-variant
/// renames produce the discriminator the frontend's `InitGitRepoResponse` union
/// expects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InitGitRepoResponse {
    Initialized {
        #[serde(
            rename = "commitError",
            skip_serializing_if = "Option::is_none",
            default
        )]
        commit_error: Option<String>,
    },
    AlreadyInitialized,
    NeedsConfirmation {
        #[serde(rename = "entryCount")]
        entry_count: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CreateGitHubRepoResponse {
    Ok {
        repo: String,
        #[serde(rename = "remoteUrl", skip_serializing_if = "Option::is_none", default)]
        remote_url: Option<String>,
    },
    Partial {
        repo: String,
        #[serde(rename = "remoteUrl", skip_serializing_if = "Option::is_none", default)]
        remote_url: Option<String>,
        #[serde(rename = "pushError", skip_serializing_if = "Option::is_none", default)]
        push_error: Option<String>,
        #[serde(
            rename = "defaultBranchError",
            skip_serializing_if = "Option::is_none",
            default
        )]
        default_branch_error: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn status_summary_camelcases_field_names() {
        let summary = GitStatusSummary {
            branch_name: "main".into(),
            files: vec![],
            staged_files: vec![],
            unstaged_files: vec![],
            total_additions: 0,
            total_deletions: 0,
        };
        let v = serde_json::to_value(&summary).unwrap();
        assert!(v.get("branchName").is_some(), "expected camelCase key");
        assert!(v.get("totalAdditions").is_some());
        assert!(v.get("totalDeletions").is_some());
    }

    #[test]
    fn file_diff_omits_optional_image_fields_when_absent() {
        let d = GitFileDiff {
            path: "src/lib.rs".into(),
            diff: "@@".into(),
            old_lines: None,
            new_lines: None,
            is_binary: None,
            is_image: None,
            old_image_data: None,
            new_image_data: None,
            old_image_mime: None,
            new_image_mime: None,
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(
            v,
            json!({
                "path": "src/lib.rs",
                "diff": "@@"
            })
        );
    }

    #[test]
    fn init_repo_response_serializes_with_status_tag() {
        let already = serde_json::to_value(&InitGitRepoResponse::AlreadyInitialized).unwrap();
        assert_eq!(already, json!({ "status": "already_initialized" }));

        let needs =
            serde_json::to_value(&InitGitRepoResponse::NeedsConfirmation { entry_count: 7 })
                .unwrap();
        assert_eq!(
            needs,
            json!({ "status": "needs_confirmation", "entryCount": 7 })
        );

        let ok = serde_json::to_value(&InitGitRepoResponse::Initialized {
            commit_error: Some("hook failed".into()),
        })
        .unwrap();
        assert_eq!(
            ok,
            json!({ "status": "initialized", "commitError": "hook failed" })
        );
    }
}
