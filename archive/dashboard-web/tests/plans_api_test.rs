//! Integration tests for /api/plans endpoints.
//!
//! Covers AC-8.x: CRUD + status machine transitions.
//! State machine: Draft → Approved → Executing → Completed
//!                              ↘                  ↘
//!                           Abandoned            Failed

mod common;

// ── List ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_plans_returns_list() {
    // AC-8.1: GET /api/plans returns Vec<PlanRow>
    // TODO: implement
}

#[tokio::test]
async fn get_plans_filter_by_status() {
    // AC-8.1: GET /api/plans?status=draft returns only Draft plans
    // TODO: implement
}

// ── Create ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn post_plan_creates_in_draft_status() {
    // AC-8.2: Newly created plans have status "draft"
    // POST /api/plans { "title": "...", "description": "..." } → status = "draft"
    // TODO: implement
}

// ── Read ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_plan_by_id_includes_steps() {
    // AC-8.3: GET /api/plans/{id} returns { plan: PlanRow, steps: Vec<PlanStepRow> }
    // TODO: implement
}

#[tokio::test]
async fn get_plan_by_id_not_found_returns_404() {
    // AC-8.3: Nonexistent plan ID → 404
    // TODO: implement
}

#[tokio::test]
async fn get_plan_steps_endpoint() {
    // AC-8.3: GET /api/plans/{id}/steps returns just Vec<PlanStepRow>
    // TODO: implement
}

// ── Update ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn patch_plan_updates_title() {
    // AC-8.4: PATCH /api/plans/{id} { "title": "new title" } → 200
    // TODO: implement
}

// ── Status Transitions ────────────────────────────────────────────────────────

#[tokio::test]
async fn patch_plan_status_draft_to_approved() {
    // AC-8.5: Valid transition Draft → Approved
    // PATCH /api/plans/{id}/status { "status": "approved" } → 200
    // TODO: implement
}

#[tokio::test]
async fn patch_plan_status_approved_to_executing() {
    // AC-8.5: Valid transition Approved → Executing
    // TODO: implement
}

#[tokio::test]
async fn patch_plan_status_executing_to_completed() {
    // AC-8.5: Valid transition Executing → Completed
    // TODO: implement
}

#[tokio::test]
async fn patch_plan_status_any_to_abandoned() {
    // AC-8.5: Any active state can transition to Abandoned
    // Test from Draft, Approved, Executing
    // TODO: implement
}

#[tokio::test]
async fn patch_plan_status_invalid_transition_returns_409() {
    // AC-8.5: Invalid transitions are rejected with 409 Conflict
    // e.g., Draft → Executing (skipping Approved) → 409
    // e.g., Completed → Approved → 409
    // TODO: implement
}

#[tokio::test]
async fn patch_plan_status_completed_is_terminal() {
    // AC-8.5: No transitions allowed from Completed
    // TODO: implement
}

#[tokio::test]
async fn patch_plan_status_failed_is_terminal() {
    // AC-8.5: No transitions allowed from Failed
    // TODO: implement
}

#[tokio::test]
async fn plan_response_fields_are_camel_case() {
    // AC-1.4: Plan response fields use camelCase (sessionKey, currentStepIndex, etc.)
    // TODO: implement
}
