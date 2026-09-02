# Requirements: Entity-update intent projection

Feature code: EUPI
Status: Approved
Date: 2026-09-02

## 1. Classify tool calls into entity-update intents

**Story:** As a maintainer, I want a pure `app-core` function that maps tool name + action to kind-level entity-update intents, so that chat and MCP stop inventing separate mutate/kind rules.

- **EUPI-1.1** THE SYSTEM SHALL provide a pure entity-update intent projection in `app-core` that accepts only a tool name and an action (including missing/empty action) and returns zero or more entity-update intents without constructing `AppCore` or an event emitter.
- **EUPI-1.2** THE SYSTEM SHALL represent each entity-update intent with an entity kind and the wildcard id `"*"`, where `"*"` means kind-level invalidation (not a concrete entity id).
- **EUPI-1.3** THE SYSTEM SHALL keep entity-update intent distinct from `state::EntityUpdate`, which remains a known mutation with a handler-provided concrete id.
- **EUPI-1.4** WHEN a tool has no projection entry THE SYSTEM SHALL return no intents for that tool name and action.
- **EUPI-1.5** WHEN a tool has a projection entry and the action is listed in that tool’s read-only denylist THE SYSTEM SHALL return no intents.
- **EUPI-1.6** WHEN a tool has a projection entry and the action is missing, empty, unknown, or otherwise not in that tool’s read-only denylist THE SYSTEM SHALL return intents for every entity kind declared on that entry, each with id `"*"`.
- **EUPI-1.7** THE SYSTEM SHALL implement projection entries as a hand-maintained static table (including multi-kind entries such as tools that invalidate more than one kind).
- **EUPI-1.8** THE SYSTEM SHALL NOT inspect tool params or tool results inside the projection module.
- **EUPI-1.9** THE SYSTEM SHALL NOT decide tool existence, registration, Tool exposure policy, MCP membership, approval class, session auto-context, or recall-domain dispatch inside the projection module.
- **EUPI-1.10** THE SYSTEM SHALL NOT consult `AiFeatureRegistry` to classify entity-update intents.
- **EUPI-1.11** WHEN tests exercise the projection module THE SYSTEM SHALL cover projected and non-projected tools and SHALL include the former chat/MCP mismatch action names (`show`, `status`, `stats`, `query`, `search-semantic`, `search-hybrid`) under the per-tool denylist rules — verified by automated unit tests.

## 2. Emit from chat ToolEnd via the shared projection

**Story:** As a desktop user, I want a successful mutating chat tool call to refresh lists via the shared projection, so that chat refresh matches MCP for the same tool name and action.

- **EUPI-2.1** WHEN a chat `ToolEnd` event reports success THE SYSTEM SHALL compute entity-update intents solely via the shared projection using that tool’s name and action.
- **EUPI-2.2** WHEN that projection returns one or more intents THE SYSTEM SHALL emit `entity:updated` for each intent’s kind with id `"*"`.
- **EUPI-2.3** WHEN that projection returns no intents THE SYSTEM SHALL NOT emit `entity:updated` from the ToolEnd projection path for that tool call.
- **EUPI-2.4** THE SYSTEM SHALL remove or stop using `is_mutating_action` and `entity_kind_for_tool` for emission (they MUST NOT reconnect emission to the session tool→domain map).

## 3. Emit from MCP bridge-executed tools via the shared projection

**Story:** As an MCP client user, I want a successful bridge-executed tool call to refresh via the same projection as chat, so that dual-path drift cannot recur for registry-backed tools.

- **EUPI-3.1** WHEN a bridge-executed (non-builtin) MCP tool call completes with a successful tool outcome (not a tool-error `CallToolResult`) THE SYSTEM SHALL compute entity-update intents solely via the shared projection using that tool’s name and action.
- **EUPI-3.2** WHEN that projection returns one or more intents THE SYSTEM SHALL emit `entity:updated` for each intent’s kind with id `"*"`.
- **EUPI-3.3** WHEN that projection returns no intents THE SYSTEM SHALL NOT emit `entity:updated` from the bridge-executed projection path for that tool call.
- **EUPI-3.4** THE SYSTEM SHALL remove or stop using MCP `READ_ONLY_ACTIONS`, `NON_FEATURE_TOOL_ENTITY_KINDS`, and AiFeatureRegistry lookup for emission on that path.
- **EUPI-3.5** WHEN the same projected tool name and action succeed on chat ToolEnd and on MCP bridge execution THE SYSTEM SHALL produce the same set of entity-update intents (kinds + `"*"`) — verified by automated tests shared across both consumer cases.
- **EUPI-3.6** WHEN a bridge-executed tool call completes as a tool-error outcome (`CallToolResult` error) THE SYSTEM SHALL NOT emit projection-based `entity:updated` for that call (aligning with chat ToolEnd failure omission).

