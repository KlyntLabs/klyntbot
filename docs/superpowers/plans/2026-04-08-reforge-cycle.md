# Reforge Cycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace 5 separate cron jobs with a single nightly Reforge Cycle that improves Memory (facts/rules), Skills (file edits), and Parameters (autotuner) in one coordinated pass.

**Architecture:** A `ReforgeService` orchestrates 7 phases (Collect → Synthesize → Review → Narrate → Apply → Optimize → Compact). Data collection is incremental (since last run). Three focused LLM calls handle knowledge synthesis, skill review, and narrative generation. Skill edits write to `~/.klyntbot/skills/` with version history in SQLite.

**Tech Stack:** Rust, SQLite (sqlx), Tauri IPC, async_trait, serde_json, similar (diff library), chrono, uuid

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `crates/storage/src/repos/reforge_state.rs` | Singleton state: last_run_at, stats, run_count |
| `crates/storage/src/repos/skill_version.rs` | Skill version history: content snapshots, diffs, source labels |
| `crates/storage/src/rows/reforge_state.rs` | Row type for reforge_state table |
| `crates/storage/src/rows/skill_version.rs` | Row type for skill_versions table |
| `crates/cognitive/src/services/reforge/mod.rs` | ReforgeService orchestrator + ReforgeHandler trait |
| `crates/cognitive/src/services/reforge/collector.rs` | Phase 1: data gathering |
| `crates/cognitive/src/services/reforge/skill_files.rs` | Read/write/diff/hash skills on disk |
| `crates/cognitive/src/services/reforge/types.rs` | Input/output types for all phases |
| `crates/agent/src/adapters/reforge_handlers.rs` | LLM implementations of ReforgeHandler trait |
| `crates/desktop-shared/src/commands/reforge.rs` | IPC response types for Brain page |
| `crates/app-core/src/handlers/reforge.rs` | AppCore methods for Reforge data |

### Modified Files
| File | Change |
|------|--------|
| `crates/storage/migrations/001_initial.sql` | Add `reforge_state` + `skill_versions` tables |
| `crates/storage/src/repos/mod.rs` | Export new repos, add to `Repos` struct |
| `crates/storage/src/rows/mod.rs` | Export new row types |
| `crates/cognitive/src/services/mod.rs` | Add `pub mod reforge;` |
| `crates/cognitive/src/services/compaction.rs` | Add embedding reindex step |
| `crates/app-core/src/init/cron.rs` | Register Reforge job, remove 5 old jobs |
| `crates/app-core/src/handlers/cognitive/mod.rs` | Add reforge handler builder |
| `crates/desktop/src/commands/cognitive.rs` | Add Reforge IPC commands |

### Deleted Files/Code
| Target | What |
|--------|------|
| `crates/cognitive/src/services/reflection.rs` | Entire file — replaced by Reforge Phase 2+5 |
| `crates/app-core/src/init/cron.rs` | 5 cron job registrations + constants |
| `crates/cognitive/src/mirror/facade.rs` | `generate_weekly_narrative` method body (keep stub) |

---

### Task 1: Storage — reforge_state table and repo

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`
- Create: `crates/storage/src/rows/reforge_state.rs`
- Create: `crates/storage/src/repos/reforge_state.rs`
- Modify: `crates/storage/src/rows/mod.rs`
- Modify: `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Add reforge_state table to migration**

Append to `crates/storage/migrations/001_initial.sql`:

```sql
-- ============================================================
-- Reforge Cycle State
-- ============================================================
CREATE TABLE reforge_state (
    id              TEXT PRIMARY KEY DEFAULT 'singleton',
    last_run_at     TEXT,
    last_run_stats  TEXT,
    run_count       INTEGER NOT NULL DEFAULT 0
);

INSERT INTO reforge_state (id) VALUES ('singleton');
```

- [ ] **Step 2: Create row type**

Create `crates/storage/src/rows/reforge_state.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReforgeStateRow {
    pub id: String,
    pub last_run_at: Option<String>,
    pub last_run_stats: Option<String>,
    pub run_count: i64,
}
```

Add to `crates/storage/src/rows/mod.rs`:
```rust
mod reforge_state;
pub use reforge_state::ReforgeStateRow;
```

- [ ] **Step 3: Create repo with tests**

Create `crates/storage/src/repos/reforge_state.rs`:

```rust
//! Repository for the `reforge_state` singleton table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::ReforgeStateRow;

#[derive(Debug, Clone)]
pub struct ReforgeStateRepo {
    pool: SqlitePool,
}

impl ReforgeStateRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get the singleton reforge state.
    pub async fn get(&self) -> Result<ReforgeStateRow, StorageError> {
        let row = sqlx::query_as::<_, ReforgeStateRow>(
            "SELECT id, last_run_at, last_run_stats, run_count FROM reforge_state WHERE id = 'singleton'",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Record a successful Reforge run.
    pub async fn record_run(&self, stats_json: &str) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE reforge_state SET
                last_run_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                last_run_stats = ?1,
                run_count = run_count + 1
             WHERE id = 'singleton'",
        )
        .bind(stats_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::StoragePool;

    #[tokio::test]
    async fn test_get_initial_state() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ReforgeStateRepo::new(pool.inner().clone());
        let state = repo.get().await.unwrap();
        assert!(state.last_run_at.is_none());
        assert_eq!(state.run_count, 0);
    }

    #[tokio::test]
    async fn test_record_run_updates_state() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ReforgeStateRepo::new(pool.inner().clone());
        repo.record_run(r#"{"facts_added": 3}"#).await.unwrap();
        let state = repo.get().await.unwrap();
        assert!(state.last_run_at.is_some());
        assert_eq!(state.run_count, 1);
        assert!(state.last_run_stats.unwrap().contains("facts_added"));
    }

    #[tokio::test]
    async fn test_record_run_increments_count() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ReforgeStateRepo::new(pool.inner().clone());
        repo.record_run("{}").await.unwrap();
        repo.record_run("{}").await.unwrap();
        let state = repo.get().await.unwrap();
        assert_eq!(state.run_count, 2);
    }
}
```

- [ ] **Step 4: Register in Repos aggregate**

In `crates/storage/src/repos/mod.rs`, add:
```rust
pub mod reforge_state;
pub use reforge_state::ReforgeStateRepo;
```

Add field to `Repos` struct:
```rust
pub reforge_state: ReforgeStateRepo,
```

Add to `from_pool()`:
```rust
reforge_state: ReforgeStateRepo::new(db.clone()),
```

- [ ] **Step 5: Run tests and verify**

Run: `cargo nextest run -p storage -E 'test(reforge)'`
Expected: 3 tests pass

- [ ] **Step 6: Delete dev database and rebuild**

Since we modified `001_initial.sql` (pre-release, no migrations needed):
```bash
rm -f ~/.klyntbot-dev/data.db
cargo build -p storage
```

