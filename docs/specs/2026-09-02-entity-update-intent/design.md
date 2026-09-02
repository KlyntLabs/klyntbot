# Design: Entity-update intent projection

Feature code: EUPI
Status: Approved
Date: 2026-09-02
Requirements: ./requirements.md

## Context

Today chat and MCP invent tool→kind and mutate heuristics independently. Chat `ToolEnd` in `event_translator.rs` calls `is_mutating_action` + `entity_kind_for_tool` (derived from `tool_domain` in `streaming.rs`) and emits `entity:updated` with id `""`. MCP bridge success in `klyntbot-server` `handler.rs` uses `READ_ONLY_ACTIONS`, `AiFeatureRegistry` lookup, `NON_FEATURE_TOOL_ENTITY_KINDS`, and params id or `"*"`, with an OKR special-case second emit for `KeyResult`. The same `tool_domain` map also drives session `auto_detect_context`.

The binding constraint is **one pure classification seam** for successful tool-call → kind-level refresh intent, shared by chat ToolEnd and MCP bridge-executed tools, without expanding AiFeatureRegistry or Tool exposure into a refresh catalog (ADR-0103). That rules out “just call the registry from chat,” wrapping emit in Tool derive policy, or unifying desktop `useMutation` in this change.

Fresh retrieval: catalog shard `agent-runtime` holds **EXPO** (path neighbor via `app-core` / `klyntbot-server`) and **EUPI**. No code reuse from EXPO for emit classification (`owns_coverage` incomplete globally; these two cards share surface roots only). Scan digest: `.skills/EUPI/scan.md`. No `docs/architecture/` spine.

## Decisions

1. Pure `app-core` entity-update intent module; inputs = tool name + action only; output intents always use id `"*"` (ADR-0103).
2. Per-tool read-only denylists; no entry ⇒ no intent; missing/empty/unknown action ⇒ intent when entry exists.
3. Hand-maintained static table with multi-kind entries (e.g. `okr` → Objective + KeyResult).
4. Consumers: chat successful ToolEnd + MCP successful bridge-executed tools only.
5. Session: private `session_tool_domain`; preserve auto_detect mapping; no import of the intent module.
6. Remove emit use of `is_mutating_action`, `entity_kind_for_tool`, MCP `READ_ONLY_ACTIONS`, `NON_FEATURE_TOOL_ENTITY_KINDS`, and AiFeatureRegistry lookup for emission.
7. Approach: static table (frame-change approach 1); extract to a lower crate only if a non-app-core consumer appears later.

## Architecture

### Entity-update intent module (`app-core`)

Satisfies: EUPI-1.1, EUPI-1.2, EUPI-1.3, EUPI-1.4, EUPI-1.5, EUPI-1.6, EUPI-1.7, EUPI-1.8, EUPI-1.9, EUPI-1.10, EUPI-1.11, EUPI-5.1
Reuse: rung 7 — new module in `app-core` (no existing shared classify helper; must not reuse AiFeatureRegistry for this)
Surface: new readers only (omit pre-existing reader inventory)
Interface:
- `EntityUpdateIntent { kind: EntityKind, id: &'static str }` with `id` always `"*"` for projected intents (type distinct from `state::EntityUpdate`).
- `pub fn project_entity_update(tool_name: &str, action: Option<&str>) -> Vec<EntityUpdateIntent>`
- Static `&[ProjectionEntry]` where each entry is `{ tool_name, kinds: &[EntityKind], read_only_actions: &[&str] }` (exact field names may vary; semantics locked).
- Empty/`None` action treated as not in the denylist ⇒ intents when entry exists.
Depth: Callers must still know the tool name and action strings from the call site and how to emit `entity:updated`; they must not know per-tool denylist contents, multi-kind fan-out tables, or former chat/MCP heuristic divergence.
Locality: new file e.g. `crates/app-core/src/entity_update_intent.rs` (+ `pub mod` in `lib.rs`); neighbors chat/MCP **extend** to call it; AiFeatureRegistry **leave**; `mcp::dispatch` **leave**

