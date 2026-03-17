# Knowledge Graph V2 — Design Spec

## Overview

Upgrade the knowledge graph visualization from a basic d3-force SVG renderer to a production-grade Cytoscape.js-powered graph with intelligent clustering, PageRank-based node sizing, relationship visibility, and AI-powered community detection.

**Current state:** 652-line d3-force SVG component with manual zoom/pan/drag, basic tag coloring, and force-directed layout with no meaningful clustering.

**Target state:** Cytoscape.js graph with compound node clustering (by notebook default, Leiden communities via AI toggle), PageRank + recency node sizing, labeled edges, interactive legend, zoom-adaptive labels, and theme-aware styling.

## 1. Layout & Clustering Engine

### Default View — Notebook Clusters (fCoSE)

- Each notebook becomes a Cytoscape **compound node** (parent)
- Notes are child nodes inside their notebook compound
- Layout algorithm: `fcose` (fast Compound Spring Embedder) — respects parent-child grouping, force-directed within and between clusters
- Compound nodes get a subtle fill (cluster color at 8% opacity) + label at the top
- **Orphan handling (two compounds):**
  - "Floating ideas" — notes with links but no notebook (warm gray `#9CA3AF`)
  - "Isolated notes" — no links at all (dim cool gray `#6B7280`, smaller base node size 10px)

### AI Smart Clusters Toggle (Leiden Community Detection)

- Runs **Leiden community detection** on the link graph in the Rust backend (`crates/cognitive`)
- Replaces notebook compounds with detected communities
- Community labels: LLM-generated 1-2 line summary per community (async, cached in `graph_communities` table)
- Cached result — recompute on demand via "Refresh clusters" button, or auto-refresh after 10+ new links
- Results stored in SQLite: `graph_communities` table (`community_id`, `node_ids JSON`, `label`, `modularity_score`, `last_computed`)
- Frontend receives lightweight JSON: `{ communities: [{ id, label, nodeIds: string[] }] }`

### Hybrid Mode (Advanced Toggle)

- Notebook compounds as primary structure (level-1 parents)
- Leiden sub-clusters as **nested compound nodes** within each notebook (level-2 parents)
- fCoSE supports nested compounds natively — no convex hull rendering needed
- Tags provide sub-cluster coloring

### View Modes (Preserved)

- **Full** — all notes and links, clustered layout
- **Local** — BFS from active note up to N hops (1, 2, or 3)
- **Orphans** — only orphan compounds visible
- These filter which nodes appear; the layout algorithm stays the same

### Layout Transitions

- Switching between Notebook / AI / Hybrid uses `cy.layout().run()` with `animate: true, duration: 800ms`
- `fit()` + `center()` after layout completion to prevent disorientation
- Show brief loading overlay ("Recomputing clusters...") during Leiden computation (200-800ms for 2000+ notes)

## 2. Node Visual Design

### Sizing — Hybrid PageRank + Recency Boost

- Primary driver: **PageRank** (computed in Rust, stored alongside Leiden results)
- Algorithm: iterative PageRank, 20 iterations, damping factor 0.85
- Mapping: logarithmic scale, `12px → 40px`
- Small recency boost: +25% weight to notes edited within last 14 days
- Formula:
  ```
  radius = 12 + (pageRankNormalized * 24) + (recencyFactor * 6)
  ```
- Performance guard: when node count > 1200, cap max radius at 32px and reduce label density
- Performance mode (> 1500 nodes): cap radius at 28px, disable recency glow, switch to WebGL renderer via `cytoscape-canvas` extension

### Appearance — Colored Circles

- Shape: `ellipse` (Cytoscape native)
- Fill color: cluster color (notebook color in default, community color in AI view)
- Palette: 12-color palette assigned per cluster (existing `TAG_PALETTE`)
- Border: 1.5px, same color at 35% opacity
- Active/selected: 3px border + soft outer glow ring (`shadow-blur` + `shadow-color`)
- Orphan nodes:
  - "Floating ideas": warm gray `#9CA3AF`, slightly larger base size
  - "Isolated notes": dim cool gray `#6B7280`, 10px min size

### Labels — Zoom-Adaptive

- Position: below node (`text-valign: bottom, text-halign: center`)
- Font: Inter or system sans, weight 500
- Anti-collision: Cytoscape `text-margin-y: 4`
- Truncation: 20 characters + ellipsis
- **Zoom tiers:**
  - `< 0.5×`: hide labels completely (only colored dots)
  - `0.5× – 1.2×`: show truncated title
  - `> 1.2×`: show full title, no truncation
