//! Secret/password input with masking.

use std::io::{self, Write};

use anyhow::Result;
use common::utils::terminal::*;
use crossterm::event::KeyCode;

use super::{is_interactive, read_key, step_prefix, RawModeGuard};

/// Prompt for secret input (API keys, passwords).
///
/// In interactive mode, masks input with `●` characters and supports
/// backspace. Falls back to plain text input for non-TTY.
pub fn prompt_secret(label: &str, min_length: usize) -> Result<String> {
    let prefix = step_prefix();

    if is_interactive() {
        loop {
            let _guard = RawModeGuard::enable()?;
            let mut out = io::stdout();
            let mut secret = String::new();

            write!(out, "{}{}: ", prefix, label)?;
            out.flush()?;

            loop {
                let key = read_key()?;
                match key.code {
                    KeyCode::Enter => {
                        write!(out, "\r\n")?;
                        out.flush()?;
                        break;
                    }
                    KeyCode::Backspace => {
                        if !secret.is_empty() {
                            secret.pop();
                            // Move back, overwrite with space, move back again
                            write!(out, "\x08 \x08")?;
                            out.flush()?;
                        }
                    }
                    KeyCode::Char(c) => {
                        secret.push(c);
                        write!(out, "●")?;
                        out.flush()?;
                    }
                    _ => {}
                }
            }

            // Drop guard before validation print to restore normal mode
            drop(_guard);

            if secret.is_empty() {
                println!("{}{}", prefix, colorize("This field is required", ERROR));
                continue;
            }

            if secret.len() < min_length {
                println!(
                    "{}{}",
                    prefix,
                    colorize(
                        &format!(
                            "Value seems too short (expected at least {} chars)",
                            min_length
                        ),
                        WARNING
                    )
                );
                continue;
            }

            return Ok(secret);
        }
    } else {
        // Non-TTY fallback
        loop {
            print!("{}{}: ", prefix, label);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();

            if input.is_empty() {
                println!("{}{}", prefix, colorize("This field is required", ERROR));
                continue;
            }

            if input.len() < min_length {
                println!(
                    "{}{}",
                    prefix,
                    colorize(
                        &format!(
                            "Value seems too short (expected at least {} chars)",
                            min_length
                        ),
                        WARNING
                    )
                );
                continue;
            }

            return Ok(input);
        }
    }
}

/// Mask a secret for display, showing a prefix and last 4 chars.
///
/// Examples: `"sk-ant-api03-abc...xyz1234"` → `"sk-ant-****1234"`
///           `"short"` → `"●●●●●"`
pub fn mask_secret(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }

    let len = s.len();
    if len <= 8 {
        return "●".repeat(len);
    }

    // Find a recognizable prefix (e.g. "sk-ant-", "sk-or-", "gsk_", "sk-", "BSA-")
    let prefix_len = if s.starts_with("sk-ant-") {
        7
    } else if s.starts_with("sk-or-") {
        6
    } else if s.starts_with("gsk_") || s.starts_with("BSA-") {
        4
    } else if s.starts_with("sk-") {
        3
    } else {
        0
    };

    let suffix_len = 4;
    if prefix_len + suffix_len >= len {
        return "●".repeat(len);
    }

    let prefix = &s[..prefix_len];
    let suffix = &s[len - suffix_len..];
    format!("{}****{}", prefix, suffix)
}

/// Prompt for a secret with an existing value. Shows masked preview, Enter to keep.
///
/// Returns `None` if the user pressed Enter (keep existing), `Some(new)` if they
/// entered a new value.
pub fn prompt_secret_with_existing(
    label: &str,
    existing: &str,
    min_length: usize,
) -> Result<Option<String>> {
    let prefix = step_prefix();
    let masked = mask_secret(existing);

    if is_interactive() {
        loop {
            let _guard = RawModeGuard::enable()?;
            let mut out = io::stdout();
            let mut secret = String::new();

            write!(out, "{}{} [{}]: ", prefix, label, masked)?;
            out.flush()?;

            loop {
                let key = read_key()?;
                match key.code {
                    KeyCode::Enter => {
                        write!(out, "\r\n")?;
                        out.flush()?;
                        break;
                    }
                    KeyCode::Backspace => {
                        if !secret.is_empty() {
                            secret.pop();
                            write!(out, "\x08 \x08")?;
                            out.flush()?;
                        }
                    }
                    KeyCode::Char(c) => {
                        secret.push(c);
                        write!(out, "●")?;
                        out.flush()?;
                    }
                    _ => {}
                }
            }

            drop(_guard);

            if secret.is_empty() {
                // Keep existing
                return Ok(None);
            }

            if secret.len() < min_length {
                println!(
                    "{}{}",
                    prefix,
                    colorize(
                        &format!(
                            "Value seems too short (expected at least {} chars)",
                            min_length
                        ),
                        WARNING
                    )
                );
                continue;
            }

            return Ok(Some(secret));
        }
    } else {
        // Non-TTY fallback
        loop {
            print!("{}{} [{}]: ", prefix, label, masked);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();

            if input.is_empty() {
                return Ok(None);
            }

            if input.len() < min_length {
                println!(
                    "{}{}",
                    prefix,
                    colorize(
                        &format!(
                            "Value seems too short (expected at least {} chars)",
                            min_length
                        ),
                        WARNING
                    )
                );
                continue;
            }

            return Ok(Some(input));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_secret_empty() {
        assert_eq!(mask_secret(""), "");
    }

    #[test]
    fn test_mask_secret_short() {
        assert_eq!(mask_secret("abc"), "●●●");
        assert_eq!(mask_secret("12345678"), "●●●●●●●●");
    }

    #[test]
    fn test_mask_secret_anthropic_key() {
        assert_eq!(
            mask_secret("sk-ant-api03-abcdefghijklmnop"),
            "sk-ant-****mnop"
        );
    }

    #[test]
    fn test_mask_secret_openai_key() {
        assert_eq!(mask_secret("sk-proj-abcdefghij"), "sk-****ghij");
    }

    #[test]
    fn test_mask_secret_openrouter_key() {
        assert_eq!(mask_secret("sk-or-v1-abcdefghij"), "sk-or-****ghij");
    }

    #[test]
    fn test_mask_secret_groq_key() {
        assert_eq!(mask_secret("gsk_abcdefghijklm"), "gsk_****jklm");
    }

    #[test]
    fn test_mask_secret_brave_key() {
        assert_eq!(mask_secret("BSA-abcdefghijklm"), "BSA-****jklm");
    }

    #[test]
    fn test_mask_secret_no_prefix() {
        assert_eq!(mask_secret("abcdefghijklmnop"), "****mnop");
    }
}
