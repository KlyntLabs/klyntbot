//! Template manifest types and installation.

use serde::{Deserialize, Serialize};

use crate::store::EntityStore;
use crate::types::*;
use crate::views;
use common::Result;

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDatabaseDef {
    pub name: String,
    pub slug: String,
    pub fields: Vec<TemplateFieldDef>,
    #[serde(default)]
    pub views: Vec<TemplateViewDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFieldDef {
    pub name: String,
    pub slug: String,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    #[serde(default)]
    pub options: Option<serde_json::Value>,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateViewDef {
    pub name: String,
    #[serde(rename = "viewType")]
    pub view_type: ViewType,
    #[serde(default)]
    pub is_default: Option<bool>,
    #[serde(default)]
    pub config: ViewConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRelationDef {
    pub source_db_slug: String,
    pub target_db_slug: String,
    pub relation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDashboardDef {
    pub name: String,
    pub widgets: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Installation
// ---------------------------------------------------------------------------

/// Install a template: create databases, fields, and views. Returns created database IDs.
pub async fn install_template(
    store: &EntityStore,
    manifest: &TemplateManifest,
) -> Result<Vec<String>> {
    let mut db_ids = Vec::new();

    for db_def in &manifest.databases {
        let db = store
            .create_database(CreateDatabaseInput {
                name: db_def.name.clone(),
                slug: Some(db_def.slug.clone()),
                icon: manifest.icon.clone(),
                description: Some(manifest.description.clone()),
                template_id: Some(manifest.name.clone()),
            })
            .await?;

        for (i, field_def) in db_def.fields.iter().enumerate() {
            store
                .add_field(
                    &db.id,
                    CreateFieldInput {
                        name: field_def.name.clone(),
                        slug: Some(field_def.slug.clone()),
                        field_type: field_def.field_type.clone(),
                        options: field_def.options.clone(),
                        required: field_def.required,
                        default_value: field_def.default_value.clone(),
                        position: Some(i as i32),
                    },
                )
                .await?;
        }

        for view_def in &db_def.views {
            views::create_view(
                store.pool(),
                &db.id,
                &view_def.name,
                view_def.view_type.clone(),
                view_def.config.clone(),
                view_def.is_default.unwrap_or(false),
            )
            .await?;
        }

        db_ids.push(db.id);
    }

    Ok(db_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> EntityStore {
        crate::test_helpers::setup_test_store().await
    }

    #[tokio::test]
    async fn test_install_template() {
        let store = setup().await;

        let manifest_json = r#"{
            "name": "CRM",
            "description": "Simple CRM template",
            "icon": "📇",
            "version": "1.0.0",
            "databases": [
                {
                    "name": "Contacts",
                    "slug": "contacts",
                    "fields": [
                        { "name": "Name", "slug": "name", "type": "text", "required": true },
                        { "name": "Email", "slug": "email", "type": "email" },
                        { "name": "Company", "slug": "company", "type": "text" }
                    ],
                    "views": [
                        {
                            "name": "All Contacts",
                            "viewType": "table",
                            "is_default": true
                        },
                        {
                            "name": "By Company",
                            "viewType": "board",
                            "config": { "groupBy": "company" }
                        }
                    ]
                },
                {
                    "name": "Deals",
                    "slug": "deals",
                    "fields": [
                        { "name": "Title", "slug": "title", "type": "text", "required": true },
                        { "name": "Value", "slug": "value", "type": "number" }
                    ]
                }
            ],
            "relations": [
                { "source_db_slug": "deals", "target_db_slug": "contacts", "relation_type": "belongs_to" }
            ]
        }"#;

        let manifest: TemplateManifest = serde_json::from_str(manifest_json).unwrap();
        let db_ids = install_template(&store, &manifest).await.unwrap();
        assert_eq!(db_ids.len(), 2);

        // Verify first database
        let contacts = store.get_database(&db_ids[0]).await.unwrap();
        assert_eq!(contacts.name, "Contacts");
        assert_eq!(contacts.fields.len(), 3);
        assert_eq!(contacts.fields[0].slug, "name");
        assert!(contacts.fields[0].required);
        assert_eq!(contacts.views.len(), 2);
        assert_eq!(contacts.views[0].name, "All Contacts");
        assert!(contacts.views[0].is_default);
        assert_eq!(contacts.views[1].view_type, ViewType::Board);

        // Verify second database
        let deals = store.get_database(&db_ids[1]).await.unwrap();
        assert_eq!(deals.name, "Deals");
        assert_eq!(deals.fields.len(), 2);
    }
}
