//! Skill binding — auto-generate and update SKILL.md for each database.

use crate::types::{DatabaseSchema, FieldType};

/// Generate a SKILL.md content string for a database schema.
/// This produces a minimal skill that lists all fields with basic schema_hints
/// derived from field types.
pub fn generate_skill(schema: &DatabaseSchema) -> String {
    let mut hints = String::new();
    let mut format_parts = Vec::new();
    let mut active_filter = None;

    for field in &schema.fields {
        // Skip hidden and computed fields
        if field.hidden || !field.field_type.is_editable() {
            continue;
        }

        format_parts.push(format!("{{{}}}", field.slug));

        // Auto-derive schema hints from field type
        match field.field_type {
            FieldType::Date => {
                hints.push_str(&format!("      {}:\n        temporal: true\n", field.slug));
            }
            FieldType::Select => {
                // If this looks like a lifecycle field (common names)
                if is_lifecycle_field(&field.slug) {
                    hints.push_str(&format!(
                        "      {}:\n        lifecycle: true\n        grouping: true\n",
                        field.slug
                    ));
                    // Try to build active_filter from options
                    if let Some(ref opts) = field.options {
                        if let Some(arr) = opts.as_array() {
                            let done_values: Vec<String> = arr
                                .iter()
                                .filter_map(|v| v.as_str())
                                .filter(|s| is_done_value(s))
                                .map(|s| format!("'{s}'"))
                                .collect();
                            if !done_values.is_empty() {
                                active_filter = Some(format!(
                                    "{} NOT IN ({})",
                                    field.slug,
                                    done_values.join(", ")
                                ));
                            }
                        }
                    }
                } else {
                    hints.push_str(&format!("      {}:\n        grouping: true\n", field.slug));
                }
            }
            FieldType::Number => {
                if is_budget_field(&field.slug) {
                    hints.push_str(&format!(
                        "      {}:\n        budget_field: true\n",
                        field.slug
                    ));
                }
            }
            _ => {}
        }
    }

    let format_str = format_parts.join(" | ");
    let active_filter_str = active_filter.as_deref().unwrap_or("1=1");

    format!(
        r#"---
name: db-{slug}
description: Manages the "{name}" database.
metadata:
  klyntbot:
    type: skill
    tools: [database]
    schema_hints:
{hints}    salience:
      accumulate_on:
        - event: entity_created
          importance: 0.3
        - event: entity_updated
          importance: 0.2
    context_rules:
      active_filter: "{active_filter}"
      sort_by: "created_at DESC"
      max_items: 15
      format: "{format}"
---

You manage the "{name}" database.

## Fields

{field_list}

## Behavior

- When creating entities, fill all required fields.
- When listing entities, use the active filter to show only relevant items.
- When the user asks about this database, query it and present results clearly.
"#,
        slug = schema.slug,
        name = schema.name,
        hints = if hints.is_empty() {
            "      {}\n".to_string()
        } else {
            hints
        },
        active_filter = active_filter_str,
        format = format_str,
        field_list = field_list_markdown(schema),
    )
}

