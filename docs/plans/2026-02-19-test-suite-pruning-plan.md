# Test Suite Pruning Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce ~1,885 tests to ~1,600-1,650 by deleting trivial/dead tests, consolidating near-duplicates into parameterized tests, and relocating misplaced unit tests from integration suite into proper crates.

**Architecture:** Bottom-up pruning through dependency layers. Leaf crates first so relocated tests have landing spots. Each task is self-contained with a verification step. No new dependencies needed.

**Tech Stack:** Rust, cargo-nextest, clippy. Consolidation uses inline data-driven test patterns (no rstest dependency).

**Worktree:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/.worktrees/test-pruning` on branch `chore/test-suite-pruning`

**Baseline:** 1,885 passing tests, 53 skipped.

---

## Phase 1: Leaf Crates (common, config)

### Task 1: Consolidate common/prompts.rs serde round-trip tests

**Files:**
- Modify: `crates/common/src/prompts.rs:97-276` (test module)

**Step 1: Read the current tests**

8 separate serde round-trip tests (lines 97-276):
- `test_answer_type_serde_single_select` (98-119)
- `test_answer_type_serde_yes_no` (121-135)
- `test_answer_value_serde_selected` (137-152)
- `test_answer_value_serde_multi_selected` (154-171)
- `test_form_response_serde_completed` (173-192)
- `test_form_response_serde_cancelled` (194-202)
- `test_interaction_request_serde_roundtrip` (204-235)
- `test_question_max_4_validation_in_docs` (237-275)

**Step 2: Replace 8 tests with 3 consolidated tests**

Keep `test_interaction_request_serde_roundtrip` (it's the most comprehensive and exercises nested types). Keep `test_question_max_4_validation_in_docs` (documents a business rule). Delete the other 6 — they only test that serde derive works for simple enum variants, which is already exercised by the comprehensive roundtrip test.

Replace the test module with:
- `test_answer_types_serde_roundtrip` — single test covering SingleSelect and YesNo in one function
- `test_answer_values_serde_roundtrip` — single test covering Selected, MultiSelected, Completed, Cancelled
- `test_interaction_request_serde_roundtrip` — keep as-is
- `test_question_max_4_validation_in_docs` — keep as-is

Net: 8 → 4 tests.

**Step 3: Run tests**

Run: `cargo nextest run -p common -E 'test(prompts)'`
Expected: 4 passing tests.

**Step 4: Run clippy**

Run: `cargo clippy -p common --all-targets`
Expected: 0 warnings.

**Step 5: Commit**

```bash
git add crates/common/src/prompts.rs
git commit -m "test(common): consolidate prompts serde tests (8 → 4)"
```

---

### Task 2: Remove redundant SessionKey tests in common/types.rs

**Files:**
- Modify: `crates/common/src/types.rs:152-187` (test module)

**Step 1: Read the current tests**

3 tests:
- `test_session_key` (157-169) — construction + splitting
- `test_session_key_from_parts` (173-175) — construction from parts (trivial)
- `test_session_key_equality` (179-185) — equality comparison (already tested implicitly by `test_session_key`)

**Step 2: Keep only `test_session_key`, delete the other 2**

`test_session_key` already tests construction, formatting, and splitting. `test_session_key_from_parts` tests a 1-line constructor. `test_session_key_equality` tests `==` on two equal constructions — a Rust derive test.

Net: 3 → 1 test.

**Step 3: Run tests**

Run: `cargo nextest run -p common -E 'test(session_key)'`
Expected: 1 passing test.

**Step 4: Commit**

```bash
git add crates/common/src/types.rs
git commit -m "test(common): remove redundant SessionKey tests (3 → 1)"
```

---

### Task 3: Consolidate config serde round-trip tests

**Files:**
- Modify: `crates/config/src/schema/core.rs:1081-1345` (test module)

**Step 1: Read the current tests**

11 tests. 4 are trivial serde round-trips:
- `test_calendar_config_serialization_new_format` (1157-1173)
- `test_calendar_config_roundtrip` (1176-1206)
- `test_daily_planning_config_serde_roundtrip` (1245-1262)
- `test_config_calendar_serialization` (1222-1238)

7 are meaningful (keep all):
- `test_calendar_config_secret_redaction` (1090-1107)
- `test_calendar_provider_config_helpers` (1109-1132)
- `test_calendar_config_multi_provider` (1134-1154)
- `test_secret_is_empty` (1209-1215)
- `test_conversation_config_defaults` (1269-1281)
- `test_conversation_config_deserialize` (1284-1317)
- `test_exclude_channels_config` (1320-1344)

**Step 2: Merge the 4 serde tests into 1**

Create `test_config_serde_roundtrips` that exercises CalendarConfig roundtrip and DailyPlanningConfig roundtrip in a single function.

Net: 11 → 8 tests.

**Step 3: Run tests**

Run: `cargo nextest run -p config`
Expected: All passing.

**Step 4: Commit**

```bash
git add crates/config/src/schema/core.rs
git commit -m "test(config): consolidate serde round-trip tests (11 → 8)"
```

---

## Phase 2: Tools Crate

### Task 4: Consolidate ParamExtractor tests (MAJOR — 47 → ~8)

**Files:**
- Modify: `crates/tools/src/params.rs:206-553` (test module)

**Step 1: Analyze the 47 tests**

They fall into these identical-structure groups:

| Group | Pattern | Test Count |
|-------|---------|------------|
| required_str | present/missing/wrong_type/null | 4 |
| required_i64 | present/missing/wrong_type | 3 |
| required_u64 | present/missing/negative | 3 |
| required_bool | present/wrong_type | 2 |
| required_array | present/wrong_type | 2 |
| required_object | present/wrong_type | 2 |
| optional_str | present/absent/wrong_type/null | 4 |
| str_or | present/absent/wrong_type | 3 |
| optional_i64 | present/absent/wrong_type | 3 |
| i64_or | present/absent/wrong_type | 3 |
| optional_u64 | present/absent/wrong_type | 3 |
| optional_bool | present/absent/wrong_type | 3 |
| optional_array | present/absent/wrong_type | 3 |
| string_array_or_empty | present/absent/filters/wrong_type | 4 |

**Step 2: Consolidate into 8 focused tests**

Replace with:
1. `test_required_extractors_present` — one test that extracts str, i64, u64, bool, array, object from valid args
2. `test_required_extractors_missing` — one test that verifies MissingRequired errors for each type
3. `test_required_extractors_wrong_type` — one test that verifies TypeMismatch errors for each type
4. `test_required_edge_cases` — null-as-missing for str, negative for u64
5. `test_optional_extractors_present_and_absent` — extracts present values and verifies None for absent keys
6. `test_optional_extractors_wrong_type` — verifies TypeMismatch errors for optional types
7. `test_or_default_extractors` — tests str_or and i64_or present/absent/wrong_type
8. `test_string_array_or_empty` — present/absent/filters/wrong_type

Each test uses data-driven assertions with `"failed for: {context}"` messages.

Net: 47 → 8 tests.

**Step 3: Run tests**

Run: `cargo nextest run -p tools -E 'test(params)' --nocapture`
Expected: 8 passing tests.

**Step 4: Run clippy**

Run: `cargo clippy -p tools --all-targets`
Expected: 0 warnings.

**Step 5: Commit**

```bash
git add crates/tools/src/params.rs
git commit -m "test(tools): consolidate ParamExtractor tests (47 → 8)"
```

---

## Phase 3: Agent Crate

### Task 5: Consolidate enrichment priority tests (15 → 3)

**Files:**
- Modify: `crates/agent/src/enrichment/priority.rs:126-327` (test module)

**Step 1: Analyze the 15 tests**

Group by pattern:
- Simple keyword → priority: `test_urgent_keyword`, `test_bug_keyword`, `test_feature_keyword`, `test_low_priority_keyword`, `test_no_keywords_defaults_medium`, `test_multiword_keyword_nice_to_have`, `test_multiword_keyword_low_priority`, `test_case_insensitive_matching`, `test_whitespace_only_title`, `test_empty_title_defaults_medium` (10 tests)
- Source-aware (description/tags): `test_description_keywords_counted`, `test_tag_keywords_counted`, `test_tags_only_matching_no_title_keywords` (3 tests)
- Conflict resolution: `test_conflicting_keywords_picks_highest_confidence`, `test_multiple_conflicting_keywords_picks_best` (2 tests)

**Step 2: Replace with 3 tests**

1. `test_priority_inference_from_keywords` — data-driven test with cases array: `[("URGENT: fix", 1), ("broken login", 2), ("feature: dark mode", 3), ("typo in docs", 4), ("regular task", 3), ("nice to have: x", 4), ("low priority: y", 4), ("", 3), ("   ", 3)]`
2. `test_priority_inference_from_multiple_sources` — tests description keywords, tag keywords, and tags-only matching
3. `test_priority_conflict_resolution` — tests that highest-confidence keyword wins when multiple keywords point to different priorities

Net: 15 → 3 tests.

**Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(priority)'`
Expected: 3 passing tests.

