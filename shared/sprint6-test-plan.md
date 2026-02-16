# Sprint 6: Daily Planning — Test Plan

**Author**: Test Engineer
**Date**: 2026-02-16
**Status**: Ready for Implementation (TDD)
**Unblocks**: Tasks #9 (planning engine) and #10 (skill, CLI, config)

---

## 1. Test Strategy

### Approach: TDD (Test-Driven Development)
1. Write skeleton tests first (this document + test files)
2. Implement the feature code to make tests pass
3. Tests map 1:1 to acceptance criteria — every AC has at least one test

### Test Categories

| Category | File | Count | Framework |
|----------|------|------:|-----------|
| **Scoring algorithm** | `tests/sprint6_daily_planning_integration.rs` | 5 | `#[tokio::test]` |
| **Plan generation** | `tests/sprint6_daily_planning_integration.rs` | 7 | `#[tokio::test]` |
| **Notification format** | `tests/sprint6_daily_planning_integration.rs` | 3 | `#[tokio::test]` |
| **Response parsing** | `tests/sprint6_daily_planning_integration.rs` | 9 | `#[tokio::test]` |
| **Plan execution** | `tests/sprint6_daily_planning_integration.rs` | 6 | `#[tokio::test]` |
| **State machine** | `tests/sprint6_daily_planning_integration.rs` | 6 | `#[tokio::test]` |
| **Cron integration** | `tests/sprint6_daily_planning_integration.rs` | 2 | `#[tokio::test]` |
| **CLI commands** | `tests/sprint6_daily_planning_integration.rs` | 3 | `#[tokio::test]` |
| **Config toggle** | `tests/sprint6_daily_planning_integration.rs` | 3 | `#[tokio::test]` |
| **Calendar integration** | `tests/sprint6_daily_planning_integration.rs` | 2 | `#[tokio::test]` |
| **Unit: scoring** | `crates/agent/src/daily_planning.rs` (inline) | 8 | `#[test]` |
| **Unit: response parser** | `crates/agent/src/daily_planning.rs` (inline) | 10 | `#[test]` |
| **Unit: config** | `crates/config/src/schema/core.rs` (inline) | 3 | `#[test]` |
| **Total** | | **67** | |

---

## 2. Acceptance Criteria Coverage Map

| AC# | Criterion | Test(s) | Type |
|-----|-----------|---------|------|
| AC1 | Cron job runs daily at digest time | `test_cron_triggers_daily_plan`, `test_cron_respects_config_time` | Integration |
| AC2 | Notification shows suggested focus order with reasoning | `test_plan_suggests_top_3_tasks`, `test_plan_includes_reasoning_for_each_suggestion`, `test_plan_notification_format`, `test_plan_includes_due_context` | Integration |
| AC3 | User can reply "yes", "swap X,Y", "skip N", "defer all" | `test_response_accept_variants`, `test_response_swap_variants`, `test_response_skip_variants`, `test_response_defer_variants` + 5 validation tests | Integration + Unit |
| AC4 | Agent auto-focuses tasks on confirmation | `test_accept_plan_focuses_tasks`, `test_accept_with_stale_task`, `test_accept_replaces_existing_focus` | Integration |
| AC5 | `klyntbot todo plan` manually triggers planning | `test_cli_todo_plan_command`, `test_cli_todo_plan_accept_flag`, `test_cli_todo_plan_skip_flag` | Integration |
| AC6 | Config `todo.daily_planning: false` disables feature | `test_config_disabled_skips_planning`, `test_config_daily_planning_defaults_to_true`, `test_config_serde_roundtrip` | Integration + Unit |
| AC7 | Zero clippy warnings | CI check (`cargo clippy --workspace`) | CI |

---

## 3. Unit Test Specifications

### 3.1 Scoring Algorithm Unit Tests

**File**: `crates/agent/src/daily_planning.rs` (inline `#[cfg(test)] mod tests`)

```rust
// Urgency tier tests
test_urgency_overdue_returns_10()       // due_date < now → 10
test_urgency_today_returns_5()          // due_date is today → 5
test_urgency_tomorrow_returns_3()       // due_date is tomorrow → 3
test_urgency_future_returns_1()         // due_date > tomorrow → 1
test_urgency_no_due_date_returns_1()    // due_date is None → 1

// Priority weight tests
test_priority_weight_p1_returns_5()     // P1 → weight 5
test_priority_weight_none_returns_3()   // None → default weight 3

// Composite score test
test_score_formula_correct()
// overdue P1, age=5: (10 × 5) + (5 × 0.1) = 50.5
```

### 3.2 Response Parser Unit Tests

**File**: `crates/agent/src/daily_planning.rs` (inline)

```rust
test_parse_accept_yes()                  // "yes" → Accept
test_parse_accept_y()                    // "y" → Accept
test_parse_accept_ok()                   // "ok" → Accept
test_parse_swap_with_and()               // "swap 1 and 2" → Swap(1,2)
test_parse_swap_with_comma()             // "swap 1,2" → Swap(1,2)
test_parse_skip()                        // "skip 2" → Skip(2)
test_parse_skip_with_hash()              // "skip #2" → Skip(2)
test_parse_defer()                       // "defer" → DeferAll
test_parse_defer_all()                   // "defer all" → DeferAll
test_parse_unrecognized_returns_error()  // "hello" → Err
```

