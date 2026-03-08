//! NotesTool — feature-notes Tool implementation.

use async_trait::async_trait;
use serde_json::Value;

use common::{Result, ToolError};
use tools_core::{ParamExtractor, RoutingContext, Tool};

use crate::models::{Note, NoteRow, Notebook, NotebookRow};
use crate::repo::{utc_now_str, NoteRepo};

pub struct NotesTool {
    repo: NoteRepo,
}

impl NotesTool {
    pub fn new(repo: NoteRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl Tool for NotesTool {
    fn name(&self) -> &str {
        "notes"
    }

    fn description(&self) -> &str {
        "Manage notes and notebooks. Actions: create_note, get_note, update_note, delete_note, list_notes, search_notes, tag_note, link_notes, create_notebook, list_notebooks."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "create_note", "get_note", "update_note", "delete_note",
                        "list_notes", "search_notes", "tag_note", "link_notes",
                        "create_notebook", "list_notebooks"
                    ],
                    "description": "Action to perform"
                },
                "id": { "type": "string", "description": "Note or notebook ID" },
                "title": { "type": "string", "description": "Title" },
                "body": { "type": "string", "description": "Note body (markdown)" },
                "notebook_id": { "type": "string", "description": "Notebook ID to place note in" },
                "pinned": { "type": "boolean", "description": "Pin the note" },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags for the note"
                },
                "query": { "type": "string", "description": "Search query" },
                "target_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Target note IDs for linking"
                },
                "icon": { "type": "string", "description": "Notebook icon emoji" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "create_note" => self.handle_create_note(&p).await,
            "get_note" => self.handle_get_note(&p).await,
            "update_note" => self.handle_update_note(&p).await,
            "delete_note" => self.handle_delete_note(&p).await,
            "list_notes" => self.handle_list_notes(&p).await,
            "search_notes" => self.handle_search_notes(&p).await,
            "tag_note" => self.handle_tag_note(&p).await,
            "link_notes" => self.handle_link_notes(&p).await,
            "create_notebook" => self.handle_create_notebook(&p).await,
            "list_notebooks" => self.handle_list_notebooks().await,
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {action}")).into()),
        }
    }
}

impl NotesTool {
    async fn maybe_set_tags(&self, p: &ParamExtractor<'_>, id: &str) -> Result<()> {
        let tags = p.string_array_or_empty("tags")?;
        if !tags.is_empty() {
            self.repo.set_tags(id, &tags).await?;
        }
        Ok(())
    }

    async fn handle_create_note(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let title = p.required_str("title")?;
        let body = p.optional_str("body")?.unwrap_or("");
        let notebook_id = p.optional_str("notebook_id")?;
        let now = utc_now_str();
        let id = uuid::Uuid::new_v4().to_string();

        let row = NoteRow {
            id: id.clone(),
            notebook_id: notebook_id.map(String::from),
            title: title.to_string(),
            body: body.to_string(),
            body_html: None,
            pinned: 0,
            archived: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        self.repo.create_note(&row).await?;
        self.maybe_set_tags(p, &id).await?;

        Ok(format!("Created note \"{title}\" (id: {id})"))
    }

    async fn handle_get_note(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let row = self
            .repo
            .get_note(id)
            .await?
            .ok_or_else(|| ToolError::InvalidParams(format!("Note not found: {id}")))?;
        let tags = self.repo.get_tags(id).await?;
        let note = Note::from_row(row, tags);
        let links = self.repo.get_links_from(id).await?;

        let mut out = format!("# {}\n\n{}", note.title, note.body);
        if !note.tags.is_empty() {
            out.push_str(&format!("\n\nTags: {}", note.tags.join(", ")));
        }
        if note.pinned {
            out.push_str("\nPinned");
        }
        if !links.is_empty() {
            let link_ids: Vec<&str> = links.iter().map(|l| l.target_id.as_str()).collect();
            out.push_str(&format!("\n\nLinks to: {}", link_ids.join(", ")));
        }
        if let Some(nb) = &note.notebook_id {
            out.push_str(&format!("\nNotebook: {nb}"));
        }
        Ok(out)
    }

    async fn handle_update_note(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let title = p.optional_str("title")?;
        let body = p.optional_str("body")?;
        let pinned = p.optional_bool("pinned")?;
        let notebook_id = p.optional_str("notebook_id")?.map(Some);

        let updated = self
            .repo
            .update_note(id, title, body, None, pinned, notebook_id)
            .await?;
        self.maybe_set_tags(p, id).await?;

        Ok(format!(
            "Updated note \"{}\" (id: {})",
            updated.title, updated.id
        ))
    }

    async fn handle_delete_note(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        if self.repo.delete_note(id).await? {
            Ok(format!("Deleted note {id}"))
        } else {
            Err(ToolError::InvalidParams(format!("Note not found: {id}")).into())
        }
    }

    async fn handle_list_notes(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let notebook_id = p.optional_str("notebook_id")?;
        let rows = self.repo.list_notes(notebook_id).await?;

        if rows.is_empty() {
            return Ok("No notes found.".to_string());
        }

        let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
        let tags_map = self.repo.get_tags_batch(&ids).await?;

        let mut out = format!("{} note(s):\n", rows.len());
        for row in &rows {
            let tags = tags_map.get(&row.id);
            let tag_str = tags
                .map(|t| {
                    if t.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", t.join(", "))
                    }
                })
                .unwrap_or_default();
            let pin = if row.pinned != 0 { " pinned" } else { "" };
            let preview = match row.body.char_indices().nth(80) {
                Some((i, _)) => format!("{}...", &row.body[..i]),
                None => row.body.clone(),
            };
            out.push_str(&format!(
                "\n- **{}**{}{} ({})\n  {}\n",
                row.title, pin, tag_str, row.id, preview
            ));
        }
        Ok(out)
    }

