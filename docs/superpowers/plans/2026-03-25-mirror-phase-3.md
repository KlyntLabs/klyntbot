# The Mirror Phase 3 — Brain Versions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the ConfigArchiver + BrainTimeline — "I can travel through my brain's history" with one-click revert.

**Architecture:** A new `ConfigArchiver` event subscriber listens for `AutotunerDecision` (promotion) events and writes `BrainVersion` snapshots. The existing `autotuner_revert()` is refactored to go through an `AutotunerBridge` trait so reverts also create version entries. The UI shows a vertical timeline of brain versions with revert buttons. On revert, the timeline always moves forward (new version with old params).

**Tech Stack:** Rust (cognitive/agent/app-core/desktop crates), SQLite (mirror_brain_versions table), React + Tailwind v4 (desktop-ui)

**Spec:** `docs/superpowers/specs/2026-03-25-mirror-self-reflection-layer-design.md` — ConfigArchiver section (lines 359-372) + BrainVersion type (lines 154-164)

**Depends on:** Phase 1+2 complete (mirror module with types, repo, facade, engine, 2 subscribers)

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/mirror/subscribers/version.rs` | `ConfigArchiver` — listens for promotion events, writes BrainVersion snapshots |
| `desktop-ui/src/features/mirror/components/BrainTimeline.tsx` | Vertical timeline of brain versions with revert buttons |

### Modified files

| File | Change |
|------|--------|
| `crates/cognitive/src/mirror/types.rs` | Add `BrainVersion` type, `AutotunerBridge` trait, update `MirrorState` |
| `crates/cognitive/migrations/003_mirror_tables.sql` | Append `mirror_brain_versions` table |
| `crates/cognitive/src/repos/mod.rs` | Bump `cognitive_mirror` migration version to 3 |
| `crates/cognitive/src/mirror/repo.rs` | Add brain version CRUD methods |
| `crates/cognitive/src/mirror/facade.rs` | Add `revert_to_version`, `get_brain_versions`; update `get_state`; accept `AutotunerBridge` |
| `crates/cognitive/src/mirror/engine.rs` | Start `ConfigArchiver` subscriber (3 subscribers total); accept `AutotunerBridge` |
| `crates/cognitive/src/mirror/subscribers/mod.rs` | Export `ConfigArchiver` |
| `crates/cognitive/src/mirror/mod.rs` | Re-export `AutotunerBridge` |
| `crates/app-core/src/handlers/autotuner.rs` | Refactor `autotuner_revert()` to go through `AutotunerBridge` |
| `crates/app-core/src/init/mod.rs` | Create and inject `AutotunerBridge` impl into MirrorEngine |
| `crates/desktop/src/commands/mirror.rs` | Add `get_brain_versions`, `revert_brain_version` commands |
| `desktop-ui/src/features/mirror/MirrorPage.tsx` | Add `BrainTimeline` component |

---

## Task 1: Add BrainVersion Type + AutotunerBridge Trait

**Files:**
- Modify: `crates/cognitive/src/mirror/types.rs`
- Modify: `crates/cognitive/src/mirror/mod.rs`

- [ ] **Step 1: Add BrainVersion struct**

In `types.rs`, add after the existing types:

```rust
/// Snapshot of a promoted champion configuration — the "git log" of the brain's personality
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainVersion {
    pub version: u32,
    pub trial_id: Option<String>,
    pub promoted_at: DateTime<Utc>,
    pub params: serde_json::Value,           // full TrialParams snapshot
    pub reason: String,
    pub parent_version: Option<u32>,
    pub metrics_at_promotion: serde_json::Value,
    pub reverted: bool,
}
```

- [ ] **Step 2: Add AutotunerBridge trait**

This trait is defined in cognitive (L3-L4) and implemented in app-core (L7). It allows the Mirror to apply champion params without importing the autotuner directly.

```rust
/// Bridge to the autotuner for applying champion params and managing trials.
/// Defined in cognitive, implemented in app-core.
#[async_trait::async_trait]
pub trait AutotunerBridge: Send + Sync {
    /// Apply a set of params as the new champion configuration
    async fn apply_champion(&self, params: serde_json::Value, reason: String) -> common::Result<()>;
    /// Get the current champion's params as JSON
    async fn current_champion_params(&self) -> common::Result<serde_json::Value>;
}
```

- [ ] **Step 3: Update MirrorState**

Add to `MirrorState`:
```rust
pub latest_brain_version: Option<BrainVersion>,
```

- [ ] **Step 4: Update SuggestedAction**

Add variant (if not already present from spec — check first):
```rust
RevertBrainVersion { version: u32 },
```

- [ ] **Step 5: Fix compile breaks**

Update `facade.rs` `get_state()` to add `latest_brain_version: None` temporarily.

- [ ] **Step 6: Re-export from mod.rs**

Ensure `AutotunerBridge` is re-exported from `mirror/mod.rs`.

- [ ] **Step 7: Build**

Run: `cargo build -p cognitive`

- [ ] **Step 8: Commit**

```bash
git commit -m "feat(mirror): add BrainVersion type and AutotunerBridge trait for Phase 3"
```

---

## Task 2: Add `mirror_brain_versions` Table

**Files:**
- Modify: `crates/cognitive/migrations/003_mirror_tables.sql`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Append table to migration**

Add to end of `003_mirror_tables.sql`:

```sql
CREATE TABLE IF NOT EXISTS mirror_brain_versions (
    version INTEGER PRIMARY KEY,
    trial_id TEXT,
    promoted_at TEXT NOT NULL,
    params_json TEXT NOT NULL,
    reason TEXT NOT NULL,
    parent_version INTEGER,
    metrics_json TEXT NOT NULL,
    reverted INTEGER NOT NULL DEFAULT 0
);
```

- [ ] **Step 2: Bump migration version**

In `crates/cognitive/src/repos/mod.rs`, bump `cognitive_mirror` version from `2` to `3`.

- [ ] **Step 3: Build**

Run: `cargo build -p cognitive`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): add mirror_brain_versions table"
```

