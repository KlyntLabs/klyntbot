# Brain View Overhaul -- Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the Brain View from an empty notes-only graph into a cognitive-first visualization showing topic nodes (grouped facts), co-activation edges, Louvain communities, and expandable fact satellites -- with the existing notes graph as an opt-in layer.

**Architecture:** Two new Tauri commands (`cognitive_graph_data`, `cognitive_graph_expand_topic`) build the graph from existing `semantic_facts` and `co_activation` tables. A new `useCognitiveGraph` hook transforms the data into `ForceNode`/`ForceLink` arrays. The existing `useGraphElements` hook merges cognitive nodes when the new `layerCognitive` setting is enabled (default: true). Three.js node objects in `useBrainView` get new branch logic for topic/fact/rule node types with glow encoding.

**Tech Stack:** Rust (SQLite queries, Louvain), TypeScript (React, react-force-graph-2d/3d, Three.js), Tauri IPC

**Spec:** `docs/superpowers/specs/2026-04-08-brain-view-overhaul-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/desktop-shared/src/commands/cognitive_graph.rs` | Create | Response types: `CognitiveGraphData`, `TopicNode`, `TopicEdge`, `CognitiveCommunity`, `RuleNode`, `GraphStats`, `TopicDetail`, `FactNode` |
| `crates/desktop-shared/src/commands/mod.rs` | Modify | Export `cognitive_graph` module |
| `crates/app-core/src/handlers/cognitive/graph.rs` | Create | `cognitive_graph_data()` and `cognitive_graph_expand_topic()` handlers |
| `crates/app-core/src/handlers/cognitive/mod.rs` | Modify | Export `graph` module |
| `crates/desktop/src/commands/cognitive.rs` | Modify | Add `cognitive_graph_data` and `cognitive_graph_expand_topic` Tauri commands |
| `desktop-ui/src/features/notes/hooks/useCognitiveGraph.ts` | Create | Hook fetching cognitive graph and transforming to ForceNode/ForceLink |
| `desktop-ui/src/features/notes/hooks/useGraphSettings.ts` | Modify | Add `layerCognitive`, `cognitiveRules`, `cognitiveEpisodes` settings |
| `desktop-ui/src/features/notes/hooks/useGraphElements.ts` | Modify | Add `ForceNodeType` variants, merge cognitive nodes when layer enabled |
| `desktop-ui/src/features/notes/hooks/useBrainView.ts` | Modify | Add topic/fact/rule Three.js node objects with glow encoding |
| `desktop-ui/src/features/notes/components/GraphView.tsx` | Modify | Wire cognitive layer toggle and data flow |

---

### Task 1: Response Types (desktop-shared)

Define the IPC response types for the cognitive graph.

**Files:**
- Create: `crates/desktop-shared/src/commands/cognitive_graph.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`

- [ ] **Step 1: Create the response types module**

Create `crates/desktop-shared/src/commands/cognitive_graph.rs`:

```rust
//! Response types for the cognitive graph visualization.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicNode {
    pub id: String,
    pub subject: String,
    pub domain: String,
    pub fact_count: u32,
    pub avg_convergence: f64,
    pub max_confidence: f64,
    pub total_access_count: i64,
    pub last_accessed: Option<String>,
    pub community_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicEdge {
    pub source_topic_id: String,
    pub target_topic_id: String,
    pub strength: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CognitiveCommunity {
    pub id: String,
    pub name: String,
    pub color: String,
    pub member_topic_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleNode {
    pub id: String,
    pub rule_text: String,
    pub domain: String,
    pub signal_count: i64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStats {
    pub total_facts: u32,
    pub total_topics: u32,
    pub total_edges: u32,
    pub total_communities: u32,
    pub avg_convergence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CognitiveGraphData {
    pub topics: Vec<TopicNode>,
    pub edges: Vec<TopicEdge>,
    pub communities: Vec<CognitiveCommunity>,
    pub rules: Vec<RuleNode>,
    pub stats: GraphStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicDetail {
    pub topic_id: String,
    pub facts: Vec<FactNode>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicExpandParams {
    pub subject: String,
    pub domain: String,
}
```

- [ ] **Step 2: Register in commands/mod.rs**

In `crates/desktop-shared/src/commands/mod.rs`, add:

```rust
pub mod cognitive_graph;
```

