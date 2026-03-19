//! Repository for notes, notebooks, tags, links, and version history.

mod inbox;
mod links;
mod notebooks;
mod notes;
mod suggestions;
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
            icon: None,
            color: None,
            embedding_updated_at: None,
            split_content: None,
            split_mode: None,
            perspective_config: None,
            last_visited_at: None,
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
            .update_note(
                "n1",
                Some("Updated"),
                None,
                None,
                Some(true),
                None,
                None,
                None,
                None,
                None,
                None,
            )
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
            color: None,
            sort_order: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        repo.create_notebook(&nb).await.unwrap();

        let notebooks = repo.list_notebooks().await.unwrap();
        assert_eq!(notebooks.len(), 1);
        assert_eq!(notebooks[0].title, "My Notebook");

        let updated = repo
            .update_notebook("nb1", Some("Renamed"), None, None, None)
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
    async fn test_search_fts() {
        let repo = setup().await;
        let mut note1 = sample_note("n1", "Rust Programming Guide");
        note1.body = "A comprehensive guide to systems programming with Rust".to_string();
        repo.create_note(&note1).await.unwrap();

        let mut note2 = sample_note("n2", "Go Concurrency Patterns");
        note2.body = "Goroutines and channels in Go".to_string();
        repo.create_note(&note2).await.unwrap();

        let mut note3 = sample_note("n3", "Learning Rust Async");
        note3.body = "Understanding async/await in Rust programming".to_string();
        repo.create_note(&note3).await.unwrap();

        // Search for "Rust" — should find notes 1 and 3
        let results = repo.search_fts("Rust").await.unwrap();
        assert_eq!(results.len(), 2);
        // Both results should have positive rank
        assert!(results[0].rank > 0.0);
        assert!(results[1].rank > 0.0);
        // Higher rank first
        assert!(results[0].rank >= results[1].rank);

        // Search for "Go" — should find only note 2
        let results = repo.search_fts("Go").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "n2");

        // Search for something that doesn't exist
        let results = repo.search_fts("Python").await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_inbox_create_and_list() {
        let repo = setup().await;

        let item = repo
            .create_inbox_item("Quick thought about architecture")
            .await
            .unwrap();
        assert_eq!(item.content, "Quick thought about architecture");
        assert_eq!(item.status, "pending");

        let items = repo.list_inbox_items().await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, item.id);
    }

    #[tokio::test]
    async fn test_inbox_delete() {
        let repo = setup().await;

        let item = repo.create_inbox_item("Temporary thought").await.unwrap();
        repo.delete_inbox_item(&item.id).await.unwrap();

        let items = repo.list_inbox_items().await.unwrap();
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn test_inbox_count() {
        let repo = setup().await;

        assert_eq!(repo.count_inbox_items().await.unwrap(), 0);
        repo.create_inbox_item("Item 1").await.unwrap();
        repo.create_inbox_item("Item 2").await.unwrap();
        assert_eq!(repo.count_inbox_items().await.unwrap(), 2);
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
            color: None,
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

    #[tokio::test]
    async fn test_find_structural_holes() {
        let repo = setup().await;

        // Create A -> B, A -> C, B -> D, C -> D
        // D shares 2 neighbors (B,C) with A but no direct link A->D
        let a = repo.create_note(&sample_note("a", "Note A")).await.unwrap();
        let b = repo.create_note(&sample_note("b", "Note B")).await.unwrap();
        let c = repo.create_note(&sample_note("c", "Note C")).await.unwrap();
        let d = repo.create_note(&sample_note("d", "Note D")).await.unwrap();

        repo.set_links(&a.id, &[b.id.clone(), c.id.clone()])
            .await
            .unwrap();
        repo.set_links(&b.id, std::slice::from_ref(&d.id))
            .await
            .unwrap();
        repo.set_links(&c.id, std::slice::from_ref(&d.id))
            .await
            .unwrap();

        let holes = repo.find_structural_holes(&a.id).await.unwrap();
        assert!(holes.iter().any(|(id, count)| id == &d.id && *count >= 2));
    }

    #[tokio::test]
    async fn test_find_entity_cooccurrences() {
        let repo = setup().await;
        let a = repo.create_note(&sample_note("a", "Note A")).await.unwrap();
        let b = repo.create_note(&sample_note("b", "Note B")).await.unwrap();

        repo.set_entity_mentions(
            &a.id,
            &[
                ("task".to_string(), "t1".to_string()),
                ("project".to_string(), "p1".to_string()),
            ],
        )
        .await
        .unwrap();
        repo.set_entity_mentions(
            &b.id,
            &[
                ("task".to_string(), "t1".to_string()),
                ("project".to_string(), "p1".to_string()),
            ],
        )
        .await
        .unwrap();

        let cooccurrences = repo.find_entity_cooccurrences(&a.id).await.unwrap();
        assert!(cooccurrences
            .iter()
            .any(|(id, count)| id == &b.id && *count == 2));
    }

    #[tokio::test]
    async fn test_find_tag_overlaps() {
        let repo = setup().await;
        let a = repo.create_note(&sample_note("a", "Note A")).await.unwrap();
        let b = repo.create_note(&sample_note("b", "Note B")).await.unwrap();

        repo.set_tags(&a.id, &["rust".to_string(), "async".to_string()])
            .await
            .unwrap();
        repo.set_tags(&b.id, &["rust".to_string(), "async".to_string()])
            .await
            .unwrap();

        let overlaps = repo.find_tag_overlaps(&a.id).await.unwrap();
        assert!(overlaps
            .iter()
            .any(|(id, count)| id == &b.id && *count == 2));
    }

    #[tokio::test]
    async fn test_get_backlinks_with_context() {
        let repo = setup().await;
        let target = sample_note("target", "Target Note");
        repo.create_note(&target).await.unwrap();

        let mut source = sample_note("source", "Source Note");
        source.body = "This links to [[Target Note]] in context".to_string();
        repo.create_note(&source).await.unwrap();

        repo.set_links("source", &["target".to_string()])
            .await
            .unwrap();

        let backlinks = repo.get_backlinks_with_context("target").await.unwrap();
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].0.id, "source");
        assert!(backlinks[0].1.as_ref().unwrap().contains("[[Target Note]]"));
    }

    #[tokio::test]
    async fn test_list_notes_paginated() {
        let repo = setup().await;

        for i in 0..10 {
            let note = sample_note(&format!("n{i}"), &format!("Note {i}"));
            repo.create_note(&note).await.unwrap();
        }

        let page1 = repo.list_notes_paginated(None, None, 5, 0).await.unwrap();
        assert_eq!(page1.len(), 5);

        let page2 = repo.list_notes_paginated(None, None, 5, 5).await.unwrap();
        assert_eq!(page2.len(), 5);

        let all = repo.list_notes_paginated(None, None, 50, 0).await.unwrap();
        assert_eq!(all.len(), 10);
    }

    #[tokio::test]
    async fn test_list_notes_with_tag_filter() {
        let repo = setup().await;

        let a = sample_note("a", "Rust Note");
        repo.create_note(&a).await.unwrap();
        let b = sample_note("b", "Python Note");
        repo.create_note(&b).await.unwrap();
        let c = sample_note("c", "Untagged Note");
        repo.create_note(&c).await.unwrap();

        repo.set_tags("a", &["rust".to_string()]).await.unwrap();
        repo.set_tags("b", &["python".to_string()]).await.unwrap();

        let results = repo
            .list_notes_paginated(None, Some(&["rust".to_string()]), 50, 0)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Note");
    }

    #[tokio::test]
    async fn test_get_all_tags() {
        let repo = setup().await;
        let a = repo.create_note(&sample_note("a", "Note A")).await.unwrap();
        let b = repo.create_note(&sample_note("b", "Note B")).await.unwrap();

        repo.set_tags(&a.id, &["rust".to_string(), "async".to_string()])
            .await
            .unwrap();
        repo.set_tags(&b.id, &["rust".to_string()]).await.unwrap();

        let tags = repo.get_all_tags().await.unwrap();
        assert_eq!(tags[0].0, "rust");
        assert_eq!(tags[0].1, 2);
        assert_eq!(tags[1].0, "async");
        assert_eq!(tags[1].1, 1);
    }
}
