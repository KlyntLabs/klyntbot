use desktop_shared::commands::{ChatMessageResponse, ChatThreadResponse};
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn chat_threads(
    state: State<'_, AppCore>,
) -> Result<Vec<ChatThreadResponse>, String> {
    let sessions = state
        .repos
        .sessions
        .list_sessions()
        .await
        .map_err(|e| e.to_string())?;

    Ok(sessions
        .iter()
        .map(|s| {
            let title = s
                .metadata
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or(&s.key)
                .to_string();
            let project_id = s
                .metadata
                .get("projectId")
                .and_then(|v| v.as_str())
                .map(String::from);

            ChatThreadResponse {
                session_key: s.key.clone(),
                title,
                message_count: s.message_count,
                updated_at: s.updated_at,
                project_id,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn chat_messages(
    state: State<'_, AppCore>,
    session_key: String,
    limit: Option<i64>,
) -> Result<Vec<ChatMessageResponse>, String> {
    let rows = if let Some(lim) = limit {
        state
            .repos
            .sessions
            .get_recent_messages(&session_key, lim)
            .await
    } else {
        state
            .repos
            .sessions
            .get_messages(&session_key)
            .await
    }
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| ChatMessageResponse {
            id: m.id.to_string(),
            role: m.role.clone(),
            content: m.content.clone(),
            timestamp: m.timestamp,
        })
        .collect())
}

#[tauri::command]
pub async fn chat_send(
    state: State<'_, AppCore>,
    content: String,
    session_key: String,
) -> Result<ChatMessageResponse, String> {
    // Ensure session exists
    let metadata = serde_json::json!({ "title": "Desktop Chat" });
    state
        .repos
        .sessions
        .upsert_session(&session_key, &metadata)
        .await
        .map_err(|e| e.to_string())?;

    // Store user message
    let msg_id = uuid::Uuid::new_v4();
    let msg = state
        .repos
        .sessions
        .add_message(&session_key, msg_id, "user", &content, None, None, None)
        .await
        .map_err(|e| e.to_string())?;

    Ok(ChatMessageResponse {
        id: msg.id.to_string(),
        role: msg.role,
        content: msg.content,
        timestamp: msg.timestamp,
    })
}

#[tauri::command]
pub async fn chat_cancel(_session_key: String) -> Result<(), String> {
    // TODO: cancel agent processing when AgentLoop is wired
    Ok(())
}
