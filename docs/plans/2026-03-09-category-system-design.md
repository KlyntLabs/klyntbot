# Category System Improvements — Design

## Problem

1. **Visual**: All productive categories share `var(--success)` green, all distracting share `var(--destructive)` red — donut chart is unreadable
2. **Classification**: No way to fix wrong categorizations (e.g., ChatGPT marked as distraction instead of productive work)
3. **Discoverability**: Users can't see which apps/domains are tracked or how they're classified

## Solution

Three changes: unique colors, grouped CategoriesList, and a Category Manager page.

### 1. Color System

Each category gets a **unique hex color**. A small type badge dot (green/gray/red) indicates productive/neutral/distracting.

| Category | Hex | Type |
|----------|-----|------|
| Coding | `#22C55E` | Productive |
| AI Tools | `#06B6D4` | Productive |
| Developer Tools | `#10B981` | Productive |
| Design | `#A78BFA` | Productive |
| Documentation | `#60A5FA` | Productive |
| Project Management | `#8B5CF6` | Productive |
| Learning | `#2DD4BF` | Productive |
| Cloud/DevOps | `#34D399` | Productive |
| Communication | `#F59E0B` | Neutral |
| Browsing | `#94A3B8` | Neutral |
| Email | `#78716C` | Neutral |
| Music | `#C084FC` | Neutral |
| News & Forums | `#FB923C` | Neutral |
| Finance | `#A1A1AA` | Neutral |
| Social Media | `#F43F5E` | Distracting |
| Video & Streaming | `#EF4444` | Distracting |
| Gaming | `#E11D48` | Distracting |
| Shopping | `#FB7185` | Distracting |
| Entertainment | `#F87171` | Distracting |

Type badge colors: `#22C55E` (productive), `#94A3B8` (neutral), `#F43F5E` (distracting).

### 2. CategoriesList Component

- **Hide 0% categories** — only show categories with tracked time
- **Group by type** — Work / Utilities / Distraction sections
- **Group headers** show total time and percentage for the group
- **Nested donut** — outer ring = 3 type groups, inner ring = individual categories
- **Clickable rows** — navigate to Category Manager filtered to that category

Layout:
```
Work                              56%  2h 5m
  ● Coding              48%  1h 48m
  ● AI Tools             7%    16m

Utilities                          9%    20m
  ● Browsing             6%    13m
  ● Communication        3%     7m

Distraction                        8%    18m
  ● Social Media         7%    16m
```

### 3. Category Manager Page

New tab in the Productivity section (alongside Day/Week/Month).

**Three panels:**

**Panel A — Category List (left):** All categories grouped by Work/Utilities/Distraction. Shows name, color dot, app/domain count. Click to select. "+" button to create custom categories.

**Panel B — Category Editor (center):** Editable name, type dropdown (Work/Utilities/Distraction), color picker (swatches + hex), rules lists (app names, bundle IDs, URL patterns). Delete button for custom categories only.

**Panel C — Tracked Apps & Domains (right):** All apps/domains seen in activity_events with current category, time tracked. Uncategorized items highlighted at top. Click to reassign via dropdown. Search/filter bar.

### 4. Data Flow

**Existing infrastructure (no schema changes needed):**
- `activity_categories` table already has `name`, `category_type`, `color`, `rules` (JSON), `is_system`
- `ActivityCategoryRepo` already has `get`, `list_all`, `upsert`, `delete`
- `productivity_categories` command already exists
- `productivity_category_upsert` handler already exists in AppCore

**New commands needed:**
- `productivity_category_create` — create custom category with rules
- `productivity_category_delete` — delete custom category
- `productivity_category_update` — update existing category (name, type, color, rules)
- `productivity_tracked_apps` — query distinct apps/domains from activity_events with category + duration
- `productivity_reassign_app` — move an app/domain rule from one category to another
- `productivity_reaggregate_day` — recompute daily_summary after category type change

**Historical data behavior:**
- `activity_events.category_id` references the category — changing the category's type automatically changes how those events are classified in new aggregations
- When category type changes, trigger `reaggregate_day` for affected dates so cached daily_summaries update
- Rule changes (moving an app between categories) only affect future events by default

### 5. CategoryUsage Enhancement

Currently `CategoryUsage` only has `category` (name) and `durationSecs`. Add `categoryId` and `categoryType` so the frontend can group without a second lookup.

```typescript
// Before
interface CategoryUsage { category: string; durationSecs: number; }

// After
interface CategoryUsage {
  categoryId: string;
  category: string;
  categoryType: "productive" | "neutral" | "distracting";
  durationSecs: number;
}
```

Same change in Rust `CategoryUsageResponse`.
