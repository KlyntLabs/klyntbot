# Entity Store — Plan 3: Frontend

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the universal database UI — schema-driven views (table, board, calendar, list, gallery, timeline), field renderers/editors, entity detail, schema editor, dynamic sidebar navigation, custom dashboard builder with widgets.

**Architecture:** A single `features/database/` module renders any database based on its schema. `FieldRenderer` and `FieldEditor` handle all 16 field types. `ViewShell` manages multiple named views per database. `DashboardPage` lets users build custom dashboards with widgets querying any database. The sidebar becomes dynamic — listing user databases alongside fixed system items.

**Tech Stack:** React 18, TypeScript, Tailwind v4, dnd-kit (drag-and-drop), Biome 2.0, Vite, path aliases (@features/, @shared/)

**Spec:** `docs/superpowers/specs/2026-04-12-flexible-database-engine-design.md` (Section 12)

**Depends on:** Plan 1 (Tauri commands must exist for db_* operations)

---

## File Structure

### New Files

| File | Responsibility |
|------|----------------|
| `desktop-ui/src/shared/types/database.ts` | Entity, DatabaseSchema, FieldDefinition, ViewDefinition, FieldType, etc. |
| `desktop-ui/src/features/database/pages/DatabasePage.tsx` | Load schema + active view, render ViewShell |
| `desktop-ui/src/features/database/hooks/useDatabase.ts` | Fetch schema, fields, views via useQuery |
| `desktop-ui/src/features/database/hooks/useEntities.ts` | Query entities with filters/sorts |
| `desktop-ui/src/features/database/hooks/useEntity.ts` | Single entity CRUD |
| `desktop-ui/src/features/database/hooks/useViews.ts` | View CRUD |
| `desktop-ui/src/features/database/components/ViewShell.tsx` | View tabs + toolbar + active view |
| `desktop-ui/src/features/database/components/ViewToolbar.tsx` | Filters, sorts, search, group-by, new entity |
| `desktop-ui/src/features/database/components/ViewTabBar.tsx` | View tabs with add/rename/delete |
| `desktop-ui/src/features/database/components/fields/FieldRenderer.tsx` | Switch on fieldType → render read-only |
| `desktop-ui/src/features/database/components/fields/FieldEditor.tsx` | Switch on fieldType → render editable |
| `desktop-ui/src/features/database/components/fields/renderers/*.tsx` | Per-type renderers (Text, Number, Select, Date, etc.) |
| `desktop-ui/src/features/database/components/fields/editors/*.tsx` | Per-type editors |
| `desktop-ui/src/features/database/components/views/TableView.tsx` | Schema-driven DataTable |
| `desktop-ui/src/features/database/components/views/BoardView.tsx` | Kanban grouped by select field |
| `desktop-ui/src/features/database/components/views/CalendarView.tsx` | Monthly/weekly calendar |
| `desktop-ui/src/features/database/components/views/ListView.tsx` | Compact list |
| `desktop-ui/src/features/database/components/views/GalleryView.tsx` | Card grid |
| `desktop-ui/src/features/database/components/views/TimelineView.tsx` | Gantt-style horizontal bars |
| `desktop-ui/src/features/database/components/entity/EntityDetail.tsx` | Full entity slide panel |
| `desktop-ui/src/features/database/components/entity/PropertyList.tsx` | All fields rendered dynamically |
| `desktop-ui/src/features/database/components/entity/CreateEntityModal.tsx` | Dynamic form from schema |
| `desktop-ui/src/features/database/components/schema/SchemaEditor.tsx` | Add/remove/reorder fields |
| `desktop-ui/src/features/database/components/schema/FieldTypeSelector.tsx` | Dropdown of 16 types |
| `desktop-ui/src/features/database/components/suggestions/SchemaSuggestionBar.tsx` | AI suggestions |
| `desktop-ui/src/features/database/lib/query-builder.ts` | Build filter/sort params from ViewConfig |
| `desktop-ui/src/features/database/lib/field-utils.ts` | Format, validate, compare field values |
| `desktop-ui/src/features/dashboard/components/DashboardBuilder.tsx` | Widget grid with drag-and-drop |
| `desktop-ui/src/features/dashboard/components/widgets/*.tsx` | Widget renderers (count, list, chart, metric, etc.) |

