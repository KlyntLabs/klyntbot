use crate::class::{ApprovalClass, ApprovalScope};
use crate::policy::ClassifyHook;
use config::schema::{CodingPermissions, DefaultPolicy};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_json::Value;
use std::collections::HashMap;

pub enum CodingApprovalPolicy {
    Default {
        allow: CompiledRules,
        deny: CompiledRules,
        ask: CompiledRules,
        default_if_no_match: DefaultPolicy,
    },
    PlanMode {
        plan_session_id: String,
        plan_file_slug: String,
        plan_file_path: std::path::PathBuf,
        allow: CompiledRules,
        deny: CompiledRules,
        ask: CompiledRules,
        default_if_no_match: DefaultPolicy,
    },
    YoloMode {
        until: jiff::Timestamp,
    },
}

impl CodingApprovalPolicy {
    /// Compile permissions into the `Default` variant — production entry point.
    pub fn compile(permissions: &CodingPermissions) -> Result<Self, String> {
        Ok(Self::Default {
            allow: CompiledRules::compile(&permissions.allow).map_err(|e| e.to_string())?,
            deny: CompiledRules::compile(&permissions.deny).map_err(|e| e.to_string())?,
            ask: CompiledRules::compile(&permissions.ask).map_err(|e| e.to_string())?,
            default_if_no_match: permissions.default_if_no_match,
        })
    }

    pub fn is_plan_mode(&self) -> bool {
        matches!(self, Self::PlanMode { .. })
    }

    pub fn plan_session_id(&self) -> Option<&str> {
        match self {
            Self::PlanMode {
                plan_session_id, ..
            } => Some(plan_session_id.as_str()),
            _ => None,
        }
    }

    pub fn plan_file_path(&self) -> Option<&std::path::Path> {
        match self {
            Self::PlanMode { plan_file_path, .. } => Some(plan_file_path.as_path()),
            _ => None,
        }
    }

    pub fn plan_file_slug(&self) -> Option<&str> {
        match self {
            Self::PlanMode { plan_file_slug, .. } => Some(plan_file_slug.as_str()),
            _ => None,
        }
    }

    fn evaluate_layer1(&self, tool: &str, payload: &str) -> bool {
        let (allow, deny, ask, default_if_no_match) = match self {
            Self::Default {
                allow,
                deny,
                ask,
                default_if_no_match,
            }
            | Self::PlanMode {
                allow,
                deny,
                ask,
                default_if_no_match,
                ..
            } => (allow, deny, ask, *default_if_no_match),
            Self::YoloMode { until } => {
                if jiff::Timestamp::now() < *until {
                    return true;
                }
                // Yolo expired → fall through to ask, treating as if no allow/deny matched.
                return matches!(DefaultPolicy::Ask, DefaultPolicy::Allow);
            }
        };
        if deny.find_match(tool, payload).is_some() {
            return false;
        }
        if allow.find_match(tool, payload).is_some() {
            return true;
        }
        if ask.find_match(tool, payload).is_some() {
            return false;
        }
        default_if_no_match == DefaultPolicy::Allow
    }
}

fn extract_resource(tool: &str, args: &Value) -> Option<String> {
    let key = match tool {
        "bash" => "command",
        "edit" | "write" | "apply_patch" | "notebook_edit" => "file_path",
        "web_fetch" => "url",
        _ => return None,
    };
    args.get(key)?.as_str().map(str::to_string)
}

/// Tools that mutate the workspace. `bash` is always a write because shell
/// commands can mutate anything.
pub(crate) fn is_write_tool(tool: &str) -> bool {
    matches!(
        normalize_tool(tool).as_str(),
        "edit" | "write" | "multiedit" | "notebookedit" | "applypatch" | "bash" | "codingshell"
    )
}

/// Tools that only read state. Anything not in this whitelist is treated
/// as write-or-unknown by `classify_plan_mode`.
pub(crate) fn is_read_tool(tool: &str) -> bool {
    matches!(
        normalize_tool(tool).as_str(),
        "read" | "grep" | "glob" | "ls" | "codingtodo" | "websearch" | "webfetch" | "lsp"
    )
}

