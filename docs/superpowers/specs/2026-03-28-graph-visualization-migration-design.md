# Graph Visualization Migration: Cytoscape.js → react-force-graph

**Date:** 2026-03-28
**Status:** Draft
**Scope:** Full replacement of Cytoscape.js graph rendering with react-force-graph (2D) + react-force-graph-3d (3D) in `desktop-ui/src/features/notes/`

## Summary

Replace the Cytoscape.js-based knowledge graph with a hybrid 2D/3D renderer built on `react-force-graph`. The 2D canvas view is the everyday workhorse with custom glow painting and link particles. A "Brain View" toggle switches to a 3D WebGL renderer with true bloom post-processing via Three.js. The migration removes compound node clustering in favor of color + proximity grouping, replaces the D3 minimap with a viewport-rectangle thumbnail, and preserves the progressive BFS reveal system with a pluggable architecture for future cognitive integration.

Pre-release — all breaking changes are acceptable. No migration path needed.

## Motivation

- Cytoscape.js is Canvas-only with no WebGL path — glow/bloom effects require hacks and hit a performance ceiling at ~1K nodes
- Compound node clustering (invisible parent containers) is rigid and fights the organic, biological aesthetic goal
- The reference aesthetic (glowing nodes, particle trails, dense organic clusters on dark backgrounds) requires custom Canvas painting and true post-processing that Cytoscape's stylesheet system cannot deliver
- `react-force-graph` provides built-in link particles, custom Canvas/Three.js rendering, and d3-force physics — all of which align with the existing tech stack (`d3-force` already installed, React components)

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Library | `react-force-graph-2d` + `react-force-graph-3d` | Built-in particles, custom rendering, d3-force physics, React-native |
| Clustering | Color + proximity (custom d3-force attraction) | Organic grouping without rigid compound boundaries; pluggable for semantic communities |
| Drag behavior | Smooth reposition with settle, no continuous springs | Brain-guided layout over manual sculpting; drag emits `onNodeNudge` callback for future cognitive integration |
| Rendering | Hybrid — 2D canvas default + 3D "Brain View" toggle | 2D for daily productivity, 3D for immersive exploration |
| Load animation | Progressive BFS reveal with pluggable wave ordering | Signature "brain waking up" moment; cache-hit fast path preserves instant loads |
| Minimap | Viewport-rectangle thumbnail (replaces D3 minimap) | Spatial orientation without duplicated force simulation |
| Migration | Full replacement (Approach A) | Pre-release, no legacy code paths needed |

## Architecture

### Data Flow

```
useGraphData (notes, notebooks, view mode, hop radius)
  → useGraphElements (flat nodes/links, colors, sizes, clustering)
    → useForceGraph (2D canvas)  ──┐
    → useBrainView (3D WebGL)   ──┤── selected by renderMode setting
    → useWaveReveal (BFS entrance) ┘
    → useGraphPositionCache (persist/restore positions)
    → GraphMinimap (viewport thumbnail from shared positions)
```

### Data Model

```typescript
interface ForceNode {
  id: string;
  label: string;
  color: string;           // notebook-derived color from 12-color cyclic palette
  size: number;             // 18–46px based on linkCount
  linkCount: number;
  tags: string[];
  bodyPreview: string;      // first 2 lines, max 120 chars
  notebookId: string | null;
  clusterId: string;        // "nb:{id}" | "_floating" | "_isolated"
  // d3-force managed (populated after simulation):
  x?: number;
  y?: number;
  z?: number;               // 3D mode only
  fx?: number;              // fixed x (for pinning during reveal/drag)
  fy?: number;
  // Future cognitive enrichment:
  // cognitiveScore?: number;      // 0–1 composite of FSRS stability + relevance
  // lastPromotedAt?: string;      // ISO timestamp for salience-driven wave pulses
}

interface ForceLink {
  source: string;
  target: string;
  weight: number;           // 1.0 default, higher for deduplicated edges
  color: string;            // derived from source node's cluster color
}

type ClusteringMode = 'notebook' | 'semantic' | 'hybrid';

interface GraphElements {
  nodes: ForceNode[];
  links: ForceLink[];
  clusters: ClusterInfo[];  // { id, label, color, count }
  fingerprint: string;      // cache key via graphFingerprint.ts
}
```

**Clustering:** No compound parent nodes. Nodes sharing a `clusterId` get the same color. A custom d3-force applies gentle attraction between same-cluster nodes, creating organic proximity grouping. `ClusteringMode` controls how `clusterId` is computed — `'notebook'` (default) uses notebook membership, `'semantic'` and `'hybrid'` are future modes that use cognitive-layer community detection.