/// Build a markdown field list from schema.
fn field_list_markdown(schema: &DatabaseSchema) -> String {
    schema
        .fields
        .iter()
        .map(|f| {
            let required = if f.required { " *(required)*" } else { "" };
            let hidden = if f.hidden { " *(hidden)*" } else { "" };
            format!(
                "- **{}** (`{}`): {}{}{}",
                f.name,
                f.slug,
                field_type_label(&f.field_type),
                required,
                hidden
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn field_type_label(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Text => "text",
        FieldType::Number => "number",
        FieldType::Select => "select",
        FieldType::MultiSelect => "multi-select",
        FieldType::Date => "date",
        FieldType::Checkbox => "checkbox",
        FieldType::Url => "URL",
        FieldType::Email => "email",
        FieldType::Phone => "phone",
        FieldType::Relation => "relation",
        FieldType::Rollup => "rollup",
        FieldType::Formula => "formula",
        FieldType::CreatedTime => "created time",
        FieldType::LastEdited => "last edited",
        FieldType::Files => "files",
        FieldType::Person => "person",
    }
}

fn is_lifecycle_field(slug: &str) -> bool {
    matches!(slug, "status" | "state" | "stage" | "phase" | "lifecycle")
}

fn is_done_value(val: &str) -> bool {
    let lower = val.to_lowercase();
    matches!(
        lower.as_str(),
        "done" | "completed" | "closed" | "archived" | "cancelled" | "resolved"
    )
}

fn is_budget_field(slug: &str) -> bool {
    slug.contains("amount")
        || slug.contains("budget")
        || slug.contains("cost")
        || slug.contains("price")
        || slug.contains("balance")
}

/// Update an existing skill file's field list and format string when fields change.
/// Returns the updated skill content, or None if the skill doesn't exist or can't be parsed.
pub fn update_skill_fields(existing_skill: &str, schema: &DatabaseSchema) -> Option<String> {
    // Find and replace the ## Fields section
    let fields_marker = "## Fields";
    let behavior_marker = "## Behavior";

    let fields_start = existing_skill.find(fields_marker)?;
    let behavior_start = existing_skill.find(behavior_marker)?;

    if behavior_start <= fields_start {
        return None;
    }

    let new_fields = format!("{}\n\n{}\n\n", fields_marker, field_list_markdown(schema));

    let mut result = String::new();
    result.push_str(&existing_skill[..fields_start]);
    result.push_str(&new_fields);
    result.push_str(&existing_skill[behavior_start..]);

    // Update context_rules format string in frontmatter
    let format_parts: Vec<String> = schema
        .fields
        .iter()
        .filter(|f| !f.hidden && f.field_type.is_editable())
        .map(|f| format!("{{{}}}", f.slug))
        .collect();
    let new_format = format_parts.join(" | ");

    // Simple regex-free replacement of format line in frontmatter
    let result = replace_frontmatter_value(&result, "format:", &format!("\"{new_format}\""));

    Some(result)
}

fn replace_frontmatter_value(content: &str, key: &str, new_value: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.starts_with(key) {
            let indent = &line[..line.len() - line.trim_start().len()];
            *line = format!("{indent}{key} {new_value}");
            break;
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn test_schema() -> DatabaseSchema {
        DatabaseSchema {
            id: "db-001".into(),
            name: "Tasks".into(),
            slug: "tasks".into(),
            icon: None,
            description: Some("Task tracker".into()),
            template_id: None,
            skill_id: None,
            fields: vec![
                FieldDefinition {
                    id: "f1".into(),
                    database_id: "db-001".into(),
                    name: "Title".into(),
                    slug: "title".into(),
                    field_type: FieldType::Text,
                    options: None,
                    position: 0,
                    required: true,
                    hidden: false,
                    ai_managed: false,
                    ai_config: None,
                    default_value: None,
                    created_at: "2026-01-01T00:00:00Z".into(),
                },
                FieldDefinition {
                    id: "f2".into(),
                    database_id: "db-001".into(),
                    name: "Status".into(),
                    slug: "status".into(),
                    field_type: FieldType::Select,
                    options: Some(serde_json::json!(["todo", "doing", "done"])),
                    position: 1,
                    required: false,
                    hidden: false,
                    ai_managed: false,
                    ai_config: None,
                    default_value: None,
                    created_at: "2026-01-01T00:00:00Z".into(),
                },
                FieldDefinition {
                    id: "f3".into(),
                    database_id: "db-001".into(),
                    name: "Due Date".into(),
                    slug: "due_date".into(),
                    field_type: FieldType::Date,
                    options: None,
                    position: 2,
                    required: false,
                    hidden: false,
                    ai_managed: false,
                    ai_config: None,
                    default_value: None,
                    created_at: "2026-01-01T00:00:00Z".into(),
                },
            ],
            views: vec![],
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn generate_skill_produces_valid_frontmatter() {
        let schema = test_schema();
        let skill = generate_skill(&schema);
        assert!(skill.starts_with("---\n"));
        assert!(skill.contains("name: db-tasks"));
        assert!(skill.contains("description: Manages the \"Tasks\" database."));
        assert!(skill.contains("tools: [database]"));
        assert!(skill.contains("schema_hints:"));
        assert!(skill.contains("lifecycle: true"));
        assert!(skill.contains("temporal: true"));
        assert!(skill.contains("## Fields"));
        assert!(skill.contains("**Title** (`title`): text *(required)*"));
        assert!(skill.contains("**Status** (`status`): select"));
    }

    #[test]
    fn generate_skill_active_filter_from_lifecycle() {
        let schema = test_schema();
        let skill = generate_skill(&schema);
        assert!(skill.contains("status NOT IN ('done')"));
    }

    #[test]
    fn update_skill_fields_replaces_field_section() {
        let original = generate_skill(&test_schema());
        let mut updated_schema = test_schema();
        updated_schema.fields.push(FieldDefinition {
            id: "f4".into(),
            database_id: "db-001".into(),
            name: "Priority".into(),
            slug: "priority".into(),
            field_type: FieldType::Select,
            options: None,
            position: 3,
            required: false,
            hidden: false,
            ai_managed: false,
            ai_config: None,
            default_value: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        });
        let updated = update_skill_fields(&original, &updated_schema).unwrap();
        assert!(updated.contains("**Priority** (`priority`): select"));
        assert!(updated.contains("## Behavior"));
    }
}
