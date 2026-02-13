//! Shared utility functions.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

/// Format a timestamp in milliseconds to ISO 8601 string
pub fn format_timestamp_ms(ms: i64) -> String {
    let dt = DateTime::from_timestamp_millis(ms).unwrap_or_else(Utc::now);
    dt.to_rfc3339()
}

/// Format a timestamp in milliseconds to a human-readable relative time
pub fn format_relative_time(ms: i64) -> String {
    let now = Utc::now().timestamp_millis();
    let diff_ms = now - ms;

    if diff_ms < 0 {
        return "in the future".to_string();
    }

    let diff_s = diff_ms / 1000;

    if diff_s < 60 {
        return format!("{}s ago", diff_s);
    }

    let diff_m = diff_s / 60;
    if diff_m < 60 {
        return format!("{}m ago", diff_m);
    }

    let diff_h = diff_m / 60;
    if diff_h < 24 {
        return format!("{}h ago", diff_h);
    }

    let diff_d = diff_h / 24;
    format!("{}d ago", diff_d)
}

/// Expand tilde (~) in path to home directory
pub fn expand_tilde(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let path_str = path.to_string_lossy();

    if path_str.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            home.join(path_str.trim_start_matches("~/"))
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    }
}

/// Create a directory and all parent directories if they don't exist
pub fn ensure_dir(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    let path = path.as_ref();
    std::fs::create_dir_all(path)?;
    Ok(path.to_path_buf())
}

/// Create a safe filename from a string (replace invalid characters)
pub fn safe_filename(s: impl AsRef<str>) -> String {
    s.as_ref()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Get the klyntbot data directory (~/.klyntbot)
pub fn get_data_dir() -> std::io::Result<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join(".klyntbot"))
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Unable to determine home directory",
            )
        })
}

