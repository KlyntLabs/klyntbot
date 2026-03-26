//! YAML front matter parsing and serialization for Markdown note files.
//!
//! Supports the standard `---` delimited front matter format used by static site
//! generators, Obsidian, and other Markdown-based tools.

use serde::{Deserialize, Serialize};

/// Metadata extracted from YAML front matter in a Markdown file.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NoteFrontMatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Result of parsing a Markdown file that may contain front matter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMarkdown {
    /// Parsed front matter, if present and valid.
    pub front_matter: Option<NoteFrontMatter>,
    /// The Markdown body (everything after the front matter block).
    pub body: String,
    /// Warning message if front matter was present but malformed.
    pub warning: Option<String>,
}

/// Parse a Markdown string, extracting optional YAML front matter.
///
/// Front matter is only recognized when the file starts with `---\n` at byte 0.
/// The closing delimiter is `\n---\n` or `\n---` at EOF.
pub fn parse(content: &str) -> ParsedMarkdown {
    // Only parse front matter when file starts with exactly "---\n"
    if !content.starts_with("---\n") {
        return ParsedMarkdown {
            front_matter: None,
            body: content.to_string(),
            warning: None,
        };
    }

    let after_opening = &content[4..];

    // Find closing delimiter: "\n---\n" or "\n---" at EOF
    let close_pos = match after_opening.find("\n---\n") {
        Some(pos) => Some((pos, 5)), // "\n---\n" is 5 bytes
        None => {
            // Check for "\n---" at EOF
            if after_opening.ends_with("\n---") {
                Some((after_opening.len() - 4, 4))
            } else {
                None
            }
        }
    };

    let (close_pos, delimiter_len) = match close_pos {
        Some(pair) => pair,
        None => {
            // No closing delimiter found — not front matter
            return ParsedMarkdown {
                front_matter: None,
                body: content.to_string(),
                warning: None,
            };
        }
    };

    let yaml_str = &after_opening[..close_pos];
    let body_start = 4 + close_pos + delimiter_len;
    let body = if body_start < content.len() {
        let raw = &content[body_start..];
        // Trim exactly one leading newline, not all
        if let Some(stripped) = raw.strip_prefix('\n') {
            stripped.to_string()
        } else {
            raw.to_string()
        }
    } else {
        String::new()
    };

    match serde_yml::from_str::<NoteFrontMatter>(yaml_str) {
        Ok(fm) => ParsedMarkdown {
            front_matter: Some(fm),
            body,
            warning: None,
        },
        Err(e) => ParsedMarkdown {
            front_matter: None,
            body: content.to_string(),
            warning: Some(format!("Failed to parse front matter: {e}")),
        },
    }
}

/// Serialize front matter and body into a complete Markdown string.
///
/// Output format: `---\n{yaml}---\n\n{body}`
pub fn serialize(front_matter: &NoteFrontMatter, body: &str) -> String {
    let yaml = serde_yml::to_string(front_matter).unwrap_or_default();
    format!("---\n{yaml}---\n\n{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_front_matter() {
        let input =
            "---\ntitle: My Note\ntags:\n  - rust\n  - notes\npinned: true\n---\n\nHello world";
        let result = parse(input);
        assert!(result.warning.is_none());
        let fm = result.front_matter.unwrap();
        assert_eq!(fm.title, Some("My Note".to_string()));
        assert_eq!(fm.tags, Some(vec!["rust".to_string(), "notes".to_string()]));
        assert_eq!(fm.pinned, Some(true));
        assert_eq!(result.body, "Hello world");
    }

    #[test]
    fn parse_no_front_matter() {
        let input = "Just a regular markdown file.\n\nWith paragraphs.";
        let result = parse(input);
        assert!(result.front_matter.is_none());
        assert!(result.warning.is_none());
        assert_eq!(result.body, input);
    }

    #[test]
    fn parse_horizontal_rule_not_treated_as_front_matter() {
        let input = "Some text\n\n---\n\nMore text";
        let result = parse(input);
        assert!(result.front_matter.is_none());
        assert!(result.warning.is_none());
        assert_eq!(result.body, input);
    }

    #[test]
    fn parse_malformed_yaml() {
        let input = "---\ntitle: [unclosed bracket\n---\n\nBody text";
        let result = parse(input);
        assert!(result.front_matter.is_none());
        assert!(result.warning.is_some());
        assert!(result.warning.unwrap().contains("Failed to parse"));
        // On malformed YAML, full content returned as body
        assert_eq!(result.body, input);
    }

    #[test]
    fn parse_front_matter_only_no_body() {
        let input = "---\ntitle: Empty Note\n---";
        let result = parse(input);
        assert!(result.warning.is_none());
        let fm = result.front_matter.unwrap();
        assert_eq!(fm.title, Some("Empty Note".to_string()));
        assert_eq!(result.body, "");
    }

    #[test]
    fn parse_empty_file() {
        let result = parse("");
        assert!(result.front_matter.is_none());
        assert!(result.warning.is_none());
        assert_eq!(result.body, "");
    }

    #[test]
    fn parse_unknown_fields_ignored() {
        let input = "---\ntitle: Note\ncustom_field: value\nanother: 42\n---\n\nBody";
        let result = parse(input);
        assert!(result.warning.is_none());
        let fm = result.front_matter.unwrap();
        assert_eq!(fm.title, Some("Note".to_string()));
        // Unknown fields should not cause an error
        assert_eq!(result.body, "Body");
    }

    #[test]
    fn serialize_round_trip() {
        let fm = NoteFrontMatter {
            title: Some("Round Trip".to_string()),
            tags: Some(vec!["test".to_string()]),
            pinned: Some(true),
            ..Default::default()
        };
        let body = "Some content here.";

        let serialized = serialize(&fm, body);
        let parsed = parse(&serialized);

        assert!(parsed.warning.is_none());
        let parsed_fm = parsed.front_matter.unwrap();
        assert_eq!(parsed_fm.title, fm.title);
        assert_eq!(parsed_fm.tags, fm.tags);
        assert_eq!(parsed_fm.pinned, fm.pinned);
        assert_eq!(parsed.body, body);
    }

    #[test]
    fn serialize_omits_none_fields() {
        let fm = NoteFrontMatter {
            title: Some("Minimal".to_string()),
            ..Default::default()
        };
        let output = serialize(&fm, "body");
        // Should contain title but not tags, pinned, etc.
        assert!(output.contains("title:"));
        assert!(!output.contains("tags:"));
        assert!(!output.contains("pinned:"));
        assert!(!output.contains("icon:"));
        assert!(!output.contains("color:"));
        assert!(!output.contains("created:"));
        assert!(!output.contains("updated:"));
    }
}
