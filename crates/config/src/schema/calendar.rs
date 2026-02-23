//! Calendar sync configuration: CalendarConfig, provider configs (Apple, Google, Generic CalDAV).

use serde::{Deserialize, Serialize};

use super::core::{default_true, Secret};

/// Calendar sync configuration — supports multiple providers simultaneously.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarConfig {
    /// Calendar providers (Apple, Google, Generic CalDAV).
    #[serde(default)]
    pub providers: Vec<CalendarProviderConfig>,

    #[serde(default = "default_conflict_resolution")]
    pub conflict_resolution: String,

    /// Enable bidirectional sync reconciliation (default: true).
    /// When true, the reconciliation engine periodically checks calendar events
    /// and updates linked todos based on event status/completion.
    #[serde(default = "default_true")]
    pub bidirectional_sync: bool,

    // --- Legacy fields for backward compatibility during deserialization ---
    // These fields are populated by the custom deserializer when reading old-format configs.
    // They are NOT serialized. Code should use `providers` instead.
    #[serde(skip)]
    #[doc(hidden)]
    pub legacy_migrated: bool,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            conflict_resolution: default_conflict_resolution(),
            bidirectional_sync: true,
            legacy_migrated: false,
        }
    }
}

impl CalendarConfig {
    /// Check if any provider is enabled.
    pub fn is_any_enabled(&self) -> bool {
        self.providers.iter().any(|p| p.is_enabled())
    }

    /// Get all enabled providers.
    pub fn enabled_providers(&self) -> Vec<&CalendarProviderConfig> {
        self.providers.iter().filter(|p| p.is_enabled()).collect()
    }

    /// Find a provider by its ID (e.g., "apple", "google", "generic-nextcloud").
    pub fn find_provider(&self, provider_id: &str) -> Option<&CalendarProviderConfig> {
        self.providers
            .iter()
            .find(|p| p.provider_id() == provider_id)
    }

    /// Find a provider by its ID (mutable).
    pub fn find_provider_mut(&mut self, provider_id: &str) -> Option<&mut CalendarProviderConfig> {
        self.providers
            .iter_mut()
            .find(|p| p.provider_id() == provider_id)
    }

    /// Get the Apple provider config, if present.
    pub fn apple(&self) -> Option<&AppleCalendarConfig> {
        self.providers.iter().find_map(|p| match p {
            CalendarProviderConfig::Apple(c) => Some(c),
            _ => None,
        })
    }

    /// Get the Apple provider config mutably, if present.
    pub fn apple_mut(&mut self) -> Option<&mut AppleCalendarConfig> {
        self.providers.iter_mut().find_map(|p| match p {
            CalendarProviderConfig::Apple(c) => Some(c),
            _ => None,
        })
    }

    /// Get the Google provider config, if present.
    pub fn google(&self) -> Option<&GoogleCalendarConfig> {
        self.providers.iter().find_map(|p| match p {
            CalendarProviderConfig::Google(c) => Some(c),
            _ => None,
        })
    }

    /// Get the Google provider config mutably, if present.
    pub fn google_mut(&mut self) -> Option<&mut GoogleCalendarConfig> {
        self.providers.iter_mut().find_map(|p| match p {
            CalendarProviderConfig::Google(c) => Some(c),
            _ => None,
        })
    }

    /// Get or create the Apple provider config (mutable).
    pub fn ensure_apple_mut(&mut self) -> &mut AppleCalendarConfig {
        if !self
            .providers
            .iter()
            .any(|p| matches!(p, CalendarProviderConfig::Apple(_)))
        {
            self.providers
                .push(CalendarProviderConfig::Apple(AppleCalendarConfig::default()));
        }
        self.apple_mut().unwrap()
    }

    /// Get or create the Google provider config (mutable).
    pub fn ensure_google_mut(&mut self) -> &mut GoogleCalendarConfig {
        if !self
            .providers
            .iter()
            .any(|p| matches!(p, CalendarProviderConfig::Google(_)))
        {
            self.providers.push(CalendarProviderConfig::Google(
                GoogleCalendarConfig::default(),
            ));
        }
        self.google_mut().unwrap()
    }

