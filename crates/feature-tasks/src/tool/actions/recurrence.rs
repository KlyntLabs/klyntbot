//! Recurrence action handlers (recur, list_recurring, delete_recurring).

use chrono::Utc;
use common::{Result, ToolError};
use tools_core::ParamExtractor;

use super::super::TaskTool;
use crate::types::Task;

impl TaskTool {
    pub(crate) async fn handle_recur(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let title = p.required_str("title")?;
        let rule_str = p.required_str("rule")?;
        let area_id = p.required_str("area_id")?;

        // Validate the RRULE
        crate::rrule_utils::validate_rrule(rule_str)?;

        // Compute next instance date
        let now = Utc::now();
        let next_instance_date = crate::rrule_utils::next_occurrence(rule_str, now)?;

        let mut template = Task::default_instance();
        template.title = title.to_string();
        template.area_id = area_id.to_string();
        template.key_result_id = p.optional_str("key_result_id")?.map(String::from);
        template.description = p.optional_str("description")?.map(String::from);
        template.priority = p.optional_u64("priority")?.map(|v| v as i16);
        template.tags = p.string_array_or_empty("tags")?;
        template.project_id = p.optional_str("project_id")?.map(String::from);
        template.recurrence_rule = Some(rule_str.to_string());
        template.is_template = true;
        template.next_instance_date = next_instance_date;

        // New fields
        if let Some(tt) = p.optional_str("task_type")? {
            template.task_type = tt
                .parse()
                .map_err(|e: String| ToolError::InvalidParams(e))?;
        }
        if let Some(el) = p.optional_str("energy_level")? {
            template.energy_level = Some(
                el.parse()
                    .map_err(|e: String| ToolError::InvalidParams(e))?,
            );
        }

        let row = storage::TaskRow::from(&template);
        let created_row = self.repo.add(&row).await?;
        let created = Task::from(created_row);

        let human = crate::rrule_utils::humanize_rrule(rule_str);
        let next_str = created
            .next_instance_date
            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "none".to_string());

        Ok(format!(
            "Recurring template created: {} (ID: {}) | {} | Next: {}",
            created.title, created.id, human, next_str
        ))
    }

    pub(crate) async fn handle_list_recurring(&self) -> Result<String> {
        let rows = self.repo.list_templates().await?;
        let templates: Vec<Task> = rows.into_iter().map(Task::from).collect();

        if templates.is_empty() {
            return Ok("No recurring task templates found.".to_string());
        }

        let mut output = format!("{} recurring template(s):\n\n", templates.len());
        for t in &templates {
            let rule_str = t.recurrence_rule.as_deref().unwrap_or("(no rule)");
            let human = crate::rrule_utils::humanize_rrule(rule_str);
            let next_str = t
                .next_instance_date
                .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| "none".to_string());

            output.push_str(&format!(
                "- [{}] {} | {} | Next: {}\n",
                t.id, t.title, human, next_str
            ));
            if let Some(ref desc) = t.description {
                output.push_str(&format!("  Description: {}\n", desc));
            }
            if let Some(pri) = t.priority {
                output.push_str(&format!("  Priority: {}\n", pri));
            }
            if !t.tags.is_empty() {
                output.push_str(&format!("  Tags: {}\n", t.tags.join(", ")));
            }
        }

        Ok(output)
    }

    pub(crate) async fn handle_delete_recurring(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let template_id = p.required_str("template_id")?;

        match self.repo.get(template_id).await? {
            Some(t) if t.is_template => {
                self.repo.delete(template_id).await?;
                Ok(format!(
                    "Recurring template deleted: [{}] {}. Existing instances are now standalone tasks.",
                    t.id, t.title
                ))
            }
            Some(_) => Err(ToolError::InvalidParams(format!(
                "Task {} is not a recurring template. Use 'delete' for regular tasks.",
                template_id
            ))
            .into()),
            None => Err(
                ToolError::ExecutionFailed(format!("Template not found: {}", template_id)).into(),
            ),
        }
    }
}
