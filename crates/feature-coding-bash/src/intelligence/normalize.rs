//! Command normalization for diff/recall lookup.
//!
//! `command_key` produces a stable 64-char sha256 hex over the command after
//! trimming, collapsing internal whitespace, and stripping leading `KEY=VAL`
//! environment-variable prefixes. Two commands that share intent but differ
//! in formatting or debug env vars hash to the same key.

use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};

static LEADING_ENV_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([A-Z_][A-Z0-9_]*=\S+\s+)+").unwrap());

pub fn command_key(raw: &str) -> String {
    let no_env = strip_leading_env_vars(raw);
    let trimmed = no_env.trim();
    let collapsed = collapse_whitespace(trimmed);
    let mut hasher = Sha256::new();
    hasher.update(collapsed.as_bytes());
    hex::encode(hasher.finalize())
}

fn strip_leading_env_vars(s: &str) -> &str {
    match LEADING_ENV_RE.find(s) {
        Some(m) => &s[m.end()..],
        None => s,
    }
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotent() {
        let k = command_key("cargo nextest run -p agent");
        assert_eq!(k, command_key("cargo nextest run -p agent"));
        assert_eq!(k.len(), 64);
    }

    #[test]
    fn whitespace_collapsing_matches() {
        assert_eq!(
            command_key("cargo nextest run -p agent"),
            command_key("  cargo nextest run  -p agent")
        );
    }

    #[test]
    fn leading_env_stripped() {
        assert_eq!(
            command_key("cargo nextest run -p agent"),
            command_key("RUST_LOG=debug cargo nextest run -p agent")
        );
        assert_eq!(
            command_key("cargo nextest run -p agent"),
            command_key("RUST_LOG=debug RUST_BACKTRACE=1 cargo nextest run -p agent")
        );
    }

    #[test]
    fn non_leading_env_preserved() {
        // FOO=bar after the command, not before — should remain in the key
        assert_ne!(
            command_key("cargo nextest run"),
            command_key("cargo nextest run FOO=bar")
        );
    }

    #[test]
    fn different_flags_different_keys() {
        assert_ne!(
            command_key("cargo nextest run -p agent"),
            command_key("cargo nextest run -p agent --nocapture")
        );
    }

    #[test]
    fn empty_and_only_env() {
        // Empty input
        assert_eq!(command_key(""), command_key(""));
        // All env, no command — strip leaves empty string
        assert_eq!(command_key(""), command_key("FOO=bar BAZ=qux "));
    }
}
