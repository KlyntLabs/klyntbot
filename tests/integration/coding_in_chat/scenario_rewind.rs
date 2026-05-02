use klynt_core::snapshots::SnapshotRepo;
use tempfile::TempDir;

#[tokio::test]
async fn scenario_rewind_restores_files_and_truncates_messages() {
    let dir = TempDir::new().unwrap();
    let foo = dir.path().join("foo.txt");
    std::fs::write(&foo, b"v1").unwrap();

    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let snap_repo = SnapshotRepo::new(pool.clone());
    let session_repo = storage::repos::SessionRepo::new(pool.inner().clone());

    session_repo
        .upsert_session("s", &serde_json::json!({}))
        .await
        .unwrap();
    session_repo
        .add_message(
            "s",
            uuid::Uuid::new_v4(),
            "user",
            "edit foo",
            None,
            None,
            None,
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    // Simulate edit: v1 -> v2 with snapshot
    let old = tokio::fs::read(&foo).await.unwrap();
    snap_repo
        .record("s", None, &foo.to_string_lossy(), &old, true)
        .await
        .unwrap();
    tokio::fs::write(&foo, b"v2").await.unwrap();

    session_repo
        .add_message(
            "s",
            uuid::Uuid::new_v4(),
            "assistant",
            "edited",
            None,
            None,
            None,
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    // Simulate write: bar.txt = x with snapshot (file didn't exist)
    let bar = dir.path().join("bar.txt");
    snap_repo
        .record("s", None, &bar.to_string_lossy(), b"", false)
        .await
        .unwrap();
    tokio::fs::write(&bar, b"x").await.unwrap();

    session_repo
        .add_message(
            "s",
            uuid::Uuid::new_v4(),
            "assistant",
            "wrote bar",
            None,
            None,
            None,
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    // Get the first message id for rewind
    let msgs = session_repo.get_messages("s").await.unwrap();
    let first_msg_id = msgs[0].id.to_string();

    // Rewind to first message
    let snaps = snap_repo
        .list_after_message("s", &first_msg_id)
        .await
        .unwrap();
    for snap in snaps.iter().rev() {
        if snap.file_existed {
            tokio::fs::write(&snap.file_path, &snap.content_before)
                .await
                .unwrap();
        } else {
            let _ = tokio::fs::remove_file(&snap.file_path).await;
        }
    }
    session_repo
        .rewind_to_message("s", &first_msg_id)
        .await
        .unwrap();

    // Assert foo.txt == v1, bar.txt gone, only 1 message left
    assert_eq!(tokio::fs::read_to_string(&foo).await.unwrap(), "v1");
    assert!(!bar.exists(), "bar.txt should have been deleted");
    assert_eq!(session_repo.count_messages("s").await.unwrap(), 1);
}