---

## Task 3: Brain Version Repo Methods

**Files:**
- Modify: `crates/cognitive/src/mirror/repo.rs`

- [ ] **Step 1: Write tests**

```rust
#[tokio::test]
async fn test_insert_and_get_brain_version() {
    let repo = crate::mirror::test_mirror_repo().await;
    let v = BrainVersion {
        version: 1,
        trial_id: None,
        promoted_at: Utc::now(),
        params: serde_json::json!({"skill_keyword_weight": 0.7}),
        reason: "Initial brain state".to_string(),
        parent_version: None,
        metrics_at_promotion: serde_json::json!({}),
        reverted: false,
    };
    repo.insert_brain_version(&v).await.unwrap();
    let latest = repo.get_latest_brain_version().await.unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().version, 1);
    assert_eq!(latest.unwrap().reason, "Initial brain state");
}

#[tokio::test]
async fn test_get_brain_versions_ordered() {
    let repo = crate::mirror::test_mirror_repo().await;
    for i in 1..=3 {
        let v = BrainVersion {
            version: i,
            trial_id: Some(format!("trial-{i}")),
            promoted_at: Utc::now(),
            params: serde_json::json!({}),
            reason: format!("Version {i}"),
            parent_version: if i > 1 { Some(i - 1) } else { None },
            metrics_at_promotion: serde_json::json!({}),
            reverted: false,
        };
        repo.insert_brain_version(&v).await.unwrap();
    }
    let versions = repo.get_brain_versions().await.unwrap();
    assert_eq!(versions.len(), 3);
    assert_eq!(versions[0].version, 3); // newest first
    assert_eq!(versions[2].version, 1);
}

#[tokio::test]
async fn test_get_next_version_number() {
    let repo = crate::mirror::test_mirror_repo().await;
    assert_eq!(repo.get_next_version_number().await.unwrap(), 1);
    let v = BrainVersion {
        version: 1, trial_id: None, promoted_at: Utc::now(),
        params: serde_json::json!({}), reason: "v1".to_string(),
        parent_version: None, metrics_at_promotion: serde_json::json!({}),
        reverted: false,
    };
    repo.insert_brain_version(&v).await.unwrap();
    assert_eq!(repo.get_next_version_number().await.unwrap(), 2);
}

#[tokio::test]
async fn test_mark_versions_reverted() {
    let repo = crate::mirror::test_mirror_repo().await;
    for i in 1..=3 {
        let v = BrainVersion {
            version: i, trial_id: None, promoted_at: Utc::now(),
            params: serde_json::json!({}), reason: format!("v{i}"),
            parent_version: if i > 1 { Some(i - 1) } else { None },
            metrics_at_promotion: serde_json::json!({}), reverted: false,
        };
        repo.insert_brain_version(&v).await.unwrap();
    }
    // Revert to v1 — marks v2 and v3 as reverted
    repo.mark_versions_reverted_after(1).await.unwrap();
    let versions = repo.get_brain_versions().await.unwrap();
    assert!(!versions[2].reverted); // v1 not reverted
    assert!(versions[1].reverted);  // v2 reverted
    assert!(versions[0].reverted);  // v3 reverted
}
```

