/// Extract the first sentence or up to `max_chars` characters,
/// preferring to cut at the last complete sentence boundary within the limit.
///
/// Strategy:
/// 1. If the text fits within `max_chars`, return it as-is.
/// 2. Look for sentence-ending punctuation (`. `, `! `, `? `, or newline) within the limit.
///    Prefer the *last* such boundary to capture as much content as possible.
/// 3. If no sentence boundary is found, fall back to the last word boundary (space).
/// 4. If no word boundary either, hard-cut at `max_chars`.
pub fn first_snippet(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= max_chars {
        return trimmed.to_string();
    }

    // Find a UTF-8 char boundary at-or-below max_chars. Slicing `&trimmed[..max_chars]`
    // panics if max_chars lands inside a multi-byte char (e.g. an em-dash `—` is 3
    // bytes — LoCoMo's punctuation hits this constantly). Walk back to the previous
    // char boundary and slice there.
    let mut cut = max_chars.min(trimmed.len());
    while cut > 0 && !trimmed.is_char_boundary(cut) {
        cut -= 1;
    }
    let window = &trimmed[..cut];

    // Look for sentence-ending punctuation followed by a space or end-of-window,
    // to avoid cutting mid-abbreviation (e.g., "Dr.Smith").
    let sentence_end = window
        .rmatch_indices(['.', '!', '?'])
        .find(|&(pos, _)| {
            // Accept if it's the last char in window, or followed by whitespace
            pos + 1 >= window.len()
                || window
                    .as_bytes()
                    .get(pos + 1)
                    .is_some_and(|b| b.is_ascii_whitespace())
        })
        .map(|(pos, _)| pos + 1); // include the punctuation

    // Also check for newline boundaries
    let newline_end = window.rfind('\n');

    // Pick the best sentence boundary (whichever is later/longer)
    let best_sentence = match (sentence_end, newline_end) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };

    // Require a minimum cut position to avoid degenerate snippets (e.g., "I.")
    let min_cut = max_chars / 4;
    if let Some(cut) = best_sentence {
        if cut >= min_cut {
            return format!("{}…", trimmed[..cut].trim_end());
        }
    }

    // Fall back to word boundary
    if let Some(space_pos) = window.rfind(' ') {
        if space_pos >= min_cut {
            return format!("{}…", &trimmed[..space_pos]);
        }
    }

    // Hard cut
    format!("{}…", window)
}
