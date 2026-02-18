# Test Specification: CLI Cleanup + JSONL→SQL Migration

## Overview

This document specifies all tests for the PostgreSQL storage migration and CLI cleanup.
Cross-references acceptance criteria from the architecture plan at `.claude/plans/refactored-fluttering-stearns.md`.

**Total tests: 147** (72 unit, 48 integration, 27 e2e/smoke)

---

## Phase A: CLI Cleanup (27 tests)

### A.1 — Deleted Commands Return Clap Error (12 tests, e2e)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 1 | `cli_todo_subcommand_removed` | e2e | `klyntbot todo list` → clap error, not panic |
| 2 | `cli_project_subcommand_removed` | e2e | `klyntbot project list` → clap error |
| 3 | `cli_goal_subcommand_removed` | e2e | `klyntbot goal list` → clap error |
| 4 | `cli_plan_subcommand_removed` | e2e | `klyntbot plan list` → clap error |
| 5 | `cli_calendar_subcommand_removed` | e2e | `klyntbot calendar reconcile` → clap error |
| 6 | `cli_channels_subcommand_removed` | e2e | `klyntbot channels list` → clap error |
| 7 | `cli_cron_subcommand_removed` | e2e | `klyntbot cron list` → clap error |
| 8 | `cli_config_subcommand_removed` | e2e | `klyntbot config show` → clap error |
| 9 | `cli_skills_subcommand_removed` | e2e | `klyntbot skills list` → clap error |
| 10 | `cli_usage_subcommand_removed` | e2e | `klyntbot usage report` → clap error |
| 11 | `cli_learning_subcommand_removed` | e2e | `klyntbot learning status` → clap error |
| 12 | `cli_provider_subcommand_removed` | e2e | `klyntbot provider status` → clap error |

### A.2 — Kept Commands Still Work (4 tests, e2e)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 13 | `cli_chat_subcommand_exists` | e2e | `klyntbot chat --help` → success |
| 14 | `cli_serve_subcommand_exists` | e2e | `klyntbot serve --help` → success |
| 15 | `cli_init_subcommand_exists` | e2e | `klyntbot init --help` → success |
| 16 | `cli_status_subcommand_exists` | e2e | `klyntbot status --help` → success |

### A.3 — After-Help Hint (2 tests, e2e)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 17 | `cli_help_shows_chat_hint` | e2e | `klyntbot --help` output contains chat-first hint |
| 18 | `cli_error_shows_after_help` | e2e | Invalid subcommand error includes after_help text |

### A.4 — Commands Enum (4 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 19 | `commands_enum_has_exactly_four_variants` | unit | Commands enum has Chat, Serve, Init, Status only |
| 20 | `commands_parse_chat` | unit | `Commands::try_parse_from(["app", "chat"])` succeeds |
| 21 | `commands_parse_serve` | unit | `Commands::try_parse_from(["app", "serve"])` succeeds |
| 22 | `commands_reject_todo` | unit | `Commands::try_parse_from(["app", "todo"])` fails |

### A.5 — Module Declarations (3 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 23 | `cli_lib_no_todo_module` | unit | `cli::todo` module does not exist (compile-time) |
| 24 | `cli_lib_no_project_module` | unit | `cli::project` module does not exist (compile-time) |
| 25 | `cli_lib_kept_modules_exist` | unit | `cli::chat`, `cli::serve`, `cli::status`, `cli::wizard` exist |

### A.6 — Dead Code (2 tests, integration)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 26 | `no_dead_code_warnings` | integration | `cargo clippy` produces zero warnings |
| 27 | `all_existing_tests_pass` | integration | `cargo nextest run` passes after deletion |

---

## Phase B: Storage Crate Foundation (18 tests)

### B.1 — StoragePool (4 tests, integration)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 28 | `pool_connect_runs_migrations` | integration | Given valid DB URL, When connect(), Then all 16+ tables exist |
| 29 | `pool_connect_invalid_url_returns_error` | integration | Given bad URL, When connect(), Then StorageError returned |
| 30 | `pool_connect_idempotent` | integration | Given already-migrated DB, When connect() again, Then no error |
| 31 | `pool_is_clone_send_sync` | unit | `StoragePool` implements Clone + Send + Sync |

