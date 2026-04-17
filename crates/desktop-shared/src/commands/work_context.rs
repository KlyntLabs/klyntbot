use serde::{Deserialize, Serialize};

// ── Work Contexts ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkContextResponse {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub context_type: String,
    pub linked_project_id: Option<String>,
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub confidence: f64,
    pub first_seen_at: String,
    pub last_active_at: String,
    pub total_duration_secs: i64,
    pub event_count: i64,
}

impl From<activity_log::WorkContext> for WorkContextResponse {
    fn from(c: activity_log::WorkContext) -> Self {
        Self {
            id: c.id,
            title: c.title,
            description: c.description,
            status: c.status.as_str().to_string(),
            context_type: c.context_type.as_str().to_string(),
            linked_project_id: c.linked_project_id,
            color: c.color,
            tags: c.tags,
            confidence: c.confidence,
            first_seen_at: c.first_seen_at.to_string(),
            last_active_at: c.last_active_at.to_string(),
            total_duration_secs: c.total_duration_secs,
            event_count: c.event_count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkResourceResponse {
    pub id: String,
    pub resource_type: String,
    pub resource_name: String,
    pub resource_path: Option<String>,
    pub resource_uri: Option<String>,
    pub access_count: i64,
}

impl From<activity_log::WorkResource> for WorkResourceResponse {
    fn from(r: activity_log::WorkResource) -> Self {
        Self {
            id: r.id,
            resource_type: r.resource_type,
            resource_name: r.resource_name,
            resource_path: r.resource_path,
            resource_uri: r.resource_uri,
            access_count: r.access_count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkContextDetailResponse {
    pub context: WorkContextResponse,
    pub resources: Vec<WorkResourceResponse>,
    pub linked_task_ids: Vec<String>,
    pub recent_events: Vec<ActivityEventResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEventResponse {
    pub id: String,
    pub timestamp: String,
    pub source: String,
    pub actor: String,
    pub resource_name: Option<String>,
    pub action: String,
    pub content_preview: Option<String>,
    pub app_name: Option<String>,
    pub duration_secs: Option<i64>,
}

impl From<activity_log::ActivityLogEntry> for ActivityEventResponse {
    fn from(e: activity_log::ActivityLogEntry) -> Self {
        Self {
            id: e.id,
            timestamp: e.timestamp.to_string(),
            source: e.source.as_str().to_string(),
            actor: e.actor.as_str().to_string(),
            resource_name: e.resource_name,
            action: e.action,
            content_preview: e.content_preview,
            app_name: e.app_name,
            duration_secs: e.duration_secs,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextTimelineBlockResponse {
    pub context_id: Option<String>,
    pub context_title: Option<String>,
    pub context_color: Option<String>,
    pub context_type: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub event_count: i64,
    pub is_idle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextResumeResponse {
    pub context_id: String,
    pub context_title: String,
    pub summary: String,
    pub suggested_prompt: String,
    pub recent_resources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkContextUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub color: Option<String>,
    pub status: Option<String>,
    pub linked_project_id: Option<String>,
}

// ── Inference Stats ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceStatsResponse {
    pub active_context_count: i64,
    pub archived_context_count: i64,
    pub events_last_hour: i64,
    pub events_last_24h: i64,
    pub assignment_rate: f64,
    pub avg_confidence: f64,
    pub merges_last_24h: i64,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceConfigUpdate {
    pub assignment_threshold: Option<f64>,
    pub merge_threshold: Option<f64>,
    pub semantic_weight: Option<f64>,
    pub temporal_weight: Option<f64>,
    pub resource_weight: Option<f64>,
    pub inference_interval_mins: Option<u64>,
    pub max_dormancy_days: Option<f64>,
    pub max_active_contexts: Option<usize>,
}

// ── Dashboard Intelligence ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardIntelligenceResponse {
    pub active_context: Option<WorkContextSummary>,
    pub focus_recommendation: Option<String>,
    pub session_summary: Vec<SessionBlock>,
    pub context_switches: i64,
    pub switch_quality: String,
    pub productivity_score: f64,
    pub score_trend: f64,
    pub patterns: Vec<String>,
    pub nudges: Vec<DashboardNudge>,
    pub resource_clusters: Vec<ResourceCluster>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkContextSummary {
    pub id: String,
    pub title: String,
    pub context_type: String,
    pub color: Option<String>,
    pub duration_mins: i64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBlock {
    pub context_type: String,
    pub total_duration_mins: i64,
    pub session_count: i64,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardNudge {
    pub message: String,
    pub nudge_type: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCluster {
    pub resources: Vec<String>,
    pub access_count: i64,
}
