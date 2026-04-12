//! Views and dashboards CRUD.

use sqlx::SqlitePool;

use crate::store::view_from_row;
use crate::types::*;
use common::Result;

/// Insert a new view for a database.
pub async fn create_view(
    pool: &SqlitePool,
    database_id: &str,
    name: &str,
    view_type: ViewType,
    config: ViewConfig,
    is_default: bool,
) -> Result<ViewDefinition> {
    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let vt_str = serde_json::to_value(&view_type)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "table".to_string());
    let config_json = serde_json::to_string(&config)?;

    sqlx::query(
        "INSERT INTO database_views (id, database_id, name, view_type, config_json, position, is_default, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?)",
    )
    .bind(&id)
    .bind(database_id)
    .bind(name)
    .bind(&vt_str)
    .bind(&config_json)
    .bind(is_default)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("Failed to create view: {e}")))?;

    Ok(ViewDefinition {
        id,
        database_id: database_id.to_string(),
        name: name.to_string(),
        view_type,
        config,
        position: 0,
        is_default,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Update a view's name and/or config.
pub async fn update_view(
    pool: &SqlitePool,
    view_id: &str,
    name: Option<&str>,
    config: Option<ViewConfig>,
) -> Result<ViewDefinition> {
    let now = chrono::Utc::now().to_rfc3339();

    let mut set_clauses = vec!["updated_at = ?".to_string()];
    let mut values = vec![now];

    if let Some(n) = name {
        set_clauses.push("name = ?".to_string());
        values.push(n.to_string());
    }
    if let Some(ref c) = config {
        set_clauses.push("config_json = ?".to_string());
        values.push(serde_json::to_string(c)?);
    }

    values.push(view_id.to_string());

    let sql = format!(
        "UPDATE database_views SET {} WHERE id = ?",
        set_clauses.join(", ")
    );

    let mut query = sqlx::query(&sql);
    for val in &values {
        query = query.bind(val);
    }
    query
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("Failed to update view: {e}")))?;

    get_view(pool, view_id).await
}

/// Delete a view by ID.
pub async fn delete_view(pool: &SqlitePool, view_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM database_views WHERE id = ?")
        .bind(view_id)
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}

