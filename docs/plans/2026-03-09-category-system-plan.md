# Category System Improvements — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve category colors, group categories by type (Work/Utilities/Distraction) with nested donut chart, and add a Category Manager page for viewing/editing all app & domain classifications.

**Architecture:** Enhance existing `CategoryUsage` to include `categoryId` and `categoryType` so the frontend can group without extra lookups. Add new backend commands for tracked-app queries and category CRUD with rules. Frontend gets redesigned CategoriesList with grouped layout + nested donuts, and a new Category Manager page as a tab in the Productivity section.

**Tech Stack:** Rust (feature-productivity, app-core, desktop-shared, desktop), React + Recharts + Tailwind v4 + Lucide icons

---

## Batch 1: Data Layer & Color System (Tasks 1–4)

### Task 1: Enhance CategoryUsage with type info

Add `category_id` and `category_type` fields to `CategoryUsage` (Rust) and `CategoryUsageResponse` (shared) so the frontend can group by type without a second API call.

**Files:**
- Modify: `crates/feature-productivity/src/types.rs:187-192`
- Modify: `crates/desktop-shared/src/commands.rs:681-686`
- Modify: `crates/feature-productivity/src/aggregator.rs:109-112`
- Modify: `crates/app-core/src/handlers/productivity.rs` (summary_to_response)

**Step 1: Add fields to `CategoryUsage` in types.rs**

```rust
// crates/feature-productivity/src/types.rs — replace lines 187-192
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryUsage {
    pub category_id: String,
    pub category: String,
    pub category_type: String,
    pub duration_secs: i64,
}
```

**Step 2: Update `CategoryUsageResponse` in desktop-shared**

```rust
// crates/desktop-shared/src/commands.rs — replace lines 681-686
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryUsageResponse {
    pub category_id: String,
    pub category: String,
    pub category_type: String,
    pub duration_secs: i64,
}
```

**Step 3: Update aggregator to populate new fields**

In `crates/feature-productivity/src/aggregator.rs`, the `top_categories.push` block (~line 109) becomes:

```rust
top_categories.push(CategoryUsage {
    category_id: cat.id.clone(),
    category: cat.name.clone(),
    category_type: cat.category_type.to_string(),
    duration_secs: *secs,
});
```

**Step 4: Update `summary_to_response` in app-core**

In `crates/app-core/src/handlers/productivity.rs`, update the `top_categories` mapping inside `summary_to_response` to include new fields:

```rust
top_categories: s.top_categories.into_iter().map(|c| CategoryUsageResponse {
    category_id: c.category_id,
    category: c.category,
    category_type: c.category_type,
    duration_secs: c.duration_secs,
}).collect(),
```

**Step 5: Update TypeScript types**

In `desktop-ui/src/lib/types.ts`, update `CategoryUsage`:

```typescript
export interface CategoryUsage {
  categoryId: string;
  category: string;
  categoryType: "productive" | "neutral" | "distracting";
  durationSecs: number;
}
```

**Step 6: Verify build**

Run: `cargo build --workspace 2>&1 | tail -5` — fix any compilation errors from the new fields.
Run: `cd desktop-ui && bun run build 2>&1 | tail -5`

---

### Task 2: New unique category colors in shared.tsx

Replace the `CATEGORY_COLORS` map with unique hex colors per category, and add a `TYPE_BADGE_COLORS` map and `getCategoryTypeColor` helper.

**Files:**
- Modify: `desktop-ui/src/components/productivity/shared.tsx:424-463`

**Step 1: Replace `CATEGORY_COLORS` and helpers**

Replace lines 424–463 in `shared.tsx` with:

```typescript
// ── Category colors ───────────────────────────────────────────────────

/** Unique color per category — visually distinct on dark backgrounds. */
const CATEGORY_COLORS: Record<string, string> = {
  coding: "#22C55E",
  design: "#A78BFA",
  communication: "#F59E0B",
  entertainment: "#F87171",
  project_management: "#8B5CF6",
  documentation: "#60A5FA",
  email: "#78716C",
  browsing: "#94A3B8",
  ai_tools: "#06B6D4",
  social_media: "#F43F5E",
  video_streaming: "#EF4444",
  news_forums: "#FB923C",
  developer_tools: "#10B981",
  cloud_devops: "#34D399",
  shopping: "#FB7185",
  finance: "#A1A1AA",
  learning: "#2DD4BF",
  music: "#C084FC",
  gaming: "#E11D48",
};

const FALLBACK_COLORS = [
  "#60A5FA", "#A78BFA", "#F59E0B", "#22C55E", "#94A3B8", "#F43F5E",
];

/** Type badge colors: productive (green), neutral (slate), distracting (rose). */
export const TYPE_BADGE_COLORS: Record<string, string> = {
  productive: "#22C55E",
  neutral: "#94A3B8",
  distracting: "#F43F5E",
};

/**
 * Resolve a category color from either an ID ("coding") or display name ("Coding").
 * Falls back to a rotating palette, then to slate.
 */
export function getCategoryColor(nameOrId: string, index = 0): string {
  const key = nameOrId.toLowerCase().replace(/[ &]/g, "_");
  return CATEGORY_COLORS[key] ?? FALLBACK_COLORS[index % FALLBACK_COLORS.length];
}

/** Get the type badge color for a category type. */
export function getCategoryTypeColor(categoryType: string): string {
  return TYPE_BADGE_COLORS[categoryType] ?? TYPE_BADGE_COLORS.neutral;
}
```

