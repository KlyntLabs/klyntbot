# Web Dashboard Test Plan

**Date:** 2026-02-24
**Status:** FINAL
**Covers:** Tasks 1-17 (Backend Foundation + Frontend Scaffolding)
**References:**
- Architecture plan: `/Users/jayden/.claude/plans/swirling-knitting-dolphin.md`
- UX spec: `docs/plans/2026-02-24-web-dashboard-ux-spec.md`
- Design doc: `docs/plans/2026-02-24-web-dashboard-design.md`

---

## 1. Test Categories

| Category | Scope | Framework | Location |
|----------|-------|-----------|----------|
| **Serialization unit tests** | Derive correctness for all row types | Rust `#[test]` | `crates/storage/src/rows/serialization_tests.rs` |
| **AgentLoop refactor unit tests** | Two-phase construction, backward compat | `#[tokio::test]` | `crates/agent/src/agent_loop/refactor_tests.rs` |
| **AgentEvent serialization unit tests** | Tagged JSON, camelCase variant names | `#[test]` | `crates/agent/src/events_tests.rs` |
| **Dashboard REST integration tests** | All HTTP endpoints | `#[tokio::test]` + `tower::ServiceExt` | `crates/dashboard/tests/` |
| **WebSocket integration tests** | Upgrade, streaming, interaction, cancel | `#[tokio::test]` | `crates/dashboard/tests/ws_test.rs` |
| **CORS/error middleware tests** | Headers, error shapes | `#[tokio::test]` | `crates/dashboard/tests/{cors,error}_test.rs` |
| **Frontend component tests** | Layout, navigation, routing | Vitest + React Testing Library | `frontend/src/app/**/__tests__/` |
| **Frontend hook tests** | useApi, useAgent, ws client | Vitest (jsdom) | `frontend/src/lib/**/__tests__/` |

---

## 2. Acceptance Criteria Coverage Matrix

### Task 1: Serialization Big-Bang (AC-1.x)

| AC | Description | Test File | Test Function(s) |
|----|-------------|-----------|-----------------|
| AC-1.1 | TodoRow serializes to camelCase JSON | `serialization_tests.rs` | `todo_row_camel_case`, `todo_row_round_trip` |
| AC-1.2 | TodoAttachmentRow, TodoTimeEntryRow, TodoDependencyRow serialize | `serialization_tests.rs` | `todo_attachment_row_camel_case`, `todo_time_entry_row_camel_case`, `todo_dependency_row_camel_case` |
| AC-1.3 | ProjectRow serializes | `serialization_tests.rs` | `project_row_camel_case` |
| AC-1.4 | PlanRow, PlanStepRow serialize | `serialization_tests.rs` | `plan_row_camel_case`, `plan_step_row_camel_case` |
| AC-1.5 | SessionRow, SessionMessageRow, SessionListRow serialize | `serialization_tests.rs` | `session_row_camel_case`, `session_message_row_camel_case`, `session_list_row_camel_case` |
| AC-1.6 | CalendarSyncStateRow, CalendarEventCacheRow serialize | `serialization_tests.rs` | `calendar_sync_state_row_camel_case`, `calendar_event_cache_row_camel_case` |
| AC-1.7 | CronJobRow serializes | `serialization_tests.rs` | `cron_job_row_camel_case` |
| AC-1.8 | Finance row types serialize (all 10 types) | `serialization_tests.rs` | `finance_account_row_camel_case`, etc. |
| AC-1.9 | Aggregate types (TodoSummary, ProjectWithStats) serialize | `serialization_tests.rs` | `todo_summary_camel_case`, `project_with_stats_camel_case` |
| AC-1.10 | No snake_case keys appear in any serialized output | `serialization_tests.rs` | `no_snake_case_keys_in_any_row` |

### Task 2: AgentLoop Refactor (AC-2.x)

| AC | Description | Test File | Test Function(s) |
|----|-------------|-----------|-----------------|
| AC-2.1 | `take_inbound_rx()` returns the receiver | `refactor_tests.rs` | `take_inbound_rx_returns_receiver` |
| AC-2.2 | After `take_inbound_rx()`, calling it again returns None/panics | `refactor_tests.rs` | `take_inbound_rx_leaves_none` |
| AC-2.3 | `run_with_rx()` takes `&self` (no mut) | `refactor_tests.rs` | `run_with_rx_takes_shared_ref` |
| AC-2.4 | Existing `run(&mut self)` still compiles and works | `refactor_tests.rs` | `run_backward_compat` |
| AC-2.5 | `Arc<AgentLoop>` can call `process_direct_streaming()` | `refactor_tests.rs` | `arc_agent_loop_process_direct_streaming` |
| AC-2.6 | `skill_manager()` accessor returns SkillManager | `refactor_tests.rs` | `skill_manager_accessor` |

