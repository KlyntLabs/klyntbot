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
