use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingMemoryStatusResponse {
    pub daemon_alive: bool,
    pub buffered_event_count: i64,
    pub unprocessed_event_count: i64,
    pub socket_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliHealthRow {
    pub cli: String,
    pub enabled: bool,
    pub last_event_at: Option<String>,
    pub event_count_24h: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReplayEntry {
    pub id: String,
    pub source: String,
    pub session_id: String,
    pub kind: String,
    pub occurred_at: String,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnoseResult {
    pub ok: bool,
    pub message: String,
}
