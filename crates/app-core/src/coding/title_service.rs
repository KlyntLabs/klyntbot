//! Background title generation for coding sessions.
//!
//! Spawned from `coding_message_send` when a session has no title yet and
//! the incoming message is its first. Calls the cognitive provider,
//! sanitizes the response, persists via `rename_session`, and emits
//! `coding:thread_updated` so the sidebar refetches.

use crate::events::AppEventEmitter;
use common::Result;
use providers::DynProvider;
use std::sync::Arc;
use storage::SessionRepo;

const TITLE_TIMEOUT_SECS: u64 = 5;
const MAX_TITLE_LEN: usize = 60;
const MAX_PROMPT_LEN: usize = 500;

#[tracing::instrument(
    skip(repo, provider, emitter, first_user_message),
    fields(session_key = %session_key)
)]
pub async fn autogenerate_title(
    repo: SessionRepo,
    provider: DynProvider,
    emitter: Arc<dyn AppEventEmitter>,
    session_key: String,
    first_user_message: String,
    model: String,
) -> Result<()> {
    // Re-check: user may have manually renamed in the meantime.
    let session = match repo.get_session(&session_key).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "title autogen: session lookup failed");
            return Ok(());
        }
    };
    if session
        .metadata
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        tracing::debug!("title autogen: skipping — title already set");
        return Ok(());
    }

    let prompt_capped = if first_user_message.len() > MAX_PROMPT_LEN {
        let end = first_user_message.floor_char_boundary(MAX_PROMPT_LEN);
        &first_user_message[..end]
    } else {
        &first_user_message[..]
    };

    let messages = vec![
        providers::Message::system(
            "Generate a concise 3 to 6 word title for this coding session based on the user's first request. \
             Output only the title — no quotes, no period, in Title Case."
        ),
        providers::Message::user(prompt_capped),
    ];

    let params = providers::ChatParams::new(model)
        .with_temperature(0.2)
        .with_max_tokens(24);

    let response = match tokio::time::timeout(
        std::time::Duration::from_secs(TITLE_TIMEOUT_SECS),
        provider.chat(&messages, None, &params, &[]),
    ).await {
        Ok(Ok(resp)) => resp,
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "title autogen: provider error");
            return Ok(());
        }
        Err(_) => {
            tracing::warn!("title autogen: provider timeout after {}s", TITLE_TIMEOUT_SECS);
            return Ok(());
        }
    };

    let raw_title = response.content.unwrap_or_default();
    let title = sanitize_title(&raw_title);
    if title.is_empty() {
        tracing::warn!("title autogen: empty/whitespace response, skipping persist");
        return Ok(());
    }

    if let Err(e) = repo.rename_session(&session_key, &title).await {
        tracing::error!(error = %e, "title autogen: rename failed");
        return Ok(());
    }

    emitter.emit_event(
        "coding:thread_updated",
        serde_json::json!({ "thread_id": session_key }),
    );
    tracing::info!(title = %title, "title autogen: persisted and emitted");
    Ok(())
}

