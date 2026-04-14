//! DDL operations for dynamic entity tables.

use common::Result;
use sqlx::SqlitePool;

use crate::FieldDefinition;

/// Validate a slug: non-empty, starts with lowercase letter, only [a-z0-9_].
pub fn validate_slug(slug: &str) -> Result<()> {
    if slug.is_empty() {
        return Err(common::KlyntbotError::Storage(
            "Slug must not be empty".into(),
        ));
    }
    let first = slug.as_bytes()[0];
    if !first.is_ascii_lowercase() {
        return Err(common::KlyntbotError::Storage(
            "Slug must start with a lowercase letter".into(),
        ));
    }
    if !slug
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(common::KlyntbotError::Storage(
            "Slug must only contain lowercase letters, digits, and underscores".into(),
        ));
    }
    Ok(())
}

/// Convert a display name to a valid slug.
pub fn slugify(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let slug = slug.trim_matches('_').to_string();
    // Collapse consecutive underscores
    let mut result = String::with_capacity(slug.len());
    let mut prev_underscore = false;
    for c in slug.chars() {
        if c == '_' {
            if !prev_underscore {
                result.push('_');
            }
            prev_underscore = true;
        } else {
            result.push(c);
            prev_underscore = false;
        }
    }
    result
}

/// Create a dynamic entity table: db_{slug} with id, position, created_at, updated_at.
pub async fn create_entity_table(pool: &SqlitePool, slug: &str) -> Result<()> {
    validate_slug(slug)?;
    let table = format!("db_{slug}");
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS [{table}] (\
         id TEXT PRIMARY KEY, \
         position TEXT NOT NULL DEFAULT '', \
         created_at TEXT NOT NULL, \
         updated_at TEXT NOT NULL)"
    );
    sqlx::query(&sql).execute(pool).await.map_err(|e| {
        common::KlyntbotError::Storage(format!("Failed to create table {table}: {e}"))
    })?;
    let idx_sql =
        format!("CREATE INDEX IF NOT EXISTS [idx_{slug}_position] ON [{table}](position)");
    sqlx::query(&idx_sql).execute(pool).await.map_err(|e| {
        common::KlyntbotError::Storage(format!("Failed to create position index on {table}: {e}"))
    })?;
    Ok(())
}