impl ClassifyHook for CodingApprovalPolicy {
    fn classify(&self, tool: &str, _action: Option<&str>, args: &Value) -> Option<ApprovalClass> {
        match self {
            Self::PlanMode { plan_file_path, .. } => {
                Some(classify_plan_mode(tool, args, plan_file_path))
            }
            Self::Default { .. } | Self::YoloMode { .. } => {
                let payload = extract_resource(tool, args)?;
                if self.evaluate_layer1(tool, &payload) {
                    Some(ApprovalClass::Safe)
                } else {
                    Some(ApprovalClass::Destructive)
                }
            }
        }
    }

    fn scope(&self, tool: &str, _action: Option<&str>, args: &Value) -> Option<ApprovalScope> {
        Some(ApprovalScope::ToolActionResource(extract_resource(
            tool, args,
        )?))
    }
}

fn classify_plan_mode(tool: &str, args: &Value, plan_file_path: &std::path::Path) -> ApprovalClass {
    if is_write_tool(tool) {
        let target = extract_resource(tool, args).map(std::path::PathBuf::from);
        match target {
            Some(p) if p == plan_file_path => ApprovalClass::Safe,
            _ => ApprovalClass::Destructive,
        }
    } else if is_read_tool(tool) {
        ApprovalClass::Safe
    } else {
        // Unknown tools (e.g., MCP destructive) treated as writes.
        ApprovalClass::Destructive
    }
}

// ── Matcher (ported from klynt-core/src/approval/matcher.rs) ─────────────

pub struct CompiledRules {
    sets: HashMap<String, (GlobSet, Vec<String>)>,
}

/// Normalized tool name used as the HashMap key.
fn normalize_tool(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '_' | '-'))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