### 3.3 Config Unit Tests

**File**: `crates/config/src/schema/core.rs` (inline, extend existing `mod tests`)

```rust
test_daily_planning_config_defaults()     // enabled=true by default
test_daily_planning_config_serde()        // camelCase roundtrip
test_todo_config_includes_daily_planning() // new field on TodoConfig
```

---

## 4. Integration Test Specifications

### 4.1 Test Data Fixtures

**Location**: `tests/sprint6_daily_planning_integration.rs` (inline fixture function)

```rust
async fn create_planning_test_data() -> (Arc<RwLock<TodoStore>>, TempDir, HashMap<String, String>)
```

Creates 7 tasks covering all scoring tiers:

| Key | Title | Priority | Due | Status | Purpose |
|-----|-------|----------|-----|--------|---------|
| `overdue_p1` | Fix auth token expiry bug | P1 | -2d | Todo | Should rank #1 |
| `today_p2` | Implement user settings page | P2 | today | Todo | Should rank #2 |
| `tomorrow_p3` | Update API docs for v2 | P3 | +1d | Todo | Should rank #3 |
| `future_p4` | Refactor database module | P4 | +7d | Todo | Should NOT be in top 3 |
| `completed` | Already done task | P1 | -1d | Done | Should be excluded |
| `template` | Daily standup template | - | - | Template | Should be excluded |
| `no_priority` | Triage incoming tickets | None | -1d | Todo | Tests default priority |

### 4.2 Full Test List (46 integration tests)

See `tests/sprint6_daily_planning_integration.rs` header comment for the complete AC mapping.

---

## 5. Edge Cases & Error Scenarios

### 5.1 Scoring Edge Cases

| Scenario | Expected Behavior | Test |
|----------|-------------------|------|
| Task with no due date | Uses urgency=1 (future) | `test_urgency_no_due_date_returns_1` |
| Task with no priority | Uses default weight=3 (P3 equivalent) | `test_scoring_no_priority_uses_default_weight` |
| Two tasks with identical scores | Older task (created_at) ranks first | `test_scoring_tiebreak_by_created_at` |
| Only 1 eligible task | Plan has 1 suggestion, no error | `test_plan_with_fewer_than_3_eligible_tasks` |
| Zero eligible tasks | Empty plan, "all clear" message | `test_plan_empty_when_no_eligible_tasks` |

### 5.2 Response Parsing Edge Cases

| Scenario | Expected Behavior | Test |
|----------|-------------------|------|
| `skip 5` on 3-task plan | Error: "only 3 tasks" | `test_response_skip_out_of_range` |
| `swap 1 and 1` | Error: "same position" | `test_response_swap_same_position` |
| `"  YES  "` (whitespace) | Accepts after trim+lowercase | `test_response_case_insensitive` |
| `"yes!"` (punctuation) | Accepts after strip | `test_response_strips_punctuation` |
| Random text | Error with help options | `test_response_unrecognized_input` |

### 5.3 Execution Edge Cases

| Scenario | Expected Behavior | Test |
|----------|-------------------|------|
| Task completed after plan sent | Skip it, promote next, mention in confirmation | `test_accept_with_stale_task` |
| Focus slots full | Unfocus old tasks, focus new ones | `test_accept_replaces_existing_focus` |
| Plan already confirmed today | "Already confirmed" message | `test_plan_already_confirmed_today` |
| Plan expired (midnight) | "Plan expired" message | `test_plan_expires_at_midnight` |

---

## 6. Dependencies & Mocks

### 6.1 Required Mocks

| Mock | Purpose | Location |
|------|---------|----------|
| `MockProvider` | LLM responses (not needed — planning is pure logic) | `tests/mock_provider.rs` (existing) |
| `MockCalendarHandler` | Calendar events for plan display | `tests/mock_calendar_handler.rs` (existing) |

### 6.2 No New Mocks Needed

The daily planning engine is pure computation:
- **Scoring**: Takes `&[Todo]` → returns scores (no I/O)
- **Response parsing**: Takes `&str` → returns `PlanAction` (no I/O)
- **Plan generation**: Reads from `TodoStore` (already uses `TempDir` in tests)

The only external dependency is `CalendarHandler` for optional calendar events, which already has a mock.

---

## 7. Test Execution

```bash
# Run all Sprint 6 tests
cargo test --test sprint6_daily_planning_integration

# Run scoring tests only
cargo test --test sprint6_daily_planning_integration test_scoring

# Run response parsing tests
cargo test --test sprint6_daily_planning_integration test_response

# Run with output
cargo test --test sprint6_daily_planning_integration -- --nocapture

# Run unit tests in the agent crate
cargo test -p agent daily_planning

# Run unit tests in the config crate
cargo test -p config daily_planning
```

---

## 8. Coverage Requirements

- Every acceptance criterion (AC1-AC7) must have at least 1 passing test
- Response parser must cover all patterns from UX spec §4.1
- Scoring algorithm must cover all 4 urgency tiers
- Config serde roundtrip must pass
- No regressions: existing 1424+ tests must continue passing
