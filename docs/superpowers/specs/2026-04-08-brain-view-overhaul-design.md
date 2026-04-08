# Brain View Overhaul -- Design Spec

> **Goal:** Transform the Brain View from an empty notes-only graph into a comprehensive cognitive visualization that shows the user's "second brain" -- topics, facts, co-activation relationships, and community clusters -- with the existing notes graph as an opt-in overlay layer.

## Executive Summary

The Brain View currently reads from `note_links` and `entity_tree_links`, both of which are sparsely populated. Meanwhile, the cognitive memory system (SP1-SP3) has built a rich knowledge graph: 10+ semantic facts per conversation, co-activation pairs tracking which facts are retrieved together, convergence scores measuring multi-source confirmation, and procedural rules encoding behavioral patterns. None of this is visible in the Brain View.

This spec redesigns the Brain View to be cognitive-first: topic nodes (grouped facts) connected by co-activation edges, colored by Louvain communities, with size/glow encoding knowledge depth and convergence. The existing notes graph becomes an opt-in layer toggle alongside entities and tree sections.

---

## Architecture

### Data Flow

```
semantic_facts + co_activation + procedural_rules
    |
    v
cognitive_graph_data (Tauri command)
    |
    +---> Group facts by (subject, domain) --> TopicNodes
    +---> Map fact co-activation to topic edges --> TopicEdges
    +---> Run Louvain on topic edges (or domain fallback) --> Communities
    +---> Load active rules --> RuleNodes
    |
    v
Frontend: useGraphElements transforms to ForceNode/ForceLink
    |
    v
react-force-graph-2d / react-force-graph-3d (existing)
```

### Design Principles

- **No new tables** -- reads entirely from existing `semantic_facts`, `co_activation`, `procedural_rules`, and `episodic_memories` tables
- **Derived topic nodes** -- topics are computed at query time by grouping facts, not stored as entities
- **Layer toggle** -- cognitive graph is default, notes/entities/tree are opt-in overlays using existing `GraphSettings` infrastructure
- **Progressive detail** -- topics show aggregates at the top level, individual facts on expansion
- **Graceful degradation** -- works immediately on fresh installs via domain-based fallback clustering

---

## Layer System

| Layer | Default | Content | Data Source |
|-------|---------|---------|-------------|
| **Cognitive** (new) | ON | Topic nodes, co-activation edges, communities, rules | `cognitive_graph_data` |
| **Notes** (existing) | OFF | Note nodes, wiki-link edges | `note_links_all` |
| **Entities** (existing) | OFF | Entity nodes, entity-note edges | `entity_tree_links` |
| **Tree** (existing) | OFF | Section sub-nodes within notes | `book_tree_nodes` |

The default view flips from notes-first to cognitive-first. When a user opens Brain View, they see their knowledge graph -- topics connected by usage patterns.

### Cross-Layer Edges

When both Cognitive and Notes layers are active:
- Facts with atom origins (`source_note_id` on their knowledge atom) draw a faint edge from the topic to the source note
- This shows knowledge provenance without cluttering the default view

### Sub-Toggles Under Cognitive Layer

- **Rules** (default ON): Diamond-shaped nodes connected to their domain's topic cluster
- **Episodes** (default OFF): Small circle nodes with temporal positioning (recent = more visible)

---

## Node Types & Visual Encoding

### Topic Nodes (Collapsed)

A topic node represents all facts sharing the same `(subject, domain)`.

| Property | Encoding |
|----------|----------|
| **Size** | `18 + fact_count * 4` (capped at 60px) |
| **Glow intensity** | `avg_convergence * 2.0` (multi-source topics glow brighter) |
| **Color** | Community color (Louvain or domain fallback) |
| **Label** | Subject name ("Jayden", "Klynt", "Rust") |
| **Badge** | Fact count as small number overlay |
| **Activity ring** | Pulsing ring on topics accessed in last 24h |

### Topic Nodes (Expanded)

On click, a topic expands to show its individual facts as satellites:

| Property | Encoding |
|----------|----------|
| **Topic size** | Shrinks to 70% |
| **Satellite size** | Proportional to `confidence` |
| **Satellite opacity** | Proportional to `stability` (fading facts are visually dim) |
| **Satellite label** | `predicate = object` (e.g., "occupation = software engineer") |
| **Layout** | Spring force keeps satellites near parent topic |

### Co-Activation Edges

Edges between topics represent how often their facts are retrieved together.

| Property | Encoding |
|----------|----------|
| **Width** | `0.5 + strength * 0.3` (capped at 3px) |
| **Opacity** | `0.3 + min(strength / 10, 0.5)` |
| **Particles** | Directional flow on strong edges (strength > 5), reuses `showArrows` |
| **Color** | Source community color at 40% opacity |

### Rule Nodes (Diamond)

| Property | Value |
|----------|-------|
| **Size** | 12px |
| **Shape** | Diamond (custom Three.js geometry) |
| **Color** | Community color with blue tint |
| **Label** | Rule text on hover (truncated to 60 chars) |

### Community Hulls

- Semi-transparent convex hull around community members (reuses existing `convexHull.ts`)
- Color matches community at 10% opacity fill
- Label: auto-generated from top 2 subjects ("Klynt & Rust")

---

## Community Detection

### Primary: Co-Activation Louvain

When `co_activation` table has data (any pairs with strength > 0):

1. Load all co-activation pairs from `co_activation` table
2. Map fact-level pairs to topic-level: for each `(fact_a, fact_b, strength)`, resolve to `(topic_of_a, topic_of_b)` and sum strengths
3. Run existing `louvain::detect_communities()` on the topic-level edge list
4. Auto-name each community from top 2 subjects by fact count (e.g., "Klynt & Rust")
5. Assign colors from a fixed 8-color palette

### Fallback: Domain Grouping

When co-activation is empty (fresh install, no retrievals):

1. Group topics by `domain` field
2. Each domain becomes a community: "Identity", "Work", "Learning", "Finance", "General"
3. Same color palette assignment

### Transition

As the user chats and co-activation accumulates, the graph naturally evolves from static domain clusters to organic usage-based clusters. No explicit switch -- Louvain discovers different communities as edges grow.

### Caching

Community detection result cached with 5-minute TTL. Recomputed on next `cognitive_graph_data` call after expiry.

---

## Backend API

### Command 1: `cognitive_graph_data`

Returns the complete cognitive graph for visualization.

```rust
pub struct CognitiveGraphData {
    pub topics: Vec<TopicNode>,
    pub edges: Vec<TopicEdge>,
    pub communities: Vec<CognitiveCommunity>,
    pub rules: Vec<RuleNode>,
    pub stats: GraphStats,
}

pub struct TopicNode {
    pub id: String,                  // "topic:{subject}:{domain}"
    pub subject: String,
    pub domain: String,
    pub fact_count: u32,
    pub avg_convergence: f64,
    pub max_confidence: f64,
    pub total_access_count: i64,
    pub last_accessed: Option<String>,
    pub community_id: Option<String>,
}

pub struct TopicEdge {
    pub source_topic_id: String,
    pub target_topic_id: String,
    pub strength: f64,               // Summed co-activation between topics
}

pub struct CognitiveCommunity {
    pub id: String,
    pub name: String,                // Auto-generated "Klynt & Rust"
    pub color: String,               // From palette
    pub member_topic_ids: Vec<String>,
}

pub struct RuleNode {
    pub id: String,
    pub rule_text: String,
    pub domain: String,
    pub signal_count: i64,
    pub confidence: f64,
}

pub struct GraphStats {
    pub total_facts: u32,
    pub total_topics: u32,
    pub total_edges: u32,
    pub total_communities: u32,
    pub avg_convergence: f64,
}
```

**Implementation:**

1. `fact_repo.list_all_active()` -- load all active semantic facts
2. Group by `(subject.to_lowercase(), domain)` into topic map
3. For each topic, compute aggregates (fact_count, avg convergence, max confidence, etc.)
4. Load `co_activation` pairs, join with fact->topic mapping, sum strengths per topic pair
5. If co-activation edges exist: run `louvain::detect_communities()` on topic edges
6. Else: group topics by domain as fallback communities
7. Auto-name communities from top subjects
8. Load active procedural rules
9. Return assembled graph