- [ ] **Step 3: Build**

```bash
cargo build -p desktop-shared 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add crates/desktop-shared/src/commands/cognitive_graph.rs crates/desktop-shared/src/commands/mod.rs
git commit -m "feat(desktop-shared): add cognitive graph response types

TopicNode, TopicEdge, CognitiveCommunity, RuleNode, GraphStats,
CognitiveGraphData, FactNode, TopicDetail for brain view overhaul."
```

---

### Task 2: Backend Handlers (app-core)

Build the cognitive graph from existing tables.

**Files:**
- Create: `crates/app-core/src/handlers/cognitive/graph.rs`
- Modify: `crates/app-core/src/handlers/cognitive/mod.rs`

- [ ] **Step 1: Create the graph handler module**

Create `crates/app-core/src/handlers/cognitive/graph.rs`:

```rust
//! Cognitive graph handlers for the Brain View overhaul.
//!
//! Builds a topic-level graph from semantic facts and co-activation pairs.
//! Topics are derived by grouping facts by (subject, domain).
//! Community detection uses Louvain on co-activation edges with domain fallback.

use std::collections::HashMap;

use desktop_shared::commands::cognitive_graph::{
    CognitiveCommunity, CognitiveGraphData, FactNode, GraphStats, RuleNode, TopicDetail,
    TopicEdge, TopicNode,
};
use tracing::debug;

use crate::errors::ApiError;
use crate::state::AppCore;

/// Community color palette (8 colors).
const COMMUNITY_COLORS: &[&str] = &[
    "#8b5cf6", "#06b6d4", "#f59e0b", "#ef4444",
    "#22c55e", "#ec4899", "#3b82f6", "#f97316",
];

impl AppCore {
    pub async fn cognitive_graph_data(&self) -> Result<CognitiveGraphData, ApiError> {
        let pool = self.storage_pool().inner().clone();
        let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
        let co_repo = cognitive::CoActivationRepo::new(pool.clone());
        let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());

        // 1. Load all active facts
        let all_facts = fact_repo
            .list_all_active()
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        if all_facts.is_empty() {
            return Ok(CognitiveGraphData {
                topics: Vec::new(),
                edges: Vec::new(),
                communities: Vec::new(),
                rules: Vec::new(),
                stats: GraphStats {
                    total_facts: 0,
                    total_topics: 0,
                    total_edges: 0,
                    total_communities: 0,
                    avg_convergence: 0.0,
                },
            });
        }

        // 2. Group facts by (subject_lower, domain) into topics
        let mut topic_map: HashMap<(String, String), Vec<&cognitive::types::SemanticFact>> =
            HashMap::new();
        for fact in &all_facts {
            let key = (fact.subject.to_lowercase(), fact.domain.clone());
            topic_map.entry(key).or_default().push(fact);
        }

        // 3. Build TopicNodes with aggregates
        let mut topics: Vec<TopicNode> = Vec::with_capacity(topic_map.len());
        let mut fact_to_topic: HashMap<String, String> = HashMap::new();

        for ((subject_lower, domain), facts) in &topic_map {
            let topic_id = format!("topic:{}:{}", subject_lower, domain);
            let fact_count = facts.len() as u32;
            let avg_convergence =
                facts.iter().map(|f| f.convergence_score).sum::<f64>() / facts.len() as f64;
            let max_confidence = facts
                .iter()
                .map(|f| f.confidence)
                .fold(0.0_f64, f64::max);
            let total_access_count: i64 = facts.iter().map(|f| f.access_count).sum();
            let last_accessed = facts
                .iter()
                .filter_map(|f| f.last_accessed.as_ref())
                .max()
                .cloned();

            for fact in facts {
                fact_to_topic.insert(fact.id.clone(), topic_id.clone());
            }

            topics.push(TopicNode {
                id: topic_id,
                subject: facts[0].subject.clone(),
                domain: domain.clone(),
                fact_count,
                avg_convergence,
                max_confidence,
                total_access_count,
                last_accessed,
                community_id: None, // Set after community detection
            });
        }

        // 4. Load co-activation pairs and map to topic-level edges
        let co_pairs = co_repo.list_all_pairs().await.unwrap_or_default();
        let mut topic_edge_map: HashMap<(String, String), f64> = HashMap::new();

        for (fact_a, fact_b, strength) in &co_pairs {
            if let (Some(topic_a), Some(topic_b)) =
                (fact_to_topic.get(fact_a), fact_to_topic.get(fact_b))
            {
                if topic_a != topic_b {
                    let key = if topic_a < topic_b {
                        (topic_a.clone(), topic_b.clone())
                    } else {
                        (topic_b.clone(), topic_a.clone())
                    };
                    *topic_edge_map.entry(key).or_default() += strength;
                }
            }
        }

        let edges: Vec<TopicEdge> = topic_edge_map
            .into_iter()
            .map(|((src, tgt), strength)| TopicEdge {
                source_topic_id: src,
                target_topic_id: tgt,
                strength,
            })
            .collect();

        // 5. Community detection
        let communities = if !edges.is_empty() {
            // Louvain on topic-level edges
            let louvain_edges: Vec<(String, String, f64)> = edges
                .iter()
                .map(|e| {
                    (
                        e.source_topic_id.clone(),
                        e.target_topic_id.clone(),
                        e.strength,
                    )
                })
                .collect();
            let assignment = cognitive::louvain::detect_communities(&louvain_edges);

            // Group topics by community
            let mut community_members: HashMap<usize, Vec<String>> = HashMap::new();
            for (topic_id, &comm_id) in &assignment.assignments {
                community_members
                    .entry(comm_id)
                    .or_default()
                    .push(topic_id.clone());
            }

            // Assign community_id on topic nodes
            for topic in &mut topics {
                if let Some(&comm_id) = assignment.assignments.get(&topic.id) {
                    topic.community_id = Some(format!("cognitive:{comm_id}"));
                }
            }

            // Build community structs with auto-naming
            community_members
                .into_iter()
                .map(|(comm_id, member_ids)| {
                    let name = auto_name_community(&member_ids, &topics);
                    let color =
                        COMMUNITY_COLORS[comm_id % COMMUNITY_COLORS.len()].to_string();
                    CognitiveCommunity {
                        id: format!("cognitive:{comm_id}"),
                        name,
                        color,
                        member_topic_ids: member_ids,
                    }
                })
                .collect()
        } else {
            // Fallback: domain-based grouping
            let mut domain_groups: HashMap<String, Vec<String>> = HashMap::new();
            for topic in &mut topics {
                let comm_id = format!("domain:{}", topic.domain);
                topic.community_id = Some(comm_id.clone());
                domain_groups
                    .entry(comm_id)
                    .or_default()
                    .push(topic.id.clone());
            }

            domain_groups
                .into_iter()
                .enumerate()
                .map(|(i, (comm_id, member_ids))| {
                    let domain_name = comm_id
                        .strip_prefix("domain:")
                        .unwrap_or(&comm_id);
                    let capitalized = capitalize_first(domain_name);
                    CognitiveCommunity {
                        id: comm_id,
                        name: capitalized,
                        color: COMMUNITY_COLORS[i % COMMUNITY_COLORS.len()].to_string(),
                        member_topic_ids: member_ids,
                    }
                })
                .collect()
        };

        // 6. Load procedural rules
        let rules_raw = rule_repo
            .list_all_active()
            .await
            .unwrap_or_default();
        let rules: Vec<RuleNode> = rules_raw
            .iter()
            .map(|r| RuleNode {
                id: r.id.clone(),
                rule_text: r.rule_text.clone(),
                domain: r.domain.clone(),
                signal_count: r.signal_count,
                confidence: r.confidence,
            })
            .collect();

        // 7. Stats
        let total_convergence: f64 = all_facts.iter().map(|f| f.convergence_score).sum();
        let stats = GraphStats {
            total_facts: all_facts.len() as u32,
            total_topics: topics.len() as u32,
            total_edges: edges.len() as u32,
            total_communities: communities.len() as u32,
            avg_convergence: if all_facts.is_empty() {
                0.0
            } else {
                total_convergence / all_facts.len() as f64
            },
        };

        debug!(
            "Cognitive graph: {} topics, {} edges, {} communities from {} facts",
            topics.len(),
            edges.len(),
            communities.len(),
            all_facts.len()
        );

        Ok(CognitiveGraphData {
            topics,
            edges,
            communities,
            rules,
            stats,
        })
    }

    pub async fn cognitive_graph_expand_topic(
        &self,
        subject: &str,
        domain: &str,
    ) -> Result<TopicDetail, ApiError> {
        let pool = self.storage_pool().inner().clone();
        let fact_repo = cognitive::SemanticFactRepo::new(pool);

        let all_domain = fact_repo
            .list_active(domain)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        let subject_lower = subject.to_lowercase();
        let facts: Vec<FactNode> = all_domain
            .into_iter()
            .filter(|f| f.subject.to_lowercase() == subject_lower)
            .map(|f| FactNode {
                id: f.id,
                predicate: f.predicate,
                object: f.object,
                confidence: f.confidence,
                convergence_score: f.convergence_score,
                stability: f.stability,
                access_count: f.access_count,
                source: f.source,
                last_accessed: f.last_accessed,
            })
            .collect();

        Ok(TopicDetail {
            topic_id: format!("topic:{}:{}", subject_lower, domain),
            facts,
        })
    }
}

/// Auto-name a community from the top 2 subjects by fact count.
fn auto_name_community(member_ids: &[String], topics: &[TopicNode]) -> String {
    let mut members: Vec<&TopicNode> = topics
        .iter()
        .filter(|t| member_ids.contains(&t.id))
        .collect();
    members.sort_by(|a, b| b.fact_count.cmp(&a.fact_count));
    match members.len() {
        0 => "Unknown".to_string(),
        1 => members[0].subject.clone(),
        _ => format!("{} & {}", members[0].subject, members[1].subject),
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}
```

