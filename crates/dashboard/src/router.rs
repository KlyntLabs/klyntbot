//! Axum router: assembles all routes, CORS layer, and SPA fallback.

use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};

use crate::api::{
    calendar::{get_sync_status, list_events, trigger_sync},
    cron::{create_cron, delete_cron, list_cron, run_cron, toggle_cron},
    finance::{
        create_account, create_budget, create_goal, create_investment, create_liability,
        create_transaction, delete_account, delete_budget, delete_goal, delete_investment,
        delete_liability, delete_transaction, get_account, get_budget_usage, get_transaction,
        list_accounts, list_budgets, list_goals, list_investments, list_liabilities,
        list_transactions, patch_account, patch_budget, patch_goal, patch_investment,
        patch_liability, patch_transaction,
    },
    health::health,
    plans::{
        create_plan, delete_plan, get_plan, get_plan_steps, list_plans, patch_plan,
        update_plan_status,
    },
    projects::{create_project, delete_project, get_project, list_projects, patch_project},
    sessions::{delete_session, get_session, list_sessions},
    settings::{get_settings, get_settings_section, patch_settings_section},
    skills::{create_skill, delete_skill, get_skill, list_skills, update_skill},
    status::get_status,
    tasks::{
        add_time_entry, create_task, delete_focus, delete_task, get_attachments, get_subtasks,
        get_summary, get_task, get_time_entries, list_tasks, patch_task, set_focus,
    },
};
use crate::embed::spa_handler;
use crate::state::AppState;
use crate::ws::ws_handler;

/// Build the complete Axum router with all middleware attached.
///
/// Route tree:
/// ```text
/// GET  /ws                               → WebSocket streaming
/// GET  /api/health                       → liveness probe
/// GET  /api/status                       → agent status + uptime
/// GET  /api/tasks                        → list tasks
/// POST /api/tasks                        → create task
/// GET  /api/tasks/summary                → task counts
/// GET  /api/tasks/:id                    → get task
/// PATCH  /api/tasks/:id                  → update task
/// DELETE /api/tasks/:id                  → delete task
/// GET  /api/tasks/:id/subtasks           → subtasks
/// GET  /api/tasks/:id/attachments        → attachments
/// GET  /api/tasks/:id/time-entries       → time entries
/// POST /api/tasks/:id/time-entries       → add time entry
/// POST /api/tasks/:id/focus              → set focus
/// DELETE /api/tasks/:id/focus            → clear focus
/// GET  /api/projects                     → list projects
/// POST /api/projects                     → create project
/// GET  /api/projects/:id                 → get project
/// PATCH  /api/projects/:id               → update project
/// DELETE /api/projects/:id               → delete project
/// GET  /api/plans                        → list plans
/// POST /api/plans                        → create plan
/// GET  /api/plans/:id                    → get plan + steps
/// PATCH  /api/plans/:id                  → update plan metadata
/// DELETE /api/plans/:id                  → delete plan (cascades steps)
/// GET  /api/plans/:id/steps              → get steps only
/// POST /api/plans/:id/status             → transition plan status
/// GET  /api/sessions                     → list sessions
/// GET  /api/sessions/:id                 → get session + messages
/// DELETE /api/sessions/:id               → delete session
/// GET  /api/cron                         → list cron jobs
/// POST /api/cron                         → create cron job
/// PATCH  /api/cron/:id/toggle            → enable/disable cron job
/// POST /api/cron/:id/run                 → manually trigger cron job
/// DELETE /api/cron/:id                   → delete cron job
/// GET  /api/calendar/events              → calendar events
/// GET  /api/calendar/sync-status         → sync state
/// POST /api/calendar/sync                → trigger sync
/// GET  /api/skills                       → list skills
/// POST /api/skills                       → create workspace skill
/// GET  /api/skills/:name                 → get skill detail
/// PATCH  /api/skills/:name               → toggle skill
/// DELETE /api/skills/:name               → delete workspace skill
/// GET  /api/finance/accounts             → accounts
/// POST /api/finance/accounts             → create account
/// … (full finance CRUD)
/// GET  /api/settings                     → full config (secrets redacted)
/// GET  /api/settings/:section            → config section
/// PATCH  /api/settings/:section          → merge-patch config section
/// GET  /*                                → SPA fallback
/// ```
pub fn build(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api = Router::new()
        .route("/health", get(health))
        .route("/status", get(get_status))
        // Tasks
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/summary", get(get_summary))
        .route("/tasks/{id}", get(get_task).patch(patch_task).delete(delete_task))
        .route("/tasks/{id}/subtasks", get(get_subtasks))
        .route("/tasks/{id}/attachments", get(get_attachments))
        .route("/tasks/{id}/time-entries", get(get_time_entries).post(add_time_entry))
        .route("/tasks/{id}/focus", post(set_focus).delete(delete_focus))
        // Projects
        .route("/projects", get(list_projects).post(create_project))
        .route("/projects/{id}", get(get_project).patch(patch_project).delete(delete_project))
        // Plans
        .route("/plans", get(list_plans).post(create_plan))
        .route("/plans/{id}", get(get_plan).patch(patch_plan).delete(delete_plan))
        .route("/plans/{id}/steps", get(get_plan_steps))
        .route("/plans/{id}/status", post(update_plan_status))
        // Sessions
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", get(get_session).delete(delete_session))
        // Cron
        .route("/cron", get(list_cron).post(create_cron))
        .route("/cron/{id}/toggle", patch(toggle_cron))
        .route("/cron/{id}/run", post(run_cron))
        .route("/cron/{id}", delete(delete_cron))
        // Calendar
        .route("/calendar/events", get(list_events))
        .route("/calendar/sync-status", get(get_sync_status))
        .route("/calendar/sync", post(trigger_sync))
        // Skills
        .route("/skills", get(list_skills).post(create_skill))
        .route("/skills/{name}", get(get_skill).patch(update_skill).delete(delete_skill))
        // Finance
        .route("/finance/accounts", get(list_accounts).post(create_account))
        .route("/finance/accounts/{id}", get(get_account).patch(patch_account).delete(delete_account))
        .route("/finance/transactions", get(list_transactions).post(create_transaction))
        .route("/finance/transactions/{id}", get(get_transaction).patch(patch_transaction).delete(delete_transaction))
        .route("/finance/budgets", get(list_budgets).post(create_budget))
        .route("/finance/budgets/{id}", patch(patch_budget).delete(delete_budget))
        .route("/finance/budgets/usage", get(get_budget_usage))
        .route("/finance/investments", get(list_investments).post(create_investment))
        .route("/finance/investments/{id}", patch(patch_investment).delete(delete_investment))
        .route("/finance/goals", get(list_goals).post(create_goal))
        .route("/finance/goals/{id}", patch(patch_goal).delete(delete_goal))
        .route("/finance/liabilities", get(list_liabilities).post(create_liability))
        .route("/finance/liabilities/{id}", patch(patch_liability).delete(delete_liability))
        // Settings
        .route("/settings", get(get_settings))
        .route("/settings/{section}", get(get_settings_section).patch(patch_settings_section));

    Router::new()
        .route("/ws", get(ws_handler))
        .nest("/api", api)
        .fallback(spa_handler)
        .layer(cors)
        .layer(CompressionLayer::new())
        .with_state(state)
}
