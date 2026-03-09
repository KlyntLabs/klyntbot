# Layered Activity Calendar Design

## Overview

Upgrade the day and week calendar views from flat timeline entries to a **layered container model** where all time is categorized as **Focused** (purple) or **Unfocused** (gray). Inside each container, task time entries and app activity are nested as inner layers. Point events appear as small dots.

## Layer Hierarchy (outer → inner)

| Layer | Display | Color | Default |
|-------|---------|-------|---------|
| Focus Sessions | Thick container blocks | Brand purple (#A855F7), intensity varies by `quality_score` | On |
| Task Time Entries | Inner blocks with green left-border | Green (#22C55E) | On |
| App Activity | Thin horizontal bars inside containers | Light gray, per-app color | On |
| Point Events | Small colored dots | Blue (notes), dark green (task completed), yellow (finance) | On |
| Calendar Events | — | — | Disabled (coming soon) |

## Container Model

All time on the day axis is covered by containers:

- **Focused containers** (purple) — wrap each `FocusSession` time range. Quality gradient: darker purple = higher `quality_score`.
- **Unfocused containers** (gray) — fill gaps between focus sessions. Generated client-side by computing time ranges not covered by any focus session.

Inside both container types:
- Task time entries render as blocks with green left-border accent
- App activity renders as thin horizontal bars with per-app colors
- Point events render as small colored circles at their timestamp position

## View Adaptations

### Day View (full layered rendering)

```
┌─────────────────────────────────────────┐
│ 9:00 AM                                 │
│ ┌─ gray container (unfocused) ─────────┐│
│ │  [Chrome ███░] [Slack ██░]           ││
│ └──────────────────────────────────────┘│
│ 10:00 AM                                │
│ ┌─ purple container (focus, quality=8) ┐│
│ │  ▌green: "Implement API" (task)      ││
│ │  [Ghostty ████░] [Chrome █░]         ││
│ │  ● blue dot: note updated            ││
│ └──────────────────────────────────────┘│
│ 12:00 PM                                │
│ ┌─ gray container (unfocused) ─────────┐│
│ │  [Chrome ██████░]                    ││
│ │  ● yellow dot: transaction           ││
│ └──────────────────────────────────────┘│
└─────────────────────────────────────────┘
```

### Week View (compressed, same model)

- Containers: purple (focus) / subtle gray (unfocused), positioned by time
- Left border accent: green for task entries, per-app color for activity
- No inner app bars (too narrow) — visible on hover tooltip
- Tooltip: "2h 15m focus · 1h 40m on 'Implement API' · Chrome 65%"
- Click → navigates to day view for full detail

### Month View (unchanged + focus coloring)

- Keep current grid of day cells
- Color intensity = focus hours (purple scale)
- Optional small label in corner = total focus minutes (e.g., "2h45")
- Hover tooltip: "3 focus sessions · 87% quality"

### Year View (unchanged + focus labeling)

- Keep heatmap
- Color intensity = total focus hours
- Legend: "Focus time" instead of "Activity"

## Data Flow

```
timeline_query({ startDate, endDate, sources: enabledLayers })
  → TimelineResponse { entries, summary }
  → Client groups entries by source:
      focusSessions  = entries.filter(source === "focus")
      taskEntries    = entries.filter(source === "task")
      appActivity    = entries.filter(source === "productivity")
      pointEvents    = entries.filter(entryType is point event)
  → Build containers:
      1. Sort focus sessions by startedAt
      2. Fill gaps with unfocused containers
      3. Assign task/app/event entries to their enclosing container
  → Render layered
```

**No backend changes needed** — existing `timeline_query` with `sources: Option<Vec<TimelineSource>>` filter already supports all required data. Layer toggling changes which sources are requested.

## Layer Toggle UI

A "Layers" icon button in the DashboardLayout toolbar, right side, opens a `glass-dropdown` popover:

- Checkbox rows: Focus Sessions, Task Time Entries, App Activity, Point Events
- Calendar Events row (disabled, "Coming soon" label)
- "Reset to defaults" button at bottom
- State persisted to `localStorage` key `dashboard-layers`
- Enabled layers passed as `sources` param to `timeline_query`

## Summary Panel Updates

Layer-aware breakdown:
- **Focus**: total time + session count + avg quality score
- **Focus ratio**: "62% (↑12% from last week)" — pulled from coaching/comparison data
- **Tasks**: time breakdown by task name
- **Apps**: top apps by usage duration
- **Events**: point event count by type

## Color Tokens

New CSS variables in `theme.css`:

```css
--timeline-focus: #A855F7;
--timeline-focus-high: #9333EA;    /* quality_score > 7 */
--timeline-focus-low: #C084FC;     /* quality_score < 4 */
--timeline-unfocused: rgba(255, 255, 255, 0.04);
--timeline-task: #22C55E;
--timeline-dot-note: #60A5FA;
--timeline-dot-task: #16A34A;
--timeline-dot-finance: #FACC15;
```

## Constraints

- Finance transactions have date-only timestamps (`NaiveDate`), so they appear at start-of-day on the time axis
- Calendar events: schema exists (`calendar_event_cache` table) but no Rust repo/handler — disabled in layer toggle until backend is built
- `ActivityEvent.category_type` (productive/neutral/distracting) is not available at timeline query level — all app activity renders with neutral color for now
