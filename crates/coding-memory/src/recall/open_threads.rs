//! Query for unfinished turn-trace episodes from the last `since` window.
//!
//! "Unfinished" = coarse heuristic: recent `turn_trace` episodes whose
//! session ended within the window.  Refined by Mirror later.

use cognitive::EpisodicMemoryRepo;
use jiff::{Timestamp, ToSpan};

pub struct OpenThread {
    pub episode_id: String,
    pub when: Timestamp,
    pub last_user_prompt: String,
    pub repo_id: Option<String>,
}

pub async fn list_open_threads(
    ep_repo: &EpisodicMemoryRepo,
    _repo: Option<&str>,
    since_days: u32,
    limit: usize,
) -> common::Result<Vec<OpenThread>> {
    let cutoff = Timestamp::now()
        .checked_sub(since_days.days())
        .unwrap_or(Timestamp::MIN)
        .to_string();
    let rows = ep_repo
        .list_by_memory_type("turn_trace", limit as i64)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("open_threads: {e}")))?;
    let mut out = Vec::new();
    for row in rows {
        if row.recorded_at < cutoff {
            continue;
        }
        let when = row.occurred_at.parse().unwrap_or_else(|_| Timestamp::now());
        out.push(OpenThread {
            episode_id: row.id,
            when,
            last_user_prompt: row.content,
            repo_id: row.scope_repo_id,
        });
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}
