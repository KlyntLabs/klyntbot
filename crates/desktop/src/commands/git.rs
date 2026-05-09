//! Tauri command surface for the git panel. Each function is a thin adapter
//! that delegates to `AppCore`. Errors flow through `?` (the macro wraps the
//! return type in `CommandResult<T>`).

use desktop_macros::klynt_command;
use desktop_shared::git::{
    BranchInfo, CreateGitHubRepoResponse, GitCommitDiff, GitFileDiff, GitLogResponse,
    GitStatusSummary, InitGitRepoResponse,
};

#[klynt_command]
pub async fn get_git_status(workspace_id: String) -> GitStatusSummary {
    state.get_git_status(&workspace_id).await
}

#[klynt_command]
pub async fn get_git_diffs(workspace_id: String) -> Vec<GitFileDiff> {
    state.get_git_diffs(&workspace_id).await
}

#[klynt_command]
pub async fn get_git_commit_diff(workspace_id: String, sha: String) -> Vec<GitCommitDiff> {
    state.get_git_commit_diff(&workspace_id, &sha).await
}

#[klynt_command]
pub async fn get_git_log(workspace_id: String, limit: u32) -> GitLogResponse {
    state.get_git_log(&workspace_id, limit).await
}

#[klynt_command]
pub async fn get_git_remote(workspace_id: String) -> Option<String> {
    state.get_git_remote(&workspace_id).await
}

#[klynt_command]
pub async fn list_git_branches(workspace_id: String) -> Vec<BranchInfo> {
    state.list_git_branches(&workspace_id).await
}

#[klynt_command]
pub async fn checkout_git_branch(workspace_id: String, name: String) -> () {
    state.checkout_git_branch(&workspace_id, &name).await
}

#[klynt_command]
pub async fn create_git_branch(workspace_id: String, name: String) -> () {
    state.create_git_branch(&workspace_id, &name).await
}

#[klynt_command]
pub async fn list_git_roots(workspace_id: String, depth: u32) -> Vec<String> {
    state.list_git_roots(&workspace_id, depth).await
}

#[klynt_command]
pub async fn init_git_repo(
    workspace_id: String,
    branch: String,
    force: bool,
) -> InitGitRepoResponse {
    state.init_git_repo(&workspace_id, &branch, force).await
}

#[klynt_command]
pub async fn stage_git_file(workspace_id: String, path: String) -> () {
    state.stage_git_file(&workspace_id, &path).await
}

#[klynt_command]
pub async fn stage_git_all(workspace_id: String) -> () {
    state.stage_git_all(&workspace_id).await
}

#[klynt_command]
pub async fn unstage_git_file(workspace_id: String, path: String) -> () {
    state.unstage_git_file(&workspace_id, &path).await
}

#[klynt_command]
pub async fn revert_git_file(workspace_id: String, path: String) -> () {
    state.revert_git_file(&workspace_id, &path).await
}

#[klynt_command]
pub async fn revert_git_all(workspace_id: String) -> () {
    state.revert_git_all(&workspace_id).await
}

#[klynt_command]
pub async fn commit_git(workspace_id: String, message: String) -> () {
    state.commit_git(&workspace_id, &message).await
}

#[klynt_command]
pub async fn push_git(workspace_id: String) -> () {
    state.push_git(&workspace_id).await
}

#[klynt_command]
pub async fn pull_git(workspace_id: String) -> () {
    state.pull_git(&workspace_id).await
}

#[klynt_command]
pub async fn fetch_git(workspace_id: String) -> () {
    state.fetch_git(&workspace_id).await
}

#[klynt_command]
pub async fn sync_git(workspace_id: String) -> () {
    state.sync_git(&workspace_id).await
}

#[klynt_command]
pub async fn generate_commit_message(
    workspace_id: String,
    commit_message_model_id: Option<String>,
) -> String {
    state
        .generate_commit_message(&workspace_id, commit_message_model_id.as_deref())
        .await
}

#[klynt_command]
pub async fn create_github_repo(
    workspace_id: String,
    repo: String,
    visibility: String,
    branch: Option<String>,
) -> CreateGitHubRepoResponse {
    state
        .create_github_repo(&workspace_id, &repo, &visibility, branch.as_deref())
        .await
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "get_git_status" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            dev::val(core.get_git_status(&workspace_id).await)
        }
        "get_git_diffs" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            dev::val(core.get_git_diffs(&workspace_id).await)
        }
        "get_git_commit_diff" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let sha = try_field!(dev::get_str(body, "sha"));
            dev::val(core.get_git_commit_diff(&workspace_id, &sha).await)
        }
        "get_git_log" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let limit = body.get("limit").and_then(|v| v.as_u64()).unwrap_or(40) as u32;
            dev::val(core.get_git_log(&workspace_id, limit).await)
        }
        "get_git_remote" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            dev::val(core.get_git_remote(&workspace_id).await)
        }
        "list_git_branches" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            dev::val(core.list_git_branches(&workspace_id).await)
        }
        "checkout_git_branch" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let name = try_field!(dev::get_str(body, "name"));
            dev::val(core.checkout_git_branch(&workspace_id, &name).await)
        }
        "create_git_branch" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let name = try_field!(dev::get_str(body, "name"));
            dev::val(core.create_git_branch(&workspace_id, &name).await)
        }
        "list_git_roots" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let depth = body.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as u32;
            dev::val(core.list_git_roots(&workspace_id, depth).await)
        }
        "init_git_repo" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let branch = try_field!(dev::get_str(body, "branch"));
            let force = body.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            dev::val(core.init_git_repo(&workspace_id, &branch, force).await)
        }
        "stage_git_file" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let path = try_field!(dev::get_str(body, "path"));
            dev::val(core.stage_git_file(&workspace_id, &path).await)
        }
        "stage_git_all" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            dev::val(core.stage_git_all(&workspace_id).await)
        }
        "unstage_git_file" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let path = try_field!(dev::get_str(body, "path"));
            dev::val(core.unstage_git_file(&workspace_id, &path).await)
        }
        "revert_git_file" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let path = try_field!(dev::get_str(body, "path"));
            dev::val(core.revert_git_file(&workspace_id, &path).await)
        }
        "revert_git_all" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            dev::val(core.revert_git_all(&workspace_id).await)
        }
        "commit_git" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let message = try_field!(dev::get_str(body, "message"));
            dev::val(core.commit_git(&workspace_id, &message).await)
        }
        "push_git" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            dev::val(core.push_git(&workspace_id).await)
        }
        "pull_git" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            dev::val(core.pull_git(&workspace_id).await)
        }
        "fetch_git" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            dev::val(core.fetch_git(&workspace_id).await)
        }
        "sync_git" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            dev::val(core.sync_git(&workspace_id).await)
        }
        "generate_commit_message" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let model = body
                .get("commitMessageModelId")
                .and_then(|v| v.as_str())
                .map(String::from);
            dev::val(
                core.generate_commit_message(&workspace_id, model.as_deref())
                    .await,
            )
        }
        "create_github_repo" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let repo = try_field!(dev::get_str(body, "repo"));
            let visibility = try_field!(dev::get_str(body, "visibility"));
            let branch = body
                .get("branch")
                .and_then(|v| v.as_str())
                .map(String::from);
            dev::val(
                core.create_github_repo(&workspace_id, &repo, &visibility, branch.as_deref())
                    .await,
            )
        }
        _ => return None,
    })
}