**Step 2: Verify build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`

---

### Task 3: Redesign CategoriesList — grouped layout with nested donut

Rewrite the `CategoriesList` component to group categories by Work/Utilities/Distraction, show group totals, and render a nested donut chart.

**Files:**
- Modify: `desktop-ui/src/components/productivity/CategoriesList.tsx` (full rewrite)

**Step 1: Rewrite CategoriesList**

```tsx
import { useMemo } from "react";
import { Cell, Pie, PieChart } from "recharts";
import { formatHumanDuration } from "../../lib/dates";
import type { CategoryUsage } from "../../lib/types";
import { getCategoryColor, getCategoryTypeColor } from "./shared";

interface CategoriesListProps {
  categories: CategoryUsage[];
  totalSecs: number;
}

interface CategoryGroup {
  label: string;
  type: string;
  color: string;
  categories: CategoryUsage[];
  totalSecs: number;
}

const GROUP_CONFIG: { type: string; label: string }[] = [
  { type: "productive", label: "Work" },
  { type: "neutral", label: "Utilities" },
  { type: "distracting", label: "Distraction" },
];

export function CategoriesList({ categories, totalSecs }: CategoriesListProps) {
  const active = useMemo(() => categories.filter((c) => c.durationSecs > 0), [categories]);

  const groups = useMemo<CategoryGroup[]>(() => {
    return GROUP_CONFIG.map((g) => {
      const cats = active.filter((c) => c.categoryType === g.type);
      return {
        label: g.label,
        type: g.type,
        color: getCategoryTypeColor(g.type),
        categories: cats,
        totalSecs: cats.reduce((sum, c) => sum + c.durationSecs, 0),
      };
    }).filter((g) => g.totalSecs > 0);
  }, [active]);

  // Inner ring: individual categories
  const innerData = useMemo(
    () =>
      active.map((cat, i) => ({
        name: cat.category,
        value: cat.durationSecs,
        color: getCategoryColor(cat.categoryId, i),
      })),
    [active],
  );

  // Outer ring: type groups
  const outerData = useMemo(
    () =>
      groups.map((g) => ({
        name: g.label,
        value: g.totalSecs,
        color: g.color,
      })),
    [groups],
  );

  if (active.length === 0) {
    return (
      <div className="glass-card p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Categories</h2>
        <p className="text-[12px] font-light text-dim">No category data</p>
      </div>
    );
  }

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Categories</h2>
        <span className="text-[10px] font-light text-dim tabular-nums">
          {formatHumanDuration(totalSecs)} tracked
        </span>
      </div>

      {/* Nested donut: outer = type groups, inner = categories */}
      <div className="flex justify-center">
        <PieChart width={100} height={100}>
          {/* Inner ring — categories */}
          <Pie
            data={innerData}
            cx={49}
            cy={49}
            innerRadius={20}
            outerRadius={32}
            startAngle={90}
            endAngle={-270}
            dataKey="value"
            stroke="none"
            paddingAngle={1}
          >
            {innerData.map((entry) => (
              <Cell key={entry.name} fill={entry.color} />
            ))}
          </Pie>
          {/* Outer ring — type groups */}
          <Pie
            data={outerData}
            cx={49}
            cy={49}
            innerRadius={35}
            outerRadius={46}
            startAngle={90}
            endAngle={-270}
            dataKey="value"
            stroke="none"
            paddingAngle={2}
          >
            {outerData.map((entry) => (
              <Cell key={entry.name} fill={entry.color} />
            ))}
          </Pie>
        </PieChart>
      </div>

      {/* Grouped legend */}
      <div className="flex flex-col gap-3">
        {groups.map((group) => {
          const groupPct = totalSecs > 0 ? Math.round((group.totalSecs / totalSecs) * 100) : 0;
          return (
            <div key={group.type} className="flex flex-col gap-1">
              {/* Group header */}
              <div className="flex items-center gap-2">
                <span
                  className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                  style={{ backgroundColor: group.color }}
                />
                <span className="text-[11px] font-medium text-secondary flex-1">
                  {group.label}
                </span>
                <span className="text-[10px] font-medium text-secondary tabular-nums">
                  {groupPct}%
                </span>
                <span className="text-[10px] font-light text-dim tabular-nums w-14 text-right">
                  {formatHumanDuration(group.totalSecs)}
                </span>
              </div>
              {/* Category rows */}
              {group.categories.map((cat, i) => {
                const pct =
                  totalSecs > 0 ? Math.round((cat.durationSecs / totalSecs) * 100) : 0;
                return (
                  <div key={cat.categoryId} className="flex items-center gap-2 pl-3.5">
                    <span
                      className="w-2 h-2 rounded-sm flex-shrink-0"
                      style={{ backgroundColor: getCategoryColor(cat.categoryId, i) }}
                    />
                    <span className="text-[11px] font-light text-primary flex-1 truncate">
                      {cat.category}
                    </span>
                    <span className="text-[10px] font-light text-dim tabular-nums">
                      {pct}%
                    </span>
                    <span className="text-[10px] font-light text-dim tabular-nums w-14 text-right">
                      {formatHumanDuration(cat.durationSecs)}
                    </span>
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

**Step 2: Verify build + lint**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Run: `cd desktop-ui && bun run lint:fix 2>&1 | tail -10`

---

### Task 4: Add `ActivityCategoryResponse` rules field + enhance category_upsert

The existing `ActivityCategoryResponse` is missing `rules`. Add it so the Category Manager can display/edit rules. Also update the existing `productivity_category_upsert` to accept rules.

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs:748-757` — add `rules` field
- Modify: `crates/app-core/src/handlers/productivity.rs:222-236` — include rules in response
- Modify: `crates/app-core/src/handlers/productivity.rs:465-499` — accept rules in upsert
- Modify: `desktop-ui/src/lib/types.ts` — update `ActivityCategory` TS type

**Step 1: Add `rules` to `ActivityCategoryResponse`**

```rust
// crates/desktop-shared/src/commands.rs — replace ActivityCategoryResponse
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCategoryResponse {
    pub id: String,
    pub name: String,
    pub category_type: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub is_system: bool,
    pub rules: Option<CategoryRulesResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRulesResponse {
    pub app_names: Vec<String>,
    pub bundle_ids: Vec<String>,
    pub url_patterns: Vec<String>,
}
```

**Step 2: Update `productivity_categories` in app-core to include rules**

In the `productivity_categories` method, map the rules:

```rust
.map(|c| ActivityCategoryResponse {
    id: c.id,
    name: c.name,
    category_type: c.category_type.to_string(),
    color: c.color,
    icon: c.icon,
    is_system: c.is_system,
    rules: c.rules.map(|r| CategoryRulesResponse {
        app_names: r.app_names,
        bundle_ids: r.bundle_ids,
        url_patterns: r.url_patterns,
    }),
})
```

**Step 3: Update `productivity_category_upsert` to accept rules**

```rust
pub async fn productivity_category_upsert(
    &self,
    id: String,
    name: String,
    category_type: String,
    color: Option<String>,
    icon: Option<String>,
    rules: Option<CategoryRulesResponse>,
) -> Result<ActivityCategoryResponse, ApiError> {
    let repos = self.productivity_repos()?;
    let ct: feature_productivity::types::CategoryType =
        category_type.parse().map_err(|_| {
            ApiError::new("VALIDATION", "Invalid category_type. Use: productive, neutral, distracting")
        })?;
    let cat_rules = rules.as_ref().map(|r| feature_productivity::types::CategoryRules {
        app_names: r.app_names.clone(),
        bundle_ids: r.bundle_ids.clone(),
        url_patterns: r.url_patterns.clone(),
    });
    let cat = feature_productivity::types::ActivityCategory {
        id,
        name,
        category_type: ct,
        color,
        icon,
        rules: cat_rules,
        is_system: false,
    };
    repos.categories.upsert(&cat).await.map_err(map_prod_err)?;
    Ok(ActivityCategoryResponse {
        id: cat.id,
        name: cat.name,
        category_type: cat.category_type.to_string(),
        color: cat.color,
        icon: cat.icon,
        is_system: false,
        rules: cat.rules.map(|r| CategoryRulesResponse {
            app_names: r.app_names,
            bundle_ids: r.bundle_ids,
            url_patterns: r.url_patterns,
        }),
    })
}
```

**Step 4: Update the Tauri command adapter** in `crates/desktop/src/commands/productivity.rs` to pass the `rules` param through.

**Step 5: Update TypeScript types**

```typescript
export interface CategoryRules {
  appNames: string[];
  bundleIds: string[];
  urlPatterns: string[];
}

export interface ActivityCategory {
  id: string;
  name: string;
  categoryType: string;
  color: string | null;
  icon: string | null;
  isSystem: boolean;
  rules: CategoryRules | null;
}
```

**Step 6: Verify build**

Run: `cargo build --workspace 2>&1 | tail -5`
Run: `cd desktop-ui && bun run build 2>&1 | tail -5`

---

## Batch 2: Backend — Tracked Apps Query & Category Delete (Tasks 5–6)

### Task 5: Add `productivity_tracked_apps` command

New command returning all distinct apps/domains from activity_events with their current category and total tracked time.

**Files:**
- Modify: `crates/feature-productivity/src/repos/activity_event.rs` — add `tracked_apps` query
- Modify: `crates/desktop-shared/src/commands.rs` — add `TrackedAppResponse`
- Modify: `crates/app-core/src/handlers/productivity.rs` — add handler
- Modify: `crates/desktop/src/commands/productivity.rs` — add Tauri command + dev dispatch
- Modify: `desktop-ui/src/lib/types.ts` — add TS type

**Step 1: Add repo query**

In `crates/feature-productivity/src/repos/activity_event.rs`, add:

```rust
/// Returns all distinct app/site combinations with their category and total duration.
pub async fn tracked_apps(&self) -> common::Result<Vec<TrackedAppRow>> {
    let rows = sqlx::query_as::<_, TrackedAppRow>(
        r#"SELECT
               COALESCE(site_name, app_name) AS display_name,
               app_name,
               site_name,
               category_id,
               COALESCE(SUM(duration_secs), 0) AS total_secs,
               COUNT(*) AS event_count
           FROM activity_events
           WHERE is_idle = FALSE
           GROUP BY display_name
           ORDER BY total_secs DESC"#,
    )
    .fetch_all(&self.pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    Ok(rows)
}
```

Add the row struct:

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TrackedAppRow {
    pub display_name: String,
    pub app_name: String,
    pub site_name: Option<String>,
    pub category_id: Option<String>,
    pub total_secs: i64,
    pub event_count: i64,
}
```

**Step 2: Add `TrackedAppResponse` in desktop-shared**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedAppResponse {
    pub display_name: String,
    pub app_name: String,
    pub site_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub total_secs: i64,
    pub event_count: i64,
}
```

**Step 3: Add AppCore handler**

```rust
pub async fn productivity_tracked_apps(&self) -> Result<Vec<TrackedAppResponse>, ApiError> {
    let repos = self.productivity_repos()?;
    let rows = repos.events.tracked_apps().await.map_err(map_prod_err)?;
    let categories = repos.categories.list_all().await.map_err(map_prod_err)?;
    let cat_map: std::collections::HashMap<&str, &str> = categories
        .iter()
        .map(|c| (c.id.as_str(), c.name.as_str()))
        .collect();
    Ok(rows.into_iter().map(|r| {
        let cat_name = r.category_id.as_deref().and_then(|id| cat_map.get(id).copied()).map(String::from);
        TrackedAppResponse {
            display_name: r.display_name,
            app_name: r.app_name,
            site_name: r.site_name,
            category_id: r.category_id,
            category_name: cat_name,
            total_secs: r.total_secs,
            event_count: r.event_count,
        }
    }).collect())
}
```

**Step 4: Add Tauri command + dev dispatch**

Tauri command:
```rust
#[tauri::command]
pub async fn productivity_tracked_apps(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<TrackedAppResponse>, ApiError> {
    state.productivity_tracked_apps().await
}
```

Add `"productivity_tracked_apps"` to `DEV_COMMANDS` and `dispatch_dev`.

**Step 5: Add TypeScript type**

```typescript
export interface TrackedApp {
  displayName: string;
  appName: string;
  siteName: string | null;
  categoryId: string | null;
  categoryName: string | null;
  totalSecs: number;
  eventCount: number;
}
```

**Step 6: Verify build**

Run: `cargo build --workspace 2>&1 | tail -5`

---

### Task 6: Add `productivity_category_delete` command

**Files:**
- Modify: `crates/desktop/src/commands/productivity.rs` — add Tauri command
- Modify: `crates/app-core/src/handlers/productivity.rs` — add handler
- Add dev dispatch entry

**Step 1: Add AppCore handler**

```rust
pub async fn productivity_category_delete(&self, id: String) -> Result<bool, ApiError> {
    let repos = self.productivity_repos()?;
    let deleted = repos.categories.delete(&id).await.map_err(map_prod_err)?;
    Ok(deleted)
}
```

**Step 2: Add Tauri command**

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn productivity_category_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    state.productivity_category_delete(id).await
}
```

**Step 3: Wire into command list and dev dispatch**

Add `"productivity_category_delete"` to `DEV_COMMANDS` and `dispatch_dev`.

**Step 4: Verify build**

Run: `cargo build --workspace 2>&1 | tail -5`

---

## Batch 3: Frontend — Category Manager Page (Tasks 7–9)

### Task 7: Create Category Manager route and page shell

Add a new route `/productivity/categories` and create the three-panel page layout.

**Files:**
- Create: `desktop-ui/src/components/productivity/pages/CategoriesPage.tsx`
- Modify: `desktop-ui/src/App.tsx` — add route + lazy import

**Step 1: Create CategoriesPage with three-panel layout**

Create `desktop-ui/src/components/productivity/pages/CategoriesPage.tsx`:

```tsx
import { useState } from "react";
import { useQuery } from "../../../hooks/useQuery";
import type { ActivityCategory, TrackedApp } from "../../../lib/types";
import { CategoryEditor } from "../CategoryEditor";
import { CategoryList } from "../CategoryList";
import { TrackedAppsList } from "../TrackedAppsList";

export function CategoriesPage() {
  const { data: categories, refetch: refetchCategories } = useQuery<ActivityCategory[]>(
    "productivity_categories",
    undefined,
    [],
  );
  const { data: trackedApps, refetch: refetchApps } = useQuery<TrackedApp[]>(
    "productivity_tracked_apps",
    undefined,
    [],
  );

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected = categories.find((c) => c.id === selectedId) ?? null;

  const refresh = () => {
    refetchCategories();
    refetchApps();
  };

  return (
    <div className="flex gap-4 h-full min-h-0">
      {/* Panel A: Category list */}
      <div className="w-56 flex-shrink-0 overflow-y-auto">
        <CategoryList
          categories={categories}
          selectedId={selectedId}
          onSelect={setSelectedId}
          onCreated={refresh}
        />
      </div>

      {/* Panel B: Category editor */}
      <div className="flex-1 min-w-0 overflow-y-auto">
        <CategoryEditor
          category={selected}
          onSaved={refresh}
          onDeleted={() => {
            setSelectedId(null);
            refresh();
          }}
        />
      </div>

      {/* Panel C: Tracked apps */}
      <div className="w-72 flex-shrink-0 overflow-y-auto">
        <TrackedAppsList
          apps={trackedApps}
          categories={categories}
          onReassigned={refresh}
        />
      </div>
    </div>
  );
}
```

**Step 2: Add route in App.tsx**

Add lazy import:
```typescript
const CategoriesPage = lazy(() =>
  import("./components/productivity/pages/CategoriesPage").then((m) => ({
    default: m.CategoriesPage,
  })),
);
```

Add route after the month route:
```typescript
{ path: "/productivity/categories", element: <CategoriesPage /> },
```

**Step 3: Add "Categories" tab to ProductivityLayout**

In `desktop-ui/src/components/productivity/ProductivityLayout.tsx`, add a "Categories" entry to the periods array — but since categories isn't a time period, add it as a separate nav link:

```tsx
// After the period tabs, add:
<button
  type="button"
  onClick={() => navigate("/productivity/categories")}
  className={`px-3 py-1.5 rounded-xl text-[13px] font-light transition-all duration-200 ${
    pathname.includes("/categories")
      ? "glass-button-active text-primary"
      : "text-muted hover:text-secondary hover:bg-white/[0.04]"
  }`}
>
  Categories
</button>
```

This requires accessing `useLocation()` in the component. Add import and `const { pathname } = useLocation()`.

**Step 4: Verify build** (will fail on missing components — that's expected, we create them next)

---

### Task 8: Create CategoryList panel (Panel A)

The left sidebar showing all categories grouped by type.

**Files:**
- Create: `desktop-ui/src/components/productivity/CategoryList.tsx`

**Step 1: Create CategoryList component**

```tsx
import { Plus } from "lucide-react";
import { useMemo } from "react";
import { useMutation } from "../../hooks/useMutation";
import type { ActivityCategory } from "../../lib/types";
import { getCategoryColor, getCategoryTypeColor } from "./shared";

interface CategoryListProps {
  categories: ActivityCategory[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreated: () => void;
}

const TYPE_GROUPS: { type: string; label: string }[] = [
  { type: "productive", label: "Work" },
  { type: "neutral", label: "Utilities" },
  { type: "distracting", label: "Distraction" },
];

export function CategoryList({ categories, selectedId, onSelect, onCreated }: CategoryListProps) {
  const groups = useMemo(
    () =>
      TYPE_GROUPS.map((g) => ({
        ...g,
        color: getCategoryTypeColor(g.type),
        items: categories.filter((c) => c.categoryType === g.type),
      })).filter((g) => g.items.length > 0),
    [categories],
  );

  const createMut = useMutation("productivity_category_upsert");

  const handleCreate = async () => {
    const id = `custom_${Date.now()}`;
    await createMut.mutate({
      id,
      name: "New Category",
      category_type: "neutral",
      color: "#94A3B8",
      icon: null,
      rules: { app_names: [], bundle_ids: [], url_patterns: [] },
    });
    onCreated();
    onSelect(id);
  };

  return (
    <div className="glass-card p-3 flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h3 className="text-[12px] font-medium text-secondary">Categories</h3>
        <button
          type="button"
          onClick={handleCreate}
          className="p-1 rounded-md hover:bg-white/[0.06] text-muted hover:text-primary transition-colors"
          title="Add category"
        >
          <Plus size={14} />
        </button>
      </div>

      {groups.map((group) => (
        <div key={group.type} className="flex flex-col gap-0.5">
          <div className="flex items-center gap-1.5 py-1">
            <span
              className="w-1.5 h-1.5 rounded-full"
              style={{ backgroundColor: group.color }}
            />
            <span className="text-[10px] font-medium text-muted uppercase tracking-wider">
              {group.label}
            </span>
          </div>
          {group.items.map((cat) => {
            const isSelected = cat.id === selectedId;
            return (
              <button
                type="button"
                key={cat.id}
                onClick={() => onSelect(cat.id)}
                className={`flex items-center gap-2 px-2 py-1.5 rounded-lg text-left transition-colors ${
                  isSelected
                    ? "bg-white/[0.08] text-primary"
                    : "text-secondary hover:bg-white/[0.04] hover:text-primary"
                }`}
              >
                <span
                  className="w-2.5 h-2.5 rounded-sm flex-shrink-0"
                  style={{ backgroundColor: getCategoryColor(cat.id) }}
                />
                <span className="text-[11px] font-light truncate">{cat.name}</span>
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}
```

---

### Task 9: Create CategoryEditor panel (Panel B)

The center editor for name, type, color, and rules.

**Files:**
- Create: `desktop-ui/src/components/productivity/CategoryEditor.tsx`

**Step 1: Create CategoryEditor component**

```tsx
import { Palette, Save, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useMutation } from "../../hooks/useMutation";
import type { ActivityCategory } from "../../lib/types";
import { getCategoryTypeColor } from "./shared";

interface CategoryEditorProps {
  category: ActivityCategory | null;
  onSaved: () => void;
  onDeleted: () => void;
}

const COLOR_SWATCHES = [
  "#22C55E", "#10B981", "#06B6D4", "#2DD4BF", "#34D399",
  "#60A5FA", "#8B5CF6", "#A78BFA", "#C084FC",
  "#F59E0B", "#FB923C", "#94A3B8", "#78716C", "#A1A1AA",
  "#F43F5E", "#EF4444", "#E11D48", "#FB7185", "#F87171",
];

const TYPE_OPTIONS = [
  { value: "productive", label: "Work (Productive)" },
  { value: "neutral", label: "Utilities (Neutral)" },
  { value: "distracting", label: "Distraction (Distracting)" },
];

export function CategoryEditor({ category, onSaved, onDeleted }: CategoryEditorProps) {
  const [name, setName] = useState("");
  const [type, setType] = useState("neutral");
  const [color, setColor] = useState("#94A3B8");
  const [appNames, setAppNames] = useState<string[]>([]);
  const [urlPatterns, setUrlPatterns] = useState<string[]>([]);
  const [newApp, setNewApp] = useState("");
  const [newUrl, setNewUrl] = useState("");

  const saveMut = useMutation("productivity_category_upsert");
  const deleteMut = useMutation("productivity_category_delete");

  useEffect(() => {
    if (category) {
      setName(category.name);
      setType(category.categoryType);
      setColor(category.color ?? "#94A3B8");
      setAppNames(category.rules?.appNames ?? []);
      setUrlPatterns(category.rules?.urlPatterns ?? []);
    }
  }, [category]);

  if (!category) {
    return (
      <div className="glass-card p-6 flex items-center justify-center h-full">
        <p className="text-[13px] font-light text-dim">Select a category to edit</p>
      </div>
    );
  }

  const handleSave = async () => {
    await saveMut.mutate({
      id: category.id,
      name,
      category_type: type,
      color,
      icon: null,
      rules: {
        app_names: appNames,
        bundle_ids: category.rules?.bundleIds ?? [],
        url_patterns: urlPatterns,
      },
    });
    onSaved();
  };

  const handleDelete = async () => {
    await deleteMut.mutate({ id: category.id });
    onDeleted();
  };

  const addApp = () => {
    const trimmed = newApp.trim();
    if (trimmed && !appNames.includes(trimmed)) {
      setAppNames([...appNames, trimmed]);
      setNewApp("");
    }
  };

  const addUrl = () => {
    const trimmed = newUrl.trim();
    if (trimmed && !urlPatterns.includes(trimmed)) {
      setUrlPatterns([...urlPatterns, trimmed]);
      setNewUrl("");
    }
  };

  return (
    <div className="glass-card p-4 flex flex-col gap-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span
            className="w-3 h-3 rounded-sm"
            style={{ backgroundColor: color }}
          />
          <h3 className="text-[13px] font-medium text-secondary">Edit Category</h3>
          {category.isSystem && (
            <span className="text-[9px] font-light text-dim bg-white/[0.06] px-1.5 py-0.5 rounded">
              System
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={handleSave}
            className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-brand/20 text-brand text-[11px] font-medium hover:bg-brand/30 transition-colors"
          >
            <Save size={12} />
            Save
          </button>
          {!category.isSystem && (
            <button
              type="button"
              onClick={handleDelete}
              className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-destructive/70 text-[11px] hover:bg-destructive/10 transition-colors"
            >
              <Trash2 size={12} />
            </button>
          )}
        </div>
      </div>

      {/* Name */}
      <div className="flex flex-col gap-1">
        <label className="text-[10px] font-medium text-muted uppercase tracking-wider">Name</label>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="glass-input px-3 py-1.5 text-[12px] rounded-lg"
        />
      </div>

      {/* Type */}
      <div className="flex flex-col gap-1">
        <label className="text-[10px] font-medium text-muted uppercase tracking-wider">Type</label>
        <div className="flex gap-1.5">
          {TYPE_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => setType(opt.value)}
              className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[11px] font-light transition-colors ${
                type === opt.value
                  ? "bg-white/[0.08] text-primary"
                  : "text-muted hover:bg-white/[0.04]"
              }`}
            >
              <span
                className="w-1.5 h-1.5 rounded-full"
                style={{ backgroundColor: getCategoryTypeColor(opt.value) }}
              />
              {opt.label}
            </button>
          ))}
        </div>
      </div>

      {/* Color */}
      <div className="flex flex-col gap-1.5">
        <label className="text-[10px] font-medium text-muted uppercase tracking-wider flex items-center gap-1">
          <Palette size={10} />
          Color
        </label>
        <div className="flex flex-wrap gap-1.5">
          {COLOR_SWATCHES.map((swatch) => (
            <button
              key={swatch}
              type="button"
              onClick={() => setColor(swatch)}
              className={`w-5 h-5 rounded-md transition-all ${
                color === swatch ? "ring-2 ring-white/40 scale-110" : "hover:scale-105"
              }`}
              style={{ backgroundColor: swatch }}
            />
          ))}
        </div>
      </div>

      {/* App Names */}
      <div className="flex flex-col gap-1.5">
        <label className="text-[10px] font-medium text-muted uppercase tracking-wider">
          App Names
        </label>
        <div className="flex flex-wrap gap-1">
          {appNames.map((app) => (
            <span
              key={app}
              className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-white/[0.06] text-[11px] font-light text-secondary"
            >
              {app}
              <button
                type="button"
                onClick={() => setAppNames(appNames.filter((a) => a !== app))}
                className="text-muted hover:text-destructive"
              >
                <X size={10} />
              </button>
            </span>
          ))}
        </div>
        <div className="flex gap-1">
          <input
            type="text"
            value={newApp}
            onChange={(e) => setNewApp(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addApp()}
            placeholder="Add app name..."
            className="glass-input flex-1 px-2.5 py-1 text-[11px] rounded-lg"
          />
          <button
            type="button"
            onClick={addApp}
            className="px-2 py-1 rounded-lg bg-white/[0.06] text-[11px] text-muted hover:text-primary transition-colors"
          >
            Add
          </button>
        </div>
      </div>

      {/* URL Patterns */}
      <div className="flex flex-col gap-1.5">
        <label className="text-[10px] font-medium text-muted uppercase tracking-wider">
          URL / Domain Patterns
        </label>
        <div className="flex flex-wrap gap-1">
          {urlPatterns.map((url) => (
            <span
              key={url}
              className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-white/[0.06] text-[11px] font-light text-secondary"
            >
              {url}
              <button
                type="button"
                onClick={() => setUrlPatterns(urlPatterns.filter((u) => u !== url))}
                className="text-muted hover:text-destructive"
              >
                <X size={10} />
              </button>
            </span>
          ))}
        </div>
        <div className="flex gap-1">
          <input
            type="text"
            value={newUrl}
            onChange={(e) => setNewUrl(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addUrl()}
            placeholder="Add domain (e.g. github.com)..."
            className="glass-input flex-1 px-2.5 py-1 text-[11px] rounded-lg"
          />
          <button
            type="button"
            onClick={addUrl}
            className="px-2 py-1 rounded-lg bg-white/[0.06] text-[11px] text-muted hover:text-primary transition-colors"
          >
            Add
          </button>
        </div>
      </div>
    </div>
  );
}
```

---

### Task 10: Create TrackedAppsList panel (Panel C)

The right panel showing all tracked apps/domains with reassignment.

**Files:**
- Create: `desktop-ui/src/components/productivity/TrackedAppsList.tsx`

**Step 1: Create TrackedAppsList component**

```tsx
import { Search } from "lucide-react";
import { useMemo, useState } from "react";
import { useMutation } from "../../hooks/useMutation";
import { formatHumanDuration } from "../../lib/dates";
import type { ActivityCategory, TrackedApp } from "../../lib/types";
import { getCategoryColor, getCategoryTypeColor } from "./shared";

interface TrackedAppsListProps {
  apps: TrackedApp[];
  categories: ActivityCategory[];
  onReassigned: () => void;
}

export function TrackedAppsList({ apps, categories, onReassigned }: TrackedAppsListProps) {
  const [search, setSearch] = useState("");
  const [showUncategorized, setShowUncategorized] = useState(false);
  const [editingApp, setEditingApp] = useState<string | null>(null);

  const filtered = useMemo(() => {
    let result = apps;
    if (showUncategorized) {
      result = result.filter((a) => !a.categoryId);
    }
    if (search) {
      const q = search.toLowerCase();
      result = result.filter(
        (a) =>
          a.displayName.toLowerCase().includes(q) ||
          a.appName.toLowerCase().includes(q),
      );
    }
    return result;
  }, [apps, search, showUncategorized]);

  const uncategorizedCount = useMemo(
    () => apps.filter((a) => !a.categoryId).length,
    [apps],
  );

  return (
    <div className="glass-card p-3 flex flex-col gap-2">
      <h3 className="text-[12px] font-medium text-secondary">Tracked Apps & Sites</h3>

      {/* Search */}
      <div className="relative">
        <Search size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted" />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search..."
          className="glass-input w-full pl-7 pr-2.5 py-1.5 text-[11px] rounded-lg"
        />
      </div>

      {/* Filter toggle */}
      {uncategorizedCount > 0 && (
        <button
          type="button"
          onClick={() => setShowUncategorized(!showUncategorized)}
          className={`text-[10px] font-light px-2 py-1 rounded-lg transition-colors ${
            showUncategorized
              ? "bg-brand/20 text-brand"
              : "text-muted hover:bg-white/[0.04]"
          }`}
        >
          Uncategorized ({uncategorizedCount})
        </button>
      )}

      {/* App list */}
      <div className="flex flex-col gap-0.5 max-h-[600px] overflow-y-auto">
        {filtered.map((app) => (
          <TrackedAppRow
            key={app.displayName}
            app={app}
            categories={categories}
            isEditing={editingApp === app.displayName}
            onEdit={() => setEditingApp(app.displayName)}
            onDone={() => {
              setEditingApp(null);
              onReassigned();
            }}
          />
        ))}
        {filtered.length === 0 && (
          <p className="text-[11px] font-light text-dim py-4 text-center">No apps found</p>
        )}
      </div>
    </div>
  );
}

function TrackedAppRow({
  app,
  categories,
  isEditing,
  onEdit,
  onDone,
}: {
  app: TrackedApp;
  categories: ActivityCategory[];
  isEditing: boolean;
  onEdit: () => void;
  onDone: () => void;
}) {
  const reassignMut = useMutation("productivity_category_upsert");

  const handleReassign = async (newCategoryId: string) => {
    const cat = categories.find((c) => c.id === newCategoryId);
    if (!cat) return;

    // Add this app/domain as a rule to the target category
    const rules = cat.rules ?? { appNames: [], bundleIds: [], urlPatterns: [] };
    const updatedRules = app.siteName
      ? {
          app_names: rules.appNames,
          bundle_ids: rules.bundleIds,
          url_patterns: [...new Set([...rules.urlPatterns, app.siteName])],
        }
      : {
          app_names: [...new Set([...rules.appNames, app.appName])],
          bundle_ids: rules.bundleIds,
          url_patterns: rules.urlPatterns,
        };

    await reassignMut.mutate({
      id: cat.id,
      name: cat.name,
      category_type: cat.categoryType,
      color: cat.color,
      icon: null,
      rules: updatedRules,
    });
    onDone();
  };

  return (
    <div className="flex items-center gap-2 px-1.5 py-1 rounded-md hover:bg-white/[0.03] group">
      {app.categoryId ? (
        <span
          className="w-2 h-2 rounded-sm flex-shrink-0"
          style={{ backgroundColor: getCategoryColor(app.categoryId) }}
        />
      ) : (
        <span className="w-2 h-2 rounded-sm flex-shrink-0 border border-dashed border-muted" />
      )}
      <div className="flex-1 min-w-0">
        <div className="text-[11px] font-light text-primary truncate">{app.displayName}</div>
        <div className="text-[9px] font-light text-dim">
          {app.categoryName ?? "Uncategorized"} · {formatHumanDuration(app.totalSecs)}
        </div>
      </div>
      {isEditing ? (
        <select
          className="glass-input text-[10px] px-1.5 py-0.5 rounded-md w-24"
          defaultValue={app.categoryId ?? ""}
          onChange={(e) => handleReassign(e.target.value)}
          onBlur={onDone}
          autoFocus
        >
          <option value="" disabled>
            Pick...
          </option>
          {categories.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
      ) : (
        <button
          type="button"
          onClick={onEdit}
          className="text-[9px] font-light text-dim opacity-0 group-hover:opacity-100 transition-opacity hover:text-primary"
        >
          Edit
        </button>
      )}
    </div>
  );
}
```

**Step 2: Verify full build + lint**

Run: `cd desktop-ui && bun run build 2>&1 | tail -10`
Run: `cd desktop-ui && bun run lint:fix 2>&1 | tail -10`

---

## Batch 4: Wiring & Polish (Tasks 11–12)

### Task 11: Wire CategoriesPage into ProductivityLayout

The CategoriesPage needs to use the ProductivityLayout wrapper for consistent nav. Adjust the page to detect the `/categories` route and show the Categories tab as active.

**Files:**
- Modify: `desktop-ui/src/components/productivity/ProductivityLayout.tsx`
- Modify: `desktop-ui/src/components/productivity/pages/CategoriesPage.tsx` — wrap in layout

**Step 1:** Wrap `CategoriesPage` content in `ProductivityLayout` with a special period prop or detect from pathname.

**Step 2:** In `ProductivityLayout`, add a "Categories" nav button after the period tabs. Use `useLocation()` to highlight it when active. Hide the DateNavigator when on categories page (no date to navigate).

**Step 3: Verify build**

---

### Task 12: Register new Tauri commands in main.rs

All new Tauri commands (`productivity_tracked_apps`, `productivity_category_delete`) must be registered in the Tauri builder's `invoke_handler`.

**Files:**
- Modify: `crates/desktop/src/main.rs` — add to `invoke_handler`
- Modify: `crates/desktop/tauri.conf.json` — add to allowed commands if needed

**Step 1:** Find the `invoke_handler` macro call and add the new commands.

**Step 2:** Register in capabilities/permissions if the app uses Tauri's permission system.

**Step 3: Verify full build**

Run: `cargo build --workspace 2>&1 | tail -5`
Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
