# Knowledge Fabric Explorer — Phase 3 Design Spec

**Goal:** Upgrade the existing graph view into a unified, layered Knowledge Fabric Explorer that visualizes notes, Louvain communities, entities, and hierarchical tree nodes as progressive layers — turning the graph from a viewing tool into a living thinking workspace.

**Builds on:** Phase 1 (hierarchical tree nodes, NoteTreeNavigator, 10-factor scoring) and Phase 2 (Louvain community detection, CommunityBuilder, community embeddings, Path 4 community traversal).

**Scope:** Core layered graph + clean hooks for future integrations (coaching, mirror, focus, autotuner). Integrations are follow-up phases.

---

## 1. Architecture — Data Flow & API Surface

### Base Load

New Tauri command `fabric_graph_base` returns a single lightweight JSON payload:

```
FabricGraphBase {
    notes: Vec<FabricNote>,            // id, title, notebookId, tags, bodyPreview, treeSectionCount, entityCount
    links: Vec<FabricLink>,            // sourceId, targetId, linkType: "wiki"
    communities: Vec<FabricCommunity>, // id, name, color, stability, memberCount, memberNoteIds
    suggested_preset: Option<String>,  // from autotuner learning (null in Phase 3, UI falls back to last-used)
    last_activity_timestamp: String,   // UTC ISO — most recent community/entity update
    live_pulse_active: bool,           // true if any community updated in last 5 min
}
```

- `FabricNote`: `{ id, title, notebookId, tags: Vec<String>, bodyPreview: String, treeSectionCount: u32, entityCount: u32 }`
- `FabricLink`: `{ sourceId, targetId, linkType: String }`
- `FabricCommunity`: `{ id, name, color: String, stability: f64, memberCount: u32, memberNoteIds: Vec<String> }`

The base payload keeps the initial load instant — users see the familiar note graph immediately.

### Incremental Expand

New Tauri command `fabric_graph_expand` fetches layer-specific data on demand:

```
FabricExpandRequest { layer: String, scopes: Vec<String> }
```

Responses by layer:

- `layer: "entities"` → `FabricEntities { entities: Vec<FabricEntity>, edges: Vec<FabricEntityEdge> }`
  - `FabricEntity`: `{ id, name, entityType, mentionCount }`
  - `FabricEntityEdge`: `{ entityId, noteId, weight: f64 }`

- `layer: "tree"`, scopes = noteIds → `Vec<FabricTreeNodes>` per note
  - `FabricTreeNodes`: `{ noteId, nodes: Vec<FabricTreeNode> }`
  - `FabricTreeNode`: `{ id, parentId, nodeType, title, contentPreview, level }`

- `layer: "community_detail"`, scopes = communityIds → `Vec<FabricCommunityDetail>` per community
  - `FabricCommunityDetail`: `{ communityId, representativePaths: Vec<String>, topEntities: Vec<String>, stabilityHistory: Vec<f64>, members: Vec<FabricMember> }`
  - `FabricMember`: `{ noteId, treeNodeId, membershipScore: f64 }`

Supports batching: multi-select 3 nodes → one call returns all subtrees.

### Live Updates (SSE)

Reuse the existing SSE pipeline event channel. New `FabricGraphEvent` variants mapped from `DomainEvent`:

| DomainEvent | FabricGraphEvent | animationHint | intensity |
|-------------|------------------|---------------|-----------|
| `CommunityDiscovered` | `node_added` (community) | `pulse` | 0.8 |
| `CommunityUpdated` | `node_updated` (community) | `grow` | 0.5 |
| `CommunityWeakened` | `node_updated` (community) | `fade` | 0.7 |
| `NoteContentChanged` | `node_updated` (note) | `pulse` | 0.3 |

Event shape: `{ type, nodeType, id, data/delta, animationHint, intensity }`

Edge events: `FabricEdgeChanged { source, target, weight, action: "added"|"removed"|"updated" }`

### User Actions

New Tauri command `fabric_graph_action`:

```
FabricActionRequest { action: String, payload: serde_json::Value }
```

