//! DatabaseTool — Tool trait implementation dispatching to action handlers.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use common::Result;
use entity_store::store::EntityStore;
use tools_core::params::ParamExtractor;
use tools_core::routing::RoutingContext;
use tools_core::Tool;

use crate::actions;

pub struct DatabaseTool {
    store: Arc<EntityStore>,
}

impl DatabaseTool {
    pub fn new(store: Arc<EntityStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for DatabaseTool {
    fn name(&self) -> &str {
        "database"
    }

    fn description(&self) -> &str {
        "Manage databases, entities, fields, views, and relations. Create any type of database with custom fields."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "create_database", "list_databases", "get_schema", "delete_database",
                        "create", "get", "list", "update", "delete",
                        "add_field", "modify_field", "remove_field",
                        "search",
                        "link", "unlink", "list_relations",
                        "create_view", "update_view", "delete_view", "list_views"
                    ]
                },
                "database_id": { "type": "string", "description": "Database ID (required for most actions)" },
                "entity_id": { "type": "string", "description": "Entity ID" },
                "name": { "type": "string", "description": "Name for database, field, or view" },
                "slug": { "type": "string", "description": "URL-safe identifier" },
                "description": { "type": "string" },
                "icon": { "type": "string" },
                "fields": { "type": "object", "description": "Entity field values as key-value pairs" },
                "field_id": { "type": "string", "description": "Field ID (for modify_field, remove_field)" },
                "field_type": { "type": "string", "description": "Field type (text, number, select, etc.)" },
                "position": { "type": "integer", "description": "Field position" },
                "options": { "type": "object", "description": "Field type-specific options" },
                "required": { "type": "boolean" },
                "query": { "type": "string", "description": "Search query" },
                "filters": { "type": "array", "description": "Filter rules" },
                "sorts": { "type": "array", "description": "Sort rules" },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "target_id": { "type": "string", "description": "Target entity ID for relations" },
                "target_db_id": { "type": "string", "description": "Target database ID for relations" },
                "relation_type": { "type": "string" },
                "view_id": { "type": "string" },
                "view_type": { "type": "string", "description": "View type (table, board, calendar, list, gallery, timeline)" },
                "config": { "type": "object", "description": "View or field configuration" }
            }
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "create_database" => {
                actions::database_ops::create_database(&self.store, &p, &args).await
            }
            "list_databases" => actions::database_ops::list_databases(&self.store).await,
            "get_schema" => actions::database_ops::get_schema(&self.store, &p).await,
            "delete_database" => actions::database_ops::delete_database(&self.store, &p).await,
            "create" => actions::entity_crud::create_entity(&self.store, &p, &args).await,
            "get" => actions::entity_crud::get_entity(&self.store, &p).await,
            "list" => actions::entity_crud::list_entities(&self.store, &p, &args).await,
            "update" => actions::entity_crud::update_entity(&self.store, &p, &args).await,
            "delete" => actions::entity_crud::delete_entity(&self.store, &p).await,
            "add_field" => actions::field_ops::add_field(&self.store, &p, &args).await,
            "modify_field" => actions::field_ops::modify_field(&self.store, &p, &args).await,
            "remove_field" => actions::field_ops::remove_field(&self.store, &p).await,
            "search" => actions::search::search(&self.store, &p).await,
            "link" => actions::relation_ops::link(&self.store, &p).await,
            "unlink" => actions::relation_ops::unlink(&self.store, &p).await,
            "list_relations" => actions::relation_ops::list_relations(&self.store, &p).await,
            "create_view" => actions::view_ops::create_view(&self.store, &p, &args).await,
            "update_view" => actions::view_ops::update_view(&self.store, &p, &args).await,
            "delete_view" => actions::view_ops::delete_view(&self.store, &p).await,
            "list_views" => actions::view_ops::list_views(&self.store, &p).await,
            _ => Err(common::ToolError::InvalidParams(format!("Unknown action: {action}")).into()),
        }
    }
}
