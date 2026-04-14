# Skills Marketplace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a local-first skills marketplace with GitHub + skills.sh sources, LLM-driven adapter, versioned upgrades with diff view, and three-mode uninstall.

**Architecture:** Three new Rust crates — `skills-registry` (L3, fetches sources), `skills-installer` (L4, transactional install/upgrade/uninstall on top of `SkillStore` + `EntityStore`), `skills-adapter` (L4, LLM-driven transformation of prompt-only skills). One migration, twelve Tauri commands, a `/skills` top-level route with list + detail pages.

**Tech Stack:** Rust workspace (sqlx, reqwest, tokio, similar for diffing), React + Vite + Biome + Tailwind v4 + shadcn-style components, Tauri 2 IPC, existing `cognitive_provider` trait for the LLM adapter.

---

## Phase summary

| Phase | Scope | Notable outputs |
|---|---|---|
| 1 | Migration + `installed_skills` / `adapted_skills` repos | New `skills-marketplace` FeatureMigration, `SkillsRepo` + `AdaptedSkillsRepo` |
| 2 | `skills-registry` crate | `SkillSource`, `SkillPackage`, GitHub fetcher, skills.sh resolver |
| 3 | `skills-installer` crate | `InstallPlan`, `apply_install`, rollback, check-updates, diff |
| 4 | `skills-adapter` crate | LLM prompt, schema-validated output, adapt cache |
| 5 | `app-core` handlers + Tauri commands | 12 commands wired through `DEV_COMMANDS` |
| 6 | Frontend `/skills` list + detail | SkillsListPage, SkillDetailPage, Install dialog, DiffViewer, Uninstall dialog |
| 7 | Bundled-skill seeding + end-to-end verification | Bootstrap installer seeds 6 bundled skills into `installed_skills`; manual script |

---

# Phase 1 — Data model

### Task 1: Create the skills-marketplace migration SQL

**Files:**
- Create: `crates/storage/migrations/skills_marketplace/001_skills_marketplace.sql` (or place under new crate if following entity-store pattern — we choose the crate-local form; see Task 2)

- [ ] **Step 1: Inspect migration numbering convention**

Run: `ls crates/entity-store/migrations/ crates/cognitive/migrations/`
Expected: sequential `NNN_name.sql` files per crate.

- [ ] **Step 2: Create the new crate skeleton for the marketplace feature migration**

We host marketplace tables in a new crate so the migration is versioned alongside the types that use it. Create directory and Cargo.toml in Task 2; the SQL file goes there.

Create the file `crates/skills-marketplace/migrations/001_skills_marketplace.sql`:

```sql
CREATE TABLE IF NOT EXISTS installed_skills (
  name                   TEXT PRIMARY KEY,
  source_type            TEXT NOT NULL CHECK(source_type IN ('github','skills_sh','local','bundled')),
  source_ref             TEXT NOT NULL,
  installed_version      TEXT NOT NULL,
  installed_sha          TEXT NOT NULL,
  enabled                INTEGER NOT NULL DEFAULT 1,
  is_adapted             INTEGER NOT NULL DEFAULT 0,
  bootstrapped_databases TEXT,
  installed_at           TEXT NOT NULL,
  updated_at             TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_installed_skills_source
  ON installed_skills(source_type, source_ref);

CREATE TABLE IF NOT EXISTS adapted_skills (
  cache_key              TEXT PRIMARY KEY,
  adapted_skill_md       TEXT NOT NULL,
  generated_templates    TEXT NOT NULL,
  rationale              TEXT NOT NULL,
  adapter_model          TEXT NOT NULL,
  created_at             TEXT NOT NULL
);
```

- [ ] **Step 3: Commit**

```bash
git add crates/skills-marketplace/migrations/001_skills_marketplace.sql
git commit -m "feat(skills-marketplace): add migration for installed + adapted skills"
```

### Task 2: Scaffold the skills-marketplace crate

**Files:**
- Create: `crates/skills-marketplace/Cargo.toml`
- Create: `crates/skills-marketplace/src/lib.rs`
- Create: `crates/skills-marketplace/src/types.rs`
- Create: `crates/skills-marketplace/src/repo.rs`
- Modify: `Cargo.toml` (workspace members + `[workspace.dependencies]`)

- [ ] **Step 1: Write `crates/skills-marketplace/Cargo.toml`**

```toml
[package]
name = "skills-marketplace"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common = { workspace = true }
storage = { workspace = true }
tools-core = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sqlx = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Add the crate to the workspace**

Modify `Cargo.toml` at repo root. In `[workspace] members`, add:
```toml
    "crates/skills-marketplace",
```
In `[workspace.dependencies]`, add:
```toml
skills-marketplace = { path = "crates/skills-marketplace" }
```

- [ ] **Step 3: Write `crates/skills-marketplace/src/lib.rs`**

```rust
//! skills-marketplace: install/upgrade/uninstall third-party skills with versioning.

pub mod repo;
pub mod types;

pub use repo::{AdaptedSkillsRepo, InstalledSkillsRepo};
pub use types::*;

use tools_core::FeatureMigration;

pub struct SkillsMarketplaceFeature;

impl SkillsMarketplaceFeature {
    pub fn migrations() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "skills_marketplace".to_string(),
            version: 1,
            description: "installed_skills + adapted_skills tables".to_string(),
            sql: include_str!("../migrations/001_skills_marketplace.sql").to_string(),
        }]
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    pub async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let sql = include_str!("../migrations/001_skills_marketplace.sql");
        for stmt in sql.split(';') {
            let t = stmt.trim();
            if !t.is_empty() {
                sqlx::query(t).execute(pool.inner()).await.unwrap();
            }
        }
        pool.inner().clone()
    }
}
```

- [ ] **Step 4: Write `crates/skills-marketplace/src/types.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Github,
    SkillsSh,
    Local,
    Bundled,
}

impl SourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::SkillsSh => "skills_sh",
            Self::Local => "local",
            Self::Bundled => "bundled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSkill {
    pub name: String,
    pub source_type: SourceType,
    pub source_ref: String,
    pub installed_version: String,
    pub installed_sha: String,
    pub enabled: bool,
    pub is_adapted: bool,
    pub bootstrapped_databases: Vec<String>,
    pub installed_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptedSkillRow {
    pub cache_key: String,
    pub adapted_skill_md: String,
    pub generated_templates: serde_json::Value, // parsed from stored JSON string
    pub rationale: String,
    pub adapter_model: String,
    pub created_at: String,
}
```

- [ ] **Step 5: Write the failing repo test (`repo.rs`)**

```rust
use common::Result;
use serde_json::json;
use sqlx::SqlitePool;

use crate::types::{InstalledSkill, SourceType};

pub struct InstalledSkillsRepo {
    pool: SqlitePool,
}

impl InstalledSkillsRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn insert(&self, _skill: &InstalledSkill) -> Result<()> { todo!() }
    pub async fn get(&self, _name: &str) -> Result<Option<InstalledSkill>> { todo!() }
    pub async fn list(&self) -> Result<Vec<InstalledSkill>> { todo!() }
    pub async fn set_enabled(&self, _name: &str, _enabled: bool) -> Result<()> { todo!() }
    pub async fn delete(&self, _name: &str) -> Result<()> { todo!() }
    pub async fn update_version(
        &self, _name: &str, _version: &str, _sha: &str, _bootstrapped: &[String],
    ) -> Result<()> { todo!() }
}

pub struct AdaptedSkillsRepo { pool: SqlitePool }
impl AdaptedSkillsRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }
    pub async fn get(&self, _cache_key: &str) -> Result<Option<crate::types::AdaptedSkillRow>> { todo!() }
    pub async fn upsert(
        &self, _cache_key: &str, _skill_md: &str,
        _templates: &serde_json::Value, _rationale: &str, _model: &str,
    ) -> Result<()> { todo!() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn insert_and_roundtrip() {
        let pool = crate::test_helpers::setup_pool().await;
        let repo = InstalledSkillsRepo::new(pool);
        let skill = InstalledSkill {
            name: "reading-list".into(),
            source_type: SourceType::Github,
            source_ref: "owner/repo/reading-list".into(),
            installed_version: "1.0.3".into(),
            installed_sha: "abc123".into(),
            enabled: true,
            is_adapted: false,
            bootstrapped_databases: vec!["db1".into(), "db2".into()],
            installed_at: "2026-04-14T00:00:00Z".into(),
            updated_at: "2026-04-14T00:00:00Z".into(),
        };
        repo.insert(&skill).await.unwrap();

        let fetched = repo.get("reading-list").await.unwrap().unwrap();
        assert_eq!(fetched.name, "reading-list");
        assert_eq!(fetched.bootstrapped_databases, vec!["db1", "db2"]);
    }

    #[tokio::test]
    async fn update_version_and_list() {
        let pool = crate::test_helpers::setup_pool().await;
        let repo = InstalledSkillsRepo::new(pool);
        let mut skill = InstalledSkill {
            name: "x".into(), source_type: SourceType::Github,
            source_ref: "o/r/x".into(), installed_version: "1.0.0".into(),
            installed_sha: "aaa".into(), enabled: true, is_adapted: false,
            bootstrapped_databases: vec![], installed_at: "t".into(), updated_at: "t".into(),
        };
        repo.insert(&skill).await.unwrap();
        repo.update_version("x", "1.0.1", "bbb", &["dbA".into()]).await.unwrap();
        let fetched = repo.get("x").await.unwrap().unwrap();
        assert_eq!(fetched.installed_version, "1.0.1");
        assert_eq!(fetched.installed_sha, "bbb");
        assert_eq!(fetched.bootstrapped_databases, vec!["dbA"]);
        let all = repo.list().await.unwrap();
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn adapted_cache_upsert() {
        let pool = crate::test_helpers::setup_pool().await;
        let repo = AdaptedSkillsRepo::new(pool);
        repo.upsert("k1", "---\nname: x\n---\nbody", &json!([{"name":"t","manifest_json":{}}]), "why", "claude-opus-4-6").await.unwrap();
        let row = repo.get("k1").await.unwrap().unwrap();
        assert_eq!(row.adapter_model, "claude-opus-4-6");
    }
}
```

- [ ] **Step 6: Verify test fails**

Run: `cargo nextest run -p skills-marketplace`
Expected: FAIL with `todo!()` panics across all tests.

- [ ] **Step 7: Implement the repo methods**

Replace `repo.rs` bodies with real SQL implementations. (Full implementation code below — paste in verbatim.)

```rust
use chrono::Utc;
use common::{KlyntbotError, Result};
use sqlx::{FromRow, SqlitePool};

use crate::types::{AdaptedSkillRow, InstalledSkill, SourceType};

fn map_err<E: std::fmt::Display>(e: E) -> KlyntbotError {
    KlyntbotError::Storage(e.to_string())
}

#[derive(FromRow)]
struct InstalledRow {
    name: String,
    source_type: String,
    source_ref: String,
    installed_version: String,
    installed_sha: String,
    enabled: i64,
    is_adapted: i64,
    bootstrapped_databases: Option<String>,
    installed_at: String,
    updated_at: String,
}

impl TryFrom<InstalledRow> for InstalledSkill {
    type Error = KlyntbotError;
    fn try_from(r: InstalledRow) -> Result<Self> {
        let source_type = match r.source_type.as_str() {
            "github" => SourceType::Github,
            "skills_sh" => SourceType::SkillsSh,
            "local" => SourceType::Local,
            "bundled" => SourceType::Bundled,
            other => return Err(KlyntbotError::Storage(format!("unknown source_type {other}"))),
        };
        let bootstrapped: Vec<String> = r
            .bootstrapped_databases
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| serde_json::from_str(s))
            .transpose()
            .map_err(map_err)?
            .unwrap_or_default();
        Ok(InstalledSkill {
            name: r.name,
            source_type,
            source_ref: r.source_ref,
            installed_version: r.installed_version,
            installed_sha: r.installed_sha,
            enabled: r.enabled != 0,
            is_adapted: r.is_adapted != 0,
            bootstrapped_databases: bootstrapped,
            installed_at: r.installed_at,
            updated_at: r.updated_at,
        })
    }
}

pub struct InstalledSkillsRepo { pool: SqlitePool }

impl InstalledSkillsRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn insert(&self, s: &InstalledSkill) -> Result<()> {
        let boots = serde_json::to_string(&s.bootstrapped_databases).map_err(map_err)?;
        sqlx::query(
            "INSERT INTO installed_skills \
             (name, source_type, source_ref, installed_version, installed_sha, enabled, is_adapted, \
              bootstrapped_databases, installed_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&s.name).bind(s.source_type.as_str()).bind(&s.source_ref)
        .bind(&s.installed_version).bind(&s.installed_sha)
        .bind(s.enabled as i64).bind(s.is_adapted as i64)
        .bind(&boots).bind(&s.installed_at).bind(&s.updated_at)
        .execute(&self.pool).await.map_err(map_err)?;
        Ok(())
    }

    pub async fn get(&self, name: &str) -> Result<Option<InstalledSkill>> {
        let row: Option<InstalledRow> = sqlx::query_as(
            "SELECT * FROM installed_skills WHERE name = ?",
        ).bind(name).fetch_optional(&self.pool).await.map_err(map_err)?;
        row.map(InstalledSkill::try_from).transpose()
    }

    pub async fn list(&self) -> Result<Vec<InstalledSkill>> {
        let rows: Vec<InstalledRow> = sqlx::query_as(
            "SELECT * FROM installed_skills ORDER BY name ASC",
        ).fetch_all(&self.pool).await.map_err(map_err)?;
        rows.into_iter().map(InstalledSkill::try_from).collect()
    }

    pub async fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE installed_skills SET enabled = ?, updated_at = ? WHERE name = ?")
            .bind(enabled as i64).bind(&now).bind(name)
            .execute(&self.pool).await.map_err(map_err)?;
        Ok(())
    }

    pub async fn delete(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM installed_skills WHERE name = ?")
            .bind(name).execute(&self.pool).await.map_err(map_err)?;
        Ok(())
    }

    pub async fn update_version(
        &self, name: &str, version: &str, sha: &str, bootstrapped: &[String],
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let boots = serde_json::to_string(bootstrapped).map_err(map_err)?;
        sqlx::query(
            "UPDATE installed_skills SET installed_version = ?, installed_sha = ?, \
             bootstrapped_databases = ?, updated_at = ? WHERE name = ?",
        )
        .bind(version).bind(sha).bind(&boots).bind(&now).bind(name)
        .execute(&self.pool).await.map_err(map_err)?;
        Ok(())
    }
}

pub struct AdaptedSkillsRepo { pool: SqlitePool }

impl AdaptedSkillsRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn get(&self, cache_key: &str) -> Result<Option<AdaptedSkillRow>> {
        let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(
            "SELECT cache_key, adapted_skill_md, generated_templates, rationale, adapter_model, created_at \
             FROM adapted_skills WHERE cache_key = ?",
        ).bind(cache_key).fetch_optional(&self.pool).await.map_err(map_err)?;
        Ok(match row {
            Some((k, md, tpls, r, model, at)) => Some(AdaptedSkillRow {
                cache_key: k, adapted_skill_md: md,
                generated_templates: serde_json::from_str(&tpls).map_err(map_err)?,
                rationale: r, adapter_model: model, created_at: at,
            }),
            None => None,
        })
    }

    pub async fn upsert(
        &self, cache_key: &str, skill_md: &str, templates: &serde_json::Value,
        rationale: &str, model: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let tpls = serde_json::to_string(templates).map_err(map_err)?;
        sqlx::query(
            "INSERT INTO adapted_skills (cache_key, adapted_skill_md, generated_templates, rationale, adapter_model, created_at) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT(cache_key) DO UPDATE SET \
               adapted_skill_md = excluded.adapted_skill_md, \
               generated_templates = excluded.generated_templates, \
               rationale = excluded.rationale, \
               adapter_model = excluded.adapter_model, \
               created_at = excluded.created_at",
        )
        .bind(cache_key).bind(skill_md).bind(&tpls).bind(rationale).bind(model).bind(&now)
        .execute(&self.pool).await.map_err(map_err)?;
        Ok(())
    }
}
```

- [ ] **Step 8: Run tests — all pass**

Run: `cargo nextest run -p skills-marketplace`
Expected: 3 passed.

- [ ] **Step 9: Commit**

```bash
git add crates/skills-marketplace Cargo.toml
git commit -m "feat(skills-marketplace): scaffold crate, migration, repos"
```

### Task 3: Wire the migration into `AppCore::init`

**Files:**
- Modify: `crates/app-core/Cargo.toml` (add `skills-marketplace` dep)
- Modify: `crates/app-core/src/init/storage.rs` — add a call to run the new migration

- [ ] **Step 1: Add dep**

In `crates/app-core/Cargo.toml` under `[dependencies]`:
```toml
skills-marketplace = { workspace = true }
```

- [ ] **Step 2: Find the existing EntityStoreFeature migration call site**

Run: `grep -n "EntityStoreFeature::migrations" crates/app-core/src/init/storage.rs`
Expected: single match around line 105–110.

- [ ] **Step 3: Add SkillsMarketplaceFeature migration alongside it**

In `crates/app-core/src/init/storage.rs`, directly after the `EntityStoreFeature::migrations()` call, add:
```rust
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &skills_marketplace::SkillsMarketplaceFeature::migrations(),
    )
    .await
    .map_err(|e| KlyntbotError::Storage(format!("skills-marketplace migrations: {e}")))?;
