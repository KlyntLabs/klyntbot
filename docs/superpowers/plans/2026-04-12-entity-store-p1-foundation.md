# Entity Store — Plan 1: Foundation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `entity-store` and `database-tool` crates — the core engine for dynamic schema management, entity CRUD, query building, views, templates, and cross-database relations. Wire into app-core with Tauri commands and MCP exposure.

**Architecture:** `EntityStore` manages dynamic SQLite tables via `ALTER TABLE`. Each "database" is a registry entry in `databases` table with field definitions in `database_fields`. Entity data lives in real `db_{slug}` tables with typed columns. `DatabaseTool` exposes all operations to the AI agent. App-core provides thin Tauri command wrappers. Domain events flow through the existing `DomainEventBus`.

**Tech Stack:** Rust, sqlx (SQLite), serde/serde_json, async-trait, nanoid, tools-core macros

**Spec:** `docs/superpowers/specs/2026-04-12-flexible-database-engine-design.md` (Sections 3, 4.2-4.3, 10, 11)

---

## File Structure

### New Files

| File | Responsibility |
|------|----------------|
| `crates/entity-store/Cargo.toml` | Crate manifest — depends on storage, common, bus, serde, sqlx |
| `crates/entity-store/src/lib.rs` | Public API: re-exports EntityStore, types, query builder |
| `crates/entity-store/src/types.rs` | `FieldType`, `FieldDefinition`, `DatabaseSchema`, `Entity`, `ViewDefinition`, `ViewType`, `ViewConfig`, `FilterRule`, `SortRule` |
| `crates/entity-store/src/store.rs` | `EntityStore` — the central facade (create/get/update/delete databases, entities, fields, views) |
| `crates/entity-store/src/schema_ops.rs` | Schema DDL operations: CREATE TABLE, ALTER TABLE ADD/DROP COLUMN |
| `crates/entity-store/src/query.rs` | Dynamic SQL builder: SELECT with filters, sorts, pagination from schema metadata |
| `crates/entity-store/src/relations.rs` | Cross-database entity relation CRUD |
| `crates/entity-store/src/views.rs` | View and dashboard CRUD |
| `crates/entity-store/src/templates.rs` | Template manifest parsing and instantiation |
| `crates/entity-store/src/evolution.rs` | Schema evolution storage (proposals, autonomy thresholds) |
| `crates/entity-store/migrations/001_entity_store.sql` | Foundation tables: databases, database_fields, database_views, dashboards, entity_relations, schema_evolutions, schema_autonomy |
| `crates/database-tool/Cargo.toml` | Crate manifest — depends on entity-store, tools-core, tools-core-macros, common, bus |
| `crates/database-tool/src/lib.rs` | Public API: re-exports DatabaseTool, handler traits |
| `crates/database-tool/src/tool.rs` | `DatabaseTool` — Tool trait impl with 20+ actions |
| `crates/database-tool/src/actions/mod.rs` | Action module re-exports |
| `crates/database-tool/src/actions/database_ops.rs` | create_database, list_databases, get_schema, delete_database |
| `crates/database-tool/src/actions/entity_crud.rs` | create, get, list, update, delete entities |
| `crates/database-tool/src/actions/field_ops.rs` | add_field, remove_field, modify_field |
| `crates/database-tool/src/actions/search.rs` | Keyword + semantic search |
| `crates/database-tool/src/actions/relation_ops.rs` | link, unlink, list_relations |
| `crates/database-tool/src/actions/view_ops.rs` | create_view, update_view, delete_view, list_views |
| `crates/database-tool/src/handlers.rs` | Generic handler traits: EnrichmentHandler, EmbeddingHandler |

### Modified Files

| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | Add `entity-store` and `database-tool` to workspace members |
| `crates/bus/src/domain_events.rs` | Add entity lifecycle events (EntityCreated, EntityUpdated, EntityDeleted, etc.) and schema evolution events |
| `crates/app-core/Cargo.toml` | Add `entity-store` and `database-tool` dependencies |
| `crates/app-core/src/handlers/mod.rs` | Add `pub mod database;` |
| `crates/app-core/src/handlers/database/mod.rs` | Database handler functions (thin wrappers around EntityStore) |
| `crates/app-core/src/state.rs` | Add `entity_store: Arc<EntityStore>` to AppCore |
| `crates/app-core/src/init/mod.rs` | Initialize EntityStore, run migrations, install default templates |
| `crates/desktop/src/commands/database.rs` | Tauri `#[command]` functions for db_* operations |
| `crates/desktop/src/commands/mod.rs` | Add `pub mod database;` + DEV_COMMANDS |
| `crates/desktop/src/dev_server/mod.rs` | Add database commands to dev server routing |
| `crates/config/src/schema/mcp.rs` | Add "database" to `default_exposed_tools()` |
| `crates/agent/src/agent_loop/builder.rs` | Register DatabaseTool in ToolRegistry |
| `crates/storage/src/lib.rs` | Add entity-store FeatureMigration to migration list |
| `src/lib.rs` | Re-export entity-store types from facade |

---

## Task 1: Create entity-store crate scaffold

**Files:**
- Create: `crates/entity-store/Cargo.toml`
- Create: `crates/entity-store/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create crate directory and Cargo.toml**

```toml
# crates/entity-store/Cargo.toml
[package]
name = "entity-store"
version = "0.1.0"
edition = "2021"

[dependencies]
common = { path = "../common" }
storage = { path = "../storage" }
bus = { path = "../bus" }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sqlx = { workspace = true }
async-trait = { workspace = true }
chrono = { workspace = true }
tracing = { workspace = true }
nanoid = "0.4"
```

- [ ] **Step 2: Create lib.rs with module stubs**

```rust
// crates/entity-store/src/lib.rs
pub mod types;
pub mod store;
pub mod schema_ops;
pub mod query;
pub mod relations;
pub mod views;
pub mod templates;
pub mod evolution;

pub use store::EntityStore;
pub use types::*;

use storage::StoragePool;
use tools_core::FeatureMigration;

pub struct EntityStoreFeature;

impl EntityStoreFeature {
    pub fn migrations() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature: "entity_store",
            version: 1,
            sql: include_str!("../migrations/001_entity_store.sql"),
        }]
    }
}
```

- [ ] **Step 3: Add to workspace members**

In root `Cargo.toml`, add `"crates/entity-store"` to `[workspace] members` array.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p entity-store`
Expected: Compilation errors for missing modules (that's fine — we'll create them next)

- [ ] **Step 5: Commit**

```bash
git add crates/entity-store/ Cargo.toml
git commit -m "feat(entity-store): scaffold crate with Cargo.toml and lib.rs"
```

---

## Task 2: Define core types

**Files:**
- Create: `crates/entity-store/src/types.rs`

- [ ] **Step 1: Write type definitions**