/// Get a single view by ID.
pub async fn get_view(pool: &SqlitePool, view_id: &str) -> Result<ViewDefinition> {
    let row: crate::store::ViewRow = sqlx::query_as("SELECT * FROM database_views WHERE id = ?")
        .bind(view_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?
        .ok_or_else(|| {
            common::KlyntbotError::StorageNotFound(format!("View {view_id} not found"))
        })?;
    view_from_row(row)
}

// ---------------------------------------------------------------------------
// Dashboard CRUD
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct DashboardConfig {
    widgets: Vec<WidgetDefinition>,
}

/// Create a dashboard.
pub async fn create_dashboard(
    pool: &SqlitePool,
    name: &str,
    widgets: Vec<WidgetDefinition>,
) -> Result<Dashboard> {
    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let config_json = serde_json::to_string(&DashboardConfig {
        widgets: widgets.clone(),
    })?;

    sqlx::query(
        "INSERT INTO dashboards (id, name, config_json, position, created_at, updated_at) \
         VALUES (?, ?, ?, 0, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(&config_json)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("Failed to create dashboard: {e}")))?;

    Ok(Dashboard {
        id,
        name: name.to_string(),
        widgets,
        position: 0,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// List all dashboards.
pub async fn list_dashboards(pool: &SqlitePool) -> Result<Vec<Dashboard>> {
    let rows: Vec<(String, String, String, i32, String, String)> = sqlx::query_as(
        "SELECT id, name, config_json, position, created_at, updated_at FROM dashboards ORDER BY position ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    rows.into_iter()
        .map(
            |(id, name, config_json, position, created_at, updated_at)| {
                let cfg: DashboardConfig = serde_json::from_str(&config_json)?;
                Ok(Dashboard {
                    id,
                    name,
                    widgets: cfg.widgets,
                    position,
                    created_at,
                    updated_at,
                })
            },
        )
        .collect()
}

/// Update a dashboard.
pub async fn update_dashboard(
    pool: &SqlitePool,
    dashboard_id: &str,
    name: Option<&str>,
    widgets: Option<Vec<WidgetDefinition>>,
) -> Result<Dashboard> {
    let now = chrono::Utc::now().to_rfc3339();

    let mut set_clauses = vec!["updated_at = ?".to_string()];
    let mut values = vec![now];

    if let Some(n) = name {
        set_clauses.push("name = ?".to_string());
        values.push(n.to_string());
    }
    if let Some(ref w) = widgets {
        set_clauses.push("config_json = ?".to_string());
        values.push(serde_json::to_string(&DashboardConfig {
            widgets: w.clone(),
        })?);
    }

    values.push(dashboard_id.to_string());

    let sql = format!(
        "UPDATE dashboards SET {} WHERE id = ?",
        set_clauses.join(", ")
    );

    let mut query = sqlx::query(&sql);
    for val in &values {
        query = query.bind(val);
    }
    query
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("Failed to update dashboard: {e}")))?;

    // Re-fetch
    let row: (String, String, String, i32, String, String) = sqlx::query_as(
        "SELECT id, name, config_json, position, created_at, updated_at FROM dashboards WHERE id = ?",
    )
    .bind(dashboard_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?
    .ok_or_else(|| {
        common::KlyntbotError::StorageNotFound(format!("Dashboard {dashboard_id} not found"))
    })?;

    let cfg: DashboardConfig = serde_json::from_str(&row.2)?;
    Ok(Dashboard {
        id: row.0,
        name: row.1,
        widgets: cfg.widgets,
        position: row.3,
        created_at: row.4,
        updated_at: row.5,
    })
}

/// Delete a dashboard.
pub async fn delete_dashboard(pool: &SqlitePool, dashboard_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM dashboards WHERE id = ?")
        .bind(dashboard_id)
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::test_helpers::setup_test_pool().await
    }

    async fn create_test_db(pool: &SqlitePool) -> String {
        let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO databases (id, name, slug, created_at, updated_at) VALUES (?, 'Test', 'test', ?, ?)",
        )
        .bind(&id)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    #[tokio::test]
    async fn test_create_and_get_view() {
        let pool = setup().await;
        let db_id = create_test_db(&pool).await;

        let view = create_view(
            &pool,
            &db_id,
            "Board View",
            ViewType::Board,
            ViewConfig::default(),
            false,
        )
        .await
        .unwrap();

        assert_eq!(view.name, "Board View");
        assert_eq!(view.view_type, ViewType::Board);
        assert!(!view.is_default);

        let fetched = get_view(&pool, &view.id).await.unwrap();
        assert_eq!(fetched.name, "Board View");
        assert_eq!(fetched.database_id, db_id);
    }

    #[tokio::test]
    async fn test_update_view_config() {
        let pool = setup().await;
        let db_id = create_test_db(&pool).await;

        let view = create_view(
            &pool,
            &db_id,
            "Table",
            ViewType::Table,
            ViewConfig::default(),
            true,
        )
        .await
        .unwrap();

        let new_config = ViewConfig {
            group_by: Some("status".to_string()),
            ..Default::default()
        };

        let updated = update_view(&pool, &view.id, Some("Renamed"), Some(new_config))
            .await
            .unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.config.group_by, Some("status".to_string()));
    }

    #[tokio::test]
    async fn test_delete_view() {
        let pool = setup().await;
        let db_id = create_test_db(&pool).await;

        let view = create_view(
            &pool,
            &db_id,
            "Temp",
            ViewType::List,
            ViewConfig::default(),
            false,
        )
        .await
        .unwrap();

        delete_view(&pool, &view.id).await.unwrap();
        assert!(get_view(&pool, &view.id).await.is_err());
    }

    #[tokio::test]
    async fn test_create_and_list_dashboards() {
        let pool = setup().await;

        let d1 = create_dashboard(&pool, "Overview", vec![]).await.unwrap();
        let d2 = create_dashboard(&pool, "Finance", vec![]).await.unwrap();

        let list = list_dashboards(&pool).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, d1.id);
        assert_eq!(list[1].id, d2.id);

        // Update
        let updated = update_dashboard(&pool, &d1.id, Some("Updated Overview"), None)
            .await
            .unwrap();
        assert_eq!(updated.name, "Updated Overview");

        // Delete
        delete_dashboard(&pool, &d2.id).await.unwrap();
        let list = list_dashboards(&pool).await.unwrap();
        assert_eq!(list.len(), 1);
    }
}
