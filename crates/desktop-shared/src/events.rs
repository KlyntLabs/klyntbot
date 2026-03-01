use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentChunkPayload {
    pub session_key: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityUpdatedPayload {
    pub entity_type: String,
    pub id: String,
}