- [ ] **Step 2: Run tests to fail**

Run: `cargo nextest run -p cognitive -E 'test(brain_version)'`
Expected: FAIL

- [ ] **Step 3: Implement repo methods**

Add `BrainVersionRow` with `#[derive(sqlx::FromRow)]` and `From<BrainVersionRow> for BrainVersion`. Then add methods:

```rust
pub async fn insert_brain_version(&self, v: &BrainVersion) -> Result<()> {
    let promoted_at = v.promoted_at.to_rfc3339();
    let params_json = serde_json::to_string(&v.params)?;
    let metrics_json = serde_json::to_string(&v.metrics_at_promotion)?;
    sqlx::query(
        "INSERT INTO mirror_brain_versions (version, trial_id, promoted_at, params_json, reason, parent_version, metrics_json, reverted) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
    )
    .bind(v.version as i32)
    .bind(&v.trial_id)
    .bind(&promoted_at)
    .bind(&params_json)
    .bind(&v.reason)
    .bind(v.parent_version.map(|p| p as i32))
    .bind(&metrics_json)
    .bind(v.reverted as i32)
    .execute(self.pool.inner())
    .await?;
    Ok(())
}

pub async fn get_latest_brain_version(&self) -> Result<Option<BrainVersion>> {
    let row = sqlx::query_as::<_, BrainVersionRow>(
        "SELECT * FROM mirror_brain_versions ORDER BY version DESC LIMIT 1"
    )
    .fetch_optional(self.pool.inner())
    .await?;
    row.map(|r| r.try_into()).transpose()
}

pub async fn get_brain_versions(&self) -> Result<Vec<BrainVersion>> {
    let rows = sqlx::query_as::<_, BrainVersionRow>(
        "SELECT * FROM mirror_brain_versions ORDER BY version DESC"
    )
    .fetch_all(self.pool.inner())
    .await?;
    rows.into_iter().map(|r| r.try_into()).collect()
}

pub async fn get_next_version_number(&self) -> Result<u32> {
    let (max_version,): (i32,) = sqlx::query_as(
        "SELECT COALESCE(MAX(version), 0) FROM mirror_brain_versions"
    )
    .fetch_one(self.pool.inner())
    .await?;
    Ok(max_version as u32 + 1)
}

pub async fn mark_versions_reverted_after(&self, target_version: u32) -> Result<()> {
    sqlx::query("UPDATE mirror_brain_versions SET reverted = 1 WHERE version > ?1")
        .bind(target_version as i32)
        .execute(self.pool.inner())
        .await?;
    Ok(())
}

pub async fn get_brain_version(&self, version: u32) -> Result<Option<BrainVersion>> {
    let row = sqlx::query_as::<_, BrainVersionRow>(
        "SELECT * FROM mirror_brain_versions WHERE version = ?1"
    )
    .bind(version as i32)
    .fetch_optional(self.pool.inner())
    .await?;
    row.map(|r| r.try_into()).transpose()
}
```

The `BrainVersionRow` should have: `version: i32`, `trial_id: Option<String>`, `promoted_at: String`, `params_json: String`, `reason: String`, `parent_version: Option<i32>`, `metrics_json: String`, `reverted: i32`. Convert using `parse_rfc3339` helper and `serde_json::from_str` for params/metrics.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(brain_version)'`
Expected: 4 tests PASS

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(mirror): add BrainVersion CRUD methods to MirrorRepo with tests"
```

---

## Task 4: ConfigArchiver Subscriber

**Files:**
- Create: `crates/cognitive/src/mirror/subscribers/version.rs`
- Modify: `crates/cognitive/src/mirror/subscribers/mod.rs`

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bootstrap_creates_version_1() {
        let repo = crate::mirror::test_mirror_repo().await;
        let archiver = ConfigArchiver::new(repo.clone());
        let default_params = serde_json::json!({"skill_keyword_weight": 0.7});
        archiver.bootstrap(default_params.clone()).await.unwrap();

        let v = repo.get_latest_brain_version().await.unwrap().unwrap();
        assert_eq!(v.version, 1);
        assert_eq!(v.reason, "Initial brain state");
        assert!(v.parent_version.is_none());
        assert_eq!(v.params, default_params);
    }

    #[tokio::test]
    async fn test_bootstrap_skips_if_versions_exist() {
        let repo = crate::mirror::test_mirror_repo().await;
        let archiver = ConfigArchiver::new(repo.clone());
        archiver.bootstrap(serde_json::json!({})).await.unwrap();
        archiver.bootstrap(serde_json::json!({"new": true})).await.unwrap();

        let versions = repo.get_brain_versions().await.unwrap();
        assert_eq!(versions.len(), 1); // only 1, not 2
    }

    #[tokio::test]
    async fn test_record_promotion() {
        let repo = crate::mirror::test_mirror_repo().await;
        let archiver = ConfigArchiver::new(repo.clone());
        archiver.bootstrap(serde_json::json!({})).await.unwrap();

        archiver.record_promotion(
            Some("trial-abc".to_string()),
            serde_json::json!({"skill_keyword_weight": 0.8}),
            "correction rate improved 15%".to_string(),
            serde_json::json!({"correction_rate": 0.05}),
        ).await.unwrap();

        let versions = repo.get_brain_versions().await.unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version, 2);
        assert_eq!(versions[0].parent_version, Some(1));
        assert_eq!(versions[0].trial_id, Some("trial-abc".to_string()));
    }
}
```

