# Tasks: Entity-update intent projection

> **For agentic workers:** after plan approval, pick one execute skill —
> `build-in-waves`, `build-by-story`, or `build-inline`.
> The chosen skill writes `Execution-mode:`. Steps use checkbox (`- [x]`) syntax.

Feature code: EUPI
Status: Approved
Date: 2026-09-02
Execution-mode: continuous
Max-concurrency: auto
Requirements: ./requirements.md
Design: ./design.md

**Goal:** Chat ToolEnd and MCP bridge-executed tools share one pure `app-core` entity-update intent projection (per-tool read-only denylists, id `"*"`) while session auto-context stays on a separate map.

**Architecture:** New `project_entity_update` module owns static entries; `event_translator` and `klyntbot-server` handler call it; `session_tool_domain` replaces emission-coupled `tool_domain`. See `design.md` and ADR-0103.

**Tech Stack:** Rust workspace (`app-core`, `klyntbot-server`, `desktop-shared`), `cargo nextest`.

## Global Constraints

- `Claude.md` (sha256: `62d190bc553b55763b9bdbb1eaeed4e12ddc8133dc3d1e74573475c5c03c71a4`) — `cargo nextest`, clippy zero warnings, conventional commits, surgical diffs.
- `docs/agents/project.md` (sha256: `b8e0f8e7bcbda17191d343079c9f62239f9eea66badd31985568303c967ee0a8`) — verify commands; Team band Small.
- ADRs `docs/adr/0100`–`0102` unchanged; `docs/adr/0103-entity-update-intent-projection.md` binding.
- No `app-core` → `klyntbot-server` new dep direction (handler already depends on `app-core`).
- Do not put requirement IDs in production/test source titles; cite IDs only in task footers.

## File Structure

| Path | Responsibility |
|---|---|
| `crates/app-core/src/entity_update_intent.rs` (create) | `EntityUpdateIntent`, static table, `project_entity_update` |
| `crates/app-core/src/lib.rs` | `pub mod entity_update_intent` |
| `crates/app-core/src/handlers/chat/streaming.rs` | Private `session_tool_domain`; remove emit helpers |
| `crates/app-core/src/handlers/chat/event_translator.rs` | ToolEnd uses projection; EntityCreated unchanged |
| `crates/klyntbot-server/src/handler.rs` | Bridge path uses projection; gate on tool success; delete old tables |
| Unit tests colocated under those modules / `crates/app-core` / `crates/klyntbot-server` | Projection, consumers, session, guards |

## Tasks

### Task 1: Pure entity-update intent module

**Files:**
- Create: `crates/app-core/src/entity_update_intent.rs`
- Modify: `crates/app-core/src/lib.rs`
- Test: unit tests in `entity_update_intent.rs` (`#[cfg(test)]`)

**Reuse:** rung 7 — new module in `app-core` (design: Entity-update intent module)

**Interfaces:**
- Consumes: `desktop_shared::types::EntityKind`
- Produces: `EntityUpdateIntent { kind: EntityKind, id: &'static str }`, `project_entity_update(tool_name: &str, action: Option<&str>) -> Vec<EntityUpdateIntent>`; static entries per design table (multi-kind `okr`; denylists as in `design.md`)

**Depends-on:** none

**Steps:**
- [x] Write failing unit tests: no entry ⇒ empty; read-only action ⇒ empty (include `list_recurring` for tasks); missing/unknown action with entry ⇒ kinds with id `"*"`; `okr` ⇒ Objective+KeyResult; cover mismatch verbs (`show`, `status`, `stats`, `query`, `search-semantic`, `search-hybrid`) under the seeded denylists
- [x] Run `cargo nextest run -p app-core -E 'test(project_entity_update)'` — expect fail (module missing)
- [x] Implement module + export; seed full design table; do not parse params/results; do not touch AiFeatureRegistry
- [x] Run same filter — expect pass
- [x] Commit `feat(app-core): add entity-update intent projection`

_Requirements: EUPI-1.1, EUPI-1.2, EUPI-1.3, EUPI-1.4, EUPI-1.5, EUPI-1.6, EUPI-1.7, EUPI-1.8, EUPI-1.9, EUPI-1.10, EUPI-1.11, EUPI-5.1_

---

### Task 2: Session-only `session_tool_domain`

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`
- Test: unit tests for `auto_detect_context` / session mapping (extend existing or add beside streaming)

**Reuse:** rung 2 — rename/narrow existing `tool_domain` (design: Session tool-domain helper)

**Interfaces:**
- Consumes: none from Task 1
- Produces: private `session_tool_domain(tool_name: &str) -> Option<&'static str>` with today’s mappings; `auto_detect_context` uses it only

**Depends-on:** Task 3