    /// Get the minimum sync interval across all enabled providers.
    pub fn min_sync_interval_secs(&self) -> u64 {
        self.providers
            .iter()
            .filter(|p| p.is_enabled())
            .map(|p| p.sync_interval_secs())
            .min()
            .unwrap_or(300)
    }
}

/// Provider-specific configuration (tagged enum).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum CalendarProviderConfig {
    #[serde(rename = "apple")]
    Apple(AppleCalendarConfig),
    #[serde(rename = "google")]
    Google(GoogleCalendarConfig),
    #[serde(rename = "genericCaldav")]
    GenericCalDav(GenericCalDavConfig),
}

impl CalendarProviderConfig {
    /// Check if this provider is enabled.
    pub fn is_enabled(&self) -> bool {
        match self {
            Self::Apple(c) => c.enabled,
            Self::Google(c) => c.enabled,
            Self::GenericCalDav(c) => c.enabled,
        }
    }

    /// Get the unique provider ID.
    pub fn provider_id(&self) -> String {
        match self {
            Self::Apple(_) => "apple".to_string(),
            Self::Google(_) => "google".to_string(),
            Self::GenericCalDav(c) => {
                format!(
                    "generic-{}",
                    c.name
                        .to_lowercase()
                        .replace(|ch: char| !ch.is_alphanumeric(), "-")
                )
            }
        }
    }

    /// Check if auto-sync of due dates is enabled.
    pub fn auto_sync_due_dates(&self) -> bool {
        match self {
            Self::Apple(c) => c.auto_sync_due_dates,
            Self::Google(c) => c.auto_sync_due_dates,
            Self::GenericCalDav(c) => c.auto_sync_due_dates,
        }
    }

    /// Get the calendar name.
    pub fn calendar_name(&self) -> &str {
        match self {
            Self::Apple(c) => &c.calendar_name,
            Self::Google(c) => &c.calendar_name,
            Self::GenericCalDav(c) => &c.calendar_name,
        }
    }

    /// Get the human-readable display name for this provider type.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Apple(_) => "Apple Calendar",
            Self::Google(_) => "Google Calendar",
            Self::GenericCalDav(c) => &c.name,
        }
    }

    /// Get the sync interval in seconds.
    pub fn sync_interval_secs(&self) -> u64 {
        match self {
            Self::Apple(c) => c.sync_interval_secs,
            Self::Google(c) => c.sync_interval_secs,
            Self::GenericCalDav(c) => c.sync_interval_secs,
        }
    }

    /// Set enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        match self {
            Self::Apple(c) => c.enabled = enabled,
            Self::Google(c) => c.enabled = enabled,
            Self::GenericCalDav(c) => c.enabled = enabled,
        }
    }
}

/// Apple Calendar (iCloud CalDAV) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleCalendarConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: Secret<String>,
    #[serde(default = "default_caldav_url")]
    pub caldav_url: String,
    #[serde(default = "default_calendar_name")]
    pub calendar_name: String,
    #[serde(default = "default_sync_interval_secs")]
    pub sync_interval_secs: u64,
    #[serde(default = "default_true")]
    pub auto_sync_due_dates: bool,
}

impl Default for AppleCalendarConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            username: String::new(),
            password: Secret::default(),
            caldav_url: default_caldav_url(),
            calendar_name: default_calendar_name(),
            sync_interval_secs: default_sync_interval_secs(),
            auto_sync_due_dates: true,
        }
    }
}

/// Google Calendar (OAuth2 CalDAV) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleCalendarConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Secret<String>,
    #[serde(default)]
    pub access_token: Secret<String>,
    #[serde(default)]
    pub refresh_token: Secret<String>,
    #[serde(default = "default_google_calendar_id")]
    pub calendar_id: String,
    #[serde(default = "default_calendar_name")]
    pub calendar_name: String,
    #[serde(default = "default_sync_interval_secs")]
    pub sync_interval_secs: u64,
    #[serde(default = "default_true")]
    pub auto_sync_due_dates: bool,
}

impl Default for GoogleCalendarConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            client_id: String::new(),
            client_secret: Secret::default(),
            access_token: Secret::default(),
            refresh_token: Secret::default(),
            calendar_id: default_google_calendar_id(),
            calendar_name: default_calendar_name(),
            sync_interval_secs: default_sync_interval_secs(),
            auto_sync_due_dates: true,
        }
    }
}

