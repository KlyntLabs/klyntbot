use crate::tracing::types::ContextMessage;
use common::Result;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};

pub async fn load_context(path: &Path) -> Result<Vec<ContextMessage>> {
    let file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(common::KlyntbotError::Storage(format!("open {}: {e}", path.display()))),
    };
    let mut out = Vec::new();
    let mut reader = BufReader::new(file).lines();
    let mut idx: u32 = 0;
    while let Some(line) = reader.next_line().await
        .map_err(|e| common::KlyntbotError::Storage(format!("read line: {e}")))?
    {
        if line.trim().is_empty() { continue; }
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "skipping unparseable context line");
                continue;
            }
        };
        let role = v.get("role").and_then(|x| x.as_str()).unwrap_or("").to_string();
        out.push(ContextMessage { index: idx, role, content: v });
        idx += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/context.jsonl");
        p
    }

    #[tokio::test]
    async fn loads_fixture() {
        let msgs = load_context(&fixture()).await.unwrap();
        assert_eq!(msgs.len(), 5);
        assert_eq!(msgs[0].role, "_system_prompt");
        assert_eq!(msgs[1].role, "user");
        assert_eq!(msgs[3].role, "_usage");
        assert_eq!(msgs[0].index, 0);
        assert_eq!(msgs[4].index, 4);
    }

    #[tokio::test]
    async fn missing_returns_empty() {
        let msgs = load_context(&PathBuf::from("/nope")).await.unwrap();
        assert!(msgs.is_empty());
    }
}