    async fn handle_search_notes(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let query = p.required_str("query")?;
        let rows = self.repo.search_notes(query).await?;

        if rows.is_empty() {
            return Ok(format!("No notes matching \"{query}\"."));
        }

        let mut out = format!("{} result(s) for \"{query}\":\n", rows.len());
        for row in &rows {
            out.push_str(&format!("\n- **{}** ({})", row.title, row.id));
        }
        Ok(out)
    }

    async fn handle_tag_note(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let tags = p.string_array_or_empty("tags")?;

        // Verify note exists
        self.repo
            .get_note(id)
            .await?
            .ok_or_else(|| ToolError::InvalidParams(format!("Note not found: {id}")))?;

        self.repo.set_tags(id, &tags).await?;
        Ok(format!("Set {} tag(s) on note {id}", tags.len()))
    }

    async fn handle_link_notes(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let target_ids = p.string_array_or_empty("target_ids")?;

        // Verify source exists
        self.repo
            .get_note(id)
            .await?
            .ok_or_else(|| ToolError::InvalidParams(format!("Source note not found: {id}")))?;

        self.repo.set_links(id, &target_ids).await?;
        Ok(format!(
            "Linked note {id} to {} target(s)",
            target_ids.len()
        ))
    }

    async fn handle_create_notebook(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let title = p.required_str("title")?;
        let icon = p.optional_str("icon")?;
        let now = utc_now_str();
        let id = uuid::Uuid::new_v4().to_string();

        let row = NotebookRow {
            id: id.clone(),
            parent_id: None,
            title: title.to_string(),
            icon: icon.map(String::from),
            sort_order: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        self.repo.create_notebook(&row).await?;
        Ok(format!("Created notebook \"{title}\" (id: {id})"))
    }

    async fn handle_list_notebooks(&self) -> Result<String> {
        let rows = self.repo.list_notebooks().await?;
        if rows.is_empty() {
            return Ok("No notebooks.".to_string());
        }

        let counts = self.repo.count_notes_by_notebook().await?;
        let mut out = format!("{} notebook(s):\n", rows.len());
        for nb in &rows {
            let count = counts.get(&nb.id).copied().unwrap_or(0);
            let icon = nb.icon.as_deref().unwrap_or("N");
            let notebook: Notebook = nb.clone().into();
            out.push_str(&format!(
                "\n- {icon} **{}** ({count} notes) — {}\n",
                notebook.title, nb.id
            ));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{ChannelName, ChatId};

    fn ctx() -> RoutingContext {
        RoutingContext::new(ChannelName::from("test"), ChatId::from("123"))
    }

    async fn setup() -> NotesTool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        let sql = crate::NotesFeature::migration_sql();
        sqlx::query(sql).execute(&pool).await.unwrap();
        NotesTool::new(crate::repo::NoteRepo::new(pool))
    }

    #[tokio::test]
    async fn test_create_and_get_note() {
        let tool = setup().await;
        let result = tool
            .execute(
                serde_json::json!({"action": "create_note", "title": "Test Note", "body": "Hello world"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(result.contains("Test Note"));
        assert!(result.contains("Created note"));
    }

    #[tokio::test]
    async fn test_list_notes_empty() {
        let tool = setup().await;
        let result = tool
            .execute(serde_json::json!({"action": "list_notes"}), &ctx())
            .await
            .unwrap();
        assert!(result.contains("No notes") || result.contains("0 notes"));
    }

    #[tokio::test]
    async fn test_search_notes() {
        let tool = setup().await;
        tool.execute(
            serde_json::json!({"action": "create_note", "title": "Rust Tips", "body": "Use pattern matching"}),
            &ctx(),
        )
        .await
        .unwrap();

        let result = tool
            .execute(
                serde_json::json!({"action": "search_notes", "query": "Rust"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(result.contains("Rust Tips"));
    }

    #[tokio::test]
    async fn test_create_and_list_notebooks() {
        let tool = setup().await;
        tool.execute(
            serde_json::json!({"action": "create_notebook", "title": "Work"}),
            &ctx(),
        )
        .await
        .unwrap();

        let result = tool
            .execute(serde_json::json!({"action": "list_notebooks"}), &ctx())
            .await
            .unwrap();
        assert!(result.contains("Work"));
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let tool = setup().await;
        let result = tool
            .execute(serde_json::json!({"action": "fly"}), &ctx())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_action() {
        let tool = setup().await;
        let result = tool
            .execute(serde_json::json!({"title": "oops"}), &ctx())
            .await;
        assert!(result.is_err());
    }
}
