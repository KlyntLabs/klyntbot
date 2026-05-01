//! Copy a foreign `wire.jsonl` (and optional sibling `context.jsonl`/`state.json`)
//! into `{klyntbot_data}/coding_memory/imported_wire/<new_uuid>/`.

use common::Result;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub async fn import_from_file(imported_root: &Path, source_wire: &Path) -> Result<String> {
    if !tokio::fs::try_exists(source_wire).await.map_err(|e| {
        common::KlyntbotError::Storage(format!("stat {}: {e}", source_wire.display()))
    })? {
        return Err(common::KlyntbotError::Storage(format!(
            "source wire missing: {}",
            source_wire.display()
        )));
    }
    let new_id = Uuid::new_v4().to_string();
    let target_dir = imported_root.join(&new_id);
    tokio::fs::create_dir_all(&target_dir).await.map_err(|e| {
        common::KlyntbotError::Storage(format!("mkdir {}: {e}", target_dir.display()))
    })?;
    tokio::fs::copy(source_wire, target_dir.join("wire.jsonl"))
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("copy wire: {e}")))?;

    let parent = source_wire.parent().unwrap_or(Path::new("."));
    for sibling in &["context.jsonl", "state.json"] {
        let sp = parent.join(sibling);
        if tokio::fs::try_exists(&sp).await.unwrap_or(false) {
            let _ = tokio::fs::copy(&sp, target_dir.join(sibling)).await;
        }
    }

    let meta = serde_json::json!({
        "imported_at": jiff::Timestamp::now().to_string(),
        "original_path": source_wire.display().to_string(),
    });
    tokio::fs::write(
        target_dir.join("meta.json"),
        serde_json::to_vec_pretty(&meta).unwrap(),
    )
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("write meta: {e}")))?;
    Ok(new_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

    #[tokio::test]
    async fn copies_wire_and_siblings() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("foreign");
        fs::create_dir_all(&src).await.unwrap();
        fs::write(src.join("wire.jsonl"), "{}").await.unwrap();
        fs::write(src.join("context.jsonl"), "{}").await.unwrap();
        fs::write(src.join("state.json"), "{}").await.unwrap();
        let imported = tmp.path().join("imported");
        let new_id = import_from_file(&imported, &src.join("wire.jsonl"))
            .await
            .unwrap();
        let target = imported.join(&new_id);
        assert!(target.join("wire.jsonl").exists());
        assert!(target.join("context.jsonl").exists());
        assert!(target.join("state.json").exists());
        assert!(target.join("meta.json").exists());
    }

    #[tokio::test]
    async fn missing_source_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let imp = tmp.path().join("imp");
        let result = import_from_file(&imp, &tmp.path().join("nope.jsonl")).await;
        assert!(result.is_err());
    }
}
