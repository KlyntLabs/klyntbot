use desktop_shared::errors::ApiError;
use entity_store::{
    evolution::{self, SchemaEvolution},
    CreateDatabaseInput, CreateFieldInput, DatabaseSchema, Entity, FieldDefinition, ViewConfig,
    ViewDefinition, ViewType,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolveAction {
    Accept,
    Dismiss,
}

use crate::state::AppCore;

impl AppCore {
    pub async fn db_list(&self) -> Result<Vec<DatabaseSchema>, ApiError> {
        let store = self.require_entity_store()?;
        store.list_databases().await.map_err(Into::into)
    }

    pub async fn db_get_schema(&self, database_id: String) -> Result<DatabaseSchema, ApiError> {
        let store = self.require_entity_store()?;
        store.get_database(&database_id).await.map_err(Into::into)
    }

    pub async fn db_create_database(
        &self,
        name: String,
        slug: Option<String>,
        icon: Option<String>,
        description: Option<String>,
    ) -> Result<DatabaseSchema, ApiError> {
        let store = self.require_entity_store()?;
        let input = CreateDatabaseInput {
            name,
            slug,
            icon,
            description,
            template_id: None,
        };
        store.create_database(input).await.map_err(Into::into)
    }

    pub async fn db_delete_database(&self, database_id: String) -> Result<(), ApiError> {
        let store = self.require_entity_store()?;
        store
            .delete_database(&database_id)
            .await
            .map_err(Into::into)
    }

    pub async fn db_query(
        &self,
        database_id: String,
        filters: Option<Value>,
        sorts: Option<Value>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Value, ApiError> {
        let store = self.require_entity_store()?;
        let schema = store
            .get_database(&database_id)
            .await
            .map_err(ApiError::from)?;
        let params = entity_store::query::QueryParams {
            filters: filters
                .and_then(|f| serde_json::from_value(f).ok())
                .unwrap_or_default(),
            sorts: sorts
                .and_then(|s| serde_json::from_value(s).ok())
                .unwrap_or_default(),
            limit,
            offset,
        };
        let result = entity_store::query::query_entities(store.pool(), &schema, &params)
            .await
            .map_err(ApiError::from)?;
        Ok(serde_json::json!({
            "entities": result.entities,
            "total": result.total,
        }))
    }

    pub async fn db_create_entity(
        &self,
        database_id: String,
        fields: Value,
    ) -> Result<Entity, ApiError> {
        let store = self.require_entity_store()?;
        let field_map: std::collections::HashMap<String, Value> = serde_json::from_value(fields)
            .map_err(|e| ApiError::new("VALIDATION", e.to_string()))?;
        store
            .create_entity(&database_id, field_map)
            .await
            .map_err(Into::into)
    }

    pub async fn db_update_entity(
        &self,
        database_id: String,
        entity_id: String,
        fields: Value,
    ) -> Result<Entity, ApiError> {
        let store = self.require_entity_store()?;
        let field_map: std::collections::HashMap<String, Value> = serde_json::from_value(fields)
            .map_err(|e| ApiError::new("VALIDATION", e.to_string()))?;
        store
            .update_entity(&database_id, &entity_id, field_map)
            .await
            .map_err(Into::into)
    }

    pub async fn db_delete_entity(
        &self,
        database_id: String,
        entity_id: String,
    ) -> Result<(), ApiError> {
        let store = self.require_entity_store()?;
        store
            .delete_entity(&database_id, &entity_id)
            .await
            .map_err(Into::into)
    }

    pub async fn db_add_field(
        &self,
        database_id: String,
        name: String,
        field_type: String,
        options: Option<Value>,
    ) -> Result<FieldDefinition, ApiError> {
        let store = self.require_entity_store()?;
        let ft: entity_store::FieldType = serde_json::from_value(serde_json::json!(field_type))
            .map_err(|e| ApiError::new("VALIDATION", format!("Invalid field type: {e}")))?;
        let input = CreateFieldInput {
            name,
            slug: None,
            field_type: ft,
            options,
            required: None,
            default_value: None,
            position: None,
        };
        store
            .add_field(&database_id, input)
            .await
            .map_err(Into::into)
    }

    pub async fn db_modify_field(
        &self,
        database_id: String,
        field_id: String,
        name: Option<String>,
        options: Option<Value>,
        required: Option<bool>,
        position: Option<i32>,
    ) -> Result<FieldDefinition, ApiError> {
        let store = self.require_entity_store()?;
        store
            .modify_field(&database_id, &field_id, name, options, required, position)
            .await
            .map_err(Into::into)
    }

    pub async fn db_remove_field(
        &self,
        database_id: String,
        field_id: String,
    ) -> Result<(), ApiError> {
        let store = self.require_entity_store()?;
        store
            .remove_field(&database_id, &field_id, false)
            .await
            .map_err(Into::into)
    }

    // ── Views ──────────────────────────────────────────────

    pub async fn db_list_views(
        &self,
        database_id: String,
    ) -> Result<Vec<ViewDefinition>, ApiError> {
        let store = self.require_entity_store()?;
        store.list_views(&database_id).await.map_err(Into::into)
    }

    pub async fn db_create_view(
        &self,
        database_id: String,
        name: String,
        view_type: ViewType,
        config: Option<ViewConfig>,
        is_default: Option<bool>,
    ) -> Result<ViewDefinition, ApiError> {
        let store = self.require_entity_store()?;
        store
            .create_view(
                &database_id,
                &name,
                view_type,
                config.unwrap_or_default(),
                is_default.unwrap_or(false),
            )
            .await
            .map_err(Into::into)
    }

    pub async fn db_update_view(
        &self,
        database_id: String,
        view_id: String,
        name: Option<String>,
        config: Option<ViewConfig>,
    ) -> Result<ViewDefinition, ApiError> {
        let store = self.require_entity_store()?;
        store
            .update_view(&database_id, &view_id, name.as_deref(), config)
            .await
            .map_err(Into::into)
    }

    pub async fn db_delete_view(
        &self,
        database_id: String,
        view_id: String,
    ) -> Result<(), ApiError> {
        let store = self.require_entity_store()?;
        store
            .delete_view(&database_id, &view_id)
            .await
            .map_err(Into::into)
    }

    // ── Schema Suggestions ────────────────────────────────

    pub async fn db_get_suggestions(
        &self,
        database_id: String,
    ) -> Result<Vec<SchemaEvolution>, ApiError> {
        let store = self.require_entity_store()?;
        evolution::list_pending_evolutions(store.pool(), &database_id)
            .await
            .map_err(Into::into)
    }

    pub async fn db_resolve_suggestion(
        &self,
        database_id: String,
        evolution_id: String,
        action: ResolveAction,
    ) -> Result<(), ApiError> {
        let store = self.require_entity_store()?;
        let pool = store.pool();
        let status = match action {
            ResolveAction::Accept => evolution::STATUS_ACCEPTED,
            ResolveAction::Dismiss => evolution::STATUS_DISMISSED,
        };
        evolution::resolve_evolution(pool, &evolution_id, status)
            .await
            .map_err(ApiError::from)?;
        match action {
            ResolveAction::Accept => evolution::record_acceptance(pool, &database_id).await,
            ResolveAction::Dismiss => evolution::record_dismissal(pool, &database_id).await,
        }
        .map_err(Into::into)
    }

    fn require_entity_store(&self) -> Result<&entity_store::store::EntityStore, ApiError> {
        self.entity_store
            .as_ref()
            .map(|s| s.as_ref())
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Entity store not initialized"))
    }
}
