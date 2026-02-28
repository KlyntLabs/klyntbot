# Test Suite Cleanup & Restructuring

## Problem

~2,174 tests across 58 files in a flat structure. Build times are slow (22 separate link targets for integration tests). Naming uses sprint/phase prefixes. Significant overlap and structural tests inflate the count without protecting business logic.

## Goals

1. Remove ~400-500 low-value tests (structural, duplicate, no-op)
2. Restructure from flat `tests/*.rs` to domain+type hybrid (`tests/{integration,e2e,unit}/<domain>.rs`)
3. Standardize naming: drop `test_` prefix, sprint/phase/AC codes; use `<subject>_<verb>_<condition>`

## Tests to Remove

### Pure structural (14 tests, 3 files deleted)

- `phase4_queue_tests.rs` (5) — tests struct fields + tokio mpsc
- `phase4_learning_events_tests.rs` (5) — tests broadcast channel pub/sub
- `browser_integration_tests.rs` (4) — feature-gated stub, needs external daemon

### Collapse via parametric tests (~100 tests → ~15)

- `crates/feature-finance/tests/types_test.rs` (60 → 10) — replace per-variant as_str/from_str_loose with loop-based tests
- `crates/storage/src/rows/serialization_tests.rs` (50 → 5) — replace per-row camelCase checks with macro-driven test

### Merge overlapping coverage (~43 tests absorbed)

- `phase4_learning_handler_tests.rs` (24) + `phase4_e2e_feedback_loop_test.rs` (10) + `learning_loop_test.rs` (4) → merge unique tests into `learning_integration.rs`, discard duplicates
- `sprint3_calendar_integration.rs` (5) → merge into `calendar_edge_cases.rs`

### Remove low-value tests (~various)

- "Just verify no panic" tests (e.g., `test_channel_manager_initialization`)
- `test_default_*` inline tests that only check Default derive works

## New Directory Structure

```
tests/
├── common/                     # Keep as-is
│   ├── mod.rs
│   └── mocks/
├── fixtures/                   # Keep as-is
├── integration/                # ONE binary
│   ├── main.rs
│   ├── calendar.rs             # ← calendar_edge_cases + sprint3_calendar_integration
│   ├── channels.rs             # ← channel_tests + channel_unit_tests
│   ├── finance.rs              # ← finance_integration_tests
│   ├── learning.rs             # ← learning_integration + phase4_* + learning_loop_test
│   ├── memory.rs               # ← memory_tool_integration + memory_and_context_tests
│   ├── plugins.rs              # ← plugin_integration_tests
│   ├── sessions.rs             # ← conversation_embedding_integration
│   └── skills.rs               # ← skills_tests
├── e2e/                        # ONE binary
│   ├── main.rs
│   ├── agent_loop.rs           # ← agent_loop_tests
│   ├── agent_pipeline.rs       # ← orchestrator_e2e
│   ├── ask_user.rs             # ← ask_user_tests
│   └── reminders.rs            # ← reminder_and_tracking_edge_cases
└── unit/                       # ONE binary
    ├── main.rs
    ├── channels.rs             # ← channel_unit_tests (allowlist, config validation)
    └── providers.rs            # ← provider_tests
```

3 binaries instead of 22. Compile time improvement from fewer link steps.

## Naming Conventions

**Files:** Domain name only, no sprint/phase/type suffixes. Directory implies test type.

**Functions:** `<subject>_<verb>_<condition>` — no `test_` prefix, no AC/sprint codes.

**Sections:** Group related tests with `// ── Section Name ──` headers.

## Expected Outcome

- ~1,650-1,750 tests remaining (down from ~2,174)
- 3 integration test binaries (down from 22)
- Professional naming throughout
- Zero overlap between test files
