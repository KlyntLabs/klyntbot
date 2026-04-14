//! Core EntityStore — database and field CRUD.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use sqlx::{FromRow, Row, SqlitePool};

use crate::schema_ops::{create_entity_table, drop_entity_table, slugify, validate_slug};
use crate::types::*;
use common::Result;

// ---------------------------------------------------------------------------
// Internal row types
// ---------------------------------------------------------------------------

#[derive(Debug, FromRow)]
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

#[derive(Debug, FromRow)]
struct FieldRow {
    id: String,
    database_id: String,
    name: String,
    slug: String,
    field_type: String,
    options_json: Option<String>,
    position: i32,
    required: bool,
    hidden: bool,
    ai_managed: bool,
    ai_config: Option<String>,
    default_value: Option<String>,
    created_at: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct ViewRow {
    id: String,
    database_id: String,
    name: String,
    view_type: String,
    config_json: String,
    position: i32,
    is_default: bool,
    created_at: String,
    updated_at: String,
}

// ---------------------------------------------------------------------------
// Row conversions
// ---------------------------------------------------------------------------

fn field_from_row(row: FieldRow) -> Result<FieldDefinition> {
    let field_type: FieldType = serde_json::from_str(&format!("\"{}\"", row.field_type))?;
    let options = row
        .options_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?;
    let ai_config = row
        .ai_config
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?;
    Ok(FieldDefinition {
        id: row.id,
        database_id: row.database_id,
        name: row.name,
        slug: row.slug,
        field_type,
        options,
        position: row.position,
        required: row.required,
        hidden: row.hidden,
        ai_managed: row.ai_managed,
        ai_config,
        default_value: row.default_value,
        created_at: row.created_at,
    })
}

pub(crate) fn view_from_row(row: ViewRow) -> Result<ViewDefinition> {
    let view_type: ViewType = serde_json::from_str(&format!("\"{}\"", row.view_type))?;
    let config: ViewConfig = serde_json::from_str(&row.config_json)?;
    Ok(ViewDefinition {
        id: row.id,
        database_id: row.database_id,
        name: row.name,
        view_type,
        config,
        position: row.position,
        is_default: row.is_default,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn field_type_to_db(ft: &FieldType) -> String {
    // serde_json::to_value gives Value::String("multi_select"), extract the str
    serde_json::to_value(ft)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "text".to_string())
}

// ---------------------------------------------------------------------------
// Value conversion helpers
// ---------------------------------------------------------------------------

pub(crate) fn json_value_to_sql(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => val.to_string(),
    }
}

pub(crate) fn sql_to_json_value(val: &str, field_type: &FieldType) -> serde_json::Value {
    match field_type {
        FieldType::Number => val
            .parse::<f64>()
            .map(|n| serde_json::Value::Number(serde_json::Number::from_f64(n).unwrap()))
            .unwrap_or(serde_json::Value::String(val.to_string())),
        FieldType::Checkbox => {
            serde_json::Value::Bool(val == "1" || val.eq_ignore_ascii_case("true"))
        }
        FieldType::MultiSelect | FieldType::Files | FieldType::Relation => {
            serde_json::from_str(val).unwrap_or(serde_json::Value::String(val.to_string()))
        }
        _ => serde_json::Value::String(val.to_string()),
    }
}

/// Read a column value from a dynamic row, handling type-specific extraction.
pub fn read_column_value(
    row: &sqlx::sqlite::SqliteRow,
    col: &str,
    field_type: &FieldType,
) -> Option<serde_json::Value> {
    match field_type {
        FieldType::Number => {
            // REAL columns — try f64 first, fall back to string
            if let Ok(Some(v)) = row.try_get::<Option<f64>, _>(col) {
                Some(serde_json::Value::Number(
                    serde_json::Number::from_f64(v).unwrap(),
                ))
            } else {
                None
            }
        }
        FieldType::Checkbox => {
            if let Ok(Some(v)) = row.try_get::<Option<i32>, _>(col) {
                Some(serde_json::Value::Bool(v != 0))
            } else {
                None
            }
        }
        _ => {
            // TEXT columns
            let raw: Option<String> = row.try_get(col).unwrap_or(None);
            raw.map(|v| sql_to_json_value(&v, field_type))
        }
    }
}

// ---------------------------------------------------------------------------
// EntityStore
// ---------------------------------------------------------------------------

pub struct EntityStore {
    pool: SqlitePool,
    #[allow(dead_code)]
    bus: Option<Arc<bus::DomainEventBus>>,
    schema_cache: Mutex<HashMap<String, DatabaseSchema>>,
}

impl EntityStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            bus: None,
            schema_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_event_bus(mut self, bus: Arc<bus::DomainEventBus>) -> Self {
        self.bus = Some(bus);
        self
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    fn emit(&self, event: bus::DomainEvent) {
        if let Some(ref bus) = self.bus {
            bus.publish(event);
        }
    }

    /// Get schema, using cache if available.
    async fn get_schema_cached(&self, database_id: &str) -> Result<DatabaseSchema> {
        if let Some(cached) = self.schema_cache.lock().unwrap().get(database_id) {
            return Ok(cached.clone());
        }
        let schema = self.get_database(database_id).await?;
        self.schema_cache
            .lock()
            .unwrap()
            .insert(database_id.to_string(), schema.clone());
        Ok(schema)
    }

    /// Invalidate cache for a database (call after schema changes).
    fn invalidate_cache(&self, database_id: &str) {
        self.schema_cache.lock().unwrap().remove(database_id);
    }

    /// Get a single entity using an already-loaded schema (avoids redundant DB lookup).
    async fn get_entity_with_schema(
        &self,
        database_id: &str,
        entity_id: &str,
        schema: &DatabaseSchema,
    ) -> Result<Entity> {
        let table = format!("db_{}", schema.slug);
        let sql = format!("SELECT * FROM [{}] WHERE id = ?", table);
        let row = sqlx::query(&sql)
            .bind(entity_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?
            .ok_or_else(|| {
                common::KlyntbotError::StorageNotFound(format!("Entity {entity_id} not found"))
            })?;

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

        let mut fields_map = HashMap::new();
        for field_def in &schema.fields {
            if let Some(val) = read_column_value(&row, &field_def.slug, &field_def.field_type) {
                fields_map.insert(field_def.slug.clone(), val);
            }
        }

        Ok(Entity {
            id,
            database_id: database_id.to_string(),
            fields: fields_map,
            position,
            created_at,
            updated_at,
        })
    }

    // -----------------------------------------------------------------------
    // Database CRUD
    // -----------------------------------------------------------------------

    pub async fn create_database(&self, input: CreateDatabaseInput) -> Result<DatabaseSchema> {
        let slug = match input.slug {
            Some(s) => {
                validate_slug(&s)?;
                s
            }
            None => {
                let s = slugify(&input.name);
                if s.is_empty() {
                    return Err(common::KlyntbotError::Storage(
                        "Cannot derive a valid slug from the given name".into(),
                    ));
                }
                validate_slug(&s)?;
                s
            }
        };

        let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO databases (id, name, slug, icon, description, template_id, skill_id, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&slug)
        .bind(&input.icon)
        .bind(&input.description)
        .bind(&input.template_id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("Failed to create database: {e}")))?;

        // Create the dynamic entity table
        create_entity_table(&self.pool, &slug).await?;

        let schema = DatabaseSchema {
            id,
            name: input.name,
            slug,
            icon: input.icon,
            description: input.description,
            template_id: input.template_id,
            skill_id: None,
            fields: vec![],
            views: vec![],
            created_at: now.clone(),
            updated_at: now,
        };

        self.emit(bus::DomainEvent::DatabaseCreated {
            database_id: schema.id.clone(),
            name: schema.name.clone(),
        });

        Ok(schema)
    }

    pub async fn get_database(&self, id: &str) -> Result<DatabaseSchema> {
        let row: DatabaseRow = sqlx::query_as("SELECT * FROM databases WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?
            .ok_or_else(|| {
                common::KlyntbotError::StorageNotFound(format!("Database {id} not found"))
            })?;

        let fields = self.list_fields(&row.id).await?;
        let views = self.list_views(&row.id).await?;

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

    pub async fn list_databases(&self) -> Result<Vec<DatabaseSchema>> {
        let rows: Vec<DatabaseRow> =
            sqlx::query_as("SELECT * FROM databases ORDER BY created_at ASC")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let mut dbs = Vec::with_capacity(rows.len());
        for row in rows {
            let fields = self.list_fields(&row.id).await?;
            let views = self.list_views(&row.id).await?;
            dbs.push(DatabaseSchema {
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
        Ok(dbs)
    }

    pub async fn delete_database(&self, id: &str) -> Result<()> {
        let row: DatabaseRow = sqlx::query_as("SELECT * FROM databases WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?
            .ok_or_else(|| {
                common::KlyntbotError::StorageNotFound(format!("Database {id} not found"))
            })?;

        // Drop dynamic table
        drop_entity_table(&self.pool, &row.slug).await?;

        // Delete registry row (CASCADE deletes fields, views, autonomy)
        sqlx::query("DELETE FROM databases WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        self.invalidate_cache(id);

        self.emit(bus::DomainEvent::DatabaseDeleted {
            database_id: id.to_string(),
        });

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Field CRUD
    // -----------------------------------------------------------------------

    pub async fn add_field(
        &self,
        database_id: &str,
        input: CreateFieldInput,
    ) -> Result<FieldDefinition> {
        // Ensure database exists and get its slug
        let db = self.get_database(database_id).await?;

        let slug = match input.slug {
            Some(s) => {
                validate_slug(&s)?;
                s
            }
            None => {
                let s = slugify(&input.name);
                if s.is_empty() {
                    return Err(common::KlyntbotError::Storage(
                        "Cannot derive a valid slug from the given field name".into(),
                    ));
                }
                validate_slug(&s)?;
                s
            }
        };

        let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let ft_str = field_type_to_db(&input.field_type);
        let options_json = input
            .options
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let position = input.position.unwrap_or(db.fields.len() as i32);
        let required = input.required.unwrap_or(false);

        sqlx::query(
            "INSERT INTO database_fields \
             (id, database_id, name, slug, field_type, options_json, position, required, hidden, ai_managed, ai_config, default_value, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, 0, NULL, ?, ?)",
        )
        .bind(&id)
        .bind(database_id)
        .bind(&input.name)
        .bind(&slug)
        .bind(&ft_str)
        .bind(&options_json)
        .bind(position)
        .bind(required)
        .bind(&input.default_value)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("Failed to add field: {e}")))?;

        // Add column to the dynamic table
        let field = FieldDefinition {
            id: id.clone(),
            database_id: database_id.to_string(),
            name: input.name,
            slug,
            field_type: input.field_type,
            options: input.options,
            position,
            required,
            hidden: false,
            ai_managed: false,
            ai_config: None,
            default_value: input.default_value,
            created_at: now,
        };
        crate::schema_ops::add_column(&self.pool, &db.slug, &field).await?;

        self.invalidate_cache(database_id);

        self.emit(bus::DomainEvent::SchemaFieldAdded {
            database_id: field.database_id.clone(),
            field_id: field.id.clone(),
            field_name: field.name.clone(),
        });

        Ok(field)
    }

    /// Remove a field. If `hard` is true, drop the column; otherwise just set hidden=1.
    pub async fn remove_field(&self, database_id: &str, field_id: &str, hard: bool) -> Result<()> {
        let db = self.get_database(database_id).await?;
        let field = self.get_field(field_id).await?;

        if field.database_id != database_id {
            return Err(common::KlyntbotError::Storage(
                "Field does not belong to this database".into(),
            ));
        }

        if hard {
            crate::schema_ops::drop_column(&self.pool, &db.slug, &field.slug).await?;
            sqlx::query("DELETE FROM database_fields WHERE id = ?")
                .bind(field_id)
                .execute(&self.pool)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        } else {
            sqlx::query("UPDATE database_fields SET hidden = 1 WHERE id = ?")
                .bind(field_id)
                .execute(&self.pool)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        }

        self.invalidate_cache(database_id);

        self.emit(bus::DomainEvent::SchemaFieldRemoved {
            database_id: database_id.to_string(),
            field_id: field_id.to_string(),
        });

        Ok(())
    }

    async fn get_field(&self, id: &str) -> Result<FieldDefinition> {
        let row: FieldRow = sqlx::query_as("SELECT * FROM database_fields WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?
            .ok_or_else(|| {
                common::KlyntbotError::StorageNotFound(format!("Field {id} not found"))
            })?;
        field_from_row(row)
    }

    /// Modify a field's metadata (name, options, required, position). Does NOT change the column type.
    pub async fn modify_field(
        &self,
        database_id: &str,
        field_id: &str,
        name: Option<String>,
        options: Option<serde_json::Value>,
        required: Option<bool>,
        position: Option<i32>,
    ) -> Result<FieldDefinition> {
        let field = self.get_field(field_id).await?;
        if field.database_id != database_id {
            return Err(common::KlyntbotError::Storage(
                "Field does not belong to this database".into(),
            ));
        }

        let mut sets = Vec::new();
        if name.is_some() {
            sets.push("name = ?".to_string());
        }
        if options.is_some() {
            sets.push("options_json = ?".to_string());
        }
        if required.is_some() {
            sets.push("required = ?".to_string());
        }
        if position.is_some() {
            sets.push("position = ?".to_string());
        }

        if sets.is_empty() {
            return Ok(field);
        }

        let sql = format!(
            "UPDATE database_fields SET {} WHERE id = ?",
            sets.join(", ")
        );
        let mut q = sqlx::query(&sql);

        if let Some(ref n) = name {
            q = q.bind(n);
        }
        if let Some(ref o) = options {
            let json_str = serde_json::to_string(o)?;
            q = q.bind(json_str);
        }
        if let Some(r) = required {
            q = q.bind(r);
        }
        if let Some(p) = position {
            q = q.bind(p);
        }
        q = q.bind(field_id);

        q.execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("Failed to modify field: {e}")))?;

        self.invalidate_cache(database_id);
        self.get_field(field_id).await
    }

    pub async fn list_fields(&self, database_id: &str) -> Result<Vec<FieldDefinition>> {
        let rows: Vec<FieldRow> = sqlx::query_as(
            "SELECT * FROM database_fields WHERE database_id = ? ORDER BY position ASC",
        )
        .bind(database_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        rows.into_iter().map(field_from_row).collect()
    }

    // -----------------------------------------------------------------------
    // Views
    // -----------------------------------------------------------------------

    pub async fn list_views(&self, database_id: &str) -> Result<Vec<ViewDefinition>> {
        let rows: Vec<ViewRow> = sqlx::query_as(
            "SELECT * FROM database_views WHERE database_id = ? ORDER BY position ASC",
        )
        .bind(database_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        rows.into_iter().map(view_from_row).collect()
    }

    pub async fn create_view(
        &self,
        database_id: &str,
        name: &str,
        view_type: ViewType,
        config: ViewConfig,
        is_default: bool,
    ) -> Result<ViewDefinition> {
        crate::views::create_view(&self.pool, database_id, name, view_type, config, is_default)
            .await
    }

    pub async fn update_view(
        &self,
        database_id: &str,
        view_id: &str,
        name: Option<&str>,
        config: Option<ViewConfig>,
    ) -> Result<ViewDefinition> {
        let existing = crate::views::get_view(&self.pool, view_id).await?;
        if existing.database_id != database_id {
            return Err(common::KlyntbotError::Storage(format!(
                "View {view_id} does not belong to database {database_id}"
            )));
        }
        crate::views::update_view(&self.pool, view_id, name, config).await
    }

    pub async fn delete_view(&self, database_id: &str, view_id: &str) -> Result<()> {
        let existing = crate::views::get_view(&self.pool, view_id).await?;
        if existing.database_id != database_id {
            return Err(common::KlyntbotError::Storage(format!(
                "View {view_id} does not belong to database {database_id}"
            )));
        }
        crate::views::delete_view(&self.pool, view_id).await
    }

    // -----------------------------------------------------------------------
    // Entity CRUD
    // -----------------------------------------------------------------------

    pub async fn create_entity(
        &self,
        database_id: &str,
        fields: HashMap<String, serde_json::Value>,
    ) -> Result<Entity> {
        let db = self.get_schema_cached(database_id).await?;
        let entity_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let table = format!("db_{}", db.slug);

        // Validate required fields
        for field_def in &db.fields {
            if field_def.required && !field_def.hidden && field_def.field_type.is_editable() {
                let has_value =
                    fields.contains_key(&field_def.slug) || field_def.default_value.is_some();
                if !has_value {
                    return Err(common::ToolError::InvalidParams(format!(
                        "Required field '{}' not provided",
                        field_def.name
                    ))
                    .into());
                }
            }
        }

        // Compute a fractional index position that sorts after the current last row.
        let last_position: Option<(String,)> = sqlx::query_as(&format!(
            "SELECT position FROM [{}] ORDER BY position DESC LIMIT 1",
            table
        ))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let prev_key = last_position.and_then(|(p,)| if p.is_empty() { None } else { Some(p) });
        let position = lexicon_fractional_index::key_between(&prev_key, &None)
            .map_err(|e| common::KlyntbotError::Storage(format!("fractional key error: {e}")))?;

        // Build column list and values
        let mut columns = vec![
            "id".to_string(),
            "position".to_string(),
            "created_at".to_string(),
            "updated_at".to_string(),
        ];
        let mut placeholders = vec![
            "?".to_string(),
            "?".to_string(),
            "?".to_string(),
            "?".to_string(),
        ];
        let mut values: Vec<String> = vec![entity_id.clone(), position, now.clone(), now.clone()];

        for field_def in &db.fields {
            if !field_def.field_type.is_editable() {
                continue;
            }
            if let Some(val) = fields.get(&field_def.slug) {
                columns.push(format!("[{}]", field_def.slug));
                placeholders.push("?".to_string());
                values.push(json_value_to_sql(val));
            } else if let Some(ref default) = field_def.default_value {
                columns.push(format!("[{}]", field_def.slug));
                placeholders.push("?".to_string());
                values.push(default.clone());
            }
        }

        let sql = format!(
            "INSERT INTO [{}] ({}) VALUES ({})",
            table,
            columns.join(", "),
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for val in &values {
            query = query.bind(val);
        }
        query
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("Failed to create entity: {e}")))?;

        self.emit(bus::DomainEvent::EntityCreated {
            database_id: database_id.to_string(),
            entity_id: entity_id.clone(),
        });

        self.get_entity_with_schema(database_id, &entity_id, &db)
            .await
    }

    pub async fn get_entity(&self, database_id: &str, entity_id: &str) -> Result<Entity> {
        let schema = self.get_schema_cached(database_id).await?;
        self.get_entity_with_schema(database_id, entity_id, &schema)
            .await
    }

    pub async fn update_entity(
        &self,
        database_id: &str,
        entity_id: &str,
        fields: HashMap<String, serde_json::Value>,
    ) -> Result<Entity> {
        let db = self.get_schema_cached(database_id).await?;
        let table = format!("db_{}", db.slug);
        let now = chrono::Utc::now().to_rfc3339();

        let schema_slugs: HashMap<&str, &FieldDefinition> =
            db.fields.iter().map(|f| (f.slug.as_str(), f)).collect();

        let mut set_clauses = vec!["updated_at = ?".to_string()];
        let mut values = vec![now];

        for (slug, val) in &fields {
            if let Some(field_def) = schema_slugs.get(slug.as_str()) {
                if field_def.field_type.is_editable() {
                    set_clauses.push(format!("[{}] = ?", slug));
                    values.push(json_value_to_sql(val));
                }
            }
        }

        values.push(entity_id.to_string());

        let sql = format!(
            "UPDATE [{}] SET {} WHERE id = ?",
            table,
            set_clauses.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for val in &values {
            query = query.bind(val);
        }
        let changed_fields: Vec<String> = fields.keys().cloned().collect();

        query
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("Failed to update entity: {e}")))?;

        self.emit(bus::DomainEvent::EntityUpdated {
            database_id: database_id.to_string(),
            entity_id: entity_id.to_string(),
            changed_fields,
        });

        self.get_entity_with_schema(database_id, entity_id, &db)
            .await
    }

    /// Reorder an entity by setting its fractional position between two neighbors.
    /// Pass the IDs of the entities that should end up immediately before and after.
    /// Either may be `None` to signal "start" or "end" of the list.
    pub async fn reorder_entity(
        &self,
        database_id: &str,
        entity_id: &str,
        before_id: Option<&str>,
        after_id: Option<&str>,
    ) -> Result<Entity> {
        let db = self.get_schema_cached(database_id).await?;
        let table = format!("db_{}", db.slug);

        let before_pos = if let Some(id) = before_id {
            let row: Option<(String,)> =
                sqlx::query_as(&format!("SELECT position FROM [{}] WHERE id = ?", table))
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            row.map(|(p,)| p).filter(|p| !p.is_empty())
        } else {
            None
        };
        let after_pos = if let Some(id) = after_id {
            let row: Option<(String,)> =
                sqlx::query_as(&format!("SELECT position FROM [{}] WHERE id = ?", table))
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            row.map(|(p,)| p).filter(|p| !p.is_empty())
        } else {
            None
        };

        let new_key = lexicon_fractional_index::key_between(&before_pos, &after_pos)
            .map_err(|e| common::KlyntbotError::Storage(format!("fractional key error: {e}")))?;
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(&format!(
            "UPDATE [{}] SET position = ?, updated_at = ? WHERE id = ?",
            table
        ))
        .bind(&new_key)
        .bind(&now)
        .bind(entity_id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("Failed to reorder entity: {e}")))?;

        self.emit(bus::DomainEvent::EntityUpdated {
            database_id: database_id.to_string(),
            entity_id: entity_id.to_string(),
            changed_fields: vec!["position".to_string()],
        });

        self.get_entity_with_schema(database_id, entity_id, &db)
            .await
    }

    /// Reorder all views for a database by setting their position to match the given order.
    pub async fn reorder_views(
        &self,
        database_id: &str,
        view_ids: Vec<String>,
    ) -> Result<Vec<ViewDefinition>> {
        let now = chrono::Utc::now().to_rfc3339();
        for (idx, view_id) in view_ids.iter().enumerate() {
            sqlx::query(
                "UPDATE database_views SET position = ?, updated_at = ? WHERE id = ? AND database_id = ?",
            )
            .bind(idx as i32)
            .bind(&now)
            .bind(view_id)
            .bind(database_id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("Failed to reorder view: {e}")))?;
        }
        self.list_views(database_id).await
    }

    /// Ensure all existing entity tables have the `position` column and backfill missing keys.
    /// Called once at startup to bring legacy databases up to current schema.
    pub async fn ensure_ready(&self) -> Result<()> {
        let dbs = self.list_databases().await?;
        for db in dbs {
            crate::schema_ops::ensure_position_column(&self.pool, &db.slug).await?;
        }
        Ok(())
    }

    pub async fn delete_entity(&self, database_id: &str, entity_id: &str) -> Result<()> {
        let db = self.get_schema_cached(database_id).await?;
        let table = format!("db_{}", db.slug);

        let sql = format!("DELETE FROM [{}] WHERE id = ?", table);
        sqlx::query(&sql)
            .bind(entity_id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("Failed to delete entity: {e}")))?;

        self.emit(bus::DomainEvent::EntityDeleted {
            database_id: database_id.to_string(),
            entity_id: entity_id.to_string(),
        });

        Ok(())
    }

    /// Search entities across all text fields using SQL LIKE (OR semantics).
    pub async fn search_entities(
        &self,
        database_id: &str,
        query_str: &str,
        limit: usize,
    ) -> Result<Vec<Entity>> {
        let schema = self.get_schema_cached(database_id).await?;
        let table = format!("db_{}", schema.slug);

        let text_slugs: Vec<&str> = schema
            .fields
            .iter()
            .filter(|f| {
                matches!(
                    f.field_type,
                    FieldType::Text | FieldType::Url | FieldType::Email
                ) && !f.hidden
            })
            .map(|f| f.slug.as_str())
            .collect();

        if text_slugs.is_empty() {
            return Ok(vec![]);
        }

        let like_conditions: Vec<String> = text_slugs
            .iter()
            .map(|slug| format!("[{}] LIKE ?", slug))
            .collect();
        let where_clause = like_conditions.join(" OR ");
        let like_pattern = format!("%{}%", query_str);

        let sql = format!(
            "SELECT * FROM [{}] WHERE {} ORDER BY created_at DESC LIMIT ?",
            table, where_clause
        );

        let mut query = sqlx::query(&sql);
        for _ in &text_slugs {
            query = query.bind(like_pattern.clone());
        }
        query = query.bind(limit as i64);

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

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

            let mut fields_map = HashMap::new();
            for field_def in &schema.fields {
                if let Some(val) = read_column_value(row, &field_def.slug, &field_def.field_type) {
                    fields_map.insert(field_def.slug.clone(), val);
                }
            }

            entities.push(Entity {
                id,
                database_id: database_id.to_string(),
                fields: fields_map,
                position,
                created_at,
                updated_at,
            });
        }

        Ok(entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> EntityStore {
        crate::test_helpers::setup_test_store().await
    }

    fn test_input(name: &str) -> CreateDatabaseInput {
        CreateDatabaseInput {
            name: name.to_string(),
            slug: None,
            icon: None,
            description: None,
            template_id: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_database() {
        let store = setup().await;
        let db = store
            .create_database(test_input("My Projects"))
            .await
            .unwrap();
        assert_eq!(db.name, "My Projects");
        assert_eq!(db.slug, "my_projects");
        assert!(db.fields.is_empty());

        let fetched = store.get_database(&db.id).await.unwrap();
        assert_eq!(fetched.name, "My Projects");
        assert_eq!(fetched.slug, "my_projects");
    }

    #[tokio::test]
    async fn test_list_databases() {
        let store = setup().await;
        store.create_database(test_input("Alpha")).await.unwrap();
        store.create_database(test_input("Beta")).await.unwrap();

        let dbs = store.list_databases().await.unwrap();
        assert_eq!(dbs.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_database() {
        let store = setup().await;
        let db = store.create_database(test_input("ToDelete")).await.unwrap();
        store.delete_database(&db.id).await.unwrap();

        assert!(store.get_database(&db.id).await.is_err());
        assert!(!crate::schema_ops::table_exists(&store.pool, "todelete")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_add_field_creates_column() {
        let store = setup().await;
        let db = store.create_database(test_input("Tasks")).await.unwrap();

        let field = store
            .add_field(
                &db.id,
                CreateFieldInput {
                    name: "Priority".into(),
                    slug: None,
                    field_type: FieldType::Number,
                    options: None,
                    required: Some(false),
                    default_value: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(field.slug, "priority");
        assert_eq!(field.field_type, FieldType::Number);

        // Verify column exists by inserting data
        sqlx::query("INSERT INTO db_tasks (id, created_at, updated_at, priority) VALUES ('r1', '', '', 42.0)")
            .execute(store.pool())
            .await
            .unwrap();

        // Verify field appears in database schema
        let db = store.get_database(&db.id).await.unwrap();
        assert_eq!(db.fields.len(), 1);
        assert_eq!(db.fields[0].slug, "priority");
    }

    #[tokio::test]
    async fn test_remove_field_soft_hides() {
        let store = setup().await;
        let db = store.create_database(test_input("Notes")).await.unwrap();

        let field = store
            .add_field(
                &db.id,
                CreateFieldInput {
                    name: "Tags".into(),
                    slug: None,
                    field_type: FieldType::MultiSelect,
                    options: None,
                    required: None,
                    default_value: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        // Soft remove — field still in DB but hidden
        store.remove_field(&db.id, &field.id, false).await.unwrap();

        let fields = store.list_fields(&db.id).await.unwrap();
        assert_eq!(fields.len(), 1);
        assert!(fields[0].hidden);
    }

    #[tokio::test]
    async fn test_remove_field_hard_deletes() {
        let store = setup().await;
        let db = store.create_database(test_input("Items")).await.unwrap();

        let field = store
            .add_field(
                &db.id,
                CreateFieldInput {
                    name: "Color".into(),
                    slug: None,
                    field_type: FieldType::Text,
                    options: None,
                    required: None,
                    default_value: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        store.remove_field(&db.id, &field.id, true).await.unwrap();

        let fields = store.list_fields(&db.id).await.unwrap();
        assert!(fields.is_empty());
    }

    #[tokio::test]
    async fn test_modify_field() {
        let store = setup().await;
        let db = store.create_database(test_input("Modify")).await.unwrap();

        let field = store
            .add_field(
                &db.id,
                CreateFieldInput {
                    name: "Status".into(),
                    slug: None,
                    field_type: FieldType::Select,
                    options: Some(serde_json::json!({"choices": ["open", "closed"]})),
                    required: Some(false),
                    default_value: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(field.name, "Status");
        assert!(!field.required);

        let modified = store
            .modify_field(
                &db.id,
                &field.id,
                Some("State".into()),
                Some(serde_json::json!({"choices": ["open", "closed", "in_progress"]})),
                Some(true),
                None,
            )
            .await
            .unwrap();

        assert_eq!(modified.name, "State");
        assert!(modified.required);
        let opts = modified.options.unwrap();
        let choices = opts["choices"].as_array().unwrap();
        assert_eq!(choices.len(), 3);
    }

    // -------------------------------------------------------------------
    // Entity CRUD tests
    // -------------------------------------------------------------------

    async fn setup_db_with_fields(store: &EntityStore) -> DatabaseSchema {
        let db = store.create_database(test_input("Tasks")).await.unwrap();
        store
            .add_field(
                &db.id,
                CreateFieldInput {
                    name: "Title".into(),
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
                    name: "Priority".into(),
                    slug: None,
                    field_type: FieldType::Number,
                    options: None,
                    required: Some(false),
                    default_value: None,
                    position: None,
                },
            )
            .await
            .unwrap();
        store.get_database(&db.id).await.unwrap()
    }

    #[tokio::test]
    async fn test_create_and_get_entity() {
        let store = setup().await;
        let db = setup_db_with_fields(&store).await;

        let mut fields = std::collections::HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("Buy milk"));
        fields.insert("priority".to_string(), serde_json::json!(3));

        let entity = store.create_entity(&db.id, fields).await.unwrap();
        assert_eq!(entity.database_id, db.id);
        assert_eq!(entity.fields["title"], serde_json::json!("Buy milk"));
        assert_eq!(entity.fields["priority"], serde_json::json!(3.0));

        let fetched = store.get_entity(&db.id, &entity.id).await.unwrap();
        assert_eq!(fetched.fields["title"], serde_json::json!("Buy milk"));
        assert_eq!(fetched.fields["priority"], serde_json::json!(3.0));
    }

    #[tokio::test]
    async fn test_update_entity() {
        let store = setup().await;
        let db = setup_db_with_fields(&store).await;

        let mut fields = std::collections::HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("Original"));
        fields.insert("priority".to_string(), serde_json::json!(1));

        let entity = store.create_entity(&db.id, fields).await.unwrap();

        let mut updates = std::collections::HashMap::new();
        updates.insert("priority".to_string(), serde_json::json!(5));

        let updated = store
            .update_entity(&db.id, &entity.id, updates)
            .await
            .unwrap();
        assert_eq!(updated.fields["title"], serde_json::json!("Original"));
        assert_eq!(updated.fields["priority"], serde_json::json!(5.0));
    }

    #[tokio::test]
    async fn test_delete_entity() {
        let store = setup().await;
        let db = setup_db_with_fields(&store).await;

        let mut fields = std::collections::HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("To delete"));

        let entity = store.create_entity(&db.id, fields).await.unwrap();
        store.delete_entity(&db.id, &entity.id).await.unwrap();
        assert!(store.get_entity(&db.id, &entity.id).await.is_err());
    }

    #[tokio::test]
    async fn test_create_entity_unknown_fields_ignored() {
        let store = setup().await;
        let db = setup_db_with_fields(&store).await;

        let mut fields = std::collections::HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("Valid"));
        fields.insert("nonexistent".to_string(), serde_json::json!("ignored"));

        let entity = store.create_entity(&db.id, fields).await.unwrap();
        assert_eq!(entity.fields["title"], serde_json::json!("Valid"));
        assert!(!entity.fields.contains_key("nonexistent"));
    }

    #[tokio::test]
    async fn test_get_nonexistent_database() {
        let store = setup().await;
        assert!(store.get_database("nonexistent").await.is_err());
    }

    #[tokio::test]
    async fn test_duplicate_slug_rejected() {
        let store = setup().await;
        store.create_database(test_input("Alpha")).await.unwrap();
        assert!(store.create_database(test_input("Alpha")).await.is_err());
    }

    #[tokio::test]
    async fn test_create_entity_missing_required_field() {
        let store = setup().await;
        let db = setup_db_with_fields(&store).await;

        // "title" is required, omit it
        let mut fields = std::collections::HashMap::new();
        fields.insert("priority".to_string(), serde_json::json!(1));

        let result = store.create_entity(&db.id, fields).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Required field"),
            "Expected required field error, got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_create_entity_required_field_with_default_ok() {
        let store = setup().await;
        let db = store
            .create_database(test_input("WithDefault"))
            .await
            .unwrap();
        store
            .add_field(
                &db.id,
                CreateFieldInput {
                    name: "Status".into(),
                    slug: None,
                    field_type: FieldType::Text,
                    options: None,
                    required: Some(true),
                    default_value: Some("open".to_string()),
                    position: None,
                },
            )
            .await
            .unwrap();

        // No fields provided — should succeed because default exists
        let entity = store
            .create_entity(&db.id, std::collections::HashMap::new())
            .await
            .unwrap();
        assert_eq!(entity.fields["status"], serde_json::json!("open"));
    }

    #[tokio::test]
    async fn test_entity_position_is_assigned_on_create() {
        let store = setup().await;
        let db = store.create_database(test_input("Ranked")).await.unwrap();
        store
            .add_field(
                &db.id,
                CreateFieldInput {
                    name: "Title".into(),
                    slug: None,
                    field_type: FieldType::Text,
                    options: None,
                    required: None,
                    default_value: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        let mut positions = Vec::new();
        for i in 0..3 {
            let mut fields = std::collections::HashMap::new();
            fields.insert("title".to_string(), serde_json::json!(format!("Item {i}")));
            let e = store.create_entity(&db.id, fields).await.unwrap();
            assert!(!e.position.is_empty(), "position should be non-empty");
            positions.push(e.position);
        }
        // Positions must be strictly increasing lexicographically
        assert!(positions[0] < positions[1]);
        assert!(positions[1] < positions[2]);
    }

    #[tokio::test]
    async fn test_reorder_entity_between_neighbors() {
        let store = setup().await;
        let db = store.create_database(test_input("DragDrop")).await.unwrap();
        store
            .add_field(
                &db.id,
                CreateFieldInput {
                    name: "Title".into(),
                    slug: None,
                    field_type: FieldType::Text,
                    options: None,
                    required: None,
                    default_value: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        let mut ids = Vec::new();
        for i in 0..3 {
            let mut fields = std::collections::HashMap::new();
            fields.insert("title".to_string(), serde_json::json!(format!("{i}")));
            let e = store.create_entity(&db.id, fields).await.unwrap();
            ids.push(e.id);
        }
        // Move the last item between the first two.
        let moved = store
            .reorder_entity(&db.id, &ids[2], Some(&ids[0]), Some(&ids[1]))
            .await
            .unwrap();
        // Fetch all and confirm order by position.
        let first = store.get_entity(&db.id, &ids[0]).await.unwrap();
        let second = store.get_entity(&db.id, &ids[1]).await.unwrap();
        assert!(first.position < moved.position);
        assert!(moved.position < second.position);
    }

    #[tokio::test]
    async fn test_reorder_views_batch() {
        let store = setup().await;
        let db = store
            .create_database(test_input("ViewOrder"))
            .await
            .unwrap();
        let v1 = store
            .create_view(&db.id, "A", ViewType::Table, ViewConfig::default(), false)
            .await
            .unwrap();
        let v2 = store
            .create_view(&db.id, "B", ViewType::Board, ViewConfig::default(), false)
            .await
            .unwrap();
        let v3 = store
            .create_view(&db.id, "C", ViewType::List, ViewConfig::default(), false)
            .await
            .unwrap();
        // Reverse order: v3, v2, v1
        let reordered = store
            .reorder_views(&db.id, vec![v3.id.clone(), v2.id.clone(), v1.id.clone()])
            .await
            .unwrap();
        assert_eq!(reordered[0].id, v3.id);
        assert_eq!(reordered[1].id, v2.id);
        assert_eq!(reordered[2].id, v1.id);
    }
}