**Step 4: Commit**

```bash
git add crates/agent/src/enrichment/priority.rs
git commit -m "test(agent): consolidate priority inference tests (15 → 3)"
```

---

### Task 6: Consolidate enrichment duration tests (5 → 1)

**Files:**
- Modify: `crates/agent/src/enrichment/duration.rs:81-131` (test module)

**Step 1: Replace 5 tests with 1 data-driven test**

```rust
#[test]
fn test_duration_prediction_from_keywords() {
    let cases = [
        ("fix typo in readme", 15),    // quick
        ("fix login bug", 30),          // small
        ("implement dark mode", 60),    // medium
        ("refactor auth module", 120),  // large
        ("regular task", 45),           // default (no keywords)
    ];
    for (title, expected_minutes) in cases {
        let task = TodoItem::new(title);
        let result = predict_duration(&task);
        assert_eq!(result.value, expected_minutes, "failed for: {title}");
    }
}
```

Net: 5 → 1 test.

**Step 2: Run tests**

Run: `cargo nextest run -p agent -E 'test(duration)'`
Expected: 1 passing test.

**Step 3: Commit**

```bash
git add crates/agent/src/enrichment/duration.rs
git commit -m "test(agent): consolidate duration prediction tests (5 → 1)"
```

---

### Task 7: Consolidate confidence evaluator tests (12 → 8)

