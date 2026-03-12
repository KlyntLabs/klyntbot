//! Repository for notes, notebooks, tags, links, and version history.

mod links;
mod notebooks;
mod notes;
mod tags;

use sqlx::SqlitePool;

/// Convert a tri-state `Option<Option<&str>>` to a sentinel value for SQL:
/// - `None` → `None`  (bind NULL → COALESCE keeps existing)
/// - `Some(None)` → `Some("")` (bind "" → CASE sets NULL)
/// - `Some(Some(v))` → `Some(v)` (bind v → CASE sets v)
pub(crate) fn nullable_to_sentinel(v: Option<Option<&str>>) -> Option<&str> {
    v.map(|opt| opt.unwrap_or(""))
}

/// UTC "now" as an ISO-8601 string (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn utc_now_str() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[derive(Debug, Clone)]
pub struct NoteRepo {
    pub(crate) pool: SqlitePool,
}

impl NoteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    async fn setup() -> NoteRepo {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        let sql = crate::NotesFeature::migration_sql();
        sqlx::query(sql).execute(&pool).await.unwrap();
        NoteRepo::new(pool)
    }

    fn sample_note(id: &str, title: &str) -> NoteRow {
        let now = utc_now_str();
        NoteRow {
            id: id.to_string(),
            notebook_id: None,
            title: title.to_string(),
            body: "hello world".to_string(),
            body_html: None,
            pinned: 0,
            archived: 0,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_note() {
        let repo = setup().await;
        let row = sample_note("n1", "Test Note");
        repo.create_note(&row).await.unwrap();

        let fetched = repo.get_note("n1").await.unwrap().unwrap();
        assert_eq!(fetched.title, "Test Note");
        assert_eq!(fetched.body, "hello world");
    }

    #[tokio::test]
    async fn test_list_notes() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "First")).await.unwrap();
        repo.create_note(&sample_note("n2", "Second"))
            .await
            .unwrap();

        let notes = repo.list_notes(None).await.unwrap();
        assert_eq!(notes.len(), 2);
    }

    #[tokio::test]
    async fn test_update_note() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Original"))
            .await
            .unwrap();

        let updated = repo
            .update_note("n1", Some("Updated"), None, None, Some(true), None)
            .await
            .unwrap();
        assert_eq!(updated.title, "Updated");
        assert_eq!(updated.pinned, 1);
    }

    #[tokio::test]
    async fn test_delete_note() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "To Delete"))
            .await
            .unwrap();

        assert!(repo.delete_note("n1").await.unwrap());
        assert!(repo.get_note("n1").await.unwrap().is_none());
        assert!(!repo.delete_note("n1").await.unwrap());
    }

    #[tokio::test]
    async fn test_health_check() {
        let repo = setup().await;
        repo.check_health().await.unwrap();
        repo.create_note(&sample_note("n1", "One")).await.unwrap();
        repo.check_health().await.unwrap();
    }

    #[tokio::test]
    async fn test_search_notes() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Rust Programming"))
            .await
            .unwrap();
        repo.create_note(&sample_note("n2", "Go Programming"))
            .await
            .unwrap();

        let results = repo.search_notes("Rust").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming");
    }

    #[tokio::test]
    async fn test_tags() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Tagged"))
            .await
            .unwrap();

        repo.set_tags("n1", &["rust".to_string(), "notes".to_string()])
            .await
            .unwrap();
        let tags = repo.get_tags("n1").await.unwrap();
        assert_eq!(tags, vec!["notes", "rust"]);

        // Replace tags
        repo.set_tags("n1", &["only-this".to_string()])
            .await
            .unwrap();
        let tags = repo.get_tags("n1").await.unwrap();
        assert_eq!(tags, vec!["only-this"]);
    }

    #[tokio::test]
    async fn test_notebooks_crud() {
        let repo = setup().await;
        let now = utc_now_str();
        let nb = NotebookRow {
            id: "nb1".to_string(),
            parent_id: None,
            title: "My Notebook".to_string(),
            icon: Some("\u{1f4d3}".to_string()),
            sort_order: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        repo.create_notebook(&nb).await.unwrap();

        let notebooks = repo.list_notebooks().await.unwrap();
        assert_eq!(notebooks.len(), 1);
        assert_eq!(notebooks[0].title, "My Notebook");

        let updated = repo
            .update_notebook("nb1", Some("Renamed"), None, None)
            .await
            .unwrap();
        assert_eq!(updated.title, "Renamed");

        assert!(repo.delete_notebook("nb1").await.unwrap());
        assert!(repo.list_notebooks().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_links() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Source"))
            .await
            .unwrap();
        repo.create_note(&sample_note("n2", "Target A"))
            .await
            .unwrap();
        repo.create_note(&sample_note("n3", "Target B"))
            .await
            .unwrap();

        repo.set_links("n1", &["n2".to_string(), "n3".to_string()])
            .await
            .unwrap();

        let from = repo.get_links_from("n1").await.unwrap();
        assert_eq!(from.len(), 2);

        let to = repo.get_links_to("n2").await.unwrap();
        assert_eq!(to.len(), 1);
        assert_eq!(to[0].source_id, "n1");

        let all = repo.get_all_links().await.unwrap();
        assert_eq!(all.len(), 2);

        // Replace links
        repo.set_links("n1", &["n3".to_string()]).await.unwrap();
        assert_eq!(repo.get_links_from("n1").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_versions() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Versioned"))
            .await
            .unwrap();

        for i in 0..5 {
            let v = NoteVersionRow {
                id: format!("v{i}"),
                note_id: "n1".to_string(),
                body: format!("version {i}"),
                created_at: format!("2026-01-0{i}T00:00:00Z", i = i + 1),
            };
            repo.create_version(&v).await.unwrap();
        }

        let versions = repo.list_versions("n1").await.unwrap();
        assert_eq!(versions.len(), 5);
        // Most recent first
        assert_eq!(versions[0].body, "version 4");

        // Prune to 2
        let pruned = repo.prune_versions("n1", 2).await.unwrap();
        assert_eq!(pruned, 3);
        assert_eq!(repo.list_versions("n1").await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_notes_by_entity() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Meeting notes"))
            .await
            .unwrap();
        repo.create_note(&sample_note("n2", "Other note"))
            .await
            .unwrap();

        // Link n1 to task t1
        repo.set_entity_mentions("n1", &[("task".to_string(), "t1".to_string())])
            .await
            .unwrap();

        let linked = repo.list_notes_by_entity("task", "t1").await.unwrap();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].id, "n1");

        // No notes for unknown entity
        let empty = repo.list_notes_by_entity("task", "t99").await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_notes_filtered_by_notebook() {
        let repo = setup().await;
        let now = utc_now_str();
        let nb = NotebookRow {
            id: "nb1".to_string(),
            parent_id: None,
            title: "Work".to_string(),
            icon: None,
            sort_order: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        repo.create_notebook(&nb).await.unwrap();

        let mut note_in_nb = sample_note("n1", "In notebook");
        note_in_nb.notebook_id = Some("nb1".to_string());
        repo.create_note(&note_in_nb).await.unwrap();
        repo.create_note(&sample_note("n2", "No notebook"))
            .await
            .unwrap();

        let filtered = repo.list_notes(Some("nb1")).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "n1");

        let all = repo.list_notes(None).await.unwrap();
        assert_eq!(all.len(), 2);
    }
}
