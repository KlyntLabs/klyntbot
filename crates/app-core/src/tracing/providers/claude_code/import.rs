//! Copies a picked `.jsonl` into `imported_claude_code/<new-uuid>/<new-uuid>.jsonl`.

use common::Result;
use std::path::Path;
use uuid::Uuid;

pub async fn import_from_file(imported_root: &Path, source_jsonl: &Path) -> Result<String> {
    if !tokio::fs::try_exists(source_jsonl)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("stat: {e}")))?
    {
        return Err(common::KlyntbotError::Storage(format!(
            "source jsonl missing: {}",
            source_jsonl.display()
        )));
    }
    let new_id = Uuid::new_v4().to_string();
    let target_dir = imported_root.join(&new_id);
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("mkdir: {e}")))?;
    tokio::fs::copy(source_jsonl, target_dir.join(format!("{new_id}.jsonl")))
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("copy: {e}")))?;
    let meta = serde_json::json!({
        "imported_at": jiff::Timestamp::now().to_string(),
        "original_path": source_jsonl.display().to_string(),
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
    async fn copies_file_and_writes_meta() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("orig.jsonl");
        fs::write(&src, "hello\n").await.unwrap();
        let imported_root = tmp.path().join("imp");
        let id = import_from_file(&imported_root, &src).await.unwrap();
        let copied = imported_root.join(&id).join(format!("{id}.jsonl"));
        assert!(tokio::fs::try_exists(&copied).await.unwrap());
        let meta = imported_root.join(&id).join("meta.json");
        let txt = fs::read_to_string(meta).await.unwrap();
        assert!(txt.contains("original_path"));
        assert!(txt.contains("orig.jsonl"));
    }

    #[tokio::test]
    async fn missing_source_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let r = import_from_file(&tmp.path().join("imp"), &tmp.path().join("nope")).await;
        assert!(r.is_err());
    }
}