- [ ] **Step 2: Add `list_all_pairs` to CoActivationRepo**

The handler needs to load all co-activation pairs. In `crates/cognitive/src/repos/co_activation.rs`, add:

```rust
    /// Load all co-activation pairs (for graph building).
    pub async fn list_all_pairs(&self) -> Result<Vec<(String, String, f64)>, sqlx::Error> {
        let rows: Vec<(String, String, f64)> = sqlx::query_as(
            "SELECT fact_id_a, fact_id_b, strength FROM co_activation ORDER BY strength DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
```

- [ ] **Step 3: Register in cognitive/mod.rs**

In `crates/app-core/src/handlers/cognitive/mod.rs`, add:

```rust
pub mod graph;
```

- [ ] **Step 4: Build and verify**

```bash
cargo build -p app-core 2>&1 | tail -10
```

Fix any import issues. The handler uses `cognitive::SemanticFactRepo`, `cognitive::CoActivationRepo`, `cognitive::ProceduralRuleRepo`, `cognitive::louvain`, and `cognitive::types::SemanticFact` -- verify these are accessible from app-core.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/cognitive/graph.rs crates/app-core/src/handlers/cognitive/mod.rs crates/cognitive/src/repos/co_activation.rs
git commit -m "feat(app-core): add cognitive_graph_data and expand_topic handlers

