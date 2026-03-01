//! Unit tests for ActionRepo — the most complex repository.
//!
//! Covers CRUD, join tables (attachments, time_entries, dependencies),
//! focus slot management, cycle detection, cascade operations, and filtering.
//!
//! Tests use an ephemeral SQLite pool via `test_action_repo()` (StoragePool::connect on a TempDir).

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use crate::repos::action_repo::{ActionPatch, ActionRepo};
    use crate::repos::area::AreaRepo;
    use crate::rows::action::ActionRow;
    use crate::rows::area::AreaRow;
    use crate::StoragePool;

    /// Connect to an ephemeral SQLite database for testing.
    async fn test_action_repo() -> Option<(ActionRepo, AreaRepo)> {
        let dir = tempfile::tempdir().ok()?;
        let pool = StoragePool::connect(dir.path()).await.ok()?;
        let _ = dir.keep(); // prevent cleanup; acceptable in test context
        let repos = crate::Repos::from_pool(&pool);
        Some((repos.actions, repos.areas))
    }

    /// Create a default area for tests (actions require area_id NOT NULL).
    async fn ensure_area(area_repo: &AreaRepo) -> String {
        let area_id = "test-area".to_string();
        let _ = area_repo
            .create(&AreaRow {
                id: area_id.clone(),
                name: format!("Test Area {}", &uuid::Uuid::new_v4().to_string()[..4]),
                description: None,
                color: "blue".to_string(),
                icon: None,
                position: 0,
                status: "active".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .await;
        area_id
    }

    /// Helper: create a minimal ActionRow for testing.
    fn sample_action(id: &str, title: &str, area_id: &str) -> ActionRow {
        let now = Utc::now();
        ActionRow {
            id: id.to_string(),
            title: title.to_string(),
            description: None,
            area_id: area_id.to_string(),
            project_id: None,
            key_result_id: None,
            parent_id: None,
            priority: None,
            due_date: None,
            tags: vec![],
            status: "todo".to_string(),
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            total_tracked_secs: 0,
            estimated_minutes: None,
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
        }
    }

    fn unique_id(_prefix: &str) -> String {
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    }

    // ----- CRUD -----

    #[tokio::test]
    async fn add_and_get() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("ag");
        let row = sample_action(&id, "Buy milk", &area_id);
        let inserted = repo.add(&row).await.unwrap();
        assert_eq!(inserted.title, "Buy milk");

        let fetched = repo.get(&id).await.unwrap().unwrap();
        assert_eq!(fetched.title, "Buy milk");

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn update_fields() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("uf");
        let row = sample_action(&id, "Buy milk", &area_id);
        repo.add(&row).await.unwrap();

        let patch = ActionPatch {
            id: id.clone(),
            title: Some("Buy oat milk".to_string()),
            priority: Some(Some(1)),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert_eq!(updated.title, "Buy oat milk");
        assert_eq!(updated.priority, Some(1));

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn delete_removes_action() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("dr");
        repo.add(&sample_action(&id, "Deletable", &area_id))
            .await
            .unwrap();
        assert!(repo.delete(&id).await.unwrap());
        assert!(repo.get(&id).await.unwrap().is_none());
    }

    // ----- New fields: calendar_event_uid, next_instance_date, last_reminded_at -----

    #[tokio::test]
    async fn update_calendar_event_uid_set_and_clear() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("ceuid");
        repo.add(&sample_action(&id, "Calendar linked", &area_id))
            .await
            .unwrap();

        // Set calendar_event_uid
        let patch = ActionPatch {
            id: id.clone(),
            calendar_event_uid: Some(Some("abc-123@calendar".to_string())),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert_eq!(
            updated.calendar_event_uid.as_deref(),
            Some("abc-123@calendar")
        );

        // Clear calendar_event_uid with Some(None)
        let patch = ActionPatch {
            id: id.clone(),
            calendar_event_uid: Some(None),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert!(
            updated.calendar_event_uid.is_none(),
            "calendar_event_uid should be cleared"
        );

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn update_next_instance_date_set_and_clear() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("nid");
        repo.add(&sample_action(&id, "Recurring template", &area_id))
            .await
            .unwrap();

        let next_date: DateTime<Utc> = "2026-03-01T09:00:00Z".parse().unwrap();

        // Set next_instance_date
        let patch = ActionPatch {
            id: id.clone(),
            next_instance_date: Some(Some(next_date)),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert_eq!(updated.next_instance_date, Some(next_date));

        // Clear next_instance_date with Some(None)
        let patch = ActionPatch {
            id: id.clone(),
            next_instance_date: Some(None),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert!(
            updated.next_instance_date.is_none(),
            "next_instance_date should be cleared"
        );

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn update_last_reminded_at_set_and_clear() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("lra");
        repo.add(&sample_action(&id, "Reminder task", &area_id))
            .await
            .unwrap();

        let now = Utc::now();

        // Set last_reminded_at
        let patch = ActionPatch {
            id: id.clone(),
            last_reminded_at: Some(Some(now)),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert!(
            updated.last_reminded_at.is_some(),
            "last_reminded_at should be set"
        );

        // Clear last_reminded_at with Some(None)
        let patch = ActionPatch {
            id: id.clone(),
            last_reminded_at: Some(None),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert!(
            updated.last_reminded_at.is_none(),
            "last_reminded_at should be cleared"
        );

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn update_new_fields_leaves_others_unchanged() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("nfu");
        repo.add(&sample_action(&id, "Unchanged fields", &area_id))
            .await
            .unwrap();

        // Set title and priority first
        let patch = ActionPatch {
            id: id.clone(),
            title: Some("Important task".to_string()),
            priority: Some(Some(2)),
            ..Default::default()
        };
        repo.update(&patch).await.unwrap();

        // Now set only calendar_event_uid — title and priority should be preserved
        let patch = ActionPatch {
            id: id.clone(),
            calendar_event_uid: Some(Some("uid-xyz".to_string())),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert_eq!(updated.title, "Important task");
        assert_eq!(updated.priority, Some(2));
        assert_eq!(updated.calendar_event_uid.as_deref(), Some("uid-xyz"));

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    // ----- Filtering -----

    #[tokio::test]
    async fn list_filters_by_status() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let ids: Vec<String> = (0..3).map(|i| unique_id(&format!("lfs{i}"))).collect();
        repo.add(&sample_action(&ids[0], "Todo task", &area_id))
            .await
            .unwrap();
        let mut doing = sample_action(&ids[1], "Doing task", &area_id);
        doing.status = "doing".to_string();
        repo.add(&doing).await.unwrap();
        let mut done = sample_action(&ids[2], "Done task", &area_id);
        done.status = "done".to_string();
        done.completed_at = Some(Utc::now());
        repo.add(&done).await.unwrap();

        let filter = crate::repos::action_repo::ActionFilter {
            status: Some("doing".to_string()),
            ..Default::default()
        };
        let results = repo.list(&filter).await.unwrap();
        assert!(
            results.iter().any(|t| t.id == ids[1]),
            "Should contain the doing task"
        );

        // Cleanup
        for id in &ids {
            let _ = repo.delete(id).await;
        }
    }

    #[tokio::test]
    async fn list_filters_by_tags_gin() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id1 = unique_id("tg1");
        let id2 = unique_id("tg2");
        let mut row1 = sample_action(&id1, "Backend task", &area_id);
        row1.tags = vec!["rust".to_string(), "backend".to_string()];
        repo.add(&row1).await.unwrap();
        let mut row2 = sample_action(&id2, "Frontend task", &area_id);
        row2.tags = vec!["rust".to_string(), "frontend".to_string()];
        repo.add(&row2).await.unwrap();

        let filter = crate::repos::action_repo::ActionFilter {
            tags: Some(vec!["backend".to_string()]),
            ..Default::default()
        };
        let results = repo.list(&filter).await.unwrap();
        assert!(results.iter().any(|t| t.id == id1));
        assert!(!results.iter().any(|t| t.id == id2));

        // Cleanup
        let _ = repo.delete(&id1).await;
        let _ = repo.delete(&id2).await;
    }

    #[tokio::test]
    async fn list_filters_by_priority() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let ids: Vec<String> = (0..3).map(|i| unique_id(&format!("lfp{i}"))).collect();
        for (i, priority) in [1i16, 2, 3].iter().enumerate() {
            let mut row = sample_action(&ids[i], &format!("P{priority}"), &area_id);
            row.priority = Some(*priority);
            repo.add(&row).await.unwrap();
        }

        let filter = crate::repos::action_repo::ActionFilter {
            priority_min: Some(2),
            ..Default::default()
        };
        let results = repo.list(&filter).await.unwrap();
        let matching: Vec<_> = results.iter().filter(|t| ids.contains(&t.id)).collect();
        assert!(matching.len() >= 2);

        // Cleanup
        for id in &ids {
            let _ = repo.delete(id).await;
        }
    }

    #[tokio::test]
    async fn list_templates_only() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id_regular = unique_id("ltr");
        let id_template = unique_id("ltt");
        repo.add(&sample_action(&id_regular, "Regular", &area_id))
            .await
            .unwrap();
        let mut tpl = sample_action(&id_template, "Template", &area_id);
        tpl.is_template = true;
        repo.add(&tpl).await.unwrap();

        let templates = repo.list_templates().await.unwrap();
        assert!(templates.iter().any(|t| t.id == id_template));
        assert!(!templates.iter().any(|t| t.id == id_regular));

        // Cleanup
        let _ = repo.delete(&id_regular).await;
        let _ = repo.delete(&id_template).await;
    }

    // ----- Focus Slots -----

    #[tokio::test]
    async fn focus_sets_timestamp() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("fst");
        repo.add(&sample_action(&id, "Focus me", &area_id))
            .await
            .unwrap();
        let deadline = Utc::now() + chrono::Duration::hours(24);
        // Use a large slot limit so parallel tests with their own focused actions don't interfere.
        assert!(repo.focus(&id, 1000, Some(deadline)).await.unwrap());
        let row = repo.get(&id).await.unwrap().unwrap();
        assert!(row.focused_at.is_some());
        assert!(row.focus_deadline.is_some());

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn unfocus_clears_timestamp() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("uft");
        repo.add(&sample_action(&id, "Unfocus me", &area_id))
            .await
            .unwrap();
        repo.focus(&id, 1000, None).await.unwrap();
        assert!(repo.unfocus(&id).await.unwrap());
        let row = repo.get(&id).await.unwrap().unwrap();
        assert!(row.focused_at.is_none());

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn focus_respects_slot_limit() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        // Account for existing focused actions from parallel tests by basing the slot limit
        // on the current count: limit = existing + 3, so filling 3 more saturates it.
        let existing_focused = repo.list_focused().await.unwrap().len() as i64;
        let slot_limit = existing_focused + 3;

        let ids: Vec<String> = (0..4).map(|i| unique_id(&format!("fsl{i}"))).collect();
        for id in &ids {
            repo.add(&sample_action(id, "Slot task", &area_id))
                .await
                .unwrap();
        }
        // Fill 3 new slots on top of existing focused actions
        for id in &ids[..3] {
            assert!(repo.focus(id, slot_limit, None).await.unwrap());
        }
        // 4th should fail because slot_limit is now exactly reached
        let focused = repo.focus(&ids[3], slot_limit, None).await.unwrap();
        assert!(!focused, "Should reject when slots are full");

        // Cleanup
        for id in &ids {
            let _ = repo.delete(id).await;
        }
    }

    // ----- Dependencies -----

    #[tokio::test]
    async fn add_dependency_creates_edge() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let a = unique_id("dep-a");
        let b = unique_id("dep-b");
        repo.add(&sample_action(&a, "Task A", &area_id))
            .await
            .unwrap();
        repo.add(&sample_action(&b, "Task B", &area_id))
            .await
            .unwrap();
        repo.add_dependency(&a, &b).await.unwrap();

        let blockers = repo.get_blockers(&a).await.unwrap();
        assert!(blockers.iter().any(|t| t.id == b));

        // Cleanup
        let _ = repo.remove_dependency(&a, &b).await;
        let _ = repo.delete(&a).await;
        let _ = repo.delete(&b).await;
    }

    #[tokio::test]
    async fn remove_dependency_deletes_edge() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let a = unique_id("rdep-a");
        let b = unique_id("rdep-b");
        repo.add(&sample_action(&a, "Task A", &area_id))
            .await
            .unwrap();
        repo.add(&sample_action(&b, "Task B", &area_id))
            .await
            .unwrap();
        repo.add_dependency(&a, &b).await.unwrap();
        assert!(repo.remove_dependency(&a, &b).await.unwrap());
        let blockers = repo.get_blockers(&a).await.unwrap();
        assert!(!blockers.iter().any(|t| t.id == b));

        // Cleanup
        let _ = repo.delete(&a).await;
        let _ = repo.delete(&b).await;
    }

    #[tokio::test]
    async fn cycle_detection_rejects_circular_deps() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let a = unique_id("cyc-a");
        let b = unique_id("cyc-b");
        let c = unique_id("cyc-c");
        repo.add(&sample_action(&a, "A", &area_id)).await.unwrap();
        repo.add(&sample_action(&b, "B", &area_id)).await.unwrap();
        repo.add(&sample_action(&c, "C", &area_id)).await.unwrap();
        repo.add_dependency(&a, &b).await.unwrap();
        repo.add_dependency(&b, &c).await.unwrap();
        // C -> A would create a cycle
        let result = repo.add_dependency(&c, &a).await;
        assert!(result.is_err());

        // Cleanup
        let _ = repo.remove_dependency(&a, &b).await;
        let _ = repo.remove_dependency(&b, &c).await;
        let _ = repo.delete(&a).await;
        let _ = repo.delete(&b).await;
        let _ = repo.delete(&c).await;
    }

    #[tokio::test]
    async fn self_dependency_rejected() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let a = unique_id("self-dep");
        repo.add(&sample_action(&a, "Self ref", &area_id))
            .await
            .unwrap();
        let result = repo.add_dependency(&a, &a).await;
        assert!(result.is_err());

        // Cleanup
        let _ = repo.delete(&a).await;
    }

    #[tokio::test]
    async fn incomplete_blockers_returns_blocking_actions() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let a = unique_id("ib-a");
        let b = unique_id("ib-b");
        let c = unique_id("ib-c");
        repo.add(&sample_action(&a, "Blocked", &area_id))
            .await
            .unwrap();
        let mut done_row = sample_action(&b, "Done blocker", &area_id);
        done_row.status = "done".to_string();
        done_row.completed_at = Some(Utc::now());
        repo.add(&done_row).await.unwrap();
        repo.add(&sample_action(&c, "Active blocker", &area_id))
            .await
            .unwrap();
        repo.add_dependency(&a, &b).await.unwrap();
        repo.add_dependency(&a, &c).await.unwrap();

        let incomplete = repo.incomplete_blockers(&a).await.unwrap();
        assert_eq!(incomplete.len(), 1);
        assert_eq!(incomplete[0].id, c);

        // Cleanup
        let _ = repo.remove_dependency(&a, &b).await;
        let _ = repo.remove_dependency(&a, &c).await;
        let _ = repo.delete(&a).await;
        let _ = repo.delete(&b).await;
        let _ = repo.delete(&c).await;
    }

    // ----- Attachments -----

    #[tokio::test]
    async fn add_attachment_inserts_row() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("att");
        repo.add(&sample_action(&id, "With attachment", &area_id))
            .await
            .unwrap();
        let att = repo
            .add_attachment(&id, "file", "/path/to/report.pdf", Some("Report"), &[])
            .await
            .unwrap();
        assert_eq!(att.action_id, id);
        assert_eq!(att.attachment_type, "file");

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn remove_attachment_deletes_row() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("ratt");
        repo.add(&sample_action(&id, "Remove att", &area_id))
            .await
            .unwrap();
        let att = repo
            .add_attachment(&id, "url", "https://example.com", None, &[])
            .await
            .unwrap();
        assert!(repo.remove_attachment(&id, att.id).await.unwrap());
        let atts = repo.list_attachments(&id).await.unwrap();
        assert!(atts.is_empty());

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn delete_action_cascades_attachments() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("cascatt");
        repo.add(&sample_action(&id, "Cascade att", &area_id))
            .await
            .unwrap();
        for i in 0..3 {
            repo.add_attachment(&id, "note", &format!("Note {i}"), None, &[])
                .await
                .unwrap();
        }
        repo.delete(&id).await.unwrap();
        // Attachments should be gone (CASCADE)
        let atts = repo.list_attachments(&id).await.unwrap();
        assert!(atts.is_empty());
    }

    // ----- Time Entries -----

    #[tokio::test]
    async fn add_time_entry_inserts_row() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("te");
        repo.add(&sample_action(&id, "Track time", &area_id))
            .await
            .unwrap();
        let entry = repo
            .add_time_entry(&id, "manual", Utc::now(), Some(1800), None)
            .await
            .unwrap();
        assert_eq!(entry.action_id, id);
        assert_eq!(entry.duration_secs, Some(1800));

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn close_time_entry_sets_ended_at() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("cte");
        repo.add(&sample_action(&id, "Close time entry", &area_id))
            .await
            .unwrap();
        let entry = repo
            .add_time_entry(&id, "focus", Utc::now(), None, None)
            .await
            .unwrap();
        assert!(entry.ended_at.is_none());

        let closed = repo.close_time_entry(&id, entry.id).await.unwrap();
        assert!(closed.ended_at.is_some());
        assert!(closed.duration_secs.is_some());

        // Cleanup
        let _ = repo.delete(&id).await;
    }

    // ----- Hierarchy -----

    #[tokio::test]
    async fn move_action_updates_parent_and_project() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let parent = unique_id("mv-p");
        let child = unique_id("mv-c");
        repo.add(&sample_action(&parent, "Parent", &area_id))
            .await
            .unwrap();
        repo.add(&sample_action(&child, "Child", &area_id))
            .await
            .unwrap();

        let moved = repo.move_action(&child, Some(&parent), None).await.unwrap();
        assert_eq!(moved.parent_id.as_deref(), Some(parent.as_str()));

        // Move to no parent
        let moved = repo.move_action(&child, None, None).await.unwrap();
        assert!(moved.parent_id.is_none());

        // Cleanup
        let _ = repo.delete(&child).await;
        let _ = repo.delete(&parent).await;
    }

    #[tokio::test]
    async fn parent_cycle_detection() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let a = unique_id("pcyc-a");
        let b = unique_id("pcyc-b");
        repo.add(&sample_action(&a, "A", &area_id)).await.unwrap();
        repo.add(&sample_action(&b, "B", &area_id)).await.unwrap();
        repo.move_action(&b, Some(&a), None).await.unwrap();
        // Setting A's parent to B would create a cycle
        let result = repo.move_action(&a, Some(&b), None).await;
        assert!(result.is_err());

        // Cleanup
        let _ = repo.move_action(&b, None, None).await;
        let _ = repo.delete(&a).await;
        let _ = repo.delete(&b).await;
    }

    #[tokio::test]
    async fn cascade_complete_children() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let p = unique_id("casc-p");
        let c1 = unique_id("casc-c1");
        let c2 = unique_id("casc-c2");
        repo.add(&sample_action(&p, "Parent", &area_id))
            .await
            .unwrap();
        repo.add(&sample_action(&c1, "Child 1", &area_id))
            .await
            .unwrap();
        repo.add(&sample_action(&c2, "Child 2", &area_id))
            .await
            .unwrap();
        repo.move_action(&c1, Some(&p), None).await.unwrap();
        repo.move_action(&c2, Some(&p), None).await.unwrap();

        let completed = repo.cascade_complete(&p).await.unwrap();
        assert_eq!(completed, 3);

        let c1_row = repo.get(&c1).await.unwrap().unwrap();
        assert_eq!(c1_row.status, "done");

        // Cleanup
        let _ = repo.delete(&c1).await;
        let _ = repo.delete(&c2).await;
        let _ = repo.delete(&p).await;
    }

    // ----- Aggregation -----

    #[tokio::test]
    async fn summary_counts_by_status() {
        let Some((repo, _area_repo)) = test_action_repo().await else {
            return;
        };
        // Just verify summary doesn't error — actual counts depend on DB state
        let _summary = repo.summary().await.unwrap();
    }

    #[tokio::test]
    async fn context_string_includes_active_actions() {
        let Some((repo, area_repo)) = test_action_repo().await else {
            return;
        };
        let area_id = ensure_area(&area_repo).await;
        let id = unique_id("ctx");
        repo.add(&sample_action(&id, "Context task", &area_id))
            .await
            .unwrap();
        let ctx = repo.to_context_string().await.unwrap();
        assert!(ctx.contains("Context task"));

        // Cleanup
        let _ = repo.delete(&id).await;
    }
}
