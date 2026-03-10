use serde::{Deserialize, Serialize};

/// Configuration for external capture sources (Phase 3).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureConfig {
    #[serde(default)]
    pub shell_hook: ShellHookConfig,

    #[serde(default)]
    pub file_watcher: FileWatcherConfig,

    #[serde(default)]
    pub ingestion_api: IngestionApiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellHookConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_shell_exclude_patterns")]
    pub exclude_patterns: Vec<String>,
}

impl Default for ShellHookConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            exclude_patterns: default_shell_exclude_patterns(),
        }
    }
}

fn default_shell_exclude_patterns() -> Vec<String> {
    vec![
        "export *=*".into(),
        "ssh-keygen*".into(),
        "gpg *".into(),
        "pass *".into(),
        "aws configure*".into(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWatcherConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub directories: Vec<String>,

    #[serde(default = "default_ignore_patterns")]
    pub ignore_patterns: Vec<String>,

    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

impl Default for FileWatcherConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            directories: vec![],
            ignore_patterns: default_ignore_patterns(),
            debounce_ms: default_debounce_ms(),
        }
    }
}

fn default_ignore_patterns() -> Vec<String> {
    vec![
        "node_modules".into(),
        ".git".into(),
        "target".into(),
        "build".into(),
        "dist".into(),
        "__pycache__".into(),
        ".next".into(),
        ".cache".into(),
        ".DS_Store".into(),
    ]
}

fn default_debounce_ms() -> u64 {
    500
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestionApiConfig {
    #[serde(default = "super::core::default_true")]
    pub enabled: bool,

    #[serde(default = "default_ingestion_port")]
    pub port: u16,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl Default for IngestionApiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: default_ingestion_port(),
            token: None,
        }
    }
}

fn default_ingestion_port() -> u16 {
    3456
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_config_default() {
        let config = CaptureConfig::default();
        assert!(!config.shell_hook.enabled);
        assert!(!config.file_watcher.enabled);
        assert!(config.ingestion_api.enabled);
        assert_eq!(config.ingestion_api.port, 3456);
        assert!(config.ingestion_api.token.is_none());
    }

    #[test]
    fn test_capture_config_serde_roundtrip() {
        let config = CaptureConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let loaded: CaptureConfig = serde_json::from_str(&json).unwrap();
        assert!(!loaded.shell_hook.enabled);
        assert_eq!(loaded.file_watcher.debounce_ms, 500);
    }

    #[test]
    fn test_capture_config_camel_case() {
        let config = CaptureConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("shellHook"));
        assert!(json.contains("fileWatcher"));
        assert!(json.contains("ingestionApi"));
        assert!(json.contains("debounceMs"));
        assert!(json.contains("excludePatterns"));
        assert!(json.contains("ignorePatterns"));
    }

    #[test]
    fn test_capture_config_from_empty_json() {
        let config: CaptureConfig = serde_json::from_str("{}").unwrap();
        assert!(!config.shell_hook.enabled);
        assert_eq!(config.file_watcher.ignore_patterns.len(), 9);
        assert_eq!(config.shell_hook.exclude_patterns.len(), 5);
    }
}
