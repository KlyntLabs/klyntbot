use context_engine::book_index::types::{SourceType, TreeNode, TreeNodeType};
use uuid::Uuid;

/// Parse markdown content into a flat list of TreeNodes with parent-child hierarchy.
/// Headings become Section nodes; paragraphs, code fences, and other content become children.
pub fn parse_markdown_to_tree(source_id: &str, content: &str) -> Vec<TreeNode> {
    let mut nodes: Vec<TreeNode> = Vec::new();
    let mut heading_stack: Vec<(u32, String)> = Vec::new(); // (level, id)
    let mut in_code_block = false;
    let mut code_buffer = String::new();
    let mut code_parent_id: Option<String> = None;
    let mut text_buffer = String::new();
    let mut position: u32 = 0;

    let flush_text = |buffer: &mut String,
                      nodes: &mut Vec<TreeNode>,
                      parent_id: &Option<String>,
                      pos: &mut u32| {
        let trimmed = buffer.trim();
        if !trimmed.is_empty() {
            let level: u32 = parent_id
                .as_ref()
                .and_then(|pid| nodes.iter().find(|n: &&TreeNode| n.id == *pid))
                .map(|n| n.level + 1)
                .unwrap_or(0);
            nodes.push(TreeNode {
                id: Uuid::new_v4().to_string(),
                parent_id: parent_id.clone(),
                node_type: TreeNodeType::Text,
                content: trimmed.to_string(),
                title: None,
                level,
                source_type: SourceType::Note,
                source_id: String::new(), // filled by caller
                position: *pos,
                metadata: None,
            });
            *pos += 1;
        }
        buffer.clear();
    };

    let current_parent_id =
        |stack: &[(u32, String)]| -> Option<String> { stack.last().map(|(_, id)| id.clone()) };

    for line in content.lines() {
        if in_code_block {
            if line.trim_start().starts_with("```") {
                // End code block
                in_code_block = false;
                let code_content = code_buffer.trim().to_string();
                if !code_content.is_empty() {
                    let level = code_parent_id
                        .as_ref()
                        .and_then(|pid| nodes.iter().find(|n| n.id == *pid))
                        .map(|n| n.level + 1)
                        .unwrap_or(0);
                    nodes.push(TreeNode {
                        id: Uuid::new_v4().to_string(),
                        parent_id: code_parent_id.clone(),
                        node_type: TreeNodeType::Code,
                        content: code_content,
                        title: None,
                        level,
                        source_type: SourceType::Note,
                        source_id: source_id.to_string(),
                        position,
                        metadata: None,
                    });
                    position += 1;
                }
                code_buffer.clear();
            } else {
                code_buffer.push_str(line);
                code_buffer.push('\n');
            }
            continue;
        }

        if line.trim_start().starts_with("```") {
            // Start code block — flush any pending text first
            let parent_id = current_parent_id(&heading_stack);
            flush_text(&mut text_buffer, &mut nodes, &parent_id, &mut position);
            // Fix source_id on last node
            if let Some(last) = nodes.last_mut() {
                if last.source_id.is_empty() {
                    last.source_id = source_id.to_string();
                }
            }
            in_code_block = true;
            code_parent_id = parent_id;
            code_buffer.clear();
            continue;
        }

        // Check for heading
        if let Some(heading_level) = parse_heading_level(line) {
            // Flush pending text
            let parent_id = current_parent_id(&heading_stack);
            flush_text(&mut text_buffer, &mut nodes, &parent_id, &mut position);
            if let Some(last) = nodes.last_mut() {
                if last.source_id.is_empty() {
                    last.source_id = source_id.to_string();
                }
            }

            // Pop stack to find correct parent
            while heading_stack
                .last()
                .is_some_and(|(lvl, _)| *lvl >= heading_level)
            {
                heading_stack.pop();
            }
            let parent_id = current_parent_id(&heading_stack);

            let title = line.trim_start_matches('#').trim().to_string();
            let section_id = Uuid::new_v4().to_string();

            nodes.push(TreeNode {
                id: section_id.clone(),
                parent_id,
                node_type: TreeNodeType::Section,
                content: title.clone(),
                title: Some(title),
                level: heading_level,
                source_type: SourceType::Note,
                source_id: source_id.to_string(),
                position,
                metadata: None,
            });
            position += 1;

            heading_stack.push((heading_level, section_id));
        } else {
            // Regular text line
            text_buffer.push_str(line);
            text_buffer.push('\n');
        }
    }

    // Flush remaining text
    let parent_id = current_parent_id(&heading_stack);
    flush_text(&mut text_buffer, &mut nodes, &parent_id, &mut position);
    if let Some(last) = nodes.last_mut() {
        if last.source_id.is_empty() {
            last.source_id = source_id.to_string();
        }
    }

    // Flush remaining code block if unclosed
    if in_code_block && !code_buffer.trim().is_empty() {
        let level = code_parent_id
            .as_ref()
            .and_then(|pid| nodes.iter().find(|n| n.id == *pid))
            .map(|n| n.level + 1)
            .unwrap_or(0);
        nodes.push(TreeNode {
            id: Uuid::new_v4().to_string(),
            parent_id: code_parent_id,
            node_type: TreeNodeType::Code,
            content: code_buffer.trim().to_string(),
            title: None,
            level,
            source_type: SourceType::Note,
            source_id: source_id.to_string(),
            position,
            metadata: None,
        });
    }

    nodes
}

fn parse_heading_level(line: &str) -> Option<u32> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|&c| c == '#').count();
    // Must have a space after the hashes (e.g., "# Title" not "#notaheading")
    if level > 0 && level <= 6 && trimmed.len() > level && trimmed.as_bytes()[level] == b' ' {
        Some(level as u32)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_markdown() {
        let md = "# Chapter 1\nSome text.\n## Section 1.1\nMore text.\n## Section 1.2\nFinal.";
        let nodes = parse_markdown_to_tree("note-1", md);
        // 3 sections + 3 text blocks = 6
        assert_eq!(nodes.len(), 6);
        assert_eq!(nodes[0].node_type.as_str(), "section");
        assert_eq!(nodes[0].level, 1);
        assert_eq!(nodes[1].node_type.as_str(), "text");
        assert_eq!(nodes[1].parent_id, Some(nodes[0].id.clone()));
    }

    #[test]
    fn parse_code_blocks() {
        let md = "# Title\n```rust\nfn main() {}\n```\nAfter code.";
        let nodes = parse_markdown_to_tree("note-1", md);
        let code_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| matches!(n.node_type, TreeNodeType::Code))
            .collect();
        assert_eq!(code_nodes.len(), 1);
        assert!(code_nodes[0].content.contains("fn main()"));
    }

    #[test]
    fn parse_nested_headings() {
        let md = "# H1\n## H2\n### H3\nDeep content.";
        let nodes = parse_markdown_to_tree("note-1", md);
        assert_eq!(nodes.len(), 4); // 3 sections + 1 text
        assert_eq!(nodes[2].level, 3); // H3
        assert_eq!(nodes[2].parent_id, Some(nodes[1].id.clone())); // H3 parent is H2
        assert_eq!(nodes[3].parent_id, Some(nodes[2].id.clone())); // text parent is H3
    }

    #[test]
    fn parse_no_headings() {
        let md = "Just some plain text\nwith multiple lines.";
        let nodes = parse_markdown_to_tree("note-1", md);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_type.as_str(), "text");
        assert!(nodes[0].parent_id.is_none());
    }
}
