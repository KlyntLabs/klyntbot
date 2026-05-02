use storage::repos::ApprovalHistorySummary;

pub struct Layer3Config {
    pub enabled: bool,
    pub min_approvals: u32,
    pub cooldown_seconds: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Layer3Outcome {
    AutoAllow { reason: String },
    Ask { reason: String },
    FallThrough,
}

pub fn evaluate(
    cfg: &Layer3Config,
    summary: &ApprovalHistorySummary,
    now_unix: i64,
) -> Layer3Outcome {
    if !cfg.enabled {
        return Layer3Outcome::FallThrough;
    }
    if summary.denial_count >= 1 {
        return Layer3Outcome::Ask {
            reason: "mirror: prior denial — always confirm".into(),
        };
    }
    if summary.approval_count < cfg.min_approvals {
        return Layer3Outcome::FallThrough;
    }
    let last = summary.last_decided_at.unwrap_or(0);
    if now_unix - last < cfg.cooldown_seconds {
        return Layer3Outcome::FallThrough;
    }
    Layer3Outcome::AutoAllow {
        reason: format!(
            "mirror: {}+ prior approvals, no denials",
            summary.approval_count
        ),
    }
}

pub fn args_hash_for_relevance(tool: &str, args_json: &str) -> String {
    use blake3::Hasher;
    let mut h = Hasher::new();
    h.update(tool.as_bytes());
    h.update(b"\0");
    let normalized = match tool {
        "bash" => normalize_bash(args_json),
        "edit" | "write" | "apply_patch" => normalize_path(args_json),
        _ => args_json.to_string(),
    };
    h.update(normalized.as_bytes());
    h.finalize().to_hex().to_string()
}

fn normalize_bash(args_json: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(args_json).unwrap_or(serde_json::Value::Null);
    let cmd = v.get("command").and_then(|c| c.as_str()).unwrap_or("");
    cmd.split_whitespace().next().unwrap_or("").to_string()
}

fn normalize_path(args_json: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(args_json).unwrap_or(serde_json::Value::Null);
    v.get("path")
        .and_then(|p| p.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cfg(enabled: bool) -> Layer3Config {
        Layer3Config {
            enabled,
            min_approvals: 5,
            cooldown_seconds: 86400,
        }
    }
    fn s(ok: u32, deny: u32, last: Option<i64>) -> ApprovalHistorySummary {
        ApprovalHistorySummary {
            approval_count: ok,
            denial_count: deny,
            last_decided_at: last,
        }
    }

    #[test]
    fn disabled_falls_through() {
        assert_eq!(
            evaluate(&cfg(false), &s(100, 0, Some(0)), 999_999),
            Layer3Outcome::FallThrough
        );
    }
    #[test]
    fn single_denial_locks_to_ask() {
        match evaluate(&cfg(true), &s(100, 1, Some(0)), 999_999) {
            Layer3Outcome::Ask { .. } => {}
            other => panic!("got {other:?}"),
        }
    }
    #[test]
    fn under_threshold_falls_through() {
        assert_eq!(
            evaluate(&cfg(true), &s(4, 0, Some(0)), 999_999),
            Layer3Outcome::FallThrough
        );
    }
    #[test]
    fn cooldown_falls_through() {
        // 5 approvals but last decided 1 hour ago → still in cooldown
        assert_eq!(
            evaluate(&cfg(true), &s(5, 0, Some(999_000)), 999_999 + 3600),
            Layer3Outcome::FallThrough
        );
    }
    #[test]
    fn five_approvals_post_cooldown_auto_allow() {
        match evaluate(&cfg(true), &s(5, 0, Some(0)), 90_000 + 999_999) {
            Layer3Outcome::AutoAllow { .. } => {}
            other => panic!("got {other:?}"),
        }
    }
    #[test]
    fn args_hash_strips_command_args() {
        let a = args_hash_for_relevance("bash", r#"{"command":"git status"}"#);
        let b = args_hash_for_relevance("bash", r#"{"command":"git status --short"}"#);
        assert_eq!(a, b, "trailing flags should not change the relevance hash");
    }
}