fn sanitize_title(raw: &str) -> String {
    let trimmed = raw.trim();

    // Strip exactly one wrapping pair of quotes
    let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    // Drop trailing sentence-end punctuation
    let trimmed_punct = unquoted.trim_end_matches(|c: char| matches!(c, '.' | '!' | '?'));

    // Hard-cap length (char-boundary safe)
    let end = trimmed_punct.floor_char_boundary(MAX_TITLE_LEN);
    trimmed_punct[..end].trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_wrapping_quotes() {
        assert_eq!(sanitize_title("\"Refactor Sidebar\""), "Refactor Sidebar");
        assert_eq!(sanitize_title("'Refactor Sidebar'"), "Refactor Sidebar");
    }

    #[test]
    fn sanitize_strips_trailing_punctuation() {
        assert_eq!(sanitize_title("Refactor sidebar."), "Refactor sidebar");
        assert_eq!(sanitize_title("Fix bug!"), "Fix bug");
        assert_eq!(sanitize_title("What broke?"), "What broke");
    }

    #[test]
    fn sanitize_trims_whitespace_and_newlines() {
        assert_eq!(sanitize_title("  Refactor sidebar\n"), "Refactor sidebar");
    }

    #[test]
    fn sanitize_caps_at_60_chars() {
        let long = "a".repeat(120);
        assert_eq!(sanitize_title(&long).len(), MAX_TITLE_LEN);
    }

    #[test]
    fn sanitize_preserves_inner_punctuation() {
        assert_eq!(sanitize_title("Fix: parser bug"), "Fix: parser bug");
    }

    use crate::events::RecordingEmitter;

    async fn setup_repo() -> SessionRepo {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);
        repos.sessions
    }

    async fn insert_session_with_title(repo: &SessionRepo, key: &str, title: Option<&str>) {
        let metadata = match title {
            Some(t) => serde_json::json!({ "title": t }),
            None => serde_json::json!({}),
        };
        repo.upsert_session_with_mode(key, common::SessionMode::Coding, &metadata)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn skips_when_title_already_set() {
        let repo = setup_repo().await;
        insert_session_with_title(&repo, "coding:t1", Some("Existing Title")).await;

        let emitter = Arc::new(RecordingEmitter::new());
        let provider: DynProvider = Arc::new(providers::adapters::NoopProvider);

        autogenerate_title(
            repo.clone(),
            provider,
            emitter.clone() as Arc<dyn AppEventEmitter>,
            "coding:t1".into(),
            "first message".into(),
            "fake-model".into(),
        )
        .await
        .unwrap();

        let row = repo.get_session("coding:t1").await.unwrap();
        assert_eq!(
            row.metadata.get("title").and_then(|v| v.as_str()),
            Some("Existing Title")
        );
        assert!(emitter.events().is_empty(), "no emit when title preset");
    }

    /// Test provider that returns a fixed completion.
    #[derive(Clone)]
    struct CannedProvider(String);

    #[async_trait::async_trait]
    impl providers::LlmProvider for CannedProvider {
        async fn chat(
            &self,
            _messages: &[providers::Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &providers::ChatParams,
            _cache: &[providers::CacheBreakpoint],
        ) -> common::Result<providers::LlmResponse> {
            Ok(providers::LlmResponse {
                content: Some(self.0.clone()),
                tool_calls: vec![],
                finish_reason: "stop".into(),
                usage: providers::Usage::default(),
                reasoning_content: None,
            })
        }
        fn default_model(&self) -> &str {
            "canned-model"
        }
        fn name(&self) -> &str {
            "canned"
        }
    }

    #[tokio::test]
    async fn happy_path_renames_and_emits() {
        let repo = setup_repo().await;
        insert_session_with_title(&repo, "coding:t2", None).await;

        let emitter = Arc::new(RecordingEmitter::new());
        let provider: DynProvider =
            Arc::new(CannedProvider("\"Refactor Sidebar Layout.\"".into()));

        autogenerate_title(
            repo.clone(),
            provider,
            emitter.clone() as Arc<dyn AppEventEmitter>,
            "coding:t2".into(),
            "Help me refactor the sidebar layout".into(),
            "canned-model".into(),
        )
        .await
        .unwrap();

        let row = repo.get_session("coding:t2").await.unwrap();
        assert_eq!(
            row.metadata.get("title").and_then(|v| v.as_str()),
            Some("Refactor Sidebar Layout"),
            "title should be sanitized + persisted"
        );

        let events = emitter.events();
        assert_eq!(events.len(), 1, "exactly one event emitted");
        assert_eq!(events[0].0, "coding:thread_updated");
        assert_eq!(
            events[0].1.get("thread_id").and_then(|v| v.as_str()),
            Some("coding:t2")
        );
    }

    struct ErrorProvider;

    #[async_trait::async_trait]
    impl providers::LlmProvider for ErrorProvider {
        async fn chat(
            &self,
            _messages: &[providers::Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &providers::ChatParams,
            _cache: &[providers::CacheBreakpoint],
        ) -> common::Result<providers::LlmResponse> {
            Err(common::ProviderError::Http("upstream 500".into()).into())
        }
        fn default_model(&self) -> &str {
            "err-model"
        }
        fn name(&self) -> &str {
            "error"
        }
    }

    #[tokio::test]
    async fn provider_error_is_non_fatal() {
        let repo = setup_repo().await;
        insert_session_with_title(&repo, "coding:t3", None).await;

        let emitter = Arc::new(RecordingEmitter::new());
        let provider: DynProvider = Arc::new(ErrorProvider);

        let result = autogenerate_title(
            repo.clone(),
            provider,
            emitter.clone() as Arc<dyn AppEventEmitter>,
            "coding:t3".into(),
            "anything".into(),
            "err-model".into(),
        )
        .await;

        assert!(result.is_ok(), "provider errors must not bubble");
        let row = repo.get_session("coding:t3").await.unwrap();
        assert!(
            row.metadata.get("title").and_then(|v| v.as_str()).is_none(),
            "title remains None on provider error"
        );
        assert!(emitter.events().is_empty());
    }
}
