use context_engine::book_index::types::{SourceType, TreeNode};
use sha2::{Digest, Sha256};

/// Build tree nodes from a skill's SKILL.md content.
/// Reuses the same heading-based parser as notes but with SourceType::Skill.
pub fn build_skill_tree(skill_name: &str, content: &str) -> Vec<TreeNode> {
    let body = match skill_system::parser::split_frontmatter(content) {
        Ok((_, body)) => body,
        Err(_) => content.to_string(),
    };
    let mut nodes = cognitive::repos::parse_markdown_to_tree(skill_name, &body);
    for node in &mut nodes {
        node.source_type = SourceType::Skill;
    }
    nodes
}

/// Build all skill trees from the loaded skill store.
/// Returns the SHA-256 checksum of all content for change detection.
pub async fn build_all_skill_trees(
    tree_repo: &dyn context_engine::book_index::BookTreeRepo,
    store: &skill_system::SkillStore,
) -> common::Result<String> {
    let mut hasher = Sha256::new();

    for (name, body) in store.build_reference_index() {
        hasher.update(body.as_bytes());

        tree_repo
            .delete_by_source(&SourceType::Skill, &name)
            .await?;

        let nodes = build_skill_tree(&name, &body);
        if !nodes.is_empty() {
            tree_repo.insert_nodes(&nodes).await?;
            tracing::debug!(
                "BookIndex: built {} tree nodes for skill '{name}'",
                nodes.len()
            );
        }
    }

    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skill_to_tree() {
        let content =
            "---\nname: test-skill\n---\n# Test Skill\n\n## Overview\nA test skill.\n\n## Instructions\nDo this thing.";
        let nodes = build_skill_tree("test-skill", content);
        assert!(
            nodes.len() >= 3,
            "Expected at least 3 nodes, got {}",
            nodes.len()
        );
        assert_eq!(nodes[0].source_type.as_str(), "skill");
    }

    #[test]
    fn no_frontmatter_still_parses() {
        let content = "# Hello\n\nWorld";
        let nodes = build_skill_tree("test", content);
        assert!(!nodes.is_empty());
    }
}