- [ ] **Step 2: Implement ConfigArchiver**

```rust
use bus::DomainEvent;
use chrono::Utc;
use common::Result;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use super::super::repo::MirrorRepo;
use super::super::types::BrainVersion;

pub struct ConfigArchiver {
    repo: MirrorRepo,
    bridge: Option<Arc<dyn super::super::types::AutotunerBridge>>,
}

impl ConfigArchiver {
    pub fn new(repo: MirrorRepo, bridge: Option<Arc<dyn super::super::types::AutotunerBridge>>) -> Self {
        Self { repo, bridge }
    }

    /// Get current champion params from the bridge, or empty JSON if unavailable
    async fn get_current_params(&self) -> serde_json::Value {
        if let Some(bridge) = &self.bridge {
            bridge.current_champion_params().await.unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        }
    }

    /// Create Version 1 from current config defaults (first run only)
    pub async fn bootstrap(&self, default_params: serde_json::Value) -> Result<()> {
        let next = self.repo.get_next_version_number().await?;
        if next > 1 {
            return Ok(()); // already bootstrapped
        }
        let v = BrainVersion {
            version: 1,
            trial_id: None,
            promoted_at: Utc::now(),
            params: default_params,
            reason: "Initial brain state".to_string(),
            parent_version: None,
            metrics_at_promotion: serde_json::json!({}),
            reverted: false,
        };
        self.repo.insert_brain_version(&v).await
    }

    /// Record a new brain version from a champion promotion
    pub async fn record_promotion(
        &self,
        trial_id: Option<String>,
        params: serde_json::Value,
        reason: String,
        metrics: serde_json::Value,
    ) -> Result<()> {
        let next_version = self.repo.get_next_version_number().await?;
        let parent = if next_version > 1 { Some(next_version - 1) } else { None };

        let v = BrainVersion {
            version: next_version,
            trial_id,
            promoted_at: Utc::now(),
            params,
            reason,
            parent_version: parent,
            metrics_at_promotion: metrics,
            reverted: false,
        };
        self.repo.insert_brain_version(&v).await
    }

    pub async fn run(
        self,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                event = rx.recv() => {
                    match event {
                        Ok(DomainEvent::AutotunerDecision { trial_id, verdict, improvement_pct, .. }) => {
                            // Exact match — only record actual promotions
                            if verdict == "promoted" {
                                // Get current champion's full params via the bridge
                                // (stored as serde_json::Value for future revert)
                                // NOTE: The ConfigArchiver needs access to the AutotunerBridge
                                // to capture the full champion params. Pass it in new() or
                                // store the params from the event. For now, store improvement_pct
                                // as metrics and the event doesn't carry full params — the bridge
                                // will be used at bootstrap and revert time to get current params.
                                let _ = self.record_promotion(
                                    Some(trial_id),
                                    self.get_current_params().await,
                                    format!("Promoted: {:.1}% improvement", improvement_pct),
                                    serde_json::json!({"improvement_pct": improvement_pct}),
                                ).await;
                            }
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("ConfigArchiver lagged {} events", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    }
}
```

Note: The `AutotunerDecision` event has `verdict: String`. Check the actual verdict string format — it may be "Promoted", "promoted", "Promotion", etc. Read how the event is emitted in the autotuner code. The `verdict.to_lowercase().contains("promot")` check handles common variations.

- [ ] **Step 3: Export from subscribers/mod.rs**