```rust
// crates/entity-store/src/types.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported field types — maps to Notion property types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Number,
    Select,
    MultiSelect,
    Date,
    Checkbox,
    Url,
    Email,
    Phone,
    Relation,
    Rollup,
    Formula,
    CreatedTime,
    LastEdited,
    Files,
    Person,
}

impl FieldType {
    /// SQLite column type for this field.
    pub fn sqlite_type(&self) -> &'static str {
        match self {
            Self::Number => "REAL",
            Self::Checkbox => "INTEGER",
            _ => "TEXT",
        }
    }

    /// Whether this field type is user-editable (not computed).
    pub fn is_editable(&self) -> bool {
        !matches!(self, Self::Rollup | Self::Formula | Self::CreatedTime | Self::LastEdited)
    }
}

/// Definition of a single field in a database schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDefinition {
    pub id: String,
    pub database_id: String,
    pub name: String,
    pub slug: String,
    pub field_type: FieldType,
    #[serde(default)]
    pub options: Option<serde_json::Value>,
    pub position: i32,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub ai_managed: bool,
    #[serde(default)]
    pub ai_config: Option<serde_json::Value>,
    #[serde(default)]
    pub default_value: Option<String>,
    pub created_at: String,
}

/// Schema of a database — metadata + ordered field definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSchema {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub template_id: Option<String>,
    pub skill_id: Option<String>,
    pub fields: Vec<FieldDefinition>,
    pub views: Vec<ViewDefinition>,
    pub created_at: String,
    pub updated_at: String,
}

/// A single entity (row) in a database. Fields stored as slug→value map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub database_id: String,
    pub fields: HashMap<String, serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

/// View type — how to render a database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewType {
    Table,
    Board,
    Calendar,
    List,
    Gallery,
    Timeline,
}

/// A named view configuration for a database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewDefinition {
    pub id: String,
    pub database_id: String,
    pub name: String,
    pub view_type: ViewType,
    pub config: ViewConfig,
    pub position: i32,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// View-specific configuration: filters, sorts, visible fields, grouping.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewConfig {
    #[serde(default)]
    pub filters: Vec<FilterRule>,
    #[serde(default)]
    pub sorts: Vec<SortRule>,
    #[serde(default)]
    pub visible_fields: Vec<String>,
    pub group_by: Option<String>,
    pub calendar_field: Option<String>,
    pub gallery_field: Option<String>,
    #[serde(default)]
    pub card_fields: Vec<String>,
    #[serde(default)]
    pub layout: HashMap<String, serde_json::Value>,
}

/// A single filter condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterRule {
    pub field: String,
    pub op: FilterOp,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
    NotContains,
    IsEmpty,
    IsNotEmpty,
    In,
    NotIn,
}

/// A sort specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortRule {
    pub field: String,
    #[serde(default = "default_sort_dir")]
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    Desc,
}

fn default_sort_dir() -> SortDirection {
    SortDirection::Asc
}

/// Cross-database entity relation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityRelation {
    pub id: String,
    pub source_id: String,
    pub source_db_id: String,
    pub target_id: String,
    pub target_db_id: String,
    pub relation_type: String,
    pub inferred: bool,
    pub confidence: Option<f64>,
    pub created_at: String,
}

/// Dashboard with widgets querying any database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub id: String,
    pub name: String,
    pub widgets: Vec<WidgetDefinition>,
    pub position: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetDefinition {
    pub id: String,
    pub widget_type: String,
    pub database_id: String,
    pub config: serde_json::Value,
    pub position: GridPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridPosition {
    pub row: i32,
    pub col: i32,
    pub width: i32,
    pub height: i32,
}

/// Input for creating a new database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDatabaseInput {
    pub name: String,
    pub slug: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub template_id: Option<String>,
}

/// Input for creating a new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFieldInput {
    pub name: String,
    pub slug: Option<String>,
    pub field_type: FieldType,
    pub options: Option<serde_json::Value>,
    pub required: Option<bool>,
    pub default_value: Option<String>,
    pub position: Option<i32>,
}
```