**Files:**
- Modify: `crates/agent/src/confidence/evaluator.rs:140-276` (test module)

**Step 1: Identify duplicates**

Delete:
- `test_threshold_getter` (210-214) — duplicated by `test_threshold_getter_clamped` and `test_atomic_threshold_is_readable`
- `test_threshold_getter_clamped` (216-220) — duplicated by `test_threshold_clamped_to_valid_range`
- `test_strip_no_blocks` (191-195) — trivial no-op case
- `test_atomic_threshold_is_readable` (224-228) — tests same getter as other threshold tests

Keep the 8 that test distinct behavior:
- `test_extract_confidence_block`, `test_extract_no_block`
- `test_parse_assessment_high_confidence`, `test_parse_assessment_low_confidence`
- `test_strip_confidence_blocks`
- `test_scores_clamped`
- `test_threshold_handle_allows_external_update`
- `test_threshold_clamped_to_valid_range` (subsumes both getter tests)
- `test_decide_threshold_boundary`

Net: 12 → 8 tests (delete 4).

**Step 2: Run tests**

Run: `cargo nextest run -p agent -E 'test(evaluator)'`
Expected: 8 passing tests.

**Step 3: Commit**

```bash
git add crates/agent/src/confidence/evaluator.rs
git commit -m "test(agent): remove duplicate threshold getter tests (12 → 8)"
```

---

### Task 8: Consolidate learning types serde tests (5 → 2)

**Files:**
- Modify: `crates/agent/src/learning/types.rs:113-220` (test module)

**Step 1: Analyze 5 tests**

- `test_outcome_record_serde_round_trip` (117-142) — OutcomeRecord roundtrip
- `test_execution_mode_plan_step_serde` (144-162) — ExecutionMode variant
- `test_enrichment_feedback_serde` (164-179) — EnrichmentFeedbackEntry
- `test_adaptive_threshold_state_serde` (182-198) — AdaptiveThresholdState
- `test_outcome_record_has_no_tool_args_field` (200-219) — **KEEP** — privacy-by-omission business rule

**Step 2: Merge first 4 into 1 comprehensive roundtrip test, keep the privacy test**

Net: 5 → 2 tests.

**Step 3: Run tests and commit**

Run: `cargo nextest run -p agent -E 'test(learning::types)'`

```bash
git add crates/agent/src/learning/types.rs
git commit -m "test(agent): consolidate learning types serde tests (5 → 2)"
```

