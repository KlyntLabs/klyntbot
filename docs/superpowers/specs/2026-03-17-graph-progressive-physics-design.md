# Graph Progressive Loading & Interactive Physics

**Date:** 2026-03-17
**Status:** Draft
**Scope:** `desktop-ui/src/features/notes/` — graph visualization system

## Problem Statement

The current graph visualization has three UX issues:

1. **All-at-once loading** — All nodes appear simultaneously, then fCoSE rearranges them. Users see a jarring "explosion" on every graph open.
2. **Full re-layout on any change** — Editing a note, adding a link, or any data mutation triggers `cy.json({ elements })` which replaces all elements and re-runs fCoSE from scratch. Every node moves.
3. **Disconnected-feeling interactions** — After fCoSE finishes, nodes are static. Dragging one node doesn't affect its neighbors. The graph feels like a diagram, not a living system.

**Root cause:** fCoSE is a static layout algorithm that runs to completion. It excels at initial placement (especially with compound nodes) but provides no incremental, interactive, or progressive capabilities.

## Solution: Hybrid fCoSE + Cola Architecture

Use **fCoSE for initial/full layouts** (fast, compound-aware) and **Cola for interactive physics** (incremental, constraint-based). A shared **position cache** bridges both, ensuring seamless transitions with zero position jumps.

This pattern is used by Neo4j Bloom, yEd, and modern Obsidian graph plugins for graphs scaling to 5k–10k+ nodes.

---

## Section 1: Position Cache & Layout Lifecycle

### Cache storage

- **Primary:** IndexedDB (table `graph_positions`, structured for fast key lookup)
- **Fallback:** localStorage for vaults < 800 nodes (instant sync reads)
- **Eviction:** Keep only the 3 most recent view modes per graph; auto-evict oldest

### Cache key

```
key = `${viewMode}-${graphFingerprint}-${leidenVersion}`
```

- `graphFingerprint`: Hash of `sorted(nodeIds) + sorted(edgePairs)` — changes when structure changes
- `leidenVersion`: Timestamp from `graph_communities.last_computed`
- Cache miss or fingerprint mismatch triggers full fCoSE

### Layout lifecycle

```
Graph mounts
  -> Compute fingerprint + leidenVersion
  -> Cache HIT:
      - Apply cached positions via "preset" layout (instant)
      - If user drags -> switch to Cola interactive (seamless handoff)
  -> Cache MISS:
      - Run fCoSE (max 2500 iterations)
      - Snapshot ALL positions (leaf nodes + compound parents)
      - Write to IndexedDB

Node added/removed (or edge change):
  -> Load cached positions for ALL existing nodes (they stay pinned)
  -> Run fCoSE with fixedNodeConstraint on every cached node
  -> Only new nodes + affected compounds get repositioned
  -> Snapshot -> update cache

Cola interactive mode (drag / Live Physics toggle):
  -> Start Cola from current cached positions (no jump)
  -> User drags freely (Cola physics)
  -> When user stops (or exits mode) -> snapshot final positions back to cache
  -> Auto-switch back to fCoSE for next render

Settings change (repulsion, gravity, etc.):
  -> Full fCoSE re-layout (user explicitly requested)
  -> Snapshot -> cache
```

### Compound node handling

- Cache stores both leaf node positions **and** compound parent bounds
- When Leiden communities change -> invalidate only affected view modes (partial clear)

### Invalidation & fallback

- On cache miss or fingerprint mismatch -> show brief "Recomputing layout..." overlay (never blank screen)
- Emergency fallback: always fall back to fCoSE if IndexedDB fails

---

## Section 2: Progressive BFS Loading

### Hub selection (center node)

1. **Active note** — if user navigated from a specific note, that note is the hub
2. **Most connected** — node with the highest link count
3. **Fallback** — first note of the first notebook (alphabetical)

### BFS wave generation (computed once per data load)

```
Wave 0: Hub node
Wave 1: Direct neighbors of hub
Wave 2: Neighbors of wave 1 (excluding already added nodes)
...
Wave N: Remaining connected nodes
Wave N+1: Orphan nodes (no edges) — placed in outer arc
```

### Reveal sequence — two distinct speeds

#### Cache-HIT path (positions already cached — most reopen cases)

- Reveal all waves using a **light staggered fade-in** (no layout run)
- Wave 0: opacity 0->1 + scale 0.7->1.0 at t=0ms
- Wave 1: +80ms delay
- Wave 2: +80ms delay
- Total time ~350ms ("breathing" effect, purely visual)
- Edges only appear when **both endpoints are visible** (no dangling edges)