### Modified Files

| File | Change |
|------|--------|
| `desktop-ui/src/app/router.tsx` | Add `/db/:databaseId` and `/dashboard/:dashboardId` routes, remove `/tasks` and `/finance/*` |
| `desktop-ui/src/app/layouts/Sidebar.tsx` | Dynamic database items from `db_list` query + "New Database" button |
| `desktop-ui/src/shared/types/index.ts` | Add `database.ts` export |

### Removed Files

| File | Why |
|------|-----|
| `desktop-ui/src/features/tasks/` (entire directory) | Replaced by `features/database/` |
| `desktop-ui/src/features/finance/` (entire directory) | Replaced by `features/database/` |
| `desktop-ui/src/shared/types/tasks.ts` | Replaced by `database.ts` Entity type |
| `desktop-ui/src/shared/types/finance.ts` | Replaced by `database.ts` Entity type |

---

## Task 1: Define TypeScript types for database system

**Files:**
- Create: `desktop-ui/src/shared/types/database.ts`
- Modify: `desktop-ui/src/shared/types/index.ts`

- [ ] **Step 1: Define all types matching the Rust types from entity-store**

```typescript
// Entity, DatabaseSchema, FieldDefinition, FieldType, ViewDefinition,
// ViewType, ViewConfig, FilterRule, FilterOp, SortRule, SortDirection,
// EntityRelation, Dashboard, WidgetDefinition, GridPosition
```

- [ ] **Step 2: Export from index.ts**
- [ ] **Step 3: Run lint**

Run: `cd desktop-ui && bun run lint`

- [ ] **Step 4: Commit**

---

## Task 2: Create database hooks

**Files:**
- Create: `desktop-ui/src/features/database/hooks/useDatabase.ts`
- Create: `desktop-ui/src/features/database/hooks/useEntities.ts`
- Create: `desktop-ui/src/features/database/hooks/useEntity.ts`
- Create: `desktop-ui/src/features/database/hooks/useViews.ts`

- [ ] **Step 1: Implement useDatabase — useQuery("db_get_schema", { databaseId })**
- [ ] **Step 2: Implement useEntities — useQuery("db_query", { databaseId, filters, sorts, limit, offset })**
- [ ] **Step 3: Implement useEntity — useMutation for CRUD operations**
- [ ] **Step 4: Implement useViews — CRUD for views**
- [ ] **Step 5: Commit**

---

## Task 3: Build FieldRenderer and FieldEditor

**Files:**
- Create: `desktop-ui/src/features/database/components/fields/FieldRenderer.tsx`
- Create: `desktop-ui/src/features/database/components/fields/FieldEditor.tsx`
- Create: `desktop-ui/src/features/database/components/fields/renderers/TextRenderer.tsx`
- Create: `desktop-ui/src/features/database/components/fields/renderers/NumberRenderer.tsx`
- Create: `desktop-ui/src/features/database/components/fields/renderers/SelectRenderer.tsx`
- Create: `desktop-ui/src/features/database/components/fields/renderers/DateRenderer.tsx`
- Create: `desktop-ui/src/features/database/components/fields/renderers/CheckboxRenderer.tsx`
- Create: similar for editors/

- [ ] **Step 1: Implement FieldRenderer — switch on field.fieldType, delegate to per-type renderer**
- [ ] **Step 2: Implement FieldEditor — switch on field.fieldType, delegate to per-type editor**
- [ ] **Step 3: Implement renderers for all 16 types (simple ones: text, url, email, phone can share a TextRenderer)**
- [ ] **Step 4: Implement editors for editable types (text, number, select, multi_select, date, checkbox, url, email, phone, relation)**
- [ ] **Step 5: Write Vitest tests for FieldRenderer**
- [ ] **Step 6: Commit**

---

## Task 4: Build EntityDetail and CreateEntityModal

**Files:**
- Create: `desktop-ui/src/features/database/components/entity/EntityDetail.tsx`
- Create: `desktop-ui/src/features/database/components/entity/PropertyList.tsx`
- Create: `desktop-ui/src/features/database/components/entity/CreateEntityModal.tsx`