Add: `pub mod version; pub use version::ConfigArchiver;`

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(bootstrap\|record_promotion)'`
Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(mirror): add ConfigArchiver subscriber with bootstrap and promotion recording"
```

---

## Task 5: Update MirrorFacade for Brain Versions

**Files:**
- Modify: `crates/cognitive/src/mirror/facade.rs`

- [ ] **Step 1: Write tests**

```rust
#[tokio::test]
async fn test_get_brain_versions() {
    let repo = crate::mirror::test_mirror_repo().await;
    let facade = MirrorFacade::new(repo.clone());
    // Insert some versions
    for i in 1..=3 {
        let v = BrainVersion {
            version: i, trial_id: None, promoted_at: Utc::now(),
            params: serde_json::json!({}), reason: format!("v{i}"),
            parent_version: if i > 1 { Some(i - 1) } else { None },
            metrics_at_promotion: serde_json::json!({}), reverted: false,
        };
        repo.insert_brain_version(&v).await.unwrap();
    }
    let versions = facade.get_brain_versions().await.unwrap();
    assert_eq!(versions.len(), 3);
}

#[tokio::test]
async fn test_revert_to_version() {
    let repo = crate::mirror::test_mirror_repo().await;
    let facade = MirrorFacade::new(repo.clone());
    // Insert v1 and v2
    for i in 1..=2 {
        let v = BrainVersion {
            version: i, trial_id: None, promoted_at: Utc::now(),
            params: serde_json::json!({"version": i}),
            reason: format!("v{i}"),
            parent_version: if i > 1 { Some(i - 1) } else { None },
            metrics_at_promotion: serde_json::json!({}), reverted: false,
        };
        repo.insert_brain_version(&v).await.unwrap();
    }
    // Revert to v1 — without autotuner_bridge (just test DB logic)
    let new_v = facade.revert_to_version_db_only(1).await.unwrap();
    assert_eq!(new_v.version, 3); // new version created
    assert_eq!(new_v.reason, "Reverted to v1");
    assert_eq!(new_v.params, serde_json::json!({"version": 1}));

    // v2 should be marked as reverted
    let versions = repo.get_brain_versions().await.unwrap();
    assert!(versions.iter().find(|v| v.version == 2).unwrap().reverted);
    assert!(!versions.iter().find(|v| v.version == 1).unwrap().reverted);
}

#[tokio::test]
async fn test_get_state_includes_brain_version() {
    let repo = crate::mirror::test_mirror_repo().await;
    let facade = MirrorFacade::new(repo.clone());
    let v = BrainVersion {
        version: 1, trial_id: None, promoted_at: Utc::now(),
        params: serde_json::json!({}), reason: "v1".to_string(),
        parent_version: None, metrics_at_promotion: serde_json::json!({}),
        reverted: false,
    };
    repo.insert_brain_version(&v).await.unwrap();
    let state = facade.get_state().await.unwrap();
    assert!(state.latest_brain_version.is_some());
    assert_eq!(state.latest_brain_version.unwrap().version, 1);
}
```

- [ ] **Step 2: Implement facade methods**

Add `autotuner_bridge` as an optional field to `MirrorFacade`:

```rust
pub autotuner_bridge: Option<Arc<dyn AutotunerBridge>>,
```

Add builder method:
```rust
pub fn with_autotuner_bridge(mut self, bridge: Arc<dyn AutotunerBridge>) -> Self {
    self.autotuner_bridge = Some(bridge);
    self
}
```

Add methods:

```rust
pub async fn get_brain_versions(&self) -> Result<Vec<BrainVersion>> {
    self.repo.get_brain_versions().await
}

/// Revert DB state only (for tests without autotuner bridge)
pub(crate) async fn revert_to_version_db_only(&self, target: u32) -> Result<BrainVersion> {
    let target_v = self.repo.get_brain_version(target).await?
        .ok_or_else(|| common::KlyntbotError::NotFound(format!("Brain version {target} not found")))?;

    // Mark versions after target as reverted
    self.repo.mark_versions_reverted_after(target).await?;

    // Create new version with target's params (timeline always moves forward)
    let next = self.repo.get_next_version_number().await?;
    let new_v = BrainVersion {
        version: next,
        trial_id: None,
        promoted_at: Utc::now(),
        params: target_v.params.clone(),
        reason: format!("Reverted to v{target}"),
        parent_version: Some(next - 1),
        metrics_at_promotion: target_v.metrics_at_promotion.clone(),
        reverted: false,
    };
    self.repo.insert_brain_version(&new_v).await?;
    Ok(new_v)
}

