use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::events::{MessageSegment, TransparencyData};

// ── Task ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponse {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub priority: Option<String>,
    pub status: String,
    pub due_date: Option<String>,
    pub tags: Vec<String>,
    pub project_id: Option<String>,
    pub area_id: String,
    pub objective_id: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<String>,
    pub subtask_count: u32,
    pub subtask_completed_count: u32,
    pub status_label_id: Option<String>,
    pub status_label: Option<StatusLabelResponse>,
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCreateParams {
    pub title: String,
    pub area_id: Option<String>,
    pub project_id: Option<String>,
    pub priority: Option<i16>,
    pub due_date: Option<String>,
    pub tags: Option<Vec<String>>,
    pub parent_id: Option<String>,
    pub status_label_id: Option<String>,
    pub group_id: Option<String>,
}

// ── Today Task (tray view) ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayTaskResponse {
    pub id: String,
    pub title: String,
    pub priority: Option<String>,
    pub status: String,
    pub completed: bool,
    pub is_overdue: bool,
    pub is_due_today: bool,
    pub due_display: Option<String>,
}

// ── Project ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    pub color: String,
    pub area_id: String,
    pub task_count: u32,
    pub completed_count: u32,
    pub objective_ids: Option<Vec<String>>,
    pub workflow_id: Option<String>,
}

// ── Objective / Key Result ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveResponse {
    pub id: String,
    pub title: String,
    pub status: String,
    pub progress: f64,
    pub project_id: String,
    pub key_results: Option<Vec<KeyResultResponse>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultResponse {
    pub id: String,
    pub title: String,
    pub progress: f64,
    pub current: f64,
    pub target: f64,
    pub unit: String,
}

