use std::sync::Arc;

use crate::coding::review_prompt::{
    render_system_prompt, REVIEW_DEFAULT_MODEL, REVIEW_MAX_ITER, REVIEW_TOOL_TIMEOUT,
};
use crate::coding::review_types::parse_review_output;
use crate::AppCore;
use common::{KlyntbotError, Result};
use tokio::sync::RwLock;
use uuid::Uuid;

pub use storage::messages::parts::ReviewIssue;

/// Result for coding_review_start.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResult {
    pub review_id: String,
    pub thread_id: String,
    pub summary: String,
    pub issues: Vec<ReviewIssue>,
}

impl AppCore {
    /// Start a code review pass on a coding thread.
    ///
    /// If `target` is provided, reviews specific files/diffs; otherwise reviews
    /// recent changes in the thread.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_review_start(
        &self,
        thread_id: &str,
        target: Option<&str>,
        delivery: Option<&str>,
    ) -> Result<ReviewResult> {
        let session = self
            .repos
            .sessions
            .get_session(thread_id)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        let review_id = Uuid::new_v4().to_string();

        let system_prompt = render_system_prompt(target);

        let recent_msgs = self
            .repos
            .sessions
            .get_messages_parts(
                &session.key,
                crate::coding::review_prompt::REVIEW_CONTEXT_TURN_LIMIT as i64,
            )
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        let model = self
            .resolve_review_model()
            .unwrap_or_else(|| REVIEW_DEFAULT_MODEL.to_string());

        // Resolve workspace path for the read-only tool registry's `cwd`.
        // Falls back to "." when the session is not workspace-scoped (review on a
        // chat thread without a workspace is uncommon but legal — the read tools
        // simply have no useful scope).
        let workspace_path = match session.workspace_id.as_deref() {
            Some(ws_id) => self
                .repos
                .workspaces
                .get(ws_id)
                .await
                .map(|ws| std::path::PathBuf::from(ws.path))
                .unwrap_or_else(|_| std::path::PathBuf::from(".")),
            None => std::path::PathBuf::from("."),
        };

        let raw_output = self
            .review_provider_call(
                &system_prompt,
                &recent_msgs,
                &model,
                REVIEW_TOOL_TIMEOUT,
                &session.key,
                workspace_path,
            )
            .await?;

        let parsed = parse_review_output(&raw_output)?;

        let result = ReviewResult {
            review_id: review_id.clone(),
            thread_id: session.key.clone(),
            summary: parsed.summary,
            issues: parsed.issues,
        };

        // Persist
        self.repos
            .coding_reviews
            .insert(&storage::repos::coding_reviews::CodingReviewRow {
                id: review_id,
                session_id: session.key.clone(),
                summary: result.summary.clone(),
                issues_json: serde_json::to_string(&result.issues).unwrap_or_default(),
                target: target.map(String::from),
                delivery: delivery.map(String::from),
                created_at: jiff::Timestamp::now().to_string(),
            })
            .await?;

        // Emit a ReviewResult MessagePart in the session for inline rendering.
        if matches!(delivery, None | Some("inline")) {
            self.append_review_result_part(&session, &result).await?;
        }

        Ok(result)
    }

    fn resolve_review_model(&self) -> Option<String> {
        // Phase 4 chain: workspace.settings.review_model → config.coding.review.defaults.model
        // Sprint A defaults to REVIEW_DEFAULT_MODEL when neither is set.
        // Implement workspace lookup once Workspace.settings.review_model field lands.
        None
    }

    /// Run the review pass via the unified execute loop with a **read-only**
    /// tool registry. The registry is built fresh from `ToolKitBuilder::register_read_only`
    /// only — `register_mutating` and `register_all` are never called. This is the
    /// runtime half of invariant K17 (review purity): a reviewer LLM cannot reach
    /// `bash`, `write`, `edit`, or any other mutating tool by construction.
    async fn review_provider_call(
        &self,
        system_prompt: &str,
        recent_msgs: &[storage::repos::session::SessionMessageWithParts],
        model: &str,
        tool_timeout: std::time::Duration,
        session_key: &str,
        workspace_path: std::path::PathBuf,
    ) -> Result<String> {
        use ::agent::execution::{
            execute_loop, DepthMode, ExecutionBudget, ExecutionCore, ExecutionParams,
        };
        use tools::{registry::ToolRegistry, RoutingContext};

        let mut messages = vec![providers::Message::system(system_prompt.to_string())];
        for m in recent_msgs {
            let text = storage::messages::render::extract_text_from_parts(&m.parts);
            let msg = match m.role.as_str() {
                "user" => providers::Message::user(text),
                "assistant" => providers::Message::assistant(text),
                "system" => providers::Message::system(text),
                _ => providers::Message::user(text),
            };
            messages.push(msg);
        }

        let provider = self
            .cognitive_provider
            .clone()
            .ok_or_else(|| KlyntbotError::Storage("no cognitive provider available".into()))?;

        let kit = self
            .tool_kit
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| KlyntbotError::Storage("review: tool kit not initialized".into()))?;
        let kit = (*kit).clone().with_cwd(workspace_path);

        let mut registry = ToolRegistry::new();
        kit.register_read_only(&mut registry);
        let tool_defs: Vec<serde_json::Value> = (*registry.get_definitions()).clone();
        let context_window = provider.context_window();
        let core = ExecutionCore::new(provider, Arc::new(RwLock::new(registry)));

        let params = ExecutionParams::new(model, context_window)
            .with_timeout(tool_timeout)
            .with_max_iterations(REVIEW_MAX_ITER);
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 120_000, REVIEW_MAX_ITER);
        let routing_ctx = RoutingContext::new("review".into(), session_key.into());

        let result = execute_loop(
            &core,
            messages,
            &tool_defs,
            &params,
            &mut budget,
            &routing_ctx,
            None,
        )
        .await
        .map_err(|e| KlyntbotError::Storage(format!("review execute_loop: {e}")))?;

        Ok(result.content)
    }

    async fn append_review_result_part(
        &self,
        session: &storage::SessionRow,
        result: &ReviewResult,
    ) -> Result<()> {
        use storage::messages::parts::MessagePart;
        let part = MessagePart::ReviewResult {
            review_id: result.review_id.clone(),
            summary: result.summary.clone(),
            issues: result.issues.clone(),
        };
        self.repos
            .sessions
            .add_message_with_parts(
                &session.key,
                uuid::Uuid::new_v4(),
                "tool",
                &[part],
                None,
                None,
            )
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
}
