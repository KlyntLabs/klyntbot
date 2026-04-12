//! Schema evolution tracking and autonomy calibration.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use common::Result;

// ---------------------------------------------------------------------------
// Status constants
// ---------------------------------------------------------------------------

pub const STATUS_PROPOSED: &str = "proposed";
pub const STATUS_ACCEPTED: &str = "accepted";
pub const STATUS_DISMISSED: &str = "dismissed";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaEvolution {
    pub id: String,
    pub database_id: String,
    pub action_type: String,
    pub action_json: serde_json::Value,
    pub confidence: f64,
    pub reasoning: String,
    pub status: String,
    pub source: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaAutonomy {
    pub database_id: String,
    pub auto_threshold: f64,
    pub suggest_threshold: f64,
    pub acceptance_rate: f64,
    pub total_proposed: i32,
    pub total_accepted: i32,
}

#[derive(Debug, FromRow)]
struct EvolutionRow {
    id: String,
    database_id: String,
    action_type: String,
    action_json: String,
    confidence: f64,
    reasoning: String,
    status: String,
    source: String,
    created_at: String,
    resolved_at: Option<String>,
}

impl TryFrom<EvolutionRow> for SchemaEvolution {
    type Error = common::KlyntbotError;

    fn try_from(r: EvolutionRow) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            id: r.id,
            database_id: r.database_id,
            action_type: r.action_type,
            action_json: serde_json::from_str(&r.action_json)?,
            confidence: r.confidence,
            reasoning: r.reasoning,
            status: r.status,
            source: r.source,
            created_at: r.created_at,
            resolved_at: r.resolved_at,
        })
    }
}

#[derive(Debug, FromRow)]
struct AutonomyRow {
    database_id: String,
    auto_threshold: f64,
    suggest_threshold: f64,
    acceptance_rate: f64,
    total_proposed: i32,
    total_accepted: i32,
}

impl From<AutonomyRow> for SchemaAutonomy {
    fn from(r: AutonomyRow) -> Self {
        Self {
            database_id: r.database_id,
            auto_threshold: r.auto_threshold,
            suggest_threshold: r.suggest_threshold,
            acceptance_rate: r.acceptance_rate,
            total_proposed: r.total_proposed,
            total_accepted: r.total_accepted,
        }
    }
}

// ---------------------------------------------------------------------------
// Evolution CRUD
// ---------------------------------------------------------------------------

/// Propose a schema evolution. Returns the evolution ID.
pub async fn propose_evolution(
    pool: &SqlitePool,
    database_id: &str,
    action_type: &str,
    action_json: &serde_json::Value,
    confidence: f64,
    reasoning: &str,
    source: &str,
) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let action_str = serde_json::to_string(action_json)?;

    sqlx::query(
        "INSERT INTO schema_evolutions (id, database_id, action_type, action_json, confidence, reasoning, status, source, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(database_id)
    .bind(action_type)
    .bind(&action_str)
    .bind(confidence)
    .bind(reasoning)
    .bind(STATUS_PROPOSED)
    .bind(source)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("Failed to propose evolution: {e}")))?;

    Ok(id)
}

/// List pending (proposed) evolutions for a database.
pub async fn list_pending_evolutions(
    pool: &SqlitePool,
    database_id: &str,
) -> Result<Vec<SchemaEvolution>> {
    let rows: Vec<EvolutionRow> = sqlx::query_as(
        "SELECT * FROM schema_evolutions WHERE database_id = ? AND status = ? ORDER BY created_at ASC",
    )
    .bind(database_id)
    .bind(STATUS_PROPOSED)
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    rows.into_iter().map(SchemaEvolution::try_from).collect()
}