### B.2 — StorageError (4 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 32 | `storage_error_not_found` | unit | `StorageError::NotFound` converts to `KlyntbotError` |
| 33 | `storage_error_conflict` | unit | `StorageError::Conflict` converts to `KlyntbotError` |
| 34 | `storage_error_sqlx` | unit | `StorageError::Sqlx` wraps sqlx errors correctly |
| 35 | `storage_error_display` | unit | All variants have human-readable Display impl |

### B.3 — Config Changes (3 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 36 | `config_database_url_optional` | unit | Config deserializes with `database_url: null` |
| 37 | `config_database_url_present` | unit | Config deserializes with `database_url: "postgresql://..."` |
| 38 | `config_env_override_database_url` | unit | `KLYNTBOT_DATABASE_URL` overrides config file |

### B.4 — Row Structs (4 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 39 | `todo_row_from_row_derives` | unit | `TodoRow` has `#[derive(sqlx::FromRow)]` |
| 40 | `project_row_from_row_derives` | unit | `ProjectRow` has sqlx::FromRow |
| 41 | `session_row_from_row_derives` | unit | `SessionRow` has sqlx::FromRow |
| 42 | `all_row_structs_have_from_impl` | unit | Each `*Row` has `From<*Row> for DomainType` |

### B.5 — SQL Migration (3 tests, integration)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 43 | `migration_creates_all_tables` | integration | All 16+ tables exist after migration |
| 44 | `migration_creates_indexes` | integration | B-tree, GIN, partial, IVFFlat indexes exist |
| 45 | `migration_pgvector_extension` | integration | `vector` extension enabled, embedding columns work |

---

## Phase C: Repository Implementation (68 tests)

### C.1 — TodoRepo (20 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 46 | `todo_repo_add_and_get` | unit | Insert todo, retrieve by ID, fields match |
| 47 | `todo_repo_update` | unit | Update title/status/priority, verify changes persist |
| 48 | `todo_repo_delete` | unit | Delete by ID, subsequent get returns None |
| 49 | `todo_repo_list_with_status_filter` | unit | List only todos matching status filter |
| 50 | `todo_repo_list_with_tag_filter` | unit | GIN index containment query on tags |
| 51 | `todo_repo_list_with_priority_filter` | unit | Filter by priority range |
| 52 | `todo_repo_focus_and_unfocus` | unit | Focus sets focused_at, unfocus clears it |
| 53 | `todo_repo_focus_slot_limit` | unit | Exceeding max focus slots returns false |
| 54 | `todo_repo_cascade_complete` | unit | Completing parent cascades to children |
| 55 | `todo_repo_add_dependency` | unit | Insert edge into todo_dependencies |
| 56 | `todo_repo_remove_dependency` | unit | Remove edge from todo_dependencies |
| 57 | `todo_repo_cycle_detection` | unit | A→B→C→A dependency rejected |
| 58 | `todo_repo_add_attachment` | unit | Insert into todo_attachments join table |
| 59 | `todo_repo_remove_attachment` | unit | Delete from todo_attachments |
| 60 | `todo_repo_add_time_entry` | unit | Insert into todo_time_entries join table |
| 61 | `todo_repo_close_time_entry` | unit | Set ended_at on time entry |
| 62 | `todo_repo_move_todo` | unit | Update parent_id and project_id |
| 63 | `todo_repo_parent_cycle_detection` | unit | Setting parent to descendant rejected |
| 64 | `todo_repo_summary` | unit | Aggregation counts by status |
| 65 | `todo_repo_list_templates` | unit | Filter where is_template = true |

### C.2 — ProjectRepo (6 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 66 | `project_repo_add_and_get` | unit | Insert project, retrieve by ID |
| 67 | `project_repo_update` | unit | Update name/status/color |
| 68 | `project_repo_delete` | unit | Delete project by ID |
| 69 | `project_repo_list_with_status_filter` | unit | Filter by project status |
| 70 | `project_repo_list_with_tag_filter` | unit | GIN containment on tags |
| 71 | `project_repo_all` | unit | List all projects |

### C.3 — SessionRepo (8 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 72 | `session_repo_create_and_get` | unit | Create session, retrieve by key |
| 73 | `session_repo_add_message` | unit | Insert message, ordered by timestamp |
| 74 | `session_repo_get_messages` | unit | Retrieve all messages for session |
| 75 | `session_repo_delete` | unit | Delete session cascades messages |
| 76 | `session_repo_list` | unit | List all session keys with metadata |
| 77 | `session_repo_compact` | unit | Keep last N messages, delete older |
| 78 | `session_repo_message_order` | unit | Messages returned in timestamp order |
| 79 | `session_repo_concurrent_writes` | unit | Multiple tasks write to same session safely |