Builds topic-level graph from semantic_facts + co_activation. Groups
facts by (subject, domain), maps co-activation to topic edges, runs
Louvain community detection with domain fallback. Auto-names communities."
```

---

### Task 3: Tauri Commands

Wire the handlers into Tauri IPC.

**Files:**
- Modify: `crates/desktop/src/commands/cognitive.rs`

- [ ] **Step 1: Add the two commands**

In `crates/desktop/src/commands/cognitive.rs`, add:

```rust
#[tauri::command]
pub async fn cognitive_graph_data(
    state: State<'_, Arc<AppCore>>,
) -> Result<desktop_shared::commands::cognitive_graph::CognitiveGraphData, ApiError> {
    state.cognitive_graph_data().await
}

#[tauri::command]
pub async fn cognitive_graph_expand_topic(
    state: State<'_, Arc<AppCore>>,
    params: desktop_shared::commands::cognitive_graph::TopicExpandParams,
) -> Result<desktop_shared::commands::cognitive_graph::TopicDetail, ApiError> {
    state
        .cognitive_graph_expand_topic(&params.subject, &params.domain)
        .await
}
```

- [ ] **Step 2: Register commands in Tauri app builder**

Find where Tauri commands are registered (likely in `crates/desktop/src/main.rs` or a `setup` function). Add `cognitive_graph_data` and `cognitive_graph_expand_topic` to the `invoke_handler` list.

- [ ] **Step 3: Add to DEV_COMMANDS**

In the same file, add both command names to `DEV_COMMANDS` so the dev server coverage test passes.

- [ ] **Step 4: Add dev server dispatch**

In the dev server dispatch function (same file or `dispatch_dev` function), add POST routes for both commands matching the existing pattern.

- [ ] **Step 5: Build and verify**

```bash
cargo build -p desktop 2>&1 | tail -10
```

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/commands/cognitive.rs
git commit -m "feat(desktop): register cognitive_graph Tauri commands

cognitive_graph_data and cognitive_graph_expand_topic wired into
Tauri invoke handler, DEV_COMMANDS, and dev server dispatch."
```