impl CompiledRules {
    pub fn compile(patterns: &[String]) -> Result<Self, String> {
        let mut buckets: HashMap<String, GlobSetBuilder> = Default::default();
        let mut raws: HashMap<String, Vec<String>> = Default::default();
        for p in patterns {
            let (tool, glob) = parse_rule(p)?;
            let key = normalize_tool(&tool);
            buckets
                .entry(key.clone())
                .or_insert_with(GlobSetBuilder::new)
                .add(Glob::new(&glob).map_err(|e| e.to_string())?);
            raws.entry(key).or_default().push(p.clone());
        }
        let sets = buckets
            .into_iter()
            .map(|(t, b)| {
                let r = raws.remove(&t).unwrap_or_default();
                Ok::<_, String>((t, (b.build().map_err(|e| e.to_string())?, r)))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { sets })
    }

    fn find_match(&self, tool: &str, payload: &str) -> Option<String> {
        let key = normalize_tool(tool);
        let (set, raws) = self.sets.get(&key)?;
        let m: Vec<usize> = set.matches(payload);
        m.first().map(|&i| raws[i].clone())
    }
}

fn parse_rule(rule: &str) -> Result<(String, String), String> {
    let open = rule
        .find('(')
        .ok_or_else(|| format!("malformed rule {rule:?}: must be Tool(glob)"))?;
    if !rule.ends_with(')') {
        return Err(format!("malformed rule {rule:?}: must end with ')'"));
    }
    Ok((
        rule[..open].trim().to_string(),
        rule[open + 1..rule.len() - 1].to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_hook_promotes_based_on_layer1() {
        let perms = CodingPermissions {
            allow: vec!["bash(ls *)".into(), "bash(git status)".into()],
            deny: vec!["bash(rm *)".into()],
            ask: vec![],
            default_if_no_match: DefaultPolicy::Ask,
            mirror_learning: false,
            mirror_min_approvals: 5,
            mirror_cooldown_hours: 24,
        };
        let policy = CodingApprovalPolicy::compile(&perms).unwrap();

        // ls should be Safe (allow rule)
        let class = policy.classify("bash", None, &serde_json::json!({"command": "ls -la"}));
        assert_eq!(class, Some(ApprovalClass::Safe));

        // rm should be Destructive (deny rule)
        let class = policy.classify(
            "bash",
            None,
            &serde_json::json!({"command": "rm -rf /tmp/x"}),
        );
        assert_eq!(class, Some(ApprovalClass::Destructive));

        // unknown command → default ask → Destructive
        let class = policy.classify(
            "bash",
            None,
            &serde_json::json!({"command": "curl example.com"}),
        );
        assert_eq!(class, Some(ApprovalClass::Destructive));
    }

    #[test]
    fn scope_returns_resource_for_bash() {
        let perms = CodingPermissions {
            allow: vec![],
            deny: vec![],
            ask: vec![],
            default_if_no_match: DefaultPolicy::Ask,
            mirror_learning: false,
            mirror_min_approvals: 5,
            mirror_cooldown_hours: 24,
        };
        let policy = CodingApprovalPolicy::compile(&perms).unwrap();

        let scope = policy.scope("bash", None, &serde_json::json!({"command": "ls"}));
        assert_eq!(scope, Some(ApprovalScope::ToolActionResource("ls".into())));
    }

    #[test]
    fn non_coding_tool_returns_none() {
        let perms = CodingPermissions {
            allow: vec![],
            deny: vec![],
            ask: vec![],
            default_if_no_match: DefaultPolicy::Ask,
            mirror_learning: false,
            mirror_min_approvals: 5,
            mirror_cooldown_hours: 24,
        };
        let policy = CodingApprovalPolicy::compile(&perms).unwrap();

        let class = policy.classify("notes", Some("read"), &serde_json::json!({}));
        assert_eq!(class, None);
    }

    #[test]
    fn is_write_tool_recognizes_known_writes() {
        for t in ["edit", "write", "multi_edit", "notebook_edit", "bash"] {
            assert!(super::is_write_tool(t), "{t} should be a write tool");
        }
    }

    #[test]
    fn is_read_tool_recognizes_known_reads() {
        for t in [
            "read",
            "grep",
            "glob",
            "coding_todo",
            "web_fetch",
            "web_search",
        ] {
            assert!(super::is_read_tool(t), "{t} should be a read tool");
        }
    }

    #[test]
    fn write_and_read_classifications_are_disjoint() {
        for t in ["edit", "write", "bash"] {
            assert!(super::is_write_tool(t));
            assert!(!super::is_read_tool(t));
        }
    }

    #[test]
    fn plan_mode_allows_edit_to_plan_file_only() {
        use std::path::PathBuf;
        let plan_path = PathBuf::from("/tmp/plan.md");
        let policy = CodingApprovalPolicy::PlanMode {
            plan_session_id: "p_abc".into(),
            plan_file_slug: "plan".into(),
            plan_file_path: plan_path.clone(),
            allow: CompiledRules::compile(&[]).unwrap(),
            deny: CompiledRules::compile(&[]).unwrap(),
            ask: CompiledRules::compile(&[]).unwrap(),
            default_if_no_match: DefaultPolicy::Ask,
        };

        // Edit to plan file → Safe
        let class = policy.classify(
            "edit",
            None,
            &serde_json::json!({"file_path": "/tmp/plan.md"}),
        );
        assert_eq!(class, Some(ApprovalClass::Safe));

        // Edit elsewhere → Destructive
        let class = policy.classify(
            "edit",
            None,
            &serde_json::json!({"file_path": "/tmp/other.rs"}),
        );
        assert_eq!(class, Some(ApprovalClass::Destructive));
    }

    #[test]
    fn plan_mode_allows_reads() {
        let policy = CodingApprovalPolicy::PlanMode {
            plan_session_id: "p_abc".into(),
            plan_file_slug: "plan".into(),
            plan_file_path: "/tmp/plan.md".into(),
            allow: CompiledRules::compile(&[]).unwrap(),
            deny: CompiledRules::compile(&[]).unwrap(),
            ask: CompiledRules::compile(&[]).unwrap(),
            default_if_no_match: DefaultPolicy::Ask,
        };
        let class = policy.classify(
            "read",
            None,
            &serde_json::json!({"file_path": "/tmp/anything.rs"}),
        );
        assert_eq!(class, Some(ApprovalClass::Safe));
    }

    #[test]
    fn plan_mode_rejects_bash() {
        let policy = CodingApprovalPolicy::PlanMode {
            plan_session_id: "p_abc".into(),
            plan_file_slug: "plan".into(),
            plan_file_path: "/tmp/plan.md".into(),
            allow: CompiledRules::compile(&[]).unwrap(),
            deny: CompiledRules::compile(&[]).unwrap(),
            ask: CompiledRules::compile(&[]).unwrap(),
            default_if_no_match: DefaultPolicy::Ask,
        };
        let class = policy.classify("bash", None, &serde_json::json!({"command": "ls"}));
        assert_eq!(class, Some(ApprovalClass::Destructive));
    }
}