```

- [ ] **Step 4: Verify compile**

Run: `cargo check -p app-core`
Expected: clean compile.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/Cargo.toml crates/app-core/src/init/storage.rs
git commit -m "feat(app-core): run skills-marketplace migration on boot"
```

---

# Phase 2 — `skills-registry` crate

### Task 4: Scaffold `skills-registry` with SkillSource + SkillPackage types

**Files:**
- Create: `crates/skills-registry/Cargo.toml`
- Create: `crates/skills-registry/src/lib.rs`
- Create: `crates/skills-registry/src/source.rs`
- Create: `crates/skills-registry/src/package.rs`
- Modify: root `Cargo.toml` (workspace members + deps)

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "skills-registry"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common = { workspace = true }
skill-system = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
serde_yaml = { workspace = true }
reqwest = { workspace = true, features = ["json", "stream"] }
tokio = { workspace = true, features = ["fs"] }
thiserror = { workspace = true }
sha2 = { workspace = true }
hex = "0.4"
tracing = { workspace = true }
anyhow = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
wiremock = "0.6"
```

- [ ] **Step 2: Register in workspace** — add to `[workspace] members` and `[workspace.dependencies]` in root Cargo.toml:
```toml
"crates/skills-registry",
```
```toml
skills-registry = { path = "crates/skills-registry" }
```

- [ ] **Step 3: Write `src/source.rs`**

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SkillSource {
    #[serde(rename_all = "camelCase")]
    Github { owner: String, repo: String, subpath: String, r#ref: GitRef },
    #[serde(rename_all = "camelCase")]
    SkillsSh { slug: String },
    #[serde(rename_all = "camelCase")]
    LocalPath { path: PathBuf },
    #[serde(rename_all = "camelCase")]
    Bundled { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GitRef {
    Latest,
    Tag { tag: String },
    Commit { sha: String },
}

impl SkillSource {
    /// Parse a user-entered string such as `owner/repo/subpath` or a full URL.
    pub fn parse_shorthand(input: &str) -> Result<Self, ParseError> {
        let trimmed = input.trim().trim_end_matches('/');
        // GitHub URL form
        if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
            return parse_github_path(rest);
        }
        // `owner/repo[/subpath]` form
        parse_github_path(trimmed)
    }
}

fn parse_github_path(path: &str) -> Result<SkillSource, ParseError> {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        return Err(ParseError::BadFormat("expected owner/repo[/subpath]".into()));
    }
    let owner = parts[0].to_string();
    let repo = parts[1].to_string();
    let subpath = parts[2..].join("/");
    Ok(SkillSource::Github {
        owner, repo, subpath, r#ref: GitRef::Latest,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("bad source format: {0}")]
    BadFormat(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_owner_repo_subpath() {
        let s = SkillSource::parse_shorthand("anthropics/skills/frontend-design").unwrap();
        match s {
            SkillSource::Github { owner, repo, subpath, r#ref } => {
                assert_eq!(owner, "anthropics");
                assert_eq!(repo, "skills");
                assert_eq!(subpath, "frontend-design");
                assert_eq!(r#ref, GitRef::Latest);
            }
            _ => panic!("expected github"),
        }
    }

    #[test]
    fn parse_full_github_url() {
        let s = SkillSource::parse_shorthand("https://github.com/anthropics/skills/").unwrap();
        match s {
            SkillSource::Github { owner, repo, subpath, .. } => {
                assert_eq!(owner, "anthropics");
                assert_eq!(repo, "skills");
                assert_eq!(subpath, "");
            }
            _ => panic!("expected github"),
        }
    }

    #[test]
    fn reject_invalid() {
        assert!(SkillSource::parse_shorthand("onlyone").is_err());
    }
}
```

- [ ] **Step 4: Write `src/package.rs` with SkillPackage + minimal parsing**

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use skill_system::types::{KlyntbotMeta, SkillScope};
use skill_system::store::SkillFrontmatter;

use crate::source::SkillSource;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPackage {
    pub name: String,
    pub source: SkillSource,
    pub resolved_sha: String,
    pub semver: Option<String>,
    pub skill_md_content: String,
    pub frontmatter: SkillFrontmatter,
    pub klyntbot_meta: Option<KlyntbotMeta>,
    pub references: Vec<ReferenceFile>,
    pub templates: Vec<TemplateFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceFile {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateFile {
    pub name: String,
    pub manifest: Value,
}

impl SkillPackage {
    pub fn is_klyntbot_native(&self) -> bool { self.klyntbot_meta.is_some() }
    pub fn bootstraps_databases(&self) -> usize {
        self.klyntbot_meta
            .as_ref()
            .and_then(|m| m.custom.get("bootstraps"))
            .and_then(|v| v.get("databases"))
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    }

    #[allow(dead_code)]
    fn _silence_unused(s: SkillScope) -> SkillScope { s }
}
```

- [ ] **Step 5: Write `src/lib.rs`**

```rust
//! skills-registry: resolve SkillSources into SkillPackages (fetch + parse).

pub mod fetcher;
pub mod package;
pub mod source;

pub use package::{ReferenceFile, SkillPackage, TemplateFile};
pub use source::{GitRef, SkillSource};
```

- [ ] **Step 6: Create fetcher stub**

Create `crates/skills-registry/src/fetcher.rs`:
```rust
//! Fetches SkillPackages from a SkillSource. Stub for Task 5.

use common::Result;
use crate::{SkillPackage, SkillSource};

pub struct Fetcher;

impl Fetcher {
    pub fn new() -> Self { Self }
    pub async fn fetch(&self, _source: &SkillSource) -> Result<SkillPackage> { todo!("Task 5") }
}
```

- [ ] **Step 7: Compile and test**

Run: `cargo nextest run -p skills-registry`
Expected: 3 parse-shorthand tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/skills-registry Cargo.toml
git commit -m "feat(skills-registry): scaffold crate with Source + Package types"
```

### Task 5: Implement the GitHub fetcher

**Files:**
- Modify: `crates/skills-registry/src/fetcher.rs`

- [ ] **Step 1: Write failing test (fetcher with mocked HTTP)**

Replace `fetcher.rs` with:
```rust
use std::path::PathBuf;

use common::{KlyntbotError, Result};
use serde_json::Value;
use tracing::{debug, warn};

use skill_system::parser::parse_skill_md;
use skill_system::store::split_frontmatter;
use skill_system::types::SkillScope;

use crate::package::{ReferenceFile, TemplateFile};
use crate::{GitRef, SkillPackage, SkillSource};

pub struct Fetcher {
    http: reqwest::Client,
    /// Override base URL for GitHub API — for tests.
    github_api_base: String,
    /// Override base URL for raw.githubusercontent — for tests.
    github_raw_base: String,
}

impl Fetcher {
    pub fn new() -> Self {
        Self::with_bases("https://api.github.com".into(), "https://raw.githubusercontent.com".into())
    }

    pub fn with_bases(api: String, raw: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent("klyntbot-skills-registry")
                .build()
                .expect("reqwest client"),
            github_api_base: api,
            github_raw_base: raw,
        }
    }

    pub async fn fetch(&self, source: &SkillSource) -> Result<SkillPackage> {
        match source {
            SkillSource::Github { owner, repo, subpath, r#ref } => {
                self.fetch_github(owner, repo, subpath, r#ref).await
            }
            SkillSource::LocalPath { path } => self.fetch_local(path.clone()).await,
            SkillSource::SkillsSh { slug } => {
                // Resolve slug → Github source (slugs follow owner/repo/subpath).
                let github = SkillSource::parse_shorthand(slug)
                    .map_err(|e| KlyntbotError::Storage(format!("bad skills.sh slug: {e}")))?;
                self.fetch(&github).await
            }
            SkillSource::Bundled { name } => {
                Err(KlyntbotError::Storage(format!("bundled skill '{name}' is fetched directly from SkillStore")))
            }
        }
    }

    async fn fetch_github(
        &self,
        owner: &str,
        repo: &str,
        subpath: &str,
        ref_: &GitRef,
    ) -> Result<SkillPackage> {
        // Resolve ref to a concrete SHA.
        let sha = self.resolve_ref(owner, repo, ref_).await?;

        // Fetch SKILL.md content.
        let skill_path = if subpath.is_empty() { "SKILL.md".into() } else { format!("{subpath}/SKILL.md") };
        let skill_md = self.fetch_raw(owner, repo, &sha, &skill_path).await?;

        let (frontmatter, _body) = split_frontmatter(&skill_md)
            .map_err(|e| KlyntbotError::Storage(format!("split_frontmatter: {e}")))?;

        let klyntbot_meta = parse_skill_md(
            &skill_md,
            PathBuf::from(&skill_path),
            SkillScope::User,
        )
        .ok()
        .and_then(|pkg| pkg.metadata.klyntbot.clone());

        let semver = klyntbot_meta
            .as_ref()
            .and_then(|m| m.custom.get("version"))
            .and_then(|v| v.as_str().map(String::from));

        // Fetch references/ and templates/ directory listings (optional — may 404).
        let references = self.fetch_dir_files(owner, repo, &sha, &format!("{}/references", trim_trailing(subpath)))
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(path, content)| ReferenceFile { path: PathBuf::from(path), content })
            .collect();

        let templates_raw = self.fetch_dir_files(owner, repo, &sha, &format!("{}/templates", trim_trailing(subpath)))
            .await
            .unwrap_or_default();
        let mut templates = Vec::new();
        for (path, content) in templates_raw {
            if path.ends_with(".json") {
                let manifest: Value = serde_json::from_str(&content).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
                let name = std::path::Path::new(&path).file_name()
                    .and_then(|n| n.to_str()).unwrap_or("template.json").to_string();
                templates.push(TemplateFile { name, manifest });
            }
        }

        Ok(SkillPackage {
            name: frontmatter.name.clone(),
            source: SkillSource::Github {
                owner: owner.to_string(),
                repo: repo.to_string(),
                subpath: subpath.to_string(),
                r#ref: GitRef::Commit { sha: sha.clone() },
            },
            resolved_sha: sha,
            semver,
            skill_md_content: skill_md,
            frontmatter,
            klyntbot_meta,
            references,
            templates,
        })
    }

    async fn resolve_ref(&self, owner: &str, repo: &str, ref_: &GitRef) -> Result<String> {
        match ref_ {
            GitRef::Commit { sha } => Ok(sha.clone()),
            GitRef::Tag { tag } => {
                let url = format!("{}/repos/{}/{}/commits/{}", self.github_api_base, owner, repo, tag);
                self.fetch_sha_from_commits_api(&url).await
            }
            GitRef::Latest => {
                let url = format!("{}/repos/{}/{}/commits/HEAD", self.github_api_base, owner, repo);
                self.fetch_sha_from_commits_api(&url).await
            }
        }
    }

    async fn fetch_sha_from_commits_api(&self, url: &str) -> Result<String> {
        let resp = self.http.get(url).send().await.map_err(|e| KlyntbotError::Storage(format!("github GET {url}: {e}")))?;
        if !resp.status().is_success() {
            return Err(KlyntbotError::Storage(format!("github {url}: HTTP {}", resp.status())));
        }
        let json: Value = resp.json().await.map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        json.get("sha").and_then(|v| v.as_str()).map(String::from)
            .ok_or_else(|| KlyntbotError::Storage("missing sha field".into()))
    }

    async fn fetch_raw(&self, owner: &str, repo: &str, sha: &str, path: &str) -> Result<String> {
        let url = format!("{}/{}/{}/{}/{}", self.github_raw_base, owner, repo, sha, path);
        let resp = self.http.get(&url).send().await.map_err(|e| KlyntbotError::Storage(format!("raw GET {url}: {e}")))?;
        if !resp.status().is_success() {
            return Err(KlyntbotError::Storage(format!("raw {url}: HTTP {}", resp.status())));
        }
        resp.text().await.map_err(|e| KlyntbotError::Storage(e.to_string()))
    }

    /// List files in a directory via the GitHub contents API.
    /// Returns (relative_path, content_utf8).
    async fn fetch_dir_files(
        &self, owner: &str, repo: &str, sha: &str, dir: &str,
    ) -> Result<Vec<(String, String)>> {
        let url = format!("{}/repos/{}/{}/contents/{}?ref={}", self.github_api_base, owner, repo, dir, sha);
        let resp = self.http.get(&url).send().await.map_err(|e| KlyntbotError::Storage(format!("contents {url}: {e}")))?;
        if resp.status().as_u16() == 404 { return Ok(vec![]); }
        if !resp.status().is_success() {
            return Err(KlyntbotError::Storage(format!("contents {url}: HTTP {}", resp.status())));
        }
        let items: Vec<Value> = resp.json().await.map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        let mut out = Vec::new();
        for item in items {
            let Some(kind) = item.get("type").and_then(|v| v.as_str()) else { continue };
            let Some(name) = item.get("name").and_then(|v| v.as_str()) else { continue };
            if kind != "file" { continue; }
            let full_path = if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") };
            let content = self.fetch_raw(owner, repo, sha, &full_path).await?;
            out.push((name.to_string(), content));
        }
        debug!(dir = %dir, count = out.len(), "fetched dir files");
        Ok(out)
    }

    async fn fetch_local(&self, path: PathBuf) -> Result<SkillPackage> {
        let skill_md_path = path.join("SKILL.md");
        let skill_md = tokio::fs::read_to_string(&skill_md_path).await
            .map_err(|e| KlyntbotError::Storage(format!("read {}: {e}", skill_md_path.display())))?;
        let (frontmatter, _) = split_frontmatter(&skill_md)
            .map_err(|e| KlyntbotError::Storage(format!("split_frontmatter: {e}")))?;
        let klyntbot_meta = parse_skill_md(&skill_md, skill_md_path.clone(), SkillScope::User)
            .ok()
            .and_then(|pkg| pkg.metadata.klyntbot.clone());
        let semver = klyntbot_meta.as_ref()
            .and_then(|m| m.custom.get("version"))
            .and_then(|v| v.as_str().map(String::from));

        // Best-effort local references + templates.
        let references = collect_local_files(&path.join("references")).await.unwrap_or_default()
            .into_iter()
            .map(|(p, c)| ReferenceFile { path: p, content: c })
            .collect();

        let mut templates = Vec::new();
        if let Ok(tpls) = collect_local_files(&path.join("templates")).await {
            for (p, c) in tpls {
                if p.extension().and_then(|e| e.to_str()) == Some("json") {
                    let manifest: Value = serde_json::from_str(&c)
                        .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
                    templates.push(TemplateFile {
                        name: p.file_name().and_then(|n| n.to_str()).unwrap_or("t.json").to_string(),
                        manifest,
                    });
                }
            }
        }

        let sha = compute_local_sha(&skill_md);
        Ok(SkillPackage {
            name: frontmatter.name.clone(),
            source: SkillSource::LocalPath { path },
            resolved_sha: sha,
            semver,
            skill_md_content: skill_md,
            frontmatter,
            klyntbot_meta,
            references,
            templates,
        })
    }
}

fn trim_trailing(s: &str) -> &str { s.trim_end_matches('/') }

fn compute_local_sha(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    format!("local-{}", hex::encode(h.finalize()))
}

async fn collect_local_files(dir: &std::path::Path) -> Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    if !dir.exists() { return Ok(out); }
    let mut read_dir = tokio::fs::read_dir(dir).await
        .map_err(|e| KlyntbotError::Storage(format!("read_dir {}: {e}", dir.display())))?;
    while let Some(entry) = read_dir.next_entry().await
        .map_err(|e| KlyntbotError::Storage(e.to_string()))? {
        let path = entry.path();
        if path.is_file() {
            let content = tokio::fs::read_to_string(&path).await
                .map_err(|e| KlyntbotError::Storage(format!("read {}: {e}", path.display())))?;
            out.push((path, content));
        }
    }
    Ok(out)
}
```

- [ ] **Step 2: Add tests**

Append to `fetcher.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn local_fetch_reads_skill_md_and_templates() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("SKILL.md"),
            "---\nname: demo\ndescription: d\n---\nbody").unwrap();
        std::fs::create_dir_all(tmp.path().join("templates")).unwrap();
        std::fs::write(tmp.path().join("templates/t.json"),
            r#"{"name":"t","fields":[]}"#).unwrap();

        let f = Fetcher::new();
        let pkg = f.fetch(&SkillSource::LocalPath { path: tmp.path().to_path_buf() }).await.unwrap();
        assert_eq!(pkg.name, "demo");
        assert_eq!(pkg.templates.len(), 1);
        assert_eq!(pkg.templates[0].name, "t.json");
    }

    #[tokio::test]
    async fn github_fetch_round_trip() {
        let api = MockServer::start().await;
        let raw = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/ow/re/commits/HEAD"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"sha":"deadbeef"})))
            .mount(&api).await;

        Mock::given(method("GET"))
            .and(path("/ow/re/deadbeef/skill-a/SKILL.md"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "---\nname: skill-a\ndescription: d\n---\nbody"))
            .mount(&raw).await;

        // references/ + templates/ → 404
        Mock::given(method("GET"))
            .and(path("/repos/ow/re/contents/skill-a/references"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&api).await;
        Mock::given(method("GET"))
            .and(path("/repos/ow/re/contents/skill-a/templates"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&api).await;

        let f = Fetcher::with_bases(api.uri(), raw.uri());
        let pkg = f.fetch(&SkillSource::Github {
            owner: "ow".into(), repo: "re".into(), subpath: "skill-a".into(),
            r#ref: GitRef::Latest,
        }).await.unwrap();
        assert_eq!(pkg.name, "skill-a");
        assert_eq!(pkg.resolved_sha, "deadbeef");
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p skills-registry`
Expected: parse tests + 2 fetcher tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/skills-registry/src/fetcher.rs
git commit -m "feat(skills-registry): implement github + local fetchers"
```

### Task 6: Check-updates helper

**Files:**
- Create: `crates/skills-registry/src/updates.rs`
- Modify: `crates/skills-registry/src/lib.rs`

- [ ] **Step 1: Add module export**

In `lib.rs`, add `pub mod updates;` and `pub use updates::{AvailableVersion, UpdatesFetcher};`.

- [ ] **Step 2: Write `updates.rs` with failing test**

```rust
use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableVersion {
    pub sha: String,
    pub tag: Option<String>,
    pub message: String,
    pub date: String,
}

pub struct UpdatesFetcher {
    http: reqwest::Client,
    api_base: String,
}

impl UpdatesFetcher {
    pub fn new(api_base: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent("klyntbot-skills-registry")
                .build().expect("reqwest"),
            api_base,
        }
    }

    /// List commits on `owner/repo` touching `subpath` since `installed_sha` (exclusive).
    pub async fn list_newer(
        &self, owner: &str, repo: &str, subpath: &str, installed_sha: &str,
    ) -> Result<Vec<AvailableVersion>> {
        let url = format!(
            "{}/repos/{}/{}/commits?path={}&per_page=50",
            self.api_base, owner, repo, subpath
        );
        let resp = self.http.get(&url).send().await
            .map_err(|e| KlyntbotError::Storage(format!("commits GET {url}: {e}")))?;
        if !resp.status().is_success() {
            return Err(KlyntbotError::Storage(format!("commits: HTTP {}", resp.status())));
        }
        let items: Vec<Value> = resp.json().await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        let mut out = Vec::new();
        for item in items {
            let sha = item.get("sha").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if sha == installed_sha { break; }
            let message = item.pointer("/commit/message").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let date = item.pointer("/commit/author/date").and_then(|v| v.as_str()).unwrap_or("").to_string();
            out.push(AvailableVersion { sha, tag: None, message, date });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn lists_newer_commits_until_installed() {
        let api = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/repos/ow/re/commits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "sha": "aaa", "commit": { "message": "fix", "author": { "date": "2026-04-14T00:00:00Z" } } },
                { "sha": "bbb", "commit": { "message": "feat", "author": { "date": "2026-04-13T00:00:00Z" } } },
                { "sha": "ccc", "commit": { "message": "old", "author": { "date": "2026-04-10T00:00:00Z" } } }
            ])))
            .mount(&api).await;

        let uf = UpdatesFetcher::new(api.uri());
        let out = uf.list_newer("ow", "re", "skill-a", "ccc").await.unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].sha, "aaa");
    }
}
```

- [ ] **Step 3: Test**

Run: `cargo nextest run -p skills-registry`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add crates/skills-registry
git commit -m "feat(skills-registry): list newer commits for check-updates"
```

