use context_engine::book_index::types::{SourceType, TreeNode, TreeNodeType};
use serde_json::Value;
use uuid::Uuid;

/// Parse a Tiptap JSON document into a flat list of `TreeNode`s with parent-child hierarchy.
///
/// This is the primary parser for note content. Headings become Section nodes,
/// paragraphs and blockquotes accumulate as Text children, and lists become
/// level-7 pseudo-section nodes. Falls back to an empty `Vec` on invalid JSON.
pub fn parse_tiptap_to_tree(source_id: &str, json_str: &str) -> Vec<TreeNode> {
    let doc: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let content = match doc.get("content").and_then(|c| c.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    // Root node (level 0)
    let root_id = Uuid::new_v4().to_string();
    let mut nodes = vec![TreeNode {
        id: root_id.clone(),
        parent_id: None,
        node_type: TreeNodeType::Section,
        content: source_id.to_string(),
        title: Some(source_id.to_string()),
        level: 0,
        source_type: SourceType::Note,
        source_id: source_id.to_string(),
        position: 0,
        metadata: None,
    }];

    // heading_stack tracks (level, node_id) for establishing parent hierarchy.
    // The root node is always at the bottom of the stack.
    let mut heading_stack: Vec<(u32, String)> = vec![(0, root_id)];
    let mut position: u32 = 1;

    for block in content {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match block_type {
            "heading" => {
                let level = block
                    .get("attrs")
                    .and_then(|a| a.get("level"))
                    .and_then(|l| l.as_u64())
                    .unwrap_or(1) as u32;

                let title = extract_text(block);

                // Pop stack entries with level >= this heading's level to find the correct parent
                while heading_stack
                    .last()
                    .is_some_and(|(lvl, _)| *lvl >= level)
                {
                    heading_stack.pop();
                }
                let parent_id = heading_stack.last().map(|(_, id)| id.clone());

                let section_id = Uuid::new_v4().to_string();
                nodes.push(TreeNode {
                    id: section_id.clone(),
                    parent_id,
                    node_type: TreeNodeType::Section,
                    content: title.clone(),
                    title: Some(title),
                    level,
                    source_type: SourceType::Note,
                    source_id: source_id.to_string(),
                    position,
                    metadata: None,
                });
                position += 1;

                heading_stack.push((level, section_id));
            }
            "paragraph" | "blockquote" => {
                let text = extract_text(block);
                if !text.is_empty() {
                    let metadata = if block_type == "blockquote" {
                        Some(r#"{"block_type":"blockquote"}"#.to_string())
                    } else {
                        None
                    };
                    push_text_node(
                        &mut nodes, &heading_stack, source_id, &mut position, text, metadata,
                    );
                }
            }
            "bulletList" | "orderedList" | "taskList" => {
                let items_text = extract_list_text(block);
                if !items_text.is_empty() {
                    let parent_id = heading_stack.last().map(|(_, id)| id.clone());
                    let node_type = if block_type == "taskList" {
                        TreeNodeType::Task
                    } else {
                        TreeNodeType::ListItem
                    };
                    nodes.push(TreeNode {
                        id: Uuid::new_v4().to_string(),
                        parent_id,
                        node_type,
                        content: items_text,
                        title: None,
                        level: 7,
                        source_type: SourceType::Note,
                        source_id: source_id.to_string(),
                        position,
                        metadata: Some(
                            format!(r#"{{"block_type":"{}"}}"#, block_type),
                        ),
                    });
                    position += 1;
                }
            }
            _ => {
                let text = extract_text(block);
                if !text.is_empty() {
                    push_text_node(
                        &mut nodes, &heading_stack, source_id, &mut position, text, None,
                    );
                }
            }
        }
    }

    nodes
}

fn push_text_node(
    nodes: &mut Vec<TreeNode>,
    heading_stack: &[(u32, String)],
    source_id: &str,
    position: &mut u32,
    content: String,
    metadata: Option<String>,
) {
    let (parent_id, level) = match heading_stack.last() {
        Some((l, id)) => (Some(id.clone()), l + 1),
        None => (None, 1),
    };
    nodes.push(TreeNode {
        id: Uuid::new_v4().to_string(),
        parent_id,
        node_type: TreeNodeType::Text,
        content,
        title: None,
        level,
        source_type: SourceType::Note,
        source_id: source_id.to_string(),
        position: *position,
        metadata,
    });
    *position += 1;
}

/// Recursively extract all text content from a Tiptap node.
fn extract_text(node: &Value) -> String {
    let mut result = String::new();
    extract_text_recursive(node, &mut result);
    let trimmed = result.trim();
    if trimmed.len() == result.len() {
        result
    } else {
        trimmed.to_string()
    }
}

fn extract_text_recursive(node: &Value, out: &mut String) {
    if let Some(node_type) = node.get("type").and_then(|t| t.as_str()) {
        if node_type == "text" {
            if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
                out.push_str(text);
            }
            return;
        }
    }

    if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
        for child in content {
            extract_text_recursive(child, out);
        }
    }
}

/// Extract concatenated text from all list items in a list node.
fn extract_list_text(list_node: &Value) -> String {
    let mut result = String::new();

    if let Some(content) = list_node.get("content").and_then(|c| c.as_array()) {
        for item in content {
            let text = extract_text(item);
            if !text.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&text);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tiptap_headings() {
        let json = r#"{
            "type": "doc",
            "content": [
                {
                    "type": "heading",
                    "attrs": { "level": 1 },
                    "content": [{ "type": "text", "text": "Main Title" }]
                },
                {
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "Intro paragraph" }]
                },
                {
                    "type": "heading",
                    "attrs": { "level": 2 },
                    "content": [{ "type": "text", "text": "Subsection" }]
                },
                {
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "Subsection content" }]
                }
            ]
        }"#;

        let nodes = parse_tiptap_to_tree("note-123", json);

        // root + h1 + paragraph + h2 + paragraph = 5
        assert_eq!(nodes.len(), 5);

        // Root node
        assert_eq!(nodes[0].level, 0);
        assert_eq!(nodes[0].title.as_deref(), Some("note-123"));
        assert!(nodes[0].parent_id.is_none());

        // H1 — level 1, parent is root
        assert_eq!(nodes[1].level, 1);
        assert_eq!(nodes[1].title.as_deref(), Some("Main Title"));
        assert_eq!(nodes[1].content, "Main Title");
        assert_eq!(nodes[1].parent_id, Some(nodes[0].id.clone()));

        // Paragraph after H1 — child of H1
        assert_eq!(nodes[2].level, 2);
        assert_eq!(nodes[2].content, "Intro paragraph");
        assert_eq!(nodes[2].parent_id, Some(nodes[1].id.clone()));

        // H2 — level 2, parent is H1
        assert_eq!(nodes[3].level, 2);
        assert_eq!(nodes[3].title.as_deref(), Some("Subsection"));
        assert_eq!(nodes[3].parent_id, Some(nodes[1].id.clone()));

        // Paragraph after H2 — child of H2
        assert_eq!(nodes[4].level, 3);
        assert_eq!(nodes[4].content, "Subsection content");
        assert_eq!(nodes[4].parent_id, Some(nodes[3].id.clone()));
    }

    #[test]
    fn test_parse_tiptap_bullet_list_as_pseudo_section() {
        let json = r#"{
            "type": "doc",
            "content": [
                {
                    "type": "bulletList",
                    "content": [
                        {
                            "type": "listItem",
                            "content": [
                                { "type": "paragraph", "content": [{ "type": "text", "text": "Item A" }] }
                            ]
                        },
                        {
                            "type": "listItem",
                            "content": [
                                { "type": "paragraph", "content": [{ "type": "text", "text": "Item B" }] }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let nodes = parse_tiptap_to_tree("note-456", json);

        // root + list pseudo-section = 2
        assert_eq!(nodes.len(), 2);

        let list_node = &nodes[1];
        assert_eq!(list_node.level, 7);
        assert!(list_node.content.contains("Item A"));
        assert!(list_node.content.contains("Item B"));
        assert_eq!(list_node.parent_id, Some(nodes[0].id.clone()));
        assert_eq!(list_node.node_type.as_str(), "ListItem");
    }

    #[test]
    fn test_parse_tiptap_task_list() {
        let json = r#"{
            "type": "doc",
            "content": [
                {
                    "type": "taskList",
                    "content": [
                        {
                            "type": "taskItem",
                            "attrs": { "checked": false },
                            "content": [
                                { "type": "paragraph", "content": [{ "type": "text", "text": "Buy groceries" }] }
                            ]
                        },
                        {
                            "type": "taskItem",
                            "attrs": { "checked": true },
                            "content": [
                                { "type": "paragraph", "content": [{ "type": "text", "text": "Walk the dog" }] }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let nodes = parse_tiptap_to_tree("note-789", json);

        // root + task list pseudo-section = 2
        assert_eq!(nodes.len(), 2);

        let task_node = &nodes[1];
        assert_eq!(task_node.level, 7);
        assert!(task_node.content.contains("Buy groceries"));
        assert!(task_node.content.contains("Walk the dog"));
        assert_eq!(task_node.node_type.as_str(), "Task");
        assert_eq!(task_node.parent_id, Some(nodes[0].id.clone()));
    }

    #[test]
    fn test_parse_tiptap_fallback_on_invalid_json() {
        let nodes = parse_tiptap_to_tree("note-bad", "not valid json");
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_parse_tiptap_empty_doc() {
        let json = r#"{"type": "doc", "content": []}"#;
        let nodes = parse_tiptap_to_tree("note-empty", json);

        // Only the root node
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].level, 0);
        assert_eq!(nodes[0].title.as_deref(), Some("note-empty"));
    }

    #[test]
    fn test_parse_tiptap_nested_headings() {
        let json = r#"{
            "type": "doc",
            "content": [
                {
                    "type": "heading",
                    "attrs": { "level": 1 },
                    "content": [{ "type": "text", "text": "H1" }]
                },
                {
                    "type": "heading",
                    "attrs": { "level": 2 },
                    "content": [{ "type": "text", "text": "H2" }]
                },
                {
                    "type": "heading",
                    "attrs": { "level": 3 },
                    "content": [{ "type": "text", "text": "H3" }]
                }
            ]
        }"#;

        let nodes = parse_tiptap_to_tree("note-nested", json);

        // root + h1 + h2 + h3 = 4
        assert_eq!(nodes.len(), 4);

        // H1 parent is root
        assert_eq!(nodes[1].title.as_deref(), Some("H1"));
        assert_eq!(nodes[1].parent_id, Some(nodes[0].id.clone()));

        // H2 parent is H1
        assert_eq!(nodes[2].title.as_deref(), Some("H2"));
        assert_eq!(nodes[2].parent_id, Some(nodes[1].id.clone()));

        // H3 parent is H2
        assert_eq!(nodes[3].title.as_deref(), Some("H3"));
        assert_eq!(nodes[3].parent_id, Some(nodes[2].id.clone()));
    }
}
