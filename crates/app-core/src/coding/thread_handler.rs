use crate::AppCore;
use coding_agents_md::WorkspaceAgentsSource;
use common::Result;
use desktop_shared::coding::{
    ApprovalPolicy, InstructionSource, MessageDto, SandboxKind, Thread, ThreadSummary,
};
use std::path::PathBuf;
use storage::messages::parts::MessagePart;

impl AppCore {
    /// Start a new coding thread in the given workspace.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_start(
        &self,
        workspace_id: &str,
        model: Option<String>,
        approval_policy: Option<ApprovalPolicy>,
        ephemeral: bool,
    ) -> Result<Thread> {
        // 1. Load workspace
        let ws = self.repos.workspaces.get(workspace_id).await?;

        // 2. Privacy: refuse risky workspace paths
        validate_workspace_path(&ws.path)?;

        // 3. Resolve provider/model/policy chain
        let config = self.config.read().await;
        let resolved_model = model.or_else(|| {
            let m = config.agents.defaults.model.clone();
            if m.is_empty() {
                None
            } else {
                Some(m)
            }
        });
        drop(config);
        let resolved_policy = approval_policy.unwrap_or_default();

        // 4. Resolve sandbox profile
        let sandbox = resolve_sandbox();

        let workspace_path = std::path::Path::new(&ws.path);
        let agents_walker = coding_agents_md::WorkspaceAgentsSource::new(workspace_path.into());
        let agents_sources = agents_walker.walk();
        let instruction_sources: Vec<InstructionSource> = agents_sources
            .iter()
            .map(|s| InstructionSource {
                path: s.path.to_string_lossy().into_owned(),
                bytes: s.contents.len() as u64,
                is_global: false,
            })
            .collect();

        // 6. Allocate session
        let session_key = format!("coding:{}", uuid::Uuid::new_v4());
        let metadata = serde_json::json!({
            "channel": "coding",
            "workspace_id": ws.id,
            "approval_policy": format!("{resolved_policy:?}"),
            "ephemeral": ephemeral,
            "model": resolved_model,
        });
        self.repos
            .sessions
            .upsert_session_with_mode(&session_key, common::SessionMode::Coding, &metadata)
            .await?;
        self.repos
            .sessions
            .set_workspace_id(&session_key, &ws.id)
            .await?;
        self.repos
            .sessions
            .set_ephemeral(&session_key, ephemeral)
            .await?;

        // 7. Inject AGENTS.md bundle as synthetic user message
        if !agents_sources.is_empty() {
            let bundle = coding_agents_md::format_agents_md_bundle(&agents_sources);
            let agents_msg_id = uuid::Uuid::new_v4();
            let agents_parts = vec![MessagePart::Text { text: bundle }];
            self.repos
                .sessions
                .add_message_with_parts(
                    &session_key,
                    agents_msg_id,
                    "user",
                    &agents_parts,
                    None,
                    None,
                )
                .await?;
        }

        let now = jiff::Timestamp::now().as_millisecond();

        // 7. Build Thread DTO
        Ok(Thread {
            id: session_key,
            workspace_id: ws.id,
            cwd: ws.path,
            model: resolved_model,
            approval_policy: resolved_policy,
            sandbox,
            instruction_sources,
            created_at: now,
            updated_at: now,
            title: None,
            starred: false,
            archived_at: None,
            ephemeral,
            forked_from_id: None,
            summary_message_id: None,
            total_cost_usd: 0.0,
            total_tokens: 0,
            items: vec![],
        })
    }

    /// Resume an existing coding thread.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_resume(
        &self,
        thread_id: &str,
        include_items: bool,
    ) -> Result<Thread> {
        let session = self.repos.sessions.get_session(thread_id).await?;

        let ws_id = session.workspace_id.as_deref().ok_or_else(|| {
            common::KlyntbotError::StorageNotFound("session has no workspace".into())
        })?;

        let ws = self.repos.workspaces.get(ws_id).await?;

        let items = if include_items {
            self.repos
                .sessions
                .get_messages_parts(thread_id, 1000)
                .await?
                .into_iter()
                .map(message_with_parts_to_dto)
                .collect()
        } else {
            vec![]
        };

        let meta = session.metadata;
        let resolved_model = meta.get("model").and_then(|v| v.as_str()).map(String::from);
        let resolved_policy = meta
            .get("approval_policy")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "AskAlways" => Some(ApprovalPolicy::AskAlways),
                "AskOnRisky" => Some(ApprovalPolicy::AskOnRisky),
                "AskOnFailure" => Some(ApprovalPolicy::AskOnFailure),
                "YoloMode" => Some(ApprovalPolicy::YoloMode),
                _ => None,
            })
            .unwrap_or_default();
        let title = meta.get("title").and_then(|v| v.as_str()).map(String::from);
        let starred = meta
            .get("starred")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(Thread {
            id: session.key,
            workspace_id: ws.id,
            cwd: ws.path,
            model: resolved_model,
            approval_policy: resolved_policy,
            sandbox: resolve_sandbox(),
            instruction_sources: vec![],
            created_at: session.created_at.as_millisecond(),
            updated_at: session.updated_at.as_millisecond(),
            title,
            starred,
            archived_at: session.archived_at,
            ephemeral: session.ephemeral != 0,
            forked_from_id: session.forked_from_id,
            summary_message_id: session.summary_message_id,
            total_cost_usd: session.total_cost_usd,
            total_tokens: session.total_tokens,
            items,
        })
    }

    /// Read messages from a coding thread with cursor-based pagination.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_read(
        &self,
        thread_id: &str,
        _cursor: Option<String>,
        limit: Option<i64>,
    ) -> Result<Thread> {
        // For now, delegate to resume with items. Cursor support can be added later.
        self.coding_thread_resume(thread_id, true)
            .await
            .map(|mut t| {
                if let Some(lim) = limit {
                    t.items.truncate(lim as usize);
                }
                t
            })
    }

    /// List coding threads for a workspace, or all coding threads if no workspace is given.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_list(
        &self,
        workspace_id: Option<&str>,
        _cursor: Option<String>,
        limit: Option<i64>,
        _sort_key: Option<String>,
    ) -> Result<Vec<ThreadSummary>> {
        let sessions = self
            .repos
            .sessions
            .list_sessions_by_mode(common::SessionMode::Coding)
            .await?;

        let summaries: Vec<ThreadSummary> = sessions
            .into_iter()
            .filter(|s| {
                workspace_id.map_or(true, |wid| {
                    s.metadata.get("workspace_id").and_then(|v| v.as_str()) == Some(wid)
                })
            })
            .filter(|s| {
                // Exclude archived unless explicitly requested
                s.metadata.get("archived_at").is_none()
            })
            .take(limit.unwrap_or(50) as usize)
            .map(|s| {
                let title = s
                    .metadata
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let wid = s
                    .metadata
                    .get("workspace_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                ThreadSummary {
                    id: s.key,
                    title,
                    workspace_id: wid,
                    message_count: s.message_count,
                    total_cost_usd: 0.0,
                    created_at: s.created_at.as_millisecond(),
                    updated_at: s.updated_at.as_millisecond(),
                    archived_at: None,
                }
            })
            .collect();

        Ok(summaries)
    }

    /// Fork a coding thread.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_fork(
        &self,
        thread_id: &str,
        from_message_id: Option<String>,
    ) -> Result<Thread> {
        let new_key = self
            .repos
            .sessions
            .fork_session(thread_id, from_message_id.as_deref())
            .await?;

        // Set forked_from_id
        let pool = self.repos.pool();
        sqlx::query("UPDATE sessions SET forked_from_id = ?1 WHERE key = ?2")
            .bind(thread_id)
            .bind(&new_key)
            .execute(pool)
            .await?;

        self.coding_thread_resume(&new_key, true).await
    }

    /// Compact a coding thread by summarizing older messages.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_compact(&self, thread_id: &str) -> Result<serde_json::Value> {
        // For now, just return a placeholder. Full compaction uses the provider to summarize.
        let summary_id = format!("summary-{}", uuid::Uuid::new_v4());
        let pool = self.repos.pool();

        // Insert a placeholder summary message
        let parts = vec![MessagePart::Text {
            text: "[Thread compacted — summary not yet implemented]".into(),
        }];
        let parts_json = serde_json::to_string(&parts)?;
        sqlx::query(
            "INSERT INTO session_messages (id, session_key, role, content, parts, timestamp) \
             VALUES (?1, ?2, 'system', '', ?3, unixepoch('now') * 1000)",
        )
        .bind(&summary_id)
        .bind(thread_id)
        .bind(&parts_json)
        .execute(pool)
        .await?;

        // Set summary_message_id
        sqlx::query("UPDATE sessions SET summary_message_id = ?1 WHERE key = ?2")
            .bind(&summary_id)
            .bind(thread_id)
            .execute(pool)
            .await?;

        Ok(serde_json::json!({ "summaryMessageId": summary_id }))
    }

    /// Archive a coding thread.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_archive(&self, thread_id: &str) -> Result<()> {
        self.repos.sessions.archive(thread_id).await?;
        Ok(())
    }

    /// Set the name/title of a coding thread.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_set_name(&self, thread_id: &str, name: &str) -> Result<()> {
        self.repos.sessions.rename_session(thread_id, name).await?;
        Ok(())
    }

    /// Refresh AGENTS.md synthetic message for a thread.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_refresh_agents_md(
        &self,
        thread_id: &str,
    ) -> Result<Vec<coding_agents_md::AgentsMdSource>> {
        let session = self
            .repos
            .sessions
            .get_session(thread_id)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let workspace_id = session
            .workspace_id
            .as_deref()
            .ok_or_else(|| common::KlyntbotError::Storage("not a coding session".into()))?;
        let workspace = self
            .repos
            .workspaces
            .get(workspace_id)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let global_path = self
            .config
            .read()
            .await
            .data_dir_path()
            .join("AGENTS.md");

        let source =
            WorkspaceAgentsSource::new(PathBuf::from(&workspace.path)).with_global(global_path);
        let new_sources = source.walk();

        if let Some(bundle) = source.build_bundle() {
            self.repos
                .sessions
                .update_synthetic_agents_md(&session.key, &bundle)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        }

        Ok(new_sources)
    }
}

