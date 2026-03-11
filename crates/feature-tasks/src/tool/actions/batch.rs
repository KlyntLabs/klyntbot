//! Batch action handler for bulk task operations.

use common::{Result, ToolError};
use tools_core::ParamExtractor;

use super::super::TaskTool;
use storage::TaskPatch;

impl TaskTool {
    /// Handle batch operations: complete, delete, tag, reorder.
    pub(crate) async fn handle_batch(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let ops = p.optional_array("operations")?.ok_or_else(|| {
            ToolError::InvalidParams("'operations' array is required".to_string())
        })?;

        if ops.is_empty() {
            return Err(
                ToolError::InvalidParams("'operations' array cannot be empty".to_string()).into(),
            );
        }

        let mut results = Vec::new();
        let mut errors = Vec::new();

        for (i, op_val) in ops.iter().enumerate() {
            let op_type = op_val
                .get("op")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let id = op_val
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if id.is_empty() {
                errors.push(format!("Op {}: missing 'id'", i + 1));
                continue;
            }

            match op_type {
                "complete" => {
                    let patch = TaskPatch {
                        id: id.to_string(),
                        status: Some("done".to_string()),
                        completed: Some(true),
                        ..Default::default()
                    };
                    match self.repo.update(&patch).await {
                        Ok(row) => results.push(format!("Completed: [{}] {}", row.id, row.title)),
                        Err(e) => errors.push(format!("Complete {}: {}", id, e)),
                    }
                }
                "delete" => match self.repo.delete(id).await {
                    Ok(true) => results.push(format!("Deleted: {}", id)),
                    Ok(false) => errors.push(format!("Delete {}: not found", id)),
                    Err(e) => errors.push(format!("Delete {}: {}", id, e)),
                },
                "tag" => {
                    let tags: Vec<String> = op_val
                        .get("tags")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();

                    if tags.is_empty() {
                        errors.push(format!("Tag {}: no tags provided", id));
                        continue;
                    }

                    // Get existing tags and merge
                    match self.repo.get(id).await {
                        Ok(Some(row)) => {
                            let mut merged = row.tags.clone();
                            for tag in &tags {
                                if !merged.contains(tag) {
                                    merged.push(tag.clone());
                                }
                            }
                            let patch = TaskPatch {
                                id: id.to_string(),
                                tags: Some(merged),
                                ..Default::default()
                            };
                            match self.repo.update(&patch).await {
                                Ok(row) => results.push(format!(
                                    "Tagged: [{}] {} +{}",
                                    row.id,
                                    row.title,
                                    tags.join(",")
                                )),
                                Err(e) => errors.push(format!("Tag {}: {}", id, e)),
                            }
                        }
                        Ok(None) => errors.push(format!("Tag {}: not found", id)),
                        Err(e) => errors.push(format!("Tag {}: {}", id, e)),
                    }
                }
                "reorder" => {
                    let position = op_val
                        .get("position")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32);

                    match position {
                        Some(pos) => {
                            let patch = TaskPatch {
                                id: id.to_string(),
                                position: Some(pos),
                                ..Default::default()
                            };
                            match self.repo.update(&patch).await {
                                Ok(row) => results.push(format!(
                                    "Reordered: [{}] {} -> pos {}",
                                    row.id, row.title, pos
                                )),
                                Err(e) => errors.push(format!("Reorder {}: {}", id, e)),
                            }
                        }
                        None => errors.push(format!("Reorder {}: missing 'position'", id)),
                    }
                }
                _ => {
                    errors.push(format!("Op {}: unknown operation '{}'", i + 1, op_type));
                }
            }
        }

        let mut output = String::new();
        if !results.is_empty() {
            output.push_str(&format!(
                "Batch complete ({} succeeded):\n  {}\n",
                results.len(),
                results.join("\n  ")
            ));
        }
        if !errors.is_empty() {
            output.push_str(&format!(
                "\nBatch errors ({}):\n  {}",
                errors.len(),
                errors.join("\n  ")
            ));
        }

        if output.is_empty() {
            output = "No operations performed.".to_string();
        }

        Ok(output)
    }
}
