# Test Suite Cleanup & Restructuring — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce ~2,174 tests to ~1,700 by removing structural/duplicate tests, restructure from 22 flat files into 3 domain+type binaries, and standardize naming.

**Architecture:** Move integration tests into `tests/{integration,e2e,unit}/` subdirectories with shared `main.rs` entry points. Each directory compiles as one binary (3 total, down from 22). Merge overlapping test files by domain, remove pure structural tests, collapse bloated enum/serialization tests via parametric loops.

**Tech Stack:** Rust, cargo-nextest, `#[path]` attribute for shared test fixtures.

---

### Task 1: Record Baseline & Create Directory Scaffold

**Files:**
- Create: `tests/integration/main.rs`
- Create: `tests/e2e/main.rs`
- Create: `tests/unit/main.rs`

**Step 1: Record baseline test count**

Run: `cargo nextest run --workspace 2>&1 | tail -5`
Expected: ~2,174 tests, all passing. Save the exact count.

**Step 2: Create directory scaffold**

```bash
mkdir -p tests/integration tests/e2e tests/unit
```

**Step 3: Create `tests/integration/main.rs`**

```rust
#[path = "../common/mod.rs"]
mod common;

mod calendar;
mod channels;
mod finance;
mod learning;
mod memory;
mod sessions;
mod skills;
```

**Step 4: Create `tests/e2e/main.rs`**

```rust
#[path = "../common/mod.rs"]
mod common;

mod agent_loop;
mod agent_pipeline;
mod ask_user;
mod reminders;
```

**Step 5: Create `tests/unit/main.rs`**

```rust
mod channels;
mod config;
mod providers;
```

**Step 6: Create empty placeholder modules**

Create each file referenced in the main.rs files as empty files (or with a single `// placeholder` comment) so the project compiles. We'll fill them in subsequent tasks.

```bash
# integration/
touch tests/integration/calendar.rs tests/integration/channels.rs \
      tests/integration/finance.rs tests/integration/learning.rs \
      tests/integration/memory.rs tests/integration/sessions.rs \
      tests/integration/skills.rs

# e2e/
touch tests/e2e/agent_loop.rs tests/e2e/agent_pipeline.rs \
      tests/e2e/ask_user.rs tests/e2e/reminders.rs

# unit/
touch tests/unit/channels.rs tests/unit/config.rs tests/unit/providers.rs
```

**Step 7: Verify compile**

Run: `cargo nextest run --test integration --test e2e --test unit 2>&1 | tail -5`
Expected: 0 tests run, no compile errors.

**Step 8: Commit**

```
chore(tests): scaffold new test directory structure
```

---

### Task 2: Delete Pure Structural Test Files

**Files:**
- Delete: `tests/phase4_queue_tests.rs` (5 tests — struct field checks + mpsc channel tests)
- Delete: `tests/phase4_learning_events_tests.rs` (5 tests — broadcast channel pub/sub)
- Delete: `tests/browser_integration_tests.rs` (4 tests — feature-gated stub, requires external daemon)

**Step 1: Delete the files**

```bash
rm tests/phase4_queue_tests.rs tests/phase4_learning_events_tests.rs tests/browser_integration_tests.rs
```

**Step 2: Verify remaining tests pass**

Run: `cargo nextest run --workspace 2>&1 | tail -5`
Expected: 14 fewer tests than baseline, all passing.

**Step 3: Commit**

```
refactor(tests): remove structural test files (phase4_queue, phase4_learning_events, browser_integration)
```

---

### Task 3: Migrate Calendar Tests

Merge `sprint3_calendar_integration.rs` + `calendar_edge_cases.rs` → `tests/integration/calendar.rs`.

**Files:**
- Source: `tests/sprint3_calendar_integration.rs` (5 tests)
- Source: `tests/calendar_edge_cases.rs` (15 tests)
- Target: `tests/integration/calendar.rs`
- Delete: both source files after migration

**Step 1: Combine both files into `tests/integration/calendar.rs`**

Copy all imports, helpers, and test functions from both files into `integration/calendar.rs`. Use section headers:

```rust
//! Calendar integration tests — reconciliation, sync, conflict detection, edge cases.

use super::common;
// ... (merged imports from both files)

// ── Reconciliation ──────────────────────────────────────────

// Tests from sprint3_calendar_integration.rs (deduplicated)

// ── Conflict Detection ──────────────────────────────────────

// Tests from calendar_edge_cases.rs

// ── Edge Cases ──────────────────────────────────────────────

// Remaining edge case tests
```

**Step 2: Deduplicate**