// ── Status Workflows ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusWorkflowResponse {
    pub id: String,
    pub name: String,
    pub is_template: bool,
    pub is_global_default: bool,
    pub labels: Vec<StatusLabelResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusLabelResponse {
    pub id: String,
    pub workflow_id: String,
    pub name: String,
    pub color: String,
    pub status_group: String,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCreateParams {
    pub name: String,
    pub is_template: Option<bool>,
    pub source_workflow_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelCreateParams {
    pub workflow_id: String,
    pub name: String,
    pub color: String,
    pub status_group: String,
    pub position: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub status_group: Option<String>,
    pub position: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelReorderParams {
    pub workflow_id: String,
    pub label_ids: Vec<String>,
}

// ── Task Groups ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroupResponse {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub color: Option<String>,
    pub position: i32,
    pub task_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroupCreateParams {
    pub project_id: Option<String>,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroupUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub position: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroupReorderParams {
    pub project_id: Option<String>,
    pub group_ids: Vec<String>,
}

// ── Area ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaResponse {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: Option<String>,
    pub project_count: i64,
    pub task_count: i64,
}

// ── Chat ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatThreadResponse {
    pub session_key: String,
    pub title: String,
    pub message_count: i64,
    pub updated_at: DateTime<Utc>,
    // Context fields from session_context join
    pub context_type: Option<String>,
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub area_id: Option<String>,
    pub area_name: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageResponse {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<MessageSegment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparency: Option<TransparencyData>,
}

/// Optional session context sent from the frontend alongside a chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContextInput {
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub context_type: Option<String>,
    pub is_ephemeral: Option<bool>,
}

// ── Finance ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancePortfolioResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub currency: String,
    pub total_value: i64,
    pub total_cost_basis: i64,
    pub holding_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceNetWorthResponse {
    pub totals_by_currency: Vec<CurrencyNetWorth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyNetWorth {
    pub currency: String,
    pub accounts: i64,
    pub investments: i64,
    pub liabilities: i64,
    pub net: i64,
}

impl CurrencyNetWorth {
    pub fn zero(currency: String) -> Self {
        Self {
            currency,
            accounts: 0,
            investments: 0,
            liabilities: 0,
            net: 0,
        }
    }
}

// ── Finance Mutation Params ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceAccountCreateParams {
    pub name: String,
    pub account_type: String,
    pub currency: Option<String>,
    pub balance: Option<i64>,
    pub institution: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceAccountUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub balance: Option<i64>,
    pub institution: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub is_archived: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTransactionCreateParams {
    pub account_id: String,
    pub tx_type: String,
    pub amount: i64,
    pub currency: Option<String>,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub counterparty: Option<String>,
    pub tx_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTransactionUpdateParams {
    pub id: String,
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub subcategory: Option<Option<String>>,
    pub counterparty: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub tx_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceBudgetCreateParams {
    pub name: String,
    pub amount: i64,
    pub period: String,
    pub currency: Option<String>,
    pub category: Option<String>,
    pub method: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub alert_threshold: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceBudgetUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceGoalCreateParams {
    pub name: String,
    pub goal_type: String,
    pub target_amount: i64,
    pub currency: Option<String>,
    pub current_amount: Option<i64>,
    pub deadline: Option<String>,
    pub monthly_contribution: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceGoalUpdateParams {
    pub id: String,
    pub current_amount: Option<i64>,
    pub target_amount: Option<i64>,
    pub monthly_contribution: Option<Option<i64>>,
    pub deadline: Option<Option<String>>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceLiabilityCreateParams {
    pub name: String,
    pub liability_type: String,
    pub principal: i64,
    pub currency: Option<String>,
    pub remaining: Option<i64>,
    pub interest_rate: Option<f64>,
    pub monthly_payment: Option<i64>,
    pub due_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceLiabilityUpdateParams {
    pub id: String,
    pub remaining: Option<i64>,
    pub monthly_payment: Option<Option<i64>>,
    pub interest_rate: Option<Option<f64>>,
    pub notes: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancePortfolioCreateParams {
    pub name: String,
    pub description: Option<String>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceInvestmentCreateParams {
    pub portfolio_id: String,
    pub asset_type: String,
    pub cost_basis: i64,
    pub quantity: f64,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub currency: Option<String>,
    pub purchase_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceInvestmentUpdateParams {
    pub id: String,
    pub current_price: Option<Option<i64>>,
    pub current_value: Option<Option<i64>>,
    pub quantity: Option<f64>,
    pub notes: Option<Option<String>>,
}

// ── Finance Filter Params ────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTransactionFilterParams {
    pub account_id: Option<String>,
    pub tx_type: Option<String>,
    pub category: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub query: Option<String>,
    pub limit: Option<i64>,
}

// ── Finance Report Responses ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceCategoryReportResponse {
    pub total: i64,
    pub breakdown: Vec<FinanceCategoryBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceCategoryBreakdown {
    pub category: String,
    pub amount: i64,
    pub pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTrendPoint {
    pub period: String,
    pub value: i64,
    pub change_pct: Option<f64>,
}

// ── Agent Status ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatusResponse {
    pub status: String,
    pub active_task_count: i64,
    pub focus_task: Option<TaskResponse>,
}

// ── Task Update ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Option<i16>>,
    pub status: Option<String>,
    pub due_date: Option<Option<String>>,
    pub project_id: Option<Option<String>>,
    pub area_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub key_result_id: Option<Option<String>>,
    pub status_label_id: Option<Option<String>>,
    pub position: Option<i32>,
    pub group_id: Option<Option<String>>,
}

// ── Area Params ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaCreateParams {
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub icon: Option<Option<String>>,
}

// ── Project Params ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateParams {
    pub name: String,
    pub area_id: String,
    pub color: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub area_id: Option<String>,
    pub color: Option<String>,
    pub description: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub workflow_id: Option<Option<String>>,
}

// ── Objective Params ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveCreateParams {
    pub title: String,
    pub project_id: String,
    pub description: Option<String>,
    pub priority: Option<i16>,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub priority: Option<Option<i16>>,
    pub due_date: Option<Option<String>>,
}

// ── MCP Settings ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfigResponse {
    pub enabled: bool,
    pub servers: Vec<McpServerResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerResponse {
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_provider: Option<String>,
    pub oauth_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAddServerParams {
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToggleParams {
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRemoveParams {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStartParams {
    pub provider: String,
    pub server_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpUpdateServerParams {
    pub name: String,
    pub transport: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
}

// ── Productivity ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivitySummaryResponse {
    pub date: String,
    pub total_active_secs: i64,
    pub total_focus_secs: i64,
    pub total_break_secs: i64,
    pub total_idle_secs: i64,
    pub productive_secs: i64,
    pub neutral_secs: i64,
    pub distracting_secs: i64,
    pub focus_sessions_count: i64,
    pub avg_session_quality: Option<f64>,
    pub interruptions_count: i64,
    pub context_switches: i64,
    pub top_apps: Vec<AppUsageResponse>,
    pub top_categories: Vec<CategoryUsageResponse>,
    pub top_projects: Vec<ProjectUsageResponse>,
    pub ai_summary: Option<String>,
    pub productivity_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsageResponse {
    pub app_name: String,
    pub duration_secs: i64,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryUsageResponse {
    pub category_id: String,
    pub category: String,
    pub category_type: String,
    pub duration_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedAppResponse {
    pub display_name: String,
    pub app_name: String,
    pub site_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub total_secs: i64,
    pub event_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUsageResponse {
    pub project_id: String,
    pub display_name: String,
    pub duration_secs: i64,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityProjectResponse {
    pub id: String,
    pub display_name: String,
    pub path: String,
    pub url_patterns: Vec<String>,
    pub color: Option<String>,
    pub is_auto_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSessionResponse {
    pub id: String,
    pub action_id: Option<String>,
    pub project_id: Option<String>,
    pub session_type: String,
    pub target_mins: Option<i64>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub actual_mins: Option<i64>,
    pub interruptions: i64,
    pub quality_score: Option<f64>,
    pub completed: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusTimerStatusResponse {
    pub active: bool,
    pub mode: Option<String>,
    pub remaining_secs: Option<u64>,
    pub total_secs: Option<u64>,
    pub session: Option<FocusSessionResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTimelineResponse {
    pub app_name: String,
    pub window_title: Option<String>,
    pub site_name: Option<String>,
    pub category_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub duration_secs: Option<i64>,
    pub is_idle: bool,
    pub project_id: Option<String>,
    pub focus_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCategoryResponse {
    pub id: String,
    pub name: String,
    pub category_type: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub is_system: bool,
    pub rules: Option<CategoryRulesResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRulesResponse {
    pub app_names: Vec<String>,
    pub bundle_ids: Vec<String>,
    pub url_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalProgressResponse {
    pub id: i64,
    pub goal_type: String,
    pub metric: String,
    pub target_value: f64,
    pub current_value: f64,
    pub met: bool,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntryResponse {
    pub id: i64,
    pub description: String,
    pub category_id: Option<String>,
    pub project_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub duration_secs: i64,
    pub source: String,
}

// ── Insight Cards ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightCardResponse {
    pub id: String,
    pub insight_type: String,
    pub title: String,
    pub body: String,
    pub sentiment: String,
    pub metric_value: Option<f64>,
    pub baseline_value: Option<f64>,
    pub date: String,
    pub dismissed: bool,
    pub generated_at: DateTime<Utc>,
}

// ── Weekly Assessment ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyAssessmentResponse {
    pub id: String,
    pub week_start: String,
    pub week_end: String,
    pub avg_score: Option<f64>,
    pub total_focus_mins: Option<i64>,
    pub total_productive_secs: Option<i64>,
    pub total_distracting_secs: Option<i64>,
    pub top_apps: Option<String>,
    pub summary: Option<String>,
}

// ── Distraction ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnedRuleResponse {
    pub id: i64,
    pub pattern: String,
    pub pattern_type: String,
    pub classification: String,
    pub confidence: f64,
    pub hit_count: i64,
    pub last_used_at: String,
    pub created_at: String,
}

// ── Notes ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteResponse {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub body: String,
    pub body_html: Option<String>,
    pub pinned: bool,
    pub archived: bool,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteCreateParams {
    pub title: String,
    pub notebook_id: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_html: Option<String>,
    pub pinned: Option<bool>,
    /// `None` = don't change, `Some(None)` = move to root, `Some(Some(id))` = move to folder
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub notebook_id: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
}

/// Deserializes a field that distinguishes between absent, null, and present.
/// - absent → `None` (don't change)
/// - `null` → `Some(None)` (set to null / move to root)
/// - `"value"` → `Some(Some("value"))` (set to value)
fn deserialize_nullable_field<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub icon: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub parent_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookResponse {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub note_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteLinkResponse {
    pub source_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteVersionResponse {
    pub id: String,
    pub note_id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookCreateParams {
    pub title: String,
    pub parent_id: Option<String>,
    pub icon: Option<String>,
}

// ── Custom Columns ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomColumnResponse {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub column_type: String,
    pub options: Option<Vec<String>>,
    pub position: i32,
    pub width: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomColumnValueResponse {
    pub task_id: String,
    pub column_id: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnCreateParams {
    pub project_id: String,
    pub name: String,
    pub column_type: String,
    pub options: Option<Vec<String>>,
    pub width: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub options: Option<Option<Vec<String>>>,
    pub width: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnReorderParams {
    pub project_id: String,
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnValueSetParams {
    pub task_id: String,
    pub column_id: String,
    pub value: serde_json::Value,
}

// ── App Info ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoResponse {
    pub version: String,
    pub data_dir: String,
    pub setup_completed: bool,
}

// ── Key Result Params ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultCreateParams {
    pub objective_id: String,
    pub title: String,
    pub target_value: Option<f64>,
    pub unit: Option<String>,
    pub tracking_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub due_date: Option<Option<String>>,
}

// ── Timeline / Dashboard ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineQuery {
    pub start_date: String,
    pub end_date: String,
    pub sources: Option<Vec<TimelineSource>>,
    pub include_point_events: Option<bool>,
    /// JS-style timezone offset in minutes (e.g. -420 for UTC+7).
    /// Used to shift day boundaries so local-time events appear on the correct date.
    pub tz_offset_mins: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineResponse {
    pub entries: Vec<TimelineEntry>,
    pub summary: TimelineSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    pub id: String,
    pub source: TimelineSource,
    pub entry_type: TimelineEntryType,
    pub title: String,
    pub description: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_secs: Option<i64>,
    pub entity_id: Option<String>,
    pub entity_route: Option<String>,
    pub color: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimelineSource {
    Productivity,
    Focus,
    Task,
    Todo,
    Note,
    Finance,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimelineEntryType {
    AppUsage,
    FocusSession,
    TaskTimeEntry,
    TaskCreated,
    TaskCompleted,
    TaskUpdated,
    TaskDue,
    NoteCreated,
    NoteUpdated,
    TransactionRecorded,
    ExpenseRecorded,
    IncomeRecorded,
    SystemEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSummary {
    pub total_tracked_secs: i64,
    pub focus_secs: i64,
    pub tasks_completed: i64,
    pub tasks_created: i64,
    pub notes_touched: i64,
    pub transactions_count: i64,
    pub top_apps: Vec<TopAppSummary>,
    pub source_breakdown: Vec<SourceBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopAppSummary {
    pub app_name: String,
    pub duration_secs: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBreakdown {
    pub source: TimelineSource,
    pub duration_secs: i64,
    pub count: i64,
}

// ── Calendar ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventInput {
    pub title: String,
    pub started_at: String,
    pub ended_at: String,
    pub external_uid: String,
    pub calendar_id: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub attendees_count: Option<i64>,
    pub is_recurring: Option<bool>,
    pub recurrence_id: Option<String>,
    pub source: Option<String>,
    pub color: Option<String>,
}
