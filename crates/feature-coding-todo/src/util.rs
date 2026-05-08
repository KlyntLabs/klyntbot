//! Slug + path helpers for plan mode.

/// Lowercase, replace non-alphanumeric runs with `-`, trim leading/trailing
/// dashes, cap at 60 chars. Used for plan filename slugs.
pub fn kebab(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = true; // skip leading dashes
    for ch in s.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > 60 {
        out.truncate(60);
        while out.ends_with('-') {
            out.pop();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::kebab;

    #[test]
    fn kebab_basic_words() {
        assert_eq!(kebab("Hello World"), "hello-world");
    }

    #[test]
    fn kebab_punctuation_collapsed() {
        assert_eq!(kebab("Add gRPC: transport!!!"), "add-grpc-transport");
    }

    #[test]
    fn kebab_trims_leading_trailing_dashes() {
        assert_eq!(kebab("---hi---"), "hi");
    }

    #[test]
    fn kebab_caps_at_60_chars() {
        let long = "a".repeat(80);
        let s = kebab(&long);
        assert!(s.len() <= 60, "got {} chars: {}", s.len(), s);
    }

    #[test]
    fn kebab_empty_input() {
        assert_eq!(kebab(""), "");
    }

    #[test]
    fn kebab_only_punctuation() {
        assert_eq!(kebab("!!!---???"), "");
    }

    #[test]
    fn kebab_unicode_letters_kept_then_lowered() {
        // ASCII alphanumeric only; non-ascii becomes dashes.
        assert_eq!(kebab("caf\u{00e9} au lait"), "caf-au-lait");
    }

    #[test]
    fn kebab_collapses_internal_runs() {
        assert_eq!(kebab("a   b___c---d"), "a-b-c-d");
    }
}
