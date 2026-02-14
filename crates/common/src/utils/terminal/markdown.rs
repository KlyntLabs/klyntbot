//! Markdown to terminal renderer.

use super::boxes::draw_code_block;
use super::colors::{colorize, BOLD, DIM, ITALIC, SEPARATOR, STRIKETHROUGH, UNDERLINE};
use super::tables::draw_table;

/// Simple markdown to terminal converter
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    /// Renders markdown text to terminal-formatted output
    pub fn render(markdown: &str) -> String {
        let mut result = String::new();
        let mut in_code_block = false;
        let mut code_block_lang = String::new();
        let mut code_block_content = String::new();
        let mut in_table = false;
        let mut table_lines: Vec<String> = Vec::new();

        for line in markdown.lines() {
            // Handle code blocks
            if line.starts_with("```") {
                if in_code_block {
                    // End of code block
                    result.push_str(&draw_code_block(
                        code_block_content.trim_end(),
                        if code_block_lang.is_empty() {
                            None
                        } else {
                            Some(&code_block_lang)
                        },
                    ));
                    result.push('\n');
                    code_block_content.clear();
                    code_block_lang.clear();
                    in_code_block = false;
                } else {
                    // Start of code block
                    code_block_lang = line.trim_start_matches('`').trim().to_string();
                    in_code_block = true;
                }
                continue;
            }

            if in_code_block {
                code_block_content.push_str(line);
                code_block_content.push('\n');
                continue;
            }

            // Handle table lines
            if line.trim_start().starts_with('|') {
                in_table = true;
                table_lines.push(line.to_string());
                continue;
            }

            // Flush table if we were in one and hit a non-table line
            if in_table {
                result.push_str(&Self::flush_table(&table_lines));
                result.push('\n');
                table_lines.clear();
                in_table = false;
            }

            // Handle blockquotes
            if line.starts_with('>') {
                let quote_text = line.trim_start_matches('>').trim();
                result.push_str(&format!(
                    "{} {}\n",
                    colorize("│", DIM),
                    Self::render_inline(quote_text)
                ));
                continue;
            }

            // Handle unordered lists
            if line.trim_start().starts_with("- ") || line.trim_start().starts_with("* ") {
                let indent = line.len() - line.trim_start().len();
                let text = line
                    .trim_start()
                    .trim_start_matches("- ")
                    .trim_start_matches("* ");
                result.push_str(&format!(
                    "{}{} {}\n",
                    " ".repeat(indent),
                    colorize("•", DIM),
                    Self::render_inline(text)
                ));
                continue;
            }

            // Handle ordered lists
            if let Some(text) = Self::parse_ordered_list(line) {
                result.push_str(&format!("{}\n", Self::render_inline(text)));
                continue;
            }

            // Handle headers
            if line.starts_with('#') {
                let level = line.chars().take_while(|&c| c == '#').count();
                let text = line.trim_start_matches('#').trim();
                result.push_str(&format!("{}\n", colorize(text, BOLD)));
                if level <= 2 {
                    let separator_line = "─".repeat(text.len());
                    result.push_str(&format!("{}\n", colorize(&separator_line, SEPARATOR)));
                }
                continue;
            }

            // Regular text with inline formatting
            if !line.trim().is_empty() {
                result.push_str(&format!("{}\n", Self::render_inline(line)));
            } else {
                result.push('\n');
            }
        }

        // Flush any remaining table at the end
        if in_table {
            result.push_str(&Self::flush_table(&table_lines));
            result.push('\n');
        }

        result
    }

    /// Parses accumulated markdown table lines and renders them via `draw_table()`
    fn flush_table(table_lines: &[String]) -> String {
        if table_lines.is_empty() {
            return String::new();
        }

        // Parse cells from a table row: split by |, trim, drop empty leading/trailing
        let parse_row = |line: &str| -> Vec<String> {
            line.split('|')
                .map(|cell| cell.trim().to_string())
                .filter(|cell| !cell.is_empty())
                .collect()
        };

        // First row is headers
        let headers = parse_row(&table_lines[0]);
        if headers.is_empty() {
            return table_lines.join("\n");
        }

        // Collect data rows, skipping the separator line (contains only dashes/colons)
        let mut rows: Vec<Vec<String>> = Vec::new();
        for line in &table_lines[1..] {
            let cells = parse_row(line);
            // Skip separator lines like |---|---|
            let is_separator = cells
                .iter()
                .all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '));
            if is_separator {
                continue;
            }
            rows.push(cells);
        }

        let header_refs: Vec<&str> = headers.iter().map(|s| s.as_str()).collect();
        draw_table(&header_refs, &rows)
    }

    /// Renders inline markdown formatting (bold, italic, code, links)
    fn render_inline(text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                // Bold **text**
                '*' if chars.peek() == Some(&'*') => {
                    chars.next(); // consume second *
                    let mut bold_text = String::new();
                    let mut found_end = false;

                    while let Some(c) = chars.next() {
                        if c == '*' && chars.peek() == Some(&'*') {
                            chars.next(); // consume second *
                            found_end = true;
                            break;
                        }
                        bold_text.push(c);
                    }

                    if found_end {
                        result.push_str(&colorize(&bold_text, BOLD));
                    } else {
                        result.push_str("**");
                        result.push_str(&bold_text);
                    }
                }
                // Italic *text* or _text_
                '*' | '_' => {
                    let mut italic_text = String::new();
                    let mut found_end = false;

                    for c in chars.by_ref() {
                        if c == ch {
                            found_end = true;
                            break;
                        }
                        italic_text.push(c);
                    }

                    if found_end {
                        result.push_str(&colorize(&italic_text, ITALIC));
                    } else {
                        result.push(ch);
                        result.push_str(&italic_text);
                    }
                }
                // Inline code `text`
                '`' => {
                    let mut code_text = String::new();
                    let mut found_end = false;

                    for c in chars.by_ref() {
                        if c == '`' {
                            found_end = true;
                            break;
                        }
                        code_text.push(c);
                    }

                    if found_end {
                        result.push_str(&colorize(&code_text, DIM));
                    } else {
                        result.push('`');
                        result.push_str(&code_text);
                    }
                }
                // Strikethrough ~~text~~
                '~' if chars.peek() == Some(&'~') => {
                    chars.next(); // consume second ~
                    let mut strike_text = String::new();
                    let mut found_end = false;

                    while let Some(c) = chars.next() {
                        if c == '~' && chars.peek() == Some(&'~') {
                            chars.next(); // consume second ~
                            found_end = true;
                            break;
                        }
                        strike_text.push(c);
                    }

                    if found_end {
                        result.push_str(&colorize(&strike_text, STRIKETHROUGH));
                    } else {
                        result.push_str("~~");
                        result.push_str(&strike_text);
                    }
                }
                // Links [text](url)
                '[' => {
                    let mut link_text = String::new();
                    let mut found_bracket = false;

                    for c in chars.by_ref() {
                        if c == ']' {
                            found_bracket = true;
                            break;
                        }
                        link_text.push(c);
                    }

                    if found_bracket && chars.peek() == Some(&'(') {
                        chars.next(); // consume (
                        let mut url = String::new();

                        for c in chars.by_ref() {
                            if c == ')' {
                                break;
                            }
                            url.push(c);
                        }

                        result.push_str(&colorize(&format!("{} ({})", link_text, url), UNDERLINE));
                    } else {
                        result.push('[');
                        result.push_str(&link_text);
                        if found_bracket {
                            result.push(']');
                        }
                    }
                }
                _ => result.push(ch),
            }
        }

        result
    }

    /// Parses ordered list items (e.g., "1. text")
    fn parse_ordered_list(line: &str) -> Option<&str> {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        if let Some(dot_pos) = trimmed.find(". ") {
            let prefix = &trimmed[..dot_pos];
            if prefix.chars().all(|c| c.is_ascii_digit()) {
                let spaces = " ".repeat(indent);
                return Some(&line[spaces.len()..]);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_markdown_bold() {
        env::set_var("NO_COLOR", "1");
        let md = "This is **bold** text";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("bold"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_italic() {
        env::set_var("NO_COLOR", "1");
        let md = "This is *italic* text";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("italic"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_code() {
        env::set_var("NO_COLOR", "1");
        let md = "Use `code` here";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("code"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_code_block() {
        env::set_var("NO_COLOR", "1");
        let md = "```rust\nfn main() {}\n```";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("rust"));
        assert!(output.contains("fn main()"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_list() {
        env::set_var("NO_COLOR", "1");
        let md = "- Item 1\n- Item 2\n- Item 3";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("Item 1"));
        assert!(output.contains("Item 2"));
        assert!(output.contains("•"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_ordered_list() {
        env::set_var("NO_COLOR", "1");
        let md = "1. First\n2. Second\n3. Third";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("First"));
        assert!(output.contains("Second"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_blockquote() {
        env::set_var("NO_COLOR", "1");
        let md = "> This is a quote\n> Multiple lines";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("This is a quote"));
        assert!(output.contains("Multiple lines"));
        assert!(output.contains("│"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_link() {
        env::set_var("NO_COLOR", "1");
        let md = "Check [this link](https://example.com)";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("this link"));
        assert!(output.contains("https://example.com"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_header() {
        env::set_var("NO_COLOR", "1");
        let md = "# Header 1\n## Header 2";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("Header 1"));
        assert!(output.contains("Header 2"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_strikethrough() {
        env::set_var("NO_COLOR", "1");
        let md = "This is ~~strikethrough~~ text";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("strikethrough"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_nested_list() {
        env::set_var("NO_COLOR", "1");
        let md = "- Item 1\n  - Nested 1\n  - Nested 2\n- Item 2";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("Item 1"));
        assert!(output.contains("Nested 1"));
        assert!(output.contains("•"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_mixed_formatting() {
        env::set_var("NO_COLOR", "1");
        let md = "This has **bold** and *italic* and `code`";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("bold"));
        assert!(output.contains("italic"));
        assert!(output.contains("code"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_incomplete_formatting() {
        env::set_var("NO_COLOR", "1");
        // Test unclosed bold
        let md = "This is **incomplete";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("incomplete"));

        // Test unclosed code
        let md2 = "This is `incomplete";
        let output2 = MarkdownRenderer::render(md2);
        assert!(output2.contains("incomplete"));

        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_empty_code_block() {
        env::set_var("NO_COLOR", "1");
        let md = "```\n```";
        let output = MarkdownRenderer::render(md);
        // Should render empty box
        assert!(output.contains("+"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_table() {
        env::set_var("NO_COLOR", "1");
        let md = "| Name | Age |\n|------|-----|\n| Alice | 30 |\n| Bob | 25 |";
        let output = MarkdownRenderer::render(md);
        // Should contain data from draw_table, not raw pipes
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
        assert!(output.contains("Name"));
        // Should use box drawing chars (ASCII in NO_COLOR mode)
        assert!(output.contains("+"));
        assert!(!output.contains("|---"));
        env::remove_var("NO_COLOR");
    }
}
