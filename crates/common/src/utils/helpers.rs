//! Shared utility functions.

use chrono::{DateTime, Utc};

/// Format a timestamp in milliseconds to ISO 8601 string
pub fn format_timestamp_ms(ms: i64) -> String {
    let dt = DateTime::from_timestamp_millis(ms).unwrap_or_else(Utc::now);
    dt.to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp_ms() {
        let ms = 1704067200000; // 2024-01-01 00:00:00 UTC
        let formatted = format_timestamp_ms(ms);

        // Verify it's a valid RFC3339 timestamp
        assert!(formatted.contains("2024"));
        assert!(formatted.contains("T"));
        assert!(formatted.ends_with("Z") || formatted.contains("+"));
    }
}