### C.4 — GoalRepo (6 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 80 | `goal_repo_add_and_get` | unit | Insert goal, retrieve by UUID |
| 81 | `goal_repo_update` | unit | Update title/status/metrics (JSONB) |
| 82 | `goal_repo_delete` | unit | Delete goal by UUID |
| 83 | `goal_repo_list_by_status` | unit | Filter by GoalStatus |
| 84 | `goal_repo_link_project` | unit | Insert into goal_project_links |
| 85 | `goal_repo_metrics_jsonb` | unit | JSONB round-trip for metrics field |

### C.5 — PlanRepo (8 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 86 | `plan_repo_upsert_and_get` | unit | Insert/update plan, retrieve by UUID |
| 87 | `plan_repo_get_active_plan` | unit | Filter by session_key + active statuses |
| 88 | `plan_repo_list_by_status` | unit | Filter by PlanStatus |
| 89 | `plan_repo_add_step` | unit | Insert into plan_steps |
| 90 | `plan_repo_update_step_status` | unit | Update step status/result |
| 91 | `plan_repo_delete` | unit | Delete plan cascades steps |
| 92 | `plan_repo_backtrack_history_jsonb` | unit | JSONB round-trip for backtrack_history |
| 93 | `plan_repo_step_ordering` | unit | Steps returned in step_index order |

### C.6 — EmbeddingRepo (5 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 94 | `embedding_repo_upsert_and_get` | unit | Insert embedding vector, retrieve by todo_id |
| 95 | `embedding_repo_delete` | unit | Delete embedding by todo_id |
| 96 | `embedding_repo_nearest_neighbors` | unit | ORDER BY <=> LIMIT N returns closest vectors |
| 97 | `embedding_repo_dimension_validation` | unit | Reject vectors != 384 dimensions |
| 98 | `embedding_repo_ids_missing` | unit | Return todo IDs without embeddings |

### C.7 — ConvEmbeddingRepo (3 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 99 | `conv_embedding_repo_insert` | unit | Insert conversation embedding |
| 100 | `conv_embedding_repo_search` | unit | ANN search by vector similarity |
| 101 | `conv_embedding_repo_by_session` | unit | Filter by session_key |

### C.8 — OutcomeRepo (4 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 102 | `outcome_repo_record` | unit | Insert outcome record |
| 103 | `outcome_repo_record_feedback` | unit | Insert enrichment feedback |
| 104 | `outcome_repo_outcomes_since` | unit | Filter by created_at >= cutoff |
| 105 | `outcome_repo_get_all` | unit | Retrieve all outcomes |

### C.9 — StrategyRepo (3 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 106 | `strategy_repo_record` | unit | Insert strategy record |
| 107 | `strategy_repo_accuracy` | unit | Calculate accuracy for strategy in time window |
| 108 | `strategy_repo_all_records` | unit | Retrieve all records |

### C.10 — UsageRepo (3 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 109 | `usage_repo_record` | unit | Insert usage record |
| 110 | `usage_repo_report_by_model` | unit | Aggregate tokens/cost grouped by model |
| 111 | `usage_repo_report_by_date` | unit | Aggregate tokens/cost grouped by date |

### C.11 — CronRepo (4 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 112 | `cron_repo_add_job` | unit | Insert cron job |
| 113 | `cron_repo_list_enabled` | unit | Filter where enabled = true |
| 114 | `cron_repo_update_next_run` | unit | Update next_run_at_ms |
| 115 | `cron_repo_delete_job` | unit | Delete cron job by ID |

### C.12 — CalendarSyncRepo (3 tests, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 116 | `calendar_sync_repo_save_state` | unit | Upsert sync state |
| 117 | `calendar_sync_repo_load_state` | unit | Retrieve by provider_id |
| 118 | `calendar_sync_repo_update_token` | unit | Update sync_token field |

### C.13 — Repos Aggregate (1 test, unit)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 119 | `repos_from_pool_creates_all` | unit | `Repos::from_pool()` returns struct with all 12 repos |

---

## Phase D: Consumer Migration (20 tests)