Remove `test_empty_sync_state_initialization` from `calendar_edge_cases.rs` — it only checks that `None` fields are `None` (pure structural).

Check for overlap between `sprint3_calendar_integration.rs::test_reconcile_event_time_changed_updates_todo` and `calendar_edge_cases.rs::test_event_time_changed` — keep the more comprehensive version.

**Step 3: Rename test functions**

Apply naming convention: drop `test_` prefix, drop sprint/phase codes, use `<subject>_<verb>_<condition>`.

Examples:
- `test_reconcile_event_time_changed_updates_todo` → `reconcile_updates_todo_on_time_change`
- `test_conflict_detection_identical_events` → `conflict_detection_skips_identical_events`
- `test_reconcile_marks_done_on_caldav_complete` → `reconcile_marks_done_on_caldav_complete`

**Step 4: Fix imports**

Replace `mod common;` and direct `use` statements with `use super::common;` or `use crate::common;` as needed since this is now a submodule of the integration binary.

**Step 5: Delete old files**

```bash
rm tests/sprint3_calendar_integration.rs tests/calendar_edge_cases.rs
```

**Step 6: Verify**

Run: `cargo nextest run --test integration -E 'test(calendar)' 2>&1 | tail -10`
Expected: ~18-19 calendar tests pass (1-2 structural tests removed).

**Step 7: Commit**

```
refactor(tests): merge calendar tests into tests/integration/calendar.rs
```

---

### Task 4: Migrate Channel Tests

Split `channel_tests.rs` + `channel_unit_tests.rs` → `tests/integration/channels.rs` (bus/routing) + `tests/unit/channels.rs` (allowlist/config).

**Files:**
- Source: `tests/channel_tests.rs` (13 tests)
- Source: `tests/channel_unit_tests.rs` (31 tests)
- Source: `tests/integration_tests.rs` (cherry-pick `test_bus_message_ordering`)
- Target: `tests/integration/channels.rs`
- Target: `tests/unit/channels.rs`
- Delete: `tests/channel_tests.rs`, `tests/channel_unit_tests.rs`

**Step 1: Populate `tests/unit/channels.rs`**