**Steps:**
- [x] Write/adjust failing tests proving session mapping unchanged for `tasks`/`todo`/`project`/`area`/`okr`/`finance` and that session tests do not import `entity_update_intent`
- [x] Run `cargo nextest run -p app-core -E 'test(auto_detect_context) | test(session_tool_domain)'` — expect fail or red on rename
- [x] Rename to private `session_tool_domain`; **only after Task 3 removed emit imports**, delete `entity_kind_for_tool` and `is_mutating_action`; ensure no emit path remains in this file
- [x] Tests pass; commit `refactor(app-core): narrow session_tool_domain off emission`

_Requirements: EUPI-4.1, EUPI-4.2, EUPI-4.3, EUPI-4.4, EUPI-6.1_

---

### Task 3: Chat ToolEnd consumer

**Files:**
- Modify: `crates/app-core/src/handlers/chat/event_translator.rs`
- Test: translator unit tests covering ToolEnd success/failure and EntityCreated guard

**Reuse:** rung 2 — extend event_translator ToolEnd (design: Chat ToolEnd consumer)

**Interfaces:**
- Consumes: `project_entity_update`
- Produces: `entity:updated` UiEmissions with `id: "*"` from ToolEnd success only

**Depends-on:** Task 1

**Steps:**
- [x] Failing tests: successful ToolEnd with mutating projected action emits kinds+`*`; read-only projected action emits nothing; failed ToolEnd emits nothing; EntityCreated still emits concrete `entity_id`
- [x] Run `cargo nextest run -p app-core -E 'test(tool_end) | test(entity_created) | test(event_translator)'` (adjust filter to new test names) — expect fail
- [x] Wire ToolEnd to `project_entity_update`; remove imports of `is_mutating_action` / `entity_kind_for_tool` from this file (leave the functions in `streaming.rs` until Task 2 deletes them); leave EntityCreated path untouched
- [x] Pass; commit `feat(app-core): emit entity updates from shared projection on ToolEnd`

_Requirements: EUPI-2.1, EUPI-2.2, EUPI-2.3, EUPI-2.4, EUPI-6.2, EUPI-6.3_

---

### Task 4: MCP bridge consumer + tool-error skip

**Files:**
- Modify: `crates/klyntbot-server/src/handler.rs`
- Test: `crates/klyntbot-server` unit tests for emit helper / handler path

**Reuse:** rung 2 — extend handler emit path (design: MCP bridge-executed consumer)

**Interfaces:**
- Consumes: `app_core::entity_update_intent::project_entity_update`; gate with `!result.is_error.unwrap_or(false)` (rmcp pattern as in `crates/mcp` `tool_adapter.rs`)
- Produces: `emit_entity_updated(kind, "*")` only on successful tool outcomes when projection non-empty

**Depends-on:** Task 1

**Steps:**
- [x] Failing tests: success + mutating action emits; read-only skips; tool-error `CallToolResult` (`is_error: Some(true)`) skips; `get_status`/`agent` paths unchanged (no projection); shared cases match Task 1 intents for same name/action
- [x] Run `cargo nextest run -p klyntbot-server -E 'test(entity_update) | test(emit_entity)'` — expect fail
- [x] Replace `emit_entity_update_for_tool` body; delete `READ_ONLY_ACTIONS`, `NON_FEATURE_TOOL_ENTITY_KINDS`, registry lookup, OKR special-case; gate with `!result.is_error.unwrap_or(false)` before projecting
- [x] Pass; commit `feat(klyntbot-server): project entity updates via app-core intent module`

_Requirements: EUPI-3.1, EUPI-3.2, EUPI-3.3, EUPI-3.4, EUPI-3.5, EUPI-3.6, EUPI-6.5, EUPI-6.6_

---

### Task 5: Shared parity + leave-path guards

**Files:**
- Test: shared cases module or duplicated fixtures in `app-core` + `klyntbot-server` tests; optional smoke for leave paths
- Modify: none required unless a small test util under `crates/app-core/src/entity_update_intent.rs` is exported for fixtures

**Reuse:** rung 2 — extend tests at agreed seams (design: Seams for testing)

**Interfaces:**
- Consumes: `project_entity_update`; chat/MCP consumer test harnesses from Tasks 3–4
- Produces: automated proof same name/action ⇒ same intents on both consumers; leave-path assertions

**Depends-on:** Task 3, Task 4

**Steps:**
- [x] Add shared fixture table (tool, action, expected kinds) asserted in both consumer test suites (EUPI-3.5 / EUPI-5.1)
- [x] Guard tests or comments+smoke: handler `EntityUpdate` concrete ids still constructible; `mcp::dispatch_entity_update` still maps kind→domain; document desktop `useMutation` unchanged (no code change)
- [x] Run `cargo nextest run -p app-core -p klyntbot-server -p mcp -E 'test(entity_update) | test(parity) | test(dispatch_entity)'` — expect pass
- [x] Commit `test(eupi): shared projection parity and leave-path guards`

_Requirements: EUPI-3.5, EUPI-5.1, EUPI-6.4, EUPI-6.7, EUPI-6.8_

---

## Coverage check

Every Approved `EUPI-*` ID appears in ≥1 task footer above. Seam table IDs covered by Tasks 1–5. Placeholder scan: clean.