| Action | Payload | Phase 3 behavior |
|--------|---------|-------------------|
| `suggest_merge` | `{ noteId, targetCommunityId }` | Creates manual override record; Louvain re-detects on next cycle |
| `create_bridge_note` | `{ sourceCommunityId, targetCommunityId, content }` | Creates note via NoteRepo, publishes NoteContentChanged |
| `link_entity` | `{ entityId, noteId }` | Creates entity_tree_link |
| `pin_to_focus` | `{ communityId }` | Returns Ok, no-op (hook for follow-up) |
| `highlight_gap` | `{ edgeId }` | Returns Ok, no-op (hook for follow-up) |

### Hooks (exported, not consumed in Phase 3)

```rust
pub enum FabricGraphAction {
    PinToFocus { community_id: String },
    CreateBridgeNote { source_community: String, target_community: String, content: String },
    HighlightGap { edge_id: String },
    SuggestMerge { note_id: String, target_community_id: String },
    LinkEntity { entity_id: String, note_id: String },
}
```

---

## 2. Frontend — Layer System & Graph Rendering

### Layer Toggles

Added to existing `GraphToolbar` in a "Layers" section. Three toggle buttons:

- **Communities** (icon: `Network`) — shows community cluster boundaries, colors nodes by community, adds community label nodes
- **Entities** (icon: `Atom`) — adds entity nodes (smaller, diamond-shaped) + entity-to-note edges
- **Tree** (icon: `TreePine`) — enables per-note expand/collapse of internal heading structure

### "Semantic" Preset

Activates the currently grayed-out "Semantic" toolbar button:
- Enables Communities + Entities layers
- Switches clustering mode from "notebook" to "semantic" (community_id replaces notebook_id as cluster key)
- Tree layer stays opt-in (per-node click to expand)
- Auto-applied if autotuner `suggested_preset` returns "Semantic", or if the user has >2 community engagement events in the last 7 days (tracked via localStorage counter). Phase 3 `suggested_preset` returns null — falls back to engagement-based heuristic, then last-used preset from localStorage

### Node Visual Hierarchy

All rendered in existing react-force-graph-2d/3d:

| Node type | Shape | Size | Color | Label |
|-----------|-------|------|-------|-------|
| Note | Circle | 18–46px (by linkCount) | Community color (or notebook color if communities off) | Title |
| Community label | Rounded rect (floating) | Auto-sized to text | Community color at 20% opacity | Name + member count |
| Entity | Diamond | 12–24px (by mentionCount) | Accent color (`text-brand`) | Entity name |
| Tree section | Small circle | 10–16px (by level) | Parent note color at 60% opacity | Heading title |
| Tree text | Dot | 6px | Parent note color at 30% opacity | None (hover for preview) |

### Community Cluster Rendering

- Reuses existing `clusterAttractionForce` from `useForceGraph.ts` — `community_id` replaces `notebook_id` as cluster key when Communities layer is on
- Community boundary: convex hull outline (dashed, community color at 15% opacity) around member nodes
- Community label node: positioned at cluster centroid, repels other nodes slightly

### Tree Expansion UX

- Small expand icon (ChevronRight) appears on note nodes when Tree layer is enabled
- Click expand → `fabric_graph_expand("tree", [noteId])` → sub-nodes spring-animate outward from parent (300ms)
- Click again (or Spacebar) → collapse: sub-nodes animate back, removed from graph
- Expanded note becomes slightly transparent (container feel)
- Hovering any note node (even when Tree layer is off) shows mini-preview tooltip with first 2–3 headings from `bodyPreview` — no expand call needed. Users can preview content before deciding to enable Tree layer

### Entity Edge Rendering

- Entity-to-note edges: thinner (1px), dashed, accent color
- Entity nodes cluster near their most-connected note (weak attraction force)
- Hovering entity highlights all connected notes

### Animation (SSE-driven)

| animationHint | Visual | Duration |
|---------------|--------|----------|
| `pulse` | Node scales 1.2x and back | 400ms |
| `grow` | Edge width increases smoothly | 600ms |
| `fade` | Node/edge opacity fades to 0.3 | 800ms |
| `drift` | Node position shifts toward new cluster centroid | 1000ms |

