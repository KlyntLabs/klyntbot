# Comprehensive Codebase Refactoring Design

**Date:** 2026-02-20
**Status:** Approved
**Scope:** Performance, dead code cleanup, file splitting, cross-channel UX parity

## Context

Klyntbot is a 106K-line Rust AI agent framework across 16 crates. Codebase quality is 8/10 — solid architecture with zero clippy warnings and clean trait hierarchy. This refactoring targets four areas: performance hotspots, legacy code removal, monolithic file splitting, and cross-channel UX normalization.

## Team Structure

3 agents + tech lead (reviewer/coordinator):

| Agent | Focus | Crate Boundaries |
|-------|-------|-------------------|
| **perf-agent** | Performance optimization | `agent/src/execution/`, `context_engine/` |
| **cleanup-agent** | Dead code removal + file splitting | `tools/`, `goal/`, `plan/`, `config/`, `cli/`, `agent/src/agent_loop.rs` |
| **ux-agent** | Cross-channel UX parity | `channels/`, `common/src/utils/` |

Branch: `refactor/v2-cleanup` — agents commit to non-overlapping files.

## Work Items

### Performance (perf-agent)

#### P1: Replace Vec<Message> cloning in dispatch
- **Problem:** `messages.clone()` at dispatch.rs:77,115,176 copies entire conversation history on every engine escalation. For 100+ message sessions, this is 5-10ms per transition.
- **Solution:** Change `EngineDispatch::execute()` to accept `Arc<Vec<Message>>`. Update `ExecutionCore`, `ReactPlusEngine`, `DirectEngine` signatures accordingly.
- **Files:** `agent/src/execution/dispatch.rs`, `agent/src/execution/core.rs`, `agent/src/execution/react_plus.rs`, `agent/src/execution/direct.rs`
- **Risk:** Signature change cascades through execution pipeline. Must verify all call sites.

#### P2: Cache assembled context
- **Problem:** Full context assembly (history compression, memory retrieval, budget allocation) runs for every message. With semantic search, this can be 500-1000ms.
- **Solution:** Add LRU cache in `ContextEngine` keyed by hash of (message_prefix, tool_definitions_hash). Invalidate on tool execution or config change.
- **Files:** `context_engine/src/assembler.rs`, `context_engine/src/lib.rs`
- **Risk:** Cache invalidation complexity. Must invalidate when tools modify state.

#### P3: Eliminate redundant serialization in dedup
- **Problem:** `serde_json::to_string(&tc.arguments)` runs for every tool call in dedup tracking, even when dedup is disabled.
- **Solution:** Use hash-based comparison (`DefaultHasher` on `Value`) instead of string serialization. Gate behind dedup-enabled check.
- **Files:** `agent/src/execution/core.rs`
- **Risk:** Low. Hash collisions theoretically possible but acceptable for dedup.

#### P4: Make compression constants configurable
- **Problem:** `chunk_size=5`, `min_recent_messages=4`, `memory_limit=5` are hard-coded.
- **Solution:** Add fields to `BudgetConfig` with current values as defaults. Thread through assembler and compressor.
- **Files:** `context_engine/src/history_compressor.rs`, `context_engine/src/assembler.rs`, `context_engine/src/budget.rs`, `config/src/schema/core.rs`
- **Risk:** Low. Pure additive change with backward-compatible defaults.

### Dead Code & File Splitting (cleanup-agent)

#### C1: Remove JSONL stores (~3,700 lines)
- **Problem:** `GoalStore` (753 lines), `PlanStore` (694 lines), `TodoStore` (2,248 lines) implement append-only JSONL persistence that's been superseded by PostgreSQL repos.
- **Solution:** Delete store files, remove all references/imports, update tests to use repos directly.
- **Files to delete:** `tools/src/todo_store.rs`, `goal/src/store.rs`, `plan/src/store.rs`
- **Files to update:** All files importing these stores, relevant test files
- **Risk:** Medium. Must verify no code paths still use file-based stores. Check for feature flags or fallback logic.