- Hover tooltip at any zoom: full title + 1-line body preview + PageRank score + last edited

### Semantic Emphasis Ring

- Nodes matching current conversation context or recent cognitive facts get a thin golden inner ring
- Uses embedding similarity from `cognitive_fact_embeddings` (cosine > 0.7 threshold)
- Makes the graph feel "alive" and connected to the agent

### Legend Panel (Bottom-Left)

- Collapsible glass-card (default collapsed on mobile, expanded on desktop)
- Shows: color swatch + cluster name + node count per row
- Click any row → highlight that entire cluster, dim others to 35% opacity
- Extra row at top: "Recently active" (shows glow nodes)
- Styled with `glass-card` class for theme consistency

## 3. Edges & Relationship Visibility

### Edge Appearance (Defaults)

- Style: 1px, curved Bézier (`curve-style: bezier`)
- Color: inherited from source node at 25% opacity
- Connected to active/selected node: 2.5px + 60% opacity (source color)
- Directional: small triangle at target end (`target-arrow-shape: triangle`, size 6px)
- Hover on edge: tooltip showing "Source Title → Target Title" + relationship type

### Edge Labels (Toggle — Off by Default)

- Toolbar toggle: "Show relationship labels"
- Displays semantic relationship at edge midpoint (e.g. "references", "extends", "supports", "contradicts")
- Source: pulled from cognitive memory (annotations, procedural rules, auto-extracted triples)
- Font: 8.5px, muted color, only visible at zoom > 0.75×

### Edge Thickness by Strength (Hybrid)

- Base: raw link count between the two notes
- Boost: semantic similarity score from LanceDB (cosine similarity)
- Mapping:
  - Single link: 1px
  - Mutual/bidirectional: 1.8px
  - 3+ links OR high semantic similarity (> 0.65): 2.8px

### Neighborhood Highlighting (Hover)

- Hover a node: 1-hop neighborhood (nodes + edges) becomes full opacity + source color
- All non-neighbor nodes + edges fade to 15% opacity
- Smooth 180ms transition
- AI context mode (when agent is active): edges semantically relevant to current conversation get subtle glow + dashed style

### Zoom-Based Edge Behavior

- At zoom < 0.6×: all edges drop to 0.6px, labels hidden
- At zoom > 1.8×: edges get subtle shadow for depth

## 4. Interactions & Navigation

### Mouse/Touch

- **Click node** → select, show detail in right panel (existing behavior)
- **Double-click node** → open in editor
- **Click compound/cluster** → zoom-to-fit that cluster with 300ms animation
- **Right-click node** → context menu: "Open", "Show neighborhood", "Pin node", "Delete"
- **Right-click compound** → context menu: "Collapse/Expand cluster", "Pin cluster", "AI Summary" (triggers LLM label refresh)
- **Box select** (shift+drag) → select multiple nodes
- **Drag node** → pin to position (existing behavior, Cytoscape native)
- **Pan** → drag background
- **Zoom** → scroll wheel, pinch

### Keyboard

- `+` / `-` — zoom in/out
- `F` — fit-to-screen
- `Esc` — deselect all
- `Tab` — cycle through nodes
- `/` — focus search input

### Breadcrumb Trail

- When in Local view, show path: "Full Graph → Notebook: X → Note: Y"
- Clickable breadcrumbs in toolbar area
- Clicking a breadcrumb navigates to that scope level

## 5. Insights Panel (Right Sidebar)

- Reuses existing `ContextPanel` slot — add a "Graph Insights" tab
- Collapsible, shown when in graph view mode

### Content

- **Graph stats:** total notes, total links, cluster count, orphan count
- **Top 5 hub notes** (by PageRank) — clickable to navigate in graph
- **Recently active** — last 5 edited notes with timestamps
- **Cluster health** — density bars showing which clusters are dense vs sparse

## 6. Theme Integration

- All Cytoscape styles read from CSS custom properties
- Initialize Cytoscape stylesheet from computed CSS vars at mount time
- Re-initialize on theme change (listen to `data-theme` attribute mutation)

### Token Mapping

| Element | CSS Variable | Dark Value | Nexora Value |
|---------|-------------|------------|--------------|
| Background | `--background` | `#000000` | `#ffffff` |
| Node label | `--text-primary` | `#f0f2f5` | `#000000` |
| Muted label | `--text-muted` | `#7d8590` | `#737373` |
| Edge default | `--border` | `rgba(255,255,255,0.08)` | `#e5e5e5` |
| Compound bg | `--surface-lowest` + cluster color at 8% | dark tint | light tint |
| Active glow | `--brand` | `#f97316` | `#ca8a04` |
| Legend card | `glass-card` class | glass material | flat white + border |

