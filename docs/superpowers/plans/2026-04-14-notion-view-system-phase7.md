# Notion View System — Phase 7: Chart + Feed Views

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two new database view types — `chart` (bar/line/pie aggregations) and `feed` (chronological cards by `updatedAt`) — wired through the existing view-CRUD pipeline.

**Architecture:** Extend `ViewType` on both Rust and TS sides. Add a pure `chartData()` helper that turns entities + an x-axis field + an aggregation into a `[{ x, y }]` series; render via the already-installed `recharts`. `FeedView` is a plain sorted-cards renderer using `formatRelativeTime`. View-specific config (chart type, x-axis, aggregation) lives under `view.config.layout` to avoid widening the strict `ViewConfig` shape for one-off fields.

**Tech Stack:** React + TypeScript, `recharts@^3.7.0` (already installed), Rust `serde` (snake_case enum), existing IPC `db_create_view` / `db_update_view`.

---

## Scope & non-goals

- **In scope:** `chart` (bar / line / pie) with one x-axis field + one aggregation (`count` only for v1; `sum`/`avg` reserved as enum values but not wired). `feed` view sorted by `entity.updatedAt` desc, paged 100 at a time client-side. Picker icons. Sensible defaults on creation.
- **Out of scope:** drill-down, multi-series charts, custom color palettes per slice, filters specific to chart, infinite-scroll/pagination IPC, `sum`/`avg` aggregation runtime.

## File structure

| File | Responsibility |
|---|---|
| `crates/entity-store/src/types.rs` (modify) | Add `Chart` and `Feed` to `ViewType` enum. |
| `desktop-ui/src/shared/types/database.ts` (modify) | Add `"chart" \| "feed"` to `ViewType`. Add `ChartConfig` type stored under `view.config.layout.chart`. |
| `desktop-ui/src/features/database/lib/chartData.ts` (new) | `chartData(entities, schema, config)` → `{ series: {x,y}[], xLabel, yLabel }`. Pure, tested. |
| `desktop-ui/src/features/database/lib/chartData.test.ts` (new) | Vitest. |
| `desktop-ui/src/features/database/components/views/ChartView.tsx` (new) | Wraps recharts BarChart / LineChart / PieChart inside `ResponsiveContainer`. |
| `desktop-ui/src/features/database/components/views/FeedView.tsx` (new) | Sorted entity cards w/ relative timestamp; tap → `onEntityClick`. |
| `desktop-ui/src/features/database/components/views/ViewTypeIcon.tsx` (modify) | Add `chart`, `feed` entries to `ICON_PATHS`, `VIEW_TYPE_LABELS`, `VIEW_TYPES`. |
| `desktop-ui/src/features/database/components/views/ViewConfigPanel.tsx` (modify) | When `view.viewType === "chart"`, render Chart-specific section (chart type, x-axis field, aggregation). |
| `desktop-ui/src/features/database/components/ViewShell.tsx` (modify) | New `chart` and `feed` cases in `ActiveViewRenderer`. |
| `desktop-ui/src/features/database/lib/view-defaults.ts` (modify if exists, else inline in ViewSwitcher) | Default `view.config` when creating chart/feed. |

## Conventions

- Chart config nested under `view.config.layout.chart` to keep the top-level `ViewConfig` shape unchanged. Read with `(view.config.layout?.chart as ChartConfig | undefined)`.
- Rust `ViewType` serialises as snake_case via existing `serde` setup — no extra annotations needed beyond adding the variants.
- Use existing helpers: `formatRelativeTime` (`@shared/lib/dates`), `groupEntities` if useful (probably not — chart wants raw counts, not bucketed entities), `getEntityTitle`, `getTitleField`.
- All values come from the entity field directly. For `select`/`multi_select` x-axis: bucket by each value; for `date`: bucket by ISO day; for any other type: bucket by stringified value, top 12 by count then collapse the rest into "Other".
- Bar / line use `<XAxis dataKey="x" />` + `<YAxis />` + `<Bar dataKey="y" />` / `<Line dataKey="y" />`. Pie uses `<Pie data nameKey="x" dataKey="y" />`.
- Theme via CSS vars: `stroke="var(--brand)"`, `fill="var(--brand)"`, ticks `stroke="var(--muted)"`. No raw hex.

---

## Task 1: Extend Rust `ViewType`

