//! Dynamic query builder for entity databases.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::store::read_column_value;
use crate::types::*;
use common::Result;

/// Parameters for querying entities in a database.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    #[serde(default)]
    pub filters: Vec<FilterRule>,
    #[serde(default)]
    pub sorts: Vec<SortRule>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Result of a query — entities plus total count (before limit/offset).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub entities: Vec<Entity>,
    pub total: i64,
}

/// Execute a dynamic query against a database's entity table.
pub async fn query_entities(
    pool: &SqlitePool,
    schema: &DatabaseSchema,
    params: &QueryParams,
) -> Result<QueryResult> {
    let table = format!("db_{}", schema.slug);
    let field_map: HashMap<&str, &FieldDefinition> =
        schema.fields.iter().map(|f| (f.slug.as_str(), f)).collect();

    // Build WHERE clause
    let mut where_parts = Vec::new();
    let mut bind_values: Vec<String> = Vec::new();

    for filter in &params.filters {
        // Allow filtering on built-in columns too
        let is_builtin = matches!(filter.field.as_str(), "id" | "created_at" | "updated_at");
        if !is_builtin && !field_map.contains_key(filter.field.as_str()) {
            continue; // skip unknown fields
        }

        let col = format!("[{}]", filter.field);
        match filter.op {
            FilterOp::Eq => {
                where_parts.push(format!("{col} = ?"));
                bind_values.push(value_to_sql_string(&filter.value));
            }
            FilterOp::Neq => {
                where_parts.push(format!("{col} != ?"));
                bind_values.push(value_to_sql_string(&filter.value));
            }
            FilterOp::Gt => {
                where_parts.push(format!("{col} > ?"));
                bind_values.push(value_to_sql_string(&filter.value));
            }
            FilterOp::Gte => {
                where_parts.push(format!("{col} >= ?"));
                bind_values.push(value_to_sql_string(&filter.value));
            }
            FilterOp::Lt => {
                where_parts.push(format!("{col} < ?"));
                bind_values.push(value_to_sql_string(&filter.value));
            }
            FilterOp::Lte => {
                where_parts.push(format!("{col} <= ?"));
                bind_values.push(value_to_sql_string(&filter.value));
            }
            FilterOp::Contains => {
                where_parts.push(format!("{col} LIKE ?"));
                let s = value_to_sql_string(&filter.value);
                bind_values.push(format!("%{s}%"));
            }
            FilterOp::NotContains => {
                where_parts.push(format!("{col} NOT LIKE ?"));
                let s = value_to_sql_string(&filter.value);
                bind_values.push(format!("%{s}%"));
            }
            FilterOp::IsEmpty => {
                where_parts.push(format!("({col} IS NULL OR {col} = '')"));
            }
            FilterOp::IsNotEmpty => {
                where_parts.push(format!("({col} IS NOT NULL AND {col} != '')"));
            }
            FilterOp::In => {
                if let Some(arr) = filter.value.as_array() {
                    if arr.is_empty() {
                        where_parts.push("0".to_string()); // always false
                    } else {
                        let placeholders: Vec<&str> = arr.iter().map(|_| "?").collect();
                        where_parts.push(format!("{col} IN ({})", placeholders.join(", ")));
                        for v in arr {
                            bind_values.push(value_to_sql_string(v));
                        }
                    }
                }
            }
            FilterOp::NotIn => {
                if let Some(arr) = filter.value.as_array() {
                    if arr.is_empty() {
                        // no exclusions — always true, skip
                    } else {
                        let placeholders: Vec<&str> = arr.iter().map(|_| "?").collect();
                        where_parts.push(format!("{col} NOT IN ({})", placeholders.join(", ")));
                        for v in arr {
                            bind_values.push(value_to_sql_string(v));
                        }
                    }
                }
            }
        }
    }

    let where_clause = if where_parts.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_parts.join(" AND "))
    };

    // Build ORDER BY
    let order_by = if params.sorts.is_empty() {
        " ORDER BY created_at DESC".to_string()
    } else {
        let parts: Vec<String> = params
            .sorts
            .iter()
            .filter(|s| {
                matches!(s.field.as_str(), "id" | "created_at" | "updated_at")
                    || field_map.contains_key(s.field.as_str())
            })
            .map(|s| {
                let dir = match s.direction {
                    SortDirection::Asc => "ASC",
                    SortDirection::Desc => "DESC",
                };
                format!("[{}] {dir}", s.field)
            })
            .collect();
        if parts.is_empty() {
            " ORDER BY created_at DESC".to_string()
        } else {
            format!(" ORDER BY {}", parts.join(", "))
        }
    };

    // Count query
    let count_sql = format!("SELECT COUNT(*) as cnt FROM [{table}]{where_clause}");
    let mut count_query = sqlx::query(&count_sql);
    for v in &bind_values {
        count_query = count_query.bind(v);
    }
    let count_row = count_query
        .fetch_one(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("Query count failed: {e}")))?;
    let total: i64 = count_row
        .try_get::<i32, _>("cnt")
        .map(|v| v as i64)
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    // Select query
    let mut select_sql = format!("SELECT * FROM [{table}]{where_clause}{order_by}");
    if let Some(limit) = params.limit {
        select_sql.push_str(&format!(" LIMIT {limit}"));
    }
    if let Some(offset) = params.offset {
        select_sql.push_str(&format!(" OFFSET {offset}"));
    }

    let mut select_query = sqlx::query(&select_sql);
    for v in &bind_values {
        select_query = select_query.bind(v);
    }
    let rows = select_query
        .fetch_all(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("Query select failed: {e}")))?;

    let mut entities = Vec::with_capacity(rows.len());
    for row in &rows {
        let id: String = row
            .try_get("id")
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let created_at: String = row
            .try_get("created_at")
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let updated_at: String = row
            .try_get("updated_at")
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let mut fields = HashMap::new();
        for field_def in &schema.fields {
            if let Some(val) = read_column_value(row, &field_def.slug, &field_def.field_type) {
                fields.insert(field_def.slug.clone(), val);
            }
        }

        entities.push(Entity {
            id,
            database_id: schema.id.clone(),
            fields,
            created_at,
            updated_at,
        });
    }

    Ok(QueryResult { entities, total })
}