---

### Task 4: Graph Settings Extension (Frontend)

Add the cognitive layer toggles to GraphSettings.

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useGraphSettings.ts`

- [ ] **Step 1: Add settings fields**

In the `GraphSettings` interface, add after `layerTree`:

```typescript
  layerCognitive: boolean;     // default: true
  cognitiveRules: boolean;     // default: true
  cognitiveEpisodes: boolean;  // default: false
```

Add to the `DEFAULT_SETTINGS` object:

```typescript
  layerCognitive: true,
  cognitiveRules: true,
  cognitiveEpisodes: false,
```

- [ ] **Step 2: Build and verify**

```bash
cd desktop-ui && bun run lint 2>&1 | tail -3
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useGraphSettings.ts
git commit -m "feat(desktop-ui): add cognitive layer toggles to GraphSettings

layerCognitive (default true), cognitiveRules (default true),
cognitiveEpisodes (default false) extend the existing layer system."
```

---

### Task 5: Cognitive Graph Data Hook (Frontend)

Create the hook that fetches cognitive graph data and transforms to ForceNode/ForceLink.

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useCognitiveGraph.ts`
- Modify: `desktop-ui/src/features/notes/hooks/useGraphElements.ts`

- [ ] **Step 1: Create useCognitiveGraph hook**

Create `desktop-ui/src/features/notes/hooks/useCognitiveGraph.ts`:

```typescript
import { useQuery, useMutation } from "@shared/hooks/useIpc";
import type { ForceLink, ForceNode } from "./useGraphElements";

// ── Backend response types ──────────────────────────────────────

interface TopicNode {
  id: string;
  subject: string;
  domain: string;
  factCount: number;
  avgConvergence: number;
  maxConfidence: number;
  totalAccessCount: number;
  lastAccessed: string | null;
  communityId: string | null;
}

interface TopicEdge {
  sourceTopicId: string;
  targetTopicId: string;
  strength: number;
}

interface CognitiveCommunity {
  id: string;
  name: string;
  color: string;
  memberTopicIds: string[];
}

interface RuleNode {
  id: string;
  ruleText: string;
  domain: string;
  signalCount: number;
  confidence: number;
}

interface GraphStats {
  totalFacts: number;
  totalTopics: number;
  totalEdges: number;
  totalCommunities: number;
  avgConvergence: number;
}

interface CognitiveGraphData {
  topics: TopicNode[];
  edges: TopicEdge[];
  communities: CognitiveCommunity[];
  rules: RuleNode[];
  stats: GraphStats;
}

interface FactNode {
  id: string;
  predicate: string;
  object: string;
  confidence: number;
  convergenceScore: number;
  stability: number;
  accessCount: number;
  source: string;
  lastAccessed: string | null;
}

interface TopicDetail {
  topicId: string;
  facts: FactNode[];
}

// ── Transform to ForceNode/ForceLink ────────────────────────────

const MAX_TOPIC_SIZE = 60;
const BASE_TOPIC_SIZE = 18;
const TOPIC_SIZE_PER_FACT = 4;

function topicToForceNode(topic: TopicNode): ForceNode {
  return {
    id: topic.id,
    label: topic.subject,
    color: "", // Set by cluster assignment in useGraphElements
    size: Math.min(BASE_TOPIC_SIZE + topic.factCount * TOPIC_SIZE_PER_FACT, MAX_TOPIC_SIZE),
    linkCount: topic.factCount,
    tags: [topic.domain],
    bodyPreview: `${topic.factCount} facts, convergence ${(topic.avgConvergence * 100).toFixed(0)}%`,
    notebookId: null,
    clusterId: topic.communityId ?? `domain:${topic.domain}`,
    nodeType: "topic",
    expandable: true,
    expanded: false,
  };
}

function ruleToForceNode(rule: RuleNode): ForceNode {
  const truncated =
    rule.ruleText.length > 60 ? `${rule.ruleText.slice(0, 57)}...` : rule.ruleText;
  return {
    id: rule.id,
    label: truncated,
    color: "",
    size: 12,
    linkCount: rule.signalCount,
    tags: [rule.domain],
    bodyPreview: rule.ruleText,
    notebookId: null,
    clusterId: `domain:${rule.domain}`,
    nodeType: "rule",
  };
}

function edgeToForceLink(edge: TopicEdge): ForceLink {
  return {
    source: edge.sourceTopicId,
    target: edge.targetTopicId,
    weight: edge.strength,
    color: "",
  };
}

function factToSatellite(fact: FactNode, parentTopicId: string): ForceNode {
  return {
    id: fact.id,
    label: `${fact.predicate} = ${fact.object}`,
    color: "",
    size: 6 + fact.confidence * 8,
    linkCount: fact.accessCount,
    tags: [],
    bodyPreview: `${fact.source} | stability ${fact.stability.toFixed(1)}`,
    notebookId: null,
    clusterId: parentTopicId,
    nodeType: "fact",
  };
}

// ── Hook ────────────────────────────────────────────────────────

export interface CognitiveGraphResult {
  nodes: ForceNode[];
  links: ForceLink[];
  communities: CognitiveCommunity[];
  stats: GraphStats | null;
  expandTopic: (subject: string, domain: string) => Promise<ForceNode[]>;
  loading: boolean;
}

export function useCognitiveGraph(enabled: boolean): CognitiveGraphResult {
  const { data, isLoading } = useQuery<CognitiveGraphData>(
    "cognitive_graph_data",
    {},
    { enabled },
  );

  const expandMutation = useMutation<TopicDetail>("cognitive_graph_expand_topic");

  const nodes: ForceNode[] = [];
  const links: ForceLink[] = [];

  if (data) {
    // Topic nodes
    for (const topic of data.topics) {
      nodes.push(topicToForceNode(topic));
    }

    // Rule nodes
    for (const rule of data.rules) {
      nodes.push(ruleToForceNode(rule));
    }

    // Co-activation edges
    for (const edge of data.edges) {
      links.push(edgeToForceLink(edge));
    }
  }

  const expandTopic = async (subject: string, domain: string): Promise<ForceNode[]> => {
    const detail = await expandMutation.mutateAsync({ subject, domain });
    return detail.facts.map((f) =>
      factToSatellite(f, `topic:${subject.toLowerCase()}:${domain}`),
    );
  };

  return {
    nodes,
    links,
    communities: data?.communities ?? [],
    stats: data?.stats ?? null,
    expandTopic,
    loading: isLoading,
  };
}
```

- [ ] **Step 2: Add ForceNodeType variants**

In `desktop-ui/src/features/notes/hooks/useGraphElements.ts`, extend the `ForceNodeType` union:

```typescript
export type ForceNodeType =
  | "note"
  | "entity"
  | "tree_section"
  | "tree_text"
  | "finance"
  | "productivity"
  | "okr"
  | "learning"
  | "project"
  | "topic"    // NEW: cognitive topic (grouped facts)
  | "fact"     // NEW: individual fact satellite
  | "rule";    // NEW: procedural rule diamond
```

- [ ] **Step 3: Lint check**

```bash
cd desktop-ui && bun run lint 2>&1 | tail -3
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useCognitiveGraph.ts desktop-ui/src/features/notes/hooks/useGraphElements.ts
git commit -m "feat(desktop-ui): add useCognitiveGraph hook and ForceNode types

Fetches cognitive_graph_data, transforms topics/rules/edges to
ForceNode/ForceLink arrays. Adds topic, fact, rule to ForceNodeType.
Includes expandTopic for satellite fact expansion."
```

---

### Task 6: Wire Cognitive Layer into GraphView