- [ ] **Step 1: EntityDetail — slide panel that loads entity, renders PropertyList**
- [ ] **Step 2: PropertyList — iterates schema.fields, renders FieldRenderer or FieldEditor per field**
- [ ] **Step 3: CreateEntityModal — dynamic form from schema, renders FieldEditor per required/editable field**
- [ ] **Step 4: Write tests**
- [ ] **Step 5: Commit**

---

## Task 5: Build TableView

**Files:**
- Create: `desktop-ui/src/features/database/components/views/TableView.tsx`

- [ ] **Step 1: Schema-driven DataTable — columns generated from ViewConfig.visibleFields**
- [ ] **Step 2: Each cell rendered via FieldRenderer, click-to-edit via FieldEditor**
- [ ] **Step 3: Column header with sort toggle**
- [ ] **Step 4: Row click opens EntityDetail slide panel**
- [ ] **Step 5: Write tests**
- [ ] **Step 6: Commit**

---

## Task 6: Build BoardView

**Files:**
- Create: `desktop-ui/src/features/database/components/views/BoardView.tsx`

- [ ] **Step 1: Kanban board grouped by ViewConfig.groupBy field (must be select type)**
- [ ] **Step 2: Cards show ViewConfig.cardFields via FieldRenderer**
- [ ] **Step 3: Drag-and-drop between columns via dnd-kit updates the grouping field value**
- [ ] **Step 4: Write tests**
- [ ] **Step 5: Commit**

---

## Task 7: Build CalendarView

**Files:**
- Create: `desktop-ui/src/features/database/components/views/CalendarView.tsx`

- [ ] **Step 1: Monthly/weekly view using ViewConfig.calendarField as the date source**
- [ ] **Step 2: Entities rendered as event cards on their date**
- [ ] **Step 3: Reuse existing calendar components from dashboard feature where possible**
- [ ] **Step 4: Drag to reschedule updates the date field**
- [ ] **Step 5: Commit**

---

## Task 8: Build ListView, GalleryView, TimelineView

**Files:**
- Create: `desktop-ui/src/features/database/components/views/ListView.tsx`
- Create: `desktop-ui/src/features/database/components/views/GalleryView.tsx`
- Create: `desktop-ui/src/features/database/components/views/TimelineView.tsx`

- [ ] **Step 1: ListView — compact list with title + inline card fields, optional grouping**
- [ ] **Step 2: GalleryView — card grid with cover image and configurable card fields**
- [ ] **Step 3: TimelineView — horizontal bars from start/end date fields, grouped by select field**
- [ ] **Step 4: Commit**

---

## Task 9: Build ViewShell, ViewToolbar, ViewTabBar

**Files:**
- Create: `desktop-ui/src/features/database/components/ViewShell.tsx`
- Create: `desktop-ui/src/features/database/components/ViewToolbar.tsx`
- Create: `desktop-ui/src/features/database/components/ViewTabBar.tsx`

- [ ] **Step 1: ViewTabBar — tabs for each view, "+" button to create new view (opens type selector)**
- [ ] **Step 2: ViewToolbar — filter bar, sort dropdown, search input, group-by picker, "New Entity" button**
- [ ] **Step 3: ViewShell — combines TabBar + Toolbar + active view renderer (switches on viewType)**
- [ ] **Step 4: Commit**

---

## Task 10: Build SchemaEditor

**Files:**
- Create: `desktop-ui/src/features/database/components/schema/SchemaEditor.tsx`
- Create: `desktop-ui/src/features/database/components/schema/FieldTypeSelector.tsx`
- Create: `desktop-ui/src/features/database/components/schema/FieldConfigEditor.tsx`

- [ ] **Step 1: SchemaEditor — Notion-style property panel listing all fields with drag-to-reorder**
- [ ] **Step 2: FieldTypeSelector — dropdown of 16 field types with icons**
- [ ] **Step 3: FieldConfigEditor — per-type config (select options, number format, relation target database)**
- [ ] **Step 4: "Add Property" button at bottom**
- [ ] **Step 5: Commit**

---

## Task 11: Build DatabasePage and wire routing

**Files:**
- Create: `desktop-ui/src/features/database/pages/DatabasePage.tsx`
- Modify: `desktop-ui/src/app/router.tsx`

