use serde::{Deserialize, Serialize};
use specta::Type;

use super::thread::ThreadId;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CostUpdate {
    pub thread_id: Option<ThreadId>,
    pub provider: String,
    pub prompt_tokens_delta: u64,
    pub completion_tokens_delta: u64,
    pub usd_delta: f64,
    pub thread_total_usd: Option<f64>,
    pub ceiling_breached: bool,
}