fn default_google_calendar_id() -> String {
    "primary".to_string()
}

/// Generic CalDAV provider configuration (Nextcloud, Fastmail, Zoho, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericCalDavConfig {
    #[serde(default)]
    pub enabled: bool,
    /// User-chosen label (e.g., "Nextcloud", "Fastmail").
    pub name: String,
    pub caldav_url: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: Secret<String>,
    #[serde(default = "default_calendar_name")]
    pub calendar_name: String,
    #[serde(default = "default_sync_interval_secs")]
    pub sync_interval_secs: u64,
    #[serde(default = "default_true")]
    pub auto_sync_due_dates: bool,
}

fn default_caldav_url() -> String {
    "https://caldav.icloud.com".to_string()
}

fn default_calendar_name() -> String {
    "Personal".to_string()
}

fn default_sync_interval_secs() -> u64 {
    300 // 5 minutes
}

fn default_conflict_resolution() -> String {
    "server_wins".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calendar_config_secret_redaction() {
        let apple = AppleCalendarConfig {
            enabled: true,
            username: "user@example.com".to_string(),
            password: Secret::new("secret123".to_string()),
            ..AppleCalendarConfig::default()
        };

        let config = CalendarConfig {
            providers: vec![CalendarProviderConfig::Apple(apple)],
            ..CalendarConfig::default()
        };

        let debug_str = format!("{:?}", config);
        assert!(!debug_str.contains("secret123"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn test_calendar_provider_config_helpers() {
        let apple = CalendarProviderConfig::Apple(AppleCalendarConfig {
            enabled: true,
            calendar_name: "My Calendar".to_string(),
            auto_sync_due_dates: false,
            ..AppleCalendarConfig::default()
        });

        assert!(apple.is_enabled());
        assert_eq!(apple.provider_id(), "apple");
        assert!(!apple.auto_sync_due_dates());
        assert_eq!(apple.calendar_name(), "My Calendar");
        assert_eq!(apple.display_name(), "Apple Calendar");

        let google = CalendarProviderConfig::Google(GoogleCalendarConfig {
            enabled: false,
            ..GoogleCalendarConfig::default()
        });

        assert!(!google.is_enabled());
        assert_eq!(google.provider_id(), "google");
        assert_eq!(google.display_name(), "Google Calendar");
    }

    #[test]
    fn test_calendar_config_multi_provider() {
        let config = CalendarConfig {
            providers: vec![
                CalendarProviderConfig::Apple(AppleCalendarConfig {
                    enabled: true,
                    ..AppleCalendarConfig::default()
                }),
                CalendarProviderConfig::Google(GoogleCalendarConfig {
                    enabled: false,
                    ..GoogleCalendarConfig::default()
                }),
            ],
            ..CalendarConfig::default()
        };

        assert!(config.is_any_enabled());
        assert_eq!(config.enabled_providers().len(), 1);
        assert!(config.apple().is_some());
        assert!(config.google().is_some());
    }

    #[test]
    fn test_calendar_config_serde_roundtrip() {
        let cal_config = CalendarConfig {
            providers: vec![
                CalendarProviderConfig::Apple(AppleCalendarConfig {
                    enabled: true,
                    username: "user@apple.com".to_string(),
                    password: Secret::new("pass".to_string()),
                    ..AppleCalendarConfig::default()
                }),
                CalendarProviderConfig::Google(GoogleCalendarConfig {
                    enabled: true,
                    client_id: "id123".to_string(),
                    client_secret: Secret::new("secret".to_string()),
                    access_token: Secret::new("tok".to_string()),
                    refresh_token: Secret::new("ref".to_string()),
                    ..GoogleCalendarConfig::default()
                }),
            ],
            conflict_resolution: "server_wins".to_string(),
            ..CalendarConfig::default()
        };

        let json = serde_json::to_string(&cal_config).unwrap();
        assert!(json.contains("\"providers\""));
        assert!(json.contains("\"type\":\"apple\""));
        assert!(json.contains("\"user@apple.com\""));

        let deserialized: CalendarConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cal_config.providers.len(), deserialized.providers.len());
        assert_eq!(
            cal_config.conflict_resolution,
            deserialized.conflict_resolution
        );
    }
}