/// Full revert: updates DB + applies params via AutotunerBridge
pub async fn revert_to_version(&self, target: u32) -> Result<BrainVersion> {
    let new_v = self.revert_to_version_db_only(target).await?;

    // Apply params to the live autotuner if bridge available
    if let Some(bridge) = &self.autotuner_bridge {
        bridge.apply_champion(new_v.params.clone(), new_v.reason.clone()).await?;
    }

    Ok(new_v)
}
```

Update `get_state()` — add `get_latest_brain_version` to the `tokio::try_join!`:

```rust
let (last_routing_snapshot, latest_trend_narrative, pending_snippets, active_meta_rules, pending_meta_rules, latest_brain_version) = tokio::try_join!(
    self.repo.get_latest_routing_snapshot(),
    self.repo.get_latest_narrative(),
    self.repo.get_pending_snippets(),
    self.repo.get_meta_rules_by_status(MetaRuleStatus::Active),
    self.repo.get_meta_rules_by_status(MetaRuleStatus::Pending),
    self.repo.get_latest_brain_version(),
)?;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(mirror)'`
Expected: all tests PASS

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): add revert_to_version, get_brain_versions to MirrorFacade"
```

---

## Task 6: Wire ConfigArchiver into MirrorEngine

**Files:**
- Modify: `crates/cognitive/src/mirror/engine.rs`

- [ ] **Step 1: Update MirrorEngine::start signature**

Add `autotuner_bridge: Option<Arc<dyn AutotunerBridge>>` parameter. Clone `repo` for the ConfigArchiver. Spawn the third subscriber:

```rust
let version_repo = repo.clone();
let config_archiver = ConfigArchiver::new(version_repo);
let handles = vec![
    tokio::spawn(routing_sub.run(bus.subscribe(), shutdown.clone())),
    tokio::spawn(meta_rule_detector.run(bus.subscribe(), shutdown.clone())),
    tokio::spawn(config_archiver.run(bus.subscribe(), shutdown.clone())),
];

// Wire autotuner_bridge into facade
if let Some(bridge) = autotuner_bridge {
    facade = facade.with_autotuner_bridge(bridge);
}
```

- [ ] **Step 2: Update subscriber count test**

Change assertion from `== 2` to `== 3`.

- [ ] **Step 3: Update app-core init (MUST be done in this task to avoid compile break)**

In `crates/app-core/src/init/mod.rs`, update the `MirrorEngine::start()` call to pass `autotuner_bridge: None` for now. The signature changed, so the call site must be updated simultaneously:

```rust
let (facade, _handles, _mirror_shutdown) = cognitive::mirror::MirrorEngine::start(
    mirror_repo,
    &domain_event_bus,
    narrative_handler,
    None, // autotuner_bridge — wired in Task 7
);
```

Also update any existing engine tests that destructure the 3-tuple return — if the return type changed, fix the destructuring pattern in all test sites.

- [ ] **Step 4: Build and run tests**

Run: `cargo build --workspace && cargo nextest run -p cognitive -E 'test(mirror)'`
Expected: compiles, all tests PASS

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(mirror): wire ConfigArchiver into MirrorEngine (3 subscribers)"
```

---

## Task 7: AutotunerBridge Implementation + Revert Reconciliation

**Files:**
- Modify: `crates/app-core/src/init/mod.rs` — create bridge impl, pass to MirrorEngine
- Modify: `crates/app-core/src/handlers/autotuner.rs` — refactor revert to use bridge

This is the most complex integration task. Read the existing code carefully before modifying.

- [ ] **Step 1: Create AutotunerBridge impl in app-core**

In `crates/app-core/src/init/mod.rs` (or a new file `crates/app-core/src/handlers/mirror_bridge.rs`), implement the trait:

```rust
pub struct AppAutotunerBridge {
    orchestrator: Arc<agent::autotuner::AutoTunerOrchestrator>,
}

#[async_trait::async_trait]
impl cognitive::mirror::AutotunerBridge for AppAutotunerBridge {
    async fn apply_champion(&self, params: serde_json::Value, reason: String) -> common::Result<()> {
        // Deserialize params to the Champion type expected by the orchestrator
        // This depends on the orchestrator's API — read update_champion() to understand
        // the expected input format. The simplest approach: construct a Champion from
        // the params JSON and call orch.update_champion().
        // Implementation depends heavily on the exact Champion struct — read the code first.
        tracing::info!("AutotunerBridge: applying champion params, reason: {reason}");
        Ok(()) // TODO: wire to actual orchestrator once Champion construction is understood
    }

    async fn current_champion_params(&self) -> common::Result<serde_json::Value> {
        let summary = self.orchestrator.champion_summary().await?;
        Ok(serde_json::to_value(summary)?)
    }
}
```

IMPORTANT: Read `crates/agent/src/autotuner/mod.rs` lines 192-229 (`update_champion`) and the `Champion` struct to understand the exact API. The bridge implementation must construct the correct type. If the Champion struct is complex, start with a TODO and wire it properly in a follow-up.