### Task 3: AgentEvent Serialize (AC-3.x)

| AC | Description | Test File | Test Function(s) |
|----|-------------|-----------|-----------------|
| AC-3.1 | `ContentChunk` serializes as `{"type":"contentChunk","data":"..."}` | `events_tests.rs` | `content_chunk_serializes_with_data_field` |
| AC-3.2 | `Done` serializes as `{"type":"done","content":"..."}` | `events_tests.rs` | `done_serializes_with_content_field` |
| AC-3.3 | `Error` serializes as `{"type":"error","message":"..."}` | `events_tests.rs` | `error_serializes_with_message_field` |
| AC-3.4 | `ToolStart` serializes with `name` + `args` | `events_tests.rs` | `tool_start_serializes` |
| AC-3.5 | `ToolEnd` serializes with camelCase `durationMs` | `events_tests.rs` | `tool_end_serializes_with_duration_ms` |
| AC-3.6 | `ClassificationComplete` serializes with camelCase fields | `events_tests.rs` | `classification_complete_serializes` |
| AC-3.7 | All 11 variants have correct `type` tag (camelCase) | `events_tests.rs` | `all_variants_have_camel_case_type_tag` |

### Task 4: GatewayConfig Default (AC-4.x)

| AC | Description | Test File | Test Function(s) |
|----|-------------|-----------|-----------------|
| AC-4.1 | Default gateway host is `127.0.0.1` | (compile/config test) | Covered in `health_test.rs` AppState fixture |

### Task 5: Dashboard Crate Scaffold (AC-5.x)

| AC | Description | Test File | Test Function(s) |
|----|-------------|-----------|-----------------|
| AC-5.1 | `GET /api/health` returns 200 `{"status":"ok"}` | `health_test.rs` | `health_returns_200_ok`, `health_response_is_json` |
| AC-5.2 | CORS headers present on all API responses | `cors_test.rs` | `cors_headers_present_on_api_response`, `cors_preflight_returns_204` |
| AC-5.3 | ApiError serializes to `{"status":N,"message":"..."}` | `error_test.rs` | `api_error_json_shape`, `klyntbot_error_converts_to_api_error` |

### Task 6-14: REST APIs (AC-6.x – AC-14.x)

| AC | Resource | Test File | Key Test Functions |
|----|----------|-----------|-------------------|
| AC-6.x | Tasks CRUD | `tasks_api_test.rs` | See §3.1 |
| AC-7.x | Projects CRUD | `projects_api_test.rs` | See §3.2 |
| AC-8.x | Plans + status transitions | `plans_api_test.rs` | See §3.3 |
| AC-9.x | Sessions list/get/delete | `sessions_api_test.rs` | See §3.4 |
| AC-10.x | Status endpoint | `status_api_test.rs` | See §3.5 |
| AC-11.x | Cron CRUD + toggle | `cron_api_test.rs` | See §3.6 |
| AC-12.x | Calendar events + sync | `calendar_api_test.rs` | See §3.7 |
| AC-13.x | Skills list + toggle | `skills_api_test.rs` | See §3.8 |
| AC-14.x | Finance 6 sub-resources | `finance_api_test.rs` | See §3.9 |
| AC-14s.x | Settings get/patch + secret redaction | `settings_api_test.rs` | See §3.10 |

### WebSocket (AC-WS.x)