Move all allowlist tests and per-channel `is_allowed` tests from `channel_unit_tests.rs` (the pure logic tests that don't need async or storage):
- `test_check_allowlist_empty_allows_all` → `allowlist_empty_allows_all`
- `test_check_allowlist_exact_match` → `allowlist_exact_match`
- `test_check_allowlist_compound_id` → `allowlist_compound_id_matches`
- `test_check_allowlist_no_partial_match` → `allowlist_rejects_partial_match`
- `test_discord_is_allowed_with_allowlist` → `discord_allowlist_filters`
- `test_slack_is_allowed_with_allowlist` → `slack_allowlist_filters`
- (and similar per-channel tests)

Also move config validation tests (e.g., default config checks, missing field handling) from `channel_unit_tests.rs`.

**Step 2: Populate `tests/integration/channels.rs`**

Move MessageBus tests (async, test real pub/sub):
- From `channel_tests.rs`: `test_message_routing_through_channels` → `message_routes_through_bus`
- From `integration_tests.rs`: `test_bus_message_ordering` → `bus_preserves_message_ordering`
- From `channel_tests.rs`: any remaining bus/routing tests
- From `channel_unit_tests.rs`: formatter tests, rate limiting tests (anything that tests real channel behavior)

**Step 3: Remove duplicates**

- `test_full_message_flow` (from `integration_tests.rs`) duplicates `test_message_routing_through_channels`. Remove the one from `integration_tests.rs`.
- `test_channel_manager_initialization` — just verifies no panic on construction. **Remove.**

**Step 4: Rename all test functions per convention**

**Step 5: Delete old files**

```bash
rm tests/channel_tests.rs tests/channel_unit_tests.rs
```

**Step 6: Verify**

Run: `cargo nextest run --test integration -E 'test(channel)' --test unit -E 'test(allowlist|channel)' 2>&1`
Expected: ~38-40 channel tests pass (a few removed as duplicates/structural).

**Step 7: Commit**

```
refactor(tests): split channel tests into integration/channels.rs + unit/channels.rs
```

---

### Task 5: Migrate Learning Tests

Merge `learning_integration.rs` (22 tests) + `phase4_learning_handler_tests.rs` (24 tests) + `phase4_e2e_feedback_loop_test.rs` (10 tests) + `learning_loop_test.rs` (4 tests) → `tests/integration/learning.rs`.

**Files:**
- Source: `tests/learning_integration.rs`
- Source: `tests/phase4_learning_handler_tests.rs`
- Source: `tests/phase4_e2e_feedback_loop_test.rs`
- Source: `tests/learning_loop_test.rs`
- Target: `tests/integration/learning.rs`
- Delete: all 4 source files

**Step 1: Use `learning_integration.rs` as the base**

This file has the most comprehensive and well-structured tests. Copy its contents into `integration/learning.rs`.

**Step 2: Merge unique tests from `phase4_learning_handler_tests.rs`**

This file has its own `MockLearningHandler`. Since `tests/common/mocks/` doesn't have a learning handler mock, either:
- Move the mock to `tests/common/mocks/learning.rs` and reference from common
- Or inline it at the top of `integration/learning.rs`

Keep only tests that add unique coverage not in `learning_integration.rs`:
- LearningTool routing tests (action dispatch to handler) — **keep** (unique)
- Threshold history with limit parameter — check if `learning_integration.rs` covers this; if not, **keep**
- `test_learning_handler_trait_get_status_none` — structural (tests that mock returns None). **Remove.**
- `test_learning_handler_trait_get_status_some` — structural. **Remove.**

**Step 3: Merge unique tests from `phase4_e2e_feedback_loop_test.rs`**

Check for overlap with `learning_integration.rs` tests on:
- AdaptiveThresholds step limits
- EnrichmentStats aggregation
- LearningService analyze_now()

Keep only non-overlapping E2E tests. If `learning_integration.rs` already tests the same path, remove the duplicate.

**Step 4: Merge unique tests from `learning_loop_test.rs`**

- `test_strategy_record_roundtrip_with_satisfaction` — tests strategy repo persistence. **Keep** (unique repo round-trip).
- `test_goal_plan_completion_metrics_end_to_end` — tests goal metrics. **Keep** (unique).
- `test_learning_handler_reads_strategy_records` — may overlap with `learning_integration.rs`. Check and deduplicate.
- `test_satisfaction_no_match_returns_false` — tests edge case. **Keep.**

**Step 5: Organize sections**

```rust
//! Learning system integration tests — outcome recording, analysis, thresholds, enrichment.

// ── Outcome Recording ───────────────────────────────────────
// ── Learning Analysis ───────────────────────────────────────
// ── Adaptive Thresholds ─────────────────────────────────────
// ── Enrichment Feedback ─────────────────────────────────────
// ── Strategy Persistence ────────────────────────────────────
// ── LearningTool Routing ────────────────────────────────────
// ── Edge Cases ──────────────────────────────────────────────
```

**Step 6: Rename all test functions per convention**

Drop `test_`, `ac_`, phase/sprint prefixes.

**Step 7: Delete old files**

```bash
rm tests/learning_integration.rs tests/phase4_learning_handler_tests.rs \
   tests/phase4_e2e_feedback_loop_test.rs tests/learning_loop_test.rs
```

**Step 8: Verify**

Run: `cargo nextest run --test integration -E 'test(learn)' 2>&1 | tail -10`
Expected: ~35-40 learning tests (down from 60, removing ~20 duplicates/structural).

**Step 9: Commit**

```
refactor(tests): consolidate learning tests into integration/learning.rs
```

---

### Task 6: Migrate Simple Integration Tests (finance, memory, sessions, skills)

Move straightforward files that don't require merging.

**Files:**
- `tests/finance_integration_tests.rs` → `tests/integration/finance.rs`
- `tests/memory_tool_integration.rs` + `tests/memory_and_context_tests.rs` → `tests/integration/memory.rs`
- `tests/conversation_embedding_integration.rs` → `tests/integration/sessions.rs`
- `tests/skills_tests.rs` + integration_tests.rs `test_skills_availability` → `tests/integration/skills.rs`

**Step 1: Move finance**

Copy `finance_integration_tests.rs` → `integration/finance.rs`. Fix imports (`use super::common` etc.). Rename test functions. Delete old file.

**Step 2: Merge memory**

Combine `memory_tool_integration.rs` (19 tests) + `memory_and_context_tests.rs` (7 tests) → `integration/memory.rs`. Section headers:

```rust
// ── Memory Tool Actions ─────────────────────────────────────
// ── Context Engine Sources ──────────────────────────────────
// ── Edge Cases ──────────────────────────────────────────────
```

Rename tests. Delete both old files.

**Step 3: Move sessions**

Copy `conversation_embedding_integration.rs` → `integration/sessions.rs`. Rename tests (drop `test_tc*_` prefixes). Delete old file.

**Step 4: Move and merge skills**

Copy `skills_tests.rs` → `integration/skills.rs`. Also move `test_skills_availability` and `test_tool_registry_integration` and `test_tool_parameter_validation` from `integration_tests.rs` into this file. Rename tests. Delete `skills_tests.rs`.

**Step 5: Verify**

Run: `cargo nextest run --test integration 2>&1 | tail -10`
Expected: All integration module tests pass.

**Step 6: Commit**

```
refactor(tests): migrate finance, memory, sessions, skills to integration/
```

---

### Task 7: Migrate E2E Tests

**Files:**
- `tests/agent_loop_tests.rs` → `tests/e2e/agent_loop.rs`
- `tests/orchestrator_e2e.rs` → `tests/e2e/agent_pipeline.rs`
- `tests/ask_user_tests.rs` → `tests/e2e/ask_user.rs`
- `tests/reminder_and_tracking_edge_cases.rs` → `tests/e2e/reminders.rs`

**Step 1: Move each file**

For each: copy content → fix imports → rename tests → delete old file.

**Step 2: Rename test functions**

Apply `<subject>_<verb>_<condition>` convention, drop `test_` prefix.

**Step 3: Verify**

Run: `cargo nextest run --test e2e 2>&1 | tail -10`
Expected: All E2E tests pass (~47 tests).

**Step 4: Commit**

```
refactor(tests): migrate e2e tests (agent_loop, pipeline, ask_user, reminders)
```

---

### Task 8: Migrate Unit Tests & Clean Up integration_tests.rs

**Files:**
- `tests/provider_tests.rs` → `tests/unit/providers.rs`
- `tests/integration_tests.rs` → distribute remaining tests, then delete
- Target: `tests/unit/config.rs`

**Step 1: Move provider tests**

Copy `provider_tests.rs` → `unit/providers.rs`. Also move `test_provider_extra_headers` from `integration_tests.rs`. Rename. Delete old file.

**Step 2: Create `tests/unit/config.rs`**

Move these from `integration_tests.rs`:
- `test_backward_compat_minimal_config` → `backward_compat_minimal_config_deserializes`
- `test_config_integration` → `config_default_round_trips`
- `test_email_consent_granted_enforcement` → `email_consent_defaults_to_false`
- `test_session_history_limit` → `session_history_returns_last_n`

**Step 3: Remove low-value tests from integration_tests.rs**

These are trivial and add no value:
- `test_full_message_flow` — duplicate of channel test
- `test_session_persistence_flow` — tests Session::add_message (trivial)
- `test_multiple_sessions_parallel` — tests creating 3 sessions (trivial)
- `test_session_cleanup` — tests Session::clear (trivial)

**Step 4: Delete `tests/integration_tests.rs`**

All tests have been distributed or removed.

**Step 5: Verify**

Run: `cargo nextest run --test unit 2>&1 | tail -10`
Expected: All unit tests pass.

**Step 6: Commit**

```
refactor(tests): migrate unit tests, distribute integration_tests.rs, delete
```

---

### Task 9: Handle Plugin Tests

**Files:**
- Rename: `tests/plugin_integration_tests.rs` → `tests/plugins.rs`

**Step 1: Rename file**

The plugin tests are feature-gated (`#[cfg(feature = "plugin-integration")]`) and must remain a separate binary. Simply rename to `tests/plugins.rs` (dropping the `_integration_tests` suffix).

**Step 2: Rename test functions inside**

Drop `test_` prefix, apply naming convention.

**Step 3: Verify**

Run: `cargo nextest run --test plugins 2>&1`
Expected: Compiles (tests skipped without feature flag, which is correct).

**Step 4: Commit**

```
refactor(tests): rename plugin_integration_tests.rs to plugins.rs
```

---

### Task 10: Collapse Finance Types Tests

Reduce `crates/feature-finance/tests/types_test.rs` from ~60 individual tests to ~10 parametric tests.

**Files:**
- Modify: `crates/feature-finance/tests/types_test.rs`

**Step 1: Replace per-variant tests with parametric loops**

For each enum module (e.g., `account_type`), replace individual tests like:

```rust
#[test]
fn test_as_str_cash() { assert_eq!(AccountType::Cash.as_str(), "cash"); }
#[test]
fn test_as_str_bank() { assert_eq!(AccountType::Bank.as_str(), "bank"); }
// ... 5 more
```

With a single test:

```rust
#[test]
fn as_str_returns_snake_case_for_all_variants() {
    let cases = [
        (AccountType::Cash, "cash"),
        (AccountType::Bank, "bank"),
        (AccountType::Ewallet, "ewallet"),
        (AccountType::CryptoWallet, "crypto_wallet"),
        (AccountType::Brokerage, "brokerage"),
    ];
    for (variant, expected) in cases {
        assert_eq!(variant.as_str(), expected, "as_str failed for {:?}", variant);
    }
}

#[test]
fn from_str_loose_resolves_aliases_and_case() {
    let cases = [
        ("cash", Some(AccountType::Cash)),
        ("CASH", Some(AccountType::Cash)),
        ("e_wallet", Some(AccountType::Ewallet)),
        ("cryptowallet", Some(AccountType::CryptoWallet)),
        ("invalid", None),
    ];
    for (input, expected) in cases {
        assert_eq!(AccountType::from_str_loose(input), expected, "from_str_loose({input})");
    }
}
```

Apply this pattern to all 10 enum modules. Result: 2 tests per enum × 10 enums = 20 tests (down from ~60).

**Step 2: Rename file**

Rename to `crates/feature-finance/tests/domain_enum_tests.rs` (clearer purpose).

**Step 3: Verify**

Run: `cargo nextest run -p feature-finance --test domain_enum_tests 2>&1 | tail -5`
Expected: ~20 tests pass.

**Step 4: Commit**

```
refactor(tests): collapse finance enum tests into parametric loops
```

---

### Task 11: Collapse Serialization Tests

Reduce `crates/storage/src/rows/serialization_tests.rs` from ~50 individual tests to ~5 macro-driven tests.

**Files:**
- Modify: `crates/storage/src/rows/serialization_tests.rs`

**Step 1: Create a macro for camelCase + round-trip checks**

Replace individual `*_camel_case()` and `*_round_trip()` tests with:

```rust
macro_rules! camel_case_test {
    ($name:ident, $row:expr) => {
        #[test]
        fn $name() {
            let v = serde_json::to_value(&$row).unwrap();
            assert_no_snake_case_keys(&v, stringify!($name));
            // Round-trip
            let json = serde_json::to_string(&$row).unwrap();
            let v2: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_no_snake_case_keys(&v2, concat!(stringify!($name), " round-trip"));
        }
    };
}

camel_case_test!(todo_row, sample_todo_row());
camel_case_test!(project_row, sample_project_row());
camel_case_test!(plan_row, sample_plan_row());
// ... one line per row type
```

This keeps the same coverage (every row type is checked) but collapses pairs of tests into single macro invocations.

**Step 2: Remove redundant spot-checks**

The individual `assert!(v.get("dueDate").is_some())` lines in each test are redundant — `assert_no_snake_case_keys` already verifies all keys are camelCase. Remove spot-checks that just verify camelCase key names exist.

Keep spot-checks only for fields with special serialization (e.g., `Secret<String>`, custom serializers).

**Step 3: Verify**

Run: `cargo nextest run -p storage 2>&1 | tail -10`
Expected: All storage tests pass, fewer serialization tests but same coverage.

**Step 4: Commit**

```
refactor(tests): collapse serialization tests into macro-driven checks
```

---

### Task 12: Remove Remaining Low-Value Inline Tests

Scan crate-level `#[cfg(test)]` modules for structural tests that only verify derives work.

**Files:**
- Scan: `crates/*/src/**/*.rs` for `test_default_*`, `test_new_*`, `test_display_*`, `test_clone_*`

**Step 1: Identify candidates**

Search for test functions matching these patterns and evaluate each:
- `test_default_*` — if it only checks `Type::default()` doesn't panic → **remove**
- `test_new_*` — if it only checks `Type::new(args)` returns a value → **remove**
- `test_display_*` / `test_debug_*` — if it only checks Display/Debug impls from derives → **remove**

Keep tests that verify non-trivial default values or constructor logic.

**Step 2: Remove identified tests**

Delete individual test functions from their `#[cfg(test)]` modules. If a `mod tests` block becomes empty, remove the entire block.

**Step 3: Verify**

Run: `cargo nextest run --workspace 2>&1 | tail -5`
Expected: All remaining tests pass.

**Step 4: Commit**

```
refactor(tests): remove trivial derive-check tests across crates
```

---

### Task 13: Final Verification & Cleanup

**Step 1: Verify no old test files remain at top level**

```bash
ls tests/*.rs
```

Expected: only `tests/plugins.rs` at the top level. All others should be in subdirectories.

**Step 2: Run full test suite**

Run: `cargo nextest run --workspace 2>&1 | tail -10`
Expected: ~1,650-1,750 tests, all passing.

**Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1`
Expected: 0 warnings.

**Step 4: Run fmt check**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

**Step 5: Record final test count and compare to baseline**

Print a summary: baseline count → final count, files removed, files merged.

**Step 6: Commit**

```
refactor(tests): complete test suite cleanup — N tests removed, 3 binaries
```