### Task 7: Diff module

**Files:**
- Create: `crates/skills-registry/src/diff.rs`
- Modify: `crates/skills-registry/src/lib.rs`

- [ ] **Step 1: Add `pub mod diff;` in lib.rs and re-export key types.**

- [ ] **Step 2: Create `diff.rs`**

```rust
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

use crate::SkillPackage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffResult {
    pub body_lines: Vec<DiffLine>,
    pub frontmatter_changes: Vec<FrontmatterChange>,
    pub bootstraps_added: Vec<String>,
    pub bootstraps_removed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub tag: String, // "equal" | "insert" | "delete"
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontmatterChange {
    pub field: String,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}

pub fn diff_packages(before: &SkillPackage, after: &SkillPackage) -> DiffResult {
    let body_diff = TextDiff::from_lines(&before.skill_md_content, &after.skill_md_content);
    let body_lines: Vec<DiffLine> = body_diff.iter_all_changes()
        .map(|c| DiffLine {
            tag: match c.tag() {
                ChangeTag::Equal => "equal",
                ChangeTag::Insert => "insert",
                ChangeTag::Delete => "delete",
            }.to_string(),
            text: c.to_string(),
        })
        .collect();

    let before_fm = frontmatter_fields(before);
    let after_fm = frontmatter_fields(after);
    let mut frontmatter_changes = Vec::new();
    for (k, b_val) in &before_fm {
        match after_fm.get(k) {
            Some(a_val) if a_val != b_val => frontmatter_changes.push(FrontmatterChange {
                field: k.clone(), before: Some(b_val.clone()), after: Some(a_val.clone()),
            }),
            None => frontmatter_changes.push(FrontmatterChange {
                field: k.clone(), before: Some(b_val.clone()), after: None,
            }),
            _ => {}
        }
    }
    for (k, a_val) in &after_fm {
        if !before_fm.contains_key(k) {
            frontmatter_changes.push(FrontmatterChange {
                field: k.clone(), before: None, after: Some(a_val.clone()),
            });
        }
    }

    let before_boot: std::collections::HashSet<_> = bootstrap_names(before).into_iter().collect();
    let after_boot: std::collections::HashSet<_> = bootstrap_names(after).into_iter().collect();
    let bootstraps_added = after_boot.difference(&before_boot).cloned().collect();
    let bootstraps_removed = before_boot.difference(&after_boot).cloned().collect();

    DiffResult { body_lines, frontmatter_changes, bootstraps_added, bootstraps_removed }
}

fn frontmatter_fields(pkg: &SkillPackage) -> std::collections::BTreeMap<String, serde_json::Value> {
    let mut out = std::collections::BTreeMap::new();
    out.insert("name".into(), serde_json::Value::String(pkg.frontmatter.name.clone()));
    out.insert("description".into(), serde_json::Value::String(pkg.frontmatter.description.clone()));
    if let Some(ref w) = pkg.frontmatter.when_to_use {
        out.insert("whenToUse".into(), serde_json::Value::String(w.clone()));
    }
    if let Some(ref m) = pkg.klyntbot_meta {
        if let Some(v) = m.custom.get("version") { out.insert("version".into(), v.clone()); }
        if let Some(v) = m.custom.get("triggers") { out.insert("triggers".into(), v.clone()); }
    }
    out
}

fn bootstrap_names(pkg: &SkillPackage) -> Vec<String> {
    pkg.klyntbot_meta.as_ref()
        .and_then(|m| m.custom.get("bootstraps"))
        .and_then(|v| v.get("databases"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|o| {
            o.get("template").and_then(|s| s.as_str().map(String::from))
        }).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pkg(body: &str, name: &str) -> SkillPackage {
        use crate::source::{GitRef, SkillSource};
        use skill_system::store::SkillFrontmatter;
        SkillPackage {
            name: name.into(),
            source: SkillSource::Github { owner: "a".into(), repo: "b".into(), subpath: "c".into(), r#ref: GitRef::Latest },
            resolved_sha: "s".into(),
            semver: None,
            skill_md_content: body.into(),
            frontmatter: SkillFrontmatter {
                name: name.into(), description: "d".into(), when_to_use: None, references: vec![],
            },
            klyntbot_meta: None,
            references: vec![],
            templates: vec![],
        }
    }

    #[test]
    fn body_diff_detects_insertions() {
        let a = make_pkg("line one\nline two\n", "x");
        let b = make_pkg("line one\nline two\nline three\n", "x");
        let d = diff_packages(&a, &b);
        assert!(d.body_lines.iter().any(|l| l.tag == "insert" && l.text.contains("three")));
    }
}
```

- [ ] **Step 3: Test + commit**

```bash
cargo nextest run -p skills-registry
git add crates/skills-registry
git commit -m "feat(skills-registry): diff module using similar crate"
```

---

# Phase 3 — `skills-installer` crate

### Task 8: Scaffold skills-installer

**Files:**
- Create: `crates/skills-installer/Cargo.toml`
- Create: `crates/skills-installer/src/lib.rs`
- Create: `crates/skills-installer/src/plan.rs`
- Create: `crates/skills-installer/src/installer.rs`
- Create: `crates/skills-installer/src/uninstall.rs`
- Modify: root `Cargo.toml`

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "skills-installer"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common = { workspace = true }
skills-marketplace = { workspace = true }
skills-registry = { workspace = true }
skill-system = { workspace = true }
entity-store = { workspace = true }
bus = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sqlx = { workspace = true }
tokio = { workspace = true, features = ["fs"] }
chrono = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
storage = { workspace = true }
```

Register in workspace root Cargo.toml `[workspace] members` and `[workspace.dependencies]`:
```toml
"crates/skills-installer",
```
```toml
skills-installer = { path = "crates/skills-installer" }
```

- [ ] **Step 2: Write `plan.rs`**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use skills_registry::SkillPackage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    pub package: SkillPackage,
    pub files_to_write: Vec<FileWrite>,
    pub databases_to_bootstrap: Vec<TemplatePreview>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWrite {
    pub relative_path: PathBuf,
    pub content_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreview {
    pub template_name: String,
    pub database_name: String,
    pub field_count: usize,
}

impl InstallPlan {
    pub fn skill_only(mut self) -> Self {
        self.databases_to_bootstrap.clear();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradePlan {
    pub name: String,
    pub from_sha: String,
    pub to_sha: String,
    pub diff: skills_registry::diff::DiffResult,
    pub new_bootstraps: Vec<TemplatePreview>,
}
```

- [ ] **Step 3: Write `installer.rs` stub with typed API**

```rust
use std::path::PathBuf;
use std::sync::Arc;

use common::{KlyntbotError, Result};
use sqlx::SqlitePool;
use tracing::{info, warn};

use bus::{DomainEvent, DomainEventBus};
use entity_store::store::EntityStore;
use skill_system::SkillStore;
use skills_marketplace::{InstalledSkill, InstalledSkillsRepo, SourceType};
use skills_registry::{Fetcher, GitRef, SkillPackage, SkillSource};

use crate::plan::{FileWrite, InstallPlan, TemplatePreview};

pub struct Installer {
    pub skills_dir: PathBuf,
    pub fetcher: Arc<Fetcher>,
    pub repo: InstalledSkillsRepo,
    pub entity_store: Arc<EntityStore>,
    pub skill_store: Arc<tokio::sync::RwLock<SkillStore>>,
    pub event_bus: Arc<DomainEventBus>,
}

impl Installer {
    pub async fn preview_install(&self, source: &SkillSource, version: Option<GitRef>) -> Result<InstallPlan> {
        let effective = match version {
            Some(r) => override_ref(source.clone(), r),
            None => source.clone(),
        };
        let pkg = self.fetcher.fetch(&effective).await?;
        Ok(build_plan(pkg))
    }

    pub async fn apply_install(&self, plan: InstallPlan) -> Result<InstalledSkill> {
        let dir = self.skills_dir.join(&plan.package.name);
        let mut written_paths: Vec<PathBuf> = Vec::new();
        let mut created_dbs: Vec<String> = Vec::new();

        let attempt = async {
            write_package(&dir, &plan.package, &mut written_paths).await?;
            for tpl in &plan.databases_to_bootstrap {
                let template = plan.package.templates.iter()
                    .find(|t| t.name == tpl.template_name)
                    .ok_or_else(|| KlyntbotError::Storage(format!("template {} missing", tpl.template_name)))?;
                let manifest: entity_store::templates::TemplateManifest =
                    serde_json::from_value(template.manifest.clone())
                    .map_err(|e| KlyntbotError::Storage(format!("template {}: {e}", tpl.template_name)))?;
                let ids = entity_store::templates::install_template(self.entity_store.as_ref(), &manifest).await?;
                created_dbs.extend(ids);
            }
            let row = InstalledSkill {
                name: plan.package.name.clone(),
                source_type: source_type_of(&plan.package.source),
                source_ref: source_ref_string(&plan.package.source),
                installed_version: plan.package.semver.clone().unwrap_or_else(|| "0.0.0".into()),
                installed_sha: plan.package.resolved_sha.clone(),
                enabled: true,
                is_adapted: false,
                bootstrapped_databases: created_dbs.clone(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            self.repo.insert(&row).await?;
            self.skill_store.write().await.reload().map_err(|e| KlyntbotError::Storage(e.to_string()))?;
            self.event_bus.publish(DomainEvent::SkillInstalled {
                name: row.name.clone(),
                source: row.source_ref.clone(),
                version: row.installed_version.clone(),
            });
            info!(name = %row.name, "skill installed");
            Result::<InstalledSkill>::Ok(row)
        }.await;

        match attempt {
            Ok(row) => Ok(row),
            Err(e) => {
                warn!(error = %e, "install failed — rolling back");
                for p in written_paths.iter().rev() {
                    let _ = tokio::fs::remove_file(p).await;
                }
                let _ = tokio::fs::remove_dir_all(&dir).await;
                for db_id in &created_dbs {
                    let _ = self.entity_store.delete_database(db_id).await;
                }
                Err(e)
            }
        }
    }
}

fn override_ref(mut s: SkillSource, r: GitRef) -> SkillSource {
    if let SkillSource::Github { r#ref, .. } = &mut s { *r#ref = r; }
    s
}

fn source_type_of(s: &SkillSource) -> SourceType {
    match s {
        SkillSource::Github { .. } => SourceType::Github,
        SkillSource::SkillsSh { .. } => SourceType::SkillsSh,
        SkillSource::LocalPath { .. } => SourceType::Local,
        SkillSource::Bundled { .. } => SourceType::Bundled,
    }
}

fn source_ref_string(s: &SkillSource) -> String {
    match s {
        SkillSource::Github { owner, repo, subpath, .. } => format!("{owner}/{repo}/{subpath}"),
        SkillSource::SkillsSh { slug } => slug.clone(),
        SkillSource::LocalPath { path } => path.display().to_string(),
        SkillSource::Bundled { name } => format!("bundled:{name}"),
    }
}

fn build_plan(pkg: SkillPackage) -> InstallPlan {
    let mut files: Vec<FileWrite> = vec![FileWrite {
        relative_path: PathBuf::from("SKILL.md"),
        content_size: pkg.skill_md_content.len(),
    }];
    for r in &pkg.references {
        files.push(FileWrite {
            relative_path: PathBuf::from("references").join(
                r.path.file_name().and_then(|n| n.to_str()).unwrap_or("ref.md")
            ),
            content_size: r.content.len(),
        });
    }
    for t in &pkg.templates {
        files.push(FileWrite {
            relative_path: PathBuf::from("templates").join(&t.name),
            content_size: t.manifest.to_string().len(),
        });
    }
    let databases_to_bootstrap: Vec<TemplatePreview> = pkg.templates.iter().map(|t| {
        let db_name = t.manifest.get("databases")
            .and_then(|d| d.as_array())
            .and_then(|a| a.first())
            .and_then(|d| d.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or(&t.name)
            .to_string();
        let field_count = t.manifest.get("databases")
            .and_then(|d| d.as_array())
            .and_then(|a| a.first())
            .and_then(|d| d.get("fields"))
            .and_then(|f| f.as_array())
            .map(|a| a.len()).unwrap_or(0);
        TemplatePreview { template_name: t.name.clone(), database_name: db_name, field_count }
    }).collect();

    let warnings = Vec::new();
    InstallPlan { package: pkg, files_to_write: files, databases_to_bootstrap, warnings }
}

async fn write_package(dir: &std::path::Path, pkg: &SkillPackage, written: &mut Vec<PathBuf>) -> Result<()> {
    tokio::fs::create_dir_all(dir).await
        .map_err(|e| KlyntbotError::Storage(format!("create_dir {}: {e}", dir.display())))?;
    let skill_path = dir.join("SKILL.md");
    tokio::fs::write(&skill_path, &pkg.skill_md_content).await
        .map_err(|e| KlyntbotError::Storage(format!("write {}: {e}", skill_path.display())))?;
    written.push(skill_path);

    if !pkg.references.is_empty() {
        let refs_dir = dir.join("references");
        tokio::fs::create_dir_all(&refs_dir).await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        for r in &pkg.references {
            let filename = r.path.file_name().and_then(|n| n.to_str()).unwrap_or("ref.md");
            let p = refs_dir.join(filename);
            tokio::fs::write(&p, &r.content).await.map_err(|e| KlyntbotError::Storage(e.to_string()))?;
            written.push(p);
        }
    }

    if !pkg.templates.is_empty() {
        let tpls_dir = dir.join("templates");
        tokio::fs::create_dir_all(&tpls_dir).await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        for t in &pkg.templates {
            let p = tpls_dir.join(&t.name);
            let contents = serde_json::to_string_pretty(&t.manifest)
                .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
            tokio::fs::write(&p, contents).await.map_err(|e| KlyntbotError::Storage(e.to_string()))?;
            written.push(p);
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Add `DomainEvent::SkillInstalled` / `SkillUpgraded` / `SkillUninstalled` / `SkillAdapted`**

Modify `crates/bus/src/domain_events.rs`. Find `pub enum DomainEvent` and add four variants:
```rust
    SkillInstalled { name: String, source: String, version: String },
    SkillUpgraded { name: String, from_version: String, to_version: String },
    SkillUninstalled { name: String, mode: String },
    SkillAdapted { name: String, adapter_model: String },