| AC | Description | Test File | Test Function(s) |
|----|-------------|-----------|-----------------|
| AC-WS.1 | `GET /ws` upgrades to WebSocket | `ws_test.rs` | `ws_upgrade_succeeds` |
| AC-WS.2 | `chat.send` triggers agent streaming | `ws_test.rs` | `chat_send_produces_content_chunk_events` |
| AC-WS.3 | `done` event ends stream, re-enables send | `ws_test.rs` | `done_event_closes_stream` |
| AC-WS.4 | `chat.cancel` stops streaming | `ws_test.rs` | `chat_cancel_stops_stream` |
| AC-WS.5 | `interaction.respond` delivers FormResponse | `ws_test.rs` | `interaction_respond_delivers_response` |
| AC-WS.6 | Error from agent propagates as `error` frame | `ws_test.rs` | `agent_error_propagates_as_error_frame` |
| AC-WS.7 | One-stream-at-a-time constraint enforced | `ws_test.rs` | `second_chat_send_while_streaming_is_rejected` |
| AC-WS.8 | Disconnect cancels in-flight processing | `ws_test.rs` | `disconnect_cancels_inflight` |

### Frontend Tasks 15-17 (AC-15.x – AC-17.x)

| AC | Description | Test File | Test Function(s) |
|----|-------------|-----------|-----------------|
| AC-15.1 | Project structure + deps present | `routes.test.tsx` | (import checks) |
| AC-15.3 | Vite proxy config resolves API paths | `api.test.ts` | `api_fetch_uses_relative_paths` |
| AC-15.4 | Codex dark theme CSS vars defined | `Layout.test.tsx` | `layout_uses_codex_theme` |
| AC-15.5 | TypeScript strict mode | (tsconfig — compile check) | N/A |
| AC-16.1 | Nav rail renders with 7 items + settings | `Layout.test.tsx` | `nav_rail_renders_all_items` |
| AC-16.2 | Active route highlighted with accent | `Layout.test.tsx` | `active_route_has_accent_color` |
| AC-16.3 | All 10 routes resolve without crashing | `routes.test.tsx` | `all_ten_routes_render` |
| AC-16.4 | Placeholder pages render | `routes.test.tsx` | `placeholder_pages_render_heading` |
| AC-16.5 | Setup page renders outside layout | `routes.test.tsx` | `setup_route_renders_outside_layout` |
| AC-17.1 | `apiFetch<T>` throws ApiError on non-2xx | `api.test.ts` | `api_fetch_throws_on_non_2xx` |
| AC-17.2 | AgentSocket auto-reconnects after 2s | `ws.test.ts` | `agent_socket_reconnects_after_disconnect` |
| AC-17.3 | `useAgent` exposes messages, isStreaming, sendMessage, cancel | `useAgent.test.ts` | `use_agent_exposes_all_state` |
| AC-17.4 | `useApi` exposes data, loading, error, refetch | `useApi.test.ts` | `use_api_exposes_loading_error_data` |
| AC-17.5 | TypeScript types match camelCase JSON | `api.test.ts` | `types_use_camel_case_keys` |
| AC-17.6 | Event type → thinking phase mapping | `useAgent.test.ts` | `classification_event_sets_thinking_phase` |

---

## 3. Per-Resource Test Inventory

### 3.1 Tasks API (`tasks_api_test.rs`)

- `get_tasks_empty_returns_empty_array`
- `post_task_creates_with_title_only`
- `post_task_all_optional_fields`
- `post_task_missing_title_returns_422`
- `get_task_by_id_returns_todo_row`
- `get_task_by_id_not_found_returns_404`
- `patch_task_updates_status`
- `patch_task_updates_priority`
- `patch_task_partial_update_preserves_other_fields`
- `delete_task_returns_204`
- `delete_task_not_found_returns_404`
- `get_tasks_filter_by_status`
- `get_tasks_filter_by_project_id`
- `get_tasks_filter_by_priority_min`
- `get_tasks_filter_by_tags`
- `get_tasks_limit_param`
- `get_task_summary_returns_todo_summary`
- `get_subtasks_for_parent`
- `get_attachments_for_task`
- `get_time_entries_for_task`
- `post_time_entry_creates_entry`
- `post_focus_sets_focus`
- `delete_focus_clears_focus`
- `response_fields_are_camel_case`

### 3.2 Projects API (`projects_api_test.rs`)

- `get_projects_empty_returns_array`
- `post_project_creates_project`
- `get_project_by_id`
- `get_project_by_id_with_stats`
- `patch_project_updates_fields`
- `delete_project_returns_204`
- `get_projects_filter_by_status`
- `response_fields_are_camel_case`

### 3.3 Plans API (`plans_api_test.rs`)

