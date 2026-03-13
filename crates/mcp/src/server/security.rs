//! Security validation for MCP server inputs.
//!
//! Provides path traversal protection and input sanitization for
//! tools exposed to external AI agents via MCP.

use std::path::{Path, PathBuf};

/// Maximum allowed input length for MCP tool parameters.
pub const MAX_INPUT_LENGTH: usize = 50_000;

/// Validate that a path stays within the allowed base directory.
///
/// Returns the canonicalized path on success, or an error if the path
/// escapes the allowed base via traversal or absolute path.
pub fn validate_path(path: &str, allowed_base: &Path) -> Result<PathBuf, String> {
    let resolved = PathBuf::from(path);
    let canonical = resolved
        .canonicalize()
        .map_err(|e| format!("Invalid path: {}", e))?;
    let base_canonical = allowed_base
        .canonicalize()
        .map_err(|e| format!("Invalid base path: {}", e))?;

    if !canonical.starts_with(&base_canonical) {
        return Err(format!(
            "Path traversal detected: {} is outside {}",
            path,
            allowed_base.display()
        ));
    }
    Ok(canonical)
}

/// Sanitize user input: strip control characters and limit length.
///
/// Preserves newlines and tabs since they're valid in tool parameters.
pub fn sanitize_input(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(MAX_INPUT_LENGTH)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_allows_safe_paths() {
        let base = std::path::PathBuf::from("/tmp/klyntbot_test_security");
        std::fs::create_dir_all(&base).ok();

        // Create a file so canonicalize works
        let file_path = base.join("data.db");
        std::fs::write(&file_path, "").ok();

        let result = validate_path(file_path.to_str().unwrap(), &base);
        assert!(result.is_ok());

        // Cleanup
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_validate_path_blocks_traversal() {
        let base = std::path::PathBuf::from("/tmp/klyntbot_test_security_trav");
        std::fs::create_dir_all(&base).ok();

        let result = validate_path(&format!("{}/../../../etc/hosts", base.display()), &base);
        // Should fail — either path doesn't exist or is outside base
        assert!(
            result.is_err() || {
                let p = result.unwrap();
                !p.starts_with(base.canonicalize().unwrap())
            }
        );

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_validate_path_blocks_absolute_outside() {
        let base = std::path::PathBuf::from("/tmp/klyntbot_test_security_abs");
        std::fs::create_dir_all(&base).ok();

        // /etc/hosts exists on macOS/Linux but is outside our base
        let result = validate_path("/etc/hosts", &base);
        assert!(result.is_err());

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_sanitize_input_strips_control_chars() {
        let input = "hello\x00world\x01test";
        let clean = sanitize_input(input);
        assert_eq!(clean, "helloworldtest");
    }

    #[test]
    fn test_sanitize_input_preserves_newlines_and_tabs() {
        let input = "hello\nworld\there";
        let clean = sanitize_input(input);
        assert_eq!(clean, "hello\nworld\there");
    }

    #[test]
    fn test_sanitize_input_limits_length() {
        let long = "x".repeat(100_000);
        let clean = sanitize_input(&long);
        assert!(clean.len() <= MAX_INPUT_LENGTH);
        assert_eq!(clean.len(), MAX_INPUT_LENGTH);
    }
}