### Drag Interaction Feedback

When Communities layer is enabled:
- Dragging a note between clusters shows a temporary ghost edge (dashed, community color) + tooltip: "Drop to suggest merge into [community name]"
- On drop → confirmation dialog → `FabricGraphAction::SuggestMerge`

### Fabric Pulse Badge

Bottom-left corner (near clusters legend):
- Small dot + "Fabric updated 12s ago" text
- Pulses green on each SSE update
- Clickable: opens mini popover with "View latest updates" (zoom to pulsing cluster) / "Switch to Semantic" / "Highlight gaps in current view" / "Pin strongest community to focus"
- Controlled by `live_pulse_active` from base payload

---

## 3. Interactions — Click, Drag, Keyboard, Context Menu

### Single-Click

| Target | Action |
|--------|--------|
| Note node | Select, show preview panel (right side). If Communities layer on, highlight same-community siblings with subtle glow |
| Community label | Highlight all members + pulse if updated in last 30s. Panel shows: name, summary, stability, representative_paths (clickable), member count |
| Entity node | Highlight all connected notes with accent edges. Panel shows: name, type, mentionCount, list of referencing notes |
| Tree section | Select, panel shows heading content. Highlight parent note + sibling sections |
| Empty canvas | Deselect all, clear preview panel |

### Double-Click

| Target | Action |
|--------|--------|
| Note node | Open in editor (existing behavior) |
| Community label | Zoom-to-fit community members (filter to community + entity bridges) |
| Entity node | Open chat with pre-filled: "Tell me about [entity] across my notes" (triggers Path 4) |
| Tree section | Open note in editor, scrolled to that heading |

### Right-Click Context Menu

| On | Menu items |
|----|-----------|
| Note node | Open in editor / Expand tree / Find related (→ chat) / Pin to focus / Quick bridge |
| Community label | Focus on community / Rename / Pin to focus / Quick bridge / View members |
| Entity node | Find across notes (→ chat) / Open references / Hide from graph |
| Tree section | Open in editor at heading / Create flashcard from section |
| Empty canvas | Fit to screen / Re-layout / Clear pins / Toggle layers submenu |

### Quick Bridge

Right-click → "Quick bridge" on any node:
- Opens small popover (not full editor)
- Pre-filled title: "Bridge: [source] ↔ [target]"
- Textarea for quick thought (2–3 sentences)
- "Create note" button → creates note + opens in editor
- Uses `FabricGraphAction::CreateBridgeNote`

### Drag & Drop

| Action | Result |
|--------|--------|
| Drag note between communities | Ghost edge during drag. On drop: show temporary merged cluster preview (semi-transparent) + "Undo" button (5s timer). If not undone → `SuggestMerge` |
| Drag entity onto note | "Link [entity] to [note]?" → creates entity_tree_link |
| Drag note freely | Pin position (existing fx/fy, persisted to IndexedDB) |

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Space | Toggle expand/collapse on selected note (Tree layer) |
| Cmd/Ctrl + Click | Multi-select nodes |
| Cmd/Ctrl + A | Select all visible nodes |
| Escape | Deselect all / close context menu |
| 1 / 2 / 3 | Toggle Communities / Entities / Tree layers |
| S | Apply "Semantic" preset |
| Cmd/Ctrl + K | Search fabric (find community, entity, or note by name) |
| F | Fit to screen |

### Multi-Select Batch Actions

Toolbar appears when 2+ nodes selected:
- "Merge into community" (if notes from different communities)
- "Expand all trees"
- "Create bridge note from selection"
- "Compare in chat" (opens chat with selected nodes as context)

---

## 4. Backend — Tauri Commands & Handler Structure

### New Files

- `crates/desktop-shared/src/commands/fabric.rs` — request/response types for `fabric_graph_base`, `fabric_graph_expand`, `fabric_graph_action`
- `crates/app-core/src/handlers/fabric.rs` — handler implementations reading from NoteRepo, CommunityRepo, EntityRepo, SqliteBookTreeRepo
- `crates/desktop/src/commands/fabric.rs` — thin Tauri command adapters delegating to AppCore

