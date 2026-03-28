# Knowledge Fabric Explorer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade the existing graph view into a unified, layered Knowledge Fabric Explorer with community clusters, entity nodes, tree expansion, and live SSE updates.

**Architecture:** Three new Tauri commands (`fabric_graph_base`, `fabric_graph_expand`, `fabric_graph_action`) feed a layered graph UI. The frontend adds layer toggles (Communities, Entities, Tree) to the existing `GraphToolbar`, activates the "Semantic" preset, and renders community convex hulls, entity diamond nodes, and expandable tree sub-nodes using the existing react-force-graph-2d/3d pipeline. SSE events push incremental graph updates with animation hints.

**Tech Stack:** Rust (app-core, desktop-shared, desktop, cognitive crates), TypeScript/React (desktop-ui), react-force-graph-2d/3d, d3-force, Tauri 2 IPC

**Spec:** `docs/superpowers/specs/2026-03-28-knowledge-fabric-explorer-design.md`

---

### Task 1: Shared Types — FabricGraph Request/Response Types

**Files:**
- Create: `crates/desktop-shared/src/commands/fabric.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`

- [ ] **Step 1: Create fabric.rs with all request/response types**

Create `crates/desktop-shared/src/commands/fabric.rs`:

```rust
use serde::{Deserialize, Serialize};

// ── Base snapshot ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricNote {
    pub id: String,
    pub title: String,
    pub notebook_id: Option<String>,
    pub tags: Vec<String>,
    pub body_preview: String,
    pub tree_section_count: u32,
    pub entity_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricLink {
    pub source_id: String,
    pub target_id: String,
    pub link_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricCommunity {
    pub id: String,
    pub name: String,
    pub color: String,
    pub stability: f64,
    pub member_count: u32,
    pub member_note_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricGraphBase {
    pub notes: Vec<FabricNote>,
    pub links: Vec<FabricLink>,
    pub communities: Vec<FabricCommunity>,
    pub suggested_preset: Option<String>,
    pub last_activity_timestamp: String,
    pub live_pulse_active: bool,
}

// ── Expand (layer-specific) ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricExpandParams {
    pub layer: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricEntity {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub mention_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricEntityEdge {
    pub entity_id: String,
    pub note_id: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricEntitiesResponse {
    pub entities: Vec<FabricEntity>,
    pub edges: Vec<FabricEntityEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricTreeNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub node_type: String,
    pub title: Option<String>,
    pub content_preview: String,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricTreeNodesResponse {
    pub note_id: String,
    pub nodes: Vec<FabricTreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricMember {
    pub note_id: String,
    pub tree_node_id: String,
    pub membership_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricCommunityDetail {
    pub community_id: String,
    pub representative_paths: Vec<String>,
    pub top_entities: Vec<String>,
    pub stability_history: Vec<f64>,
    pub members: Vec<FabricMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "layer")]
pub enum FabricExpandResponse {
    #[serde(rename = "entities")]
    Entities(FabricEntitiesResponse),
    #[serde(rename = "tree")]
    Tree(Vec<FabricTreeNodesResponse>),
    #[serde(rename = "community_detail")]
    CommunityDetail(Vec<FabricCommunityDetail>),
}

// ── Actions ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricActionParams {
    pub action: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricActionResponse {
    pub success: bool,
    pub message: Option<String>,
}

// ── SSE graph events ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricGraphEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub node_type: String,
    pub id: String,
    pub data: Option<serde_json::Value>,
    pub animation_hint: String,
    pub intensity: f64,
}
```

- [ ] **Step 2: Add module declaration**

In `crates/desktop-shared/src/commands/mod.rs`, add:

```rust
pub mod fabric;
```

- [ ] **Step 3: Build and verify**

Run: `cargo build -p desktop-shared`

- [ ] **Step 4: Commit**

```bash
git add crates/desktop-shared/src/commands/fabric.rs crates/desktop-shared/src/commands/mod.rs
git commit -m "feat(fabric): add shared request/response types for Knowledge Fabric Explorer"
```

---

### Task 2: AppCore Handler — fabric_graph_base

**Files:**
- Create: `crates/app-core/src/handlers/fabric.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`

- [ ] **Step 1: Create fabric.rs handler module**

Create `crates/app-core/src/handlers/fabric.rs`:

```rust
use std::collections::{HashMap, HashSet};

use desktop_shared::commands::fabric::*;

use crate::errors::ApiError;
use crate::state::AppCore;

/// Community color palette (matches frontend CLUSTER_PALETTE).
const COMMUNITY_COLORS: &[&str] = &[
    "#a78bfa", "#93c5fd", "#6ee7b7", "#fcd34d", "#fca5a5",
    "#f9a8d4", "#a5b4fc", "#67e8f9", "#fdba74", "#86efac",
    "#c4b5fd", "#fde68a",
];

impl AppCore {
    pub async fn fabric_graph_base(&self) -> Result<FabricGraphBase, ApiError> {
        // 1. Load all notes
        let notes_raw = self.note_repo.list_all().await.map_err(|e| {
            ApiError::new("STORAGE", &format!("Failed to load notes: {e}"))
        })?;

        // 2. Load wiki-links
        let links_raw = self.note_repo.get_all_links().await.map_err(|e| {
            ApiError::new("STORAGE", &format!("Failed to load links: {e}"))
        })?;

        // 3. Load communities (if cognitive is available)
        let communities_raw = if let Some(ref pool) = self.cognitive_pool() {
            let repo = cognitive::repos::CommunityRepo::new(pool.clone());
            repo.list_active_communities().await.unwrap_or_default()
        } else {
            vec![]
        };

        // 4. Build community membership map: note_id -> community
        let mut note_community_map: HashMap<String, String> = HashMap::new();
        let mut community_note_ids: HashMap<String, Vec<String>> = HashMap::new();

        for (i, community) in communities_raw.iter().enumerate() {
            if let Some(ref pool) = self.cognitive_pool() {
                let repo = cognitive::repos::CommunityRepo::new(pool.clone());
                if let Ok(members) = repo.get_members(&community.id).await {
                    // Get the source_id (note_id) for each tree node member
                    let tree_repo = cognitive::repos::SqliteBookTreeRepo::new(pool.clone());
                    let mut note_ids = HashSet::new();
                    for member in &members {
                        if let Ok(Some(node)) = tree_repo.get_node(&member.tree_node_id).await {
                            note_ids.insert(node.source_id.clone());
                            note_community_map.insert(node.source_id, community.id.clone());
                        }
                    }
                    community_note_ids.insert(
                        community.id.clone(),
                        note_ids.into_iter().collect(),
                    );
                }
            }
        }

        // 5. Build tree section + entity counts per note
        let mut tree_counts: HashMap<String, u32> = HashMap::new();
        let mut entity_counts: HashMap<String, u32> = HashMap::new();

        if let Some(ref pool) = self.cognitive_pool() {
            let tree_repo = cognitive::repos::SqliteBookTreeRepo::new(pool.clone());
            for note in &notes_raw {
                if let Ok(children) = tree_repo
                    .get_children_by_source("note", &note.id)
                    .await
                {
                    tree_counts.insert(note.id.clone(), children.len() as u32);
                }
            }

            // Entity counts from entity_tree_links joined with tree nodes
            let gt_link_repo = cognitive::repos::SqliteGTLinkRepo::new(pool.clone());
            for note in &notes_raw {
                if let Ok(entities) = gt_link_repo.get_entities_for_source("note", &note.id).await {
                    entity_counts.insert(note.id.clone(), entities.len() as u32);
                }
            }
        }

        // 6. Compute last_activity and live_pulse
        let now = chrono::Utc::now();
        let last_activity = communities_raw
            .iter()
            .filter_map(|c| chrono::DateTime::parse_from_rfc3339(&c.updated_at).ok())
            .max()
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| now.to_rfc3339());

        let five_min_ago = now - chrono::Duration::minutes(5);
        let live_pulse = communities_raw.iter().any(|c| {
            chrono::DateTime::parse_from_rfc3339(&c.updated_at)
                .map(|dt| dt > five_min_ago)
                .unwrap_or(false)
        });

        // 7. Build response
        let notes = notes_raw
            .iter()
            .map(|n| FabricNote {
                id: n.id.clone(),
                title: n.title.clone(),
                notebook_id: n.notebook_id.clone(),
                tags: vec![], // Tags loaded separately if needed
                body_preview: common::truncate_at_boundary(&n.body, 200).to_string(),
                tree_section_count: *tree_counts.get(&n.id).unwrap_or(&0),
                entity_count: *entity_counts.get(&n.id).unwrap_or(&0),
            })
            .collect();

        let links = links_raw
            .iter()
            .map(|l| FabricLink {
                source_id: l.source_id.clone(),
                target_id: l.target_id.clone(),
                link_type: "wiki".to_string(),
            })
            .collect();

        let communities = communities_raw
            .iter()
            .enumerate()
            .map(|(i, c)| FabricCommunity {
                id: c.id.clone(),
                name: c.name.clone(),
                color: COMMUNITY_COLORS[i % COMMUNITY_COLORS.len()].to_string(),
                stability: c.stability,
                member_count: c.member_count as u32,
                member_note_ids: community_note_ids
                    .get(&c.id)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();

        Ok(FabricGraphBase {
            notes,
            links,
            communities,
            suggested_preset: None, // Autotuner integration in follow-up
            last_activity_timestamp: last_activity,
            live_pulse_active: live_pulse,
        })
    }
}
```

Note: The `cognitive_pool()` method and `get_children_by_source` / `get_entities_for_source` may need to be adapted to the actual repo method signatures. Read `crates/cognitive/src/repos/book_tree.rs` and `gt_link.rs` for exact available methods, and use the closest match (e.g., iterate all tree nodes for a source or query by source_id). The pattern above shows the intended data flow — the implementer should adapt method calls to what's actually available.

- [ ] **Step 2: Add module declaration**

In `crates/app-core/src/handlers/mod.rs`, add:

```rust
pub mod fabric;
```

- [ ] **Step 3: Build and verify**

Run: `cargo build -p app-core`