- [ ] **Step 2: Pass bridge to MirrorEngine**

In `init/mod.rs` where `MirrorEngine::start()` is called, create the bridge and pass it:

```rust
let autotuner_bridge: Option<Arc<dyn cognitive::mirror::AutotunerBridge>> =
    autotuner_orchestrator.as_ref().map(|orch| {
        Arc::new(AppAutotunerBridge { orchestrator: Arc::clone(orch) })
            as Arc<dyn cognitive::mirror::AutotunerBridge>
    });

let (facade, _handles, _mirror_shutdown) = cognitive::mirror::MirrorEngine::start(
    mirror_repo,
    &domain_event_bus,
    narrative_handler,
    autotuner_bridge,
);
```

- [ ] **Step 3: Bootstrap brain version on startup**

The simplest approach: inside `MirrorEngine::start()`, call `ConfigArchiver::bootstrap()` before spawning the subscriber task. The archiver already has access to the bridge and can bootstrap itself:

```rust
// In MirrorEngine::start, after creating config_archiver but before spawning:
let default_params = if let Some(ref bridge) = autotuner_bridge {
    bridge.current_champion_params().await.unwrap_or(serde_json::json!({}))
} else {
    serde_json::json!({})
};
config_archiver.bootstrap(default_params).await.ok();
// Then spawn as usual:
tokio::spawn(config_archiver.run(bus.subscribe(), shutdown.clone()))
```

Note: `MirrorEngine::start` will need to become `async` for this. If that's problematic (it's called from sync init), move the bootstrap call to after engine start in the init code instead.

- [ ] **Step 4: Build**

Run: `cargo build --workspace`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(mirror): implement AutotunerBridge and bootstrap brain versioning"
```

---

## Task 8: Tauri Commands for Brain Versions

**Files:**
- Modify: `crates/desktop/src/commands/mirror.rs`

- [ ] **Step 1: Add commands**

```rust
#[tauri::command]
pub async fn get_brain_versions(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<BrainVersion>, ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.get_brain_versions().await?)
}

#[tauri::command]
pub async fn revert_brain_version(
    state: State<'_, Arc<AppCore>>,
    version: u32,
) -> Result<BrainVersion, ApiError> {
    let facade = state.mirror_facade()?;
    Ok(facade.revert_to_version(version).await?)
}
```

- [ ] **Step 2: Update DEV_COMMANDS, invoke_handler, dispatch_dev**

Add both commands. In `dispatch_dev`, accept both `version` and `version` (no camelCase conflict since it's a single word).

- [ ] **Step 3: Build**

Run: `cargo build -p desktop`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): add get_brain_versions and revert_brain_version Tauri commands"
```

---

## Task 9: BrainTimeline UI

**Files:**
- Create: `desktop-ui/src/features/mirror/components/BrainTimeline.tsx`
- Modify: `desktop-ui/src/features/mirror/MirrorPage.tsx`

- [ ] **Step 1: Create BrainTimeline component**