---

## Phase 4: Integration Tests (MAJOR)

### Task 9: Delete cli_cleanup_test.rs

**Files:**
- Delete: `tests/cli_cleanup_test.rs`

**Step 1: Delete the entire file**

All 22 tests are boilerplate: "assert removed CLI subcommand returns error". Clap already ensures unrecognized subcommands fail. These add zero coverage of actual business logic.

**Step 2: Run tests**

Run: `cargo nextest run --workspace`
Expected: ~1,863 passing (22 fewer).

**Step 3: Commit**

```bash
git rm tests/cli_cleanup_test.rs
git commit -m "test: delete cli_cleanup_test.rs (22 boilerplate removed-command checks)"
```

---

### Task 10: Delete learning_unit_tests.rs

**Files:**
- Delete: `tests/learning_unit_tests.rs`

**Step 1: Delete the file**

This is a skeleton/template file. 877 lines of commented-out test templates intended to be copied into source files. Only 3 uncommented test stubs exist (lines 853-877) and they test evaluator behavior already covered by evaluator.rs unit tests.

**Step 2: Run tests**

Run: `cargo nextest run --workspace`
Expected: Test count unchanged or -3.

**Step 3: Commit**

```bash
git rm tests/learning_unit_tests.rs
git commit -m "test: delete learning_unit_tests.rs (skeleton template, not runnable tests)"
```

---

### Task 11: Delete todo_chat_enrichment.rs

**Files:**
- Delete: `tests/todo_chat_enrichment.rs`

**Step 1: Delete the file**

2 tests, both `#[ignore]`d, testing a deprecated confidence breakdown feature that no longer exists.

**Step 2: Run tests and commit**

```bash
git rm tests/todo_chat_enrichment.rs
git commit -m "test: delete todo_chat_enrichment.rs (deprecated feature tests, both #[ignore]d)"
```

---

### Task 12: Delete mock infrastructure self-tests

**Files:**
- Modify: `tests/mock_calendar_provider.rs` (remove lines 290-392, test module)
- Modify: `tests/mock_embedding_handler.rs` (remove lines 180-233, test module)
- Modify: `tests/mock_conversation_embedding_handler.rs` (remove lines 294-356, test module)

**Step 1: Remove self-test modules from mock files**

These 20 tests verify the mock infrastructure itself (not production code). The mocks are already validated by the integration tests that use them.

Net: -20 tests.

**Step 2: Run tests**

Run: `cargo nextest run --workspace`
Expected: All passing, -20 tests.

**Step 3: Commit**

```bash
git add tests/mock_calendar_provider.rs tests/mock_embedding_handler.rs tests/mock_conversation_embedding_handler.rs
git commit -m "test: remove self-tests from mock infrastructure files (-20 tests)"
```

---

### Task 13: Prune integration_tests.rs config duplicates

**Files:**
- Modify: `tests/integration_tests.rs`

**Step 1: Identify config parsing tests to remove**

Remove tests that duplicate config crate unit tests:
- `test_email_config_defaults` (~357-384)
- `test_web_search_config` (~386-400)
- `test_feishu_config_defaults` (~522-538)
- `test_dingtalk_config_defaults` (~540-556)
- `test_mochat_config_defaults` (~558-579)
- `test_new_provider_configs` (~581-608)
- `test_gateway_config_defaults` (~610-618)
- `test_discord_config_usage` (~656-683)

Keep substantive integration tests:
- `test_full_message_flow`, `test_session_persistence_flow`, `test_tool_registry_integration`, `test_multiple_sessions_parallel`, `test_bus_message_ordering`, `test_session_history_limit`, `test_session_cleanup`, `test_session_lru_eviction`, etc.
- Keep `test_config_env_override` (tests env var override, not just parsing)
- Keep `test_backward_compat_minimal_config` (backward compatibility is important)
- Keep `test_provider_extra_headers` (tests serialization of non-trivial nested config)
- Keep `test_email_consent_granted_enforcement` (tests business rule enforcement)

Net: -8 tests.

**Step 2: Run tests**

Run: `cargo nextest run --test integration_tests`
Expected: All remaining tests pass.

**Step 3: Commit**

```bash
git add tests/integration_tests.rs
git commit -m "test: remove config-parsing duplicates from integration_tests.rs (-8 tests)"
```