Connect the cognitive data to the existing graph rendering pipeline.

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx`
- Modify: `desktop-ui/src/features/notes/hooks/useGraphElements.ts`

- [ ] **Step 1: Wire useCognitiveGraph in GraphView**

In `GraphView.tsx`, import and call the hook:

```typescript
import { useCognitiveGraph } from "../hooks/useCognitiveGraph";
```

In the component body, after the `useFabricGraph` call, add:

```typescript
const cognitive = useCognitiveGraph(settings.layerCognitive);
```

Pass the cognitive data into `useGraphElements` -- add a new `cognitiveData` parameter to the hook params. The exact integration point depends on how `useGraphElements` assembles its nodes. The cognitive nodes and links should be spread into the same arrays.

- [ ] **Step 2: Merge cognitive nodes in useGraphElements**

In `useGraphElements.ts`, add a `cognitiveData` field to the params interface:

```typescript
interface UseGraphElementsParams {
  nodes: GraphNode[];
  links: GraphLink[];
  notebooks: Notebook[];
  clusteringMode: "notebook" | "semantic";
  fabricData?: FabricData;
  cognitiveData?: {
    nodes: ForceNode[];
    links: ForceLink[];
    communities: Array<{ id: string; name: string; color: string; memberTopicIds: string[] }>;
  };
}
```

In the hook body, after the existing node assembly, add:

```typescript
// Merge cognitive nodes
if (cognitiveData) {
  for (const cn of cognitiveData.nodes) {
    // Find community color
    const comm = cognitiveData.communities.find((c) => c.id === cn.clusterId);
    forceNodes.push({
      ...cn,
      color: comm?.color ?? domainColor(cn.tags[0]) ?? "#8b5cf6",
    });
  }
  for (const cl of cognitiveData.links) {
    forceLinks.push(cl);
  }

  // Add cognitive communities to clusters
  for (const comm of cognitiveData.communities) {
    clusters.push({
      id: comm.id,
      label: comm.name,
      color: comm.color,
      count: comm.memberTopicIds.length,
    });
  }
}
```

- [ ] **Step 3: Add layer toggle UI**

In the GraphView settings panel (find the existing layer toggle section with "Entities" and "Tree" toggles), add a "Cognitive" toggle:

```tsx
<button
  onClick={() => setSettings({ layerCognitive: !settings.layerCognitive })}
  className={`... ${settings.layerCognitive ? "text-brand" : "text-muted-foreground"}`}
>
  <Brain className="size-3.5" />
  <span>Cognitive</span>
</button>
```

Follow the exact same pattern as the existing entity/tree layer toggles.

- [ ] **Step 4: Handle topic expansion click**

In the `onNodeClick` handler, detect topic node clicks:

```typescript
const handleNodeClick = (id: string) => {
  const node = elements.nodes.find((n) => n.id === id);
  if (node?.nodeType === "topic" && node.expandable) {
    // Toggle expansion
    if (node.expanded) {
      // Collapse: remove satellite nodes
      // ... remove facts for this topic from elements
    } else {
      // Expand: fetch and add satellite nodes
      const [, subject, domain] = id.split(":");
      cognitive.expandTopic(subject, domain).then((satellites) => {
        // Add satellites to graph + spring links
        // ... update elements state
      });
    }
    return;
  }
  // Existing note click behavior
  onSelectNote?.(id);
};
```

The exact state management depends on how GraphView manages its elements. If it's via refs to the force graph instance, satellites can be added/removed dynamically. If it's via React state, the node list needs to be augmented.

- [ ] **Step 5: Lint and verify**

```bash
cd desktop-ui && bun run lint 2>&1 | tail -3
```

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphView.tsx desktop-ui/src/features/notes/hooks/useGraphElements.ts
git commit -m "feat(desktop-ui): wire cognitive layer into Brain View graph

Merges cognitive topic nodes and co-activation edges into the
existing force graph. Layer toggle defaults ON. Topic click
expands to show fact satellites."
```

---

### Task 7: 3D Visual Encoding (useBrainView)

Add topic/fact/rule Three.js node objects with glow encoding.

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useBrainView.ts`

- [ ] **Step 1: Add topic node Three.js object**

In the `nodeThreeObject` callback, add cases for the new node types before the default `note` case:

```typescript
case "topic": {
  const geo = new THREE.SphereGeometry(node.size * 0.05 * settings.nodeScale, 16, 16);
  const mat = new THREE.MeshStandardMaterial({
    color: node.color || "#8b5cf6",
    emissive: node.color || "#8b5cf6",
    emissiveIntensity: Math.min(0.3 + (node.bodyPreview?.includes("convergence") ? 
      parseFloat(node.bodyPreview.match(/convergence (\d+)/)?.[1] ?? "0") / 100 * 1.5 : 0.3), 2.0),
    roughness: 0.3,
    metalness: 0.2,
  });
  const mesh = new THREE.Mesh(geo, mat);
  mesh.userData.nodeId = node.id;
  return mesh;
}