Initial static table (kinds + read-only denylists seeded from live tool schemas; mismatch verbs included where applicable):

| tool_name | kinds | read_only_actions |
|---|---|---|
| `tasks`, `todo` | Task | `list`, `show`, `summary`, `tree`, `search`, `list_recurring`, `search-semantic`, `search-hybrid`, `query`, `status`, `stats`, `get` |
| `project` | Project | `list`, `show`, `tasks`, `search`, `get`, `query`, `status`, `stats` |
| `area` | Area | `list`, `show`, `search`, `get`, `query`, `status`, `stats` |
| `okr` | Objective, KeyResult | `objective.list`, `objective.show`, `kr.list`, `kr.show`, `list`, `show`, `get`, `search`, `query`, `status`, `stats` |
| `notes` | Note | `list_notes`, `get_note`, `search_notes`, `list_notebooks`, `list_archived`, `get_backlinks`, `list_inbox`, `list`, `show`, `get`, `search`, `query`, `status`, `stats`, `search-semantic`, `search-hybrid` |
| `productivity` | Productivity | `focus_status`, `activity_today`, `activity_summary`, `activity_week`, `activity_score`, `activity_compare`, `check_goals`, `list_goals`, `list_categories`, `activity_export`, `list`, `show`, `get`, `search`, `query`, `status`, `stats` |
| `work_context` | Productivity | `list`, `show`, `search`, `get`, `query`, `status`, `stats` |

Do not add `finance` until `EntityKind` gains a finance variant (today chat `tool_domain` maps it but `EntityKind::parse("finance")` fails, so chat never emitted). Extra denylist strings that a tool never accepts are harmless (unknown actions still mutate per EUPI-1.6 only when not listed).

### Chat ToolEnd consumer

Satisfies: EUPI-2.1, EUPI-2.2, EUPI-2.3, EUPI-2.4, EUPI-6.2, EUPI-6.3
Reuse: rung 2 — extend `event_translator` ToolEnd handling
Surface:
- ToolEnd emit path using `is_mutating_action` / `entity_kind_for_tool` — **replace**
- `EntityCreated` emit path — **frozen** (concrete card id contract; leave untouched)
- UI listeners of `entity:updated` — **frozen** (event name/payload shape unchanged; id becomes `"*"` on this path only, already tolerated)
Interface: on successful ToolEnd, `let intents = project_entity_update(&name, action.as_deref());` then for each intent push `UiEmission` with `ENTITY_UPDATED` and `{ entityKind, id: "*" }`. Failed ToolEnd skips projection. Delete emit imports of the old helpers.
Depth: n/a — extends event_translator
Locality: `crates/app-core/src/handlers/chat/event_translator.rs` **extend**; streaming emit helpers **extract**/remove from emit path

### MCP bridge-executed consumer

Satisfies: EUPI-3.1, EUPI-3.2, EUPI-3.3, EUPI-3.4, EUPI-3.5, EUPI-3.6, EUPI-6.5, EUPI-6.6
Reuse: rung 2 — extend `emit_entity_update_for_tool` (or inline replacement at call site)
Surface:
- `emit_entity_update_for_tool` heuristics / registry / fallback table — **replace**
- MCP builtin `agent` post-execute broadcast — **frozen** (outside seam)
- MCP `get_status` — **frozen** (no entity emit)
- Today’s emit-after-any-`Ok(CallToolResult)` including tool-error results — **replace** (gate on successful tool outcome only)
Interface: after bridge `execute` returns, **if** the `CallToolResult` is a successful tool outcome (not `CallToolResult::error` / `is_error`), call `project_entity_update(name, params["action"].as_str())` and for each intent `emitter.emit_entity_updated(kind, "*")`; otherwise skip emission. Delete `READ_ONLY_ACTIONS`, `NON_FEATURE_TOOL_ENTITY_KINDS`, registry lookup, and OKR special-case once `okr` multi-kind lives in the table.
Depth: n/a — extends klyntbot-server handler
Locality: `crates/klyntbot-server/src/handler.rs` **extend**; depends on existing `app-core` dep

