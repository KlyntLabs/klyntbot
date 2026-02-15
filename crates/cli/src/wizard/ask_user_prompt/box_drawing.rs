//! Rounded-corner box drawing primitives for tabbed form UI.

use std::io::Write;

use anyhow::Result;
use common::utils::terminal::*;

// ============================================================================
// Visible Length (ANSI-aware)
// ============================================================================

/// Count visible characters in a string, stripping ANSI escape sequences.
pub(super) fn visible_len(s: &str) -> usize {
    let mut count = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            count += 1;
        }
    }
    count
}

// ============================================================================
// Rounded Box Drawing
// ============================================================================

/// Characters for rounded-corner box drawing.
pub(super) struct RoundedChars {
    pub top_left: &'static str,
    pub top_right: &'static str,
    pub bottom_left: &'static str,
    pub bottom_right: &'static str,
    pub horizontal: &'static str,
    pub vertical: &'static str,
    pub sep_left: &'static str,
    pub sep_right: &'static str,
}

const ROUNDED_UNICODE: RoundedChars = RoundedChars {
    top_left: "╭",
    top_right: "╮",
    bottom_left: "╰",
    bottom_right: "╯",
    horizontal: "─",
    vertical: "│",
    sep_left: "├",
    sep_right: "┤",
};

const ROUNDED_ASCII: RoundedChars = RoundedChars {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    horizontal: "-",
    vertical: "|",
    sep_left: "+",
    sep_right: "+",
};

pub(super) fn rounded_chars() -> &'static RoundedChars {
    if colors_enabled() {
        &ROUNDED_UNICODE
    } else {
        &ROUNDED_ASCII
    }
}

/// Write the top border: `  ╭─ Title ───...───╮`
pub(super) fn write_box_top(out: &mut impl Write, title: &str, inner_w: usize) -> Result<usize> {
    let rc = rounded_chars();
    let title_vis = visible_len(title);
    let fill = inner_w.saturating_sub(3 + title_vis);
    write!(
        out,
        "\r  {}{} {} {}{}\r\n",
        colorize(rc.top_left, BRAND),
        colorize(rc.horizontal, BRAND),
        title,
        colorize(&rc.horizontal.repeat(fill), BRAND),
        colorize(rc.top_right, BRAND)
    )?;
    Ok(1)
}

/// Write a content line: `  │  content  pad  │`
pub(super) fn write_box_line(out: &mut impl Write, content: &str, inner_w: usize) -> Result<usize> {
    let rc = rounded_chars();
    let content_area = inner_w.saturating_sub(4);
    let content_vis = visible_len(content);
    let pad = content_area.saturating_sub(content_vis);
    write!(
        out,
        "\r  {}  {}{}  {}\r\n",
        colorize(rc.vertical, BRAND),
        content,
        " ".repeat(pad),
        colorize(rc.vertical, BRAND)
    )?;
    Ok(1)
}

/// Write an empty line: `  │                │`
pub(super) fn write_box_empty(out: &mut impl Write, inner_w: usize) -> Result<usize> {
    let rc = rounded_chars();
    write!(
        out,
        "\r  {}{}{}\r\n",
        colorize(rc.vertical, BRAND),
        " ".repeat(inner_w),
        colorize(rc.vertical, BRAND)
    )?;
    Ok(1)
}

/// Write a separator: `  ├──────────────┤`
pub(super) fn write_box_sep(out: &mut impl Write, inner_w: usize) -> Result<usize> {
    let rc = rounded_chars();
    write!(
        out,
        "\r  {}{}{}\r\n",
        colorize(rc.sep_left, BRAND),
        colorize(&rc.horizontal.repeat(inner_w), BRAND),
        colorize(rc.sep_right, BRAND)
    )?;
    Ok(1)
}

/// Write the bottom border: `  ╰──────────────╯`
pub(super) fn write_box_bottom(out: &mut impl Write, inner_w: usize) -> Result<usize> {
    let rc = rounded_chars();
    write!(
        out,
        "\r  {}{}{}\r\n",
        colorize(rc.bottom_left, BRAND),
        colorize(&rc.horizontal.repeat(inner_w), BRAND),
        colorize(rc.bottom_right, BRAND)
    )?;
    Ok(1)
}