#### Cache-MISS path (first load or structure change — layout required)

- Wave 0: add hub -> run fCoSE (trivial)
- Wave 1: add neighbors -> fCoSE with hub pinned
- Wave 2: add next ring -> fCoSE with waves 0-1 pinned
- Each wave waits ~150ms settle time before the next wave
- After final wave: snapshot all positions -> save to cache
- Orphans: appear last, arranged loosely in an outer arc

### Performance guard (>= 800 nodes)

- Limit animated waves to **max 5 waves**
- Remaining nodes (wave 6+) appear in a single batch (still stagger opacity)
- Reduce fCoSE iterations progressively:
  - Wave 0-2: 2500
  - Wave 3-4: 1500
  - Batch: 1000
- Disable animation entirely if user enables "Instant load" in Settings

### Visual polish

- Nodes scale from 0.7->1.0 on reveal (subtle "grow" effect)
- Edges fade in smoothly when both connected nodes are visible
- Viewport gently auto-fits after each wave to keep revealed nodes in view
- Subtle pulse effect on the hub node (wave 0) during the first 800ms

---

## Section 3: Interactive Cola Physics (Drag & Live Mode)

### Mode 1: Auto-activate on drag (default, always on)

When user grabs any node:

1. Pause fCoSE (positions already cached)
2. Spin up **scoped Cola** on the dragged node + N-hop neighborhood:
   - N = 2 for graphs < 800 nodes
   - N = 1 for graphs >= 800 (performance guard)
   - Scope capped at viewport + 1-hop buffer (`cy.extent()`)
3. Cola config (tuned for knowledge graphs):
   - `handleDisconnected: false`
   - `fixedNodeConstraint` on every node outside the scope
   - Spring stiffness: 0.04 (gentle pull)
   - Repulsion: derived from user settings
   - Damping: 0.7 (quick settle)
4. During drag: Cola ticks continuously — neighbors drift naturally
5. On release:
   - Cola runs ~300ms settle time
   - Snapshot all moved positions -> update cache
   - Destroy Cola instance (free memory)

### Mode 2: "Live Physics" toggle (prominent toolbar icon)

When enabled:

1. Cola runs continuously on **viewport-visible nodes + 1-hop buffer**
2. Graph gently "breathes" — nodes repel/attract in real time
3. User can drag freely; whole visible clusters respond
4. Performance guard: Cola only ticks nodes inside `cy.extent()` + buffer
5. Auto-pause after 30s idle (fade to static with 200ms transition) — resume instantly on mouse move
6. On disable: snapshot -> cache

### Transition smoothness (critical)

- Cola always initializes from current rendered positions (zero jump)
- When Cola stops, nodes stay exactly where Cola left them
- Position cache remains the single source of truth for both fCoSE and Cola

### Connected-feel tuning (same sliders work for both layouts)

- Spring stiffness: 0.04
- Spring length: derived from `settings.linkDistance`
- Repulsion: derived from `settings.repulsion`
- **Semantic boost:** edges with high semantic similarity (>0.65) or AI-highlighted edges get 1.5x stronger springs

### Edge case: dragging a hub node (10+ connections)

- Cap Cola scope to the 8 strongest connections (by edge weight)
- Remaining neighbors get a lighter "nudge" via position interpolation (no full physics)

### Visual feedback during Cola

- Dragged node gets a subtle halo + scale 1.05
- Neighbor nodes get a soft glow ring while being influenced
- Live Physics toolbar icon glows when active

---

## Section 4: Localized Updates & Edge Change Handling

### Update taxonomy

#### A) Note content edited (no link change)

- `note_links_all` refetch returns same structure -> fingerprint unchanged
- **Nothing happens to the graph.** Zero re-layout. Node label may update in-place.

#### B) Link added/removed between existing nodes

- Fingerprint changes (edge list is part of hash)
- Add/remove the edge element from Cytoscape via `cy.add()` / `cy.remove()` (no `cy.json()`)
- Run **scoped Cola** on the two endpoints + their 1-hop neighbors (~150ms)
- All other nodes stay pinned via cache
- Snapshot affected positions -> update cache

#### C) New note created (node added)

- Fingerprint changes -> cache miss for this specific node only
- Load all cached positions for existing nodes (preset layout)
- Place new node at **average position** of its direct neighbors (weighted by edge strength), offset 120-180px in a random direction
- If orphan, place near center
- Run fCoSE with `fixedNodeConstraint` on all cached nodes
- New node finds its natural position -> snapshot -> cache