/// Get the workspace path from config or default
pub fn get_workspace_path() -> PathBuf {
    expand_tilde("~/.klyntbot/workspace")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_relative_time() {
        let now = Utc::now().timestamp_millis();

        assert_eq!(format_relative_time(now - 30_000), "30s ago");
        assert_eq!(format_relative_time(now - 120_000), "2m ago");
        assert_eq!(format_relative_time(now - 7_200_000), "2h ago");
        assert_eq!(format_relative_time(now - 86_400_000), "1d ago");
    }

    #[test]
    fn test_safe_filename() {
        assert_eq!(safe_filename("hello:world"), "hello_world");
        assert_eq!(safe_filename("file/path\\test"), "file_path_test");
        assert_eq!(safe_filename("normal.txt"), "normal.txt");
    }

    #[test]
    fn test_expand_tilde() {
        let home = dirs::home_dir().unwrap();
        let expanded = expand_tilde("~/test");
        assert_eq!(expanded, home.join("test"));

        let normal = expand_tilde("/absolute/path");
        assert_eq!(normal, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_format_timestamp_ms() {
        let ms = 1704067200000; // 2024-01-01 00:00:00 UTC
        let formatted = format_timestamp_ms(ms);

        // Verify it's a valid RFC3339 timestamp
        assert!(formatted.contains("2024"));
        assert!(formatted.contains("T"));
        assert!(formatted.ends_with("Z") || formatted.contains("+"));
    }

    #[test]
    fn test_format_relative_time_seconds() {
        let now = Utc::now().timestamp_millis();
        assert_eq!(format_relative_time(now - 10_000), "10s ago");
        assert_eq!(format_relative_time(now - 45_000), "45s ago");
        assert_eq!(format_relative_time(now - 59_000), "59s ago");
    }

    #[test]
    fn test_format_relative_time_minutes() {
        let now = Utc::now().timestamp_millis();
        assert_eq!(format_relative_time(now - 60_000), "1m ago");
        assert_eq!(format_relative_time(now - 300_000), "5m ago");
        assert_eq!(format_relative_time(now - 3_540_000), "59m ago");
    }

    #[test]
    fn test_format_relative_time_hours() {
        let now = Utc::now().timestamp_millis();
        assert_eq!(format_relative_time(now - 3_600_000), "1h ago");
        assert_eq!(format_relative_time(now - 10_800_000), "3h ago");
        assert_eq!(format_relative_time(now - 82_800_000), "23h ago");
    }

    #[test]
    fn test_format_relative_time_days() {
        let now = Utc::now().timestamp_millis();
        assert_eq!(format_relative_time(now - 86_400_000), "1d ago");
        assert_eq!(format_relative_time(now - 259_200_000), "3d ago");
        assert_eq!(format_relative_time(now - 2_592_000_000), "30d ago");
    }

    #[test]
    fn test_format_relative_time_future() {
        let now = Utc::now().timestamp_millis();
        assert_eq!(format_relative_time(now + 1000), "in the future");
        assert_eq!(format_relative_time(now + 86_400_000), "in the future");
    }

    #[test]
    fn test_safe_filename_all_invalid_chars() {
        assert_eq!(safe_filename("/:*?\"<>|"), "________");
    }

    #[test]
    fn test_safe_filename_mixed_content() {
        assert_eq!(
            safe_filename("file/name:with*invalid?chars"),
            "file_name_with_invalid_chars"
        );
        assert_eq!(safe_filename("document<2024>.pdf"), "document_2024_.pdf");
    }

    #[test]
    fn test_safe_filename_already_safe() {
        assert_eq!(
            safe_filename("safe_filename-123.txt"),
            "safe_filename-123.txt"
        );
        assert_eq!(safe_filename("README.md"), "README.md");
    }

    #[test]
    fn test_safe_filename_spaces_preserved() {
        assert_eq!(safe_filename("my document.pdf"), "my document.pdf");
        assert_eq!(
            safe_filename("file with spaces.txt"),
            "file with spaces.txt"
        );
    }

    #[test]
    fn test_expand_tilde_with_path() {
        let home = dirs::home_dir().unwrap();
        let expanded = expand_tilde("~/documents/file.txt");
        assert_eq!(expanded, home.join("documents/file.txt"));
    }

    #[test]
    fn test_expand_tilde_just_tilde() {
        let _home = dirs::home_dir().unwrap();
        let expanded = expand_tilde("~");
        // Just "~" becomes home directory with empty path joined
        // This is implementation-specific behavior
        assert!(expanded.is_absolute());
    }

    #[test]
    fn test_expand_tilde_relative_path() {
        let expanded = expand_tilde("relative/path");
        assert_eq!(expanded, PathBuf::from("relative/path"));
    }

    #[test]
    fn test_expand_tilde_current_dir() {
        let expanded = expand_tilde("./current/dir");
        assert_eq!(expanded, PathBuf::from("./current/dir"));
    }

    #[test]
    fn test_ensure_dir_creates_directory() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let test_dir = temp.path().join("test_dir");

        let result = ensure_dir(&test_dir);
        assert!(result.is_ok());
        assert!(test_dir.exists());
        assert!(test_dir.is_dir());
    }

    #[test]
    fn test_ensure_dir_creates_nested_directories() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let nested_dir = temp.path().join("a/b/c/d");

        let result = ensure_dir(&nested_dir);
        assert!(result.is_ok());
        assert!(nested_dir.exists());
        assert!(nested_dir.is_dir());
    }

    #[test]
    fn test_ensure_dir_existing_directory() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();

        // Directory already exists
        let result = ensure_dir(temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_data_dir() {
        let data_dir = get_data_dir().unwrap();
        assert!(data_dir.to_string_lossy().contains(".klyntbot"));

        let home = dirs::home_dir().unwrap();
        assert_eq!(data_dir, home.join(".klyntbot"));
    }

    #[test]
    fn test_get_workspace_path() {
        let workspace = get_workspace_path();
        assert!(workspace.is_absolute());
        assert!(workspace.to_string_lossy().contains(".klyntbot"));
        assert!(workspace.to_string_lossy().contains("workspace"));
    }

    #[test]
    fn test_safe_filename_unicode() {
        assert_eq!(safe_filename("文件名.txt"), "文件名.txt");
        assert_eq!(safe_filename("файл.doc"), "файл.doc");
        assert_eq!(safe_filename("ファイル.pdf"), "ファイル.pdf");
    }

    #[test]
    fn test_safe_filename_emoji() {
        assert_eq!(safe_filename("🚀 rocket.txt"), "🚀 rocket.txt");
        assert_eq!(safe_filename("test 👍.doc"), "test 👍.doc");
    }

    #[test]
    fn test_format_relative_time_boundary_cases() {
        let now = Utc::now().timestamp_millis();

        // Exactly 1 minute
        assert_eq!(format_relative_time(now - 60_000), "1m ago");

        // Exactly 1 hour
        assert_eq!(format_relative_time(now - 3_600_000), "1h ago");

        // Exactly 1 day
        assert_eq!(format_relative_time(now - 86_400_000), "1d ago");

        // Just under 1 minute
        assert_eq!(format_relative_time(now - 59_999), "59s ago");
    }

    #[test]
    fn test_expand_tilde_with_backslash() {
        let _home = dirs::home_dir().unwrap();
        let expanded = expand_tilde("~/documents\\file.txt");
        // On Unix, backslashes are literal
        assert!(expanded.to_string_lossy().contains("documents"));
    }

    #[test]
    fn test_safe_filename_empty_string() {
        assert_eq!(safe_filename(""), "");
    }

    #[test]
    fn test_safe_filename_only_invalid() {
        assert_eq!(safe_filename("///"), "___");
        assert_eq!(safe_filename("***"), "___");
    }
}
