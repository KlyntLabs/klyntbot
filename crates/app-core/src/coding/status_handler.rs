use crate::AppCore;
use common::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct CodingStatus {
    pub mode: String,
    pub profile: String,
    pub sandbox: String,
    pub total_cost_usd: f64,
    pub total_tokens: u64,
    pub active_skills: Vec<String>,
}

impl AppCore {
    async fn coding_mode_for_thread(&self, session_key: &str) -> String {
        match self.repos.sessions.get_session(session_key).await {
            Ok(row) => row.conversation_type.unwrap_or_else(|| "chat".into()),
            Err(_) => "chat".into(),
        }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_status(&self, session_key: &str) -> Result<CodingStatus> {
        let mode = self.coding_mode_for_thread(session_key).await;
        let row = self.repos.sessions.get_session(session_key).await.ok();
        let profile = row
            .as_ref()
            .and_then(|r| r.tool_profile.clone())
            .unwrap_or_else(|| "curated".into());
        let cost = row.as_ref().map(|r| r.total_cost_usd).unwrap_or(0.0);
        let tokens = row.as_ref().map(|r| r.total_tokens as u64).unwrap_or(0);
        let active_skills = if let Some(act) = self.coding_skill_activator.lock().await.as_ref() {
            act.active_set().iter().cloned().collect()
        } else {
            vec![]
        };
        let sandbox = if cfg!(target_os = "macos") {
            "macos".to_string()
        } else if cfg!(target_os = "linux") {
            "linux".to_string()
        } else {
            "unknown".to_string()
        };
        Ok(CodingStatus {
            mode,
            profile,
            sandbox,
            total_cost_usd: cost,
            total_tokens: tokens,
            active_skills,
        })
    }
}
