use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported field types — maps to Notion property types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Number,
    Select,
    MultiSelect,
    Date,
    Checkbox,
    Url,
    Email,
    Phone,
    Relation,
    Rollup,
    Formula,
    CreatedTime,
    LastEdited,
    Files,
    Person,
}

impl FieldType {
    /// SQLite column type for this field.
    pub fn sqlite_type(&self) -> &'static str {
        match self {
            Self::Number => "REAL",
            Self::Checkbox => "INTEGER",
            _ => "TEXT",
        }
    }

    /// Whether this field type is user-editable (not computed).
    pub fn is_editable(&self) -> bool {
        !matches!(
            self,
            Self::Rollup | Self::Formula | Self::CreatedTime | Self::LastEdited
        )
    }
}

/// Definition of a single field in a database schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDefinition {
    pub id: String,
    pub database_id: String,
    pub name: String,
    pub slug: String,
    pub field_type: FieldType,
    #[serde(default)]
    pub options: Option<serde_json::Value>,
    pub position: i32,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub ai_managed: bool,
    #[serde(default)]
    pub ai_config: Option<serde_json::Value>,
    #[serde(default)]
    pub default_value: Option<String>,
    pub created_at: String,
}

/// Schema of a database — metadata + ordered field definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSchema {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub template_id: Option<String>,
    pub skill_id: Option<String>,
    pub fields: Vec<FieldDefinition>,
    pub views: Vec<ViewDefinition>,
    pub created_at: String,
    pub updated_at: String,
}

/// A single entity (row) in a database. Fields stored as slug->value map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub database_id: String,
    pub fields: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub position: String,
    pub created_at: String,
    pub updated_at: String,
}

/// View type — how to render a database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewType {
    Table,
    Board,
    Calendar,
    List,
    Gallery,
    Timeline,
}

/// A named view configuration for a database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewDefinition {
    pub id: String,
    pub database_id: String,
    pub name: String,
    pub view_type: ViewType,
    pub config: ViewConfig,
    pub position: i32,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// View-specific configuration: filters, sorts, visible fields, grouping.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewConfig {
    #[serde(default)]
    pub filters: Vec<FilterRule>,
    #[serde(default)]
    pub filter: Option<FilterGroup>,
    #[serde(default)]
    pub sorts: Vec<SortRule>,
    #[serde(default)]
    pub visible_fields: Vec<String>,
    pub group_by: Option<String>,
    pub calendar_field: Option<String>,
    pub gallery_field: Option<String>,
    #[serde(default)]
    pub card_fields: Vec<String>,
    #[serde(default)]
    pub layout: HashMap<String, serde_json::Value>,
}

/// A single filter condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterRule {
    pub field: String,
    pub op: FilterOp,
    pub value: serde_json::Value,
}

/// Nested filter node — either a leaf rule or a logical group.
/// Max nesting depth is enforced in the query engine (matches Notion's 3-level limit).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FilterNode {
    Rule(FilterRule),
    Group(FilterGroup),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterGroup {
    pub op: LogicOp,
    #[serde(default)]
    pub nodes: Vec<FilterNode>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogicOp {
    And,
    Or,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
    NotContains,
    IsEmpty,
    IsNotEmpty,
    In,
    NotIn,
}

/// A sort specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortRule {
    pub field: String,
    #[serde(default = "default_sort_dir")]
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    Desc,
}

fn default_sort_dir() -> SortDirection {
    SortDirection::Asc
}

/// Cross-database entity relation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityRelation {
    pub id: String,
    pub source_id: String,
    pub source_db_id: String,
    pub target_id: String,
    pub target_db_id: String,
    pub relation_type: String,
    pub inferred: bool,
    pub confidence: Option<f64>,
    pub created_at: String,
}

/// Dashboard with widgets querying any database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub id: String,
    pub name: String,
    pub widgets: Vec<WidgetDefinition>,
    pub position: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetDefinition {
    pub id: String,
    pub widget_type: String,
    pub database_id: String,
    pub config: serde_json::Value,
    pub position: GridPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridPosition {
    pub row: i32,
    pub col: i32,
    pub width: i32,
    pub height: i32,
}

/// Input for creating a new database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDatabaseInput {
    pub name: String,
    pub slug: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub template_id: Option<String>,
}

/// Input for creating a new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFieldInput {
    pub name: String,
    pub slug: Option<String>,
    pub field_type: FieldType,
    pub options: Option<serde_json::Value>,
    pub required: Option<bool>,
    pub default_value: Option<String>,
    pub position: Option<i32>,
}

/// Format an entity's fields as a pipe-separated string for display.
/// Skips null values. Optionally resolves slugs to display names using field definitions.
pub fn format_entity_fields(
    fields: &HashMap<String, serde_json::Value>,
    field_defs: Option<&[FieldDefinition]>,
) -> String {
    fields
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| {
            let label = field_defs
                .and_then(|defs| defs.iter().find(|f| f.slug == *k))
                .map(|f| f.name.as_str())
                .unwrap_or(k.as_str());
            let val = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            format!("{label}: {val}")
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_type_sqlite_mapping() {
        assert_eq!(FieldType::Text.sqlite_type(), "TEXT");
        assert_eq!(FieldType::Number.sqlite_type(), "REAL");
        assert_eq!(FieldType::Checkbox.sqlite_type(), "INTEGER");
        assert_eq!(FieldType::Date.sqlite_type(), "TEXT");
        assert_eq!(FieldType::Select.sqlite_type(), "TEXT");
    }

    #[test]
    fn field_type_editability() {
        assert!(FieldType::Text.is_editable());
        assert!(FieldType::Number.is_editable());
        assert!(!FieldType::Rollup.is_editable());
        assert!(!FieldType::Formula.is_editable());
        assert!(!FieldType::CreatedTime.is_editable());
        assert!(!FieldType::LastEdited.is_editable());
    }

    #[test]
    fn field_type_serde_roundtrip() {
        let ft = FieldType::MultiSelect;
        let json = serde_json::to_string(&ft).unwrap();
        assert_eq!(json, "\"multi_select\"");
        let parsed: FieldType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ft);
    }

    #[test]
    fn view_config_defaults() {
        let config: ViewConfig = serde_json::from_str("{}").unwrap();
        assert!(config.filters.is_empty());
        assert!(config.sorts.is_empty());
        assert!(config.visible_fields.is_empty());
        assert!(config.group_by.is_none());
    }
}