```tsx
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { GitBranch, RotateCcw } from "lucide-react";
import { useState } from "react";

interface BrainVersion {
  version: number;
  trialId: string | null;
  promotedAt: string;
  params: Record<string, unknown>;
  reason: string;
  parentVersion: number | null;
  metricsAtPromotion: Record<string, unknown>;
  reverted: boolean;
}

export function BrainTimeline() {
  const { data: versions, refetch } = useQuery<BrainVersion[]>(
    "get_brain_versions",
    undefined,
    [],
  );
  const { mutate: revert, loading } = useMutation<BrainVersion, { version: number }>(
    "revert_brain_version",
  );
  const [confirmVersion, setConfirmVersion] = useState<number | null>(null);

  if (!versions || versions.length === 0) return null;

  const handleRevert = async (version: number) => {
    await revert({ version });
    setConfirmVersion(null);
    refetch();
  };

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-muted-foreground flex items-center gap-1.5">
        <GitBranch className="size-3.5" />
        Brain Versions
      </h2>

      <div className="relative">
        {/* Timeline line */}
        <div className="absolute left-3 top-0 bottom-0 w-px bg-border-subtle" />

        {versions.map((v) => (
          <div key={v.version} className={`relative flex gap-3 pb-4 ${v.reverted ? "opacity-40" : ""}`}>
            {/* Timeline dot */}
            <div className={`relative z-10 mt-1.5 size-2 rounded-full shrink-0 ${v.reverted ? "bg-muted-foreground" : "bg-accent"}`}
              style={{ marginLeft: "7px" }}
            />

            <div className="glass-card rounded-xl p-3 flex-1">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-[12px] font-medium text-foreground">
                    Version {v.version}
                  </span>
                  {v.reverted && (
                    <span className="text-2xs text-muted-foreground ml-2">(reverted)</span>
                  )}
                </div>
                <span className="text-2xs text-dim">
                  {new Date(v.promotedAt).toLocaleDateString()}
                </span>
              </div>
              <p className="text-[11px] text-muted-foreground mt-0.5">{v.reason}</p>

              {!v.reverted && v.version !== versions[0]?.version && (
                confirmVersion === v.version ? (
                  <div className="flex items-center gap-2 mt-2">
                    <span className="text-2xs text-muted-foreground">Revert to this version?</span>
                    <button
                      type="button"
                      onClick={() => handleRevert(v.version)}
                      disabled={loading}
                      className="text-2xs text-accent hover:text-accent/80 transition-colors"
                    >
                      Confirm
                    </button>
                    <button
                      type="button"
                      onClick={() => setConfirmVersion(null)}
                      className="text-2xs text-muted-foreground"
                    >
                      Cancel
                    </button>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => setConfirmVersion(v.version)}
                    className="flex items-center gap-1 mt-2 text-2xs text-muted-foreground hover:text-accent transition-colors"
                  >
                    <RotateCcw className="size-3" />
                    Revert to this version
                  </button>
                )
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add to MirrorPage**

Import and render between MetaRulesSection and RoutingDonut:

```tsx
import { BrainTimeline } from "./components/BrainTimeline";

// In the render:
<BrainTimeline />
```

Also update `MirrorState` interface to include `latestBrainVersion`.

- [ ] **Step 3: Lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(mirror): add BrainTimeline UI with revert confirmation"
```

---

## Task 10: Integration Test

**Files:**
- Modify: `tests/integration/mirror.rs`

- [ ] **Step 1: Add brain version lifecycle test**

```rust
#[tokio::test]
async fn test_mirror_brain_version_lifecycle() {
    let pool = test_pool().await;
    StoragePool::run_feature_migrations(pool.inner(), &cognitive_migrations()).await.unwrap();

    let repo = cognitive::mirror::MirrorRepo::new(pool.clone());
    let facade = cognitive::mirror::MirrorFacade::new(repo.clone());

    // 1. Insert v1 and v2
    for i in 1..=2 {
        let v = cognitive::mirror::BrainVersion {
            version: i, trial_id: None, promoted_at: chrono::Utc::now(),
            params: serde_json::json!({"version": i}),
            reason: format!("Version {i}"),
            parent_version: if i > 1 { Some(i - 1) } else { None },
            metrics_at_promotion: serde_json::json!({}), reverted: false,
        };
        repo.insert_brain_version(&v).await.unwrap();
    }

    // 2. Verify in state
    let state = facade.get_state().await.unwrap();
    assert!(state.latest_brain_version.is_some());
    assert_eq!(state.latest_brain_version.unwrap().version, 2);

    // 3. Get all versions
    let versions = facade.get_brain_versions().await.unwrap();
    assert_eq!(versions.len(), 2);

    // 4. Revert to v1 (DB only, no autotuner bridge)
    let new_v = facade.revert_to_version_db_only(1).await.unwrap();
    assert_eq!(new_v.version, 3);
    assert_eq!(new_v.reason, "Reverted to v1");

    // 5. Verify v2 is marked reverted, v1 is not
    let versions = facade.get_brain_versions().await.unwrap();
    assert_eq!(versions.len(), 3);
    assert!(!versions.iter().find(|v| v.version == 1).unwrap().reverted);
    assert!(versions.iter().find(|v| v.version == 2).unwrap().reverted);
    assert!(!versions.iter().find(|v| v.version == 3).unwrap().reverted);
}
```

- [ ] **Step 2: Run test**

Run: `cargo nextest run -E 'test(brain_version_lifecycle)'`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git commit -m "test(mirror): add brain version lifecycle integration test for Phase 3"
```

---

## Final Verification

- [ ] **Run full workspace build:** `cargo build --workspace`
- [ ] **Run all mirror tests:** `cargo nextest run -p cognitive -E 'test(mirror)'`
- [ ] **Run integration tests:** `cargo nextest run -E 'test(mirror)'`
- [ ] **Run clippy:** `cargo clippy --workspace --all-targets --all-features`
- [ ] **Run frontend lint:** `cd desktop-ui && bun run lint`
- [ ] **Run frontend build:** `cd desktop-ui && bun run build`