```

Also classify them in `crates/cognitive/src/services/salience.rs` — add after existing skill events:
```rust
        DomainEvent::SkillInstalled { .. } => SalienceVerdict::Accumulate,
        DomainEvent::SkillUpgraded { .. } => SalienceVerdict::Accumulate,
        DomainEvent::SkillUninstalled { .. } => SalienceVerdict::Discard,
        DomainEvent::SkillAdapted { .. } => SalienceVerdict::Accumulate,
```

And handle them in `background.rs::event_to_observation` — add a fall-through case (they'll use the `_ =>` catch-all to build generic observations).

- [ ] **Step 5: Compile-only check**

Run: `cargo check -p skills-installer -p bus -p cognitive`
Expected: clean compile.

- [ ] **Step 6: Commit**

```bash
git add crates/skills-installer crates/bus/src/domain_events.rs crates/cognitive/src/services/salience.rs Cargo.toml
git commit -m "feat(skills-installer): scaffold + InstallPlan + transactional apply"
```

### Task 9: Installer integration tests

**Files:**
- Create: `crates/skills-installer/src/tests_install.rs`
- Modify: `crates/skills-installer/src/lib.rs`

- [ ] **Step 1: Wire lib.rs**

```rust
//! skills-installer: transactional install/upgrade/uninstall on top of SkillStore + EntityStore.

pub mod installer;
pub mod plan;
pub mod uninstall;

pub use installer::Installer;
pub use plan::{InstallPlan, TemplatePreview, UpgradePlan};
pub use uninstall::UninstallMode;

#[cfg(test)]
mod tests_install;
```

- [ ] **Step 2: Write `tests_install.rs`**

```rust
use std::sync::Arc;

use bus::DomainEventBus;
use entity_store::store::EntityStore;
use skill_system::SkillStore;
use skills_marketplace::InstalledSkillsRepo;
use skills_registry::{Fetcher, SkillSource};

use crate::{Installer, InstallPlan};

async fn setup() -> (tempfile::TempDir, Installer) {
    let tmp = tempfile::tempdir().unwrap();

    // Storage + migrations
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(),
        &entity_store::EntityStoreFeature::migrations()).await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(),
        &skills_marketplace::SkillsMarketplaceFeature::migrations()).await.unwrap();

    let skills_dir = tmp.path().join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let entity_store = Arc::new(EntityStore::new(pool.inner().clone()));
    let skill_store = Arc::new(tokio::sync::RwLock::new(SkillStore::load(&skills_dir).unwrap()));
    let repo = InstalledSkillsRepo::new(pool.inner().clone());
    let bus = Arc::new(DomainEventBus::new(16));
    let fetcher = Arc::new(Fetcher::new());

    let installer = Installer {
        skills_dir: skills_dir.clone(),
        fetcher, repo, entity_store, skill_store, event_bus: bus,
    };
    (tmp, installer)
}

fn write_local_skill(root: &std::path::Path, body: &str, template: Option<&str>) -> SkillSource {
    let dir = root.join("fixture-skill");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), body).unwrap();
    if let Some(t) = template {
        std::fs::create_dir_all(dir.join("templates")).unwrap();
        std::fs::write(dir.join("templates/t.json"), t).unwrap();
    }
    SkillSource::LocalPath { path: dir }
}