/// Ensure an existing entity table has the `position` column. Backfills existing rows with
/// fractional index keys ordered by created_at so drag-reordering works against legacy data.
pub async fn ensure_position_column(pool: &SqlitePool, slug: &str) -> Result<()> {
    validate_slug(slug)?;
    let table = format!("db_{slug}");
    let cols: Vec<(i32, String, String, i32, Option<String>, i32)> =
        sqlx::query_as(&format!("PRAGMA table_info([{table}])"))
            .fetch_all(pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    if cols.iter().any(|c| c.1 == "position") {
        return Ok(());
    }
    let alter = format!("ALTER TABLE [{table}] ADD COLUMN position TEXT NOT NULL DEFAULT ''");
    sqlx::query(&alter).execute(pool).await.map_err(|e| {
        common::KlyntbotError::Storage(format!("Failed to add position column to {table}: {e}"))
    })?;
    let idx_sql =
        format!("CREATE INDEX IF NOT EXISTS [idx_{slug}_position] ON [{table}](position)");
    sqlx::query(&idx_sql)
        .execute(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    let ids: Vec<(String,)> =
        sqlx::query_as(&format!("SELECT id FROM [{table}] ORDER BY created_at ASC"))
            .fetch_all(pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    let mut prev: Option<String> = None;
    for (id,) in ids {
        let key = crate::store::fractional_key(&prev, &None)?;
        sqlx::query(&format!("UPDATE [{table}] SET position = ? WHERE id = ?"))
            .bind(&key)
            .bind(&id)
            .execute(pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        prev = Some(key);
    }
    Ok(())
}

/// Add a column to a dynamic entity table.
pub async fn add_column(pool: &SqlitePool, db_slug: &str, field: &FieldDefinition) -> Result<()> {
    validate_slug(db_slug)?;
    validate_slug(&field.slug)?;
    let table = format!("db_{db_slug}");
    let col = &field.slug;
    let col_type = field.field_type.sqlite_type();
    let sql = format!("ALTER TABLE [{table}] ADD COLUMN [{col}] {col_type}");
    sqlx::query(&sql).execute(pool).await.map_err(|e| {
        common::KlyntbotError::Storage(format!("Failed to add column {col} to {table}: {e}"))
    })?;
    Ok(())
}

/// Drop a column from a dynamic entity table.
pub async fn drop_column(pool: &SqlitePool, db_slug: &str, field_slug: &str) -> Result<()> {
    validate_slug(db_slug)?;
    validate_slug(field_slug)?;
    let table = format!("db_{db_slug}");
    let sql = format!("ALTER TABLE [{table}] DROP COLUMN [{field_slug}]");
    sqlx::query(&sql).execute(pool).await.map_err(|e| {
        common::KlyntbotError::Storage(format!(
            "Failed to drop column {field_slug} from {table}: {e}"
        ))
    })?;
    Ok(())
}

/// Drop a dynamic entity table.
pub async fn drop_entity_table(pool: &SqlitePool, slug: &str) -> Result<()> {
    validate_slug(slug)?;
    let table = format!("db_{slug}");
    let sql = format!("DROP TABLE IF EXISTS [{table}]");
    sqlx::query(&sql).execute(pool).await.map_err(|e| {
        common::KlyntbotError::Storage(format!("Failed to drop table {table}: {e}"))
    })?;
    Ok(())
}

/// Check whether a dynamic entity table exists.
pub async fn table_exists(pool: &SqlitePool, slug: &str) -> Result<bool> {
    validate_slug(slug)?;
    let table = format!("db_{slug}");
    let row: (i32,) =
        sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?")
            .bind(&table)
            .fetch_one(pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    Ok(row.0 > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_slug_valid() {
        assert!(validate_slug("hello").is_ok());
        assert!(validate_slug("my_db").is_ok());
        assert!(validate_slug("a123").is_ok());
        assert!(validate_slug("abc_def_123").is_ok());
    }

    #[test]
    fn test_validate_slug_invalid() {
        assert!(validate_slug("").is_err());
        assert!(validate_slug("123abc").is_err());
        assert!(validate_slug("_abc").is_err());
        assert!(validate_slug("Hello").is_err());
        assert!(validate_slug("has-dash").is_err());
        assert!(validate_slug("has space").is_err());
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("My Database"), "my_database");
        assert_eq!(slugify("Hello World!"), "hello_world");
        // trim trailing underscores — wait, trim_matches trims both ends
        assert_eq!(slugify("  spaced  "), "spaced");
        assert_eq!(slugify("CamelCase"), "camelcase");
        assert_eq!(slugify("a--b"), "a_b");
        assert_eq!(slugify("foo___bar"), "foo_bar");
    }

    #[test]
    fn test_slugify_empty() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("___"), "");
    }

    #[tokio::test]
    async fn test_create_and_check_table() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let raw = pool.inner();

        assert!(!table_exists(raw, "test").await.unwrap());
        create_entity_table(raw, "test").await.unwrap();
        assert!(table_exists(raw, "test").await.unwrap());
    }

    #[tokio::test]
    async fn test_drop_table() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let raw = pool.inner();

        create_entity_table(raw, "dropme").await.unwrap();
        assert!(table_exists(raw, "dropme").await.unwrap());
        drop_entity_table(raw, "dropme").await.unwrap();
        assert!(!table_exists(raw, "dropme").await.unwrap());
    }

    #[tokio::test]
    async fn test_add_and_drop_column() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let raw = pool.inner();

        create_entity_table(raw, "cols").await.unwrap();

        let field = FieldDefinition {
            id: "f1".into(),
            database_id: "db1".into(),
            name: "Title".into(),
            slug: "title".into(),
            field_type: crate::FieldType::Text,
            options: None,
            position: 0,
            required: false,
            hidden: false,
            ai_managed: false,
            ai_config: None,
            default_value: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        add_column(raw, "cols", &field).await.unwrap();

        // Verify column exists by inserting a row
        sqlx::query("INSERT INTO db_cols (id, created_at, updated_at, title) VALUES ('r1', '', '', 'hello')")
            .execute(raw)
            .await
            .unwrap();

        drop_column(raw, "cols", "title").await.unwrap();
    }

    #[tokio::test]
    async fn test_invalid_slug_rejected() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let raw = pool.inner();

        assert!(create_entity_table(raw, "123bad").await.is_err());
        assert!(create_entity_table(raw, "").await.is_err());
    }
}
