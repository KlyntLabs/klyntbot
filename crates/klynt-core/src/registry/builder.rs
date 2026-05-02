//! Dependency-injection builder for klynt-core primitive tools.
//!
//! Both the main agent and sub-agents construct tool registries through this
//! builder, avoiding the proliferation of 6-7-argument constructor call sites.

use std::path::PathBuf;
use std::sync::Arc;
use tools_core::ToolRegistry;

use crate::approval::{HostApprovalCache, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use crate::tools::{
    apply_patch::ApplyPatchTool,
    ask_user::AskUserTool,
    bash::BashTool,
    edit::EditTool,
    glob::GlobTool,
    grep::GrepTool,
    list_dir::ListDirTool,
    notebook_edit::NotebookEditTool,
    plan_mode::{EnterPlanModeTool, ExitPlanModeTool},
    read::ReadTool,
    recall_stubs::{
        CheckDeadEndsTool, RecallChangeHistoryTool, RecallDecisionPointsTool, RecallFactsAsOfTool,
        RecallFetchTool, RecallIndexTool, RecallTimelineTool, TraceCausesTool,
    },
    tool_search::ToolSearchTool,
    web_fetch::WebFetchTool,
    write::WriteTool,
};
use bus::DomainEventBus;
use klynt_execpolicy::Policy;
use storage::{Repos, repos::CodingApprovalHistoryRepo};

/// Shared dependencies for constructing the 13 klynt-core primitive tools.
#[derive(Clone)]
pub struct ToolKitBuilder {
    pub cwd: PathBuf,
    pub layer1: Arc<crate::approval::Layer1>,
    pub policy: Arc<Policy>,
    pub privacy: Arc<PrivacyGuard>,
    pub pending: Arc<PendingApprovalsMap>,
    pub bus: Arc<DomainEventBus>,
    pub repos: Repos,
    pub host_cache: Arc<HostApprovalCache>,
    pub non_ui_policy: common::tool_channel::NonUiPolicy,
    pub hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
    pub snapshot_repo: Option<Arc<crate::snapshots::SnapshotRepo>>,
    pub session_key: String,
    pub history_repo: Option<Arc<CodingApprovalHistoryRepo>>,
    pub mirror_learning_enabled: bool,
    pub mirror_min_approvals: u32,
    pub mirror_cooldown_seconds: i64,
    pub repo_id: String,
}

impl ToolKitBuilder {
    /// Override the working directory (used by sub-agents with a different cwd).
    pub fn with_cwd(self, cwd: PathBuf) -> Self {
        Self { cwd, ..self }
    }

    /// Six read-only / network / interaction tools (default `ChannelMask::ALL`).
    pub fn register_read_only(&self, reg: &mut ToolRegistry) {
        reg.register(ReadTool::new(self.cwd.clone(), self.privacy.clone()));
        reg.register(ListDirTool::new(self.cwd.clone(), self.privacy.clone()));
        reg.register(GlobTool::new(self.cwd.clone(), self.privacy.clone()));
        reg.register(GrepTool::new(self.cwd.clone(), self.privacy.clone()));
        reg.register(ToolSearchTool::new());
        reg.register(AskUserTool);
        let mut web = WebFetchTool::new(
            self.layer1.clone(),
            self.policy.clone(),
            self.privacy.clone(),
            self.pending.clone(),
            self.bus.clone(),
            self.non_ui_policy,
            self.host_cache.clone(),
        );
        web.history_repo = self.history_repo.clone();
        web.mirror_learning_enabled = self.mirror_learning_enabled;
        web.mirror_min_approvals = self.mirror_min_approvals;
        web.mirror_cooldown_seconds = self.mirror_cooldown_seconds;
        web.repo_id = self.repo_id.clone();
        reg.register(web);
    }

    /// Five mutating tools (`ChannelMask::CODING_ONLY`).
    pub fn register_mutating(&self, reg: &mut ToolRegistry) {
        let mut bash = BashTool::new(
            self.layer1.clone(),
            self.policy.clone(),
            self.privacy.clone(),
            self.pending.clone(),
            self.bus.clone(),
            self.non_ui_policy,
        );
        bash.history_repo = self.history_repo.clone();
        bash.mirror_learning_enabled = self.mirror_learning_enabled;
        bash.mirror_min_approvals = self.mirror_min_approvals;
        bash.mirror_cooldown_seconds = self.mirror_cooldown_seconds;
        bash.repo_id = self.repo_id.clone();
        reg.register(bash);
        let mut write = WriteTool::new(
            self.cwd.clone(),
            self.layer1.clone(),
            self.policy.clone(),
            self.privacy.clone(),
            self.pending.clone(),
            self.bus.clone(),
            self.non_ui_policy,
        );
        write.snapshot_repo = self.snapshot_repo.clone();
        write.history_repo = self.history_repo.clone();
        write.mirror_learning_enabled = self.mirror_learning_enabled;
        write.mirror_min_approvals = self.mirror_min_approvals;
        write.mirror_cooldown_seconds = self.mirror_cooldown_seconds;
        write.repo_id = self.repo_id.clone();
        reg.register(write);
        let mut edit = EditTool::new(
            self.cwd.clone(),
            self.layer1.clone(),
            self.policy.clone(),
            self.privacy.clone(),
            self.pending.clone(),
            self.bus.clone(),
            self.non_ui_policy,
        );
        edit.snapshot_repo = self.snapshot_repo.clone();
        edit.history_repo = self.history_repo.clone();
        edit.mirror_learning_enabled = self.mirror_learning_enabled;
        edit.mirror_min_approvals = self.mirror_min_approvals;
        edit.mirror_cooldown_seconds = self.mirror_cooldown_seconds;
        edit.repo_id = self.repo_id.clone();
        reg.register(edit);
        let mut patch = ApplyPatchTool::new(
            self.cwd.clone(),
            self.layer1.clone(),
            self.policy.clone(),
            self.privacy.clone(),
            self.pending.clone(),
            self.bus.clone(),
            self.non_ui_policy,
        );
        patch.snapshot_repo = self.snapshot_repo.clone();
        patch.history_repo = self.history_repo.clone();
        patch.mirror_learning_enabled = self.mirror_learning_enabled;
        patch.mirror_min_approvals = self.mirror_min_approvals;
        patch.mirror_cooldown_seconds = self.mirror_cooldown_seconds;
        patch.repo_id = self.repo_id.clone();
        reg.register(patch);
        let mut notebook = NotebookEditTool::new(
            self.cwd.clone(),
            self.layer1.clone(),
            self.policy.clone(),
            self.privacy.clone(),
            self.pending.clone(),
            self.bus.clone(),
            self.non_ui_policy,
        );
        notebook.snapshot_repo = self.snapshot_repo.clone();
        notebook.history_repo = self.history_repo.clone();
        notebook.mirror_learning_enabled = self.mirror_learning_enabled;
        notebook.mirror_min_approvals = self.mirror_min_approvals;
        notebook.mirror_cooldown_seconds = self.mirror_cooldown_seconds;
        notebook.repo_id = self.repo_id.clone();
        reg.register(notebook);
    }

    /// Two plan-mode tools (`ChannelMask::CODING_ONLY`).
    pub fn register_plan_mode(&self, reg: &mut ToolRegistry) {
        reg.register(EnterPlanModeTool::new(self.repos.clone(), self.bus.clone()));
        reg.register(ExitPlanModeTool::new(self.repos.clone(), self.bus.clone()));
    }

    /// Eight recall coding-memory tools (stub when service unavailable).
    pub fn register_recall(&self, reg: &mut ToolRegistry) {
        reg.register(RecallIndexTool);
        reg.register(RecallTimelineTool);
        reg.register(RecallFetchTool);
        reg.register(TraceCausesTool);
        reg.register(CheckDeadEndsTool);
        reg.register(RecallFactsAsOfTool);
        reg.register(RecallChangeHistoryTool);
        reg.register(RecallDecisionPointsTool);
    }

    /// All thirteen klynt-core primitive tools plus recall stubs.
    pub fn register_all(&self, reg: &mut ToolRegistry) {
        self.register_read_only(reg);
        self.register_mutating(reg);
        self.register_plan_mode(reg);
        self.register_recall(reg);
    }

    /// Builder-style no-op for API compatibility.
    pub fn with_recall_tools(self) -> Self {
        self
    }
}