case "fact": {
  const geo = new THREE.SphereGeometry(node.size * 0.04 * settings.nodeScale, 12, 12);
  const opacity = Math.max(0.3, Math.min(node.size / 14, 1.0)); // stability-based
  const mat = new THREE.MeshStandardMaterial({
    color: node.color || "#a78bfa",
    emissive: node.color || "#a78bfa",
    emissiveIntensity: 0.3,
    transparent: true,
    opacity,
    roughness: 0.5,
  });
  const mesh = new THREE.Mesh(geo, mat);
  mesh.userData.nodeId = node.id;
  return mesh;
}

case "rule": {
  const geo = new THREE.OctahedronGeometry(6 * settings.nodeScale);
  const mat = new THREE.MeshStandardMaterial({
    color: "#60a5fa", // blue tint
    emissive: "#60a5fa",
    emissiveIntensity: 0.4,
    roughness: 0.3,
  });
  const mesh = new THREE.Mesh(geo, mat);
  mesh.userData.nodeId = node.id;
  return mesh;
}
```

The exact integration depends on how `nodeThreeObject` dispatches on node type -- read the existing switch/if-else and follow the pattern.

- [ ] **Step 2: Lint and verify**

```bash
cd desktop-ui && bun run lint 2>&1 | tail -3
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useBrainView.ts
git commit -m "feat(desktop-ui): add 3D visual encoding for topic/fact/rule nodes

Topics: sphere with convergence-based glow. Facts: translucent
sphere with stability-based opacity. Rules: octahedron (diamond)
with blue tint. All use existing bloom post-processing."
```

---

### Task 8: Full Validation

- [ ] **Step 1: Build workspace**

```bash
cargo build --workspace 2>&1 | tail -10
```

- [ ] **Step 2: Clippy**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | grep "^error" | head -10
```

- [ ] **Step 3: Format (Rust)**

```bash
cargo fmt --all --check
```

If changes: `cargo fmt --all && git add -A && git commit -m "style: format after brain view overhaul"`

- [ ] **Step 4: Lint (Frontend)**

```bash
cd desktop-ui && bun run lint 2>&1 | tail -5
```

If errors: `bun run lint:fix`

- [ ] **Step 5: Run all tests**

```bash
cargo nextest run --workspace --no-fail-fast -E 'not test(smoke) and not test(software_engineer) and not test(agent_validation) and not test(fact_contradiction) and not test(onboarding) and not test(finance_focused) and not test(coaching_persona) and not test(cognitive_llm) and not test(multi_channel)' 2>&1 | grep "Summary"
```

- [ ] **Step 6: E2E verification in browser**

1. Open `localhost:1420`, navigate to Brain View
2. Verify topic nodes appear (from extracted facts)
3. Verify co-activation edges connect related topics
4. Verify community coloring groups related topics
5. Click a topic to expand -- verify fact satellites appear
6. Toggle "Notes" layer on -- verify note nodes appear alongside cognitive graph
7. Verify 3D mode shows glow on high-convergence topics

---

## Summary

| Task | What It Builds | Key Output |
|------|---------------|------------|
| 1 | Response types | `CognitiveGraphData`, `TopicNode`, `TopicEdge`, `FactNode` in desktop-shared |
| 2 | Backend handlers | `cognitive_graph_data()` + `cognitive_graph_expand_topic()` with Louvain + fallback |
| 3 | Tauri commands | IPC wiring + DEV_COMMANDS + dev server |
| 4 | Settings extension | `layerCognitive`, `cognitiveRules`, `cognitiveEpisodes` in GraphSettings |
| 5 | Data hook | `useCognitiveGraph` + ForceNodeType extensions |
| 6 | GraphView wiring | Layer toggle, node merge, topic expansion click |
| 7 | 3D visual encoding | Topic/fact/rule Three.js objects with glow |
| 8 | Full validation | Build, clippy, lint, tests, E2E |
