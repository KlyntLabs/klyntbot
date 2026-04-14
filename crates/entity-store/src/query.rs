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
    pub filter: Option<FilterGroup>,
    #[serde(default)]
    pub sorts: Vec<SortRule>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Matches Notion's 3-level nesting limit.
const MAX_FILTER_DEPTH: usize = 3;

fn is_builtin_column(name: &str) -> bool {
    matches!(name, "id" | "position" | "created_at" | "updated_at")
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

    let mut where_parts = Vec::new();
    let mut bind_values: Vec<String> = Vec::new();

    if let Some(ref tree) = params.filter {
        if let Some(sql) = build_filter_group(tree, &field_map, &mut bind_values, 0) {
            where_parts.push(sql);
        }
    }

    for filter in &params.filters {
        if let Some(sql) = build_rule_sql(filter, &field_map, &mut bind_values) {
            where_parts.push(sql);
        }
    }

    let where_clause = if where_parts.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_parts.join(" AND "))
    };

    let order_by = if params.sorts.is_empty() {
        " ORDER BY position ASC, created_at ASC".to_string()
    } else {
        let parts: Vec<String> = params
            .sorts
            .iter()
            .filter(|s| is_builtin_column(&s.field) || field_map.contains_key(s.field.as_str()))
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
        let position: String = row.try_get("position").unwrap_or_default();
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
            position,
            created_at,
            updated_at,
        });
    }

    Ok(QueryResult { entities, total })
}

/// Returns None for empty groups — SQLite rejects `()` as a WHERE clause.
fn build_filter_group(
    group: &FilterGroup,
    field_map: &HashMap<&str, &FieldDefinition>,
    bind_values: &mut Vec<String>,
    depth: usize,
) -> Option<String> {
    if depth >= MAX_FILTER_DEPTH {
        return None;
    }
    let mut parts = Vec::with_capacity(group.nodes.len());
    for node in &group.nodes {
        match node {
            FilterNode::Rule(rule) => {
                if let Some(sql) = build_rule_sql(rule, field_map, bind_values) {
                    parts.push(sql);
                }
            }
            FilterNode::Group(sub) => {
                if let Some(sql) = build_filter_group(sub, field_map, bind_values, depth + 1) {
                    parts.push(sql);
                }
            }
        }
    }
    if parts.is_empty() {
        return None;
    }
    let joiner = match group.op {
        LogicOp::And => " AND ",
        LogicOp::Or => " OR ",
    };
    Some(format!("({})", parts.join(joiner)))
}

fn build_rule_sql(
    filter: &FilterRule,
    field_map: &HashMap<&str, &FieldDefinition>,
    bind_values: &mut Vec<String>,
) -> Option<String> {
    if !is_builtin_column(&filter.field) && !field_map.contains_key(filter.field.as_str()) {
        return None;
    }
    let col = format!("[{}]", filter.field);
    Some(match filter.op {
        FilterOp::Eq => {
            bind_values.push(value_to_sql_string(&filter.value));
            format!("{col} = ?")
        }
        FilterOp::Neq => {
            bind_values.push(value_to_sql_string(&filter.value));
            format!("{col} != ?")
        }
        FilterOp::Gt => {
            bind_values.push(value_to_sql_string(&filter.value));
            format!("{col} > ?")
        }
        FilterOp::Gte => {
            bind_values.push(value_to_sql_string(&filter.value));
            format!("{col} >= ?")
        }
        FilterOp::Lt => {
            bind_values.push(value_to_sql_string(&filter.value));
            format!("{col} < ?")
        }
        FilterOp::Lte => {
            bind_values.push(value_to_sql_string(&filter.value));
            format!("{col} <= ?")
        }
        FilterOp::Contains => {
            let s = value_to_sql_string(&filter.value);
            bind_values.push(format!("%{s}%"));
            format!("{col} LIKE ?")
        }
        FilterOp::NotContains => {
            let s = value_to_sql_string(&filter.value);
            bind_values.push(format!("%{s}%"));
            format!("{col} NOT LIKE ?")
        }
        FilterOp::IsEmpty => format!("({col} IS NULL OR {col} = '')"),
        FilterOp::IsNotEmpty => format!("({col} IS NOT NULL AND {col} != '')"),
        FilterOp::In => {
            let arr = filter.value.as_array()?;
            if arr.is_empty() {
                return Some("0".to_string());
            }
            let placeholders: Vec<&str> = arr.iter().map(|_| "?").collect();
            for v in arr {
                bind_values.push(value_to_sql_string(v));
            }
            format!("{col} IN ({})", placeholders.join(", "))
        }
        FilterOp::NotIn => {
            let arr = filter.value.as_array()?;
            if arr.is_empty() {
                return Some("1".to_string());
            }
            let placeholders: Vec<&str> = arr.iter().map(|_| "?").collect();
            for v in arr {
                bind_values.push(value_to_sql_string(v));
            }
            format!("{col} NOT IN ({})", placeholders.join(", "))
        }
    })
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
    async fn test_query_nested_filter_and_or() {
        // (name = "Alpha") OR (score > 20 AND score <= 30) → Alpha + Gamma
        let store = setup().await;
        let schema = setup_with_entities(&store).await;

        let filter = FilterGroup {
            op: LogicOp::Or,
            nodes: vec![
                FilterNode::Rule(FilterRule {
                    field: "name".into(),
                    op: FilterOp::Eq,
                    value: serde_json::json!("Alpha"),
                }),
                FilterNode::Group(FilterGroup {
                    op: LogicOp::And,
                    nodes: vec![
                        FilterNode::Rule(FilterRule {
                            field: "score".into(),
                            op: FilterOp::Gt,
                            value: serde_json::json!(20),
                        }),
                        FilterNode::Rule(FilterRule {
                            field: "score".into(),
                            op: FilterOp::Lte,
                            value: serde_json::json!(30),
                        }),
                    ],
                }),
            ],
        };
        let params = QueryParams {
            filter: Some(filter),
            ..Default::default()
        };
        let result = query_entities(store.pool(), &schema, &params)
            .await
            .unwrap();
        assert_eq!(result.total, 2);
        let names: std::collections::HashSet<_> = result
            .entities
            .iter()
            .map(|e| e.fields["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains("Alpha"));
        assert!(names.contains("Gamma"));
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