#### D) Note deleted (node removed)

- Remove node + its edges from Cytoscape via `cy.remove()`
- Run scoped Cola on the gap area (former neighbors settle inward slightly)
- Update cache (remove deleted node, snapshot new neighbor positions)

#### E) Note moved between notebooks

- Remove from old compound -> add to new compound
- Run scoped Cola only on the two compounds + their 1-hop neighbors
- No full re-layout

#### F) View mode switch (Full -> By Notebook -> By Tag)

- Each view has its own cache key -> independent positions
- If target view has cache -> instant preset + wave fade-in (Section 2 cache-hit path)
- If target view has no cache -> full fCoSE + progressive reveal (Section 2 cache-miss path)
- No cross-contamination between view caches

#### G) Cluster/community membership changes

- Leiden recomputation -> `leidenVersion` in cache key changes -> cache miss
- Full fCoSE re-layout with progressive reveal
- This is rare (only when user triggers community recompute)

### Element diffing strategy (replaces `cy.json({ elements })`)

```
Current elements  ->  diff against  ->  New elements

Added nodes:    cy.add(newNodes)     -> scoped layout for new only
Removed nodes:  cy.remove(oldNodes)  -> scoped settle for neighbors
Added edges:    cy.add(newEdges)     -> scoped Cola on endpoints
Removed edges:  cy.remove(oldEdges)  -> scoped Cola on endpoints
Unchanged:      no-op
```

### Batching

If multiple changes happen within < 200ms (e.g., bulk import or AI auto-linking), batch them into **one** scoped Cola pass. Prevents micro-jitters.

### Visual feedback during update

120ms "settle glow" on affected nodes/edges (same style as Cola drag glow). Users instantly understand "the graph just adjusted."

---

## Files to Create/Modify

### New files

- `desktop-ui/src/features/notes/hooks/useGraphPositionCache.ts` — IndexedDB position cache with fingerprint-based invalidation
- `desktop-ui/src/features/notes/hooks/useColaPhysics.ts` — Cola lifecycle: scoped activation, drag integration, Live Physics mode
- `desktop-ui/src/features/notes/hooks/useProgressiveReveal.ts` — BFS wave computation + staggered reveal animation
- `desktop-ui/src/features/notes/hooks/useElementDiff.ts` — Surgical element diffing (replaces `cy.json()` approach)
- `desktop-ui/src/features/notes/lib/graphFingerprint.ts` — Fingerprint computation from node IDs + edge pairs

### Modified files

- `desktop-ui/src/features/notes/hooks/useCytoscapeGraph.ts` — Core refactor: integrate position cache, Cola handoff, progressive reveal, element diffing. Remove `cy.json({ elements })` full-replace pattern.
- `desktop-ui/src/features/notes/hooks/useCytoscapeElements.ts` — Add fingerprint computation alongside element generation
- `desktop-ui/src/features/notes/hooks/useGraphSettings.ts` — Add `livePhysics` toggle and `instantLoad` setting
- `desktop-ui/src/features/notes/components/GraphView.tsx` — Wire new hooks, add Live Physics toolbar button
- `desktop-ui/src/features/notes/components/GraphSettingsPopover.tsx` — Add "Instant load" toggle

### New dependency

- `cytoscape-cola` — Cola layout extension for Cytoscape.js

---

## Performance Budgets

| Graph size    | Initial load (cache miss) | Reopen (cache hit) | Drag response | Live Physics tick |
|---------------|---------------------------|---------------------|---------------|-------------------|
| < 200 nodes   | < 1.5s                    | < 400ms             | < 16ms        | < 16ms            |
| 200-800 nodes | < 3s                      | < 600ms             | < 16ms        | < 16ms            |
| 800-2000 nodes| < 5s                      | < 800ms             | < 32ms        | < 32ms            |
| 2000+ nodes   | < 8s                      | < 1s                | < 32ms        | N/A (auto-disable) |

---

## Risk & Mitigation

| Risk | Mitigation |
|------|------------|
| Cola performance on large scoped neighborhoods | Cap N-hop to 1 for graphs >= 800; cap hub connections to 8 |
| IndexedDB not available (private browsing) | Fallback to localStorage with 800-node cap |
| fCoSE + Cola position desync | Single position cache as source of truth; both read/write to same store |
| Progressive reveal feels slow on large graphs | Cap at 5 animated waves; "Instant load" escape hatch in settings |
| Compound parent bounds drift after Cola moves children | Re-compute compound bounds on Cola snapshot |