/// Resolve an evolution (e.g. "accepted", "dismissed", "applied").
pub async fn resolve_evolution(pool: &SqlitePool, evolution_id: &str, status: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query("UPDATE schema_evolutions SET status = ?, resolved_at = ? WHERE id = ?")
        .bind(status)
        .bind(&now)
        .bind(evolution_id)
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Autonomy calibration
// ---------------------------------------------------------------------------

/// Get autonomy settings for a database, creating defaults if absent.
pub async fn get_autonomy(pool: &SqlitePool, database_id: &str) -> Result<SchemaAutonomy> {
    let now = chrono::Utc::now().to_rfc3339();

    // Ensure row exists
    sqlx::query("INSERT OR IGNORE INTO schema_autonomy (database_id, updated_at) VALUES (?, ?)")
        .bind(database_id)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    let row: AutonomyRow = sqlx::query_as("SELECT * FROM schema_autonomy WHERE database_id = ?")
        .bind(database_id)
        .fetch_one(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    Ok(SchemaAutonomy::from(row))
}

/// Record an accepted evolution: increment both total_accepted and total_proposed.
pub async fn record_acceptance(pool: &SqlitePool, database_id: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    // Ensure row exists
    sqlx::query("INSERT OR IGNORE INTO schema_autonomy (database_id, updated_at) VALUES (?, ?)")
        .bind(database_id)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    sqlx::query(
        "UPDATE schema_autonomy SET \
         total_accepted = total_accepted + 1, \
         total_proposed = total_proposed + 1, \
         acceptance_rate = CAST(total_accepted + 1 AS REAL) / CAST(total_proposed + 1 AS REAL), \
         updated_at = ? \
         WHERE database_id = ?",
    )
    .bind(&now)
    .bind(database_id)
    .execute(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    Ok(())
}

/// Record a dismissed evolution: increment total_proposed only.
pub async fn record_dismissal(pool: &SqlitePool, database_id: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    // Ensure row exists
    sqlx::query("INSERT OR IGNORE INTO schema_autonomy (database_id, updated_at) VALUES (?, ?)")
        .bind(database_id)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    sqlx::query(
        "UPDATE schema_autonomy SET \
         total_proposed = total_proposed + 1, \
         acceptance_rate = CAST(total_accepted AS REAL) / CAST(total_proposed + 1 AS REAL), \
         updated_at = ? \
         WHERE database_id = ?",
    )
    .bind(&now)
    .bind(database_id)
    .execute(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = crate::test_helpers::setup_test_pool().await;
        // Need a database row for FK
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO databases (id, name, slug, created_at, updated_at) VALUES ('db1', 'Test', 'test', ?, ?)")
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn test_propose_and_list_pending() {
        let pool = setup().await;

        let action = serde_json::json!({"field": "priority", "type": "number"});
        let id = propose_evolution(
            &pool,
            "db1",
            "add_field",
            &action,
            0.85,
            "Frequently mentioned",
            "reforge",
        )
        .await
        .unwrap();
        assert!(!id.is_empty());

        let pending = list_pending_evolutions(&pool, "db1").await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].action_type, "add_field");
        assert_eq!(pending[0].confidence, 0.85);
        assert_eq!(pending[0].status, STATUS_PROPOSED);
    }

    #[tokio::test]
    async fn test_resolve_evolution() {
        let pool = setup().await;

        let action = serde_json::json!({"field": "tags"});
        let id = propose_evolution(&pool, "db1", "add_field", &action, 0.7, "test", "reforge")
            .await
            .unwrap();

        resolve_evolution(&pool, &id, STATUS_ACCEPTED)
            .await
            .unwrap();

        let pending = list_pending_evolutions(&pool, "db1").await.unwrap();
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn test_autonomy_acceptance_rate() {
        let pool = setup().await;

        // Initial state
        let a = get_autonomy(&pool, "db1").await.unwrap();
        assert_eq!(a.total_proposed, 0);
        assert_eq!(a.total_accepted, 0);
        assert!((a.acceptance_rate - 0.5).abs() < 0.01);

        // 3 acceptances
        record_acceptance(&pool, "db1").await.unwrap();
        record_acceptance(&pool, "db1").await.unwrap();
        record_acceptance(&pool, "db1").await.unwrap();

        // 1 dismissal
        record_dismissal(&pool, "db1").await.unwrap();

        let a = get_autonomy(&pool, "db1").await.unwrap();
        assert_eq!(a.total_proposed, 4);
        assert_eq!(a.total_accepted, 3);
        assert!((a.acceptance_rate - 0.75).abs() < 0.01);
    }
}
