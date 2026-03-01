#[tauri::command]
pub async fn chat_send(_content: String, _session_key: String) -> Result<(), String> {
    // Stubbed — will integrate with AgentLoop later
    Ok(())
}

#[tauri::command]
pub async fn chat_cancel(_session_key: String) -> Result<(), String> {
    Ok(())
}
