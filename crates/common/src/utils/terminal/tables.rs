//! Table rendering with Unicode-aware column alignment.

use super::colors::{
    color, colorize, display_width, pad_to_width, BoxChars, BOLD, RESET, SEPARATOR,
};

/// Renders a table with headers and rows
pub fn draw_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    if headers.is_empty() {
        return String::new();
    }

    let chars = BoxChars::get();

    // Calculate column widths using display width (Unicode-aware)
    let mut widths: Vec<usize> = headers.iter().map(|h| display_width(h)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(display_width(cell));
            }
        }
    }

    let mut result = String::new();

    // Top border
    result.push_str(color(SEPARATOR));
    result.push_str(chars.top_left);
    for (i, &width) in widths.iter().enumerate() {
        result.push_str(&chars.horizontal.repeat(width + 2));
        if i < widths.len() - 1 {
            result.push_str(chars.horizontal_down);
        }
    }
    result.push_str(chars.top_right);
    result.push_str(color(RESET));
    result.push('\n');

    // Header row
    result.push_str(&colorize(chars.vertical, SEPARATOR));
    for (i, header) in headers.iter().enumerate() {
        let colored = colorize(header, BOLD);
        result.push_str(&format!(
            " {} {}",
            pad_to_width(&colored, widths[i]),
            &colorize(chars.vertical, SEPARATOR),
        ));
    }
    result.push('\n');

    // Separator after header
    result.push_str(color(SEPARATOR));
    result.push_str(chars.vertical_right);
    for (i, &width) in widths.iter().enumerate() {
        result.push_str(&chars.horizontal.repeat(width + 2));
        if i < widths.len() - 1 {
            result.push('┼');
        }
    }
    result.push('┤');
    result.push_str(color(RESET));
    result.push('\n');

    // Data rows
    for row in rows {
        result.push_str(&colorize(chars.vertical, SEPARATOR));
        for (i, cell) in row.iter().enumerate() {
            let width = widths.get(i).copied().unwrap_or(0);
            result.push_str(&format!(
                " {} {}",
                pad_to_width(cell, width),
                &colorize(chars.vertical, SEPARATOR),
            ));
        }
        result.push('\n');
    }

    // Bottom border
    result.push_str(color(SEPARATOR));
    result.push_str(chars.bottom_left);
    for (i, &width) in widths.iter().enumerate() {
        result.push_str(&chars.horizontal.repeat(width + 2));
        if i < widths.len() - 1 {
            result.push_str(chars.horizontal_up);
        }
    }
    result.push_str(chars.bottom_right);
    result.push_str(color(RESET));

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_table_rendering() {
        env::set_var("NO_COLOR", "1");
        let headers = vec!["Name", "Age", "City"];
        let rows = vec![
            vec!["Alice".to_string(), "30".to_string(), "NYC".to_string()],
            vec!["Bob".to_string(), "25".to_string(), "LA".to_string()],
        ];
        let output = draw_table(&headers, &rows);
        assert!(output.contains("Name"));
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_table_empty_headers() {
        let output = draw_table(&[], &[]);
        assert_eq!(output, "");
    }

    #[test]
    fn test_table_column_width_calculation() {
        env::set_var("NO_COLOR", "1");
        let headers = vec!["Short", "Very Long Header"];
        let rows = vec![
            vec!["A".to_string(), "B".to_string()],
            vec!["C".to_string(), "D".to_string()],
        ];
        let output = draw_table(&headers, &rows);
        // Headers should determine column width
        assert!(output.contains("Very Long Header"));
        env::remove_var("NO_COLOR");
    }
}