**Edge deduplication:** Multiple links between the same pair → single `ForceLink` with increased `weight` (affects visual thickness and particle count). Same logic as current `useCytoscapeElements`.

## Component Design

### File Structure

```
features/notes/
  components/
    GraphView.tsx              # Rewired orchestrator (same props interface)
    GraphMinimap.tsx            # NEW — viewport-rectangle thumbnail
    GraphBrainView.tsx          # NEW — 3D WebGL renderer wrapper
    GraphToolbar.tsx            # MODIFY — add ClusteringMode switcher
    GraphLegend.tsx             # MODIFY — callback-based highlight (no Cytoscape classes)
    GraphNodeTooltip.tsx        # KEEP as-is
    GraphSettingsPopover.tsx    # MODIFY — add renderMode, revealSpeed, idleRotation
  hooks/
    useForceGraph.ts            # NEW — 2D force-graph instance management
    useGraphElements.ts         # NEW — flat node/link arrays with clustering
    useGraphTheme.ts            # NEW — Canvas paint config + 3D material config
    useWaveReveal.ts            # NEW — BFS progressive reveal controller
    useBrainView.ts             # NEW — 3D renderer + Three.js post-processing
    useGraphData.ts             # KEEP as-is
    useGraphSettings.ts         # MODIFY — add new settings fields
    useGraphPositionCache.ts    # KEEP as-is
  lib/
    graphBfs.ts                 # KEEP as-is
    graphBfs.test.ts            # KEEP as-is
    graphFingerprint.ts         # KEEP as-is
    graphFingerprint.test.ts    # KEEP as-is
    graphPainters.ts            # NEW — Canvas paint functions for 2D
    graphMaterials.ts           # NEW — Three.js materials/geometries for 3D
```

### GraphView.tsx (Orchestrator)

Same props interface as current:

```typescript
interface GraphViewProps {
  notes: Note[];
  notebooks: Notebook[];
  activeNoteId: string | null;
  onSelectNote: (id: string) => void;
  onOpenInEditor?: (id: string) => void;
}
```

Responsibilities:
- Coordinates `useGraphData` → `useGraphElements` → renderer hook pipeline
- Manages smart view mode, hop radius, search, cluster visibility (same as current)
- Conditionally renders `useForceGraph` (2D) or `GraphBrainView` (3D) based on `settings.renderMode`
- Passes `onNodeNudge` callback for future drag-as-teaching-signal integration
- Hosts `GraphMinimap`, `GraphLegend`, `GraphToolbar`, zoom controls, tooltip

### useForceGraph.ts (2D Renderer)

Core hook managing a `force-graph` (2D Canvas) instance.

**Instance lifecycle:**
- Creates `ForceGraph()` on mount, attaches to container ref
- Configures d3-force simulation via `graphRef.d3Force()` accessor:
  - `forceLink` — edge connections with `link.weight` as strength factor
  - `forceManyBody` — repulsion from `settings.repulsion`
  - `forceCenter` — gravity from `settings.centerForce`
  - `forceCollide` — prevent node overlap based on `node.size`
  - Custom cluster force — gentle attraction between same-`clusterId` nodes (strength configurable, implements the "force boost toggle" when `clusteringMode` changes)
- Link distance from `settings.linkDistance`
- Destroys on unmount

**Custom Canvas painting (via `graphPainters.ts`):**
- `nodeCanvasObject` → `paintNode(node, ctx, globalScale)`:
  - Filled circle at `node.size * settings.nodeScale`
  - Soft outer glow ring via `globalCompositeOperation: "screen"` + radial gradient halo
  - Color from `node.color`
  - Label drawn when `globalScale > settings.labelThreshold` (Inter font, positioned right of node, semi-transparent)
- `linkCanvasObject` → `paintLink(link, ctx, globalScale)`:
  - Gradient line using source cluster color
  - Opacity scaled by `link.weight`
- Built-in link particles via props:
  - `linkDirectionalParticles` = `link.weight` (more particles on stronger links)
  - `linkDirectionalParticleSpeed` = 0.005
  - `linkDirectionalParticleColor` = `link.color`

**Interaction events:**
- `onNodeClick(node)` → `onSelectNote(node.id)`
- `onNodeRightClick(node, event)` → `onNodeContext?.(node.id, event.x, event.y)`
- `onNodeHover(node)` → show tooltip + dim non-neighbors (reduce non-neighbor opacity in paint function via a `hoveredNode` ref)
- `onNodeDrag(node)` → update position (built-in d3-force drag behavior)
- `onNodeDragEnd(node)` → `onNodeNudge?.({ nodeId, position, clusterId, timestamp })`
- `onBackgroundClick` → clear hover state