- [ ] **Step 2: Write unit tests for FieldType**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_type_sqlite_mapping() {
        assert_eq!(FieldType::Text.sqlite_type(), "TEXT");
        assert_eq!(FieldType::Number.sqlite_type(), "REAL");
        assert_eq!(FieldType::Checkbox.sqlite_type(), "INTEGER");
        assert_eq!(FieldType::Date.sqlite_type(), "TEXT");
        assert_eq!(FieldType::Select.sqlite_type(), "TEXT");
    }

    #[test]
    fn field_type_editability() {
        assert!(FieldType::Text.is_editable());
        assert!(FieldType::Number.is_editable());
        assert!(!FieldType::Rollup.is_editable());
        assert!(!FieldType::Formula.is_editable());
        assert!(!FieldType::CreatedTime.is_editable());
        assert!(!FieldType::LastEdited.is_editable());
    }

    #[test]
    fn field_type_serde_roundtrip() {
        let ft = FieldType::MultiSelect;
        let json = serde_json::to_string(&ft).unwrap();
        assert_eq!(json, "\"multi_select\"");
        let parsed: FieldType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ft);
    }

    #[test]
    fn view_config_defaults() {
        let config: ViewConfig = serde_json::from_str("{}").unwrap();
        assert!(config.filters.is_empty());
        assert!(config.sorts.is_empty());
        assert!(config.visible_fields.is_empty());
        assert!(config.group_by.is_none());
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p entity-store`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/entity-store/src/types.rs
git commit -m "feat(entity-store): define core types — FieldType, Entity, DatabaseSchema, ViewDefinition"
```

---

## Task 3: Write migration SQL

**Files:**
- Create: `crates/entity-store/migrations/001_entity_store.sql`

- [ ] **Step 1: Write the migration**

```sql
-- crates/entity-store/migrations/001_entity_store.sql

-- Registry of all user databases
CREATE TABLE IF NOT EXISTS databases (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    slug         TEXT UNIQUE NOT NULL,
    icon         TEXT,
    description  TEXT,
    template_id  TEXT,
    skill_id     TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

-- Field definitions — the schema of each database
CREATE TABLE IF NOT EXISTS database_fields (
    id           TEXT PRIMARY KEY,
    database_id  TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    slug         TEXT NOT NULL,
    field_type   TEXT NOT NULL,
    options_json TEXT,
    position     INTEGER NOT NULL DEFAULT 0,
    required     INTEGER NOT NULL DEFAULT 0,
    hidden       INTEGER NOT NULL DEFAULT 0,
    ai_managed   INTEGER NOT NULL DEFAULT 0,
    ai_config    TEXT,
    default_value TEXT,
    created_at   TEXT NOT NULL,
    UNIQUE(database_id, slug)
);
CREATE INDEX IF NOT EXISTS idx_database_fields_db ON database_fields(database_id);

-- Views per database
CREATE TABLE IF NOT EXISTS database_views (
    id           TEXT PRIMARY KEY,
    database_id  TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    view_type    TEXT NOT NULL,
    config_json  TEXT NOT NULL DEFAULT '{}',
    position     INTEGER NOT NULL DEFAULT 0,
    is_default   INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_database_views_db ON database_views(database_id);

-- Custom dashboards
CREATE TABLE IF NOT EXISTS dashboards (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    config_json TEXT NOT NULL DEFAULT '{}',
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Cross-database entity relations
CREATE TABLE IF NOT EXISTS entity_relations (
    id            TEXT PRIMARY KEY,
    source_id     TEXT NOT NULL,
    source_db_id  TEXT NOT NULL,
    target_id     TEXT NOT NULL,
    target_db_id  TEXT NOT NULL,
    relation_type TEXT NOT NULL DEFAULT 'related',
    inferred      INTEGER NOT NULL DEFAULT 0,
    confidence    REAL,
    created_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entity_relations_source ON entity_relations(source_id, source_db_id);
CREATE INDEX IF NOT EXISTS idx_entity_relations_target ON entity_relations(target_id, target_db_id);

-- Schema evolution tracking
CREATE TABLE IF NOT EXISTS schema_evolutions (
    id            TEXT PRIMARY KEY,
    database_id   TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
    action_type   TEXT NOT NULL,
    action_json   TEXT NOT NULL,
    confidence    REAL NOT NULL,
    reasoning     TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'proposed',
    source        TEXT NOT NULL DEFAULT 'reforge',
    created_at    TEXT NOT NULL,
    resolved_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_schema_evolutions_db ON schema_evolutions(database_id, status);

-- Per-database AI autonomy calibration
CREATE TABLE IF NOT EXISTS schema_autonomy (
    database_id      TEXT PRIMARY KEY REFERENCES databases(id) ON DELETE CASCADE,
    auto_threshold   REAL NOT NULL DEFAULT 0.9,
    suggest_threshold REAL NOT NULL DEFAULT 0.6,
    acceptance_rate  REAL NOT NULL DEFAULT 0.5,
    total_proposed   INTEGER NOT NULL DEFAULT 0,
    total_accepted   INTEGER NOT NULL DEFAULT 0,
    updated_at       TEXT NOT NULL
);
```

- [ ] **Step 2: Verify SQL syntax**

Run: `sqlite3 :memory: < crates/entity-store/migrations/001_entity_store.sql && echo "OK"`
Expected: "OK" (no errors)

- [ ] **Step 3: Commit**

```bash
git add crates/entity-store/migrations/
git commit -m "feat(entity-store): add foundation migration — databases, fields, views, relations, evolution"
```

---

## Task 4: Implement schema DDL operations

**Files:**
- Create: `crates/entity-store/src/schema_ops.rs`

- [ ] **Step 1: Write failing tests for table creation and ALTER TABLE**

```rust
// crates/entity-store/src/schema_ops.rs
use common::Result;
use sqlx::SqlitePool;

use crate::types::{FieldDefinition, FieldType};

/// Create the dynamic table for a database.
pub async fn create_entity_table(pool: &SqlitePool, slug: &str) -> Result<()> {
    let table_name = format!("db_{slug}");
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS {table_name} (
            id         TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"
    );
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}

/// Add a column to a dynamic table.
pub async fn add_column(pool: &SqlitePool, db_slug: &str, field: &FieldDefinition) -> Result<()> {
    let table_name = format!("db_{db_slug}");
    let col_type = field.field_type.sqlite_type();
    let sql = format!(
        "ALTER TABLE {table_name} ADD COLUMN {} {col_type}",
        field.slug
    );
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}

/// Drop a column from a dynamic table (SQLite 3.35+).
pub async fn drop_column(pool: &SqlitePool, db_slug: &str, field_slug: &str) -> Result<()> {
    let table_name = format!("db_{db_slug}");
    let sql = format!("ALTER TABLE {table_name} DROP COLUMN {field_slug}");
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}

/// Drop the entire dynamic table for a database.
pub async fn drop_entity_table(pool: &SqlitePool, slug: &str) -> Result<()> {
    let table_name = format!("db_{slug}");
    let sql = format!("DROP TABLE IF EXISTS {table_name}");
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}

/// Check if a dynamic table exists.
pub async fn table_exists(pool: &SqlitePool, slug: &str) -> Result<bool> {
    let table_name = format!("db_{slug}");
    let row: (i32,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
    )
    .bind(&table_name)
    .fetch_one(pool)
    .await?;
    Ok(row.0 > 0)
}

/// Validate that a slug is safe for use as a SQL identifier.
/// Only allows lowercase alphanumeric + underscore, must start with letter.
pub fn validate_slug(slug: &str) -> Result<()> {
    if slug.is_empty() {
        return Err(common::KlyntbotError::Validation("slug cannot be empty".into()));
    }
    if !slug.chars().next().unwrap().is_ascii_lowercase() {
        return Err(common::KlyntbotError::Validation(
            "slug must start with a lowercase letter".into(),
        ));
    }
    if !slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(common::KlyntbotError::Validation(
            "slug must contain only lowercase letters, digits, and underscores".into(),
        ));
    }
    Ok(())
}

/// Generate a slug from a display name.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn test_pool() -> SqlitePool {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        pool.raw_pool().clone()
    }

    #[tokio::test]
    async fn create_and_check_table() {
        let pool = test_pool().await;
        assert!(!table_exists(&pool, "tasks").await.unwrap());
        create_entity_table(&pool, "tasks").await.unwrap();
        assert!(table_exists(&pool, "tasks").await.unwrap());
    }

    #[tokio::test]
    async fn add_and_drop_column() {
        let pool = test_pool().await;
        create_entity_table(&pool, "tasks").await.unwrap();

        let field = FieldDefinition {
            id: "f1".into(),
            database_id: "db1".into(),
            name: "Due Date".into(),
            slug: "due_date".into(),
            field_type: FieldType::Date,
            options: None,
            position: 0,
            required: false,
            hidden: false,
            ai_managed: false,
            ai_config: None,
            default_value: None,
            created_at: "2026-04-12T00:00:00Z".into(),
        };
        add_column(&pool, "tasks", &field).await.unwrap();

        // Verify column exists by inserting a row
        sqlx::query("INSERT INTO db_tasks (id, created_at, updated_at, due_date) VALUES ('e1', '2026-04-12', '2026-04-12', '2026-04-20')")
            .execute(&pool).await.unwrap();

        drop_column(&pool, "tasks", "due_date").await.unwrap();
    }

    #[tokio::test]
    async fn drop_table() {
        let pool = test_pool().await;
        create_entity_table(&pool, "temp").await.unwrap();
        assert!(table_exists(&pool, "temp").await.unwrap());
        drop_entity_table(&pool, "temp").await.unwrap();
        assert!(!table_exists(&pool, "temp").await.unwrap());
    }

    #[test]
    fn validate_slug_rules() {
        assert!(validate_slug("tasks").is_ok());
        assert!(validate_slug("my_database_2").is_ok());
        assert!(validate_slug("").is_err());
        assert!(validate_slug("2tasks").is_err());
        assert!(validate_slug("My Tasks").is_err());
        assert!(validate_slug("tasks!").is_err());
    }

    #[test]
    fn slugify_names() {
        assert_eq!(slugify("Job Applications"), "job_applications");
        assert_eq!(slugify("My Tasks!"), "my_tasks_");
        assert_eq!(slugify("Finance 2026"), "finance_2026");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p entity-store`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/entity-store/src/schema_ops.rs
git commit -m "feat(entity-store): schema DDL — create/drop tables, add/drop columns, slug validation"
```

---

## Task 5: Implement EntityStore core — database CRUD

**Files:**
- Create: `crates/entity-store/src/store.rs`

- [ ] **Step 1: Write the EntityStore struct with database create/get/list/delete**

```rust
// crates/entity-store/src/store.rs
use std::sync::Arc;

use chrono::Utc;
use common::Result;
use sqlx::SqlitePool;

use crate::schema_ops;
use crate::types::*;

/// Central facade for all entity store operations.
pub struct EntityStore {
    pool: SqlitePool,
    bus: Option<Arc<bus::DomainEventBus>>,
}

impl EntityStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool, bus: None }
    }

    pub fn with_event_bus(mut self, bus: Arc<bus::DomainEventBus>) -> Self {
        self.bus = Some(bus);
        self
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // ── Database CRUD ──

    /// Create a new database with its dynamic table.
    pub async fn create_database(&self, input: CreateDatabaseInput) -> Result<DatabaseSchema> {
        let id = nanoid::nanoid!(8);
        let slug = input
            .slug
            .unwrap_or_else(|| schema_ops::slugify(&input.name));
        schema_ops::validate_slug(&slug)?;

        let now = Utc::now().to_rfc3339();

        // Insert registry entry
        sqlx::query(
            "INSERT INTO databases (id, name, slug, icon, description, template_id, skill_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&slug)
        .bind(&input.icon)
        .bind(&input.description)
        .bind(&input.template_id)
        .bind::<Option<String>>(None) // skill_id set later when skill is generated
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        // Create dynamic table
        schema_ops::create_entity_table(&self.pool, &slug).await?;

        // Create default schema_autonomy entry
        sqlx::query(
            "INSERT INTO schema_autonomy (database_id, updated_at) VALUES (?, ?)",
        )
        .bind(&id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.get_database(&id).await
    }

    /// Get a database schema by ID.
    pub async fn get_database(&self, id: &str) -> Result<DatabaseSchema> {
        let row = sqlx::query_as::<_, DatabaseRow>(
            "SELECT * FROM databases WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| common::KlyntbotError::NotFound(format!("database {id}")))?;

        let fields = self.list_fields(id).await?;
        let views = self.list_views(id).await?;

        Ok(DatabaseSchema {
            id: row.id,
            name: row.name,
            slug: row.slug,
            icon: row.icon,
            description: row.description,
            template_id: row.template_id,
            skill_id: row.skill_id,
            fields,
            views,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// List all databases.
    pub async fn list_databases(&self) -> Result<Vec<DatabaseSchema>> {
        let rows = sqlx::query_as::<_, DatabaseRow>(
            "SELECT * FROM databases ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let fields = self.list_fields(&row.id).await?;
            let views = self.list_views(&row.id).await?;
            results.push(DatabaseSchema {
                id: row.id,
                name: row.name,
                slug: row.slug,
                icon: row.icon,
                description: row.description,
                template_id: row.template_id,
                skill_id: row.skill_id,
                fields,
                views,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });
        }
        Ok(results)
    }

    /// Delete a database and its dynamic table.
    pub async fn delete_database(&self, id: &str) -> Result<()> {
        let db = self.get_database(id).await?;
        schema_ops::drop_entity_table(&self.pool, &db.slug).await?;
        sqlx::query("DELETE FROM databases WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Field CRUD ──

    /// Add a field to a database (updates schema + ALTER TABLE).
    pub async fn add_field(
        &self,
        database_id: &str,
        input: CreateFieldInput,
    ) -> Result<FieldDefinition> {
        let db = self.get_database(database_id).await?;
        let field_id = nanoid::nanoid!(8);
        let slug = input
            .slug
            .unwrap_or_else(|| schema_ops::slugify(&input.name));
        schema_ops::validate_slug(&slug)?;

        let position = input.position.unwrap_or(db.fields.len() as i32);
        let now = Utc::now().to_rfc3339();

        let field = FieldDefinition {
            id: field_id.clone(),
            database_id: database_id.to_string(),
            name: input.name,
            slug: slug.clone(),
            field_type: input.field_type.clone(),
            options: input.options,
            position,
            required: input.required.unwrap_or(false),
            hidden: false,
            ai_managed: false,
            ai_config: None,
            default_value: input.default_value,
            created_at: now.clone(),
        };

        // Insert field definition
        sqlx::query(
            "INSERT INTO database_fields (id, database_id, name, slug, field_type, options_json, position, required, default_value, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&field.id)
        .bind(database_id)
        .bind(&field.name)
        .bind(&field.slug)
        .bind(serde_json::to_string(&field.field_type)?)
        .bind(field.options.as_ref().map(|o| o.to_string()))
        .bind(field.position)
        .bind(field.required)
        .bind(&field.default_value)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        // ALTER TABLE to add column
        if field.field_type.is_editable() {
            schema_ops::add_column(&self.pool, &db.slug, &field).await?;
        }

        Ok(field)
    }

    /// Remove a field from a database (hides by default, drops column if hard=true).
    pub async fn remove_field(
        &self,
        database_id: &str,
        field_id: &str,
        hard: bool,
    ) -> Result<()> {
        if hard {
            let field = self.get_field(field_id).await?;
            let db = self.get_database(database_id).await?;
            schema_ops::drop_column(&self.pool, &db.slug, &field.slug).await?;
            sqlx::query("DELETE FROM database_fields WHERE id = ?")
                .bind(field_id)
                .execute(&self.pool)
                .await?;
        } else {
            sqlx::query("UPDATE database_fields SET hidden = 1 WHERE id = ?")
                .bind(field_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Get a single field definition.
    async fn get_field(&self, field_id: &str) -> Result<FieldDefinition> {
        let row = sqlx::query_as::<_, FieldRow>(
            "SELECT * FROM database_fields WHERE id = ?",
        )
        .bind(field_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| common::KlyntbotError::NotFound(format!("field {field_id}")))?;
        Ok(field_from_row(row)?)
    }

    /// List all fields for a database (ordered by position).
    async fn list_fields(&self, database_id: &str) -> Result<Vec<FieldDefinition>> {
        let rows = sqlx::query_as::<_, FieldRow>(
            "SELECT * FROM database_fields WHERE database_id = ? ORDER BY position ASC",
        )
        .bind(database_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(field_from_row).collect()
    }

    /// List views for a database.
    pub async fn list_views(&self, database_id: &str) -> Result<Vec<ViewDefinition>> {
        let rows = sqlx::query_as::<_, ViewRow>(
            "SELECT * FROM database_views WHERE database_id = ? ORDER BY position ASC",
        )
        .bind(database_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(view_from_row).collect()
    }
}

// ── Row types ──

#[derive(sqlx::FromRow)]
struct DatabaseRow {
    id: String,
    name: String,
    slug: String,
    icon: Option<String>,
    description: Option<String>,
    template_id: Option<String>,
    skill_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(sqlx::FromRow)]
struct FieldRow {
    id: String,
    database_id: String,
    name: String,
    slug: String,
    field_type: String,
    options_json: Option<String>,
    position: i32,
    required: i32,
    hidden: i32,
    ai_managed: i32,
    ai_config: Option<String>,
    default_value: Option<String>,
    created_at: String,
}

fn field_from_row(row: FieldRow) -> Result<FieldDefinition> {
    let field_type: FieldType = serde_json::from_str(&row.field_type)?;
    Ok(FieldDefinition {
        id: row.id,
        database_id: row.database_id,
        name: row.name,
        slug: row.slug,
        field_type,
        options: row.options_json.and_then(|s| serde_json::from_str(&s).ok()),
        position: row.position,
        required: row.required != 0,
        hidden: row.hidden != 0,
        ai_managed: row.ai_managed != 0,
        ai_config: row.ai_config.and_then(|s| serde_json::from_str(&s).ok()),
        default_value: row.default_value,
        created_at: row.created_at,
    })
}

#[derive(sqlx::FromRow)]
struct ViewRow {
    id: String,
    database_id: String,
    name: String,
    view_type: String,
    config_json: String,
    position: i32,
    is_default: i32,
    created_at: String,
    updated_at: String,
}

fn view_from_row(row: ViewRow) -> Result<ViewDefinition> {
    let view_type: ViewType = serde_json::from_str(&format!("\"{}\"", row.view_type))?;
    let config: ViewConfig = serde_json::from_str(&row.config_json).unwrap_or_default();
    Ok(ViewDefinition {
        id: row.id,
        database_id: row.database_id,
        name: row.name,
        view_type,
        config,
        position: row.position,
        is_default: row.is_default != 0,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> EntityStore {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // Run migration
        sqlx::query(include_str!("../migrations/001_entity_store.sql"))
            .execute(pool.raw_pool())
            .await
            .unwrap();
        EntityStore::new(pool.raw_pool().clone())
    }

    #[tokio::test]
    async fn create_and_get_database() {
        let store = setup().await;
        let db = store
            .create_database(CreateDatabaseInput {
                name: "Tasks".into(),
                slug: None,
                icon: Some("✓".into()),
                description: Some("My tasks".into()),
                template_id: None,
            })
            .await
            .unwrap();

        assert_eq!(db.name, "Tasks");
        assert_eq!(db.slug, "tasks");
        assert_eq!(db.icon.as_deref(), Some("✓"));
        assert!(db.fields.is_empty());
        assert!(schema_ops::table_exists(store.pool(), "tasks").await.unwrap());

        let fetched = store.get_database(&db.id).await.unwrap();
        assert_eq!(fetched.name, "Tasks");
    }

    #[tokio::test]
    async fn list_databases() {
        let store = setup().await;
        store.create_database(CreateDatabaseInput {
            name: "Tasks".into(), slug: None, icon: None, description: None, template_id: None,
        }).await.unwrap();
        store.create_database(CreateDatabaseInput {
            name: "Finance".into(), slug: None, icon: None, description: None, template_id: None,
        }).await.unwrap();

        let dbs = store.list_databases().await.unwrap();
        assert_eq!(dbs.len(), 2);
    }

    #[tokio::test]
    async fn delete_database() {
        let store = setup().await;
        let db = store.create_database(CreateDatabaseInput {
            name: "Temp".into(), slug: None, icon: None, description: None, template_id: None,
        }).await.unwrap();

        store.delete_database(&db.id).await.unwrap();
        assert!(store.get_database(&db.id).await.is_err());
        assert!(!schema_ops::table_exists(store.pool(), "temp").await.unwrap());
    }

    #[tokio::test]
    async fn add_field_creates_column() {
        let store = setup().await;
        let db = store.create_database(CreateDatabaseInput {
            name: "Tasks".into(), slug: None, icon: None, description: None, template_id: None,
        }).await.unwrap();

        let field = store.add_field(&db.id, CreateFieldInput {
            name: "Due Date".into(),
            slug: None,
            field_type: FieldType::Date,
            options: None,
            required: None,
            default_value: None,
            position: None,
        }).await.unwrap();

        assert_eq!(field.slug, "due_date");
        assert_eq!(field.field_type, FieldType::Date);

        // Verify column exists by inserting
        sqlx::query("INSERT INTO db_tasks (id, created_at, updated_at, due_date) VALUES ('e1', '2026-04-12', '2026-04-12', '2026-05-01')")
            .execute(store.pool()).await.unwrap();

        let schema = store.get_database(&db.id).await.unwrap();
        assert_eq!(schema.fields.len(), 1);
        assert_eq!(schema.fields[0].slug, "due_date");
    }

    #[tokio::test]
    async fn remove_field_soft_hides() {
        let store = setup().await;
        let db = store.create_database(CreateDatabaseInput {
            name: "Tasks".into(), slug: None, icon: None, description: None, template_id: None,
        }).await.unwrap();

        let field = store.add_field(&db.id, CreateFieldInput {
            name: "Notes".into(), slug: None, field_type: FieldType::Text,
            options: None, required: None, default_value: None, position: None,
        }).await.unwrap();

        store.remove_field(&db.id, &field.id, false).await.unwrap();

        let schema = store.get_database(&db.id).await.unwrap();
        assert!(schema.fields[0].hidden);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p entity-store`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/entity-store/src/store.rs
git commit -m "feat(entity-store): EntityStore core — database + field CRUD with dynamic tables"
```

---

## Task 6: Implement entity CRUD on dynamic tables

**Files:**
- Modify: `crates/entity-store/src/store.rs`

- [ ] **Step 1: Add entity create/get/update/delete methods to EntityStore**

Add these methods to the `impl EntityStore` block in `store.rs`:

```rust
    // ── Entity CRUD ──

    /// Create an entity in a database.
    pub async fn create_entity(
        &self,
        database_id: &str,
        fields: HashMap<String, serde_json::Value>,
    ) -> Result<Entity> {
        let db = self.get_database(database_id).await?;
        let entity_id = nanoid::nanoid!(8);
        let now = Utc::now().to_rfc3339();

        let schema_fields = &db.fields;
        let table_name = format!("db_{}", db.slug);

        // Build dynamic INSERT
        let mut col_names = vec!["id".to_string(), "created_at".to_string(), "updated_at".to_string()];
        let mut placeholders = vec!["?".to_string(); 3];
        let mut values: Vec<String> = vec![entity_id.clone(), now.clone(), now.clone()];

        for sf in schema_fields {
            if sf.hidden || !sf.field_type.is_editable() {
                continue;
            }
            if let Some(val) = fields.get(&sf.slug) {
                col_names.push(sf.slug.clone());
                placeholders.push("?".into());
                values.push(json_value_to_sql(val));
            } else if let Some(ref default) = sf.default_value {
                col_names.push(sf.slug.clone());
                placeholders.push("?".into());
                values.push(default.clone());
            }
        }

        let sql = format!(
            "INSERT INTO {table_name} ({}) VALUES ({})",
            col_names.join(", "),
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for v in &values {
            query = query.bind(v);
        }
        query.execute(&self.pool).await?;

        self.get_entity(database_id, &entity_id).await
    }

    /// Get a single entity by ID.
    pub async fn get_entity(&self, database_id: &str, entity_id: &str) -> Result<Entity> {
        let db = self.get_database(database_id).await?;
        let table_name = format!("db_{}", db.slug);

        let row = sqlx::query_as::<_, (String, String, String)>(
            &format!("SELECT id, created_at, updated_at FROM {table_name} WHERE id = ?"),
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| common::KlyntbotError::NotFound(format!("entity {entity_id}")))?;

        // Fetch all field values
        let mut fields = HashMap::new();
        for sf in &db.fields {
            if sf.hidden {
                continue;
            }
            let val: Option<String> = sqlx::query_scalar(
                &format!("SELECT {} FROM {table_name} WHERE id = ?", sf.slug),
            )
            .bind(entity_id)
            .fetch_optional(&self.pool)
            .await?
            .flatten();

            if let Some(v) = val {
                fields.insert(sf.slug.clone(), sql_to_json_value(&v, &sf.field_type));
            }
        }

        Ok(Entity {
            id: row.0,
            database_id: database_id.to_string(),
            fields,
            created_at: row.1,
            updated_at: row.2,
        })
    }

    /// Update an entity's fields.
    pub async fn update_entity(
        &self,
        database_id: &str,
        entity_id: &str,
        fields: HashMap<String, serde_json::Value>,
    ) -> Result<Entity> {
        let db = self.get_database(database_id).await?;
        let table_name = format!("db_{}", db.slug);
        let now = Utc::now().to_rfc3339();

        let valid_slugs: std::collections::HashSet<_> = db.fields.iter().map(|f| &f.slug).collect();

        let mut set_clauses = vec!["updated_at = ?".to_string()];
        let mut values = vec![now];

        for (slug, val) in &fields {
            if valid_slugs.contains(slug) {
                set_clauses.push(format!("{slug} = ?"));
                values.push(json_value_to_sql(val));
            }
        }

        values.push(entity_id.to_string());

        let sql = format!(
            "UPDATE {table_name} SET {} WHERE id = ?",
            set_clauses.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for v in &values {
            query = query.bind(v);
        }
        query.execute(&self.pool).await?;

        self.get_entity(database_id, entity_id).await
    }

    /// Delete an entity.
    pub async fn delete_entity(&self, database_id: &str, entity_id: &str) -> Result<()> {
        let db = self.get_database(database_id).await?;
        let table_name = format!("db_{}", db.slug);
        sqlx::query(&format!("DELETE FROM {table_name} WHERE id = ?"))
            .bind(entity_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
```

Add these helper functions outside the impl block:

```rust
/// Convert a JSON value to a SQL-safe string for binding.
fn json_value_to_sql(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(), // arrays/objects stored as JSON string
    }
}

/// Convert a SQL string back to a typed JSON value.
fn sql_to_json_value(val: &str, field_type: &FieldType) -> serde_json::Value {
    match field_type {
        FieldType::Number => val.parse::<f64>()
            .map(|n| serde_json::json!(n))
            .unwrap_or(serde_json::Value::String(val.to_string())),
        FieldType::Checkbox => serde_json::json!(val == "1"),
        FieldType::MultiSelect | FieldType::Files | FieldType::Relation => {
            serde_json::from_str(val).unwrap_or(serde_json::Value::String(val.to_string()))
        }
        _ => serde_json::Value::String(val.to_string()),
    }
}
```

- [ ] **Step 2: Add entity CRUD tests**

```rust
    #[tokio::test]
    async fn entity_crud_lifecycle() {
        let store = setup().await;
        let db = store.create_database(CreateDatabaseInput {
            name: "Tasks".into(), slug: None, icon: None, description: None, template_id: None,
        }).await.unwrap();

        store.add_field(&db.id, CreateFieldInput {
            name: "Title".into(), slug: None, field_type: FieldType::Text,
            options: None, required: None, default_value: None, position: None,
        }).await.unwrap();
        store.add_field(&db.id, CreateFieldInput {
            name: "Priority".into(), slug: None, field_type: FieldType::Number,
            options: None, required: None, default_value: None, position: None,
        }).await.unwrap();

        // Create
        let entity = store.create_entity(&db.id, HashMap::from([
            ("title".into(), serde_json::json!("Buy groceries")),
            ("priority".into(), serde_json::json!(3)),
        ])).await.unwrap();
        assert_eq!(entity.fields["title"], serde_json::json!("Buy groceries"));
        assert_eq!(entity.fields["priority"], serde_json::json!(3.0));

        // Get
        let fetched = store.get_entity(&db.id, &entity.id).await.unwrap();
        assert_eq!(fetched.fields["title"], serde_json::json!("Buy groceries"));

        // Update
        let updated = store.update_entity(&db.id, &entity.id, HashMap::from([
            ("title".into(), serde_json::json!("Buy organic groceries")),
        ])).await.unwrap();
        assert_eq!(updated.fields["title"], serde_json::json!("Buy organic groceries"));
        assert_eq!(updated.fields["priority"], serde_json::json!(3.0));

        // Delete
        store.delete_entity(&db.id, &entity.id).await.unwrap();
        assert!(store.get_entity(&db.id, &entity.id).await.is_err());
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p entity-store`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/entity-store/src/store.rs
git commit -m "feat(entity-store): entity CRUD — create, get, update, delete on dynamic tables"
```

---

## Task 7: Implement dynamic query builder

**Files:**
- Create: `crates/entity-store/src/query.rs`

- [ ] **Step 1: Implement the query builder with filters, sorts, pagination**

```rust
// crates/entity-store/src/query.rs
use common::Result;
use sqlx::SqlitePool;

use crate::types::*;

/// Parameters for querying entities from a database.
#[derive(Debug, Clone, Default)]
pub struct QueryParams {
    pub filters: Vec<FilterRule>,
    pub sorts: Vec<SortRule>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Result of a query with total count for pagination.
pub struct QueryResult {
    pub entities: Vec<Entity>,
    pub total: i64,
}

/// Build and execute a dynamic SELECT on a database's entity table.
pub async fn query_entities(
    pool: &SqlitePool,
    schema: &DatabaseSchema,
    params: &QueryParams,
) -> Result<QueryResult> {
    let table_name = format!("db_{}", schema.slug);
    let valid_slugs: std::collections::HashSet<&str> =
        schema.fields.iter().map(|f| f.slug.as_str()).collect();

    // Build WHERE clause
    let mut where_clauses = Vec::new();
    let mut bind_values = Vec::new();

    for filter in &params.filters {
        if !valid_slugs.contains(filter.field.as_str())
            && filter.field != "id"
            && filter.field != "created_at"
            && filter.field != "updated_at"
        {
            continue; // skip unknown fields
        }

        let clause = match filter.op {
            FilterOp::Eq => {
                bind_values.push(json_value_to_bind(&filter.value));
                format!("{} = ?", filter.field)
            }
            FilterOp::Neq => {
                bind_values.push(json_value_to_bind(&filter.value));
                format!("{} != ?", filter.field)
            }
            FilterOp::Gt => {
                bind_values.push(json_value_to_bind(&filter.value));
                format!("{} > ?", filter.field)
            }
            FilterOp::Gte => {
                bind_values.push(json_value_to_bind(&filter.value));
                format!("{} >= ?", filter.field)
            }
            FilterOp::Lt => {
                bind_values.push(json_value_to_bind(&filter.value));
                format!("{} < ?", filter.field)
            }
            FilterOp::Lte => {
                bind_values.push(json_value_to_bind(&filter.value));
                format!("{} <= ?", filter.field)
            }
            FilterOp::Contains => {
                let s = filter.value.as_str().unwrap_or_default();
                bind_values.push(format!("%{s}%"));
                format!("{} LIKE ?", filter.field)
            }
            FilterOp::NotContains => {
                let s = filter.value.as_str().unwrap_or_default();
                bind_values.push(format!("%{s}%"));
                format!("{} NOT LIKE ?", filter.field)
            }
            FilterOp::IsEmpty => format!("({f} IS NULL OR {f} = '')", f = filter.field),
            FilterOp::IsNotEmpty => format!("({f} IS NOT NULL AND {f} != '')", f = filter.field),
            FilterOp::In => {
                if let Some(arr) = filter.value.as_array() {
                    let placeholders: Vec<_> = arr.iter().map(|v| {
                        bind_values.push(json_value_to_bind(v));
                        "?"
                    }).collect();
                    format!("{} IN ({})", filter.field, placeholders.join(", "))
                } else {
                    continue;
                }
            }
            FilterOp::NotIn => {
                if let Some(arr) = filter.value.as_array() {
                    let placeholders: Vec<_> = arr.iter().map(|v| {
                        bind_values.push(json_value_to_bind(v));
                        "?"
                    }).collect();
                    format!("{} NOT IN ({})", filter.field, placeholders.join(", "))
                } else {
                    continue;
                }
            }
        };
        where_clauses.push(clause);
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Build ORDER BY
    let order_sql = if params.sorts.is_empty() {
        "ORDER BY created_at DESC".to_string()
    } else {
        let clauses: Vec<String> = params
            .sorts
            .iter()
            .filter(|s| valid_slugs.contains(s.field.as_str()) || ["id", "created_at", "updated_at"].contains(&s.field.as_str()))
            .map(|s| {
                let dir = match s.direction {
                    SortDirection::Asc => "ASC",
                    SortDirection::Desc => "DESC",
                };
                format!("{} {dir}", s.field)
            })
            .collect();
        if clauses.is_empty() {
            "ORDER BY created_at DESC".to_string()
        } else {
            format!("ORDER BY {}", clauses.join(", "))
        }
    };

    // Count query
    let count_sql = format!("SELECT COUNT(*) FROM {table_name} {where_sql}");
    let mut count_query = sqlx::query_scalar::<_, i32>(&count_sql);
    for v in &bind_values {
        count_query = count_query.bind(v);
    }
    let total = count_query.fetch_one(pool).await? as i64;

    // Build column list from schema
    let mut columns = vec!["id".to_string(), "created_at".to_string(), "updated_at".to_string()];
    for f in &schema.fields {
        if !f.hidden && f.field_type.is_editable() {
            columns.push(f.slug.clone());
        }
    }

    let limit_sql = match (params.limit, params.offset) {
        (Some(l), Some(o)) => format!("LIMIT {l} OFFSET {o}"),
        (Some(l), None) => format!("LIMIT {l}"),
        _ => String::new(),
    };

    let select_sql = format!(
        "SELECT {} FROM {table_name} {where_sql} {order_sql} {limit_sql}",
        columns.join(", ")
    );

    let mut select_query = sqlx::query(&select_sql);
    for v in &bind_values {
        select_query = select_query.bind(v);
    }

    let rows = select_query.fetch_all(pool).await?;

    let entities = rows
        .iter()
        .map(|row| {
            use sqlx::Row;
            let id: String = row.get("id");
            let created_at: String = row.get("created_at");
            let updated_at: String = row.get("updated_at");

            let mut fields = std::collections::HashMap::new();
            for f in &schema.fields {
                if f.hidden || !f.field_type.is_editable() {
                    continue;
                }
                if let Ok(val) = row.try_get::<Option<String>, _>(f.slug.as_str()) {
                    if let Some(v) = val {
                        fields.insert(f.slug.clone(), crate::store::sql_to_json_value(&v, &f.field_type));
                    }
                }
            }

            Entity {
                id,
                database_id: schema.id.clone(),
                fields,
                created_at,
                updated_at,
            }
        })
        .collect();

    Ok(QueryResult { entities, total })
}

fn json_value_to_bind(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::EntityStore;
    use crate::types::*;
    use storage::StoragePool;

    async fn setup_with_data() -> (EntityStore, DatabaseSchema) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        sqlx::query(include_str!("../migrations/001_entity_store.sql"))
            .execute(pool.raw_pool()).await.unwrap();
        let store = EntityStore::new(pool.raw_pool().clone());

        let db = store.create_database(CreateDatabaseInput {
            name: "Tasks".into(), slug: None, icon: None, description: None, template_id: None,
        }).await.unwrap();
        store.add_field(&db.id, CreateFieldInput {
            name: "Title".into(), slug: None, field_type: FieldType::Text,
            options: None, required: None, default_value: None, position: None,
        }).await.unwrap();
        store.add_field(&db.id, CreateFieldInput {
            name: "Priority".into(), slug: None, field_type: FieldType::Number,
            options: None, required: None, default_value: None, position: None,
        }).await.unwrap();
        store.add_field(&db.id, CreateFieldInput {
            name: "Status".into(), slug: None, field_type: FieldType::Text,
            options: None, required: None, default_value: None, position: None,
        }).await.unwrap();

        // Insert test entities
        for (title, priority, status) in [
            ("Buy groceries", 3.0, "todo"),
            ("Fix bug", 1.0, "doing"),
            ("Write docs", 2.0, "done"),
            ("Plan sprint", 1.0, "todo"),
        ] {
            store.create_entity(&db.id, std::collections::HashMap::from([
                ("title".into(), serde_json::json!(title)),
                ("priority".into(), serde_json::json!(priority)),
                ("status".into(), serde_json::json!(status)),
            ])).await.unwrap();
        }

        let schema = store.get_database(&db.id).await.unwrap();
        (store, schema)
    }

    #[tokio::test]
    async fn query_all() {
        let (store, schema) = setup_with_data().await;
        let result = query_entities(store.pool(), &schema, &QueryParams::default()).await.unwrap();
        assert_eq!(result.total, 4);
        assert_eq!(result.entities.len(), 4);
    }

    #[tokio::test]
    async fn query_with_filter() {
        let (store, schema) = setup_with_data().await;
        let result = query_entities(store.pool(), &schema, &QueryParams {
            filters: vec![FilterRule {
                field: "status".into(),
                op: FilterOp::Eq,
                value: serde_json::json!("todo"),
            }],
            ..Default::default()
        }).await.unwrap();
        assert_eq!(result.total, 2);
    }

    #[tokio::test]
    async fn query_with_sort_and_limit() {
        let (store, schema) = setup_with_data().await;
        let result = query_entities(store.pool(), &schema, &QueryParams {
            sorts: vec![SortRule { field: "priority".into(), direction: SortDirection::Asc }],
            limit: Some(2),
            ..Default::default()
        }).await.unwrap();
        assert_eq!(result.entities.len(), 2);
        // Priority 1.0 items should come first
        assert_eq!(result.entities[0].fields["priority"], serde_json::json!(1.0));
    }

    #[tokio::test]
    async fn query_not_in_filter() {
        let (store, schema) = setup_with_data().await;
        let result = query_entities(store.pool(), &schema, &QueryParams {
            filters: vec![FilterRule {
                field: "status".into(),
                op: FilterOp::NotIn,
                value: serde_json::json!(["done"]),
            }],
            ..Default::default()
        }).await.unwrap();
        assert_eq!(result.total, 3);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p entity-store`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/entity-store/src/query.rs
git commit -m "feat(entity-store): dynamic query builder — filters, sorts, pagination on schema-driven tables"
```

---

## Task 8: Add domain events for entity lifecycle

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Add entity and schema evolution events to DomainEvent enum**

Add these variants to the `DomainEvent` enum in `crates/bus/src/domain_events.rs`:

```rust
    // ── Entity Store Events ──
    EntityCreated {
        database_id: String,
        entity_id: String,
    },
    EntityUpdated {
        database_id: String,
        entity_id: String,
        changed_fields: Vec<String>,
    },
    EntityDeleted {
        database_id: String,
        entity_id: String,
    },
    DatabaseCreated {
        database_id: String,
        name: String,
    },
    DatabaseDeleted {
        database_id: String,
    },
    SchemaFieldAdded {
        database_id: String,
        field_id: String,
        field_name: String,
    },
    SchemaFieldRemoved {
        database_id: String,
        field_id: String,
    },
    SchemaEvolutionProposed {
        database_id: String,
        evolution_id: String,
        action_type: String,
        confidence: f64,
    },
    SchemaEvolutionApplied {
        database_id: String,
        evolution_id: String,
        auto_applied: bool,
    },
```

- [ ] **Step 2: Add event_name() and is_high_volume() match arms for new variants**

Add corresponding match arms in the `event_name()` and `is_high_volume()` methods. All entity store events are non-high-volume.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p bus`
Expected: Compile success (may have warnings about non-exhaustive matches in other crates — that's expected and will be fixed in later tasks)

- [ ] **Step 4: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "feat(bus): add entity lifecycle and schema evolution domain events"
```

---

## Task 9: Wire event emission into EntityStore

**Files:**
- Modify: `crates/entity-store/src/store.rs`

- [ ] **Step 1: Add event emission to create_entity, update_entity, delete_entity, create_database, delete_database, add_field, remove_field**

Add a helper method to EntityStore:

```rust
    fn emit(&self, event: bus::DomainEvent) {
        if let Some(ref bus) = self.bus {
            let _ = bus.publish(event);
        }
    }
```

Then add `self.emit(...)` calls at the end of each mutation method. For example, at the end of `create_entity`:

```rust
        self.emit(bus::DomainEvent::EntityCreated {
            database_id: database_id.to_string(),
            entity_id: entity_id.clone(),
        });
```

At the end of `update_entity`:

```rust
        let changed: Vec<String> = fields.keys().cloned().collect();
        self.emit(bus::DomainEvent::EntityUpdated {
            database_id: database_id.to_string(),
            entity_id: entity_id.to_string(),
            changed_fields: changed,
        });
```

Similar for `delete_entity`, `create_database`, `delete_database`, `add_field`, `remove_field`.

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p entity-store`
Expected: All tests still pass (bus is optional)

- [ ] **Step 3: Commit**

```bash
git add crates/entity-store/src/store.rs
git commit -m "feat(entity-store): emit domain events on entity and schema mutations"
```

---

## Task 10: Implement views and relations CRUD

**Files:**
- Create: `crates/entity-store/src/views.rs`
- Create: `crates/entity-store/src/relations.rs`

- [ ] **Step 1: Implement views CRUD in views.rs**

Add `create_view`, `update_view`, `delete_view` methods. These operate on the `database_views` table. Follow the same pattern as database/field CRUD in `store.rs` — insert/update/delete rows with nanoid IDs and timestamps.

- [ ] **Step 2: Implement relations CRUD in relations.rs**

Add `create_relation`, `delete_relation`, `list_relations_for_entity`, `list_relations_for_database` methods. These operate on the `entity_relations` table.

- [ ] **Step 3: Wire view and relation methods into EntityStore**

Add `pub mod views;` and `pub mod relations;` to lib.rs. Either add delegate methods on EntityStore or expose the modules directly.

- [ ] **Step 4: Write tests for view CRUD and relation CRUD**

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p entity-store`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/entity-store/src/views.rs crates/entity-store/src/relations.rs crates/entity-store/src/lib.rs
git commit -m "feat(entity-store): views and relations CRUD"
```

---

## Task 11: Implement template loading

**Files:**
- Create: `crates/entity-store/src/templates.rs`

- [ ] **Step 1: Define TemplateManifest struct matching spec Section 11**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateManifest {
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub version: String,
    pub databases: Vec<TemplateDatabaseDef>,
    #[serde(default)]
    pub relations: Vec<TemplateRelationDef>,
    pub skill_dir: Option<String>,
    #[serde(default)]
    pub dashboards: Vec<TemplateDashboardDef>,
}
```

- [ ] **Step 2: Implement `install_template(store, manifest, template_dir)` that creates databases + fields + views from manifest**

- [ ] **Step 3: Write test that loads a minimal manifest JSON and verifies databases/fields/views are created**

- [ ] **Step 4: Run tests and commit**

```bash
git add crates/entity-store/src/templates.rs
git commit -m "feat(entity-store): template manifest loading and instantiation"
```

---

## Task 12: Implement schema evolution storage

**Files:**
- Create: `crates/entity-store/src/evolution.rs`

- [ ] **Step 1: Implement CRUD for schema_evolutions and schema_autonomy tables**

Methods: `propose_evolution`, `list_pending_evolutions`, `resolve_evolution`, `get_autonomy`, `update_autonomy_on_accept`, `update_autonomy_on_dismiss`.

- [ ] **Step 2: Write tests**

- [ ] **Step 3: Run tests and commit**

```bash
git add crates/entity-store/src/evolution.rs
git commit -m "feat(entity-store): schema evolution proposal storage and autonomy tracking"
```

---

## Task 13: Create database-tool crate scaffold

**Files:**
- Create: `crates/database-tool/Cargo.toml`
- Create: `crates/database-tool/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create crate with dependencies on entity-store, tools-core, tools-core-macros, common, bus**

- [ ] **Step 2: Add to workspace members**

- [ ] **Step 3: Verify compilation**

- [ ] **Step 4: Commit**

---

## Task 14: Implement DatabaseTool with core actions

**Files:**
- Create: `crates/database-tool/src/tool.rs`
- Create: `crates/database-tool/src/actions/mod.rs`
- Create: `crates/database-tool/src/actions/database_ops.rs`
- Create: `crates/database-tool/src/actions/entity_crud.rs`
- Create: `crates/database-tool/src/actions/field_ops.rs`

- [ ] **Step 1: Implement DatabaseTool with Tool trait**

Follow the pattern from `crates/feature-tasks/src/tool/mod.rs` — implement `Tool` trait with `name()`, `description()`, `parameters_schema()`, `execute()`. The execute method dispatches to action handlers based on the `action` parameter.

Actions for this task: `create_database`, `list_databases`, `get_schema`, `delete_database`, `create`, `get`, `list`, `update`, `delete`, `add_field`, `remove_field`.

- [ ] **Step 2: Write integration test that creates a database via the tool interface**

- [ ] **Step 3: Run tests and commit**

---

## Task 15: Add search, relation, and view actions to DatabaseTool

**Files:**
- Create: `crates/database-tool/src/actions/search.rs`
- Create: `crates/database-tool/src/actions/relation_ops.rs`
- Create: `crates/database-tool/src/actions/view_ops.rs`

- [ ] **Step 1: Implement search action using query builder with keyword matching**

- [ ] **Step 2: Implement link, unlink, list_relations actions**

- [ ] **Step 3: Implement create_view, update_view, delete_view actions**

- [ ] **Step 4: Write tests and commit**

---

## Task 16: Define generic handler traits

**Files:**
- Create: `crates/database-tool/src/handlers.rs`

- [ ] **Step 1: Define generic handler traits that work with Entity + DatabaseSchema + Skill**

```rust
#[async_trait]
pub trait EntityEnrichmentHandler: Send + Sync {
    async fn enrich(
        &self,
        entity: &Entity,
        schema: &DatabaseSchema,
    ) -> Result<Option<HashMap<String, serde_json::Value>>>;
}

#[async_trait]
pub trait EntityEmbeddingHandler: Send + Sync {
    async fn embed_entity(&self, entity: &Entity, schema: &DatabaseSchema) -> Result<()>;
    async fn embed_query(&self, query: &str) -> Result<Vec<f32>>;
}
```

- [ ] **Step 2: Wire optional handlers into DatabaseTool via builder pattern**

- [ ] **Step 3: Commit**

---

## Task 17: Wire EntityStore into app-core

**Files:**
- Modify: `crates/app-core/Cargo.toml`
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/init/mod.rs`
- Create: `crates/app-core/src/handlers/database/mod.rs`

- [ ] **Step 1: Add entity-store and database-tool to app-core dependencies**

- [ ] **Step 2: Add `entity_store: Arc<EntityStore>` to AppCore state**

- [ ] **Step 3: Initialize EntityStore in app-core init — create pool, run migrations**

- [ ] **Step 4: Create handler functions in `handlers/database/mod.rs` that delegate to EntityStore**

Functions: `db_list`, `db_get_schema`, `db_query`, `db_create_entity`, `db_update_entity`, `db_delete_entity`, `db_add_field`, `db_remove_field`, `db_create_view`, `db_update_view`, `db_delete_view`, `db_get_suggestions`.

- [ ] **Step 5: Commit**

---

## Task 18: Add Tauri commands for database operations

**Files:**
- Create: `crates/desktop/src/commands/database.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Create Tauri command functions that delegate to app-core handlers**

Follow the pattern from `crates/desktop/src/commands/tasks.rs` — thin wrappers with `#[tauri::command]` that call `AppCore` methods and emit entity updates.

- [ ] **Step 2: Add DEV_COMMANDS constant and register in dev_server**

- [ ] **Step 3: Verify the dev_server_covers_all_tauri_commands test passes**

Run: `cargo nextest run -p desktop dev_server_covers`

- [ ] **Step 4: Commit**

---

## Task 19: Register DatabaseTool in agent builder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Add DatabaseTool registration alongside existing tools**

```rust
// In the build() method, after other tool registrations:
let database_tool = database_tool::DatabaseTool::new(
    entity_store.clone(),
);
tool_registry.register(database_tool);
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p agent`

- [ ] **Step 3: Commit**

---

## Task 20: Add "database" to MCP exposed tools

**Files:**
- Modify: `crates/config/src/schema/mcp.rs`

- [ ] **Step 1: Add "database" to default_exposed_tools()**

- [ ] **Step 2: Run MCP tests**

Run: `cargo nextest run -p klyntbot-server`

- [ ] **Step 3: Commit**

---

## Task 21: Re-export entity-store types from facade crate

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: Add `pub use entity_store;` to the facade re-exports**

- [ ] **Step 2: Verify workspace compilation**

Run: `cargo check --workspace`

- [ ] **Step 3: Commit**

---

## Task 22: Handle non-exhaustive match arms from new DomainEvent variants

**Files:**
- Modify: `crates/cognitive/src/services/salience.rs`
- Modify: `crates/cognitive/src/services/background.rs`
- Modify: various files with `match event { ... }` on DomainEvent

- [ ] **Step 1: Add catch-all arms or explicit handling for new entity events**

For now, add default handling:
- `salience.rs`: EntityCreated → Accumulate, EntityUpdated → Accumulate, EntityDeleted → Discard, Schema events → Discard
- `background.rs`: EntityCreated/Updated → create observation with importance 0.3, others → skip

- [ ] **Step 2: Verify zero clippy warnings**

Run: `cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 3: Commit**

---

## Task 23: Full integration test

**Files:**
- Create: `crates/entity-store/tests/integration.rs` or add to `tests/integration/`

- [ ] **Step 1: Write end-to-end test: create database → add fields → create entities → query with filters → add view → delete**

- [ ] **Step 2: Write test: template loading → creates database + fields + views**

- [ ] **Step 3: Run all workspace tests**

Run: `cargo nextest run --workspace`
Expected: All tests pass, zero clippy warnings

- [ ] **Step 4: Commit**

```bash
git commit -m "test(entity-store): integration tests for full entity lifecycle"
```

---

## Task 24: Verify workspace health

- [ ] **Step 1: Run full test suite**

Run: `cargo nextest run --workspace`

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --all --check`
Expected: No formatting issues

- [ ] **Step 4: Final commit with any fixes**