- `get_plans_returns_list`
- `post_plan_creates_plan_in_draft`
- `get_plan_by_id_includes_steps`
- `patch_plan_updates_title`
- `patch_plan_status_draft_to_approved`
- `patch_plan_status_approved_to_executing`
- `patch_plan_status_invalid_transition_returns_409`
- `patch_plan_status_to_abandoned_from_any_state`
- `get_plan_steps`

### 3.4 Sessions API (`sessions_api_test.rs`)

- `get_sessions_returns_list`
- `get_session_by_key_includes_messages`
- `get_session_not_found_returns_404`
- `delete_session_returns_204`

### 3.5 Status API (`status_api_test.rs`)

- `get_status_returns_version`
- `get_status_includes_model_name`
- `get_status_includes_uptime`
- `get_status_includes_storage_stats`

### 3.6 Cron API (`cron_api_test.rs`)

- `get_cron_empty_returns_array`
- `post_cron_creates_job`
- `patch_cron_toggle_enables`
- `patch_cron_toggle_disables`
- `delete_cron_job_returns_204`

### 3.7 Calendar API (`calendar_api_test.rs`)

- `get_calendar_events_returns_array`
- `get_calendar_events_filter_by_provider`
- `get_calendar_sync_status_returns_state`
- `post_calendar_sync_returns_202`

### 3.8 Skills API (`skills_api_test.rs`)

- `get_skills_returns_list`
- `patch_skill_toggle_enabled`
- `patch_skill_toggle_disabled`
- `patch_skill_not_found_returns_404`

### 3.9 Finance API (`finance_api_test.rs`)

- Accounts: `get`, `post`, `get_by_id`, `patch`, `delete`
- Transactions: `get`, `post`, `get_by_id`, `patch`, `delete`
- Budgets: `get`, `post`, `patch`, `delete`, `get_usage`
- Investments: `get`, `post`, `patch`, `delete`
- Goals: `get`, `post`, `patch`, `delete`
- Liabilities: `get`, `post`, `patch`, `delete`
- `finance_amounts_are_integer_cents`

### 3.10 Settings API (`settings_api_test.rs`)

- `get_settings_all_sections_redacts_secrets`
- `get_settings_section_todo`
- `get_settings_section_not_found_returns_404`
- `patch_settings_section_merges_patch`
- `patch_settings_section_ignores_redacted_placeholder_values`
- `secret_fields_replaced_with_bullets`
- `api_key_field_redacted`
- `bot_token_field_redacted`
- `data_dir_not_patchable`

---

## 4. Test Infrastructure

### 4.1 Backend

- **Test helper**: `tests/common/mod.rs` — `build_test_app()` returns `axum::Router` bound to in-memory SQLite
- **AppState fixture**: Reuses `StoragePool::connect_in_memory()`, `MockProvider`, `Arc<AgentLoop>` from `tests/common/mock_provider.rs`
- **HTTP client**: `tower::ServiceExt::oneshot()` for unit-style HTTP tests (no actual TCP)
- **WebSocket**: `tokio-tungstenite` test client against a `tokio::net::TcpListener`-bound server

### 4.2 Frontend

- **Test runner**: Vitest
- **DOM environment**: jsdom (configured in `vite.config.ts` test section)
- **Component testing**: `@testing-library/react`
- **Fetch mock**: `vi.fn()` replacing `window.fetch`
- **WebSocket mock**: Custom `MockWebSocket` class replacing `window.WebSocket`
- **Router wrap**: `MemoryRouter` from `react-router`

### 4.3 Test Data Fixtures Strategy

All backend tests use ephemeral in-memory SQLite — no external DB needed. Frontend tests use mock data inline, never hitting real HTTP endpoints.

**Backend fixture pattern:**
```rust
async fn setup() -> (Router, StoragePool) {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    let app = build_test_app(repos).await;
    (app, pool)
}
```

**Frontend fixture pattern:**
```typescript
const mockTask: Task = {
  id: 'test-id',
  title: 'Test Task',
  status: 'todo',
  // ... other required fields with sensible defaults
};
```

---

## 5. Known Test Gaps / Out-of-Scope

- Performance / load testing — deferred
- Mobile responsiveness — out of scope (desktop-only by design)
- Accessibility automation (axe-core) — recommended as follow-up after Task 15-17 implementation
- E2E (Playwright/Cypress) — deferred to post-implementation verification sprint
- Finance FIRE calculator accuracy — client-side math, verified by unit test of formula constants
