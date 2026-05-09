//! Git-panel handler methods. Each method resolves `workspace_id` to its
//! filesystem path via `WorkspaceRepo`, then delegates to `klynt-git-utils`.
//! Errors from the git layer are mapped to `ApiError` with stable codes the
//! frontend can dispatch on (`GIT_NOT_REPO`, `GIT_COMMAND_FAILED`, etc.).

use std::path::PathBuf;

use desktop_shared::errors::ApiError;
use desktop_shared::git::{
    BranchInfo, CreateGitHubRepoResponse, GitCommitDiff, GitFileDiff, GitLogResponse,
    GitStatusSummary, InitGitRepoResponse,
};
use klynt_git_utils::{
    branches, commit, diff, index, init, log, remote, status, sync, GitToolingError,
};

use crate::state::AppCore;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn get_git_status(&self, workspace_id: &str) -> Result<GitStatusSummary, ApiError> {
        let path = self.workspace_path(workspace_id).await?;
        if !init::is_repo(&path).await.unwrap_or(false) {
            return Err(ApiError::new(
                "GIT_NOT_REPO",
                format!("{} is not a git repository", path.display()),
            ));
        }
        status::collect(&path).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn get_git_diffs(&self, workspace_id: &str) -> Result<Vec<GitFileDiff>, ApiError> {
        let path = self.workspace_path(workspace_id).await?;
        // Non-repos surface as empty (UI swaps to the init/pick-root affordance).
        Ok(diff::working_tree(&path).await.unwrap_or_default())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn get_git_commit_diff(
        &self,
        workspace_id: &str,
        sha: &str,
    ) -> Result<Vec<GitCommitDiff>, ApiError> {
        let path = self.workspace_path(workspace_id).await?;
        diff::for_commit(&path, sha).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn get_git_log(
        &self,
        workspace_id: &str,
        limit: u32,
    ) -> Result<GitLogResponse, ApiError> {
        let path = self.workspace_path(workspace_id).await?;
        Ok(log::collect(&path, limit)
            .await
            .unwrap_or_else(|_| empty_log()))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn get_git_remote(&self, workspace_id: &str) -> Result<Option<String>, ApiError> {
        let path = self.workspace_path(workspace_id).await?;
        Ok(remote::get_origin_url(&path).await.unwrap_or(None))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn list_git_branches(&self, workspace_id: &str) -> Result<Vec<BranchInfo>, ApiError> {
        let path = self.workspace_path(workspace_id).await?;
        Ok(branches::list(&path).await.unwrap_or_default())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn checkout_git_branch(
        &self,
        workspace_id: &str,
        name: &str,
    ) -> Result<(), ApiError> {
        let path = self.workspace_path(workspace_id).await?;
        branches::checkout(&path, name).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn create_git_branch(&self, workspace_id: &str, name: &str) -> Result<(), ApiError> {
        let path = self.workspace_path(workspace_id).await?;
        branches::create(&path, name).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn list_git_roots(
        &self,
        workspace_id: &str,
        depth: u32,
    ) -> Result<Vec<String>, ApiError> {
        let path = self.workspace_path(workspace_id).await?;
        let roots = init::list_roots(&path, depth).map_err(map_git_err)?;
        Ok(roots
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn init_git_repo(
        &self,
        workspace_id: &str,
        branch: &str,
        force: bool,
    ) -> Result<InitGitRepoResponse, ApiError> {
        let path = self.workspace_path(workspace_id).await?;
        let outcome = init::init(&path, branch, force)
            .await
            .map_err(map_git_err)?;
        Ok(match outcome {
            init::InitOutcome::Initialized { commit_error } => {
                InitGitRepoResponse::Initialized { commit_error }
            }
            init::InitOutcome::AlreadyInitialized => InitGitRepoResponse::AlreadyInitialized,
            init::InitOutcome::NeedsConfirmation { entry_count } => {
                InitGitRepoResponse::NeedsConfirmation { entry_count }
            }
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn stage_git_file(&self, workspace_id: &str, path: &str) -> Result<(), ApiError> {
        let repo = self.workspace_path(workspace_id).await?;
        index::stage_file(&repo, path).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn stage_git_all(&self, workspace_id: &str) -> Result<(), ApiError> {
        let repo = self.workspace_path(workspace_id).await?;
        index::stage_all(&repo).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn unstage_git_file(&self, workspace_id: &str, path: &str) -> Result<(), ApiError> {
        let repo = self.workspace_path(workspace_id).await?;
        index::unstage_file(&repo, path).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn revert_git_file(&self, workspace_id: &str, path: &str) -> Result<(), ApiError> {
        let repo = self.workspace_path(workspace_id).await?;
        index::revert_file(&repo, path).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn revert_git_all(&self, workspace_id: &str) -> Result<(), ApiError> {
        let repo = self.workspace_path(workspace_id).await?;
        index::revert_all(&repo).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn commit_git(&self, workspace_id: &str, message: &str) -> Result<(), ApiError> {
        let repo = self.workspace_path(workspace_id).await?;
        commit::commit(&repo, message).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn push_git(&self, workspace_id: &str) -> Result<(), ApiError> {
        let repo = self.workspace_path(workspace_id).await?;
        sync::push(&repo).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn pull_git(&self, workspace_id: &str) -> Result<(), ApiError> {
        let repo = self.workspace_path(workspace_id).await?;
        sync::pull(&repo).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn fetch_git(&self, workspace_id: &str) -> Result<(), ApiError> {
        let repo = self.workspace_path(workspace_id).await?;
        sync::fetch(&repo).await.map_err(map_git_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn sync_git(&self, workspace_id: &str) -> Result<(), ApiError> {
        let repo = self.workspace_path(workspace_id).await?;
        sync::sync(&repo).await.map_err(map_git_err)
    }

    /// Generate a commit message from the staged (or worktree) diff.
    ///
    /// Currently returns a `NOT_IMPLEMENTED` error: this command is wired end-
    /// to-end (frontend button → IPC → AppCore → diff collection) but the LLM
    /// hop is intentionally absent. Wiring it requires picking a cognitive
    /// provider through `ProviderManager`, which depends on the user-selected
    /// `commitMessageModelId` and the config's cognitive provider — both
    /// already accessible from `AppCore`. Replace the body with a real
    /// `summarize_text` call once the provider plumbing lands.
    #[tracing::instrument(skip(self), err)]
    pub async fn generate_commit_message(
        &self,
        workspace_id: &str,
        _commit_message_model_id: Option<&str>,
    ) -> Result<String, ApiError> {
        let repo = self.workspace_path(workspace_id).await?;
        let ctx = commit::collect_commit_context(&repo)
            .await
            .map_err(map_git_err)?;
        if ctx.diff.trim().is_empty() {
            return Err(ApiError::new(
                "GIT_NOTHING_TO_COMMIT",
                "No staged or unstaged changes to summarize",
            ));
        }
        Err(ApiError::new(
            "NOT_IMPLEMENTED",
            "Commit-message generation isn't wired to a provider yet — write the message manually for now.",
        ))
    }

    /// Workspace-id → absolute repo path. The DB row stores an absolute path
    /// at insert time, so no canonicalization is needed here.
    pub(crate) async fn workspace_path(&self, workspace_id: &str) -> Result<PathBuf, ApiError> {
        let ws = self
            .repos
            .workspaces
            .get(workspace_id)
            .await
            .map_err(|e| ApiError::new("WORKSPACE_NOT_FOUND", e.to_string()))?;
        Ok(PathBuf::from(ws.path))
    }

    /// Create a GitHub repo via the `gh` CLI when available, set `origin`,
    /// and push. Each step is best-effort — failures get surfaced through the
    /// `Partial` variant so the user sees what worked and what didn't.
    #[tracing::instrument(skip(self), err)]
    pub async fn create_github_repo(
        &self,
        workspace_id: &str,
        repo: &str,
        visibility: &str,
        branch: Option<&str>,
    ) -> Result<CreateGitHubRepoResponse, ApiError> {
        let repo_dir = self.workspace_path(workspace_id).await?;
        if !init::is_repo(&repo_dir).await.unwrap_or(false) {
            return Err(ApiError::new(
                "GIT_NOT_REPO",
                "Initialize the repo before publishing to GitHub",
            ));
        }
        let visibility_flag = match visibility {
            "private" => "--private",
            "public" => "--public",
            other => {
                return Err(ApiError::new(
                    "INVALID_PARAMS",
                    format!("unknown visibility {other:?}; expected 'private' or 'public'"),
                ));
            }
        };
        let mut args: Vec<&str> = vec!["repo", "create", repo, visibility_flag, "--source", "."];
        if let Some(b) = branch {
            args.push("--default-branch");
            args.push(b);
        }
        // Attempt `gh repo create`. If `gh` isn't installed, fall back to
        // returning a structured error.
        let out = tokio::process::Command::new("gh")
            .args(&args)
            .current_dir(&repo_dir)
            .output()
            .await;
        let out = match out {
            Ok(o) => o,
            Err(e) => {
                return Err(ApiError::new(
                    "GH_CLI_UNAVAILABLE",
                    format!("`gh` CLI not available: {e}"),
                ));
            }
        };
        if !out.status.success() {
            return Err(ApiError::new(
                "GH_CREATE_FAILED",
                String::from_utf8_lossy(&out.stderr).into_owned(),
            ));
        }
        let remote_url = remote::get_origin_url(&repo_dir).await.unwrap_or(None);
        // Try a push; record failure rather than failing the whole call.
        let push_result = sync::push(&repo_dir).await;
        match push_result {
            Ok(()) => Ok(CreateGitHubRepoResponse::Ok {
                repo: repo.to_string(),
                remote_url,
            }),
            Err(e) => Ok(CreateGitHubRepoResponse::Partial {
                repo: repo.to_string(),
                remote_url,
                push_error: Some(e.to_string()),
                default_branch_error: None,
            }),
        }
    }
}

fn map_git_err(err: GitToolingError) -> ApiError {
    match &err {
        GitToolingError::NotAGitRepository { path } => ApiError::new(
            "GIT_NOT_REPO",
            format!("{} is not a git repository", path.display()),
        ),
        GitToolingError::GitCommand { stderr, .. } => {
            ApiError::new("GIT_COMMAND_FAILED", stderr.clone())
        }
        _ => ApiError::new("GIT_ERROR", err.to_string()),
    }
}

fn empty_log() -> GitLogResponse {
    GitLogResponse {
        total: 0,
        entries: Vec::new(),
        ahead: 0,
        behind: 0,
        ahead_entries: Vec::new(),
        behind_entries: Vec::new(),
        upstream: None,
    }
}