### D.1 — Tool Migration (8 tests, integration)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 120 | `todo_tool_uses_repo` | integration | TodoTool::new takes TodoRepo, execute works |
| 121 | `project_tool_uses_repo` | integration | ProjectTool::new takes ProjectRepo + TodoRepo |
| 122 | `goal_tool_uses_repo` | integration | GoalTool execute routes through GoalRepo |
| 123 | `plan_tool_uses_repo` | integration | PlanTool execute routes through PlanRepo |
| 124 | `todo_tool_add_persists_to_db` | integration | Add via tool, verify row in DB |
| 125 | `project_tool_add_persists_to_db` | integration | Add via tool, verify row in DB |
| 126 | `todo_tool_dependency_via_db` | integration | Add dependency through tool, verify in todo_dependencies |
| 127 | `todo_tool_attachment_via_db` | integration | Add attachment through tool, verify in todo_attachments |

### D.2 — AgentLoop Migration (5 tests, integration)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 128 | `agent_loop_accepts_repos` | integration | AgentLoop::new_with_cron takes Repos struct |
| 129 | `agent_loop_no_arc_rwlock` | integration | Constructor has no Arc<RwLock<>> parameters |
| 130 | `agent_loop_process_message_with_db` | integration | Full message → tool call → DB persistence |
| 131 | `agent_loop_cost_tracking_to_db` | integration | Usage recorded to usage_records table |
| 132 | `agent_loop_learning_to_db` | integration | Outcomes recorded to learning_outcomes table |

### D.3 — Graceful Degradation (4 tests, integration)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 133 | `tool_without_db_returns_clear_error` | integration | Tool with None repos → "Database not configured" |
| 134 | `chat_without_db_llm_works` | integration | LLM conversation works without database_url |
| 135 | `graceful_error_message_mentions_init` | integration | Error text mentions `klyntbot init` |
| 136 | `serve_with_db_connects_pool` | integration | serve.rs constructs StoragePool from config |

### D.4 — Init Wizard Database Step (3 tests, integration)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 137 | `wizard_database_step_is_step_2` | integration | Database step appears after LLM provider |
| 138 | `wizard_database_skip_sets_none` | integration | Skipping DB step sets database_url = None |
| 139 | `wizard_database_connect_test` | integration | Valid URL tests connection and runs migrations |

---

## Phase E: Cleanup & Verification (12 tests)

### E.1 — Old Stores Removed (5 tests, integration)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 140 | `no_jsonl_store_imports` | integration | No `use` of TodoStore/ProjectStore/GoalStore/PlanStore |
| 141 | `no_arc_rwlock_store_pattern` | integration | No `Arc<RwLock<*Store>>` in codebase |
| 142 | `no_jsonl_file_operations_in_tools` | integration | Tools crate has no JSONL file I/O |
| 143 | `embedding_store_replaced` | integration | EmbeddingStore → EmbeddingRepo alias |
| 144 | `session_manager_delegates_to_repo` | integration | SessionManager internally uses SessionRepo |

### E.2 — Full Build Verification (3 tests, e2e)

| # | Test Name | Category | Verifies |
|---|-----------|----------|----------|
| 145 | `workspace_builds_clean` | e2e | `cargo build --workspace` zero errors |
| 146 | `clippy_zero_warnings` | e2e | `cargo clippy --workspace --all-targets` zero warnings |
| 147 | `all_tests_pass` | e2e | `cargo nextest run --workspace` all green |

---

## Test Infrastructure

### testcontainers-rs Pattern

```rust
use testcontainers::{clients::Cli, images::postgres::Postgres};

async fn test_pool() -> (StoragePool, Container<Postgres>) {
    let docker = Cli::default();
    let pg = docker.run(Postgres::default().with_version(16));
    let url = format!("postgresql://postgres@localhost:{}/postgres", pg.get_host_port_ipv4(5432));
    let pool = StoragePool::connect(&url).await.unwrap();
    (pool, pg) // pg must stay alive for duration of test
}
```

### Shared Fixtures

- `test_pool()` — ephemeral Postgres container with migrations
- `test_repos()` — `Repos::from_pool(&pool)` from test container
- `sample_todo()` — pre-built Todo with all fields populated
- `sample_project()` — pre-built Project
- `sample_goal()` — pre-built Goal with metrics
- `sample_plan_with_steps()` — Plan with 3 steps in Draft status
- `sample_embedding_384()` — 384-dim f32 vector for pgvector tests