### Session tool-domain helper

Satisfies: EUPI-4.1, EUPI-4.2, EUPI-4.3, EUPI-4.4, EUPI-6.1
Reuse: rung 2 — rename/narrow existing `tool_domain` map used by `auto_detect_context`
Surface:
- `tool_domain` / `entity_kind_for_tool` / `is_mutating_action` public helpers — **replace** (remove emit helpers; rename session map to private `session_tool_domain`)
- `auto_detect_context` callers — **compat** only if signature changes; prefer keep `auto_detect_context` signature and swap internal map name (**replace** internals)
Interface: `fn session_tool_domain(tool_name: &str) -> Option<&'static str>` private to the chat streaming module (or equivalent privacy); same mapping values as today’s `tool_domain` (`todo`|`tasks`→`task`, `project`, `area`, `okr`→`objective`, `finance`→`finance`). Must not import `entity_update_intent`.
Depth: n/a — extends streaming session path
Locality: `crates/app-core/src/handlers/chat/streaming.rs` **extend**; projection module **leave**

### Leave paths (guards only)

Satisfies: EUPI-6.4, EUPI-6.7, EUPI-6.8
Reuse: rung 2 — leave existing handler `EntityUpdate`, `mcp::dispatch_entity_update`, and desktop `useMutation` emit paths
Surface:
- Handler `EntityUpdate` concrete id emits — **frozen**
- `mcp::dispatch_entity_update` kind→RecallDomain — **frozen**
- Desktop `useMutation` `entity:updated` — **frozen**
Interface: unchanged; no calls into `project_entity_update`.
Depth: n/a — extends nothing (explicit non-touch)
Locality: **leave** `state.rs` mutation results, `crates/mcp/src/dispatch.rs`, `desktop-ui` mutation hook

## Seams for testing

| Seam | Kind | Covers |
|---|---|---|
| `project_entity_update(tool, action)` | unit | EUPI-1.1–1.11, EUPI-5.1 (classification half) |
| Shared fixture cases (same tool/action → same intents) applied to chat ToolEnd translator + MCP emit helper | unit | EUPI-2.1–2.4, EUPI-3.1–3.5, EUPI-5.1 |
| MCP emit skipped on tool-error `CallToolResult` | unit | EUPI-3.6 |
| `session_tool_domain` / `auto_detect_context` | unit | EUPI-4.1–4.4, EUPI-6.1 |
| EntityCreated / failed ToolEnd / MCP agent+get_status paths (guards) | unit | EUPI-6.2, EUPI-6.3, EUPI-6.5, EUPI-6.6 |
| `state::EntityUpdate` / `mcp::dispatch` / desktop useMutation | unit (existing or smoke) | EUPI-6.4, EUPI-6.7, EUPI-6.8 |

New seam count: **one** (`project_entity_update`). Consumers tested at existing translator/handler seams.

## Coverage check

| ID | Satisfies section |
|---|---|
| EUPI-1.1–1.11 | Entity-update intent module |
| EUPI-2.1–2.4 | Chat ToolEnd consumer |
| EUPI-3.1–3.6 | MCP bridge consumer (EUPI-3.5 shared fixture covers chat parity) |
| EUPI-4.1–4.4 | Session tool-domain helper |
| EUPI-5.1 | Intent module + shared consumer tests |
| EUPI-6.1 | Session helper |
| EUPI-6.2–6.3 | Chat ToolEnd consumer |
| EUPI-6.4, EUPI-6.7, EUPI-6.8 | Leave paths (guards only) |
| EUPI-6.5–6.6 | MCP bridge consumer |

Deliberately unmapped: none.

NFR Performance/Security/Accessibility `None` lines need no Satisfies module.