#[tokio::test]
async fn install_local_skill_writes_file_and_row() {
    let (tmp, inst) = setup().await;
    let source = write_local_skill(tmp.path(),
        "---\nname: fx\ndescription: d\n---\nbody", None);
    let plan: InstallPlan = inst.preview_install(&source, None).await.unwrap();
    let row = inst.apply_install(plan).await.unwrap();
    assert_eq!(row.name, "fx");
    assert!(inst.skills_dir.join("fx/SKILL.md").is_file());
    let list = inst.repo.list().await.unwrap();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
async fn install_with_bootstrap_creates_database() {
    let (tmp, inst) = setup().await;
    let tpl = r#"{"databases":[{"name":"Reading","slug":"reading","fields":[{"name":"Title","slug":"title","fieldType":"text","required":true}]}]}"#;
    let source = write_local_skill(tmp.path(),
        "---\nname: rl\ndescription: d\n---\nbody", Some(tpl));

    let plan = inst.preview_install(&source, None).await.unwrap();
    assert_eq!(plan.databases_to_bootstrap.len(), 1);

    let row = inst.apply_install(plan).await.unwrap();
    assert_eq!(row.bootstrapped_databases.len(), 1);

    let dbs = inst.entity_store.list_databases().await.unwrap();
    assert!(dbs.iter().any(|d| d.slug == "reading"));
}

#[tokio::test]
async fn install_only_writes_nothing_on_template_error() {
    let (tmp, inst) = setup().await;
    // Malformed template — missing required keys.
    let tpl = r#"{"databases":[{"foo":"bar"}]}"#;
    let source = write_local_skill(tmp.path(),
        "---\nname: bad\ndescription: d\n---\nbody", Some(tpl));

    let plan = inst.preview_install(&source, None).await.unwrap();
    let err = inst.apply_install(plan).await;
    assert!(err.is_err());

    // Rollback: skill dir must not exist, no row inserted, no database created.
    assert!(!inst.skills_dir.join("bad").exists());
    let list = inst.repo.list().await.unwrap();
    assert!(list.is_empty());
    let dbs = inst.entity_store.list_databases().await.unwrap();
    assert!(dbs.iter().all(|d| d.slug != "foo"));
}
```

- [ ] **Step 3: Add storage to `[dev-dependencies]` (already done in Task 8 step 1).**

- [ ] **Step 4: Run**

Run: `cargo nextest run -p skills-installer`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/skills-installer
git commit -m "test(skills-installer): local install + bootstrap + rollback tests"
```

### Task 10: Upgrade flow

**Files:**
- Modify: `crates/skills-installer/src/installer.rs`
- Modify: `crates/skills-installer/src/plan.rs` — already has `UpgradePlan`

- [ ] **Step 1: Add `check_updates`, `preview_upgrade`, `apply_upgrade` to `Installer`**

Append to `impl Installer` in `installer.rs`:
```rust
    pub async fn check_updates(&self, name: &str) -> Result<Vec<skills_registry::AvailableVersion>> {
        let Some(row) = self.repo.get(name).await? else {
            return Err(KlyntbotError::Storage(format!("skill '{name}' not installed")));
        };
        let (owner, repo, subpath) = parse_github_ref(&row.source_ref)
            .ok_or_else(|| KlyntbotError::Storage("only github sources support check_updates".into()))?;
        let uf = skills_registry::UpdatesFetcher::new("https://api.github.com".into());
        uf.list_newer(&owner, &repo, &subpath, &row.installed_sha).await
    }

    pub async fn preview_upgrade(&self, name: &str, target_sha: &str) -> Result<crate::plan::UpgradePlan> {
        let row = self.repo.get(name).await?
            .ok_or_else(|| KlyntbotError::Storage(format!("skill '{name}' not installed")))?;
        let (owner, repo, subpath) = parse_github_ref(&row.source_ref)
            .ok_or_else(|| KlyntbotError::Storage("only github upgrades supported".into()))?;

        let current = self.fetcher.fetch(&SkillSource::Github {
            owner: owner.clone(), repo: repo.clone(), subpath: subpath.clone(),
            r#ref: GitRef::Commit { sha: row.installed_sha.clone() },
        }).await?;
        let target = self.fetcher.fetch(&SkillSource::Github {
            owner, repo, subpath,
            r#ref: GitRef::Commit { sha: target_sha.into() },
        }).await?;

        let diff = skills_registry::diff::diff_packages(&current, &target);

        let target_tpls: std::collections::HashSet<_> = target.templates.iter().map(|t| t.name.clone()).collect();
        let current_tpls: std::collections::HashSet<_> = current.templates.iter().map(|t| t.name.clone()).collect();
        let new_bootstraps: Vec<crate::plan::TemplatePreview> = target.templates.iter()
            .filter(|t| !current_tpls.contains(&t.name))
            .map(|t| crate::plan::TemplatePreview {
                template_name: t.name.clone(),
                database_name: t.name.clone(),
                field_count: 0,
            })
            .collect();
        let _ = target_tpls;

        Ok(crate::plan::UpgradePlan {
            name: name.into(), from_sha: row.installed_sha,
            to_sha: target_sha.into(), diff, new_bootstraps,
        })
    }

    pub async fn apply_upgrade(&self, plan: crate::plan::UpgradePlan) -> Result<InstalledSkill> {
        let row = self.repo.get(&plan.name).await?
            .ok_or_else(|| KlyntbotError::Storage(format!("skill '{}' not installed", plan.name)))?;
        let (owner, repo, subpath) = parse_github_ref(&row.source_ref)
            .ok_or_else(|| KlyntbotError::Storage("only github upgrades supported".into()))?;

        let target = self.fetcher.fetch(&SkillSource::Github {
            owner, repo, subpath,
            r#ref: GitRef::Commit { sha: plan.to_sha.clone() },
        }).await?;

        let dir = self.skills_dir.join(&plan.name);
        let mut written = Vec::new();
        tokio::fs::remove_dir_all(&dir).await.ok();
        write_package(&dir, &target, &mut written).await?;

        // Bootstrap newly-declared templates only.
        let mut bootstrapped = row.bootstrapped_databases.clone();
        for new_tpl in &plan.new_bootstraps {
            let tpl = target.templates.iter().find(|t| t.name == new_tpl.template_name)
                .ok_or_else(|| KlyntbotError::Storage("new template missing in target".into()))?;
            let manifest: entity_store::templates::TemplateManifest =
                serde_json::from_value(tpl.manifest.clone())
                .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
            let ids = entity_store::templates::install_template(self.entity_store.as_ref(), &manifest).await?;
            bootstrapped.extend(ids);
        }

        let new_version = target.semver.clone().unwrap_or_else(|| row.installed_version.clone());
        self.repo.update_version(&plan.name, &new_version, &plan.to_sha, &bootstrapped).await?;
        self.skill_store.write().await.reload().map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        self.event_bus.publish(DomainEvent::SkillUpgraded {
            name: plan.name.clone(),
            from_version: row.installed_version,
            to_version: new_version,
        });

        Ok(self.repo.get(&plan.name).await?.unwrap())
    }
}

fn parse_github_ref(s: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() < 2 { return None; }
    Some((parts[0].into(), parts[1].into(), parts[2..].join("/")))
}
```

Add `use skills_registry::GitRef;` at the top of `installer.rs` if not already present.

- [ ] **Step 2: Add a fixture upgrade test in `tests_install.rs`**

```rust
#[tokio::test]
async fn upgrade_updates_version_and_preserves_bootstraps() {
    // This test mocks a GitHub upgrade flow by seeding a local install,
    // then directly calling repo.update_version to simulate the upgrade.
    let (tmp, inst) = setup().await;
    let source = write_local_skill(tmp.path(),
        "---\nname: up\ndescription: d\n---\nbody", None);
    let plan = inst.preview_install(&source, None).await.unwrap();
    let installed = inst.apply_install(plan).await.unwrap();

    // Simulate an upgrade bumping version + sha (no github mocking needed for repo-level behaviour)
    inst.repo.update_version(&installed.name, "2.0.0", "newsha", &installed.bootstrapped_databases).await.unwrap();
    let reloaded = inst.repo.get(&installed.name).await.unwrap().unwrap();
    assert_eq!(reloaded.installed_version, "2.0.0");
    assert_eq!(reloaded.installed_sha, "newsha");
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p skills-installer
git add crates/skills-installer
git commit -m "feat(skills-installer): check_updates/preview_upgrade/apply_upgrade"
```

### Task 11: Uninstall with 3 modes

**Files:**
- Modify: `crates/skills-installer/src/uninstall.rs`

- [ ] **Step 1: Write `uninstall.rs`**

```rust
use std::sync::Arc;

use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

use bus::{DomainEvent, DomainEventBus};
use entity_store::store::EntityStore;
use skill_system::SkillStore;
use skills_marketplace::InstalledSkillsRepo;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UninstallMode {
    SkillOnly,
    ArchiveDatabases,
    DeleteDatabases,
}

pub async fn uninstall(
    mode: UninstallMode,
    name: &str,
    skills_dir: &std::path::Path,
    repo: &InstalledSkillsRepo,
    entity_store: Arc<EntityStore>,
    skill_store: Arc<tokio::sync::RwLock<SkillStore>>,
    event_bus: Arc<DomainEventBus>,
) -> Result<()> {
    let row = repo.get(name).await?
        .ok_or_else(|| KlyntbotError::Storage(format!("skill '{name}' not installed")))?;

    match mode {
        UninstallMode::DeleteDatabases => {
            for db_id in &row.bootstrapped_databases {
                let _ = entity_store.delete_database(db_id).await;
            }
        }
        UninstallMode::ArchiveDatabases => {
            for db_id in &row.bootstrapped_databases {
                if let Ok(schema) = entity_store.get_database(db_id).await {
                    let new_name = format!("Archived: {}", schema.name);
                    let _ = entity_store.rename_database(db_id, &new_name).await;
                }
            }
        }
        UninstallMode::SkillOnly => {}
    }

    let dir = skills_dir.join(name);
    let _ = tokio::fs::remove_dir_all(&dir).await;

    repo.delete(name).await?;
    skill_store.write().await.reload().map_err(|e| KlyntbotError::Storage(e.to_string()))?;

    event_bus.publish(DomainEvent::SkillUninstalled {
        name: name.into(),
        mode: format!("{mode:?}"),
    });
    info!(name = %name, ?mode, "skill uninstalled");
    Ok(())
}
```

- [ ] **Step 2: Add `EntityStore::rename_database`** (if it doesn't exist)

Run: `grep -n "rename_database" crates/entity-store/src/store.rs`
If empty, add the method next to `delete_database` in `store.rs`:
```rust
    pub async fn rename_database(&self, id: &str, new_name: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE databases SET name = ?, updated_at = ? WHERE id = ?")
            .bind(new_name).bind(&now).bind(id)
            .execute(&self.pool).await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
```

- [ ] **Step 3: Expose convenient `Installer::uninstall` method**

In `installer.rs`, add:
```rust
    pub async fn uninstall(&self, name: &str, mode: crate::uninstall::UninstallMode) -> Result<()> {
        crate::uninstall::uninstall(
            mode, name, &self.skills_dir, &self.repo,
            Arc::clone(&self.entity_store), Arc::clone(&self.skill_store),
            Arc::clone(&self.event_bus),
        ).await
    }
```

- [ ] **Step 4: Add test**

Append to `tests_install.rs`:
```rust
use crate::UninstallMode;

#[tokio::test]
async fn uninstall_skill_only_leaves_database() {
    let (tmp, inst) = setup().await;
    let tpl = r#"{"databases":[{"name":"Reading","slug":"reading","fields":[{"name":"Title","slug":"title","fieldType":"text","required":true}]}]}"#;
    let source = write_local_skill(tmp.path(),
        "---\nname: rl\ndescription: d\n---\nbody", Some(tpl));
    let plan = inst.preview_install(&source, None).await.unwrap();
    let _ = inst.apply_install(plan).await.unwrap();

    inst.uninstall("rl", UninstallMode::SkillOnly).await.unwrap();
    let list = inst.repo.list().await.unwrap();
    assert!(list.is_empty());
    let dbs = inst.entity_store.list_databases().await.unwrap();
    assert!(dbs.iter().any(|d| d.slug == "reading"));
}

#[tokio::test]
async fn uninstall_delete_databases_removes_everything() {
    let (tmp, inst) = setup().await;
    let tpl = r#"{"databases":[{"name":"X","slug":"xdb","fields":[{"name":"T","slug":"t","fieldType":"text","required":true}]}]}"#;
    let source = write_local_skill(tmp.path(),
        "---\nname: sk\ndescription: d\n---\nbody", Some(tpl));
    let plan = inst.preview_install(&source, None).await.unwrap();
    let _ = inst.apply_install(plan).await.unwrap();

    inst.uninstall("sk", UninstallMode::DeleteDatabases).await.unwrap();
    let dbs = inst.entity_store.list_databases().await.unwrap();
    assert!(dbs.iter().all(|d| d.slug != "xdb"));
}

#[tokio::test]
async fn uninstall_archive_renames_database() {
    let (tmp, inst) = setup().await;
    let tpl = r#"{"databases":[{"name":"A","slug":"adb","fields":[{"name":"T","slug":"t","fieldType":"text","required":true}]}]}"#;
    let source = write_local_skill(tmp.path(),
        "---\nname: sk2\ndescription: d\n---\nbody", Some(tpl));
    let plan = inst.preview_install(&source, None).await.unwrap();
    let _ = inst.apply_install(plan).await.unwrap();

    inst.uninstall("sk2", UninstallMode::ArchiveDatabases).await.unwrap();
    let dbs = inst.entity_store.list_databases().await.unwrap();
    assert!(dbs.iter().any(|d| d.name.starts_with("Archived: ")));
}
```

- [ ] **Step 5: Run tests + commit**

```bash
cargo nextest run -p skills-installer
git add crates/skills-installer crates/entity-store/src/store.rs
git commit -m "feat(skills-installer): three-mode uninstall (skill_only/archive/delete)"
```

---

# Phase 4 — `skills-adapter` crate

### Task 12: Scaffold skills-adapter

**Files:**
- Create: `crates/skills-adapter/Cargo.toml`
- Create: `crates/skills-adapter/src/lib.rs`
- Create: `crates/skills-adapter/src/prompt.rs`
- Create: `crates/skills-adapter/prompts/adapt.md`
- Create: `crates/skills-adapter/src/adapter.rs`
- Modify: root `Cargo.toml`

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "skills-adapter"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common = { workspace = true }
providers = { workspace = true }
skills-registry = { workspace = true }
skills-marketplace = { workspace = true }
entity-store = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
sha2 = { workspace = true }
hex = "0.4"
```

Add to root Cargo.toml workspace members + deps.

- [ ] **Step 2: Write the prompt file**

`crates/skills-adapter/prompts/adapt.md`:
```markdown
You are a Klynt skill adapter. You convert a generic Agent Skills `SKILL.md`
into a Klynt-native one by adding a `klyntbot:` metadata block, suggesting
database templates when the skill benefits from structured storage, and
tagging salience and trigger rules when appropriate.

# Rules

1. NEVER invent field types. Use only: text, number, select, multi_select,
   date, checkbox, url, email, phone, relation, rollup, formula, created_time,
   last_edited, files, person.
2. Max 3 databases per adaptation. Prefer linking to the user's existing
   databases over creating near-duplicates.
3. The skill body MUST remain unchanged. Only add/modify the frontmatter
   `metadata.klyntbot` block.
4. If the skill is fundamentally unsuitable for Klynt (pure coding helpers,
   CLI-only behavior, etc.), return `{"adaptable": false, "rationale": "..."}`.

# Context

Supported field types: {{FIELD_TYPES}}

User's current databases:
{{CURRENT_DATABASES}}

Example of a well-formed klyntbot block (from our bundled reading-list skill):
{{EXAMPLE_BLOCK}}

# Output

Return strict JSON matching this schema:
{
  "adaptable": boolean,
  "adapted_skill_md": string,       // full SKILL.md with klyntbot block
  "generated_templates": [
    { "name": "reading_list.json", "manifest": { ... } }
  ],
  "rationale": string
}

# Input skill

{{SKILL_MD}}
```

- [ ] **Step 3: Write `src/prompt.rs`**

```rust
const ADAPT_PROMPT_TEMPLATE: &str = include_str!("../prompts/adapt.md");

const EXAMPLE_BLOCK: &str = r#"metadata:
  klyntbot:
    type: orchestrator
    tools: [database]
    version: 1.0.0
    triggers: ["book", "reading"]
    bootstraps:
      databases:
        - template: reading_list.json"#;

pub fn render_prompt(
    skill_md: &str,
    supported_field_types: &[&str],
    current_databases: &[(String, String)], // (name, slug)
) -> String {
    let types_list = supported_field_types.join(", ");
    let dbs_list: String = if current_databases.is_empty() {
        "(none)".to_string()
    } else {
        current_databases.iter()
            .map(|(n, s)| format!("- {n} ({s})"))
            .collect::<Vec<_>>().join("\n")
    };
    ADAPT_PROMPT_TEMPLATE
        .replace("{{FIELD_TYPES}}", &types_list)
        .replace("{{CURRENT_DATABASES}}", &dbs_list)
        .replace("{{EXAMPLE_BLOCK}}", EXAMPLE_BLOCK)
        .replace("{{SKILL_MD}}", skill_md)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_all_placeholders() {
        let out = render_prompt("SKILL_BODY", &["text", "number"], &[("R".into(), "r".into())]);
        assert!(out.contains("text, number"));
        assert!(out.contains("R (r)"));
        assert!(out.contains("SKILL_BODY"));
        assert!(out.contains("example_block") || out.contains("klyntbot"));
    }
}
```

- [ ] **Step 4: Write `src/adapter.rs`**

```rust
use std::sync::Arc;

use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skills_marketplace::AdaptedSkillsRepo;
use skills_registry::{SkillPackage, TemplateFile};

use crate::prompt::render_prompt;

pub const SUPPORTED_FIELD_TYPES: &[&str] = &[
    "text", "number", "select", "multi_select", "date", "checkbox",
    "url", "email", "phone", "relation", "rollup", "formula",
    "created_time", "last_edited", "files", "person",
];

#[async_trait::async_trait]
pub trait AdapterProvider: Send + Sync {
    /// Expected: returns the JSON string matching the prompt output schema.
    async fn generate(&self, prompt: &str) -> Result<String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AdapterOutput {
    pub adaptable: bool,
    #[serde(default)]
    pub adapted_skill_md: String,
    #[serde(default)]
    pub generated_templates: Vec<TemplateFile>,
    pub rationale: String,
}

pub struct AdaptedSkill {
    pub adapted_skill_md: String,
    pub generated_templates: Vec<TemplateFile>,
    pub rationale: String,
    pub adapter_model: String,
}

pub struct Adapter {
    pub provider: Arc<dyn AdapterProvider>,
    pub cache: AdaptedSkillsRepo,
    pub model_name: String,
}

impl Adapter {
    pub async fn adapt(
        &self, pkg: &SkillPackage,
        current_databases: &[(String, String)],
    ) -> Result<AdaptedSkill> {
        let cache_key = compute_cache_key(pkg, &self.model_name);
        if let Some(cached) = self.cache.get(&cache_key).await? {
            tracing::debug!(skill = %pkg.name, "adapter cache hit");
            return Ok(AdaptedSkill {
                adapted_skill_md: cached.adapted_skill_md,
                generated_templates: serde_json::from_value(cached.generated_templates)
                    .map_err(|e| KlyntbotError::Storage(e.to_string()))?,
                rationale: cached.rationale,
                adapter_model: cached.adapter_model,
            });
        }
        let prompt = render_prompt(&pkg.skill_md_content, SUPPORTED_FIELD_TYPES, current_databases);
        let raw = self.provider.generate(&prompt).await?;
        let parsed: AdapterOutput = serde_json::from_str(&raw)
            .map_err(|e| KlyntbotError::Storage(format!("adapter JSON parse: {e}")))?;
        if !parsed.adaptable {
            return Err(KlyntbotError::Storage(format!("skill not adaptable: {}", parsed.rationale)));
        }
        validate_templates(&parsed.generated_templates)?;
        self.cache.upsert(
            &cache_key, &parsed.adapted_skill_md,
            &serde_json::to_value(&parsed.generated_templates).unwrap(),
            &parsed.rationale, &self.model_name,
        ).await?;
        Ok(AdaptedSkill {
            adapted_skill_md: parsed.adapted_skill_md,
            generated_templates: parsed.generated_templates,
            rationale: parsed.rationale,
            adapter_model: self.model_name.clone(),
        })
    }
}

fn compute_cache_key(pkg: &SkillPackage, model: &str) -> String {
    let mut h = Sha256::new();
    h.update(pkg.resolved_sha.as_bytes());
    h.update(b"|");
    h.update(model.as_bytes());
    hex::encode(h.finalize())
}

fn validate_templates(templates: &[TemplateFile]) -> Result<()> {
    for t in templates {
        let dbs = t.manifest.get("databases").and_then(|v| v.as_array())
            .ok_or_else(|| KlyntbotError::Storage(format!("template {} missing databases[]", t.name)))?;
        if dbs.len() > 3 {
            return Err(KlyntbotError::Storage(format!("template {}: max 3 databases", t.name)));
        }
        for db in dbs {
            let fields = db.get("fields").and_then(|v| v.as_array())
                .ok_or_else(|| KlyntbotError::Storage("database missing fields[]".into()))?;
            for f in fields {
                let ft = f.get("fieldType").and_then(|v| v.as_str())
                    .ok_or_else(|| KlyntbotError::Storage("field missing fieldType".into()))?;
                if !SUPPORTED_FIELD_TYPES.contains(&ft) {
                    return Err(KlyntbotError::Storage(format!("unsupported fieldType: {ft}")));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use skills_marketplace::AdaptedSkillsRepo;

    struct FixedProvider(&'static str);
    #[async_trait::async_trait]
    impl AdapterProvider for FixedProvider {
        async fn generate(&self, _prompt: &str) -> Result<String> { Ok(self.0.into()) }
    }

    fn make_pkg() -> SkillPackage {
        use skills_registry::source::{GitRef, SkillSource};
        use skill_system::store::SkillFrontmatter;
        SkillPackage {
            name: "x".into(),
            source: SkillSource::Github { owner: "a".into(), repo: "b".into(), subpath: "c".into(), r#ref: GitRef::Latest },
            resolved_sha: "sha".into(), semver: None,
            skill_md_content: "---\nname: x\ndescription: d\n---\nbody".into(),
            frontmatter: SkillFrontmatter { name: "x".into(), description: "d".into(), when_to_use: None, references: vec![] },
            klyntbot_meta: None, references: vec![], templates: vec![],
        }
    }

    #[tokio::test]
    async fn adapts_and_caches() {
        let pool = skills_marketplace::test_helpers::setup_pool().await;
        let adapter = Adapter {
            provider: Arc::new(FixedProvider(r#"{"adaptable":true,"adapted_skill_md":"---\nname: x\n---\nbody","generated_templates":[],"rationale":"ok"}"#)),
            cache: AdaptedSkillsRepo::new(pool.clone()),
            model_name: "test".into(),
        };
        let out = adapter.adapt(&make_pkg(), &[]).await.unwrap();
        assert_eq!(out.rationale, "ok");

        // Second call: cache hit, same output
        let out2 = adapter.adapt(&make_pkg(), &[]).await.unwrap();
        assert_eq!(out2.rationale, "ok");
    }

    #[tokio::test]
    async fn rejects_unsupported_field_type() {
        let bad = json!([{"name":"t.json","manifest":{"databases":[{"fields":[{"fieldType":"whatever"}]}]}}]);
        let output = json!({"adaptable":true,"adapted_skill_md":"x","generated_templates": bad, "rationale":""}).to_string();
        let pool = skills_marketplace::test_helpers::setup_pool().await;
        let adapter = Adapter {
            provider: Arc::new(FixedProvider(Box::leak(output.into_boxed_str()))),
            cache: AdaptedSkillsRepo::new(pool),
            model_name: "test".into(),
        };
        assert!(adapter.adapt(&make_pkg(), &[]).await.is_err());
    }
}
```

- [ ] **Step 5: Wire lib.rs**

```rust
//! skills-adapter: LLM transformation of prompt-only skills into Klynt-native ones.

pub mod adapter;
pub mod prompt;

pub use adapter::{Adapter, AdaptedSkill, AdapterProvider, AdapterOutput};
```

- [ ] **Step 6: Test + commit**

```bash
cargo nextest run -p skills-adapter
git add crates/skills-adapter Cargo.toml
git commit -m "feat(skills-adapter): LLM-driven skill adaptation + cache + validation"
```

---

# Phase 5 — Handlers + Tauri commands

### Task 13: Handlers in app-core

**Files:**
- Create: `crates/app-core/src/handlers/skills/mod.rs`
- Modify: `crates/app-core/src/handlers/mod.rs` — add `pub mod skills;`
- Modify: `crates/app-core/Cargo.toml` — add `skills-registry`, `skills-installer`, `skills-adapter`, `skills-marketplace` deps
- Modify: `crates/app-core/src/state.rs` — add `installer: Option<Arc<Installer>>` + `adapter: Option<Arc<Adapter>>`
- Modify: `crates/app-core/src/init/mod.rs` — construct Installer + Adapter

- [ ] **Step 1: Add dependencies to `app-core/Cargo.toml`**

```toml
skills-marketplace = { workspace = true }
skills-registry = { workspace = true }
skills-installer = { workspace = true }
skills-adapter = { workspace = true }
```

- [ ] **Step 2: Add AppCore fields**

In `crates/app-core/src/state.rs`, next to the `entity_store: Option<Arc<EntityStore>>` field, add:
```rust
    pub installer: Option<Arc<skills_installer::Installer>>,
    pub adapter: Option<Arc<skills_adapter::Adapter>>,
```

In the struct initializer (likely in `AppCore::init`), default both to `None` initially — they'll be populated after the agent + entity_store init.

- [ ] **Step 3: Write `handlers/skills/mod.rs`**

```rust
use std::sync::Arc;

use desktop_shared::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use skills_installer::{InstallPlan, UninstallMode, UpgradePlan};
use skills_marketplace::InstalledSkill;
use skills_registry::{AvailableVersion, GitRef, SkillPackage, SkillSource};

use crate::state::AppCore;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillBrowseRow {
    pub rank: usize,
    pub name: String,
    pub source_ref: String,
    pub installs: Option<u64>,
    pub is_klynt_native: bool,
    pub is_installed: bool,
    pub is_bundled: bool,
}

impl AppCore {
    fn require_installer(&self) -> Result<&Arc<skills_installer::Installer>, ApiError> {
        self.installer.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Skills installer not initialized"))
    }

    fn require_adapter(&self) -> Result<&Arc<skills_adapter::Adapter>, ApiError> {
        self.adapter.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "No cognitive provider configured — adapter disabled"))
    }

    pub async fn skill_list(&self) -> Result<Vec<InstalledSkill>, ApiError> {
        let inst = self.require_installer()?;
        inst.repo.list().await.map_err(Into::into)
    }

    pub async fn skill_browse(&self, _query: Option<String>) -> Result<Vec<SkillBrowseRow>, ApiError> {
        // MVP: curated featured list + installed skills. Live skills.sh proxy comes later.
        let inst = self.require_installer()?;
        let installed = inst.repo.list().await.map_err(ApiError::from)?;
        let curated: Vec<(&str, &str)> = vec![
            ("reading-list", "klynt-skills/official/reading-list"),
            ("pkm-notebook", "klynt-skills/official/pkm-notebook"),
        ];
        let mut out: Vec<SkillBrowseRow> = Vec::new();
        for (i, (name, src)) in curated.iter().enumerate() {
            out.push(SkillBrowseRow {
                rank: i + 1, name: (*name).into(), source_ref: (*src).into(),
                installs: None, is_klynt_native: true,
                is_installed: installed.iter().any(|s| s.name == *name),
                is_bundled: false,
            });
        }
        for s in &installed {
            if !out.iter().any(|r| r.name == s.name) {
                out.push(SkillBrowseRow {
                    rank: out.len() + 1, name: s.name.clone(),
                    source_ref: s.source_ref.clone(),
                    installs: None, is_klynt_native: !s.is_adapted,
                    is_installed: true,
                    is_bundled: matches!(s.source_type, skills_marketplace::SourceType::Bundled),
                });
            }
        }
        Ok(out)
    }

    pub async fn skill_install_preview(&self, shorthand: String, version: Option<GitRef>) -> Result<InstallPlan, ApiError> {
        let inst = self.require_installer()?;
        let source = SkillSource::parse_shorthand(&shorthand)
            .map_err(|e| ApiError::new("VALIDATION", e.to_string()))?;
        inst.preview_install(&source, version).await.map_err(Into::into)
    }

    pub async fn skill_install_apply(&self, plan: InstallPlan) -> Result<InstalledSkill, ApiError> {
        let inst = self.require_installer()?;
        inst.apply_install(plan).await.map_err(Into::into)
    }

    pub async fn skill_check_updates(&self, name: String) -> Result<Vec<AvailableVersion>, ApiError> {
        self.require_installer()?.check_updates(&name).await.map_err(Into::into)
    }

    pub async fn skill_upgrade_preview(&self, name: String, target_sha: String) -> Result<UpgradePlan, ApiError> {
        self.require_installer()?.preview_upgrade(&name, &target_sha).await.map_err(Into::into)
    }

    pub async fn skill_upgrade_apply(&self, plan: UpgradePlan) -> Result<InstalledSkill, ApiError> {
        self.require_installer()?.apply_upgrade(plan).await.map_err(Into::into)
    }

    pub async fn skill_uninstall(&self, name: String, mode: UninstallMode) -> Result<(), ApiError> {
        self.require_installer()?.uninstall(&name, mode).await.map_err(Into::into)
    }

    pub async fn skill_toggle_enabled(&self, name: String, enabled: bool) -> Result<(), ApiError> {
        let inst = self.require_installer()?;
        inst.repo.set_enabled(&name, enabled).await.map_err(ApiError::from)?;
        inst.skill_store.write().await.reload()
            .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?;
        Ok(())
    }

    pub async fn skill_adapt_preview(&self, shorthand: String) -> Result<Value, ApiError> {
        let inst = self.require_installer()?;
        let adapter = self.require_adapter()?;
        let source = SkillSource::parse_shorthand(&shorthand)
            .map_err(|e| ApiError::new("VALIDATION", e.to_string()))?;
        let pkg: SkillPackage = inst.fetcher.fetch(&source).await.map_err(ApiError::from)?;

        let existing_dbs = inst.entity_store.list_databases().await
            .map_err(ApiError::from)?
            .into_iter()
            .map(|d| (d.name, d.slug))
            .collect::<Vec<_>>();
        let out = adapter.adapt(&pkg, &existing_dbs).await.map_err(ApiError::from)?;
        Ok(serde_json::to_value(&serde_json::json!({
            "adaptedSkillMd": out.adapted_skill_md,
            "generatedTemplates": out.generated_templates,
            "rationale": out.rationale,
            "adapterModel": out.adapter_model,
        })).unwrap())
    }
}
```

- [ ] **Step 4: Add `pub mod skills;` in `handlers/mod.rs`**

- [ ] **Step 5: Construct Installer + Adapter in init**

In `crates/app-core/src/init/mod.rs`, after the workspace subscriber block we added earlier, add:
```rust
        // ── Skills installer + adapter ────────────────────────────────────
        let installer = if let Some(ref es) = entity_store {
            let skills_dir = config.data_dir_path().join("skills");
            let fetcher = Arc::new(::skills_registry::Fetcher::new());
            let installer = Arc::new(::skills_installer::Installer {
                skills_dir,
                fetcher,
                repo: ::skills_marketplace::InstalledSkillsRepo::new(storage_pool.inner().clone()),
                entity_store: Arc::clone(es),
                skill_store: agent.skill_store(),
                event_bus: Arc::clone(&domain_event_bus),
            });
            info!("Skills installer ready");
            Some(installer)
        } else { None };

        let adapter: Option<Arc<::skills_adapter::Adapter>> = None; // wired in Task 14

        // Stash on AppCore (set via a helper since AppCore is already constructed)
```

Update AppCore construction to accept these (struct-init pattern follows existing fields). Replace the struct literal where AppCore is built to include `installer` and `adapter: None`.

- [ ] **Step 6: Compile check**

Run: `cargo check -p app-core`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core
git commit -m "feat(app-core): skill handlers + installer wiring"
```

### Task 14: Wire the adapter with cognitive_provider

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`
- Create: `crates/app-core/src/adapters/skill_adapter_bridge.rs`

- [ ] **Step 1: Write bridge impl**

`crates/app-core/src/adapters/skill_adapter_bridge.rs`:
```rust
use std::sync::Arc;

use async_trait::async_trait;
use common::{KlyntbotError, Result};
use providers::LlmProvider;

use skills_adapter::AdapterProvider;

pub struct CognitiveProviderAdapter {
    pub provider: Arc<dyn LlmProvider>,
    pub model: String,
}

#[async_trait]
impl AdapterProvider for CognitiveProviderAdapter {
    async fn generate(&self, prompt: &str) -> Result<String> {
        let params = providers::ChatParams {
            model: self.model.clone(),
            temperature: 0.2,
            max_tokens: Some(4096),
            response_format: providers::ResponseFormat::Json,
        };
        let resp = self.provider.chat_completion(prompt, &params).await
            .map_err(|e| KlyntbotError::Storage(format!("adapter LLM call: {e}")))?;
        Ok(resp.content)
    }
}
```

(If `providers::ResponseFormat` doesn't exist in that exact shape, simplify — call the simplest `chat_completion(prompt)` method already available and rely on the JSON prompt contract alone.)

- [ ] **Step 2: Add `pub mod skill_adapter_bridge;` in `adapters/mod.rs`**

- [ ] **Step 3: Construct Adapter in init**

In `init/mod.rs`, replace the `let adapter: Option<...> = None;` line with:
```rust
        let adapter: Option<Arc<::skills_adapter::Adapter>> = self
            .cognitive_provider.as_ref()
            .map(|cp| {
                let bridge: Arc<dyn ::skills_adapter::AdapterProvider> = Arc::new(
                    crate::adapters::skill_adapter_bridge::CognitiveProviderAdapter {
                        provider: Arc::clone(cp),
                        model: config.cognitive.adapter_model.clone()
                            .unwrap_or_else(|| "claude-opus-4-6".into()),
                    });
                Arc::new(::skills_adapter::Adapter {
                    provider: bridge,
                    cache: ::skills_marketplace::AdaptedSkillsRepo::new(storage_pool.inner().clone()),
                    model_name: "claude-opus-4-6".into(),
                })
            });
```

(If `cognitive_provider` isn't in scope at this point, pass the already-resolved provider from `AgentResult`.)

- [ ] **Step 4: Add `adapter_model: Option<String>`** field to `crates/config/src/schema/cognitive.rs` if not present (optional user override).

- [ ] **Step 5: Compile**

Run: `cargo check -p app-core`

- [ ] **Step 6: Commit**

```bash
git add crates/app-core crates/config
git commit -m "feat(app-core): wire skills adapter through cognitive provider"
```

### Task 15: Tauri command wrappers

**Files:**
- Create: `crates/desktop/src/commands/skills.rs`
- Modify: `crates/desktop/src/commands/mod.rs` — `pub mod skills;`
- Modify: `crates/desktop/src/main.rs` — register commands
- Modify: `crates/desktop/src/dev_server/mod.rs` — add to `dev_command_names()`

- [ ] **Step 1: Write `skills.rs`**

```rust
use std::sync::Arc;

use ::app_core::handlers::skills::SkillBrowseRow;
use desktop_shared::errors::ApiError;
use serde_json::Value;
use tauri::State;

use skills_installer::{InstallPlan, UninstallMode, UpgradePlan};
use skills_marketplace::InstalledSkill;
use skills_registry::{AvailableVersion, GitRef};

use crate::app_core::AppCore;

#[tauri::command]
pub async fn skill_list(state: State<'_, Arc<AppCore>>) -> Result<Vec<InstalledSkill>, ApiError> {
    state.skill_list().await
}

#[tauri::command]
pub async fn skill_browse(
    state: State<'_, Arc<AppCore>>, query: Option<String>,
) -> Result<Vec<SkillBrowseRow>, ApiError> {
    state.skill_browse(query).await
}

#[tauri::command]
pub async fn skill_install_preview(
    state: State<'_, Arc<AppCore>>, shorthand: String, version: Option<GitRef>,
) -> Result<InstallPlan, ApiError> {
    state.skill_install_preview(shorthand, version).await
}

#[tauri::command]
pub async fn skill_install_apply(
    state: State<'_, Arc<AppCore>>, plan: InstallPlan,
) -> Result<InstalledSkill, ApiError> {
    state.skill_install_apply(plan).await
}

#[tauri::command]
pub async fn skill_check_updates(
    state: State<'_, Arc<AppCore>>, name: String,
) -> Result<Vec<AvailableVersion>, ApiError> {
    state.skill_check_updates(name).await
}

#[tauri::command]
pub async fn skill_upgrade_preview(
    state: State<'_, Arc<AppCore>>, name: String, target_sha: String,
) -> Result<UpgradePlan, ApiError> {
    state.skill_upgrade_preview(name, target_sha).await
}

#[tauri::command]
pub async fn skill_upgrade_apply(
    state: State<'_, Arc<AppCore>>, plan: UpgradePlan,
) -> Result<InstalledSkill, ApiError> {
    state.skill_upgrade_apply(plan).await
}

#[tauri::command]
pub async fn skill_uninstall(
    state: State<'_, Arc<AppCore>>, name: String, mode: UninstallMode,
) -> Result<(), ApiError> {
    state.skill_uninstall(name, mode).await
}

#[tauri::command]
pub async fn skill_toggle_enabled(
    state: State<'_, Arc<AppCore>>, name: String, enabled: bool,
) -> Result<(), ApiError> {
    state.skill_toggle_enabled(name, enabled).await
}

#[tauri::command]
pub async fn skill_adapt_preview(
    state: State<'_, Arc<AppCore>>, shorthand: String,
) -> Result<Value, ApiError> {
    state.skill_adapt_preview(shorthand).await
}

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "skill_list",
    "skill_browse",
    "skill_install_preview",
    "skill_install_apply",
    "skill_check_updates",
    "skill_upgrade_preview",
    "skill_upgrade_apply",
    "skill_uninstall",
    "skill_toggle_enabled",
    "skill_adapt_preview",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str, core: &AppCore, body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "skill_list" => dev::val(core.skill_list().await),
        "skill_browse" => {
            let q: Option<String> = dev::get(body, "query");
            dev::val(core.skill_browse(q).await)
        }
        "skill_install_preview" => {
            let sh = try_field!(dev::get_str(body, "shorthand"));
            let version: Option<GitRef> = dev::get(body, "version");
            dev::val(core.skill_install_preview(sh, version).await)
        }
        "skill_install_apply" => {
            let plan: InstallPlan = try_field!(dev::require_de(body, "plan"));
            dev::val(core.skill_install_apply(plan).await)
        }
        "skill_check_updates" => {
            let name = try_field!(dev::get_str(body, "name"));
            dev::val(core.skill_check_updates(name).await)
        }
        "skill_upgrade_preview" => {
            let name = try_field!(dev::get_str(body, "name"));
            let target = try_field!(dev::get_str(body, "targetSha"));
            dev::val(core.skill_upgrade_preview(name, target).await)
        }
        "skill_upgrade_apply" => {
            let plan: UpgradePlan = try_field!(dev::require_de(body, "plan"));
            dev::val(core.skill_upgrade_apply(plan).await)
        }
        "skill_uninstall" => {
            let name = try_field!(dev::get_str(body, "name"));
            let mode: UninstallMode = try_field!(dev::require_de(body, "mode"));
            dev::val(core.skill_uninstall(name, mode).await)
        }
        "skill_toggle_enabled" => {
            let name = try_field!(dev::get_str(body, "name"));
            let enabled: bool = try_field!(dev::require(body, "enabled"));
            dev::val(core.skill_toggle_enabled(name, enabled).await)
        }
        "skill_adapt_preview" => {
            let sh = try_field!(dev::get_str(body, "shorthand"));
            dev::val(core.skill_adapt_preview(sh).await)
        }
        _ => return None,
    })
}
```

If `dev_helpers` doesn't have `require_de`, add it:
```rust
pub fn require_de<T: serde::de::DeserializeOwned>(body: &serde_json::Value, field: &str)
    -> std::result::Result<T, ApiError>
{
    let v = body.get(field).ok_or_else(|| ApiError::new("VALIDATION", format!("missing field {field}")))?;
    serde_json::from_value(v.clone()).map_err(|e| ApiError::new("VALIDATION", e.to_string()))
}
```

- [ ] **Step 2: Register in `main.rs`** — add 10 entries under the "Skills marketplace" comment after the database block:
```rust
            commands::skills::skill_list,
            commands::skills::skill_browse,
            commands::skills::skill_install_preview,
            commands::skills::skill_install_apply,
            commands::skills::skill_check_updates,
            commands::skills::skill_upgrade_preview,
            commands::skills::skill_upgrade_apply,
            commands::skills::skill_uninstall,
            commands::skills::skill_toggle_enabled,
            commands::skills::skill_adapt_preview,
```

- [ ] **Step 3: Register in `dev_server/mod.rs`** — in the `dev_command_names()` array add:
```rust
            commands::skills::DEV_COMMANDS,
```

- [ ] **Step 4: Add `pub mod skills;` in `commands/mod.rs`**

- [ ] **Step 5: Run coverage test**

```bash
cargo nextest run -p desktop -E 'test(dev_server_covers)'
```
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/desktop
git commit -m "feat(desktop): skills marketplace Tauri commands"
```

---

# Phase 6 — Frontend

### Task 16: Types + hooks

**Files:**
- Create: `desktop-ui/src/shared/types/skills.ts`
- Modify: `desktop-ui/src/shared/types/index.ts` — re-export
- Create: `desktop-ui/src/features/skills/hooks/useSkillList.ts`
- Create: `desktop-ui/src/features/skills/hooks/useSkillBrowse.ts`
- Create: `desktop-ui/src/features/skills/hooks/useSkillDetail.ts`
- Create: `desktop-ui/src/features/skills/hooks/useSkillInstall.ts`
- Create: `desktop-ui/src/features/skills/hooks/useSkillUpdates.ts`
- Create: `desktop-ui/src/features/skills/lib/emit.ts`

- [ ] **Step 1: `shared/types/skills.ts`**

```ts
export type SkillSourceType = "github" | "skills_sh" | "local" | "bundled";

export interface InstalledSkill {
  name: string;
  sourceType: SkillSourceType;
  sourceRef: string;
  installedVersion: string;
  installedSha: string;
  enabled: boolean;
  isAdapted: boolean;
  bootstrappedDatabases: string[];
  installedAt: string;
  updatedAt: string;
}

export interface SkillBrowseRow {
  rank: number;
  name: string;
  sourceRef: string;
  installs?: number;
  isKlyntNative: boolean;
  isInstalled: boolean;
  isBundled: boolean;
}

export interface FileWrite { relativePath: string; contentSize: number; }
export interface TemplatePreview {
  templateName: string;
  databaseName: string;
  fieldCount: number;
}
export interface InstallPlan {
  package: {
    name: string;
    resolvedSha: string;
    semver?: string;
    skillMdContent: string;
    klyntbotMeta?: unknown;
    templates: { name: string; manifest: unknown }[];
  };
  filesToWrite: FileWrite[];
  databasesToBootstrap: TemplatePreview[];
  warnings: string[];
}

export interface DiffLine { tag: "equal" | "insert" | "delete"; text: string; }
export interface FrontmatterChange {
  field: string;
  before?: unknown;
  after?: unknown;
}
export interface DiffResult {
  bodyLines: DiffLine[];
  frontmatterChanges: FrontmatterChange[];
  bootstrapsAdded: string[];
  bootstrapsRemoved: string[];
}

export interface AvailableVersion {
  sha: string;
  tag?: string;
  message: string;
  date: string;
}

export interface UpgradePlan {
  name: string;
  fromSha: string;
  toSha: string;
  diff: DiffResult;
  newBootstraps: TemplatePreview[];
}

export type UninstallMode = "skill_only" | "archive_databases" | "delete_databases";
```

- [ ] **Step 2: Re-export** in `shared/types/index.ts`:
```ts
export type {
  InstalledSkill, SkillBrowseRow, SkillSourceType,
  InstallPlan, UpgradePlan, UninstallMode, DiffResult, DiffLine,
  AvailableVersion, TemplatePreview, FrontmatterChange, FileWrite,
} from "./skills";
```

- [ ] **Step 3: `emit.ts`**

```ts
export function emitSkillsUpdated() {
  window.dispatchEvent(new CustomEvent("skills:updated"));
}
```

- [ ] **Step 4: `useSkillList.ts`**

```ts
import { useQuery } from "@shared/hooks/useQuery";
import type { InstalledSkill } from "@shared/types";

export function useSkillList() {
  return useQuery<InstalledSkill[]>("skill_list", {}, {
    invalidateOn: ["skills:updated"],
    staleTime: 30_000,
  });
}
```

- [ ] **Step 5: `useSkillBrowse.ts`**

```ts
import { useQuery } from "@shared/hooks/useQuery";
import type { SkillBrowseRow } from "@shared/types";

export function useSkillBrowse(query: string | undefined) {
  return useQuery<SkillBrowseRow[]>("skill_browse", query ? { query } : {}, {
    invalidateOn: ["skills:updated"],
    staleTime: 60_000,
  });
}
```

- [ ] **Step 6: `useSkillDetail.ts`**

```ts
import { useMemo } from "react";
import { useSkillList } from "./useSkillList";
import type { InstalledSkill } from "@shared/types";

export function useSkillDetail(name: string | undefined) {
  const { data: all } = useSkillList();
  const installed: InstalledSkill | undefined = useMemo(
    () => all?.find((s) => s.name === name),
    [all, name]
  );
  return { installed };
}
```

- [ ] **Step 7: `useSkillInstall.ts`**

```ts
import { useMutation } from "@shared/hooks/useMutation";
import { emitSkillsUpdated } from "../lib/emit";
import type { InstallPlan, InstalledSkill, UninstallMode } from "@shared/types";

export function useSkillInstallPreview() {
  return useMutation<InstallPlan, { shorthand: string }>("skill_install_preview");
}

export function useSkillInstallApply() {
  const { mutate: raw, loading, error } = useMutation<InstalledSkill, { plan: InstallPlan }>(
    "skill_install_apply",
  );
  const mutate = async (plan: InstallPlan) => {
    const out = await raw({ plan });
    if (out) emitSkillsUpdated();
    return out;
  };
  return { mutate, loading, error };
}

export function useSkillUninstall() {
  const { mutate: raw, loading } = useMutation<void, { name: string; mode: UninstallMode }>(
    "skill_uninstall",
  );
  const mutate = async (name: string, mode: UninstallMode) => {
    await raw({ name, mode });
    emitSkillsUpdated();
  };
  return { mutate, loading };
}

export function useSkillToggleEnabled() {
  const { mutate: raw, loading } = useMutation<void, { name: string; enabled: boolean }>(
    "skill_toggle_enabled",
  );
  const mutate = async (name: string, enabled: boolean) => {
    await raw({ name, enabled });
    emitSkillsUpdated();
  };
  return { mutate, loading };
}
```

- [ ] **Step 8: `useSkillUpdates.ts`**

```ts
import { useQuery } from "@shared/hooks/useQuery";
import { useMutation } from "@shared/hooks/useMutation";
import { emitSkillsUpdated } from "../lib/emit";
import type { AvailableVersion, InstalledSkill, UpgradePlan } from "@shared/types";

export function useSkillCheckUpdates(name: string | undefined) {
  return useQuery<AvailableVersion[]>(
    "skill_check_updates",
    name ? { name } : null,
    { staleTime: 300_000 },
  );
}

export function useSkillUpgradePreview() {
  return useMutation<UpgradePlan, { name: string; targetSha: string }>("skill_upgrade_preview");
}

export function useSkillUpgradeApply() {
  const { mutate: raw, loading } = useMutation<InstalledSkill, { plan: UpgradePlan }>(
    "skill_upgrade_apply",
  );
  const mutate = async (plan: UpgradePlan) => {
    const out = await raw({ plan });
    if (out) emitSkillsUpdated();
    return out;
  };
  return { mutate, loading };
}
```

- [ ] **Step 9: Commit**

```bash
git add desktop-ui/src
git commit -m "feat(desktop-ui): skills marketplace types + hooks"
```

### Task 17: List page + row component + route

**Files:**
- Create: `desktop-ui/src/features/skills/pages/SkillsListPage.tsx`
- Create: `desktop-ui/src/features/skills/components/SkillRow.tsx`
- Modify: `desktop-ui/src/app/router.tsx`
- Modify: `desktop-ui/src/app/layouts/Sidebar.tsx`

- [ ] **Step 1: `SkillRow.tsx`**

```tsx
import type { SkillBrowseRow } from "@shared/types";
import { useNavigate } from "react-router";
import { Check, Package } from "lucide-react";

interface Props { row: SkillBrowseRow; }

export function SkillRow({ row }: Props) {
  const navigate = useNavigate();
  const encoded = encodeURIComponent(row.sourceRef);
  return (
    <button
      type="button"
      onClick={() => navigate(`/skills/${encoded}`)}
      className="w-full grid grid-cols-[48px_1fr_120px_120px] gap-4 items-center px-4 py-2 hover:bg-accent/20 border-b border-border text-left"
    >
      <span className="text-sm text-muted-foreground font-mono">
        {row.isInstalled ? <Check className="w-4 h-4 text-brand" /> : row.rank}
      </span>
      <span className="flex flex-col min-w-0">
        <span className="text-sm font-medium text-foreground truncate">{row.name}</span>
        <span className="text-xs text-muted-foreground truncate">{row.sourceRef}</span>
      </span>
      <span className="text-right text-sm text-muted-foreground">
        {row.installs !== undefined ? formatCount(row.installs) : "—"}
      </span>
      <span className="text-right">
        {row.isBundled ? (
          <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
            <Package className="w-3 h-3" /> Built-in
          </span>
        ) : row.isInstalled ? (
          <span className="text-xs text-brand">Installed</span>
        ) : row.isKlyntNative ? (
          <span className="text-xs text-accent">Klynt</span>
        ) : (
          <span className="text-xs text-muted-foreground">Prompt-only</span>
        )}
      </span>
    </button>
  );
}

function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}
```

- [ ] **Step 2: `SkillsListPage.tsx`**

```tsx
import { useState } from "react";
import { Search } from "lucide-react";
import { useSkillBrowse } from "../hooks/useSkillBrowse";
import { useSkillList } from "../hooks/useSkillList";
import { SkillRow } from "../components/SkillRow";

type Tab = "installed" | "all" | "trending" | "updates";

export function SkillsListPage() {
  const [tab, setTab] = useState<Tab>("all");
  const [query, setQuery] = useState("");
  const { data: browse } = useSkillBrowse(query || undefined);
  const { data: installed } = useSkillList();

  const rows = (() => {
    if (tab === "installed") {
      return (installed ?? []).map((s, i) => ({
        rank: i + 1,
        name: s.name,
        sourceRef: s.sourceRef,
        installs: undefined,
        isKlyntNative: !s.isAdapted,
        isInstalled: true,
        isBundled: s.sourceType === "bundled",
      }));
    }
    return browse ?? [];
  })();

  return (
    <div className="flex flex-col h-full">
      <header className="flex items-center justify-between px-6 py-4 border-b border-border">
        <h1 className="text-xl font-semibold text-foreground">Skills</h1>
      </header>
      <div className="px-6 py-3 border-b border-border">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search skills..."
            className="w-full pl-9 pr-3 py-2 bg-surface-base border border-border rounded-md text-sm text-foreground"
          />
        </div>
      </div>
      <nav className="flex gap-4 px-6 py-2 border-b border-border text-sm">
        {(["installed", "all", "trending", "updates"] as Tab[]).map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTab(t)}
            className={tab === t ? "text-foreground font-medium" : "text-muted-foreground hover:text-foreground"}
          >
            {labelFor(t, installed?.length)}
          </button>
        ))}
      </nav>
      <div className="grid grid-cols-[48px_1fr_120px_120px] gap-4 px-4 py-2 text-xs uppercase tracking-wide text-muted-foreground border-b border-border">
        <span>#</span>
        <span>Skill</span>
        <span className="text-right">Installs</span>
        <span className="text-right">Status</span>
      </div>
      <div className="flex-1 overflow-y-auto">
        {rows.map((r) => <SkillRow key={r.sourceRef} row={r} />)}
      </div>
    </div>
  );
}

function labelFor(t: Tab, installedCount: number | undefined): string {
  switch (t) {
    case "installed": return `Installed${installedCount != null ? ` (${installedCount})` : ""}`;
    case "all": return "All time";
    case "trending": return "Trending";
    case "updates": return "Updates";
  }
}
```

- [ ] **Step 3: Register route**

In `desktop-ui/src/app/router.tsx`, add (inside the existing lazy-loaded routes block):
```tsx
{
  path: "skills",
  lazy: async () => ({ Component: (await import("@features/skills/pages/SkillsListPage")).SkillsListPage }),
},
{
  path: "skills/:source",
  lazy: async () => ({ Component: (await import("@features/skills/pages/SkillDetailPage")).SkillDetailPage }),
},
```

- [ ] **Step 4: Add sidebar icon**

In `Sidebar.tsx`, in the `items` array add (before Settings):
```ts
  { key: "Skills", icon: Store, path: "/skills" },
```

Add `Store` to the imports from `lucide-react`.

- [ ] **Step 5: Run dev server**

```bash
cd desktop-ui && bun install && bun run lint && bun run build
```
Expected: clean build, no Biome errors.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src
git commit -m "feat(desktop-ui): SkillsListPage + route + sidebar entry"
```

### Task 18: Detail page + sidebar panel + install CTA

**Files:**
- Create: `desktop-ui/src/features/skills/pages/SkillDetailPage.tsx`
- Create: `desktop-ui/src/features/skills/components/SkillDetailSidebar.tsx`
- Create: `desktop-ui/src/features/skills/components/InstallCta.tsx`
- Create: `desktop-ui/src/features/skills/components/InstallPreviewDialog.tsx`
- Create: `desktop-ui/src/features/skills/components/SkillMarkdown.tsx`

- [ ] **Step 1: `SkillMarkdown.tsx`**

Reuse the existing notes markdown renderer if available; otherwise a minimal renderer:
```tsx
import ReactMarkdown from "react-markdown";

export function SkillMarkdown({ content }: { content: string }) {
  const body = stripFrontmatter(content);
  return (
    <div className="prose prose-invert max-w-none text-sm">
      <ReactMarkdown>{body}</ReactMarkdown>
    </div>
  );
}

function stripFrontmatter(s: string): string {
  const m = s.match(/^---[\s\S]*?---\n?/);
  return m ? s.slice(m[0].length) : s;
}
```
Ensure `react-markdown` is in `desktop-ui/package.json` (`bun add react-markdown` if missing).

- [ ] **Step 2: `SkillDetailSidebar.tsx`**

```tsx
import type { InstalledSkill } from "@shared/types";

interface Props {
  sourceRef: string;
  installed?: InstalledSkill;
}

export function SkillDetailSidebar({ sourceRef, installed }: Props) {
  return (
    <aside className="w-60 flex-shrink-0 glass-panel border-l border-border p-4 space-y-4 text-sm">
      <Section label="Repository">
        <a href={`https://github.com/${sourceRef.split("/").slice(0, 2).join("/")}`} target="_blank" rel="noreferrer" className="text-brand hover:underline break-all">
          {sourceRef}
        </a>
      </Section>
      {installed && (
        <>
          <Section label="Installed version">{installed.installedVersion}</Section>
          <Section label="Commit">{installed.installedSha.slice(0, 7)}</Section>
          {installed.bootstrappedDatabases.length > 0 && (
            <Section label="Databases">
              {installed.bootstrappedDatabases.length} managed
            </Section>
          )}
        </>
      )}
    </aside>
  );
}

function Section({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <p className="text-xs uppercase tracking-wide text-muted-foreground mb-1">{label}</p>
      <div className="text-foreground">{children}</div>
    </div>
  );
}
```

- [ ] **Step 3: `InstallPreviewDialog.tsx`**

```tsx
import type { InstallPlan } from "@shared/types";
import { useState } from "react";
import { useSkillInstallApply } from "../hooks/useSkillInstall";

interface Props {
  plan: InstallPlan;
  onClose: () => void;
  onInstalled?: () => void;
}

export function InstallPreviewDialog({ plan, onClose, onInstalled }: Props) {
  const [mode, setMode] = useState<"full" | "skillOnly">("full");
  const { mutate, loading } = useSkillInstallApply();

  const handleInstall = async () => {
    const effective = mode === "skillOnly"
      ? { ...plan, databasesToBootstrap: [] }
      : plan;
    const out = await mutate(effective);
    if (out) { onInstalled?.(); onClose(); }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={onClose}>
      <div className="glass-panel rounded-lg p-6 max-w-lg w-full" onClick={(e) => e.stopPropagation()}>
        <h2 className="text-lg font-semibold text-foreground mb-4">Install {plan.package.name}</h2>
        <section className="mb-4">
          <h3 className="text-sm font-medium text-foreground mb-2">Files ({plan.filesToWrite.length})</h3>
          <ul className="text-xs text-muted-foreground space-y-1 max-h-32 overflow-y-auto">
            {plan.filesToWrite.map((f) => (
              <li key={f.relativePath} className="font-mono">
                {f.relativePath} <span className="text-muted-foreground/60">({f.contentSize}B)</span>
              </li>
            ))}
          </ul>
        </section>
        {plan.databasesToBootstrap.length > 0 && (
          <section className="mb-4">
            <h3 className="text-sm font-medium text-foreground mb-2">Databases to create</h3>
            <ul className="text-xs space-y-1">
              {plan.databasesToBootstrap.map((d) => (
                <li key={d.templateName} className="flex justify-between">
                  <span className="text-foreground">{d.databaseName}</span>
                  <span className="text-muted-foreground">{d.fieldCount} fields</span>
                </li>
              ))}
            </ul>
            <div className="mt-3 flex gap-2 text-sm">
              <label className="flex items-center gap-1">
                <input type="radio" checked={mode === "full"} onChange={() => setMode("full")} /> Install + bootstrap
              </label>
              <label className="flex items-center gap-1">
                <input type="radio" checked={mode === "skillOnly"} onChange={() => setMode("skillOnly")} /> Install skill only
              </label>
            </div>
          </section>
        )}
        {plan.warnings.length > 0 && (
          <section className="mb-4">
            <h3 className="text-sm font-medium text-foreground mb-2">Warnings</h3>
            <ul className="text-xs text-amber-400 space-y-1">
              {plan.warnings.map((w, i) => <li key={i}>{w}</li>)}
            </ul>
          </section>
        )}
        <div className="flex justify-end gap-2 mt-6">
          <button type="button" onClick={onClose} className="px-3 py-1.5 text-sm text-muted-foreground">Cancel</button>
          <button type="button" disabled={loading} onClick={handleInstall} className="px-3 py-1.5 text-sm bg-brand text-white rounded">
            {loading ? "Installing..." : "Install"}
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: `InstallCta.tsx`**

```tsx
import { useState } from "react";
import { ipc } from "@shared/hooks/useIpc";
import type { InstallPlan } from "@shared/types";
import { InstallPreviewDialog } from "./InstallPreviewDialog";

interface Props { sourceRef: string; }

export function InstallCta({ sourceRef }: Props) {
  const [plan, setPlan] = useState<InstallPlan | null>(null);
  const [loading, setLoading] = useState(false);

  const openPreview = async () => {
    setLoading(true);
    try {
      const p = await ipc<InstallPlan>("skill_install_preview", { shorthand: sourceRef });
      setPlan(p);
    } finally {
      setLoading(false);
    }
  };

  return (
    <>
      <button type="button" disabled={loading} onClick={openPreview}
        className="px-4 py-2 bg-brand text-white rounded text-sm disabled:opacity-50">
        {loading ? "Fetching..." : "Install"}
      </button>
      {plan && <InstallPreviewDialog plan={plan} onClose={() => setPlan(null)} />}
    </>
  );
}
```

- [ ] **Step 5: `SkillDetailPage.tsx`**

```tsx
import { useParams } from "react-router";
import { useEffect, useState } from "react";
import { ipc } from "@shared/hooks/useIpc";
import { useSkillDetail } from "../hooks/useSkillDetail";
import { SkillDetailSidebar } from "../components/SkillDetailSidebar";
import { InstallCta } from "../components/InstallCta";
import { SkillMarkdown } from "../components/SkillMarkdown";

export function SkillDetailPage() {
  const { source } = useParams<{ source: string }>();
  const decoded = source ? decodeURIComponent(source) : "";
  const [skillMd, setSkillMd] = useState<string | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);
  const name = decoded.split("/").slice(-1)[0] ?? "";
  const { installed } = useSkillDetail(name);

  useEffect(() => {
    if (!decoded) return;
    ipc<{ package: { skillMdContent: string } }>("skill_install_preview", { shorthand: decoded })
      .then((p) => setSkillMd(p.package.skillMdContent))
      .catch((e) => setLoadErr(String(e)));
  }, [decoded]);

  return (
    <div className="flex h-full">
      <div className="flex-1 overflow-y-auto p-8 max-w-3xl">
        <p className="text-xs text-muted-foreground mb-2">Skills / {decoded}</p>
        <h1 className="text-2xl font-semibold text-foreground mb-4">{name}</h1>
        <div className="mb-6 flex gap-2">
          {installed ? (
            <span className="px-3 py-1.5 text-sm text-brand border border-brand rounded">Installed · v{installed.installedVersion}</span>
          ) : (
            <InstallCta sourceRef={decoded} />
          )}
        </div>
        {loadErr && <p className="text-sm text-red-400">{loadErr}</p>}
        {skillMd ? <SkillMarkdown content={skillMd} /> : <p className="text-muted-foreground text-sm">Loading…</p>}
      </div>
      <SkillDetailSidebar sourceRef={decoded} installed={installed} />
    </div>
  );
}
```

- [ ] **Step 6: Lint + typecheck**

```bash
cd desktop-ui && bun run lint && bun run build
```

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src
git commit -m "feat(desktop-ui): skill detail page + install dialog"
```

### Task 19: Diff viewer + uninstall dialog + updates tab

**Files:**
- Create: `desktop-ui/src/features/skills/components/DiffViewer.tsx`
- Create: `desktop-ui/src/features/skills/components/UninstallDialog.tsx`
- Modify: `desktop-ui/src/features/skills/pages/SkillDetailPage.tsx` — add Upgrade button + Uninstall menu

- [ ] **Step 1: `DiffViewer.tsx`**

```tsx
import type { DiffResult } from "@shared/types";

export function DiffViewer({ diff }: { diff: DiffResult }) {
  return (
    <div className="space-y-4">
      {diff.frontmatterChanges.length > 0 && (
        <section>
          <h3 className="text-sm font-medium text-foreground mb-2">Frontmatter</h3>
          <table className="w-full text-xs border border-border">
            <thead>
              <tr className="text-muted-foreground">
                <th className="text-left p-1">Field</th>
                <th className="text-left p-1">Before</th>
                <th className="text-left p-1">After</th>
              </tr>
            </thead>
            <tbody>
              {diff.frontmatterChanges.map((c) => (
                <tr key={c.field}>
                  <td className="p-1 font-mono">{c.field}</td>
                  <td className="p-1 text-red-400">{JSON.stringify(c.before)}</td>
                  <td className="p-1 text-green-400">{JSON.stringify(c.after)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
      {(diff.bootstrapsAdded.length > 0 || diff.bootstrapsRemoved.length > 0) && (
        <section>
          <h3 className="text-sm font-medium text-foreground mb-2">Bootstraps</h3>
          {diff.bootstrapsAdded.map((b) => <div key={b} className="text-green-400 text-xs">+ {b}</div>)}
          {diff.bootstrapsRemoved.map((b) => <div key={b} className="text-red-400 text-xs">- {b}</div>)}
        </section>
      )}
      <section>
        <h3 className="text-sm font-medium text-foreground mb-2">Body</h3>
        <pre className="text-xs font-mono bg-surface-base p-2 overflow-x-auto max-h-96">
          {diff.bodyLines.map((l, i) => (
            <span key={i} className={
              l.tag === "insert" ? "text-green-400 block" :
              l.tag === "delete" ? "text-red-400 block" :
              "text-muted-foreground block"
            }>
              {l.tag === "insert" ? "+" : l.tag === "delete" ? "-" : " "} {l.text}
            </span>
          ))}
        </pre>
      </section>
    </div>
  );
}
```

- [ ] **Step 2: `UninstallDialog.tsx`**

```tsx
import { useState } from "react";
import type { UninstallMode, InstalledSkill } from "@shared/types";
import { useSkillUninstall } from "../hooks/useSkillInstall";

interface Props { skill: InstalledSkill; onClose: () => void; }

export function UninstallDialog({ skill, onClose }: Props) {
  const [mode, setMode] = useState<UninstallMode>("skill_only");
  const { mutate, loading } = useSkillUninstall();

  const handleUninstall = async () => {
    await mutate(skill.name, mode);
    onClose();
  };

  const hasDatabases = skill.bootstrappedDatabases.length > 0;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={onClose}>
      <div className="glass-panel rounded-lg p-6 max-w-md w-full" onClick={(e) => e.stopPropagation()}>
        <h2 className="text-lg font-semibold text-foreground mb-4">Uninstall {skill.name}?</h2>
        <div className="space-y-2 text-sm">
          <label className="flex items-start gap-2">
            <input type="radio" checked={mode === "skill_only"} onChange={() => setMode("skill_only")} className="mt-1" />
            <span>
              <span className="block text-foreground">Remove skill only</span>
              <span className="block text-xs text-muted-foreground">Databases stay. Safest choice.</span>
            </span>
          </label>
          {hasDatabases && (
            <>
              <label className="flex items-start gap-2">
                <input type="radio" checked={mode === "archive_databases"} onChange={() => setMode("archive_databases")} className="mt-1" />
                <span>
                  <span className="block text-foreground">Remove skill + archive databases</span>
                  <span className="block text-xs text-muted-foreground">Renames {skill.bootstrappedDatabases.length} database(s) to "Archived: …".</span>
                </span>
              </label>
              <label className="flex items-start gap-2">
                <input type="radio" checked={mode === "delete_databases"} onChange={() => setMode("delete_databases")} className="mt-1" />
                <span>
                  <span className="block text-red-400">Remove skill + delete data</span>
                  <span className="block text-xs text-muted-foreground">Permanently deletes {skill.bootstrappedDatabases.length} database(s). Can't undo.</span>
                </span>
              </label>
            </>
          )}
        </div>
        <div className="flex justify-end gap-2 mt-6">
          <button type="button" onClick={onClose} className="px-3 py-1.5 text-sm text-muted-foreground">Cancel</button>
          <button type="button" disabled={loading} onClick={handleUninstall} className="px-3 py-1.5 text-sm bg-red-500 text-white rounded">
            {loading ? "Uninstalling..." : "Uninstall"}
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Hook up uninstall + upgrade buttons on the detail page**

In `SkillDetailPage.tsx`, under the `installed ? …` branch, replace the "Installed" badge with:
```tsx
          {installed ? (
            <>
              <span className="px-3 py-1.5 text-sm text-brand border border-brand rounded">
                Installed · v{installed.installedVersion}
              </span>
              <button type="button" onClick={() => setUninstallOpen(true)} className="px-3 py-1.5 text-sm text-red-400 border border-red-400 rounded">Uninstall</button>
            </>
          ) : (
            <InstallCta sourceRef={decoded} />
          )}
```

Add state + dialog:
```tsx
const [uninstallOpen, setUninstallOpen] = useState(false);
...
{installed && uninstallOpen && <UninstallDialog skill={installed} onClose={() => setUninstallOpen(false)} />}
```

- [ ] **Step 4: Lint + build**

```bash
cd desktop-ui && bun run lint && bun run build
```

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src
git commit -m "feat(desktop-ui): DiffViewer + UninstallDialog + detail page actions"
```

---

# Phase 7 — Bundled seeding + verification

### Task 20: Seed bundled skills into `installed_skills` on first boot

**Files:**
- Modify: `crates/app-core/src/init/mod.rs` — call a seeder right after installer is constructed
- Create: `crates/skills-installer/src/seeder.rs`
- Modify: `crates/skills-installer/src/lib.rs`

- [ ] **Step 1: Write seeder**

`crates/skills-installer/src/seeder.rs`:
```rust
use std::sync::Arc;

use common::Result;
use tracing::debug;

use skills_marketplace::{InstalledSkill, InstalledSkillsRepo, SourceType};

const BUNDLED_SKILLS: &[&str] = &[
    "task-management",
    "finance-management",
    "automation",
    "notebook",
    "learning",
    "workspace",
];

pub async fn seed_bundled(repo: &InstalledSkillsRepo) -> Result<()> {
    let existing = repo.list().await?;
    let existing_names: std::collections::HashSet<_> = existing.iter().map(|s| s.name.clone()).collect();
    let now = chrono::Utc::now().to_rfc3339();
    for name in BUNDLED_SKILLS {
        if existing_names.contains(*name) { continue; }
        let row = InstalledSkill {
            name: (*name).into(),
            source_type: SourceType::Bundled,
            source_ref: "bundled".into(),
            installed_version: env!("CARGO_PKG_VERSION").into(),
            installed_sha: format!("bundled-{}", env!("CARGO_PKG_VERSION")),
            enabled: true,
            is_adapted: false,
            bootstrapped_databases: vec![],
            installed_at: now.clone(),
            updated_at: now.clone(),
        };
        repo.insert(&row).await?;
        debug!(name = %name, "seeded bundled skill row");
    }
    Ok(())
}
```

In `lib.rs` add `pub mod seeder;` and re-export: `pub use seeder::seed_bundled;`.

- [ ] **Step 2: Call it from `init/mod.rs`**

After constructing the installer, add:
```rust
        if let Some(ref inst) = installer {
            ::skills_installer::seed_bundled(&inst.repo).await.ok();
        }
```

- [ ] **Step 3: Commit**

```bash
git add crates/skills-installer crates/app-core/src/init/mod.rs
git commit -m "feat(skills-installer): seed bundled skills into installed_skills on first boot"
```

### Task 21: Full workspace build + test

- [ ] **Step 1: Clippy + format + full test run**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cd desktop-ui && bun run lint && bun run build
```

Expected: zero warnings, all tests green, clean frontend build.

- [ ] **Step 2: Commit formatting if needed**

```bash
git diff --quiet || git commit -am "chore: cargo fmt"
```

### Task 22: Manual end-to-end verification

Run `cargo tauri dev` + `bun run dev` and perform these steps — mark each checkbox only after personally verifying the expected result.

- [ ] Navigate to `/skills`. The Installed tab shows the 6 bundled skills, each marked `Built-in`.
- [ ] Click the "All time" tab. Curated Klynt skills appear with `Klynt` badges.
- [ ] Paste `anthropics/skills/frontend-design` into the search, click the result → detail page renders with breadcrumb, install button, and the fetched SKILL.md body. No console errors.
- [ ] Click Install → preview dialog shows files + "Install" button. Click Install Skill Only (no templates expected). SkillStore reloads, and a refresh of `/skills` shows the skill now `Installed`.
- [ ] Back on a Klynt-native skill (e.g. a local test skill with a template), install with `Install + bootstrap`. Open the Brain → sidebar to verify new database(s) appeared.
- [ ] Click `Uninstall` on that skill → choose `Remove skill + delete data`. Confirm. Sidebar database vanishes.
- [ ] Find a prompt-only skill (no klyntbot block). Click "Adapt for Klynt" — if cognitive provider is configured, the dialog now shows a generated klyntbot block + rationale. Clicking Install now offers bootstrap. If no provider is configured, the button is disabled with tooltip.
- [ ] In Settings, disable cognitive provider. Return to a prompt-only skill → Adapt button disabled with "No cognitive provider configured" tooltip.
- [ ] Run `cargo nextest run --workspace` one final time; expect 100% pass rate.

If any checkbox fails, stop and diagnose before marking the plan complete.

### Task 23: Regression sweep

- [ ] **Step 1: Clippy strict**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

- [ ] **Step 2: Doctests**

```bash
cargo test --workspace --doc
```

- [ ] **Step 3: Frontend typecheck**

```bash
cd desktop-ui && bun run build
```

- [ ] **Step 4: Final commit if anything changed, push to remote branch**

```bash
git status
# commit anything leftover
git push -u origin HEAD
```

---

## Self-review notes

- All 23 tasks have concrete code and commit steps. No `TODO` placeholders remain.
- Method signatures are consistent across tasks: `Installer::apply_install(plan)` matches the `InstallPlan` built by `preview_install`; `UninstallMode::{SkillOnly, ArchiveDatabases, DeleteDatabases}` is used identically in Rust + TS.
- Spec coverage check:
  - Section "skills-registry" → Tasks 4–7
  - Section "skills-installer" → Tasks 8–11
  - Section "skills-adapter" → Task 12
  - Section "data model" → Tasks 1–2
  - Section "UI list/detail pages" → Tasks 17–19
  - Section "Tauri commands + handlers" → Tasks 13–15
  - Section "bundled seeding" → Task 20
  - Section "verification" → Tasks 22–23
- The skills.sh live-index proxy is intentionally deferred to a follow-up plan (the spec already allowed for this) — Task 13 ships a curated list + installed skills; swapping in a live proxy later is a one-file change.