fn value_to_sql_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::EntityStore;
    use crate::types::{CreateDatabaseInput, CreateFieldInput, FieldType};

    async fn setup() -> EntityStore {
        crate::test_helpers::setup_test_store().await
    }

    async fn setup_with_entities(store: &EntityStore) -> DatabaseSchema {
        let db = store
            .create_database(CreateDatabaseInput {
                name: "Items".into(),
                slug: None,
                icon: None,
                description: None,
                template_id: None,
            })
            .await
            .unwrap();

        store
            .add_field(
                &db.id,
                CreateFieldInput {
                    name: "Name".into(),
                    slug: None,
                    field_type: FieldType::Text,
                    options: None,
                    required: Some(true),
                    default_value: None,
                    position: None,
                },
            )
            .await
            .unwrap();
        store
            .add_field(
                &db.id,
                CreateFieldInput {
                    name: "Score".into(),
                    slug: None,
                    field_type: FieldType::Number,
                    options: None,
                    required: None,
                    default_value: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        // Insert some entities
        for (name, score) in [("Alpha", 10), ("Beta", 20), ("Gamma", 30), ("Delta", 20)] {
            let mut fields = HashMap::new();
            fields.insert("name".to_string(), serde_json::json!(name));
            fields.insert("score".to_string(), serde_json::json!(score));
            store.create_entity(&db.id, fields).await.unwrap();
        }

        store.get_database(&db.id).await.unwrap()
    }

    #[tokio::test]
    async fn test_query_all() {
        let store = setup().await;
        let schema = setup_with_entities(&store).await;

        let result = query_entities(store.pool(), &schema, &QueryParams::default())
            .await
            .unwrap();
        assert_eq!(result.total, 4);
        assert_eq!(result.entities.len(), 4);
    }

    #[tokio::test]
    async fn test_query_eq_filter() {
        let store = setup().await;
        let schema = setup_with_entities(&store).await;

        let params = QueryParams {
            filters: vec![FilterRule {
                field: "name".into(),
                op: FilterOp::Eq,
                value: serde_json::json!("Alpha"),
            }],
            ..Default::default()
        };
        let result = query_entities(store.pool(), &schema, &params)
            .await
            .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(
            result.entities[0].fields["name"],
            serde_json::json!("Alpha")
        );
    }

    #[tokio::test]
    async fn test_query_not_in_filter() {
        let store = setup().await;
        let schema = setup_with_entities(&store).await;

        let params = QueryParams {
            filters: vec![FilterRule {
                field: "name".into(),
                op: FilterOp::NotIn,
                value: serde_json::json!(["Alpha", "Beta"]),
            }],
            ..Default::default()
        };
        let result = query_entities(store.pool(), &schema, &params)
            .await
            .unwrap();
        assert_eq!(result.total, 2);
    }

    #[tokio::test]
    async fn test_query_sort_and_limit() {
        let store = setup().await;
        let schema = setup_with_entities(&store).await;

        let params = QueryParams {
            sorts: vec![SortRule {
                field: "score".into(),
                direction: SortDirection::Desc,
            }],
            limit: Some(2),
            ..Default::default()
        };
        let result = query_entities(store.pool(), &schema, &params)
            .await
            .unwrap();
        assert_eq!(result.total, 4); // total is unaffected by limit
        assert_eq!(result.entities.len(), 2);
        assert_eq!(result.entities[0].fields["score"], serde_json::json!(30.0));
    }

    #[tokio::test]
    async fn test_query_contains_filter() {
        let store = setup().await;
        let schema = setup_with_entities(&store).await;

        let params = QueryParams {
            filters: vec![FilterRule {
                field: "name".into(),
                op: FilterOp::Contains,
                value: serde_json::json!("lph"),
            }],
            ..Default::default()
        };
        let result = query_entities(store.pool(), &schema, &params)
            .await
            .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(
            result.entities[0].fields["name"],
            serde_json::json!("Alpha")
        );
    }
}