Expected: May need to adjust method calls based on actual repo APIs. Fix compilation errors by reading the actual repo methods.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/fabric.rs crates/app-core/src/handlers/mod.rs
git commit -m "feat(fabric): add fabric_graph_base handler with note/community/entity data"
```

---

### Task 3: AppCore Handler — fabric_graph_expand + fabric_graph_action

**Files:**
- Modify: `crates/app-core/src/handlers/fabric.rs`

- [ ] **Step 1: Add fabric_graph_expand handler**

Append to `crates/app-core/src/handlers/fabric.rs` inside the `impl AppCore` block:

```rust
    pub async fn fabric_graph_expand(
        &self,
        params: FabricExpandParams,
    ) -> Result<FabricExpandResponse, ApiError> {
        match params.layer.as_str() {
            "entities" => {
                let pool = self.cognitive_pool().ok_or_else(|| {
                    ApiError::new("FEATURE_DISABLED", "cognitive not available")
                })?;
                let entity_repo = cognitive::repos::EntityRepo::new(pool.clone());
                let gt_link_repo = cognitive::repos::SqliteGTLinkRepo::new(pool.clone());

                // Load all entities that have tree links
                let entities_raw = entity_repo.list_all_with_tree_links().await.map_err(|e| {
                    ApiError::new("STORAGE", &format!("Failed to load entities: {e}"))
                })?;

                let entities: Vec<FabricEntity> = entities_raw
                    .iter()
                    .map(|e| FabricEntity {
                        id: e.id.clone(),
                        name: e.name.clone(),
                        entity_type: e.entity_type.clone(),
                        mention_count: e.mention_count,
                    })
                    .collect();

                // Load entity-to-note edges via entity_tree_links
                let mut edges = Vec::new();
                for entity in &entities_raw {
                    if let Ok(linked_nodes) = gt_link_repo.get_linked_nodes(&entity.id).await {
                        let mut seen_notes = HashSet::new();
                        for node in linked_nodes {
                            if seen_notes.insert(node.source_id.clone()) {
                                edges.push(FabricEntityEdge {
                                    entity_id: entity.id.clone(),
                                    note_id: node.source_id,
                                    weight: 1.0,
                                });
                            }
                        }
                    }
                }

                Ok(FabricExpandResponse::Entities(FabricEntitiesResponse {
                    entities,
                    edges,
                }))
            }

            "tree" => {
                let pool = self.cognitive_pool().ok_or_else(|| {
                    ApiError::new("FEATURE_DISABLED", "cognitive not available")
                })?;
                let tree_repo = cognitive::repos::SqliteBookTreeRepo::new(pool.clone());

                let mut results = Vec::new();
                for note_id in &params.scopes {
                    let nodes = tree_repo
                        .get_children_recursive_by_source("note", note_id)
                        .await
                        .unwrap_or_default();

                    let fabric_nodes: Vec<FabricTreeNode> = nodes
                        .iter()
                        .map(|n| FabricTreeNode {
                            id: n.id.clone(),
                            parent_id: n.parent_id.clone(),
                            node_type: n.node_type.as_str().to_string(),
                            title: n.title.clone(),
                            content_preview: common::truncate_at_boundary(&n.content, 100)
                                .to_string(),
                            level: n.level,
                        })
                        .collect();

                    results.push(FabricTreeNodesResponse {
                        note_id: note_id.clone(),
                        nodes: fabric_nodes,
                    });
                }

                Ok(FabricExpandResponse::Tree(results))
            }

            "community_detail" => {
                let pool = self.cognitive_pool().ok_or_else(|| {
                    ApiError::new("FEATURE_DISABLED", "cognitive not available")
                })?;
                let community_repo = cognitive::repos::CommunityRepo::new(pool.clone());

                let mut results = Vec::new();
                for community_id in &params.scopes {
                    let community = community_repo.get_community(community_id).await
                        .map_err(|e| ApiError::new("STORAGE", &e.to_string()))?;

                    let Some(community) = community else { continue };

                    let members_raw = community_repo.get_members(community_id).await
                        .unwrap_or_default();

                    let members: Vec<FabricMember> = members_raw
                        .iter()
                        .map(|m| FabricMember {
                            note_id: String::new(), // Filled by tree node lookup
                            tree_node_id: m.tree_node_id.clone(),
                            membership_score: m.membership_score,
                        })
                        .collect();

                    let representative_paths: Vec<String> = community
                        .representative_paths
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default();

                    let top_entities: Vec<String> = community
                        .top_entities
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default();

                    results.push(FabricCommunityDetail {
                        community_id: community_id.clone(),
                        representative_paths,
                        top_entities,
                        stability_history: vec![community.stability],
                        members,
                    });
                }

                Ok(FabricExpandResponse::CommunityDetail(results))
            }

            _ => Err(ApiError::new("VALIDATION", &format!("Unknown layer: {}", params.layer))),
        }
    }

    pub async fn fabric_graph_action(
        &self,
        params: FabricActionParams,
    ) -> Result<FabricActionResponse, ApiError> {
        match params.action.as_str() {
            "create_bridge_note" => {
                let source = params.payload.get("sourceCommunityId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let target = params.payload.get("targetCommunityId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let content = params.payload.get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let title = format!("Bridge: {} ↔ {}", source, target);
                let body = if content.is_empty() {
                    format!("# Bridge Note\n\nConnecting communities {} and {}.", source, target)
                } else {
                    content.to_string()
                };

                let row = feature_notes::models::NoteRow {
                    id: uuid::Uuid::new_v4().to_string(),
                    notebook_id: None,
                    title,
                    body,
                    body_html: None,
                    body_json: None,
                    pinned: 0,
                    archived: 0,
                    icon: None,
                    color: None,
                    embedding_updated_at: None,
                    split_content: None,
                    split_mode: None,
                    perspective_config: None,
                    last_visited_at: None,
                    created_at: feature_notes::repo::utc_now_str(),
                    updated_at: feature_notes::repo::utc_now_str(),
                };

                self.note_repo.create_note(&row).await.map_err(|e| {
                    ApiError::new("STORAGE", &format!("Failed to create bridge note: {e}"))
                })?;

                // Publish NoteContentChanged to trigger tree building
                if let Ok(bus) = self.domain_event_bus() {
                    bus.publish(bus::DomainEvent::NoteContentChanged {
                        note_id: row.id.clone(),
                        content: row.body.clone(),
                    });
                }

                Ok(FabricActionResponse {
                    success: true,
                    message: Some(format!("Bridge note created: {}", row.id)),
                })
            }

            "link_entity" => {
                let entity_id = params.payload.get("entityId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ApiError::new("VALIDATION", "entityId required"))?;
                let note_id = params.payload.get("noteId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ApiError::new("VALIDATION", "noteId required"))?;

                if let Some(ref pool) = self.cognitive_pool() {
                    let gt_link_repo = cognitive::repos::SqliteGTLinkRepo::new(pool.clone());
                    gt_link_repo.link(entity_id, note_id).await.map_err(|e| {
                        ApiError::new("STORAGE", &format!("Failed to link entity: {e}"))
                    })?;
                }

                Ok(FabricActionResponse {
                    success: true,
                    message: Some("Entity linked".to_string()),
                })
            }

            "suggest_merge" | "pin_to_focus" | "highlight_gap" => {
                // Hooks — functional stubs for follow-up phases
                Ok(FabricActionResponse {
                    success: true,
                    message: Some(format!("Action '{}' acknowledged (hook)", params.action)),
                })
            }

            _ => Err(ApiError::new("VALIDATION", &format!("Unknown action: {}", params.action))),
        }
    }
```

Note: The `cognitive_pool()` helper may not exist yet on `AppCore`. If not, use `self.storage_pool.inner().clone()` to get the `SqlitePool`. Check `crates/app-core/src/state.rs` for the actual field name. The repos need a raw `SqlitePool`, which you can get from `StoragePool::inner()`.

- [ ] **Step 2: Build and verify**

Run: `cargo build -p app-core`

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/handlers/fabric.rs
git commit -m "feat(fabric): add fabric_graph_expand and fabric_graph_action handlers"
```

---

### Task 4: Tauri Commands + Dev Server Routes

**Files:**
- Create: `crates/desktop/src/commands/fabric.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Create Tauri command adapters**

Create `crates/desktop/src/commands/fabric.rs`:

```rust
use std::sync::Arc;

use app_core::AppCore;
use desktop_shared::commands::fabric::*;
use tauri::State;

use crate::errors::ApiError;

#[tauri::command]
pub async fn fabric_graph_base(
    state: State<'_, Arc<AppCore>>,
) -> Result<FabricGraphBase, ApiError> {
    state.fabric_graph_base().await
}

#[tauri::command]
pub async fn fabric_graph_expand(
    state: State<'_, Arc<AppCore>>,
    params: FabricExpandParams,
) -> Result<FabricExpandResponse, ApiError> {
    state.fabric_graph_expand(params).await
}

#[tauri::command]
pub async fn fabric_graph_action(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FabricActionParams,
) -> Result<FabricActionResponse, ApiError> {
    let result = state.fabric_graph_action(params).await?;
    // Bridge note creation may trigger entity updates
    if result.success {
        super::emit_updates(&app, &[]);
    }
    Ok(result)
}

pub const DEV_COMMANDS: &[&str] = &[
    "fabric_graph_base",
    "fabric_graph_expand",
    "fabric_graph_action",
];
```

- [ ] **Step 2: Register module and commands**

In `crates/desktop/src/commands/mod.rs`, add:

```rust
pub mod fabric;
```

Also add the fabric commands to the Tauri builder's `invoke_handler` (find the `.invoke_handler(tauri::generate_handler![...])` call and add `fabric::fabric_graph_base`, `fabric::fabric_graph_expand`, `fabric::fabric_graph_action`).

- [ ] **Step 3: Add dev server dispatch**

In the dev server dispatch chain, add handling for the 3 new commands. Follow the same pattern as existing command dispatchers — check how other `dispatch_dev` functions are structured in `crates/desktop/src/dev_server/`.

- [ ] **Step 4: Build full workspace**

Run: `cargo build --workspace`

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/commands/fabric.rs crates/desktop/src/commands/mod.rs crates/desktop/src/dev_server/
git commit -m "feat(fabric): add Tauri commands and dev server routes for fabric graph"
```

---

### Task 5: SSE Event Forwarding for Fabric Graph

**Files:**
- Modify: `crates/desktop/src/dev_server/streaming.rs`
- Modify: `crates/desktop/src/app_core.rs` (if Tauri events needed)

- [ ] **Step 1: Add FabricGraphEvent SSE forwarding in dev server**

In `crates/desktop/src/dev_server/streaming.rs`, find where `DomainEvent` variants are handled in the SSE stream. Add cases for community events:

```rust
DomainEvent::CommunityDiscovered { .. } => {
    let event = serde_json::json!({
        "type": "fabric_graph",
        "event": {
            "type": "node_added",
            "nodeType": "community",
            "id": "", // Extract from event if available
            "animationHint": "pulse",
            "intensity": 0.8
        }
    });
    // Send via SSE
}
```

Note: The actual `DomainEvent` variants for community events are `CommunityDiscovered`, `CommunityUpdated`, `CommunityWeakened` — but these are `ContextUpdateReason` variants, not `DomainEvent` variants. Check the bus crate for the exact event types available. You may need to add new `DomainEvent` variants or subscribe to the `ContextUpdateQueue` instead.

Read `crates/bus/src/domain_events.rs` for available `DomainEvent` variants and `crates/bus/src/context_updates.rs` for `ContextUpdateReason` variants. The community events (`CommunityDiscovered`, `CommunityUpdated`, `CommunityWeakened`) are in `ContextUpdateReason`, not `DomainEvent`. You may need to either:
1. Add new `DomainEvent` variants for fabric graph events, or
2. Forward `ContextUpdate` items through the SSE channel

Choose the approach that's most consistent with the existing codebase.

- [ ] **Step 2: Build and verify**

Run: `cargo build --workspace`

- [ ] **Step 3: Commit**

```bash
git add crates/desktop/src/dev_server/streaming.rs
git commit -m "feat(fabric): add SSE forwarding for fabric graph events"
```

---

### Task 6: Frontend Types + Fabric Data Hook

**Files:**
- Create: `desktop-ui/src/shared/types/fabric.ts`
- Create: `desktop-ui/src/features/notes/hooks/useFabricGraph.ts`

- [ ] **Step 1: Create TypeScript types**

Create `desktop-ui/src/shared/types/fabric.ts`:

```typescript
export interface FabricNote {
  id: string;
  title: string;
  notebookId: string | null;
  tags: string[];
  bodyPreview: string;
  treeSectionCount: number;
  entityCount: number;
}

export interface FabricLink {
  sourceId: string;
  targetId: string;
  linkType: string;
}

export interface FabricCommunity {
  id: string;
  name: string;
  color: string;
  stability: number;
  memberCount: number;
  memberNoteIds: string[];
}

export interface FabricGraphBase {
  notes: FabricNote[];
  links: FabricLink[];
  communities: FabricCommunity[];
  suggestedPreset: string | null;
  lastActivityTimestamp: string;
  livePulseActive: boolean;
}

export interface FabricEntity {
  id: string;
  name: string;
  entityType: string;
  mentionCount: number;
}

export interface FabricEntityEdge {
  entityId: string;
  noteId: string;
  weight: number;
}

export interface FabricTreeNode {
  id: string;
  parentId: string | null;
  nodeType: string;
  title: string | null;
  contentPreview: string;
  level: number;
}

export interface FabricMember {
  noteId: string;
  treeNodeId: string;
  membershipScore: number;
}

export interface FabricCommunityDetail {
  communityId: string;
  representativePaths: string[];
  topEntities: string[];
  stabilityHistory: number[];
  members: FabricMember[];
}

export interface FabricGraphEvent {
  type: string;
  nodeType: string;
  id: string;
  data?: Record<string, unknown>;
  animationHint: string;
  intensity: number;
}

export type FabricLayer = "communities" | "entities" | "tree";
```

- [ ] **Step 2: Create useFabricGraph hook**

Create `desktop-ui/src/features/notes/hooks/useFabricGraph.ts`:

```typescript
import { useCallback, useState } from "react";
import { useQuery } from "@shared/hooks/useQuery";
import { ipc } from "@shared/hooks/useIpc";
import type {
  FabricGraphBase,
  FabricEntity,
  FabricEntityEdge,
  FabricTreeNode,
  FabricCommunityDetail,
  FabricLayer,
} from "@shared/types/fabric";

interface FabricState {
  base: FabricGraphBase | null;
  entities: FabricEntity[];
  entityEdges: FabricEntityEdge[];
  expandedTrees: Map<string, FabricTreeNode[]>;
  communityDetails: Map<string, FabricCommunityDetail>;
}

interface UseFabricGraphResult {
  fabric: FabricState;
  loading: boolean;
  expandLayer: (layer: FabricLayer, scopes?: string[]) => Promise<void>;
  collapseTree: (noteId: string) => void;
  performAction: (action: string, payload: Record<string, unknown>) => Promise<void>;
}

export function useFabricGraph(enabled: boolean): UseFabricGraphResult {
  const { data: base, loading } = useQuery<FabricGraphBase>(
    "fabric_graph_base",
    undefined,
    [],
    { enabled },
  );

  const [entities, setEntities] = useState<FabricEntity[]>([]);
  const [entityEdges, setEntityEdges] = useState<FabricEntityEdge[]>([]);
  const [expandedTrees, setExpandedTrees] = useState<Map<string, FabricTreeNode[]>>(new Map());
  const [communityDetails, setCommunityDetails] = useState<
    Map<string, FabricCommunityDetail>
  >(new Map());

  const expandLayer = useCallback(
    async (layer: FabricLayer, scopes: string[] = []) => {
      const result = await ipc("fabric_graph_expand", {
        params: { layer, scopes },
      });

      if (layer === "entities" && result?.entities) {
        setEntities(result.entities);
        setEntityEdges(result.edges ?? []);
      } else if (layer === "tree" && Array.isArray(result)) {
        setExpandedTrees((prev) => {
          const next = new Map(prev);
          for (const item of result) {
            next.set(item.noteId, item.nodes);
          }
          return next;
        });
      } else if (layer === "community_detail" && Array.isArray(result)) {
        setCommunityDetails((prev) => {
          const next = new Map(prev);
          for (const item of result) {
            next.set(item.communityId, item);
          }
          return next;
        });
      }
    },
    [],
  );

  const collapseTree = useCallback((noteId: string) => {
    setExpandedTrees((prev) => {
      const next = new Map(prev);
      next.delete(noteId);
      return next;
    });
  }, []);

  const performAction = useCallback(
    async (action: string, payload: Record<string, unknown>) => {
      await ipc("fabric_graph_action", { params: { action, payload } });
    },
    [],
  );

  return {
    fabric: {
      base: base ?? null,
      entities,
      entityEdges,
      expandedTrees,
      communityDetails,
    },
    loading,
    expandLayer,
    collapseTree,
    performAction,
  };
}
```

- [ ] **Step 3: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/shared/types/fabric.ts desktop-ui/src/features/notes/hooks/useFabricGraph.ts
git commit -m "feat(fabric-ui): add TypeScript types and useFabricGraph data hook"
```

---

### Task 7: Graph Settings — Layer Toggles

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useGraphSettings.ts`

- [ ] **Step 1: Add layer fields to GraphSettings**

In `desktop-ui/src/features/notes/hooks/useGraphSettings.ts`, extend the `GraphSettings` interface:

```typescript
export interface GraphSettings {
  // ... existing fields ...
  layerCommunities: boolean;  // false default
  layerEntities: boolean;     // false default
  layerTree: boolean;         // false default
}
```

Update the `DEFAULT_SETTINGS` constant to include:

```typescript
layerCommunities: false,
layerEntities: false,
layerTree: false,
```

- [ ] **Step 2: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useGraphSettings.ts
git commit -m "feat(fabric-ui): add layer toggle settings to GraphSettings"
```

---

### Task 8: GraphToolbar — Layer Toggle Buttons + Semantic Preset

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphToolbar.tsx`

- [ ] **Step 1: Add layer toggles and Semantic preset**

Read the current `GraphToolbar.tsx` to understand the existing structure. Add:

1. A "Layers" section after the existing smart view pills with 3 toggle buttons:
   - Communities (Network icon from lucide-react)
   - Entities (Atom icon)
   - Tree (TreePine icon)

2. Enable the "Semantic" smart view option (currently disabled). When clicked, it should:
   - Set `layerCommunities: true` and `layerEntities: true`
   - Set `clusteringMode: "semantic"`

Add new props to `GraphToolbarProps`:

```typescript
interface GraphToolbarProps {
  // ... existing props ...
  layerCommunities: boolean;
  layerEntities: boolean;
  layerTree: boolean;
  onLayerToggle: (layer: "communities" | "entities" | "tree") => void;
}
```

Add keyboard shortcuts: press `1` toggles communities, `2` toggles entities, `3` toggles tree, `S` applies semantic preset. These should be handled via `useEffect` with `keydown` event listener in `GraphView.tsx` (not in the toolbar itself).

- [ ] **Step 2: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphToolbar.tsx
git commit -m "feat(fabric-ui): add layer toggle buttons and Semantic preset to GraphToolbar"
```

---

### Task 9: GraphView — Wire Layers + Fabric Data

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx`
- Modify: `desktop-ui/src/features/notes/hooks/useGraphElements.ts`

- [ ] **Step 1: Wire useFabricGraph into GraphView**

In `GraphView.tsx`:
1. Import and call `useFabricGraph(anyLayerEnabled)`
2. Add layer toggle handler that calls `expandLayer` when a layer is turned on
3. Pass layer state + fabric data to `useGraphElements`
4. Add keyboard shortcut listener for `1`, `2`, `3`, `S`, `F`, `Escape`

- [ ] **Step 2: Extend useGraphElements to handle fabric nodes**

In `useGraphElements.ts`, extend the transformation logic:

When `layerCommunities` is on and fabric communities are available:
- Use `community_id` as `clusterId` for notes (instead of notebook_id)
- Use community color for node color
- Add community label nodes (type: "community_label") at cluster centroids

When `layerEntities` is on and fabric entities are loaded:
- Add entity nodes (type: "entity") with diamond shape indicator
- Add entity-to-note links with thinner weight

When `layerTree` is on and a note is expanded:
- Add tree section nodes (type: "tree_section") as children of the note node
- Add tree text nodes (type: "tree_text") as smaller dots
- Add parent-child links within the tree

Extend `ForceNode` with a `nodeType` field:

```typescript
export interface ForceNode {
  // ... existing fields ...
  nodeType: "note" | "community_label" | "entity" | "tree_section" | "tree_text";
  expandable?: boolean; // true for notes when tree layer is on
  expanded?: boolean;   // true when tree is showing sub-nodes
}
```

- [ ] **Step 3: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphView.tsx desktop-ui/src/features/notes/hooks/useGraphElements.ts
git commit -m "feat(fabric-ui): wire fabric data + layer state into graph elements pipeline"
```

---

### Task 10: Canvas Painting — Entity Diamonds + Tree Nodes + Community Hulls

**Files:**
- Modify: `desktop-ui/src/features/notes/lib/graphPainters.ts`
- Modify: `desktop-ui/src/features/notes/hooks/useForceGraph.ts`

- [ ] **Step 1: Extend paintNode for new node types**

In `graphPainters.ts`, extend `paintNode` to handle new `nodeType` values:

- `"entity"`: Draw a diamond shape (rotated square) instead of circle. Size 12-24px. Use accent color.
- `"tree_section"`: Draw smaller circle (10-16px based on level). Use parent note color at 60% opacity.
- `"tree_text"`: Draw tiny dot (6px). Parent note color at 30% opacity. No label (show on hover only).
- `"community_label"`: Draw rounded rectangle with community name + member count. Background: community color at 20% opacity.

- [ ] **Step 2: Add community convex hull rendering**

In `useForceGraph.ts`, add a custom canvas callback that draws convex hull outlines around community clusters. This should:
1. After each simulation tick, collect positions of all nodes in each community
2. Compute convex hull polygon for each community
3. Draw dashed outline (community color at 15% opacity) around each hull
4. Position community label node at centroid

Use the existing `onRenderFramePost` or `onEngineStop` callback pattern from react-force-graph.

- [ ] **Step 3: Update community clustering force**

In `useForceGraph.ts`, modify the `clusterAttractionForce`:
- When `clusteringMode === "semantic"`, use `community_id` from the fabric data as the cluster key instead of `notebook_id`
- Keep the existing attraction strength (0.03)

- [ ] **Step 4: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/lib/graphPainters.ts desktop-ui/src/features/notes/hooks/useForceGraph.ts
git commit -m "feat(fabric-ui): add entity diamond, tree node, and community hull rendering"
```

---

### Task 11: Tree Expansion Interaction

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx`

- [ ] **Step 1: Add expand/collapse interaction**

In `GraphView.tsx`, add expand/collapse behavior:

1. When Tree layer is on, note nodes show a small expand icon (ChevronRight)
2. Clicking the expand area calls `expandLayer("tree", [noteId])` and marks the node as `expanded: true`
3. Sub-nodes spring-animate outward from the parent (d3-force handles this naturally — new nodes added with position near parent will spread out)
4. Clicking again (or pressing Space on selected node) calls `collapseTree(noteId)` and removes sub-nodes
5. Expanded note node becomes slightly transparent (opacity: 0.7)

The expand icon rendering goes in `paintNode` — draw a small `>` or `v` icon at the right edge of note nodes when `expandable: true`.

- [ ] **Step 2: Add hover mini-preview**

When hovering any note node (regardless of Tree layer state), show a tooltip with first 2-3 headings extracted from `bodyPreview`. Use the existing `GraphNodeTooltip` component pattern.

- [ ] **Step 3: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphView.tsx
git commit -m "feat(fabric-ui): add tree expand/collapse interaction with hover preview"
```

---

### Task 12: Click + Double-Click Interactions

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useForceGraph.ts`
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx`

- [ ] **Step 1: Extend click handlers for new node types**

In `useForceGraph.ts`, extend the `onNodeClick` handler:

- `"note"`: Existing behavior (select + preview). If Communities layer on, also highlight same-community siblings.
- `"community_label"`: Highlight all member nodes. Show community summary in preview panel.
- `"entity"`: Highlight all connected notes with accent edge color. Show entity details in panel.
- `"tree_section"`: Select, show content in panel. Highlight parent note + siblings.

Extend `onNodeDoubleClick`:

- `"note"`: Open in editor (existing).
- `"community_label"`: Zoom-to-fit community members.
- `"entity"`: Open chat with pre-filled query: `"Tell me about [entity name] across my notes"`.
- `"tree_section"`: Open note in editor, scrolled to heading.

- [ ] **Step 2: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useForceGraph.ts desktop-ui/src/features/notes/components/GraphView.tsx
git commit -m "feat(fabric-ui): extend click and double-click handlers for all node types"
```

---

### Task 13: Right-Click Context Menu

**Files:**
- Create: `desktop-ui/src/features/notes/components/GraphContextMenu.tsx`
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx`

- [ ] **Step 1: Create GraphContextMenu component**

Create `desktop-ui/src/features/notes/components/GraphContextMenu.tsx`:

```tsx
import type { ForceNode } from "@features/notes/hooks/useGraphElements";

interface Props {
  node: ForceNode | null;
  position: { x: number; y: number };
  onClose: () => void;
  onOpenInEditor: (id: string) => void;
  onExpandTree: (id: string) => void;
  onFindRelated: (query: string) => void;
  onQuickBridge: (node: ForceNode) => void;
  onPinToFocus: (communityId: string) => void;
}

export function GraphContextMenu({
  node,
  position,
  onClose,
  onOpenInEditor,
  onExpandTree,
  onFindRelated,
  onQuickBridge,
  onPinToFocus,
}: Props) {
  if (!node) return null;

  const items = getMenuItems(node);

  return (
    <div
      className="glass-panel fixed z-50 min-w-[180px] rounded-lg border border-border/50 p-1 text-xs shadow-lg"
      style={{ left: position.x, top: position.y }}
    >
      {items.map((item) => (
        <button
          key={item.label}
          type="button"
          className="flex w-full items-center gap-2 rounded px-2.5 py-1.5 text-left text-foreground hover:bg-surface-raised/50"
          onClick={() => {
            item.action();
            onClose();
          }}
        >
          {item.label}
        </button>
      ))}
    </div>
  );

  function getMenuItems(n: ForceNode) {
    switch (n.nodeType) {
      case "note":
        return [
          { label: "Open in editor", action: () => onOpenInEditor(n.id) },
          { label: "Expand tree", action: () => onExpandTree(n.id) },
          { label: "Find related", action: () => onFindRelated(n.label) },
          { label: "Quick bridge", action: () => onQuickBridge(n) },
        ];
      case "community_label":
        return [
          { label: "Focus on community", action: () => {} },
          { label: "Pin to focus", action: () => onPinToFocus(n.id) },
          { label: "Quick bridge", action: () => onQuickBridge(n) },
          { label: "View members", action: () => {} },
        ];
      case "entity":
        return [
          { label: "Find across notes", action: () => onFindRelated(n.label) },
          { label: "Open references", action: () => {} },
          { label: "Hide from graph", action: () => {} },
        ];
      case "tree_section":
        return [
          { label: "Open in editor at heading", action: () => onOpenInEditor(n.id) },
          { label: "Create flashcard", action: () => {} },
        ];
      default:
        return [
          { label: "Fit to screen", action: () => {} },
        ];
    }
  }
}
```

- [ ] **Step 2: Wire into GraphView**

In `GraphView.tsx`, add right-click handler that opens `GraphContextMenu` at cursor position. Add click-outside handler to close.

- [ ] **Step 3: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphContextMenu.tsx desktop-ui/src/features/notes/components/GraphView.tsx
git commit -m "feat(fabric-ui): add right-click context menu for all node types"
```

---

### Task 14: Quick Bridge Popover + Drag-to-Merge

**Files:**
- Create: `desktop-ui/src/features/notes/components/QuickBridgePopover.tsx`
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx`

- [ ] **Step 1: Create QuickBridgePopover**

Create `desktop-ui/src/features/notes/components/QuickBridgePopover.tsx`:

```tsx
import { useState } from "react";

interface Props {
  sourceName: string;
  targetName: string;
  onClose: () => void;
  onCreateNote: (title: string, content: string) => void;
}

export function QuickBridgePopover({ sourceName, targetName, onClose, onCreateNote }: Props) {
  const [content, setContent] = useState("");
  const title = `Bridge: ${sourceName} ↔ ${targetName}`;

  return (
    <div className="glass-panel fixed left-1/2 top-1/3 z-50 w-[400px] -translate-x-1/2 rounded-lg border border-border/50 p-4 shadow-xl">
      <h3 className="mb-2 text-sm font-medium text-foreground">{title}</h3>
      <textarea
        className="mb-3 h-20 w-full resize-none rounded border border-border/30 bg-surface-base p-2 text-xs text-foreground placeholder:text-muted"
        placeholder="How do these connect? (2-3 sentences)"
        value={content}
        onChange={(e) => setContent(e.target.value)}
        autoFocus
      />
      <div className="flex justify-end gap-2">
        <button
          type="button"
          className="rounded px-3 py-1 text-xs text-muted hover:text-foreground"
          onClick={onClose}
        >
          Cancel
        </button>
        <button
          type="button"
          className="rounded bg-brand px-3 py-1 text-xs text-white hover:bg-brand/80"
          onClick={() => onCreateNote(title, content)}
        >
          Create note
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add drag-to-merge visual feedback**

In `GraphView.tsx`, add drag detection between community clusters:
1. Track which community a dragged node started in
2. If dropped in a different community area, show temporary merged cluster preview (semi-transparent overlay) + "Undo" button with 5s timer
3. If not undone → call `performAction("suggest_merge", { noteId, targetCommunityId })`

- [ ] **Step 3: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/QuickBridgePopover.tsx desktop-ui/src/features/notes/components/GraphView.tsx
git commit -m "feat(fabric-ui): add Quick Bridge popover and drag-to-merge with undo"
```

---

### Task 15: Fabric Pulse Badge + Multi-Select

**Files:**
- Create: `desktop-ui/src/features/notes/components/FabricPulseBadge.tsx`
- Modify: `desktop-ui/src/features/notes/components/GraphView.tsx`

- [ ] **Step 1: Create FabricPulseBadge**

Create `desktop-ui/src/features/notes/components/FabricPulseBadge.tsx`:

```tsx
import { useState } from "react";

interface Props {
  lastActivityTimestamp: string;
  livePulseActive: boolean;
  onViewUpdates: () => void;
  onSwitchToSemantic: () => void;
}

export function FabricPulseBadge({
  lastActivityTimestamp,
  livePulseActive,
  onViewUpdates,
  onSwitchToSemantic,
}: Props) {
  const [popoverOpen, setPopoverOpen] = useState(false);

  const ago = formatTimeAgo(lastActivityTimestamp);

  return (
    <div className="relative">
      <button
        type="button"
        className="flex items-center gap-1.5 rounded-full border border-border/30 bg-surface-base/80 px-2.5 py-1 text-[10px] text-muted backdrop-blur-sm"
        onClick={() => setPopoverOpen(!popoverOpen)}
      >
        <span
          className={`inline-block h-1.5 w-1.5 rounded-full ${
            livePulseActive ? "animate-pulse bg-success" : "bg-muted/50"
          }`}
        />
        Fabric updated {ago}
      </button>

      {popoverOpen && (
        <div className="glass-panel absolute bottom-full left-0 z-50 mb-1 min-w-[200px] rounded-lg border border-border/50 p-1 text-xs shadow-lg">
          <button
            type="button"
            className="flex w-full items-center gap-2 rounded px-2.5 py-1.5 text-left hover:bg-surface-raised/50"
            onClick={() => { onViewUpdates(); setPopoverOpen(false); }}
          >
            View latest updates
          </button>
          <button
            type="button"
            className="flex w-full items-center gap-2 rounded px-2.5 py-1.5 text-left hover:bg-surface-raised/50"
            onClick={() => { onSwitchToSemantic(); setPopoverOpen(false); }}
          >
            Switch to Semantic
          </button>
        </div>
      )}
    </div>
  );
}

function formatTimeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const secs = Math.floor(diff / 1000);
  if (secs < 60) return `${secs}s ago`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  return `${hours}h ago`;
}
```

- [ ] **Step 2: Add multi-select support**

In `GraphView.tsx`:
1. Track `selectedNodes: Set<string>` state
2. Cmd/Ctrl + Click adds/removes from selection
3. When 2+ nodes selected, show a floating batch action toolbar:
   - "Merge into community"
   - "Expand all trees"
   - "Create bridge note from selection"
   - "Compare in chat"

- [ ] **Step 3: Wire FabricPulseBadge into GraphView**

Render `FabricPulseBadge` in the bottom-left corner of `GraphView`, near the clusters legend. Pass fabric base data for timestamp and pulse state.

- [ ] **Step 4: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/components/FabricPulseBadge.tsx desktop-ui/src/features/notes/components/GraphView.tsx
git commit -m "feat(fabric-ui): add Fabric Pulse badge, multi-select, and batch actions"
```

---

### Task 16: 3D Brain View — Fabric Layer Support

**Files:**
- Modify: `desktop-ui/src/features/notes/components/GraphBrainView.tsx`
- Modify: `desktop-ui/src/features/notes/lib/graphMaterials.ts`

- [ ] **Step 1: Extend 3D node rendering for new node types**

In `GraphBrainView.tsx`, extend the `nodeThreeObject` callback:

- `"entity"` nodes: Use `OctahedronGeometry` (diamond-like) instead of `SphereGeometry`. Smaller size. Accent color material.
- `"tree_section"` nodes: Smaller `SphereGeometry`. Parent note color at 60% opacity.
- `"tree_text"` nodes: Tiny sphere (radius 2). Parent note color at 30% opacity.
- `"community_label"` nodes: Use `PlaneGeometry` with a `CanvasTexture` showing the community name as text sprite.

In `graphMaterials.ts`, add material factory functions for the new node types.

- [ ] **Step 2: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/GraphBrainView.tsx desktop-ui/src/features/notes/lib/graphMaterials.ts
git commit -m "feat(fabric-ui): extend 3D Brain View with entity/tree/community node rendering"
```

---

### Task 17: Integration Tests + Final Verification

**Files:**
- Modify existing test files as needed

- [ ] **Step 1: Write backend handler tests**

Add tests for `fabric_graph_base`, `fabric_graph_expand`, and `fabric_graph_action` in the appropriate test location. Test:
- `fabric_graph_base` returns notes + links + communities with correct types
- `fabric_graph_expand("entities")` returns entities with edges
- `fabric_graph_expand("tree", [noteId])` returns tree nodes for the note
- `fabric_graph_action("create_bridge_note")` creates a note and returns success

- [ ] **Step 2: Run full Rust test suite**

Run: `cargo nextest run --workspace`

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 4: Run frontend build + lint + tests**

Run: `cd desktop-ui && bun run build && bun run lint:fix && bun run test`

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "test: add integration tests for Knowledge Fabric Explorer"
```

---

### Follow-up (not blocking Phase 3 core)

- **Coaching gap detection**: Subscribe to `HighlightGap` hook → auto-open graph with gap edges
- **Focus session debrief**: Subscribe to `PinToFocus` hook → highlight strengthened communities
- **Mirror narrative links**: "View evolution" button → graph with drift animation
- **Autotuner 30D**: Add `visual_engagement_weight`, `layer_switch_rate` params + `suggested_preset` learning
- **Community rename via LLM**: Fire-and-forget naming for communities ≥5 members
- **Stability sparkline**: Recharts mini-chart in community detail panel
- **Cmd+K fabric search**: Global search modal for communities, entities, notes
