//! Helper for reading `problemHash` out of an episode's metadata.

use cognitive::EpisodicMemoryRepo;
use uuid::Uuid;

/// Lookup the `problemHash` field stored by the Distiller in
/// `episodic_memories.metadata`. Returns `None` if absent or episode missing.
pub async fn problem_hash_for_episode(
    repo: &EpisodicMemoryRepo,
    episode_id: Uuid,
) -> common::Result<Option<String>> {
    let Some(row) = repo
        .get(&episode_id.to_string())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("get: {e}")))?
    else {
        return Ok(None);
    };
    let Some(metadata) = row.metadata else {
        return Ok(None);
    };
    let parsed: serde_json::Value = match metadata.parse() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    Ok(parsed
        .get("problemHash")
        .and_then(|v| v.as_str())
        .map(String::from))
}