- [ ] **Step 1: DatabasePage — reads databaseId from URL params, loads schema via useDatabase, renders ViewShell**
- [ ] **Step 2: Add `/db/:databaseId` route to router.tsx**
- [ ] **Step 3: Remove `/tasks` and `/finance/*` routes**
- [ ] **Step 4: Commit**

---

## Task 12: Dynamic sidebar navigation

**Files:**
- Modify: `desktop-ui/src/app/layouts/Sidebar.tsx`

- [ ] **Step 1: Replace hardcoded items with dynamic database list from useQuery("db_list")**
- [ ] **Step 2: Keep fixed items (Chat, Dashboard, Notes, Brain, Settings)**
- [ ] **Step 3: Add "New Database" button that opens create dialog**
- [ ] **Step 4: Commit**

---

## Task 13: Build Dashboard builder

**Files:**
- Modify: `desktop-ui/src/features/dashboard/pages/DashboardPage.tsx`
- Create: `desktop-ui/src/features/dashboard/components/DashboardBuilder.tsx`
- Create: `desktop-ui/src/features/dashboard/components/widgets/CountWidget.tsx`
- Create: `desktop-ui/src/features/dashboard/components/widgets/ListWidget.tsx`
- Create: `desktop-ui/src/features/dashboard/components/widgets/ChartBarWidget.tsx`
- Create: `desktop-ui/src/features/dashboard/components/widgets/ChartPieWidget.tsx`
- Create: `desktop-ui/src/features/dashboard/components/widgets/MetricWidget.tsx`
- Create: `desktop-ui/src/features/dashboard/components/widgets/ProgressWidget.tsx`

- [ ] **Step 1: DashboardBuilder — CSS grid layout with drag-and-drop widget placement**
- [ ] **Step 2: Each widget type queries a database with filters and renders the result**
- [ ] **Step 3: CountWidget — single number (filtered count)**
- [ ] **Step 4: ListWidget — filtered entity list**
- [ ] **Step 5: ChartBarWidget / ChartPieWidget — group by a select field, show counts**
- [ ] **Step 6: MetricWidget — aggregation (sum/avg/min/max) of a number field**
- [ ] **Step 7: ProgressWidget — current value vs target**
- [ ] **Step 8: Add "+" button to add widgets, configure database/filters/display**
- [ ] **Step 9: Commit**

---

## Task 14: AI suggestion bar

**Files:**
- Create: `desktop-ui/src/features/database/components/suggestions/SchemaSuggestionBar.tsx`
- Create: `desktop-ui/src/features/database/components/suggestions/SuggestionCard.tsx`
- Create: `desktop-ui/src/features/database/hooks/useSchemaSuggestions.ts`

- [ ] **Step 1: useSchemaSuggestions — useQuery("db_get_suggestions", { databaseId })**
- [ ] **Step 2: SuggestionCard — displays reasoning, accept/dismiss buttons**
- [ ] **Step 3: SchemaSuggestionBar — renders at top of DatabasePage when pending suggestions exist**
- [ ] **Step 4: Commit**

---

## Task 15: Remove old feature UIs

**Files:**
- Delete: `desktop-ui/src/features/tasks/` (entire directory)
- Delete: `desktop-ui/src/features/finance/` (entire directory)
- Delete: `desktop-ui/src/shared/types/tasks.ts`
- Delete: `desktop-ui/src/shared/types/finance.ts`
- Modify: `desktop-ui/src/shared/types/index.ts` (remove task/finance exports)

- [ ] **Step 1: Delete the directories and type files**
- [ ] **Step 2: Fix any remaining imports across the codebase**
- [ ] **Step 3: Run lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Run tests**

Run: `cd desktop-ui && bun run test`

- [ ] **Step 5: Commit**

---

## Task 16: Visual verification

- [ ] **Step 1: Start dev server**

Run: `cd desktop-ui && bun run dev` (in one terminal)
Run: `cargo tauri dev` (in another terminal)

- [ ] **Step 2: Verify database page renders with empty state**
- [ ] **Step 3: Create a database, add fields, add entities — verify table/board/calendar views work**
- [ ] **Step 4: Test dashboard with widgets**
- [ ] **Step 5: Fix any visual issues and commit**