**Neighborhood dimming on hover:**
- Maintain a `hoveredNodeId` ref + `neighborSet` ref (precomputed from links on hover)
- In `paintNode`: if `hoveredNodeId` is set and this node is not the hovered node or in `neighborSet`, draw at reduced opacity (0.12)
- In `paintLink`: if neither endpoint is the hovered node, draw at reduced opacity (0.05)
- On mouseout: clear refs, full opacity restored on next paint cycle

**Settings reactivity:**
- `linkDistance`, `repulsion`, `centerForce` → update via `graphRef.d3Force('link').distance()`, `graphRef.d3Force('charge').strength()`, etc. Then `graphRef.d3ReheatSimulation()`
- `nodeScale`, `labelThreshold` → handled in paint functions (no re-layout)
- `showArrows` → toggle `linkDirectionalArrowLength` prop (0 or 5)
- `showOrphans` → filtered at `useGraphElements` level

**Position cache integration:**
- `onEngineStop` callback → `savePositions(snapshotPositions(graphRef))` where `snapshotPositions` iterates `graphData().nodes` and extracts `{ [id]: { x, y } }`
- On mount with cached positions → set `node.fx = cached.x`, `node.fy = cached.y` on all nodes → render → release pins (`node.fx = undefined`) after first paint so simulation can micro-adjust

**Public API:**
```typescript
interface UseForceGraphReturn {
  graphRef: React.MutableRefObject<ForceGraphInstance | null>;
  runLayout: () => void;           // reheat simulation
  zoomIn: () => void;
  zoomOut: () => void;
  fitToScreen: () => void;
  getViewportBounds: () => ViewportBounds;  // for minimap
  getCurrentPositions: () => PositionMap;
}
```

### useBrainView.ts (3D Renderer)

Manages a `react-force-graph-3d` instance with Three.js post-processing.

**Post-processing pipeline:**
- `EffectComposer` with `RenderPass` + `UnrealBloomPass`
- Bloom settings: strength ~1.5, radius ~0.4, threshold ~0.2
- Optional ambient fog (`THREE.FogExp2`) for depth perception
- Background color: `#07070d`

**Node rendering (`graphMaterials.ts`):**
- `nodeThreeObject(node)` → `SphereGeometry` + `MeshStandardMaterial` with emissive color matching `node.color`
- Size = `node.size * settings.nodeScale`
- Emissive intensity scales with `node.linkCount` (hub nodes glow hotter, drive more bloom)
- Hover: pulse emissive intensity + scale up 1.3× with eased animation

**Labels:**
- `CSS2DRenderer` overlays (not sprites) — always crisp, never pixelated
- Positioned above node center
- Same zoom-adaptive visibility logic: hide when camera distance exceeds threshold
- Font matches 2D (Inter, semi-transparent)

**Link rendering:**
- `LineBasicMaterial` with cluster color, opacity from `link.weight`
- Same directional particles as 2D (`linkDirectionalParticles` prop)

**Interaction:**
- Orbit controls with gentle default speed
- Click/hover callbacks identical to 2D (same `onSelectNote`, `onNodeNudge`)
- Tooltip positioned via `CSS2DRenderer` world-to-screen projection
- Auto-rotate at 0.2°/s when idle (stops on any user interaction). Controlled by `settings.idleRotation` (default true)

**2D ↔ 3D transition:**
- Toggle button in toolbar: "Enter Brain View" / "Exit Brain View"
- On enter: nodes start at their 2D positions with `z = 0`, animate into 3D positions over ~600ms
- Cognitive-aware animation: nodes with higher `linkCount` (larger size) lift first and brighter, following BFS wave order via a micro-`revealWave` call
- Bloom intensity ramps from 0 → full during the 600ms transition
- Camera starts top-down, gently tilts to ~30° as the last animation step
- On exit: reverse — camera flattens, bloom fades, nodes settle to `z = 0`, swap to 2D renderer
- Both renderers share the same `ForceNode[]` positions — transition reads current positions from the outgoing renderer

**`graphMaterials.ts` exports:**
```typescript
function createNodeMaterial(color: string, emissiveIntensity: number): MeshStandardMaterial;
function createNodeGeometry(size: number): SphereGeometry;
function createLinkMaterial(color: string, opacity: number): LineBasicMaterial;
```

### useWaveReveal.ts (Progressive BFS Reveal)