---

### Task 14: Prune channel_unit_tests.rs trivial config checks

**Files:**
- Modify: `tests/channel_unit_tests.rs`

**Step 1: Identify trivial config default tests to remove**

Remove simple `ChannelConfig::default()` + field assertion tests:
- `test_discord_config_defaults`, `test_discord_channel_name`, `test_discord_is_allowed_empty_allowlist`
- `test_slack_config_defaults`, `test_slack_channel_name`, `test_slack_is_allowed_empty_allowlist`
- `test_qq_config_defaults`, `test_qq_channel_name`, `test_qq_is_allowed_empty_allowlist`
- `test_whatsapp_config_defaults`, `test_whatsapp_channel_name`, `test_whatsapp_is_allowed_empty_allowlist`
- `test_email_channel_name`, `test_email_is_allowed_empty_allowlist`
- `test_discord_channel_creation_empty_token`, `test_slack_channel_creation`, `test_qq_channel_creation`, `test_whatsapp_channel_creation`, `test_email_channel_creation`

Keep substantive tests:
- All allowlist tests with actual matching logic (compound ID, no-partial-match)
- All async tests (start_rejects_empty_*, send_fails_*, consent validation)
- All message routing/identity tests
- Channel manager tests
- Fixture validation tests

Net: ~-19 tests.

**Step 2: Run tests**

Run: `cargo nextest run --test channel_unit_tests`
Expected: All remaining tests pass.

**Step 3: Commit**

```bash
git add tests/channel_unit_tests.rs
git commit -m "test: remove trivial channel config defaults from integration suite (-19 tests)"
```

---

### Task 15: Delete all #[ignore] placeholder tests across the suite

**Files:**
- Multiple `tests/*.rs` files

**Step 1: Find all #[ignore] tests with todo!() bodies**

Search for `#[ignore` across `tests/` directory and remove functions that contain `todo!()` or are placeholders for unimplemented features.

**Step 2: Remove each placeholder**

Be careful to only remove tests that are genuine placeholders (contain `todo!()`, `unimplemented!()`, or empty bodies), not tests that are `#[ignore]`d for legitimate reasons (e.g., requires PostgreSQL).

**Step 3: Run tests and commit**

```bash
git add tests/
git commit -m "test: delete #[ignore] placeholder tests with todo!() bodies"
```

---

## Phase 5: Final Verification

### Task 16: Full verification pass

**Step 1: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: All passing. Count should be ~1,630-1,660.

**Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings.

**Step 3: Run fmt check**

Run: `cargo fmt --all --check`
Expected: Clean.

**Step 4: Run doctests**

Run: `cargo test --workspace --doc`
Expected: All passing.

**Step 5: Compare counts**

Run: `cargo nextest list --workspace 2>/dev/null | grep -c '::'`
Report the before (1,885) vs after count and percentage reduction.

**Step 6: Final commit (if any fmt fixes needed)**

---

## Summary

| Task | Target | Reduction |
|------|--------|-----------|
| 1 | common/prompts.rs serde | 8 → 4 (-4) |
| 2 | common/types.rs SessionKey | 3 → 1 (-2) |
| 3 | config/core.rs serde | 11 → 8 (-3) |
| 4 | tools/params.rs ParamExtractor | 47 → 8 (-39) |
| 5 | agent/enrichment/priority.rs | 15 → 3 (-12) |
| 6 | agent/enrichment/duration.rs | 5 → 1 (-4) |
| 7 | agent/confidence/evaluator.rs | 12 → 8 (-4) |
| 8 | agent/learning/types.rs | 5 → 2 (-3) |
| 9 | tests/cli_cleanup_test.rs | DELETE (-22) |
| 10 | tests/learning_unit_tests.rs | DELETE (-3) |
| 11 | tests/todo_chat_enrichment.rs | DELETE (-2) |
| 12 | Mock infrastructure self-tests | DELETE (-20) |
| 13 | tests/integration_tests.rs config | PRUNE (-8) |
| 14 | tests/channel_unit_tests.rs trivial | PRUNE (-19) |
| 15 | #[ignore] placeholders | DELETE (~-10) |
| **Total** | | **~-155** |

**Estimated final count: ~1,730 tests** (8% reduction) with identical coverage of business logic.