## 4. Keep session auto-context on a separate map

**Story:** As a chat user, I want session auto-context to keep resolving tool domains as today, so that narrowing emission does not silently change which session domain is detected.

- **EUPI-4.1** THE SYSTEM SHALL back session auto-context with a private session-only tool→domain helper named `session_tool_domain` (renamed/narrowed from today’s emission-coupled `tool_domain` usage).
- **EUPI-4.2** THE SYSTEM SHALL preserve the current session-domain mapping values and `auto_detect_context` behavior for tools that already map to a domain.
- **EUPI-4.3** THE SYSTEM SHALL ensure `session_tool_domain` does not call, import, or derive from the entity-update projection module.
- **EUPI-4.4** WHEN tests cover session-domain inference THE SYSTEM SHALL exercise it independently from entity-update projection tests — verified by automated tests that do not require projection entries for session mapping.

## 5. Quality attributes

**Section-kind:** nfr

**Story:** As a stakeholder, I want measurable quality targets for this feature, so that how-well is not left implicit.

- **Performance:** None — no standing `docs/product/metrics.md`; classification is pure in-process table lookup, not a measured interactive latency surface.
- **Security:** None — no standing `docs/security/threat-model.md`; this feature does not change authn/authz, approval, or exposure membership (ADR-0100–0102 unchanged).
- **Reliability:** **EUPI-5.1** WHEN chat ToolEnd and MCP bridge-executed paths classify the same tool name and action THE SYSTEM SHALL emit identical intent sets (including empty) — verified by automated shared consumer tests (EUPI-3.5). (Consult: `docs/ops/reliability.md` Absent — target from close-package success signal.)
- **Accessibility:** None — no new user-perceived UI surface; emits existing `entity:updated` events only.

## 6. Existing behavior guards

Touched surfaces and guards:

- `crates/app-core/src/handlers/chat/streaming.rs` — **EUPI-6.1** (guard) WHEN session auto-context runs after tools THE SYSTEM SHALL CONTINUE TO set session context only when a single mapped domain dominates among tool names used in the turn.
- `crates/app-core/src/handlers/chat/event_translator.rs` — **EUPI-6.2** (guard) WHEN an `EntityCreated` agent event arrives THE SYSTEM SHALL CONTINUE TO emit `entity:updated` using the card’s entity kind and concrete `entity_id` (not the ToolEnd projection path).
- `crates/app-core/src/handlers/chat/event_translator.rs` — **EUPI-6.3** (guard) WHEN ToolEnd reports failure THE SYSTEM SHALL CONTINUE TO omit projection-based `entity:updated` emission for that failed call.
- `crates/app-core/src/state.rs` — **EUPI-6.4** (guard) WHEN handlers return `EntityUpdate` values THE SYSTEM SHALL CONTINUE TO support concrete kind+id mutation payloads for desktop command emission.
- `crates/klyntbot-server/src/handler.rs` — **EUPI-6.5** (guard) WHEN the MCP builtin `agent` completes successfully THE SYSTEM SHALL CONTINUE TO broadcast its existing hardcoded kind-level invalidations outside the shared projection.
- `crates/klyntbot-server/src/handler.rs` — **EUPI-6.6** (guard) WHEN MCP `get_status` is invoked THE SYSTEM SHALL CONTINUE TO omit entity-update emission for that builtin.
- `crates/mcp/src/dispatch.rs` — **EUPI-6.7** (guard) WHEN recall-domain dispatch resolves a kind THE SYSTEM SHALL CONTINUE TO map kind→`RecallDomain` via `AiFeatureRegistry` independently of entity-update intent projection.
- `desktop-ui` `useMutation` / entity listeners — **EUPI-6.8** (guard) WHEN desktop commands mutate via `useMutation` THE SYSTEM SHALL CONTINUE TO emit `entity:updated` from that path without requiring the new projection module.
- Projection table seeding — no behavior to guard beyond EUPI-1.* for tools newly added to the table (additive).

## Out of Scope

- Redesigning Tool exposure policy or MCP Default/OptIn/Forbidden membership (ADR-0100–0102).
- Replacing AiFeatureRegistry’s recall-domain role.
- Unifying desktop `useMutation` emitters with the projection.
- Changing MCP `agent` builtin invalidation semantics (beyond leaving it outside the seam).
- Parsing params or tool results for concrete ids; row-level invalidation enrichment.
- Merging `EntityCreated` concrete-id emits into the ToolEnd projection.
- Extracting the pure module into a lower crate (defer until a non-`app-core` consumer exists).
- Requiring session tool-set and projection tool-set equality.
- Teaching approval_class to drive mutation/refresh classification.

## Open Questions

None — no owned unknowns from the confirmed close package.
