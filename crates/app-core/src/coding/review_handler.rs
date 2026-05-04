use crate::coding::review_prompt::{
    render_system_prompt, REVIEW_DEFAULT_MODEL, REVIEW_MAX_ITER, REVIEW_TOOL_TIMEOUT,
};
use crate::coding::review_types::parse_review_output;
use crate::AppCore;
use common::{KlyntbotError, Result};
use uuid::Uuid;

/// Result for coding_review_start.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResult {
    pub review_id: String,
    pub thread_id: String,
    pub summary: String,
    pub issues: Vec<ReviewIssue>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewIssue {
    pub severity: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub description: String,
    pub suggestion: Option<String>,
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
        let workspace = self.workspace_for_session(&session).await?;

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
            .resolve_review_model(&session, &workspace)
            .unwrap_or_else(|| REVIEW_DEFAULT_MODEL.to_string());

        let raw_output = self
            .review_provider_call(
                &system_prompt,
                &recent_msgs,
                &model,
                REVIEW_MAX_ITER,
                REVIEW_TOOL_TIMEOUT,
                &workspace,
            )
            .await?;

        let parsed = parse_review_output(&raw_output)?;

        let result = ReviewResult {
            review_id: review_id.clone(),
            thread_id: session.key.clone(),
            summary: parsed.summary,
            issues: parsed.issues.into_iter().map(Into::into).collect(),
        };

        // Persist
        self.repos
            .coding_reviews
            .insert(&storage::repos::coding_reviews::CodingReviewRow {
                id: review_id,
                session_id: session.id.clone(),
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

    async fn workspace_for_session(
        &self,
        session: &storage::SessionRow,
    ) -> Result<storage::WorkspaceRow> {
        let workspace_id = session
            .workspace_id
            .as_deref()
            .ok_or_else(|| KlyntbotError::InvalidArgument("not a coding session".into()))?;
        self.repos
            .workspaces
            .get(workspace_id)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))
    }

    fn resolve_review_model(
        &self,
        _session: &storage::SessionRow,
        _workspace: &storage::WorkspaceRow,
    ) -> Option<String> {
        // Phase 4 chain: workspace.settings.review_model → config.coding.review.defaults.model
        // Sprint A defaults to REVIEW_DEFAULT_MODEL when neither is set.
        // Implement workspace lookup once Workspace.settings.review_model field lands.
        None
    }

    async fn review_provider_call(
        &self,
        system_prompt: &str,
        recent_msgs: &[storage::repos::session::SessionMessageWithParts],
        model: &str,
        max_iter: u32,
        tool_timeout: std::time::Duration,
        _workspace: &storage::WorkspaceRow,
    ) -> Result<String> {
        let mut messages = vec![providers::Message::system(system_prompt.to_string())];
        for m in recent_msgs {
            let text = m
                .parts
                .iter()
                .filter_map(|p| match p {
                    storage::messages::parts::MessagePart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            let msg = match m.role.as_str() {
                "user" => providers::Message::user(text),
                "assistant" => providers::Message::assistant(text, None, None),
                "system" => providers::Message::system(text),
                _ => providers::Message::user(text),
            };
            messages.push(msg);
        }

        let provider = match self.cognitive_provider.clone() {
            Some(p) => p,
            None => {
                return Err(KlyntbotError::Storage(
                    "no cognitive provider available".into(),
                ))
            }
        };
        let params = providers::ChatParams::new(model.to_string())
            .with_max_tokens(2048)
            .with_temperature(0.2);

        let resp = tokio::time::timeout(tool_timeout, provider.chat(&messages, None, &params))
            .await
            .map_err(|_| KlyntbotError::Storage("review provider timeout".into()))?
            .map_err(|e| KlyntbotError::Storage(format!("review provider: {e}")))?;

        let _ = max_iter; // For Sprint A, single-shot LLM call. Multi-iteration ReAct is Sprint B.
        Ok(resp.content.unwrap_or_default())
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