Orchestrates staggered node reveal for both 2D and 3D renderers.

**Public API:**
```typescript
interface WaveRevealController {
  revealWave: (hubId: string, elements: GraphElements,
               cachedPositions?: PositionMap,
               waveOrder?: string[][]) => void;
  triggerMicroReveal: (nodeIds: string[]) => void;
  revealProgress: number;  // 0–1
  isRevealing: boolean;
  cancelReveal: () => void;
}
```

**Cache hit path (fast reveal):**
1. All nodes added to graph data with `fx`/`fy` from cached positions
2. All nodes start hidden (opacity 0, scale 0.3 in paint function)
3. BFS wave order determines reveal timing (from `computeBfsWaves` or custom `waveOrder`)
4. Each wave's nodes transition to full opacity/scale over ~200ms
5. Inter-wave delay controlled by `settings.revealSpeed`:
   - `"instant"` → no animation, render all immediately
   - `"balanced"` → 80ms between waves (default)
   - `"cinematic"` → 150ms between waves
6. Hub node gets a brief pulse effect on wave 0 (brighter glow ring)
7. Max 5 animated waves, remaining nodes batch-revealed
8. After all waves: release `fx`/`fy`, simulation micro-adjusts
9. Save final positions to cache

**Cache miss path (organic layout):**
1. Nodes added wave-by-wave to graph data
2. Each wave triggers simulation with previously placed nodes pinned (`fx`/`fy`)
3. Same inter-wave timing as cache hit path
4. After final wave: release all pins, simulation settles fully
5. `onEngineStop` → save positions to cache

**Micro-reveal (`triggerMicroReveal`):**
- For mid-session updates (new notes, future cognitive promotions)
- Target nodes get a brief opacity/scale pulse animation
- In 3D: emissive intensity ramp + bloom flash
- Does not re-run BFS or modify simulation — purely visual

**Minimap sync:**
- Minimap reads `revealProgress` and syncs dot opacity per wave — dots for unrevealed nodes are dimmed, brightening as their wave fires

### GraphMinimap.tsx (Viewport Thumbnail)

Replaces the current D3-based minimap entirely.

**Rendering:**
- Small `<canvas>` element, fixed size ~180×120px, bottom-left positioning
- All nodes drawn as small colored dots (radius 2–4px based on `node.size`) at their current positions, scaled to fit the canvas bounds
- Links drawn as faint lines (opacity ~0.1) connecting dot positions
- No labels, no glow, no physics simulation
- Dark background matching graph area
- Glass-panel border styling

**Viewport rectangle:**
- Semi-transparent bordered rectangle showing the main view's current viewport bounds
- Updated on every pan/zoom via `getViewportBounds()` from the active renderer hook
- Border color matches the brand accent

**Interaction:**
- Click anywhere on minimap → smooth animated pan in the main view to that location (via `graphRef.centerAt(x, y, 300)`)
- Drag the viewport rectangle → pan main view in real-time

**Reveal sync:**
- During progressive reveal (`isRevealing === true`), minimap dot opacity follows wave progress — unrevealed nodes are dimmed, brightening as their wave fires

**Collapsible:**
- Small toggle button to hide/show minimap
- State persisted in `useGraphSettings`

**Data source:**
- Reads positions directly from `ForceNode[]` array (nodes have `x`/`y` after simulation)
- Repaints on position changes (throttled to ~10fps for performance)
- No separate force simulation

### GraphLegend.tsx (Modified)

**Changes from current:**
- Remove all Cytoscape API calls (`cy.getElementById`, `.children()`, `.connectedEdges()`, `.addClass("dimmed")`)
- Highlight via callback: `onHighlight(clusterId | null)` → parent sets a `highlightedClusterId` state → paint functions check it and dim non-cluster nodes
- Toggle visibility: `onToggleCluster(clusterId)` → parent filters at `useGraphElements` level (same outcome, different mechanism)
- Cluster data unchanged: `ClusterInfo { id, label, color, count }`

### GraphToolbar.tsx (Modified)

**New controls:**
- **ClusteringMode switcher:** Segmented button "Notebook" / "Semantic" in the toolbar. Maps to `settings.clusteringMode`. When switched, `useGraphElements` recomputes `clusterId` values and the custom cluster force updates. Semantic mode is stubbed in v1 — it falls back to notebook clustering with a subtle indicator that semantic communities are not yet available. Once backend delivers community IDs, the switcher activates automatically.

### GraphSettingsPopover.tsx (Modified)