- [ ] **Step 7: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): add reforge_state table and repo"
```

---

### Task 2: Storage — skill_versions table and repo

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`
- Create: `crates/storage/src/rows/skill_version.rs`
- Create: `crates/storage/src/repos/skill_version.rs`
- Modify: `crates/storage/src/rows/mod.rs`
- Modify: `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Add skill_versions table to migration**

Append to `crates/storage/migrations/001_initial.sql`:

```sql
-- ============================================================
-- Skill Version History
-- ============================================================
CREATE TABLE skill_versions (
    id          TEXT PRIMARY KEY,
    skill_name  TEXT NOT NULL,
    version     INTEGER NOT NULL,
    file_path   TEXT NOT NULL,
    content     TEXT NOT NULL,
    diff        TEXT,
    source      TEXT NOT NULL,
    reason      TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_skill_versions_name ON skill_versions(skill_name, version);
CREATE INDEX idx_skill_versions_lookup ON skill_versions(skill_name, file_path, version);
```

- [ ] **Step 2: Create row type**

Create `crates/storage/src/rows/skill_version.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SkillVersionRow {
    pub id: String,
    pub skill_name: String,
    pub version: i64,
    pub file_path: String,
    pub content: String,
    pub diff: Option<String>,
    pub source: String,
    pub reason: Option<String>,
    pub created_at: String,
}
```

Add to `crates/storage/src/rows/mod.rs`:
```rust
mod skill_version;
pub use skill_version::SkillVersionRow;
```

- [ ] **Step 3: Create repo with tests**

Create `crates/storage/src/repos/skill_version.rs`:

```rust
//! Repository for the `skill_versions` table — tracks skill file version history.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::SkillVersionRow;

#[derive(Debug, Clone)]
pub struct SkillVersionRepo {
    pool: SqlitePool,
}

impl SkillVersionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new version for a skill file.
    pub async fn insert(&self, row: &SkillVersionRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO skill_versions (id, skill_name, version, file_path, content, diff, source, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(&row.id)
        .bind(&row.skill_name)
        .bind(row.version)
        .bind(&row.file_path)
        .bind(&row.content)
        .bind(&row.diff)
        .bind(&row.source)
        .bind(&row.reason)
        .bind(&row.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get the latest version number for a skill file.
    pub async fn latest_version(
        &self,
        skill_name: &str,
        file_path: &str,
    ) -> Result<Option<SkillVersionRow>, StorageError> {
        let row = sqlx::query_as::<_, SkillVersionRow>(
            "SELECT id, skill_name, version, file_path, content, diff, source, reason, created_at
             FROM skill_versions
             WHERE skill_name = ?1 AND file_path = ?2
             ORDER BY version DESC LIMIT 1",
        )
        .bind(skill_name)
        .bind(file_path)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get all versions for a skill (for Brain page history).
    pub async fn list_versions(&self, skill_name: &str) -> Result<Vec<SkillVersionRow>, StorageError> {
        let rows = sqlx::query_as::<_, SkillVersionRow>(
            "SELECT id, skill_name, version, file_path, content, diff, source, reason, created_at
             FROM skill_versions
             WHERE skill_name = ?1
             ORDER BY version DESC",
        )
        .bind(skill_name)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get a specific version by skill name and version number.
    pub async fn get_version(
        &self,
        skill_name: &str,
        version: i64,
    ) -> Result<Vec<SkillVersionRow>, StorageError> {
        let rows = sqlx::query_as::<_, SkillVersionRow>(
            "SELECT id, skill_name, version, file_path, content, diff, source, reason, created_at
             FROM skill_versions
             WHERE skill_name = ?1 AND version = ?2",
        )
        .bind(skill_name)
        .bind(version)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List all unique skill names that have versions.
    pub async fn list_skill_names(&self) -> Result<Vec<String>, StorageError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT skill_name FROM skill_versions ORDER BY skill_name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(n,)| n).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::StoragePool;

    fn test_version(skill: &str, version: i64, source: &str) -> SkillVersionRow {
        SkillVersionRow {
            id: uuid::Uuid::new_v4().to_string(),
            skill_name: skill.to_string(),
            version,
            file_path: "SKILL.md".to_string(),
            content: format!("content v{version}"),
            diff: Some(format!("diff v{version}")),
            source: source.to_string(),
            reason: Some(format!("reason v{version}")),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn test_insert_and_get_latest() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SkillVersionRepo::new(pool.inner().clone());

        repo.insert(&test_version("general", 1, "Seed")).await.unwrap();
        repo.insert(&test_version("general", 2, "Reforge")).await.unwrap();

        let latest = repo.latest_version("general", "SKILL.md").await.unwrap().unwrap();
        assert_eq!(latest.version, 2);
        assert_eq!(latest.source, "Reforge");
    }

    #[tokio::test]
    async fn test_list_versions() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SkillVersionRepo::new(pool.inner().clone());

        repo.insert(&test_version("general", 1, "Seed")).await.unwrap();
        repo.insert(&test_version("general", 2, "Reforge")).await.unwrap();
        repo.insert(&test_version("general", 3, "User")).await.unwrap();

        let versions = repo.list_versions("general").await.unwrap();
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].version, 3); // DESC order
    }

    #[tokio::test]
    async fn test_latest_version_returns_none_for_unknown() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SkillVersionRepo::new(pool.inner().clone());
        let latest = repo.latest_version("nonexistent", "SKILL.md").await.unwrap();
        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn test_list_skill_names() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SkillVersionRepo::new(pool.inner().clone());

        repo.insert(&test_version("general", 1, "Seed")).await.unwrap();
        repo.insert(&test_version("finance", 1, "Seed")).await.unwrap();

        let names = repo.list_skill_names().await.unwrap();
        assert_eq!(names, vec!["finance", "general"]);
    }
}
```

- [ ] **Step 4: Register in Repos aggregate**

In `crates/storage/src/repos/mod.rs`, add:
```rust
pub mod skill_version;
pub use skill_version::SkillVersionRepo;
```

Add field to `Repos` struct and `from_pool()` (same pattern as Task 1 Step 4).

- [ ] **Step 5: Run tests and verify**

Run: `cargo nextest run -p storage -E 'test(skill_version)'`
Expected: 4 tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): add skill_versions table and repo"
```

---

### Task 3: Skill File Manager — read/write/diff/hash

**Files:**
- Create: `crates/cognitive/src/services/reforge/mod.rs`
- Create: `crates/cognitive/src/services/reforge/skill_files.rs`
- Modify: `crates/cognitive/src/services/mod.rs`

- [ ] **Step 1: Create reforge module**

Create `crates/cognitive/src/services/reforge/mod.rs`:

```rust
pub mod skill_files;
```

Add to `crates/cognitive/src/services/mod.rs`:

```rust
pub mod reforge;
```

- [ ] **Step 2: Implement SkillFileManager**

Create `crates/cognitive/src/services/reforge/skill_files.rs`:

```rust
//! Manage skill files on disk: read, write, diff, hash, and seed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Represents a single file within a skill directory.
#[derive(Debug, Clone)]
pub struct SkillFile {
    pub skill_name: String,
    pub file_path: String, // relative: "SKILL.md", "references/cron.md"
    pub content: String,
    pub content_hash: String,
}

/// Manages skill files on disk at `~/.klyntbot/skills/`.
pub struct SkillFileManager {
    skills_dir: PathBuf,
}

impl SkillFileManager {
    pub fn new(skills_dir: PathBuf) -> Self {
        Self { skills_dir }
    }

    /// Read all files for all skills. Returns a map of skill_name → Vec<SkillFile>.
    pub fn read_all(&self) -> HashMap<String, Vec<SkillFile>> {
        let mut result: HashMap<String, Vec<SkillFile>> = HashMap::new();
        let entries = match std::fs::read_dir(&self.skills_dir) {
            Ok(e) => e,
            Err(_) => return result,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) if !n.starts_with('.') => n.to_string(),
                _ => continue,
            };
            let files = self.read_skill_dir(&skill_name, &path);
            if !files.is_empty() {
                result.insert(skill_name, files);
            }
        }
        result
    }

    /// Read all files for a single skill directory.
    fn read_skill_dir(&self, skill_name: &str, dir: &Path) -> Vec<SkillFile> {
        let mut files = Vec::new();
        self.collect_files(skill_name, dir, dir, &mut files);
        files
    }

    fn collect_files(
        &self,
        skill_name: &str,
        base: &Path,
        dir: &Path,
        out: &mut Vec<SkillFile>,
    ) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.collect_files(skill_name, base, &path, out);
            } else if path.extension().is_some_and(|e| e == "md") {
                let relative = path.strip_prefix(base).unwrap_or(&path);
                let relative_str = relative.to_string_lossy().to_string();
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let hash = content_hash(&content);
                    out.push(SkillFile {
                        skill_name: skill_name.to_string(),
                        file_path: relative_str,
                        content,
                        content_hash: hash,
                    });
                }
            }
        }
    }

    /// Write a file to disk for a skill. Creates parent directories if needed.
    pub fn write_file(
        &self,
        skill_name: &str,
        file_path: &str,
        content: &str,
    ) -> std::io::Result<()> {
        let full_path = self.skills_dir.join(skill_name).join(file_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, content)
    }

    /// Seed skills from compiled defaults if the skills directory is empty.
    /// Returns the number of skills seeded.
    pub fn seed_if_empty(&self, defaults: &HashMap<String, Vec<(&str, &str)>>) -> std::io::Result<usize> {
        // Check if any skill directories exist
        let has_skills = std::fs::read_dir(&self.skills_dir)
            .ok()
            .map(|entries| {
                entries
                    .flatten()
                    .any(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.'))
            })
            .unwrap_or(false);

        if has_skills {
            return Ok(0);
        }

        std::fs::create_dir_all(&self.skills_dir)?;
        let mut count = 0;
        for (skill_name, files) in defaults {
            for (file_path, content) in files {
                self.write_file(skill_name, file_path, content)?;
            }
            count += 1;
        }
        Ok(count)
    }

    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }
}

/// Compute SHA-256 hash of content (hex-encoded, first 16 chars).
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8]) // 16 hex chars
}

/// Compute a unified diff between two strings.
pub fn compute_diff(old: &str, new: &str) -> String {
    use std::fmt::Write;
    let diff = similar::TextDiff::from_lines(old, new);
    let mut output = String::new();
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            similar::ChangeTag::Delete => "-",
            similar::ChangeTag::Insert => "+",
            similar::ChangeTag::Equal => " ",
        };
        let _ = write!(output, "{sign}{change}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn test_content_hash_different_for_different_content() {
        let h1 = content_hash("hello");
        let h2 = content_hash("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_compute_diff() {
        let diff = compute_diff("line1\nline2\n", "line1\nline3\n");
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+line3"));
    }

    #[test]
    fn test_read_all_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let mgr = SkillFileManager::new(tmp.path().to_path_buf());
        let result = mgr.read_all();
        assert!(result.is_empty());
    }

    #[test]
    fn test_write_and_read() {
        let tmp = TempDir::new().unwrap();
        let mgr = SkillFileManager::new(tmp.path().to_path_buf());

        mgr.write_file("test-skill", "SKILL.md", "---\nname: test\n---\nBody")
            .unwrap();
        mgr.write_file("test-skill", "references/guide.md", "Guide content")
            .unwrap();

        let all = mgr.read_all();
        let files = all.get("test-skill").unwrap();
        assert_eq!(files.len(), 2);

        let skill_md = files.iter().find(|f| f.file_path == "SKILL.md").unwrap();
        assert!(skill_md.content.contains("Body"));

        let ref_md = files
            .iter()
            .find(|f| f.file_path.contains("guide"))
            .unwrap();
        assert!(ref_md.content.contains("Guide"));
    }

    #[test]
    fn test_seed_if_empty() {
        let tmp = TempDir::new().unwrap();
        let mgr = SkillFileManager::new(tmp.path().to_path_buf());

        let mut defaults = HashMap::new();
        defaults.insert(
            "general".to_string(),
            vec![("SKILL.md", "---\nname: general\n---\nGeneral skill")],
        );

        let count = mgr.seed_if_empty(&defaults).unwrap();
        assert_eq!(count, 1);

        // Second call should skip (directory not empty)
        let count2 = mgr.seed_if_empty(&defaults).unwrap();
        assert_eq!(count2, 0);
    }
}
```

- [ ] **Step 3: Add `similar`, `sha2`, and `hex` dependencies to cognitive crate**

In `crates/cognitive/Cargo.toml`:
```toml
similar = "2"
sha2 = "0.10"
hex = "0.4"
```

Also add `tempfile` to dev-dependencies:
```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(skill_file)'`
Expected: 6 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add SkillFileManager for reading/writing/diffing skills"
```

---

### Task 4: Reforge types and handler trait

**Files:**
- Create: `crates/cognitive/src/services/reforge/types.rs`
- Modify: `crates/cognitive/src/services/reforge/mod.rs`

- [ ] **Step 1: Define all Reforge types**

Create `crates/cognitive/src/services/reforge/types.rs`:

```rust
//! Types for the Reforge Cycle input/output across all phases.

use serde::{Deserialize, Serialize};

use crate::types::{EpisodicMemory, ProceduralRule, SemanticFact, UserModel};

/// Session scratchpad with metadata for grouping.
#[derive(Debug, Clone, Serialize)]
pub struct SessionContext {
    pub session_key: String,
    pub scratchpad: String,
    pub updated_at: String,
    pub turn_count: i64,
}

/// Routing snapshot summary for a skill.
#[derive(Debug, Clone, Serialize)]
pub struct RoutingSummary {
    pub skill_name: String,
    pub message_count: u32,
    pub avg_confidence: f64,
    pub fallback_rate: f64,
}

/// Full input for Phase 1 → Phase 2.
#[derive(Debug, Clone)]
pub struct ReforgeCollected {
    pub sessions: Vec<SessionContext>,
    pub episodic_memories: Vec<EpisodicMemory>,
    pub user_model: UserModel,
    pub rules: Vec<ProceduralRule>,
    pub routing_summaries: Vec<RoutingSummary>,
    pub pending_meta_rules: Vec<String>,
    pub skill_files: std::collections::HashMap<String, Vec<super::skill_files::SkillFile>>,
    pub retrieval_precision: Option<f64>,
    pub is_bootstrap: bool,
}

// ── Phase 2: Knowledge Synthesis ────────────────────────────────

/// Input for LLM call #1.
#[derive(Debug, Clone, Serialize)]
pub struct SynthesizeInput {
    pub sessions: Vec<SessionContext>,
    pub episodic_memories: Vec<EpisodicSummary>,
    pub user_model_summary: String,
    pub rules_summary: String,
    pub retrieval_precision: Option<f64>,
}

/// Abbreviated episodic memory for prompt inclusion.
#[derive(Debug, Clone, Serialize)]
pub struct EpisodicSummary {
    pub domain: String,
    pub summary: String,
    pub occurred_at: String,
}

/// Output from LLM call #1.
#[derive(Debug, Clone, Deserialize)]
pub struct SynthesizeOutput {
    #[serde(default)]
    pub fact_updates: Vec<FactUpdate>,
    #[serde(default)]
    pub rule_updates: Vec<RuleUpdate>,
    #[serde(default)]
    pub stale_facts: Vec<StaleFact>,
    #[serde(default)]
    pub cross_session_patterns: Vec<CrossSessionPattern>,
    pub extraction_quality_flag: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactUpdate {
    pub action: String, // "add", "update", "remove"
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub domain: String,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuleUpdate {
    pub action: String, // "add", "update", "reinforce"
    pub rule_text: String,
    pub domain: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StaleFact {
    pub fact_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrossSessionPattern {
    pub pattern: String,
    pub confidence: f64,
}

// ── Phase 3: Skills & Behavior Review ───────────────────────────

/// Input for LLM call #2.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewInput {
    pub pending_meta_rules: Vec<String>,
    pub routing_summaries: Vec<RoutingSummary>,
    pub skill_contents: Vec<SkillContent>,
    pub new_facts_summary: String,
    pub retrieval_precision: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillContent {
    pub skill_name: String,
    pub file_path: String,
    pub content: String,
}

/// Output from LLM call #2.
#[derive(Debug, Clone, Deserialize)]
pub struct ReviewOutput {
    #[serde(default)]
    pub skill_edits: Vec<SkillEdit>,
    #[serde(default)]
    pub routing_insights: Vec<String>,
    #[serde(default)]
    pub context_priority_suggestions: Vec<ContextPrioritySuggestion>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillEdit {
    pub skill_name: String,
    pub file_path: String,
    pub edit_type: String, // "frontmatter", "body_replace", "body_insert", "body_remove"
    pub field: Option<String>,
    pub new_value: Option<String>,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub section: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContextPrioritySuggestion {
    pub source: String,
    pub suggestion: String,
    pub reason: String,
}

// ── Phase 4: Narrative ──────────────────────────────────────────

/// Input for LLM call #3.
#[derive(Debug, Clone, Serialize)]
pub struct NarrateInput {
    pub synthesize_summary: String,
    pub review_summary: String,
    pub routing_summary: String,
}

/// Full result of a Reforge cycle.
#[derive(Debug, Clone, Default)]
pub struct ReforgeResult {
    pub facts_added: u32,
    pub facts_updated: u32,
    pub facts_stale_flagged: u32,
    pub rules_added: u32,
    pub rules_reinforced: u32,
    pub skills_edited: u32,
    pub narrative: String,
    pub skipped_skill_edits: Vec<String>,
    pub phase_errors: Vec<String>,
}
```

- [ ] **Step 2: Define ReforgeHandler trait**

Add to `crates/cognitive/src/services/reforge/mod.rs`:

```rust
pub mod collector;
pub mod skill_files;
pub mod types;

use async_trait::async_trait;

/// Trait for LLM-backed Reforge operations. Implemented in the agent crate
/// with actual LLM calls; can be stubbed for testing.
#[async_trait]
pub trait ReforgeHandler: Send + Sync {
    /// Phase 2: Knowledge synthesis from sessions + episodics.
    async fn synthesize(
        &self,
        input: &types::SynthesizeInput,
    ) -> common::Result<types::SynthesizeOutput>;

    /// Phase 3: Skill & behavior review from corrections + routing.
    async fn review(&self, input: &types::ReviewInput) -> common::Result<types::ReviewOutput>;

    /// Phase 4: Generate human-readable narrative.
    async fn narrate(&self, input: &types::NarrateInput) -> common::Result<String>;
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p cognitive`
Expected: Compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add Reforge types and ReforgeHandler trait"
```

---

### Task 5: Reforge Collector (Phase 1)

**Files:**
- Create: `crates/cognitive/src/services/reforge/collector.rs`

- [ ] **Step 1: Implement the collector**

Create `crates/cognitive/src/services/reforge/collector.rs`:

```rust
//! Phase 1: Collect all data needed for the Reforge cycle.

use std::collections::HashMap;

use storage::repos::Repos;
use tracing::{debug, warn};

use crate::repos::{
    EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo, USER_MODEL_DOMAINS,
};
use crate::types::UserModel;

use super::skill_files::SkillFileManager;
use super::types::{ReforgeCollected, RoutingSummary, SessionContext};

/// Minimum new data threshold to proceed with Reforge.
const MIN_NEW_DATA: usize = 1;

/// Collect all inputs for the Reforge cycle.
///
/// Returns `None` if there's no new data since `last_run_at` (skip gate).
pub async fn collect(
    last_run_at: Option<&str>,
    repos: &Repos,
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
    rule_repo: &ProceduralRuleRepo,
    mirror_repo: Option<&crate::mirror::MirrorRepo>,
    meta_rule_repo: Option<&crate::mirror::MirrorRepo>,
    skill_mgr: &SkillFileManager,
    feedback_repo: Option<&storage::RetrievalFeedbackRepo>,
) -> Option<ReforgeCollected> {
    let is_bootstrap = last_run_at.is_none();
    let since = last_run_at.unwrap_or_else(|| {
        // Bootstrap: 7 days ago
        Box::leak(
            (chrono::Utc::now() - chrono::Duration::days(7))
                .to_rfc3339()
                .into_boxed_str(),
        )
    });

    // Load session scratchpads since last run
    let sessions: Vec<SessionContext> = repos
        .session_memory
        .list_since(since)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| SessionContext {
            session_key: row.session_key,
            scratchpad: row.content,
            updated_at: row.updated_at,
            turn_count: row.turn_count,
        })
        .collect();

    // Load episodic memories since last run
    let period_end = chrono::Utc::now().to_rfc3339();
    let episodic_memories = episodic_repo
        .list_range(since, &period_end)
        .await
        .unwrap_or_default();

    // Skip gate: no new data
    if !is_bootstrap && sessions.is_empty() && episodic_memories.is_empty() {
        debug!("Reforge skipped: no new data since {since}");
        return None;
    }

    // Load current user model (full)
    let user_model = load_user_model(fact_repo).await;

    // Load active procedural rules (full)
    let mut rules = Vec::new();
    for domain in crate::repos::RULE_DOMAINS {
        if let Ok(domain_rules) = rule_repo.list_active(domain).await {
            rules.extend(domain_rules);
        }
    }

    // Load routing snapshots since last run
    let routing_summaries = if let Some(mirror) = mirror_repo {
        load_routing_summaries(mirror, since).await
    } else {
        Vec::new()
    };

    // Load pending MetaRules
    let pending_meta_rules = if let Some(mirror) = meta_rule_repo {
        mirror
            .list_pending_meta_rules()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|mr| mr.description)
            .collect()
    } else {
        Vec::new()
    };

    // Read all skill files from disk
    let skill_files = skill_mgr.read_all();

    // Retrieval feedback precision
    let retrieval_precision = if let Some(fb) = feedback_repo {
        fb.avg_precision_since(since).await.ok()
    } else {
        None
    };

    debug!(
        "Reforge collected: {} sessions, {} episodics, {} rules, {} routing snapshots, {} skills",
        sessions.len(),
        episodic_memories.len(),
        rules.len(),
        routing_summaries.len(),
        skill_files.len(),
    );

    Some(ReforgeCollected {
        sessions,
        episodic_memories,
        user_model,
        rules,
        routing_summaries,
        pending_meta_rules,
        skill_files,
        retrieval_precision,
        is_bootstrap,
    })
}

async fn load_user_model(fact_repo: &SemanticFactRepo) -> UserModel {
    let mut model = UserModel::default();
    for domain in USER_MODEL_DOMAINS {
        if let Ok(facts) = fact_repo.list_active(domain).await {
            model.add_domain(domain, facts);
        }
    }
    model
}

async fn load_routing_summaries(
    _mirror: &crate::mirror::MirrorRepo,
    _since: &str,
) -> Vec<RoutingSummary> {
    // Aggregate routing snapshots since last run into per-skill summaries.
    // This will be implemented when we wire the mirror repo.
    Vec::new()
}
```

Note: The `list_since` method on `SessionMemoryRepo` and `list_pending_meta_rules` on `MirrorRepo` may not exist yet. The implementer should add them — they're simple `WHERE updated_at > ?1` queries following existing patterns.

- [ ] **Step 2: Add `list_since` to SessionMemoryRepo**

In `crates/storage/src/repos/session_memory.rs`, add:

```rust
/// List session memories updated since a given timestamp.
pub async fn list_since(&self, since: &str) -> Result<Vec<SessionMemoryRow>, StorageError> {
    let rows = sqlx::query_as::<_, SessionMemoryRow>(
        "SELECT session_key, content, turn_count, updated_at
         FROM session_memory
         WHERE updated_at > ?1
         ORDER BY updated_at DESC",
    )
    .bind(since)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows)
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p cognitive`
Expected: Compiles (some warnings about unused fields are OK at this stage)

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/ crates/storage/
git commit -m "feat(cognitive): add Reforge collector (Phase 1 data gathering)"
```

---

### Task 6: LLM Reforge Handlers (Phases 2-4)

**Files:**
- Create: `crates/agent/src/adapters/reforge_handlers.rs`
- Modify: `crates/agent/src/adapters/mod.rs`

- [ ] **Step 1: Implement the LLM handler**

Create `crates/agent/src/adapters/reforge_handlers.rs`:

```rust
//! LLM implementations of the ReforgeHandler trait for Phases 2-4.

use async_trait::async_trait;
use cognitive::services::reforge::types::*;
use cognitive::services::reforge::ReforgeHandler;
use providers::{ChatParams, DynProvider, Message, ResponseFormat};
use tracing::warn;

const SYNTHESIZE_PROMPT: &str = r#"You are a knowledge consolidation engine for a personal AI assistant.
Analyze the user's recent sessions and episodic memories against their existing knowledge base.

For each session, look for:
1. New facts about the user (preferences, habits, skills, context)
2. Changes to existing beliefs (contradictions, updates, refinements)
3. Cross-session patterns (behaviors that appear across multiple sessions)
4. Stale facts no longer supported by recent evidence

Respond with JSON:
{
  "fact_updates": [{"action":"add"|"update"|"remove","subject":"...","predicate":"...","object":"...","domain":"...","confidence":0.0-1.0,"reason":"..."}],
  "rule_updates": [{"action":"add"|"update"|"reinforce","rule_text":"...","domain":"...","reason":"..."}],
  "stale_facts": [{"fact_id":"...","reason":"..."}],
  "cross_session_patterns": [{"pattern":"...","confidence":0.0-1.0}],
  "extraction_quality_flag": null
}"#;

const REVIEW_PROMPT: &str = r#"You are a skill improvement engine for a personal AI assistant.
Analyze correction patterns and routing data to propose targeted skill edits.

For each skill, consider:
1. Does whenToUse cover the actual trigger phrases users employ?
2. Are there correction patterns that indicate bad instructions in the skill body?
3. Do reference files need updates based on learned patterns?

Only propose edits with clear evidence (corrections, routing data, patterns). Do not speculate.

Respond with JSON:
{
  "skill_edits": [{"skill_name":"...","file_path":"...","edit_type":"frontmatter"|"body_replace"|"body_insert"|"body_remove","field":null,"new_value":null,"old_text":null,"new_text":null,"section":null,"reason":"..."}],
  "routing_insights": ["..."],
  "context_priority_suggestions": [{"source":"...","suggestion":"...","reason":"..."}]
}"#;

const NARRATE_PROMPT: &str = r#"Summarize tonight's Reforge cycle for the user in 2-3 concise paragraphs.
Include: what was learned, what changed (facts, rules, skills), and any notable patterns.
Be conversational, not clinical. Address the user directly."#;

pub struct LlmReforgeHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmReforgeHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonObject),
        }
    }
}

#[async_trait]
impl ReforgeHandler for LlmReforgeHandler {
    async fn synthesize(&self, input: &SynthesizeInput) -> common::Result<SynthesizeOutput> {
        let user_msg = format_synthesize_input(input);
        let messages = vec![
            Message::system(SYNTHESIZE_PROMPT),
            Message::user(&user_msg),
        ];
        let response = self.provider.chat(&messages, &self.params).await?;
        let text = response.content_text();
        serde_json::from_str(&text).map_err(|e| {
            warn!("Failed to parse synthesize output: {e}");
            common::KlyntbotError::Internal(format!("Reforge synthesize parse error: {e}"))
        })
    }

    async fn review(&self, input: &ReviewInput) -> common::Result<ReviewOutput> {
        let user_msg = format_review_input(input);
        let messages = vec![
            Message::system(REVIEW_PROMPT),
            Message::user(&user_msg),
        ];
        let response = self.provider.chat(&messages, &self.params).await?;
        let text = response.content_text();
        serde_json::from_str(&text).map_err(|e| {
            warn!("Failed to parse review output: {e}");
            common::KlyntbotError::Internal(format!("Reforge review parse error: {e}"))
        })
    }

    async fn narrate(&self, input: &NarrateInput) -> common::Result<String> {
        let user_msg = format!(
            "## Knowledge Synthesis\n{}\n\n## Skill Review\n{}\n\n## Routing\n{}",
            input.synthesize_summary, input.review_summary, input.routing_summary
        );
        let params = self.params.clone().with_response_format(ResponseFormat::Text);
        let messages = vec![
            Message::system(NARRATE_PROMPT),
            Message::user(&user_msg),
        ];
        let response = self.provider.chat(&messages, &params).await?;
        Ok(response.content_text())
    }
}

fn format_synthesize_input(input: &SynthesizeInput) -> String {
    let mut out = String::new();
    out.push_str("## Sessions Since Last Reforge\n\n");
    for session in &input.sessions {
        out.push_str(&format!(
            "[Session {} — {}]\nScratchpad:\n{}\n\n",
            session.session_key, session.updated_at, session.scratchpad
        ));
    }
    out.push_str("## Episodic Memories\n\n");
    for ep in &input.episodic_memories {
        out.push_str(&format!("- [{}] {}: {}\n", ep.occurred_at, ep.domain, ep.summary));
    }
    out.push_str(&format!("\n## Current User Model\n{}\n", input.user_model_summary));
    out.push_str(&format!("\n## Active Rules\n{}\n", input.rules_summary));
    if let Some(precision) = input.retrieval_precision {
        out.push_str(&format!(
            "\n## Retrieval Feedback\nAverage precision: {:.2}\n",
            precision
        ));
    }
    out
}

fn format_review_input(input: &ReviewInput) -> String {
    let mut out = String::new();
    out.push_str("## Pending MetaRules (correction patterns)\n");
    for mr in &input.pending_meta_rules {
        out.push_str(&format!("- {mr}\n"));
    }
    out.push_str("\n## Routing Summary\n");
    for rs in &input.routing_summaries {
        out.push_str(&format!(
            "{}: {}% ({} msgs, avg confidence {:.2})\n",
            rs.skill_name,
            if rs.message_count > 0 { 100 } else { 0 },
            rs.message_count,
            rs.avg_confidence,
        ));
    }
    out.push_str("\n## Current Skills\n\n");
    for skill in &input.skill_contents {
        out.push_str(&format!(
            "### {} ({})\n```\n{}\n```\n\n",
            skill.skill_name, skill.file_path, skill.content
        ));
    }
    if !input.new_facts_summary.is_empty() {
        out.push_str(&format!(
            "\n## New Knowledge from Synthesis\n{}\n",
            input.new_facts_summary
        ));
    }
    out
}
```

- [ ] **Step 2: Export the module**

In `crates/agent/src/adapters/mod.rs`, add:
```rust
pub mod reforge_handlers;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p agent`
Expected: Compiles (may need to adjust import paths for Message, ChatParams, etc. based on actual provider API)

- [ ] **Step 4: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): add LLM Reforge handlers for synthesize/review/narrate"
```

---

### Task 7: ReforgeService orchestrator (all phases)

**Files:**
- Create: `crates/cognitive/src/services/reforge/service.rs`
- Modify: `crates/cognitive/src/services/reforge/mod.rs`

This is the central orchestrator that ties all phases together. It calls the collector, handler, and applies results.

- [ ] **Step 1: Implement ReforgeService**

Create `crates/cognitive/src/services/reforge/service.rs`:

```rust
//! ReforgeService — orchestrates the 7-phase nightly Reforge cycle.

use std::collections::HashMap;

use storage::repos::Repos;
use storage::rows::SkillVersionRow;
use tracing::{debug, info, warn};

use crate::repos::{EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo};
use crate::services::consolidation::{execute_memory_ops, ConsolidationHandler};
use crate::types::{EpisodicMemory, SemanticFact, DEFAULT_MEMORY_TYPE};

use super::collector;
use super::skill_files::{compute_diff, content_hash, SkillFileManager};
use super::types::*;
use super::ReforgeHandler;

pub struct ReforgeService<'a> {
    repos: &'a Repos,
    fact_repo: &'a SemanticFactRepo,
    episodic_repo: &'a EpisodicMemoryRepo,
    rule_repo: &'a ProceduralRuleRepo,
    consolidation: &'a dyn ConsolidationHandler,
    handler: &'a dyn ReforgeHandler,
    skill_mgr: &'a SkillFileManager,
    embedder: Option<&'a dyn crate::embedder::SemanticFactEmbedder>,
}

impl<'a> ReforgeService<'a> {
    pub fn new(
        repos: &'a Repos,
        fact_repo: &'a SemanticFactRepo,
        episodic_repo: &'a EpisodicMemoryRepo,
        rule_repo: &'a ProceduralRuleRepo,
        consolidation: &'a dyn ConsolidationHandler,
        handler: &'a dyn ReforgeHandler,
        skill_mgr: &'a SkillFileManager,
    ) -> Self {
        Self {
            repos,
            fact_repo,
            episodic_repo,
            rule_repo,
            consolidation,
            handler,
            skill_mgr,
            embedder: None,
        }
    }

    pub fn with_embedder(mut self, embedder: &'a dyn crate::embedder::SemanticFactEmbedder) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Run the full Reforge cycle. Returns result with stats or None if skipped.
    pub async fn run(&self) -> Option<ReforgeResult> {
        let mut result = ReforgeResult::default();

        // Load state
        let state = self.repos.reforge_state.get().await.ok()?;
        let last_run_at = state.last_run_at.as_deref();

        // Phase 1: Collect
        info!("Reforge Phase 1: Collecting data");
        let collected = collector::collect(
            last_run_at,
            self.repos,
            self.fact_repo,
            self.episodic_repo,
            self.rule_repo,
            None, // mirror_repo — wire later
            None, // meta_rule_repo — wire later
            self.skill_mgr,
            None, // feedback_repo — wire later
        )
        .await?;

        // Snapshot skill hashes before LLM calls (for conflict detection)
        let pre_hashes: HashMap<(String, String), String> = collected
            .skill_files
            .iter()
            .flat_map(|(name, files)| {
                files
                    .iter()
                    .map(|f| ((name.clone(), f.file_path.clone()), f.content_hash.clone()))
            })
            .collect();

        // Phase 2: Synthesize
        info!("Reforge Phase 2: Knowledge synthesis");
        let synthesize_output = match self.run_synthesize(&collected).await {
            Ok(output) => Some(output),
            Err(e) => {
                warn!("Reforge Phase 2 failed: {e}");
                result.phase_errors.push(format!("Synthesize: {e}"));
                None
            }
        };

        // Phase 3: Review
        info!("Reforge Phase 3: Skills & behavior review");
        let review_output = match self.run_review(&collected, &synthesize_output).await {
            Ok(output) => Some(output),
            Err(e) => {
                warn!("Reforge Phase 3 failed: {e}");
                result.phase_errors.push(format!("Review: {e}"));
                None
            }
        };

        // Phase 4: Narrate
        info!("Reforge Phase 4: Narrative generation");
        let narrative = match self
            .run_narrate(&synthesize_output, &review_output)
            .await
        {
            Ok(n) => n,
            Err(e) => {
                warn!("Reforge Phase 4 failed: {e}");
                result.phase_errors.push(format!("Narrate: {e}"));
                format!("Reforge completed but narrative generation failed: {e}")
            }
        };
        result.narrative = narrative.clone();

        // Phase 5: Apply
        info!("Reforge Phase 5: Applying changes");
        if let Some(ref synth) = synthesize_output {
            self.apply_knowledge(synth, &mut result).await;
        }
        if let Some(ref review) = review_output {
            self.apply_skill_edits(review, &pre_hashes, &mut result).await;
        }

        // Store narrative as episodic memory
        self.store_narrative(&narrative).await;

        // Record successful run
        let stats_json = serde_json::to_string(&serde_json::json!({
            "facts_added": result.facts_added,
            "facts_updated": result.facts_updated,
            "rules_added": result.rules_added,
            "rules_reinforced": result.rules_reinforced,
            "skills_edited": result.skills_edited,
        }))
        .unwrap_or_default();

        if let Err(e) = self.repos.reforge_state.record_run(&stats_json).await {
            warn!("Failed to record Reforge state: {e}");
        }

        info!(
            "Reforge complete: {} facts, {} rules, {} skills edited",
            result.facts_added + result.facts_updated,
            result.rules_added + result.rules_reinforced,
            result.skills_edited,
        );

        Some(result)
    }

    async fn run_synthesize(
        &self,
        collected: &ReforgeCollected,
    ) -> common::Result<SynthesizeOutput> {
        let input = SynthesizeInput {
            sessions: collected.sessions.clone(),
            episodic_memories: collected
                .episodic_memories
                .iter()
                .map(|e| EpisodicSummary {
                    domain: e.domain.clone(),
                    summary: e.summary.clone().unwrap_or_else(|| {
                        e.content.chars().take(120).collect()
                    }),
                    occurred_at: e.occurred_at.clone(),
                })
                .collect(),
            user_model_summary: format_user_model(&collected.user_model),
            rules_summary: collected
                .rules
                .iter()
                .map(|r| format!("[{}] {} (signals: {})", r.domain, r.rule_text, r.signal_count))
                .collect::<Vec<_>>()
                .join("\n"),
            retrieval_precision: collected.retrieval_precision,
        };
        self.handler.synthesize(&input).await
    }

    async fn run_review(
        &self,
        collected: &ReforgeCollected,
        synth: &Option<SynthesizeOutput>,
    ) -> common::Result<ReviewOutput> {
        let skill_contents: Vec<SkillContent> = collected
            .skill_files
            .iter()
            .flat_map(|(name, files)| {
                files.iter().map(move |f| SkillContent {
                    skill_name: name.clone(),
                    file_path: f.file_path.clone(),
                    content: f.content.clone(),
                })
            })
            .collect();

        let new_facts_summary = synth
            .as_ref()
            .map(|s| {
                s.fact_updates
                    .iter()
                    .map(|f| format!("{}: {} {} = {}", f.action, f.subject, f.predicate, f.object))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let input = ReviewInput {
            pending_meta_rules: collected.pending_meta_rules.clone(),
            routing_summaries: collected.routing_summaries.clone(),
            skill_contents,
            new_facts_summary,
            retrieval_precision: collected.retrieval_precision,
        };
        self.handler.review(&input).await
    }

    async fn run_narrate(
        &self,
        synth: &Option<SynthesizeOutput>,
        review: &Option<ReviewOutput>,
    ) -> common::Result<String> {
        let synth_summary = synth
            .as_ref()
            .map(|s| {
                format!(
                    "{} fact updates, {} rule updates, {} stale facts flagged",
                    s.fact_updates.len(),
                    s.rule_updates.len(),
                    s.stale_facts.len()
                )
            })
            .unwrap_or_else(|| "Knowledge synthesis skipped".to_string());

        let review_summary = review
            .as_ref()
            .map(|r| {
                format!(
                    "{} skill edits proposed, {} routing insights",
                    r.skill_edits.len(),
                    r.routing_insights.len()
                )
            })
            .unwrap_or_else(|| "Skill review skipped".to_string());

        let input = NarrateInput {
            synthesize_summary: synth_summary,
            review_summary,
            routing_summary: String::new(),
        };
        self.handler.narrate(&input).await
    }

    async fn apply_knowledge(&self, synth: &SynthesizeOutput, result: &mut ReforgeResult) {
        // Apply fact updates using existing consolidation pipeline
        for update in &synth.fact_updates {
            match update.action.as_str() {
                "add" => {
                    let fact = SemanticFact {
                        id: uuid::Uuid::new_v4().to_string(),
                        domain: update.domain.clone(),
                        subject: update.subject.clone(),
                        predicate: update.predicate.clone(),
                        object: update.object.clone(),
                        confidence: update.confidence,
                        source: "reforge".to_string(),
                        valid_from: chrono::Utc::now().to_rfc3339(),
                        valid_until: None,
                        recorded_at: chrono::Utc::now().to_rfc3339(),
                        superseded_at: None,
                        superseded_by: None,
                        stability: 1.0,
                        last_accessed: None,
                        access_count: 0,
                        convergence_score: 0.0,
                        project_id: None,
                        memory_type: DEFAULT_MEMORY_TYPE.to_string(),
                        scope_type: "system".to_string(),
                        scope_id: None,
                    };
                    if let Err(e) = self.fact_repo.upsert(&fact).await {
                        warn!("Failed to add fact: {e}");
                    } else {
                        result.facts_added += 1;
                    }
                }
                "update" => {
                    // Find existing fact with matching subject+predicate, supersede it
                    if let Ok(existing) = self
                        .fact_repo
                        .find_similar(&update.subject, &update.predicate)
                        .await
                    {
                        if let Some(old) = existing.first() {
                            let new_id = uuid::Uuid::new_v4().to_string();
                            let _ = self.fact_repo.supersede(&old.id, &new_id).await;
                            let fact = SemanticFact {
                                id: new_id,
                                domain: update.domain.clone(),
                                subject: update.subject.clone(),
                                predicate: update.predicate.clone(),
                                object: update.object.clone(),
                                confidence: update.confidence,
                                source: "reforge".to_string(),
                                valid_from: chrono::Utc::now().to_rfc3339(),
                                valid_until: None,
                                recorded_at: chrono::Utc::now().to_rfc3339(),
                                superseded_at: None,
                                superseded_by: None,
                                stability: 1.0,
                                last_accessed: None,
                                access_count: 0,
                                convergence_score: old.convergence_score,
                                project_id: None,
                                memory_type: DEFAULT_MEMORY_TYPE.to_string(),
                                scope_type: "system".to_string(),
                                scope_id: None,
                            };
                            if let Err(e) = self.fact_repo.upsert(&fact).await {
                                warn!("Failed to update fact: {e}");
                            } else {
                                result.facts_updated += 1;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Apply rule updates
        for update in &synth.rule_updates {
            match update.action.as_str() {
                "add" => {
                    // Dedup check
                    if let Ok(similar) = self
                        .rule_repo
                        .find_similar(&update.rule_text, &update.domain)
                        .await
                    {
                        if let Some(existing) = similar {
                            let _ = self.rule_repo.increment_signal_count(&existing.id).await;
                            result.rules_reinforced += 1;
                            continue;
                        }
                    }
                    let rule = crate::types::ProceduralRule {
                        id: uuid::Uuid::new_v4().to_string(),
                        domain: update.domain.clone(),
                        rule_text: update.rule_text.clone(),
                        confidence: 0.7,
                        source: "reforge".to_string(),
                        signal_count: 1,
                        created_at: chrono::Utc::now().to_rfc3339(),
                        updated_at: chrono::Utc::now().to_rfc3339(),
                        active: true,
                        project_id: None,
                        scope_type: "system".to_string(),
                        scope_id: None,
                    };
                    if let Err(e) = self.rule_repo.upsert(&rule).await {
                        warn!("Failed to add rule: {e}");
                    } else {
                        result.rules_added += 1;
                    }
                }
                "reinforce" => {
                    if let Ok(Some(existing)) = self
                        .rule_repo
                        .find_similar(&update.rule_text, &update.domain)
                        .await
                    {
                        let _ = self.rule_repo.increment_signal_count(&existing.id).await;
                        result.rules_reinforced += 1;
                    }
                }
                _ => {}
            }
        }

        // Flag stale facts
        for stale in &synth.stale_facts {
            // Reduce confidence of stale facts
            if let Ok(Some(fact)) = self.fact_repo.get(&stale.fact_id).await {
                let mut updated = fact;
                updated.confidence = (updated.confidence * 0.5).max(0.1);
                let _ = self.fact_repo.upsert(&updated).await;
                result.facts_stale_flagged += 1;
            }
        }
    }

    async fn apply_skill_edits(
        &self,
        review: &ReviewOutput,
        pre_hashes: &HashMap<(String, String), String>,
        result: &mut ReforgeResult,
    ) {
        for edit in &review.skill_edits {
            let key = (edit.skill_name.clone(), edit.file_path.clone());

            // Re-read current file to check for user edits since collection
            let current_files = self.skill_mgr.read_all();
            let current_hash = current_files
                .get(&edit.skill_name)
                .and_then(|files| files.iter().find(|f| f.file_path == edit.file_path))
                .map(|f| f.content_hash.clone());

            if let (Some(pre), Some(cur)) = (pre_hashes.get(&key), current_hash.as_ref()) {
                if pre != cur {
                    info!(
                        "Reforge: skipping edit to {}/{} — user modified since collection",
                        edit.skill_name, edit.file_path
                    );
                    result
                        .skipped_skill_edits
                        .push(format!("{}/{}", edit.skill_name, edit.file_path));
                    continue;
                }
            }

            // Get current content
            let current_content = current_files
                .get(&edit.skill_name)
                .and_then(|files| files.iter().find(|f| f.file_path == edit.file_path))
                .map(|f| f.content.clone())
                .unwrap_or_default();

            // Apply edit
            let new_content = match apply_single_edit(&current_content, edit) {
                Some(c) => c,
                None => {
                    warn!(
                        "Failed to apply edit to {}/{}: old_text not found",
                        edit.skill_name, edit.file_path
                    );
                    continue;
                }
            };

            // Write to disk
            if let Err(e) = self
                .skill_mgr
                .write_file(&edit.skill_name, &edit.file_path, &new_content)
            {
                warn!("Failed to write skill file: {e}");
                continue;
            }

            // Record version
            let latest_version = self
                .repos
                .skill_version
                .latest_version(&edit.skill_name, &edit.file_path)
                .await
                .ok()
                .flatten()
                .map(|v| v.version)
                .unwrap_or(0);

            let diff = compute_diff(&current_content, &new_content);
            let version_row = SkillVersionRow {
                id: uuid::Uuid::new_v4().to_string(),
                skill_name: edit.skill_name.clone(),
                version: latest_version + 1,
                file_path: edit.file_path.clone(),
                content: new_content,
                diff: Some(diff),
                source: "Reforge".to_string(),
                reason: Some(edit.reason.clone()),
                created_at: chrono::Utc::now().to_rfc3339(),
            };

            if let Err(e) = self.repos.skill_version.insert(&version_row).await {
                warn!("Failed to record skill version: {e}");
            } else {
                result.skills_edited += 1;
            }
        }
    }

    async fn store_narrative(&self, narrative: &str) {
        let memory = EpisodicMemory {
            id: uuid::Uuid::new_v4().to_string(),
            domain: "reforge".to_string(),
            content: narrative.to_string(),
            summary: Some(
                narrative
                    .chars()
                    .take(120)
                    .collect::<String>(),
            ),
            importance: 0.9,
            occurred_at: chrono::Utc::now().to_rfc3339(),
            recorded_at: chrono::Utc::now().to_rfc3339(),
            stability: 5.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            scope_type: "system".to_string(),
            scope_id: None,
        };
        if let Err(e) = self.episodic_repo.insert(&memory).await {
            warn!("Failed to store Reforge narrative: {e}");
        }
    }
}

/// Apply a single skill edit to content. Returns None if old_text not found.
fn apply_single_edit(content: &str, edit: &SkillEdit) -> Option<String> {
    match edit.edit_type.as_str() {
        "frontmatter" => {
            let field = edit.field.as_ref()?;
            let new_value = edit.new_value.as_ref()?;
            // Replace YAML frontmatter field
            let pattern = format!("{field}: ");
            if let Some(idx) = content.find(&pattern) {
                let line_end = content[idx..].find('\n').unwrap_or(content.len() - idx);
                let mut result = content.to_string();
                result.replace_range(idx..idx + line_end, &format!("{field}: {new_value}"));
                Some(result)
            } else {
                // Field doesn't exist — insert before closing ---
                let closing = content.rfind("---")?;
                let mut result = content.to_string();
                result.insert_str(closing, &format!("{field}: {new_value}\n"));
                Some(result)
            }
        }
        "body_replace" => {
            let old_text = edit.old_text.as_ref()?;
            let new_text = edit.new_text.as_ref()?;
            if content.contains(old_text.as_str()) {
                Some(content.replacen(old_text, new_text, 1))
            } else {
                None
            }
        }
        "body_insert" => {
            let section = edit.section.as_ref()?;
            let new_text = edit.new_text.as_ref()?;
            if let Some(idx) = content.find(section.as_str()) {
                let line_end = content[idx..].find('\n').unwrap_or(content.len() - idx);
                let insert_pos = idx + line_end;
                let mut result = content.to_string();
                result.insert_str(insert_pos, &format!("\n{new_text}"));
                Some(result)
            } else {
                // Section not found — append to end
                Some(format!("{content}\n{new_text}\n"))
            }
        }
        "body_remove" => {
            let old_text = edit.old_text.as_ref()?;
            if content.contains(old_text.as_str()) {
                Some(content.replacen(old_text, "", 1))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn format_user_model(model: &crate::types::UserModel) -> String {
    let mut out = String::new();
    for (domain, facts) in model.all_domains() {
        if facts.is_empty() {
            continue;
        }
        out.push_str(&format!("[{domain}]\n"));
        for fact in facts {
            out.push_str(&format!(
                "  {}: {} = {} (confidence: {:.0}%)\n",
                fact.subject,
                fact.predicate,
                fact.object,
                fact.confidence * 100.0
            ));
        }
    }
    out
}
```

- [ ] **Step 2: Add unit tests for `apply_single_edit`**

Add to the bottom of `service.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_frontmatter_replace() {
        let content = "---\nname: test\nwhenToUse: old value\n---\nBody";
        let edit = SkillEdit {
            skill_name: "test".into(),
            file_path: "SKILL.md".into(),
            edit_type: "frontmatter".into(),
            field: Some("whenToUse".into()),
            new_value: Some("new value".into()),
            old_text: None,
            new_text: None,
            section: None,
            reason: "test".into(),
        };
        let result = apply_single_edit(content, &edit).unwrap();
        assert!(result.contains("whenToUse: new value"));
        assert!(!result.contains("old value"));
    }

    #[test]
    fn test_apply_body_replace() {
        let content = "---\nname: test\n---\nDo thing A\nDo thing B";
        let edit = SkillEdit {
            skill_name: "test".into(),
            file_path: "SKILL.md".into(),
            edit_type: "body_replace".into(),
            field: None,
            new_value: None,
            old_text: Some("Do thing A".into()),
            new_text: Some("Do thing C".into()),
            section: None,
            reason: "test".into(),
        };
        let result = apply_single_edit(content, &edit).unwrap();
        assert!(result.contains("Do thing C"));
        assert!(!result.contains("Do thing A"));
    }

    #[test]
    fn test_apply_body_replace_not_found() {
        let content = "---\nname: test\n---\nBody";
        let edit = SkillEdit {
            skill_name: "test".into(),
            file_path: "SKILL.md".into(),
            edit_type: "body_replace".into(),
            field: None,
            new_value: None,
            old_text: Some("nonexistent".into()),
            new_text: Some("replacement".into()),
            section: None,
            reason: "test".into(),
        };
        assert!(apply_single_edit(content, &edit).is_none());
    }

    #[test]
    fn test_apply_body_insert() {
        let content = "## Section A\nContent A\n## Section B\nContent B";
        let edit = SkillEdit {
            skill_name: "test".into(),
            file_path: "SKILL.md".into(),
            edit_type: "body_insert".into(),
            field: None,
            new_value: None,
            old_text: None,
            new_text: Some("- New item".into()),
            section: Some("## Section A".into()),
            reason: "test".into(),
        };
        let result = apply_single_edit(content, &edit).unwrap();
        assert!(result.contains("## Section A\n- New item"));
    }
}
```

- [ ] **Step 3: Update mod.rs to export service**

In `crates/cognitive/src/services/reforge/mod.rs`:

```rust
pub mod collector;
pub mod service;
pub mod skill_files;
pub mod types;

use async_trait::async_trait;

#[async_trait]
pub trait ReforgeHandler: Send + Sync {
    async fn synthesize(
        &self,
        input: &types::SynthesizeInput,
    ) -> common::Result<types::SynthesizeOutput>;

    async fn review(&self, input: &types::ReviewInput) -> common::Result<types::ReviewOutput>;

    async fn narrate(&self, input: &types::NarrateInput) -> common::Result<String>;
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p cognitive`

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(apply_single_edit) | test(apply_frontmatter) | test(apply_body)'`
Expected: 4 tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add ReforgeService orchestrator with 7-phase cycle"
```

---

### Task 8: Cron wiring — register Reforge, remove old jobs

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`
- Modify: `crates/app-core/src/handlers/cognitive/mod.rs`

- [ ] **Step 1: Add Reforge job constant**

In `crates/app-core/src/init/cron.rs`, add to the constants section:

```rust
const JOB_REFORGE_NIGHTLY: &str = "__klyntbot_reforge_nightly";
```

- [ ] **Step 2: Remove old job constants**

Delete these constants (or comment out — implementer's choice since they may be referenced elsewhere):
- `JOB_WEEKLY_REFLECTION`
- `JOB_MIRROR_WEEKLY_NARRATIVE`
- `JOB_COGNITIVE_COMPACTION`
- `JOB_MIRROR_CLEANUP`

Note: `JOB_AUTOTUNER_NIGHTLY` stays for now — Phase 6 integration can reuse its evaluation logic but the cron registration moves into Reforge.

- [ ] **Step 3: Register Reforge handler**

Add handler registration in `register_cron_callbacks()`. Follow the exact pattern from the reflection handler but call `ReforgeService::run()`:

```rust
cron_service.register_handler(
    JOB_REFORGE_NIGHTLY,
    Arc::new(move |_job: &scheduling::CronJob| {
        let pool = pool.clone();
        let repos = repos.clone();
        let cog_provider = cog_provider.clone();
        let cog_config = cog_config.clone();
        let data_dir = data_dir.clone();

        tokio::task::block_in_place(|| {
            rt.block_on(async move {
                let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
                let episodic_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
                let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());

                let skill_mgr = cognitive::services::reforge::skill_files::SkillFileManager::new(
                    data_dir.join("skills"),
                );

                // Build handlers
                let (reforge_handler, consolidation_handler) =
                    crate::handlers::cognitive::build_reforge_handlers(
                        &cog_provider,
                        &cog_config,
                    );

                let service = cognitive::services::reforge::service::ReforgeService::new(
                    &repos,
                    &fact_repo,
                    &episodic_repo,
                    &rule_repo,
                    consolidation_handler.as_ref(),
                    reforge_handler.as_ref(),
                    &skill_mgr,
                );

                match service.run().await {
                    Some(result) => Ok(Some(format!(
                        "Reforge #{}: +{}f +{}r {}s edited | {}",
                        repos.reforge_state.get().await.map(|s| s.run_count).unwrap_or(0),
                        result.facts_added + result.facts_updated,
                        result.rules_added + result.rules_reinforced,
                        result.skills_edited,
                        if result.phase_errors.is_empty() {
                            "OK".to_string()
                        } else {
                            format!("{} phase errors", result.phase_errors.len())
                        },
                    ))),
                    None => Ok(Some("Reforge skipped: no new data".to_string())),
                }
            })
        })
    }),
);
```

- [ ] **Step 4: Add `build_reforge_handlers` to cognitive handlers**

In `crates/app-core/src/handlers/cognitive/mod.rs`, add:

```rust
pub(crate) fn build_reforge_handlers(
    cognitive_provider: &Option<providers::DynProvider>,
    config: &config::Config,
) -> (
    Box<dyn cognitive::services::reforge::ReforgeHandler>,
    Box<dyn cognitive::ConsolidationHandler>,
) {
    let reforge: Box<dyn cognitive::services::reforge::ReforgeHandler> =
        if let Some(ref cp) = cognitive_provider {
            let params = providers::cognitive_chat_params(config, 4096);
            Box::new(agent::adapters::reforge_handlers::LlmReforgeHandler::new(
                cp.clone(),
                params,
            ))
        } else {
            // Heuristic fallback — return empty results
            Box::new(agent::adapters::reforge_handlers::NoopReforgeHandler)
        };

    let consolidation: Box<dyn cognitive::ConsolidationHandler> =
        if let Some(ref cp) = cognitive_provider {
            let params = providers::cognitive_chat_params(config, 1024);
            Box::new(agent::cognitive_handlers::LlmConsolidationHandler::new(
                cp.clone(),
                params,
            ))
        } else {
            Box::new(agent::cognitive_handlers::HeuristicConsolidationHandler)
        };

    (reforge, consolidation)
}
```

- [ ] **Step 5: Add NoopReforgeHandler for fallback**

In `crates/agent/src/adapters/reforge_handlers.rs`, add:

```rust
/// Fallback handler when no LLM provider is configured.
pub struct NoopReforgeHandler;

#[async_trait]
impl ReforgeHandler for NoopReforgeHandler {
    async fn synthesize(&self, _input: &SynthesizeInput) -> common::Result<SynthesizeOutput> {
        Ok(SynthesizeOutput {
            fact_updates: vec![],
            rule_updates: vec![],
            stale_facts: vec![],
            cross_session_patterns: vec![],
            extraction_quality_flag: None,
        })
    }

    async fn review(&self, _input: &ReviewInput) -> common::Result<ReviewOutput> {
        Ok(ReviewOutput {
            skill_edits: vec![],
            routing_insights: vec![],
            context_priority_suggestions: vec![],
        })
    }

    async fn narrate(&self, _input: &NarrateInput) -> common::Result<String> {
        Ok("Reforge completed (no LLM provider configured).".to_string())
    }
}
```

- [ ] **Step 6: Ensure Reforge job in `ensure_cron_jobs()`**

Replace the 5 old job registrations with:

```rust
ensure_job!(
    JOB_REFORGE_NIGHTLY,
    scheduling::CronSchedule::Cron {
        expr: "0 3 * * *".to_string(),
        tz: Some(config.timezone.clone()),
    },
    "Nightly Reforge cycle: knowledge synthesis, skill improvement, compaction",
    system.clone()
);
```

- [ ] **Step 7: Set intent window**

In `set_default_intent_windows()`:

```rust
(
    JOB_REFORGE_NIGHTLY,
    IntentWindow {
        trigger: IntentTrigger::UserIdle { min_idle_secs: 300 },
        tolerance: Duration::from_secs(14400),
        catch_up: CatchUpPriority::WhenIdle,
    },
),
```

- [ ] **Step 8: Remove old job registrations**

Delete or comment out the handler registrations and `ensure_job!` calls for:
- `JOB_WEEKLY_REFLECTION`
- `JOB_MIRROR_WEEKLY_NARRATIVE`
- `JOB_COGNITIVE_COMPACTION`
- `JOB_MIRROR_CLEANUP`
- `JOB_AUTOTUNER_NIGHTLY` (Phase 6 evaluation will be called from within Reforge)

- [ ] **Step 9: Verify full workspace builds**

Run: `cargo build --workspace`
Expected: Compiles with no errors (warnings OK)

- [ ] **Step 10: Commit**

```bash
git add crates/app-core/ crates/agent/
git commit -m "feat(app-core): wire Reforge cron job, remove 5 old consolidation jobs"
```

---

### Task 9: Delete reflection.rs and wire compaction into Reforge Phase 7

**Files:**
- Delete: `crates/cognitive/src/services/reflection.rs`
- Modify: `crates/cognitive/src/services/mod.rs`
- Modify: `crates/cognitive/src/services/reforge/service.rs`

- [ ] **Step 1: Delete reflection.rs**

```bash
rm crates/cognitive/src/services/reflection.rs
```

Remove from `crates/cognitive/src/services/mod.rs`:
```rust
// Delete this line:
pub mod reflection;
```

- [ ] **Step 2: Fix any compilation errors from removed reflection module**

Search for all references to `cognitive::reflection` or `reflection::run_weekly_reflection` across the workspace and remove/update them. Key locations:
- `crates/app-core/src/init/cron.rs` (handler registration — already removed in Task 8)
- `crates/app-core/src/handlers/cognitive/memory.rs` (build_reflection_handlers — keep for consolidation handler reuse, or remove if unused)
- Any re-exports in `crates/cognitive/src/lib.rs`

- [ ] **Step 3: Add Phase 7 (Compact) call to ReforgeService**

In `service.rs`, add to the `run()` method after Phase 5:

```rust
// Phase 7: Compact
info!("Reforge Phase 7: Compaction");
match crate::services::compaction::run_compaction(
    self.fact_repo,
    self.episodic_repo,
    self.rule_repo,
    // Pass other repos as needed
).await {
    Ok(compaction) => {
        debug!(
            "Compaction: {} facts archived, {} episodic deleted, {} rules deactivated",
            compaction.facts_archived,
            compaction.episodic_deleted,
            compaction.rules_deactivated,
        );
    }
    Err(e) => {
        warn!("Reforge Phase 7 (Compact) failed: {e}");
        result.phase_errors.push(format!("Compact: {e}"));
    }
}
```

- [ ] **Step 4: Verify full workspace builds**

Run: `cargo build --workspace`
Expected: Compiles

- [ ] **Step 5: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All tests pass (some reflection-specific tests may need removal)

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(cognitive): delete reflection.rs, wire compaction into Reforge Phase 7"
```

---

### Task 10: Skill seeding on first run

**Files:**
- Modify: `crates/app-core/src/init/` (or wherever app initialization happens)
- Modify: `crates/cognitive/src/services/reforge/skill_files.rs`

- [ ] **Step 1: Add seed_and_version method to SkillFileManager**

In `skill_files.rs`, add:

```rust
/// Seed skills from compiled defaults and record initial versions in DB.
pub async fn seed_and_record_versions(
    &self,
    defaults: &HashMap<String, Vec<(&str, &str)>>,
    version_repo: &storage::SkillVersionRepo,
) -> std::io::Result<usize> {
    let count = self.seed_if_empty(defaults)?;
    if count == 0 {
        return Ok(0);
    }

    // Record v1 for all seeded files
    let all_files = self.read_all();
    for (skill_name, files) in &all_files {
        for file in files {
            let row = storage::rows::SkillVersionRow {
                id: uuid::Uuid::new_v4().to_string(),
                skill_name: skill_name.clone(),
                version: 1,
                file_path: file.file_path.clone(),
                content: file.content.clone(),
                diff: None,
                source: "Seed".to_string(),
                reason: Some("Initial skill from compiled defaults".to_string()),
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            if let Err(e) = version_repo.insert(&row).await {
                tracing::warn!("Failed to record seed version for {}/{}: {e}", skill_name, file.file_path);
            }
        }
    }

    Ok(count)
}
```

- [ ] **Step 2: Wire seeding into app initialization**

In the AppCore initialization (wherever skills are currently loaded from `include_str!`), add:

```rust
// Seed skills on first run
let skill_mgr = SkillFileManager::new(data_dir.join("skills"));
let defaults = compile_skill_defaults(); // Function that returns HashMap from include_str! data
let seeded = skill_mgr
    .seed_and_record_versions(&defaults, &repos.skill_version)
    .await
    .unwrap_or(0);
if seeded > 0 {
    info!("Seeded {seeded} skills to {}", data_dir.join("skills").display());
}
```

The `compile_skill_defaults()` function should return the compiled skill content from `include_str!` in the same format as `seed_if_empty` expects. The implementer should locate where skills are currently loaded and extract the defaults.

- [ ] **Step 3: Add user edit detection to Reforge collector**

In Phase 1 (collector.rs), after reading skill files, check for user edits:

```rust
// Detect user edits since last known version
for (skill_name, files) in &skill_files {
    for file in files {
        let latest = version_repo
            .latest_version(skill_name, &file.file_path)
            .await
            .ok()
            .flatten();
        if let Some(latest) = latest {
            let known_hash = content_hash(&latest.content);
            if known_hash != file.content_hash {
                // User edited this file — record new version
                let diff = compute_diff(&latest.content, &file.content);
                let row = SkillVersionRow {
                    id: uuid::Uuid::new_v4().to_string(),
                    skill_name: skill_name.clone(),
                    version: latest.version + 1,
                    file_path: file.file_path.clone(),
                    content: file.content.clone(),
                    diff: Some(diff),
                    source: "User".to_string(),
                    reason: Some("Detected manual file edit".to_string()),
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                let _ = version_repo.insert(&row).await;
            }
        }
    }
}
```

- [ ] **Step 4: Verify build**

Run: `cargo build --workspace`

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(app-core): seed skills on first run with version tracking"
```

---

### Task 11: Integration test — full Reforge cycle

**Files:**
- Create: `tests/integration/reforge.rs` (or add to existing cognitive test file)

- [ ] **Step 1: Write integration test**

```rust
#[tokio::test]
async fn test_reforge_cycle_end_to_end() {
    // 1. Setup: in-memory pool, seed repos, create test data
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    // Run cognitive migrations
    // ...

    let fact_repo = SemanticFactRepo::new(pool.inner().clone());
    let episodic_repo = EpisodicMemoryRepo::new(pool.inner().clone());
    let rule_repo = ProceduralRuleRepo::new(pool.inner().clone());

    // 2. Insert test data: session scratchpad + episodic memory
    repos.session_memory.upsert("test-session", "## Current Task\nImplementing Reforge\n## Key Decisions\nUse nightly cycle", 3).await.unwrap();
    episodic_repo.insert(&test_episodic("User prefers morning work sessions")).await.unwrap();

    // 3. Create mock ReforgeHandler that returns known outputs
    let handler = MockReforgeHandler::new();

    // 4. Run Reforge
    let tmp = tempfile::TempDir::new().unwrap();
    let skill_mgr = SkillFileManager::new(tmp.path().to_path_buf());
    // Seed a test skill
    skill_mgr.write_file("test-skill", "SKILL.md", "---\nname: test\n---\nBody").unwrap();

    let consolidation = HeuristicConsolidationHandler;
    let service = ReforgeService::new(
        &repos, &fact_repo, &episodic_repo, &rule_repo,
        &consolidation, &handler, &skill_mgr,
    );

    let result = service.run().await;

    // 5. Verify
    assert!(result.is_some());
    let result = result.unwrap();
    assert!(result.phase_errors.is_empty());

    // Verify state was updated
    let state = repos.reforge_state.get().await.unwrap();
    assert_eq!(state.run_count, 1);
    assert!(state.last_run_at.is_some());
}
```

- [ ] **Step 2: Implement MockReforgeHandler**

```rust
struct MockReforgeHandler;

#[async_trait]
impl ReforgeHandler for MockReforgeHandler {
    async fn synthesize(&self, _: &SynthesizeInput) -> common::Result<SynthesizeOutput> {
        Ok(SynthesizeOutput {
            fact_updates: vec![FactUpdate {
                action: "add".into(),
                subject: "user".into(),
                predicate: "prefers".into(),
                object: "morning work".into(),
                domain: "productivity".into(),
                confidence: 0.8,
                reason: "Consistent across sessions".into(),
            }],
            rule_updates: vec![],
            stale_facts: vec![],
            cross_session_patterns: vec![],
            extraction_quality_flag: None,
        })
    }

    async fn review(&self, _: &ReviewInput) -> common::Result<ReviewOutput> {
        Ok(ReviewOutput {
            skill_edits: vec![],
            routing_insights: vec![],
            context_priority_suggestions: vec![],
        })
    }

    async fn narrate(&self, _: &NarrateInput) -> common::Result<String> {
        Ok("Test narrative: Reforge completed successfully.".into())
    }
}
```

- [ ] **Step 3: Run the integration test**

Run: `cargo nextest run -E 'test(reforge_cycle)'`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add tests/
git commit -m "test: add Reforge cycle integration test"
```

---

---

### Task 12: Wire Phase 6 (Autotuner) into Reforge

**Deferred task** — wire after core Reforge is working. The autotuner's `run_evaluation_and_promotion()` method in `crates/autotuner/src/cycle.rs` needs to be called from `ReforgeService::run()` between Phase 5 and Phase 7. The current `AutoTunerOrchestrator::register_nightly_cycle()` contains the evaluation + generation logic. Extract the evaluation call and invoke it from the Reforge service, passing the current champion from the orchestrator.

This is intentionally deferred because:
1. The autotuner has its own complex state (orchestrator, active trials, champion)
2. Its cron handler accesses `Arc<AutoTunerOrchestrator>` which is wired differently from cognitive repos
3. The core Reforge (Phases 1-5, 7) can ship and run without Phase 6
4. Phase 6 integration is a follow-up once the orchestrator wiring is understood

---

### Known Gaps for Implementer

1. **`load_routing_summaries`** in collector.rs returns empty vec — needs MirrorRepo wiring to aggregate routing snapshots. Implement when mirror repo access is available.
2. **`list_pending_meta_rules`** on MirrorRepo may not exist — add a simple `WHERE status = 'pending'` query.
3. **`UserModel::all_domains()`** method may not exist — implement as an iterator over the struct fields returning `(&str, &[SemanticFact])` pairs.
4. **`UserModel::add_domain()`** method may not exist — implement to push facts into the correct field.
5. **Skill defaults compilation** — Task 10 references `compile_skill_defaults()`. The implementer needs to create this function using `include_str!` to embed the built-in skill files, matching the format expected by `seed_if_empty`.

---

## Summary of Tasks

| Task | Component | Files | Tests |
|------|-----------|-------|-------|
| 1 | reforge_state repo | 5 | 3 unit |
| 2 | skill_versions repo | 5 | 4 unit |
| 3 | SkillFileManager | 3 | 6 unit |
| 4 | Reforge types + trait | 2 | compile check |
| 5 | Collector (Phase 1) | 2 | compile check |
| 6 | LLM handlers (Phases 2-4) | 2 | compile check |
| 7 | ReforgeService orchestrator | 2 | 4 unit |
| 8 | Cron wiring | 3 | build check |
| 9 | Delete reflection + wire compaction | 3 | workspace tests |
| 10 | Skill seeding + user edit detection | 2 | build check |
| 11 | Integration test | 1 | 1 integration |

**Total: ~30 files touched, ~17 unit tests, 1 integration test, 11 commits**