fn validate_workspace_path(path: &str) -> Result<()> {
    let dangerous = ["/", "/etc", "/usr", "/Users", "/home"];
    let patterns = [".ssh", ".aws", ".gnupg"];
    let p = std::path::Path::new(path);
    let canonical = p
        .canonicalize()
        .map_err(|e| common::KlyntbotError::Storage(format!("invalid path: {e}")))?;
    let s = canonical.to_string_lossy();
    for d in dangerous {
        if s == d {
            return Err(common::KlyntbotError::Storage(format!(
                "path is dangerous: {s}"
            )));
        }
    }
    for pat in patterns {
        if s.contains(pat) {
            return Err(common::KlyntbotError::Storage(format!(
                "path contains forbidden pattern: {pat}"
            )));
        }
    }
    Ok(())
}

fn resolve_sandbox() -> SandboxKind {
    if cfg!(target_os = "macos") {
        SandboxKind::MacosSeatbelt
    } else if cfg!(target_os = "linux") {
        SandboxKind::LinuxBwrapLandlock
    } else {
        SandboxKind::Disabled
    }
}

fn message_with_parts_to_dto(m: storage::repos::session::SessionMessageWithParts) -> MessageDto {
    MessageDto {
        id: m.id,
        session_id: m.session_key,
        role: m.role,
        parts: m
            .parts
            .into_iter()
            .map(|p| serde_json::to_value(p).unwrap_or_default())
            .collect(),
        model: None,
        turn_id: m.turn_id,
        created_at: m.timestamp.as_millisecond(),
        finish_reason: m
            .finish_reason
            .map(|f| serde_json::to_value(f).unwrap_or_default()),
    }
}