**New settings controls added:**
- **Render Mode:** "2D" / "Brain View" toggle (maps to `settings.renderMode`)
- **Reveal Speed:** "Instant" / "Balanced" / "Cinematic" selector (maps to `settings.revealSpeed`)
- **Idle Rotation:** checkbox, shown only when renderMode is `'3d'` (maps to `settings.idleRotation`)

**Existing controls unchanged:** Link Distance, Repulsion, Center Force, Node Size, Label Threshold, Show Arrows, Show Orphans

### useGraphSettings.ts (Modified)

```typescript
interface GraphSettings {
  // Existing (unchanged)
  linkDistance: number;        // 40–300px (default 120)
  repulsion: number;           // 1000–30000 (default 8000)
  centerForce: number;         // 0–1 (default 0.2)
  nodeScale: number;           // 0.5–2× (default 1)
  labelThreshold: number;      // 0.1–1.5× (default 0.5)
  showArrows: boolean;         // default true
  showOrphans: boolean;        // default true
  livePhysics: boolean;        // default false (kept for potential future use)

  // New (instantLoad removed — replaced by revealSpeed)
  renderMode: '2d' | '3d';              // default '2d'
  revealSpeed: 'instant' | 'balanced' | 'cinematic';  // default 'balanced'
  clusteringMode: 'notebook' | 'semantic';  // default 'notebook' (semantic stubbed in v1)
  idleRotation: boolean;                 // default true (3D only)
  showMinimap: boolean;                  // default true
}
```

Storage key unchanged: `"klynt-graph-settings"` in localStorage. New fields get defaults via spread on load.

## Dependency Changes

### Add

| Package | Purpose |
|---------|---------|
| `react-force-graph-2d` | 2D canvas force-directed graph renderer |
| `react-force-graph-3d` | 3D WebGL force-directed graph renderer |
| `three` | Peer dependency for custom materials + post-processing |
| `@types/three` | TypeScript types for Three.js |

Note: `react-force-graph-3d` bundles `three-forcegraph` which includes Three.js rendering. We need `three` explicitly for custom `MeshStandardMaterial`, `SphereGeometry`, and post-processing (`EffectComposer`, `UnrealBloomPass`). Verify whether `three-stdlib` or the `three/examples/jsm/postprocessing` path is needed for the bloom pass.

### Remove

| Package | Reason |
|---------|--------|
| `cytoscape` | Replaced entirely |
| `cytoscape-cola` | No longer needed (no continuous spring physics) |
| `cytoscape-fcose` | Replaced by d3-force simulation |
| `@types/cytoscape` | TypeScript types for removed library |
| `d3-force` | Verify — `react-force-graph` bundles its own d3-force. Remove if no other consumer exists |
| `@types/d3-force` | Same — remove if `d3-force` is removed |

### Files to Delete

```
hooks/useCytoscapeGraph.ts
hooks/useCytoscapeElements.ts
hooks/useCytoscapeTheme.ts
hooks/useColaPhysics.ts
hooks/useProgressiveReveal.ts
lib/elementDiff.ts
lib/elementDiff.test.ts
lib/graphUtils.ts
```

### Files to Keep Unchanged

```
hooks/useGraphData.ts
hooks/useGraphPositionCache.ts
lib/graphBfs.ts + test
lib/graphFingerprint.ts + test
components/GraphNodeTooltip.tsx
```

## Future Enhancements (Out of Scope)

These are explicitly not part of this migration but the architecture supports them:

- **Semantic clustering mode:** `ClusteringMode = 'semantic'` using LanceDB entity embeddings + community detection. Requires backend IPC to fetch semantic communities. The `clusterId` field and custom cluster force are designed to support this.
- **Cognitive node enrichment:** `cognitiveScore` (FSRS stability + relevance) and `lastPromotedAt` fields on `ForceNode`. Requires new IPC queries joining cognitive tables. Enables salience-driven wave ordering and brightness scaling.
- **Drag-as-teaching-signal backend:** `onNodeNudge` callback is wired but the `AppCore.recordGraphNudge()` handler needs implementation. Feeds Mirror layer / autotuner / situational_relevance.
- **Live cognitive pulses:** `triggerMicroReveal()` is exposed but not yet connected to `BackgroundConsolidationService` or `LiveContextRefresher`.
- **3D idle rotation toward fresh narrative cluster:** Auto-orbit toward Mirror-affected cluster on new weekly narrative. Requires Mirror event subscription.
- **Force boost toggle UI:** "Group by notebook" ↔ "Group by semantic community" button that changes `ClusteringMode` and updates the cluster attraction force. UI and notebook mode work at launch; semantic mode requires backend.