### fabric_graph_base Handler

Reads from:
- `NoteRepo::list_all()` → notes with bodyPreview
- `note_repo.get_all_links()` → wiki-link edges
- `CommunityRepo::list_active_communities()` → communities + member note IDs
- `EntityRepo` + `SqliteBookTreeRepo` → counts per note (treeSectionCount, entityCount)

Computes:
- `last_activity_timestamp` from most recent community `updated_at`
- `live_pulse_active` = any community updated in last 5 minutes

### fabric_graph_expand Handler

- `"entities"` → `EntityRepo::list_all()` + `SqliteGTLinkRepo` for edges
- `"tree"` → `SqliteBookTreeRepo::get_children_recursive(noteId)` for each scope
- `"community_detail"` → `CommunityRepo::get_community()` + `get_members()` for each scope

### fabric_graph_action Handler

- `suggest_merge` → insert manual override into `community_members` with boosted score; next CommunityBuilder cycle incorporates it
- `create_bridge_note` → `NoteRepo::create_note()` + publish `NoteContentChanged`
- `link_entity` → insert into `entity_tree_links`
- `pin_to_focus` → returns Ok (hook, no-op in Phase 3)
- `highlight_gap` → returns Ok (hook, no-op in Phase 3)

### SSE Event Forwarding

In `dev_server/streaming.rs` and `desktop/app_core.rs`:
- Subscribe to `DomainEventBus` for `CommunityDiscovered`, `CommunityUpdated`, `CommunityWeakened`, `NoteContentChanged`
- Map to `FabricGraphEvent` JSON with `animationHint` and `intensity` fields
- Forward on existing `/api/stream` SSE endpoint (dev server) or Tauri event channel (desktop)

### Dev Server Routes

Add to `dev_server/mod.rs`:
- `POST /api/fabric_graph_base`
- `POST /api/fabric_graph_expand`
- `POST /api/fabric_graph_action`

---

## 5. Scope Boundaries & Hooks

### Phase 3 Delivers

- `fabric_graph_base`, `fabric_graph_expand`, `fabric_graph_action` Tauri commands + dev server routes
- Unified graph upgrade: layer toggles (Communities, Entities, Tree) in `GraphToolbar`
- "Semantic" preset activates Communities + Entities layers
- All interactions: click, double-click, drag-to-merge, right-click context menu, keyboard shortcuts, multi-select batch actions
- Quick Bridge popover (creates real notes)
- SSE forwarding of community/entity events with animation hints
- Fabric pulse badge (bottom-left)
- Convex hull community boundaries + entity diamond nodes + tree expansion animation
- `FabricGraphAction` enum exported — `CreateBridgeNote`, `SuggestMerge`, `LinkEntity` functional; `PinToFocus`, `HighlightGap` return Ok (no-op hooks)

### Phase 3 Does NOT Deliver (Follow-Up)

- Coaching gap detection → auto-highlight in graph (uses `HighlightGap` hook)
- Focus session debrief → "communities strengthened" view (uses `PinToFocus` hook)
- Mirror narrative → "View evolution" link with drift animation
- Autotuner `visual_engagement_weight` + `layer_switch_rate` params (30D expansion)
- Autotuner-driven `suggested_preset` learning (returns null in Phase 3)
- Community rename via LLM (fire-and-forget naming for large communities)
- Stability sparkline component (detail panel shows raw numbers, not chart)
- Drag-to-merge backend Louvain recomputation (SuggestMerge creates manual override; Louvain re-detects on next CommunityBuilder cycle)

### Testing Approach

- **Unit tests:** `fabric_graph_base` returns correct structure; `fabric_graph_expand` returns scoped data; `fabric_graph_action` creates notes/links
- **Integration test:** create notes → trigger CommunityBuilder → verify `fabric_graph_base` includes community assignments
- **Frontend:** Vitest for layer toggle state management + graph element transformation
- **E2E:** manual browser testing via `cargo tauri dev` for visual verification of layers, animations, interactions
