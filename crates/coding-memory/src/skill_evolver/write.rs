//! Write SKILL.md bodies via atomic managed-block writes.

use crate::reforge::managed_block::ManagedBlock;
use crate::reforge::types::ProjectSkillSpec;
use common::{KlyntbotError, Result};
use std::path::PathBuf;
use storage::StoragePool;

/// Arguments for one skill write + journal entry.
#[derive(Debug, Clone)]
pub struct JournalArgs {
    /// Repo scope.
    pub repo_id: String,
    /// Absolute path to the SKILL.md file.
    pub skill_path: PathBuf,
    /// Reforge cycle id stamped into the managed block.
    pub cycle_id: String,
    /// Skill spec driving the body.
    pub spec: ProjectSkillSpec,
}

/// Outcome of `write_skill_md`.
#[derive(Debug, Clone)]
pub enum SkillWriteOutcome {
    /// File written and version assigned.
    Written {
        /// Version number (1-based).
        version: i64,
        /// Final absolute path.
        path: PathBuf,
    },
    /// Write was skipped (e.g. identical body).
    Skipped {
        /// Human-readable reason.
        reason: String,
    },
}

/// Render a markdown body from a [`ProjectSkillSpec`].
pub fn render_skill_body(spec: &ProjectSkillSpec) -> String {
    let mut body = String::new();
    body.push_str(&format!("# {}\n\n", spec.name));
    body.push_str(&format!("## Description\n\n{}\n\n", spec.description));
    if !spec.when_to_use.is_empty() {
        body.push_str("## When to Use\n\n");
        for item in &spec.when_to_use {
            body.push_str(&format!("- {item}\n"));
        }
        body.push('\n');
    }
    body.push_str("## Procedure\n\n");
    body.push_str(&spec.procedure);
    body.push_str("\n\n");
    if !spec.references.is_empty() {
        body.push_str("## References\n\n");
        for r#ref in &spec.references {
            body.push_str(&format!("- {ref}\n"));
        }
        body.push('\n');
    }
    if !spec.anchored_symbols.is_empty() {
        body.push_str("## Anchored Symbols\n\n");
        for sym in &spec.anchored_symbols {
            body.push_str(&format!("- `{}`\n", sym.symbol));
        }
        body.push('\n');
    }
    body
}

/// Sanitize a string into a kebab-case slug.
pub fn sanitize(s: &str) -> String {
    let mut out = String::new();
    let mut prev_hyphen = false;
    for c in s.to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
            prev_hyphen = false;
        } else if !prev_hyphen && !out.is_empty() {
            out.push('-');
            prev_hyphen = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

/// Return the next version number for a skill name.
pub async fn next_version_for(pool: &StoragePool, skill_name: &str) -> Result<i64> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT MAX(version) FROM skill_versions WHERE skill_name = ?1")
            .bind(skill_name)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("next_version_for: {e}")))?;
    Ok(row.map(|r| r.0).unwrap_or(0) + 1)
}

/// Atomically write a SKILL.md file using a managed block.
pub async fn write_skill_md(pool: &StoragePool, args: &JournalArgs) -> Result<SkillWriteOutcome> {
    let body = render_skill_body(&args.spec);
    let block = ManagedBlock::read(&args.skill_path)
        .map_err(|e| KlyntbotError::Storage(format!("managed_block read: {e}")))?;

    block
        .write_with_new_inside(&args.skill_path, &body, &args.cycle_id)
        .map_err(|e| KlyntbotError::Storage(format!("managed_block write: {e}")))?;

    let version = next_version_for(pool, &args.spec.skill_id).await?;

    Ok(SkillWriteOutcome::Written {
        version,
        path: args.skill_path.clone(),
    })
}