#### C2: Split monolithic files
- **todo.rs (1,815 lines):** Extract action handlers into `tools/src/todo/actions/{add,list,search,update,delete,deps}.rs`. Keep `TodoTool` struct and `execute()` dispatch in `tools/src/todo/mod.rs`.
- **core.rs (1,621 lines):** Split config sections into `config/src/schema/{agents,channels,providers,tools,gateway,todo,calendar,finance,project,conversation,learning}.rs`. Keep `Config` struct composition in `config/src/schema/mod.rs`.
- **agent_loop.rs (1,212 lines):** Extract handler/tool registration into `agent/src/agent_loop/builder.rs`. Keep event loop in `agent/src/agent_loop/mod.rs`.
- **prompts.rs (1,523 lines):** Split prompt types into `cli/src/wizard/prompts/{yes_no,select,multi_select,secret,text}.rs`.
- **Risk:** High. Module restructuring can break `pub use` re-exports and external API. Must preserve all public interfaces.

#### C3: Consolidate test mock duplication
- **Problem:** `mock_embedding_handler.rs` and `mock_conversation_embedding_handler.rs` duplicate ~30 lines of embedding generation and cosine similarity.
- **Solution:** Extract shared code to `tests/test_utils/embedding.rs`. Both mocks import from shared module.
- **Risk:** Low. Test-only change.

#### C4: Clean dead code annotations
- Remove `#[allow(dead_code)]` on `timezone` field in `finance_tool/mod.rs` (delete unused field)
- Remove TODO placeholder comments in `plan/src/store.rs` (file being deleted anyway in C1)
- Clean unused fixture imports in storage tests
- **Risk:** Low.

### Cross-Channel UX (ux-agent)

#### U1: Message length splitting
- **Problem:** Agent can generate 10KB+ responses that silently truncate on Telegram (4096), Discord (2000), WhatsApp (~4000).
- **Solution:** Add `MessageSplitter` utility in `channels/src/utils.rs` with per-channel limits. Split on paragraph boundaries, then sentence boundaries, then word boundaries as fallback. All `Channel::send()` implementations call splitter.
- **Files:** New `channels/src/utils.rs`, all channel `send()` methods
- **Risk:** Low. Additive change. Worst case: messages split at slightly wrong points.

#### U2: Markdown normalization
- **Problem:** Same agent response renders differently across channels. WhatsApp/Email show raw markdown.
- **Solution:** Add `ChannelFormatter` trait. Implementations:
  - `TelegramFormatter`: markdown -> HTML (extract existing logic from telegram.rs)
  - `PlainTextFormatter`: strip all markdown (for WhatsApp, Email)
  - `PassthroughFormatter`: no-op (for Discord, Slack with native markdown)
  - `SlackFormatter`: convert standard markdown to Slack mrkdwn format
- **Files:** New `channels/src/formatter.rs`, channel implementations
- **Risk:** Medium. Markdown parsing edge cases. Need thorough testing.

#### U3: Uniform error feedback
- **Problem:** Only Telegram sends errors back to user. Discord, Slack, WhatsApp, Email, QQ silently log errors.
- **Solution:** All channels wrap `send()` with error recovery that sends a user-facing error message. Standardize format: clear error type + remediation hint.
- **Files:** All channel implementations, `channels/src/manager.rs`
- **Risk:** Low. Must avoid infinite error loops (error sending error message).

#### U4: Typing indicators
- **Problem:** Only Discord shows typing state. Other channels give no feedback during processing.
- **Solution:** Add `send_typing(&self, chat_id: &str) -> Result<()>` with default no-op to `Channel` trait. Implement for Telegram (`sendChatAction`) and Discord (already exists, normalize). Agent loop calls before tool execution.
- **Files:** `channels/src/lib.rs` (trait), `channels/src/telegram.rs`, `channels/src/discord.rs`, agent loop
- **Risk:** Low. Default no-op means non-implementing channels are unaffected.

## Execution Order

1. **C1 first** (dead code removal) — reduces codebase before other changes
2. **C2 + P1-P4 in parallel** — file splitting and performance on separate crate boundaries
3. **U1-U4 in parallel with above** — channels crate is independent
4. **C3-C4 last** — test cleanup after all structural changes settle

## Success Criteria

- [ ] `cargo build --workspace` succeeds
- [ ] `cargo nextest run --workspace` all tests pass
- [ ] `cargo clippy --workspace --all-targets --all-features` 0 warnings
- [ ] `cargo fmt --all --check` passes
- [ ] Net LOC reduction of 3,000+ lines
- [ ] No file > 1,000 lines in modified crates (except inherently large files like provider impls)
- [ ] All 6 channels handle long messages and errors uniformly