**Files:**
- Modify: `crates/entity-store/src/types.rs` (lines 103–110)

- [ ] **Step 1: Add `Chart` and `Feed` variants**

Open `crates/entity-store/src/types.rs`. Locate the `ViewType` enum (around line 103) and add two variants:

```rust
pub enum ViewType {
    Table,
    Board,
    Calendar,
    List,
    Gallery,
    Timeline,
    Chart,
    Feed,
}
```

Leave any existing `#[derive(...)]` and `#[serde(rename_all = "snake_case")]` attributes intact — those already produce `"chart"` and `"feed"` on the wire.

- [ ] **Step 2: Search for any exhaustive `match` on `ViewType`**

Run from repo root:

```
rg -n "match.*ViewType" crates/
```

For each hit, ensure the new variants either reach a sensible arm or are caught by `_ =>`. If the file uses `_ =>`, no change needed. Otherwise add explicit arms returning the same behavior as `Table` (chart and feed render-side only — they don't change persistence).

- [ ] **Step 3: Build + test**

```
cargo nextest run -p entity-store
```

Expected: all green. If a previously-exhaustive match now fails to compile, fix it per Step 2.

- [ ] **Step 4: No commit yet — wait for full phase per existing project convention.**

---

## Task 2: Extend TS `ViewType` and add `ChartConfig`

**Files:**
- Modify: `desktop-ui/src/shared/types/database.ts`

- [ ] **Step 1: Widen `ViewType`**

Find the `ViewType` declaration (around line 39) and replace with:

```ts
export type ViewType =
  | "table"
  | "board"
  | "calendar"
  | "list"
  | "gallery"
  | "timeline"
  | "chart"
  | "feed";
```

- [ ] **Step 2: Add `ChartConfig`** (after the existing `ViewConfig` interface)

```ts
export type ChartType = "bar" | "line" | "pie";
export type ChartAggregation = "count" | "sum" | "avg";

export interface ChartConfig {
  chartType: ChartType;
  xAxis: string;            // field slug
  aggregation: ChartAggregation;
  yField?: string;          // required when aggregation !== "count"
}
```

No need to widen `ViewConfig` — the chart config nests inside `view.config.layout.chart`.

- [ ] **Step 3: Typecheck**

```
cd desktop-ui && bunx tsc --noEmit
```

Expected: no new errors caused by these additions. (Other pre-existing errors, if any, are out of scope.)

---

## Task 3: `chartData` helper + tests

**Files:**
- Create: `desktop-ui/src/features/database/lib/chartData.ts`
- Create: `desktop-ui/src/features/database/lib/chartData.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// desktop-ui/src/features/database/lib/chartData.test.ts
import { describe, expect, it } from "vitest";
import type { ChartConfig, DatabaseSchema, Entity } from "@shared/types";
import { chartData } from "./chartData";

const schema = {
  id: "db1",
  fields: [
    { id: "f1", slug: "status", name: "Status", fieldType: "select", options: ["todo", "done"] },
    { id: "f2", slug: "tags", name: "Tags", fieldType: "multi_select", options: ["urgent", "home"] },
  ],
  views: [],
} as unknown as DatabaseSchema;

const entities: Entity[] = [
  { id: "a", databaseId: "db1", fields: { status: "todo", tags: ["urgent"] } } as unknown as Entity,
  { id: "b", databaseId: "db1", fields: { status: "todo", tags: ["urgent", "home"] } } as unknown as Entity,
  { id: "c", databaseId: "db1", fields: { status: "done", tags: [] } } as unknown as Entity,
];

describe("chartData", () => {
  it("counts by select field", () => {
    const config: ChartConfig = { chartType: "bar", xAxis: "status", aggregation: "count" };
    const { series, xLabel } = chartData(entities, schema, config);
    expect(xLabel).toBe("Status");
    expect(series).toEqual([
      { x: "todo", y: 2 },
      { x: "done", y: 1 },
    ]);
  });

  it("counts by multi_select fans entities into each value", () => {
    const config: ChartConfig = { chartType: "bar", xAxis: "tags", aggregation: "count" };
    const { series } = chartData(entities, schema, config);
    expect(series.find((p) => p.x === "urgent")?.y).toBe(2);
    expect(series.find((p) => p.x === "home")?.y).toBe(1);
  });

  it("returns empty series and falls back to slug when field missing", () => {
    const config: ChartConfig = { chartType: "bar", xAxis: "missing", aggregation: "count" };
    const { series, xLabel } = chartData(entities, schema, config);
    expect(series).toEqual([]);
    expect(xLabel).toBe("missing");
  });

  it("collapses long tail into Other (top 12 + Other)", () => {
    const many: Entity[] = Array.from({ length: 20 }, (_, i) => ({
      id: `e${i}`,
      databaseId: "db1",
      fields: { status: `s${i}` },
    }) as unknown as Entity);
    const config: ChartConfig = { chartType: "bar", xAxis: "status", aggregation: "count" };
    const { series } = chartData(many, schema, config);
    expect(series.length).toBe(13); // 12 + Other
    expect(series[12]?.x).toBe("Other");
    expect(series[12]?.y).toBe(8);
  });
});
```

- [ ] **Step 2: Run, expect FAIL** (`Cannot find module './chartData'`).

```
cd desktop-ui && bun run test -- chartData
```

- [ ] **Step 3: Implement**

```ts
// desktop-ui/src/features/database/lib/chartData.ts
import type { ChartConfig, DatabaseSchema, Entity, FieldDefinition } from "@shared/types";

export interface ChartPoint {
  x: string;
  y: number;
}

export interface ChartResult {
  series: ChartPoint[];
  xLabel: string;
  yLabel: string;
}

const TOP_N = 12;

function bucketKeys(entity: Entity, field: FieldDefinition): string[] {
  const v = entity.fields[field.slug];
  if (v === null || v === undefined || v === "") return [];
  if (Array.isArray(v)) return v.length === 0 ? [] : v.map(String);
  if (field.fieldType === "date" || field.fieldType === "created_time" || field.fieldType === "last_edited") {
    return [String(v).slice(0, 10)]; // ISO day
  }
  return [String(v)];
}

export function chartData(
  entities: Entity[],
  schema: DatabaseSchema,
  config: ChartConfig,
): ChartResult {
  const field = schema.fields.find((f) => f.slug === config.xAxis);
  const xLabel = field?.name ?? config.xAxis;
  const yLabel = config.aggregation === "count" ? "Count" : (config.yField ?? "Value");
  if (!field) return { series: [], xLabel, yLabel };

  const counts = new Map<string, number>();
  for (const e of entities) {
    for (const key of bucketKeys(e, field)) {
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
  }

  const sorted = [...counts.entries()]
    .map(([x, y]) => ({ x, y }))
    .sort((a, b) => b.y - a.y);

  if (sorted.length <= TOP_N) return { series: sorted, xLabel, yLabel };
  const head = sorted.slice(0, TOP_N);
  const otherY = sorted.slice(TOP_N).reduce((acc, p) => acc + p.y, 0);
  return { series: [...head, { x: "Other", y: otherY }], xLabel, yLabel };
}
```

- [ ] **Step 4: Run tests, expect PASS**

```
cd desktop-ui && bun run test -- chartData
```

Expected: 4/4 passed.

---

## Task 4: `ChartView` component

**Files:**
- Create: `desktop-ui/src/features/database/components/views/ChartView.tsx`

- [ ] **Step 1: Reference the existing recharts pattern**

Open `desktop-ui/src/features/learn/components/RetentionChart.tsx` for the conventions in this repo (CSS variables for stroke/fill, `ResponsiveContainer`, tick font sizes). Match those.

- [ ] **Step 2: Implement**

```tsx
// desktop-ui/src/features/database/components/views/ChartView.tsx
import { chartData } from "@features/database/lib/chartData";
import type { ChartConfig, DatabaseSchema, Entity, ViewDefinition } from "@shared/types";
import { useMemo } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Legend,
  Line,
  LineChart,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

interface Props {
  schema: DatabaseSchema;
  view: ViewDefinition;
  entities: Entity[];
}

const PIE_COLORS = [
  "var(--brand)",
  "var(--accent)",
  "var(--success)",
  "var(--warning)",
  "var(--info)",
  "var(--muted)",
];

function readChartConfig(view: ViewDefinition): ChartConfig | undefined {
  const layout = view.config.layout as { chart?: ChartConfig } | undefined;
  return layout?.chart;
}

export function ChartView({ schema, view, entities }: Props) {
  const config = readChartConfig(view);
  const result = useMemo(
    () => (config ? chartData(entities, schema, config) : null),
    [entities, schema, config],
  );

  if (!config) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-[13px] text-foreground/55">
        Configure this chart in the view settings (chart type, x-axis, aggregation).
      </div>
    );
  }
  if (!result || result.series.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-[13px] text-foreground/55">
        No data to chart.
      </div>
    );
  }

  return (
    <div className="h-full w-full p-4">
      <ResponsiveContainer width="100%" height="100%">
        {config.chartType === "pie" ? (
          <PieChart>
            <Pie data={result.series} dataKey="y" nameKey="x" outerRadius="80%" label>
              {result.series.map((p, i) => (
                <Cell key={p.x} fill={PIE_COLORS[i % PIE_COLORS.length]} />
              ))}
            </Pie>
            <Tooltip />
            <Legend />
          </PieChart>
        ) : config.chartType === "line" ? (
          <LineChart data={result.series}>
            <CartesianGrid stroke="var(--border)" strokeDasharray="3 3" />
            <XAxis dataKey="x" stroke="var(--muted)" tick={{ fontSize: 11 }} />
            <YAxis stroke="var(--muted)" tick={{ fontSize: 11 }} allowDecimals={false} />
            <Tooltip />
            <Line type="monotone" dataKey="y" stroke="var(--brand)" strokeWidth={2} dot />
          </LineChart>
        ) : (
          <BarChart data={result.series}>
            <CartesianGrid stroke="var(--border)" strokeDasharray="3 3" />
            <XAxis dataKey="x" stroke="var(--muted)" tick={{ fontSize: 11 }} />
            <YAxis stroke="var(--muted)" tick={{ fontSize: 11 }} allowDecimals={false} />
            <Tooltip />
            <Bar dataKey="y" fill="var(--brand)" />
          </BarChart>
        )}
      </ResponsiveContainer>
    </div>
  );
}
```

- [ ] **Step 3: Typecheck and lint**

```
cd desktop-ui && bunx tsc --noEmit && bun run lint
```

Expected: clean on the new file. (Pre-existing repo lint warnings are not in scope.)

---

## Task 5: `FeedView` component

**Files:**
- Create: `desktop-ui/src/features/database/components/views/FeedView.tsx`

- [ ] **Step 1: Implement**

```tsx
// desktop-ui/src/features/database/components/views/FeedView.tsx
import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { getEntityTitle, getTitleField } from "@features/database/lib/schema-utils";
import type { DatabaseSchema, Entity, ViewDefinition } from "@shared/types";
import { formatRelativeTime } from "@shared/lib/dates";
import { useMemo } from "react";

interface Props {
  schema: DatabaseSchema;
  view: ViewDefinition;
  entities: Entity[];
  onEntityClick?: (entity: Entity) => void;
}

const PAGE_SIZE = 100;

export function FeedView({ schema, view, entities, onEntityClick }: Props) {
  const titleField = getTitleField(schema);
  const cardFieldSlugs = view.config.cardFields;
  const inlineFields = useMemo(() => {
    if (cardFieldSlugs && cardFieldSlugs.length > 0) {
      return schema.fields.filter((f) => cardFieldSlugs.includes(f.slug));
    }
    return schema.fields.filter((f) => !f.hidden && f !== titleField).slice(0, 3);
  }, [schema, titleField, cardFieldSlugs]);

  const sorted = useMemo(() => {
    return [...entities].sort((a, b) => {
      const ta = Date.parse(a.updatedAt ?? a.createdAt ?? "");
      const tb = Date.parse(b.updatedAt ?? b.createdAt ?? "");
      return tb - ta;
    });
  }, [entities]);

  if (sorted.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-[13px] text-foreground/55">
        No items yet
      </div>
    );
  }

  const visible = sorted.slice(0, PAGE_SIZE);

  return (
    <div className="mx-auto w-full max-w-3xl px-4 py-4">
      <ul className="space-y-2">
        {visible.map((entity) => (
          <li key={entity.id}>
            <button
              type="button"
              onClick={() => onEntityClick?.(entity)}
              className="w-full rounded-lg border border-border bg-surface-base p-3 text-left transition-colors hover:bg-surface-hover"
            >
              <div className="mb-1 flex items-baseline justify-between gap-3">
                <span className="truncate text-[14px] font-semibold text-foreground">
                  {getEntityTitle(schema, entity.fields)}
                </span>
                <span className="shrink-0 text-[11px] text-foreground/55">
                  {formatRelativeTime(entity.updatedAt ?? entity.createdAt ?? "")}
                </span>
              </div>
              <div className="flex flex-wrap gap-3 text-[12px] text-foreground/70">
                {inlineFields.map((field) => (
                  <span key={field.id} className="flex items-center gap-1">
                    <span className="text-foreground/45">{field.name}:</span>
                    <FieldRenderer field={field} value={entity.fields[field.slug]} />
                  </span>
                ))}
              </div>
            </button>
          </li>
        ))}
      </ul>
      {sorted.length > PAGE_SIZE && (
        <p className="mt-4 text-center text-[12px] text-foreground/45">
          Showing {PAGE_SIZE} of {sorted.length} — refine filters to narrow down.
        </p>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Confirm `Entity` exposes `updatedAt`/`createdAt`**

Open `desktop-ui/src/shared/types/database.ts` and verify the `Entity` interface includes both fields. If they're named differently (e.g. `updated_at`), adjust `Date.parse(a.updatedAt ...)` accordingly. Per scoping survey they are camelCase.

- [ ] **Step 3: Typecheck**

```
cd desktop-ui && bunx tsc --noEmit
```

Expected: clean for new file.

---

## Task 6: Picker icons + labels

**Files:**
- Modify: `desktop-ui/src/features/database/components/views/ViewTypeIcon.tsx`

- [ ] **Step 1: Read the file** to confirm the current structure of `ICON_PATHS`, `VIEW_TYPE_LABELS`, and the exported `VIEW_TYPES` array.

- [ ] **Step 2: Add chart + feed entries**

In `ICON_PATHS`, add:

```ts
chart:
  "M3 3v18h18 M7 14v4 M11 9v9 M15 11v7 M19 6v12",
feed:
  "M4 5h16 M4 12h16 M4 19h10",
```

In `VIEW_TYPE_LABELS`, add:

```ts
chart: "Chart",
feed: "Feed",
```

In the exported `VIEW_TYPES` array (the picker grid order), append `"chart"` and `"feed"` at the end.

- [ ] **Step 3: Typecheck**

```
cd desktop-ui && bunx tsc --noEmit
```

Expected: clean. If any `Record<ViewType, ...>` map elsewhere is now incomplete, the compiler will surface it — extend it.

---

## Task 7: View config UI for chart

**Files:**
- Modify: `desktop-ui/src/features/database/components/views/ViewConfigPanel.tsx`

- [ ] **Step 1: Add a Chart section** (only when `view.viewType === "chart"`)

Insert this section after the existing Group by section:

```tsx
{view.viewType === "chart" && (
  <Section label="Chart">
    <ChartConfigEditor
      schema={schema}
      config={(view.config.layout as { chart?: ChartConfig } | undefined)?.chart}
      onChange={(chart) =>
        updateConfig({ layout: { ...(view.config.layout ?? {}), chart } })
      }
    />
  </Section>
)}
```

Add `import type { ChartConfig } from "@shared/types";` at the top.

- [ ] **Step 2: Implement `ChartConfigEditor` (in same file, after `SortEditor`)**

```tsx
interface ChartConfigEditorProps {
  schema: DatabaseSchema;
  config: ChartConfig | undefined;
  onChange: (next: ChartConfig) => void;
}

function ChartConfigEditor({ schema, config, onChange }: ChartConfigEditorProps) {
  const fields = schema.fields.filter((f) => !f.hidden);
  const xAxis = config?.xAxis ?? fields[0]?.slug ?? "";
  const chartType = config?.chartType ?? "bar";
  const aggregation = config?.aggregation ?? "count";
  return (
    <div className="space-y-2">
      <select
        value={chartType}
        onChange={(e) =>
          onChange({ chartType: e.target.value as ChartConfig["chartType"], xAxis, aggregation })
        }
        className="w-full rounded-md border border-border bg-background px-2 py-1 text-[13px] outline-none"
      >
        <option value="bar">Bar</option>
        <option value="line">Line</option>
        <option value="pie">Pie</option>
      </select>
      <select
        value={xAxis}
        onChange={(e) => onChange({ chartType, xAxis: e.target.value, aggregation })}
        className="w-full rounded-md border border-border bg-background px-2 py-1 text-[13px] outline-none"
      >
        {fields.map((f) => (
          <option key={f.slug} value={f.slug}>
            {f.name}
          </option>
        ))}
      </select>
      <select
        value={aggregation}
        onChange={(e) =>
          onChange({
            chartType,
            xAxis,
            aggregation: e.target.value as ChartConfig["aggregation"],
          })
        }
        className="w-full rounded-md border border-border bg-background px-2 py-1 text-[13px] outline-none"
      >
        <option value="count">Count</option>
        <option value="sum" disabled>Sum (coming soon)</option>
        <option value="avg" disabled>Average (coming soon)</option>
      </select>
    </div>
  );
}
```

- [ ] **Step 3: Typecheck and quick smoke**

```
cd desktop-ui && bunx tsc --noEmit
```

Expected: clean.

---

## Task 8: Wire into `ActiveViewRenderer` + creation defaults

**Files:**
- Modify: `desktop-ui/src/features/database/components/ViewShell.tsx`
- Modify: `desktop-ui/src/features/database/components/views/ViewSwitcher.tsx` (creation defaults — only if it's where `db_create_view` is called)

- [ ] **Step 1: Add chart + feed cases in `ActiveViewRenderer`**

In `ViewShell.tsx`, inside the `switch (view.viewType)` of `ActiveViewRenderer`, add:

```tsx
case "chart":
  return <ChartView schema={schema} view={view} entities={entities} />;
case "feed":
  return (
    <FeedView
      schema={schema}
      view={view}
      entities={entities}
      onEntityClick={onEntityClick}
    />
  );
```

Add the imports:

```tsx
import { ChartView } from "./views/ChartView";
import { FeedView } from "./views/FeedView";
```

- [ ] **Step 2: Sensible default config when creating a chart**

Locate where `db_create_view` is invoked when the user picks a type from `ViewTypePicker`. (Search: `rg "db_create_view\|useCreateView" desktop-ui/src/features/database`.) When the chosen `viewType === "chart"`, default config should be:

```ts
{
  layout: {
    chart: {
      chartType: "bar",
      xAxis: schema.fields.find((f) => !f.hidden && (f.fieldType === "select" || f.fieldType === "multi_select"))?.slug
        ?? schema.fields.find((f) => !f.hidden)?.slug
        ?? "",
      aggregation: "count",
    },
  },
}
```

Feed needs no defaults beyond the empty `{}`.

If the creation site is in `ViewSwitcher.tsx`, modify the call there. If it's in a small `view-defaults.ts` helper, modify that.

- [ ] **Step 3: Typecheck + smoke**

```
cd desktop-ui && bunx tsc --noEmit && bun run test
```

Expected: all tests pass (existing 70 + 4 new chart tests = 74).

- [ ] **Step 4: Browser smoke**

Start `cd desktop-ui && bun run dev` and `cargo tauri dev`. On any database:
1. Click the `+ New view` button → confirm Chart and Feed appear in the picker with icons.
2. Create a Chart view → switch to it → confirm it renders a bar chart of counts by the auto-selected field.
3. Open Properties → switch chart type to Pie → renders pie. Switch to Line → renders line.
4. Change x-axis field → chart re-renders.
5. Create a Feed view → entities sorted newest-first with relative timestamps ("3m", "2h", "1d").
6. Click an entity in Feed → opens detail panel.

---

## Self-review checklist

- [ ] Rust `ViewType` extended; `cargo nextest run -p entity-store` clean.
- [ ] TS `ViewType` widened; `ChartConfig` type exported.
- [ ] `chartData()` covered by 4 unit tests including "Other" collapse.
- [ ] No raw hex; all colors via CSS vars (`var(--brand)`, `var(--muted)`, etc.).
- [ ] No widening of `ViewConfig` for chart-only fields (nested under `layout.chart`).
- [ ] Picker shows Chart + Feed.
- [ ] `ActiveViewRenderer` has both new cases.
- [ ] Feed sorts by `entity.updatedAt`, falls back to `createdAt`.
- [ ] `formatRelativeTime` reused (not reimplemented).
- [ ] Test counts: existing 70 + 4 chart = 74 — still passing.
- [ ] No commits during execution per project convention.

## Verification

```
cargo nextest run -p entity-store
cd desktop-ui && bunx tsc --noEmit
cd desktop-ui && bun run test
cd desktop-ui && bun run lint
```

Manual matrix: see Task 8 Step 4.