## 7. Backend Requirements (Rust)

### New IPC Command: `graph_compute_metrics`

**Request:** `{ noteIds?: string[] }` (optional filter, defaults to all)

**Response:**
```typescript
interface GraphMetrics {
  pagerank: Record<string, number>;        // noteId → score (0-1)
  communities?: {
    id: string;
    label: string;                          // LLM-generated summary
    nodeIds: string[];
    modularityScore: number;
  }[];
  edgeWeights?: Record<string, number>;    // "sourceId:targetId" → semantic similarity
}
```

### Implementation

- **PageRank:** Simple iterative algorithm on note link adjacency matrix. 20 iterations, damping 0.85. No external crate needed — pure Rust loop over the adjacency list.
- **Leiden:** Use existing `cognitive` crate graph infrastructure. Implement or integrate a Leiden algorithm. Cache results in `graph_communities` table.
- **Community labels:** Async LLM call per community. Prompt: "Briefly summarize the topic of these notes: [titles]". Cache in `graph_communities.label` column.
- **Semantic edge weights:** Batch cosine similarity from LanceDB embeddings for connected note pairs. Return as edge weight map.

### Database Schema Addition

```sql
CREATE TABLE IF NOT EXISTS graph_communities (
  community_id TEXT PRIMARY KEY,
  node_ids TEXT NOT NULL,           -- JSON array of note IDs
  label TEXT,                        -- LLM-generated summary
  modularity_score REAL,
  algorithm TEXT DEFAULT 'leiden',
  last_computed TEXT NOT NULL        -- RFC3339 timestamp
);
```

## 8. Frontend Architecture

### File Changes

| Action | File | Description |
|--------|------|-------------|
| **Rewrite** | `GraphView.tsx` | Replace d3-force SVG with Cytoscape.js canvas |
| **Rewrite** | `useGraphData.ts` | Add PageRank/community data fetching, element mapping to Cytoscape format |
| **Update** | `GraphToolbar.tsx` | Add cluster mode toggle (Notebook / AI / Hybrid), edge label toggle |
| **New** | `GraphLegend.tsx` | Collapsible legend panel component |
| **New** | `GraphInsightsTab.tsx` | Insights panel for ContextPanel |
| **New** | `useCytoscapeTheme.ts` | Hook to generate Cytoscape stylesheet from CSS variables |
| **New** | `useGraphMetrics.ts` | Hook wrapping `graph_compute_metrics` IPC with caching |
| **Update** | `GraphMinimap.tsx` | Migrate from d3-force to Cytoscape (or keep as simplified version) |
| **Update** | `KnowledgeBasePage.tsx` | Wire new toolbar controls, pass cluster mode state |
| **Delete** | Manual SVG zoom/pan/drag code (~400 lines) | Replaced by Cytoscape built-ins |

### Cytoscape Extensions

- `cytoscape-fcose` — fast compound spring embedder layout (already available via `cytoscape-cose-bilkent` dep)
- `cytoscape-context-menus` — right-click context menu (or keep custom portal-based)
- No WebGL renderer needed initially — Cytoscape canvas renderer handles 2000+ nodes well

## 9. Migration Strategy

1. **Phase 1 — Core rewrite:** Replace GraphView with Cytoscape, notebook compounds, fCoSE layout, PageRank sizing. Delete d3-force code. This alone is a massive visual upgrade.
2. **Phase 2 — Backend metrics:** Implement `graph_compute_metrics` IPC with PageRank + Leiden. Wire community labels.
3. **Phase 3 — Polish:** Edge labels, semantic emphasis ring, AI context highlighting, insights panel, keyboard shortcuts, breadcrumbs.

Phase 1 is self-contained and delivers the biggest impact. Phases 2-3 build on it incrementally.

## 10. Edge Cases & Accessibility

### Empty States
- **Empty vault** → centered empty state: "Your knowledge graph will appear here" + "Create your first note" CTA
- **Single node** → centered with subtle pulse animation
- **No links** → all nodes shown in "Isolated notes" compound, message: "Link notes with [[wikilinks]] to see connections"

### Accessibility
- ARIA labels on compound nodes and interactive elements
- Keyboard-navigable node list via Tab cycling
- Screen reader announcement on node selection ("Selected: Note Title, 5 connections")
- Reduced motion: disable layout animations when `prefers-reduced-motion` is active

### Future Feature: AI Ask in Graph
- Click any cluster → "What's the main idea here?" → agent answers using community summary
- Turns the graph into a live reasoning surface connected to the cognitive pipeline
