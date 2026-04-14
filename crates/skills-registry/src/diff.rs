use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

use crate::SkillPackage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffResult {
    pub body_lines: Vec<DiffLine>,
    pub frontmatter_changes: Vec<FrontmatterChange>,
    pub bootstraps_added: Vec<String>,
    pub bootstraps_removed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub tag: String, // "equal" | "insert" | "delete"
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontmatterChange {
    pub field: String,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}

pub fn diff_packages(before: &SkillPackage, after: &SkillPackage) -> DiffResult {
    let body_diff = TextDiff::from_lines(&before.skill_md_content, &after.skill_md_content);
    let body_lines: Vec<DiffLine> = body_diff
        .iter_all_changes()
        .map(|c| DiffLine {
            tag: match c.tag() {
                ChangeTag::Equal => "equal",
                ChangeTag::Insert => "insert",
                ChangeTag::Delete => "delete",
            }
            .to_string(),
            text: c.to_string(),
        })
        .collect();

    let before_fm = frontmatter_fields(before);
    let after_fm = frontmatter_fields(after);
    let mut frontmatter_changes = Vec::new();
    for (k, b_val) in &before_fm {
        match after_fm.get(k) {
            Some(a_val) if a_val != b_val => frontmatter_changes.push(FrontmatterChange {
                field: k.clone(),
                before: Some(b_val.clone()),
                after: Some(a_val.clone()),
            }),
            None => frontmatter_changes.push(FrontmatterChange {
                field: k.clone(),
                before: Some(b_val.clone()),
                after: None,
            }),
            _ => {}
        }
    }
    for (k, a_val) in &after_fm {
        if !before_fm.contains_key(k) {
            frontmatter_changes.push(FrontmatterChange {
                field: k.clone(),
                before: None,
                after: Some(a_val.clone()),
            });
        }
    }

    let before_boot: std::collections::HashSet<_> = bootstrap_names(before).into_iter().collect();
    let after_boot: std::collections::HashSet<_> = bootstrap_names(after).into_iter().collect();
    let bootstraps_added = after_boot.difference(&before_boot).cloned().collect();
    let bootstraps_removed = before_boot.difference(&after_boot).cloned().collect();

    DiffResult {
        body_lines,
        frontmatter_changes,
        bootstraps_added,
        bootstraps_removed,
    }
}

fn frontmatter_fields(pkg: &SkillPackage) -> std::collections::BTreeMap<String, serde_json::Value> {
    let mut out = std::collections::BTreeMap::new();
    out.insert(
        "name".into(),
        serde_json::Value::String(pkg.frontmatter.name.clone()),
    );
    out.insert(
        "description".into(),
        serde_json::Value::String(pkg.frontmatter.description.clone()),
    );
    if let Some(ref w) = pkg.frontmatter.when_to_use {
        out.insert("whenToUse".into(), serde_json::Value::String(w.clone()));
    }
    if let Some(ref m) = pkg.klyntbot_meta {
        if !m.triggers.is_empty() {
            out.insert(
                "triggers".into(),
                serde_json::Value::Array(
                    m.triggers
                        .iter()
                        .map(|t| serde_json::Value::String(t.clone()))
                        .collect(),
                ),
            );
        }
    }
    out
}

fn bootstrap_names(_pkg: &SkillPackage) -> Vec<String> {
    // KlyntbotMeta does not carry a bootstraps bag; no bootstrap names to extract.
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pkg(body: &str, name: &str) -> SkillPackage {
        use crate::source::{GitRef, SkillSource};
        use skill_system::store::SkillFrontmatter;
        SkillPackage {
            name: name.into(),
            source: SkillSource::Github {
                owner: "a".into(),
                repo: "b".into(),
                subpath: "c".into(),
                r#ref: GitRef::Latest,
            },
            resolved_sha: "s".into(),
            semver: None,
            skill_md_content: body.into(),
            frontmatter: SkillFrontmatter {
                name: name.into(),
                description: "d".into(),
                when_to_use: None,
                references: vec![],
            },
            klyntbot_meta: None,
            references: vec![],
            templates: vec![],
        }
    }

    #[test]
    fn body_diff_detects_insertions() {
        let a = make_pkg("line one\nline two\n", "x");
        let b = make_pkg("line one\nline two\nline three\n", "x");
        let d = diff_packages(&a, &b);
        assert!(d
            .body_lines
            .iter()
            .any(|l| l.tag == "insert" && l.text.contains("three")));
    }
}
