# Entity Store — Implementation Plan Overview

> **For agentic workers:** This is the overview document. Each plan is a separate file.

**Goal:** Replace fixed-schema feature crates with a Notion-like entity store that self-evolves through AI-user collaboration.

**Spec:** `docs/superpowers/specs/2026-04-12-flexible-database-engine-design.md`

---

## Plan Decomposition

The spec is broken into 4 sequential plans. Plans 2 and 3 can execute **in parallel**.

```
Plan 1: Foundation ──────────────────┐
  (entity-store, database-tool,      │
   Tauri commands, MCP, events)      │
                                     │
         ┌───────────────────────────┤
         │                           │
Plan 2: AI Integration          Plan 3: Frontend
  (skills, mirror, reforge,       (types, hooks, views,
   cognitive, autotuner)           dashboard, navigation)
         │                           │
         └───────────────────────────┤
                                     │
Plan 4: Templates + Cleanup ─────────┘
  (task-management, finance,
   remove old crates, e2e tests)
```

## Plan Files

| Plan | File | Tasks | Status |
|------|------|-------|--------|
| 1 | `2026-04-12-entity-store-p1-foundation.md` | ~25 | Pending |
| 2 | `2026-04-12-entity-store-p2-ai-integration.md` | ~20 | Pending (blocked by P1) |
| 3 | `2026-04-12-entity-store-p3-frontend.md` | ~25 | Pending (blocked by P1) |
| 4 | `2026-04-12-entity-store-p4-templates-cleanup.md` | ~15 | Pending (blocked by P2+P3) |

## Execution Strategy

1. Execute Plan 1 first (foundation must exist before anything else)
2. Execute Plans 2 and 3 in parallel (independent subsystems)
3. Execute Plan 4 last (integration + cleanup)

Each plan can be executed via subagent-driven-development or inline execution.
