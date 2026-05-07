//! Telegram approval channel — sends inline-keyboard approval prompts
//! and resolves pending approvals via callback queries.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tracing::{debug, warn};

use approval::{
    ApprovalCapabilities, ApprovalChannel, ApprovalClass, ApprovalDecision, ApprovalRequest,
};

/// Callback data prefix for approval buttons.
const CALLBACK_PREFIX: &str = "appr";

/// Default timeout for waiting on an approval response (10 minutes).
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(600);

/// A Telegram-based approval channel that sends inline-keyboard prompts
/// and resolves decisions via callback query responses.
pub struct TelegramApprovalChannel {
    client: Client,
    api_base: String,
    chat_id: i64,
    pending: Arc<DashMap<String, oneshot::Sender<ApprovalDecision>>>,
}

impl TelegramApprovalChannel {
    /// Create a new Telegram approval channel.
    ///
    /// `token` — Telegram bot token.
    /// `chat_id` — the chat where approval requests are sent.
    pub fn new(token: &str, chat_id: i64) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client");

        Self {
            client,
            api_base: format!("https://api.telegram.org/bot{}", token),
            chat_id,
            pending: Arc::new(DashMap::new()),
        }
    }

    /// Call a Telegram Bot API method.
    async fn api_call(&self, method: &str, params: Value) -> Result<Value, String> {
        let url = format!("{}/{}", self.api_base, method);
        let resp = self
            .client
            .post(&url)
            .json(&params)
            .send()
            .await
            .map_err(|e| format!("HTTP error: {e}"))?;

        let body: Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON parse error: {e}"))?;

        if body.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            Ok(body.get("result").cloned().unwrap_or(Value::Null))
        } else {
            Err(format!(
                "Telegram API error: {}",
                body.get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
            ))
        }
    }

    /// Build the approval prompt text from a request.
    fn build_prompt(req: &ApprovalRequest) -> String {
        let class_label = format!("{:?}", req.class);
        let scope_label = match &req.scope {
            approval::ApprovalScope::ToolAction => String::new(),
            approval::ApprovalScope::ToolActionResource(r) => format!(" on `{r}`"),
        };

        format!(
            "🔐 <b>Approval required</b>\n\n\
             Tool: <code>{}</code>\n\
             Class: <b>{}</b>{}\
             \nSend / Approve?",
            req.tool_name, class_label, scope_label,
        )
    }

    /// Build an inline keyboard for approval options.
    fn build_keyboard(request_id: &str) -> Value {
        json!({
            "inline_keyboard": [[
                { "text": "✅ Once",    "callback_data": format!("{CALLBACK_PREFIX}:{request_id}:once") },
                { "text": "🔁 Session", "callback_data": format!("{CALLBACK_PREFIX}:{request_id}:session") },
                { "text": "♾️ Always",  "callback_data": format!("{CALLBACK_PREFIX}:{request_id}:forever") },
                { "text": "❌ Decline",  "callback_data": format!("{CALLBACK_PREFIX}:{request_id}:decline") },
            ]]
        })
    }

    /// Handle an incoming callback_query update from the Telegram polling loop.
    ///
    /// Call this from wherever `getUpdates` is polled (e.g. the main TelegramChannel).
    /// Returns `true` if the callback was an approval callback (consumed), `false` otherwise.
    pub fn handle_callback(&self, callback: &Value) -> bool {
        let data = match callback.get("data").and_then(|v| v.as_str()) {
            Some(d) => d,
            None => return false,
        };

        let parts: Vec<&str> = data.splitn(3, ':').collect();
        if parts.len() < 3 || parts[0] != CALLBACK_PREFIX {
            return false;
        }

        let request_id = parts[1];
        let verb = parts[2];

        let decision = match verb {
            "once" => ApprovalDecision::Once,
            "session" => ApprovalDecision::Session,
            "forever" => ApprovalDecision::Forever,
            "decline" => ApprovalDecision::Decline {
                reason: "User declined via Telegram".into(),
            },
            _ => {
                debug!("Unknown approval verb: {}", verb);
                return false;
            }
        };

        if let Some((_, tx)) = self.pending.remove(request_id) {
            let _ = tx.send(decision);
            true
        } else {
            debug!("No pending approval for id={}", request_id);
            false
        }
    }

    /// Access to the pending map for integration/testing.
    pub fn pending(&self) -> &Arc<DashMap<String, oneshot::Sender<ApprovalDecision>>> {
        &self.pending
    }
}

#[async_trait]
impl ApprovalChannel for TelegramApprovalChannel {
    async fn request(&self, req: ApprovalRequest) -> ApprovalDecision {
        let request_id = uuid::Uuid::new_v4().to_string();
        let prompt = Self::build_prompt(&req);
        let keyboard = Self::build_keyboard(&request_id);

        // Send the approval message.
        let send_result = self
            .api_call(
                "sendMessage",
                json!({
                    "chat_id": self.chat_id,
                    "text": prompt,
                    "parse_mode": "HTML",
                    "reply_markup": keyboard,
                }),
            )
            .await;

        if let Err(e) = send_result {
            warn!("Failed to send approval prompt: {}", e);
            return ApprovalDecision::Decline {
                reason: format!("Failed to send approval prompt: {e}"),
            };
        }

        let msg_id = send_result
            .ok()
            .and_then(|v| v.get("message_id").and_then(|m| m.as_i64()));

        // Wait for the callback response.
        let (tx, rx) = oneshot::channel();
        self.pending.insert(request_id.clone(), tx);

        /// Guard that removes the pending entry on drop unless explicitly forgotten.
        struct PendingGuard<'a> {
            map: &'a DashMap<String, oneshot::Sender<ApprovalDecision>>,
            key: String,
            active: bool,
        }