### Command 2: `cognitive_graph_expand_topic`

Returns individual facts for a topic (on user click to expand).

```rust
pub struct TopicDetail {
    pub topic_id: String,
    pub facts: Vec<FactNode>,
}

pub struct FactNode {
    pub id: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub convergence_score: f64,
    pub stability: f64,
    pub access_count: i64,
    pub source: String,
    pub last_accessed: Option<String>,
}
```

**Implementation:** Filter `semantic_facts` by matching `subject` and `domain` from the topic ID.

---

## Frontend Integration

### Graph Settings Extension

Add to existing `GraphSettings` in `useGraphSettings.ts`:

```typescript
interface GraphSettings {
    // ... existing settings ...
    layerCognitive: boolean;   // default: true
    cognitiveRules: boolean;   // default: true (sub-toggle)
    cognitiveEpisodes: boolean; // default: false (sub-toggle)
}
```

### Data Hook: `useCognitiveGraph`

New hook that fetches `cognitive_graph_data` and transforms into `ForceNode[]` / `ForceLink[]`:

- TopicNode -> ForceNode with `nodeType: "topic"`, size/glow computed from aggregates
- TopicEdge -> ForceLink with width/opacity from strength
- CognitiveCommunity -> cluster assignments on ForceNodes
- RuleNode -> ForceNode with `nodeType: "rule"`, diamond geometry

### Integration with `useGraphElements`

The existing `useGraphElements` hook merges nodes/links from all active layers. Add cognitive as a new source:

```
if (settings.layerCognitive) {
    nodes.push(...cognitiveNodes);
    links.push(...cognitiveLinks);
}
```

### Topic Expansion

On topic node click:
1. Call `cognitive_graph_expand_topic(topic_id)`
2. Add satellite `ForceNode`s with `nodeType: "fact"` around the parent
3. Add spring links from each satellite to parent (short `linkDistance`, high strength)
4. Re-run force simulation briefly to settle satellites

On second click or click elsewhere: collapse (remove satellites, restore topic size).

### 3D Visual Enhancements

In `useBrainView.ts`:

- **Topic glow:** Set `emissiveIntensity` proportional to `avg_convergence` on topic meshes
- **Activity ring:** Reuse existing orbiting ring indicator for topics with recent activity
- **Diamond geometry:** Create `OctahedronGeometry` for rule nodes (looks like diamond when scaled)
- **Satellite spring:** Use `d3.forceLink` with short distance and high strength for expanded facts

---

## Interactions

| Action | Target | Behavior |
|--------|--------|----------|
| **Click** | Topic node | Expand/collapse to show fact satellites |
| **Click** | Fact satellite | Show detail panel (predicate, object, confidence, convergence, stability, source) |
| **Click** | Rule node | Show rule text in detail panel |
| **Hover** | Any node | Highlight connected edges and neighbors, dim others |
| **Right-click** | Topic node | Context menu: "Search related", "View all facts" |
| **Drag** | Any node | Reposition in force simulation |

---

## What This Does NOT Include (YAGNI)

- **Episodic memory timeline** -- Episodes are opt-in nodes, not a separate timeline view
- **Real-time graph updates** -- Graph refreshes on open, not via WebSocket push
- **Custom community naming** -- Auto-generated only, no user editing
- **Graph export** -- No PNG/SVG export
- **Fact editing from graph** -- Read-only visualization

---

## Success Criteria

1. **Brain View shows cognitive graph by default** -- Topics visible with co-activation edges on first open
2. **Topic expansion works** -- Click topic, see individual fact satellites with confidence/stability encoding
3. **Communities form** -- Co-activation-based Louvain clusters (or domain fallback for new users)
4. **Glow encodes convergence** -- Multi-source topics visibly brighter than single-source
5. **Notes layer toggle** -- Turning on Notes layer adds note nodes alongside cognitive graph
6. **Rules visible** -- Diamond-shaped rule nodes connected to domain clusters
7. **No empty graph** -- Even with 1 fact, the graph shows something meaningful
