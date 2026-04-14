# Notion-grade Database View System

**Date:** 2026-04-14
**Status:** Proposed — awaiting green light on phase 1
**Scope:** Upgrade the database view system to Notion/Linear-grade: view CRUD, all view types with DnD, fractional-index ordering, headless table engine, nested filters, grouping on every view.

## Goals

- Every database entity kind (tasks, notes, projects, future kinds) gets 8 view types, filters, groups, and DnD **for free** by declaring fields.
- Drag-drop quality matches Trello / Jira (pragmatic-drag-and-drop, the engine behind them).
- Reordering is O(1) at any scale (fractional indexing with jitter).
- View management is data, not code: users create/rename/duplicate/delete views at runtime from a Notion-style picker.
- Filters are nested AND/OR groups (up to 3 levels), applied per-view.
- Grouping works on every view, not just Board.
- Table view powered by TanStack Table + TanStack Virtual (sort/filter/group/resize/virtualize for free).

## Non-goals

- Multi-device sync / CRDTs (single-user local app; revisit if we add sync).
- Map view (no geo field type yet).
- Formula expression engine (defer — requires Rust expression crate choice).
- Dashboard composition (compose after chart view lands).

## Libraries to adopt

| Lib | Why | Size |
|---|---|---|
| `@atlaskit/pragmatic-drag-and-drop` | Powers Trello/Jira. Half the size of dnd-kit. File drops included (future attachments). | ~4.7kB core |
| `fractional-indexing-jittered` | Rocicorp base62 + jitter. No rebalancing. Concurrent-safe. | ~2kB |
| `@tanstack/react-table` + `@tanstack/react-virtual` | Headless table engine: sort, multi-sort, filter, group, pagination, column resize/visibility/ordering, row selection. Pairs with virtual for 1k+ rows. | ~14kB + ~5kB |

## Architectural decisions

1. **Entity gets `position: String`** — fractional index, not `f64`. Rust migration in-place (pre-release).
2. **Filters become nested groups**: `FilterGroup { op: And|Or, rules: Vec<FilterNode> }` where `FilterNode = Rule(FilterRule) | Group(FilterGroup)`. Max depth 3 enforced in Rust.
3. **Grouping is cross-view**: `ViewConfig.groupBy` applies to every view type. Board uses it for columns (current behavior); other views get collapsible sections.
4. **One mutation path**: `database_entity_patch(id, partial_fields)` handles every drag-drop outcome (board column change, calendar date change, sortable reorder via `position` patch, inline edits).
5. **View configs stay loosely typed at the Rust layer** (single `ViewConfig` struct with optional fields) but the **frontend uses discriminated unions per view type** for editor type safety.

## Phasing

Each phase is independently demo-able. All uncommitted per user directive.

### Phase 1 — Backend foundation (Rust)
1. Add `position: String` to `Entity` type + SQLite column. Populate existing rows via fractional indexing.
2. Add `database_entity_patch(id, fields: HashMap) -> Entity` IPC (generic partial patch).
3. Add `database_entity_reorder(id, before_id: Option<String>, after_id: Option<String>) -> Entity` — computes new fractional key server-side.
4. Add view CRUD IPC: `database_view_create / update / delete / reorder`.
5. Extend `FilterRule` → `FilterNode` enum with group support. Update query engine to evaluate nested groups (max depth 3).
6. Tests: entity reorder edge cases, nested filter evaluation, view CRUD.

**Verify:** `cargo nextest run -p entity-store` passes.

### Phase 2 — Frontend plumbing
1. Install `@atlaskit/pragmatic-drag-and-drop`, `fractional-indexing-jittered`, `@tanstack/react-table`, `@tanstack/react-virtual`.
2. Frontend types: widen `FilterRule` to `FilterNode` union; update `ViewConfig`.
3. Add `useEntityPatch(databaseId)` and `useEntityReorder(databaseId)` mutation hooks.
4. Add `useViewMutations(databaseId)` hook (create/update/delete/reorder).

**Verify:** `bun run lint` + `bun run test`.