        impl<'a> Drop for PendingGuard<'a> {
            fn drop(&mut self) {
                if self.active {
                    self.map.remove(&self.key);
                }
            }
        }

        let guard = PendingGuard {
            map: &self.pending,
            key: request_id.clone(),
            active: true,
        };

        let decision = match tokio::time::timeout(APPROVAL_TIMEOUT, rx).await {
            Ok(Ok(d)) => {
                // Callback handler already removed the entry; don't double-remove.
                std::mem::forget(guard);
                d
            }
            Ok(Err(_)) => {
                warn!("Approval oneshot cancelled for id={}", request_id);
                ApprovalDecision::Cancel
            }
            Err(_) => {
                warn!("Approval timed out for id={}", request_id);
                ApprovalDecision::Decline {
                    reason: "Approval request timed out (10 min)".into(),
                }
            }
        };

        // Edit the original message to show the outcome (best-effort).
        if let Some(mid) = msg_id {
            let outcome_text = match &decision {
                ApprovalDecision::Once => "✅ Approved (once)",
                ApprovalDecision::Session => "🔁 Approved (session)",
                ApprovalDecision::Forever => "♾️ Approved (always)",
                ApprovalDecision::Decline { .. } => "❌ Declined",
                ApprovalDecision::Cancel => "🚫 Cancelled",
            };

            let _ = self
                .api_call(
                    "editMessageText",
                    json!({
                        "chat_id": self.chat_id,
                        "message_id": mid,
                        "text": format!("{}\n\n{}", prompt, outcome_text),
                        "parse_mode": "HTML",
                    }),
                )
                .await;
        }

        decision
    }

    fn capabilities(&self) -> ApprovalCapabilities {
        ApprovalCapabilities {
            supports_inline: true,
            supports_classes: HashSet::from([ApprovalClass::Destructive, ApprovalClass::Admin]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approval::{ApprovalClass, ApprovalScope};
    use common::SessionMode;
    use serde_json::json;

    fn make_channel() -> TelegramApprovalChannel {
        TelegramApprovalChannel::new("test-token", -100123456)
    }

    #[test]
    fn build_keyboard_format() {
        let kb = TelegramApprovalChannel::build_keyboard("abc-123");
        let rows = kb["inline_keyboard"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        let buttons = rows[0].as_array().unwrap();
        assert_eq!(buttons.len(), 4);
        assert_eq!(
            buttons[0]["callback_data"].as_str().unwrap(),
            "appr:abc-123:once"
        );
        assert_eq!(
            buttons[1]["callback_data"].as_str().unwrap(),
            "appr:abc-123:session"
        );
        assert_eq!(
            buttons[2]["callback_data"].as_str().unwrap(),
            "appr:abc-123:forever"
        );
        assert_eq!(
            buttons[3]["callback_data"].as_str().unwrap(),
            "appr:abc-123:decline"
        );
    }

    #[test]
    fn handle_callback_resolves_once() {
        let ch = make_channel();
        let (tx, _rx) = oneshot::channel();
        ch.pending.insert("req-1".into(), tx);

        let callback = json!({ "data": "appr:req-1:once" });
        assert!(ch.handle_callback(&callback));

        // The receiver should get Once.
        // We can't easily await in a sync test, but the channel was removed.
        assert!(!ch.pending.contains_key("req-1"));
    }

    #[test]
    fn handle_callback_ignores_non_approval() {
        let ch = make_channel();
        let callback = json!({ "data": "askuser:123:q:val" });
        assert!(!ch.handle_callback(&callback));
    }

    #[test]
    fn handle_callback_ignores_unknown_verb() {
        let ch = make_channel();
        let (tx, _) = oneshot::channel();
        ch.pending.insert("req-2".into(), tx);

        let callback = json!({ "data": "appr:req-2:mystery" });
        assert!(!ch.handle_callback(&callback));
        // Pending entry should still exist.
        assert!(ch.pending.contains_key("req-2"));
    }

    #[test]
    fn capabilities_correct() {
        let ch = make_channel();
        let caps = ch.capabilities();
        assert!(caps.supports_inline);
        assert!(caps.supports_classes.contains(&ApprovalClass::Destructive));
        assert!(caps.supports_classes.contains(&ApprovalClass::Admin));
        assert!(!caps.supports_classes.contains(&ApprovalClass::Safe));
    }

    #[test]
    fn build_prompt_includes_tool_and_class() {
        let req = ApprovalRequest {
            tool_name: "bash".into(),
            action: None,
            args: json!({}),
            class: ApprovalClass::Destructive,
            scope: ApprovalScope::ToolAction,
            ctx: approval::ApprovalContext {
                mode: SessionMode::Coding,
                channel: approval::ChannelKind::Telegram,
                session_id: "s1".into(),
                user_id: None,
                cwd: std::path::PathBuf::from("."),
            },
            preview: None,
            suggested_grant: None,
        };
        let text = TelegramApprovalChannel::build_prompt(&req);
        assert!(text.contains("bash"));
        assert!(text.contains("Destructive"));
    }
}
