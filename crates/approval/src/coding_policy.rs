use crate::class::{ApprovalClass, ApprovalScope};
use crate::policy::ClassifyHook;
use config::schema::{CodingPermissions, DefaultPolicy};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_json::Value;
use std::collections::HashMap;

pub struct CodingApprovalPolicy {
    allow: CompiledRules,
    deny: CompiledRules,
    ask: CompiledRules,
    default_if_no_match: DefaultPolicy,
}

impl CodingApprovalPolicy {
    pub fn compile(permissions: &CodingPermissions) -> Result<Self, String> {
        Ok(Self {
            allow: CompiledRules::compile(&permissions.allow).map_err(|e| e.to_string())?,
            deny: CompiledRules::compile(&permissions.deny).map_err(|e| e.to_string())?,
            ask: CompiledRules::compile(&permissions.ask).map_err(|e| e.to_string())?,
            default_if_no_match: permissions.default_if_no_match,
        })
    }

    /// Evaluate a tool+payload against the layer1 rules.
    /// Returns true if allowed, false if should be prompted/denied.
    fn evaluate_layer1(&self, tool: &str, payload: &str) -> bool {
        if self.deny.find_match(tool, payload).is_some() {
            return false;
        }
        if self.allow.find_match(tool, payload).is_some() {
            return true;
        }
        if self.ask.find_match(tool, payload).is_some() {
            return false;
        }
        self.default_if_no_match == DefaultPolicy::Allow
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

impl ClassifyHook for CodingApprovalPolicy {
    fn classify(&self, tool: &str, _action: Option<&str>, args: &Value) -> Option<ApprovalClass> {
        let payload = extract_resource(tool, args)?;
        if self.evaluate_layer1(tool, &payload) {
            Some(ApprovalClass::Safe)
        } else {
            Some(ApprovalClass::Destructive)
        }
    }

    fn scope(&self, tool: &str, _action: Option<&str>, args: &Value) -> Option<ApprovalScope> {
        Some(ApprovalScope::ToolActionResource(extract_resource(
            tool, args,
        )?))
    }
}

// ── Matcher (ported from klynt-core/src/approval/matcher.rs) ─────────────

struct CompiledRules {
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
    fn compile(patterns: &[String]) -> Result<Self, String> {
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
}