### Phase 3 — View CRUD UI (matching Notion screenshot)
1. Replace `ViewTabBar` with `ViewSwitcher`: dropdown listing all views with per-row actions (rename / duplicate / delete / drag-handle to reorder) + "+ New view" opening a 10-type grid picker.
2. `ViewConfigPanel` (right-sheet overlay) editing the active view's config: name, visible fields, groupBy, sort, filter.
3. `FilterBuilder` component: nested AND/OR groups up to 3 levels. Each rule = field + op + value input dispatched on field type.
4. Sensible defaults when creating views (first select field for groupBy, first date field for calendar, etc.).

**Verify:** browser — create, rename, duplicate, delete, reorder views. Nested filters persist.

### Phase 4 — DnD on existing views
1. `DndProvider` at `ViewShell` level via pragmatic-drag-and-drop.
2. **Board**: drop card on column → `entity_patch(id, { [groupByField]: columnValue })`. Reorder within column → `entity_reorder`.
3. **Table / List / Gallery**: sortable rows via `position` field. Drag row → `entity_reorder`.
4. **Calendar**: drop on day → `entity_patch(id, { [dateField]: date })`.
5. Optimistic updates via `useMutation` rollback on error.
6. Accessibility: keyboard drag (Space to pick up, arrows to move, Space to drop).

**Verify:** browser — drag across all four view types.

### Phase 5 — TableView rewrite on TanStack
1. Replace hand-rolled table with `useReactTable` hook.
2. Column visibility, column resizing, column reordering (drag column header).
3. Virtualized rows via `useVirtualizer` when `entities.length > 50`.
4. Grouped rows (collapsible headers) when `groupBy` is set.

**Verify:** browser — 500-row test data renders smoothly; sort/filter/group work; column resize persists to view config.

### Phase 6 — Grouping on all views
1. `GroupedContainer` wrapper that splits entities by `groupByField` value, renders collapsible `<details>`-style sections per group.
2. Wire into List, Gallery, Feed views (Board already groups natively).
3. Group state (collapsed keys) persisted to `ViewConfig.layout.collapsedGroups`.

### Phase 7 — New view types: Chart + Feed
1. **Feed**: chronological cards sorted by `lastEditedAt` desc, large typography, good for "recent activity" framing.
2. **Chart**: recharts; config = chart type (bar / line / pie) + x-axis field + y-axis aggregation (count / sum / avg).

### Phase 8 — Polish
1. Toast "Undo" after destructive actions (reorder, delete view).
2. Loading skeletons.
3. Empty states per view type.
4. Reduced-motion DnD alternative (keyboard-only hints visible).

## Open questions (no blocker — assumed defaults)

1. **Formula expression engine** — deferred. When we tackle, recommend `evalexpr` crate.
2. **Rollup evaluation** — depends on relation loading. Defer until Relation field UI lands.
3. **Map view** — skip; add when `FieldType::Geo` exists.

## File-level impact estimate

- Rust: `crates/entity-store/src/{types,store,query,views}.rs` + new migration
- Rust: `crates/desktop/src/commands/database.rs` (new IPC handlers) + `dev_server/mod.rs` DEV_COMMANDS
- Rust: `crates/app-core/src/handlers/database.rs` (new AppCore methods)
- TS: `desktop-ui/src/features/database/**` — heavy changes across all views + new `ViewSwitcher`, `FilterBuilder`, `ViewConfigPanel`, `dnd/` directory
- TS: `desktop-ui/src/shared/types/database.ts` — widen filter types
- TS: new hooks in `desktop-ui/src/features/database/hooks/`

## Success criteria

- User can create a fresh database, add any fields, switch between all 8 views, filter/sort/group, drag items across board columns and between days, reorder rows in table — with no per-feature code.
- TableView at 1000 rows scrolls smoothly.
- Reorder latency < 50ms (optimistic).
- All existing databases (tasks) continue to work unchanged.

## Execution pattern

After each phase:
1. Stop and report what changed.
2. Run verification (tests or browser check).
3. Wait for user ack before continuing (user directive: no commits).

If a phase surfaces scope creep, stop and renegotiate the plan.
