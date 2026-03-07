//! Parses wiki-links and entity mentions from note HTML content.

use std::collections::HashSet;

/// A parsed entity mention like `@task:abc123` or `@project:def456`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityMention {
    pub entity_type: String,
    pub entity_id: String,
}

/// Extract note IDs from wiki-link spans in HTML.
///
/// Looks for `<span data-wiki-link ... data-note-id="...">` patterns.
pub fn extract_wiki_link_ids(html: &str) -> Vec<String> {
    let mut ids = HashSet::new();
    let marker = "data-wiki-link";
    let mut search_from = 0;
    while let Some(start) = html[search_from..].find(marker) {
        let abs_start = search_from + start;
        let after_marker = &html[abs_start..];

        // Search within the same tag (up to closing >)
        let tag_end = after_marker.find('>').unwrap_or(after_marker.len());
        let within_tag = &after_marker[..tag_end];

        if let Some(attr_start) = within_tag.find("data-note-id=\"") {
            let value_start = attr_start + "data-note-id=\"".len();
            if let Some(value_end) = within_tag[value_start..].find('"') {
                let note_id = &within_tag[value_start..value_start + value_end];
                if !note_id.is_empty() {
                    ids.insert(note_id.to_string());
                }
            }
        }

        // Skip past the tag to avoid re-examining the same attribute
        search_from = abs_start + tag_end.max(marker.len());
    }

    ids.into_iter().collect()
}

/// Extract `[[Title]]` wiki-link titles from plain text.
pub fn extract_wiki_link_titles(text: &str) -> Vec<String> {
    let mut titles = HashSet::new();
    let mut search_from = 0;

    while let Some(open) = text[search_from..].find("[[") {
        let abs_open = search_from + open;
        let title_start = abs_open + 2;

        if let Some(close) = text[title_start..].find("]]") {
            let title = text[title_start..title_start + close].trim();
            // Support [[note|alias]] syntax — use the note part
            let note_ref = title.split('|').next().unwrap_or(title).trim();
            // Strip #heading and ^block references
            let clean = note_ref
                .split('#')
                .next()
                .unwrap_or(note_ref)
                .split('^')
                .next()
                .unwrap_or(note_ref)
                .trim();
            if !clean.is_empty() {
                titles.insert(clean.to_string());
            }
            search_from = title_start + close + 2;
        } else {
            break;
        }
    }

    titles.into_iter().collect()
}

/// Extract entity mentions like `@task:id` or `@project:id` from text.
pub fn extract_entity_mentions(text: &str) -> Vec<EntityMention> {
    let mut mentions = HashSet::new();
    let mut search_from = 0;

    while let Some(at_pos) = text[search_from..].find('@') {
        let abs_at = search_from + at_pos;
        let after_at = &text[abs_at + 1..];

        if let Some(colon) = after_at.find(':') {
            let entity_type = &after_at[..colon];
            // Validate entity type is alphanumeric/underscore
            if entity_type.chars().all(|c| c.is_alphanumeric() || c == '_')
                && !entity_type.is_empty()
            {
                let id_start = colon + 1;
                // ID continues until whitespace or end
                let id_end = after_at[id_start..]
                    .find(|c: char| c.is_whitespace() || c == ',' || c == ')' || c == ']')
                    .unwrap_or(after_at.len() - id_start);
                let entity_id = &after_at[id_start..id_start + id_end];
                if !entity_id.is_empty() {
                    mentions.insert(EntityMention {
                        entity_type: entity_type.to_string(),
                        entity_id: entity_id.to_string(),
                    });
                }
            }
        }

        search_from = abs_at + 1;
    }

    mentions.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_wiki_link_ids_from_html() {
        let html = r#"<p>Hello <span data-wiki-link="" data-note-id="abc123" data-title="My Note">My Note</span> world</p>"#;
        let ids = extract_wiki_link_ids(html);
        assert_eq!(ids, vec!["abc123"]);
    }

    #[test]
    fn test_extract_wiki_link_ids_multiple() {
        let html = r#"<span data-wiki-link="" data-note-id="a1">X</span> and <span data-wiki-link="" data-note-id="b2">Y</span>"#;
        let mut ids = extract_wiki_link_ids(html);
        ids.sort();
        assert_eq!(ids, vec!["a1", "b2"]);
    }

    #[test]
    fn test_extract_wiki_link_ids_empty() {
        let html = r#"<span data-wiki-link="" data-note-id="">empty</span>"#;
        assert!(extract_wiki_link_ids(html).is_empty());
    }

    #[test]
    fn test_extract_wiki_link_titles() {
        let text = "See [[My Note]] and [[Other|alias]] for details";
        let mut titles = extract_wiki_link_titles(text);
        titles.sort();
        assert_eq!(titles, vec!["My Note", "Other"]);
    }

    #[test]
    fn test_extract_wiki_link_titles_with_heading() {
        let text = "See [[Note#section]] here";
        let titles = extract_wiki_link_titles(text);
        assert_eq!(titles, vec!["Note"]);
    }

    #[test]
    fn test_extract_entity_mentions() {
        let text = "Working on @task:abc123 in @project:def456 today";
        let mut mentions = extract_entity_mentions(text);
        mentions.sort_by(|a, b| a.entity_type.cmp(&b.entity_type));
        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0].entity_type, "project");
        assert_eq!(mentions[0].entity_id, "def456");
        assert_eq!(mentions[1].entity_type, "task");
        assert_eq!(mentions[1].entity_id, "abc123");
    }

    #[test]
    fn test_extract_entity_mentions_empty() {
        assert!(extract_entity_mentions("no mentions here").is_empty());
        assert!(extract_entity_mentions("email@example.com").is_empty());
    }
}
