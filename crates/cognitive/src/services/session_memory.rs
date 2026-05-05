//! Session memory service — maintains a per-session conversation scratchpad.
//!
//! Subscribes to `ChatTurnCompleted` events. Every [`UPDATE_INTERVAL_TURNS`] turns
//! (starting at turn [`MIN_TURNS_BEFORE_UPDATE`]), loads recent session messages and
//! either calls a cheap LLM pass or falls back to a heuristic summary, then persists
//! the result via [`storage::SessionMemoryRepo`].

use std::collections::HashMap;

use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use bus::DomainEvent;
use providers::{ChatParams, DynProvider, Message};

/// Don't update the scratchpad until the session has at least this many turns.
const MIN_TURNS_BEFORE_UPDATE: i64 = 3;

/// Re-generate the scratchpad every N turns after the minimum.
const UPDATE_INTERVAL_TURNS: i64 = 3;

/// Maximum number of recent messages passed to the summariser.
const MAX_MESSAGES_FOR_SUMMARY: i64 = 20;

/// System prompt for the LLM-based summariser.
const SESSION_MEMORY_SYSTEM_PROMPT: &str = "\
You are a conversation state tracker. Given a series of recent chat messages between \
a user and an AI assistant, produce a concise structured summary in markdown with the \
following sections (omit any that are not applicable):

## Current Task
What the user is currently working on or asking about.

## Key Decisions
Important decisions made during this conversation.

## Context
Relevant background information established in this conversation.

## Progress
What has been completed or resolved so far.

Be brief. Use bullet points where appropriate. Do not invent information not present \
in the messages.";

/// Configuration for [`SessionMemoryService`].
pub struct SessionMemoryConfig {
    pub event_rx: broadcast::Receiver<DomainEvent>,
    pub session_repo: storage::SessionRepo,
    pub memory_repo: storage::SessionMemoryRepo,
    pub provider: Option<DynProvider>,
    pub cancel: CancellationToken,
}

/// Background service that maintains per-session conversation scratchpads.
pub struct SessionMemoryService {
    cancel_token: CancellationToken,
    _task_handle: JoinHandle<()>,
}

impl SessionMemoryService {
    /// Start the background service.
    pub fn start(config: SessionMemoryConfig) -> Self {
        let SessionMemoryConfig {
            mut event_rx,
            session_repo,
            memory_repo,
            provider,
            cancel,
        } = config;

        let cancel_clone = cancel.clone();

        let handle = tokio::spawn(async move {
            // Track turn counts per session key.
            let mut turn_counts: HashMap<String, i64> = HashMap::new();

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        debug!("SessionMemoryService: cancelled, shutting down");
                        break;
                    }
                    result = event_rx.recv() => {
                        match result {
                            Ok(event) => {
                                if let DomainEvent::ChatTurnCompleted { session_key, .. } = &event {
                                    let session_key = session_key.clone();
                                    let count = turn_counts.entry(session_key.clone()).or_insert(0);
                                    *count += 1;
                                    let current = *count;

                                    if should_update(current) {
                                        update_scratchpad(
                                            &session_key,
                                            current,
                                            &session_repo,
                                            &memory_repo,
                                            provider.as_ref(),
                                        )
                                        .await;
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("SessionMemoryService: lagged, skipped {n} events");
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                debug!("SessionMemoryService: event channel closed");
                                break;
                            }
                        }
                    }
                }
            }
        });

        Self {
            cancel_token: cancel,
            _task_handle: handle,
        }
    }

    /// Request shutdown of the service.
    pub fn shutdown(&self) {
        self.cancel_token.cancel();
    }
}

/// Returns `true` when a scratchpad update should be triggered for the given turn count.
fn should_update(turn_count: i64) -> bool {
    turn_count >= MIN_TURNS_BEFORE_UPDATE
        && (turn_count - MIN_TURNS_BEFORE_UPDATE) % UPDATE_INTERVAL_TURNS == 0
}

/// Load recent messages, generate a summary, and persist it.
async fn update_scratchpad(
    session_key: &str,
    turn_count: i64,
    session_repo: &storage::SessionRepo,
    memory_repo: &storage::SessionMemoryRepo,
    provider: Option<&DynProvider>,
) {
    let messages = match session_repo
        .get_recent_messages(session_key, MAX_MESSAGES_FOR_SUMMARY)
        .await
    {
        Ok(msgs) => msgs,
        Err(e) => {
            warn!("SessionMemoryService: failed to load messages for {session_key}: {e}");
            return;
        }
    };

    if messages.is_empty() {
        return;
    }

    let content = if let Some(p) = provider {
        match generate_summary(p, &messages).await {
            Ok(s) => s,
            Err(e) => {
                warn!("SessionMemoryService: LLM summarisation failed for {session_key}: {e}");
                heuristic_summary(&messages)
            }
        }
    } else {
        heuristic_summary(&messages)
    };

    if let Err(e) = memory_repo.upsert(session_key, &content, turn_count).await {
        warn!("SessionMemoryService: failed to persist scratchpad for {session_key}: {e}");
    } else {
        debug!("SessionMemoryService: updated scratchpad for {session_key} at turn {turn_count}");
    }
}

/// Call the LLM to produce a structured markdown summary of the session.
async fn generate_summary(
    provider: &DynProvider,
    messages: &[storage::SessionMessageRow],
) -> common::Result<String> {
    let conversation = messages
        .iter()
        .map(|m| format!("[{}]: {}", m.role.to_uppercase(), m.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    let user_msg = format!("Conversation history:\n\n{conversation}");

    let llm_messages = vec![
        Message::system(SESSION_MEMORY_SYSTEM_PROMPT),
        Message::user(user_msg),
    ];

    let params = ChatParams::new(provider.default_model())
        .with_temperature(0.2)
        .with_max_tokens(800);

    let response = provider.chat(&llm_messages, None, &params).await?;
    Ok(response.content.unwrap_or_default())
}

/// Fallback: format recent messages as a simple "recent messages" block.
fn heuristic_summary(messages: &[storage::SessionMessageRow]) -> String {
    let lines: Vec<String> = messages
        .iter()
        .map(|m| format!("- **{}**: {}", m.role, common::helpers::truncate_at_boundary(&m.content, 120)))
        .collect();

    format!("## Recent Messages\n\n{}", lines.join("\n"))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_update() {
        // Below minimum — never update
        assert!(!should_update(0));
        assert!(!should_update(1));
        assert!(!should_update(2));
        // Exactly at minimum (turn 3) — update
        assert!(should_update(3));
        // Turn 4 and 5 — no update
        assert!(!should_update(4));
        assert!(!should_update(5));
        // Turn 6 — update (3 + 3)
        assert!(should_update(6));
        // Turn 9 — update
        assert!(should_update(9));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello");
    }
}
