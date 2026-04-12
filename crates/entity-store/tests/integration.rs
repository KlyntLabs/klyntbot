//! Integration tests for the entity-store crate.

use entity_store::{
    query::{query_entities, QueryParams},
    relations,
    store::EntityStore,
    templates, views, CreateDatabaseInput, CreateFieldInput, FieldType, FilterOp, FilterRule,
    ViewConfig, ViewType,
};
use std::collections::HashMap;

async fn setup() -> EntityStore {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let sql = include_str!("../migrations/001_entity_store.sql");
    for statement in sql.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed).execute(pool.inner()).await.unwrap();
        }
    }
    EntityStore::new(pool.inner().clone())
}

#[tokio::test]
async fn full_lifecycle() {
    let store = setup().await;

    // Create database
    let db = store
        .create_database(CreateDatabaseInput {
            name: "Tasks".into(),
            slug: None,
            icon: Some("✓".into()),
            description: Some("Personal tasks".into()),
            template_id: None,
        })
        .await
        .unwrap();
    assert_eq!(db.slug, "tasks");

    // Add fields
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
                name: "Status".into(),
                slug: None,
                field_type: FieldType::Select,
                options: Some(serde_json::json!(["todo", "doing", "done"])),
                required: None,
                default_value: Some("todo".into()),
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
                required: None,
                default_value: None,
                position: None,
            },
        )
        .await
        .unwrap();

    // Create entities
    let e1 = store
        .create_entity(
            &db.id,
            HashMap::from([
                ("title".into(), serde_json::json!("Buy groceries")),
                ("status".into(), serde_json::json!("todo")),
                ("priority".into(), serde_json::json!(3)),
            ]),
        )
        .await
        .unwrap();

    let _e2 = store
        .create_entity(
            &db.id,
            HashMap::from([
                ("title".into(), serde_json::json!("Fix bug")),
                ("status".into(), serde_json::json!("doing")),
                ("priority".into(), serde_json::json!(1)),
            ]),
        )
        .await
        .unwrap();

    let e3 = store
        .create_entity(
            &db.id,
            HashMap::from([
                ("title".into(), serde_json::json!("Write docs")),
                ("status".into(), serde_json::json!("done")),
                ("priority".into(), serde_json::json!(2)),
            ]),
        )
        .await
        .unwrap();

    // Query with filter
    let schema = store.get_database(&db.id).await.unwrap();
    let result = query_entities(
        store.pool(),
        &schema,
        &QueryParams {
            filters: vec![FilterRule {
                field: "status".into(),
                op: FilterOp::Eq,
                value: serde_json::json!("todo"),
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(
        result.entities[0].fields["title"],
        serde_json::json!("Buy groceries")
    );

    // Update entity
    let updated = store
        .update_entity(
            &db.id,
            &e1.id,
            HashMap::from([("status".into(), serde_json::json!("doing"))]),
        )
        .await
        .unwrap();
    assert_eq!(updated.fields["status"], serde_json::json!("doing"));

    // Add view
    let view = views::create_view(
        store.pool(),
        &db.id,
        "Board",
        ViewType::Board,
        ViewConfig {
            group_by: Some("status".into()),
            card_fields: vec!["title".into(), "priority".into()],
            ..Default::default()
        },
        false,
    )
    .await
    .unwrap();
    assert_eq!(view.name, "Board");

    // Delete entity
    store.delete_entity(&db.id, &e3.id).await.unwrap();
    assert!(store.get_entity(&db.id, &e3.id).await.is_err());

    // Schema has 3 fields
    let schema = store.get_database(&db.id).await.unwrap();
    assert_eq!(schema.fields.len(), 3);
    assert_eq!(schema.views.len(), 1);

    // Delete database
    store.delete_database(&db.id).await.unwrap();
    assert!(store.get_database(&db.id).await.is_err());
}

#[tokio::test]
async fn template_installation() {
    let store = setup().await;

    let manifest = templates::TemplateManifest {
        name: "Test Template".into(),
        description: "A test".into(),
        icon: None,
        version: "1.0.0".into(),
        databases: vec![templates::TemplateDatabaseDef {
            name: "Items".into(),
            slug: "items".into(),
            fields: vec![templates::TemplateFieldDef {
                name: "Name".into(),
                slug: "name".into(),
                field_type: FieldType::Text,
                options: None,
                required: Some(true),
                default_value: None,
            }],
            views: vec![templates::TemplateViewDef {
                name: "All Items".into(),
                view_type: ViewType::Table,
                is_default: Some(true),
                config: Default::default(),
            }],
        }],
        relations: vec![],
        skill_dir: None,
        dashboards: vec![],
    };

    let db_ids = templates::install_template(&store, &manifest)
        .await
        .unwrap();
    assert_eq!(db_ids.len(), 1);

    let db = store.get_database(&db_ids[0]).await.unwrap();
    assert_eq!(db.name, "Items");
    assert_eq!(db.fields.len(), 1);
    assert_eq!(db.views.len(), 1);
}

#[tokio::test]
async fn cross_database_relations() {
    let store = setup().await;

    let db1 = store
        .create_database(CreateDatabaseInput {
            name: "Tasks".into(),
            slug: None,
            icon: None,
            description: None,
            template_id: None,
        })
        .await
        .unwrap();
    let db2 = store
        .create_database(CreateDatabaseInput {
            name: "Projects".into(),
            slug: None,
            icon: None,
            description: None,
            template_id: None,
        })
        .await
        .unwrap();

    store
        .add_field(
            &db1.id,
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
    store
        .add_field(
            &db2.id,
            CreateFieldInput {
                name: "Name".into(),
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

    let task = store
        .create_entity(
            &db1.id,
            HashMap::from([("title".into(), serde_json::json!("Fix bug"))]),
        )
        .await
        .unwrap();
    let project = store
        .create_entity(
            &db2.id,
            HashMap::from([("name".into(), serde_json::json!("Klyntbot"))]),
        )
        .await
        .unwrap();

    let rel = relations::create_relation(
        store.pool(),
        relations::CreateRelationInput {
            source_id: &task.id,
            source_db_id: &db1.id,
            target_id: &project.id,
            target_db_id: &db2.id,
            relation_type: "belongs_to",
            inferred: false,
            confidence: None,
        },
    )
    .await
    .unwrap();

    let rels = relations::list_relations_for_entity(store.pool(), &task.id, &db1.id)
        .await
        .unwrap();
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].target_id, project.id);

    relations::delete_relation(store.pool(), &rel.id)
        .await
        .unwrap();
    let rels = relations::list_relations_for_entity(store.pool(), &task.id, &db1.id)
        .await
        .unwrap();
    assert!(rels.is_empty());
}
