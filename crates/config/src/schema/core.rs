//! Core configuration types: Secret, Config, AgentsConfig, ToolsConfig, GatewayConfig, TodoConfig.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

/// Get the klyntbot data directory (~/.klyntbot), falling back to "./.klyntbot".
fn data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".klyntbot")
}

/// Expand a leading `~` in a path to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return home.join(path.trim_start_matches("~/"));
        }
    }
    PathBuf::from(path)
}

use super::channels::ChannelsConfig;
use super::providers::{ProviderManagerConfig, ProvidersConfig};

/// Wrapper that redacts sensitive values in Debug/Display output.
/// Use `.expose()` to access the inner value.
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &T {
        &self.0
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl Default for Secret<String> {
    fn default() -> Self {
        Self(String::new())
    }
}

impl Secret<String> {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Root configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct Config {
    #[serde(default)]
    pub agents: AgentsConfig,

    #[serde(default)]
    pub channels: ChannelsConfig,

    #[serde(default)]
    pub providers: ProvidersConfig,

    #[serde(default)]
    pub tools: ToolsConfig,

    #[serde(default)]
    pub gateway: GatewayConfig,

    #[serde(default)]
    pub todo: TodoConfig,

    #[serde(default)]
    pub confidence: ConfidenceConfig,

    #[serde(default)]
    pub calendar: CalendarConfig,

    #[serde(default)]
    pub project: ProjectConfig,

    #[serde(default)]
    pub conversation: ConversationConfig,

    #[serde(default)]
    pub learning: LearningConfig,

    #[serde(default)]
    pub finance: FinanceConfig,

    /// Provider manager routing (primary/fallback/classifier)
    #[serde(default)]
    pub provider_manager: ProviderManagerConfig,

    #[serde(default = "default_timezone")]
    pub timezone: String,

    /// PostgreSQL connection URL (optional — DB features disabled when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,
}

impl Config {
    /// Get the workspace path (expanded)
    pub fn workspace_path(&self) -> PathBuf {
        expand_tilde(&self.agents.defaults.workspace)
    }

    /// Return all provider configs keyed by name (detection priority order).
    fn all_providers(&self) -> [(&str, &super::providers::ProviderConfig); 12] {
        [
            ("anthropic", &self.providers.anthropic),
            ("openai", &self.providers.openai),
            ("openrouter", &self.providers.openrouter),
            ("deepseek", &self.providers.deepseek),
            ("gemini", &self.providers.gemini),
            ("groq", &self.providers.groq),
            ("vllm", &self.providers.vllm),
            ("zhipu", &self.providers.zhipu),
            ("dashscope", &self.providers.dashscope),
            ("moonshot", &self.providers.moonshot),
            ("minimax", &self.providers.minimax),
            ("aihubmix", &self.providers.aihubmix),
        ]
    }

    /// Detect the active provider.
    ///
    /// Resolution: explicit `agents.defaults.provider` field first,
    /// then auto-detect from which API keys are configured.
    pub fn active_provider_name(&self) -> &str {
        // Check explicit provider field first
        if let Some(ref name) = self.agents.defaults.provider {
            if !name.is_empty() && self.is_provider_configured(name) {
                return name;
            }
        }

        // Fall back to auto-detection: first provider with a non-empty key
        for (name, pc) in &self.all_providers() {
            if !pc.api_key.is_empty() {
                return name;
            }
        }
        "none"
    }

    /// Check if a provider has an API key configured.
    pub fn is_provider_configured(&self, name: &str) -> bool {
        self.all_providers()
            .iter()
            .any(|(n, pc)| *n == name && !pc.api_key.is_empty())
    }

    /// Get the standardized goal store path.
    #[deprecated(
        note = "JSONL flat-file store superseded by PostgreSQL. Will be migrated in G-17."
    )]
    pub fn goal_store_path(&self) -> PathBuf {
        data_dir().join("goals.jsonl")
    }

    /// Get the standardized plan store path.
    #[deprecated(
        note = "JSONL flat-file store superseded by PostgreSQL. Will be migrated in G-17."
    )]
    pub fn plan_store_path(&self) -> PathBuf {
        data_dir().join("data").join("plans.jsonl")
    }

    /// Set the API key for a provider by name.
    pub fn set_provider_key(&mut self, provider_name: &str, key: String) {
        let secret = Secret::new(key);
        match provider_name {
            "anthropic" => self.providers.anthropic.api_key = secret,
            "openai" => self.providers.openai.api_key = secret,
            "openrouter" => self.providers.openrouter.api_key = secret,
            "deepseek" => self.providers.deepseek.api_key = secret,
            "gemini" => self.providers.gemini.api_key = secret,
            "groq" => self.providers.groq.api_key = secret,
            "vllm" => self.providers.vllm.api_key = secret,
            "zhipu" => self.providers.zhipu.api_key = secret,
            "dashscope" => self.providers.dashscope.api_key = secret,
            "moonshot" => self.providers.moonshot.api_key = secret,
            "minimax" => self.providers.minimax.api_key = secret,
            "aihubmix" => self.providers.aihubmix.api_key = secret,
            _ => {}
        }
    }
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct AgentsConfig {
    #[serde(default)]
    pub defaults: AgentDefaults,
}

/// Default agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefaults {
    #[serde(default = "default_workspace")]
    pub workspace: String,

    #[serde(default = "default_model")]
    pub model: String,

    /// Explicit active provider name (e.g., "anthropic", "deepseek").
    /// When set, takes priority over model-name auto-detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    #[serde(default = "default_temperature")]
    pub temperature: f32,

    #[serde(default = "default_max_iterations")]
    pub max_tool_iterations: u32,
}

impl Default for AgentDefaults {
    fn default() -> Self {
        Self {
            workspace: default_workspace(),
            model: default_model(),
            provider: None,
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            max_tool_iterations: default_max_iterations(),
        }
    }
}

fn default_workspace() -> String {
    "~/.klyntbot/workspace".to_string()
}

fn default_model() -> String {
    "anthropic/claude-opus-4-5".to_string()
}

fn default_max_tokens() -> u32 {
    8192
}

fn default_temperature() -> f32 {
    0.7
}

fn default_max_iterations() -> u32 {
    20
}

/// Gateway/HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConfig {
    #[serde(default = "default_gateway_host")]
    pub host: String,

    #[serde(default = "default_gateway_port")]
    pub port: u16,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: default_gateway_host(),
            port: default_gateway_port(),
        }
    }
}

fn default_gateway_host() -> String {
    "0.0.0.0".to_string()
}

fn default_gateway_port() -> u16 {
    18790
}

/// Tools configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct ToolsConfig {
    #[serde(default)]
    pub web: WebToolsConfig,

    #[serde(default)]
    pub exec: ExecToolConfig,

    #[serde(default)]
    pub restrict_to_workspace: bool,

    /// Optional per-channel permission levels for tool access control.
    /// Keys are channel names (e.g., "telegram", "discord", "cli").
    /// Values are permission levels: "readOnly", "standard", "elevated", "admin".
    /// When absent, all tools are allowed on all channels.
    #[serde(default)]
    pub permissions: Option<PermissionsConfig>,
}

/// Permission configuration for tool access control.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsConfig {
    /// Default permission level for channels not explicitly listed.
    #[serde(default = "default_permission_level")]
    pub default_level: String,

    /// Per-channel permission level overrides.
    #[serde(default)]
    pub channels: HashMap<String, String>,
}

fn default_permission_level() -> String {
    "standard".to_string()
}

/// Web tools configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebToolsConfig {
    #[serde(default)]
    pub brave_api_key: Secret<String>,

    #[serde(default = "default_web_max_results")]
    pub max_results: u8,
}

impl Default for WebToolsConfig {
    fn default() -> Self {
        Self {
            brave_api_key: Secret::default(),
            max_results: default_web_max_results(),
        }
    }
}

/// Exec tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecToolConfig {
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    #[serde(default)]
    pub allowed_commands: Vec<String>,
}

impl Default for ExecToolConfig {
    fn default() -> Self {
        Self {
            timeout: default_timeout(),
            allowed_commands: Vec::new(),
        }
    }
}

fn default_timeout() -> u64 {
    60
}

fn default_web_max_results() -> u8 {
    5
}

/// Task creation mode — controls whether the agent asks for details before creating tasks
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreationMode {
    /// Ask the user for details via ask_user before creating (default)
    #[default]
    #[serde(rename = "ask-first")]
    AskFirst,
    /// Auto-enrich from conversation context, present for confirmation
    #[serde(rename = "yolo")]
    Yolo,
    /// Interactive brainstorming, one question at a time
    #[serde(rename = "party")]
    Party,
}

fn deserialize_creation_mode<'de, D>(deserializer: D) -> std::result::Result<CreationMode, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "ask-first" => Ok(CreationMode::AskFirst),
        "yolo" => Ok(CreationMode::Yolo),
        "party" => Ok(CreationMode::Party),
        _ => Ok(CreationMode::AskFirst), // graceful fallback
    }
}

/// Todo system configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoConfig {
    #[serde(default)]
    pub notifications: TodoNotificationConfig,
    #[serde(default)]
    pub focus: TodoFocusConfig,
    #[serde(default)]
    pub enrichment: TodoEnrichmentConfig,
    #[serde(default)]
    pub search: TodoSearchConfig,
    #[serde(default)]
    pub daily_planning: DailyPlanningConfig,
    /// Task creation mode: ask-first (default), yolo, or party
    #[serde(default, deserialize_with = "deserialize_creation_mode")]
    pub creation_mode: CreationMode,
}

/// Smart enrichment configuration for auto-inferring task metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoEnrichmentConfig {
    /// Enable/disable automatic enrichment on task creation (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Confidence threshold for auto-applying suggestions without confirmation (default: 0.85)
    #[serde(default = "default_enrichment_confidence_threshold")]
    pub auto_apply_threshold: f64,
}

impl Default for TodoEnrichmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_apply_threshold: default_enrichment_confidence_threshold(),
        }
    }
}

fn default_enrichment_confidence_threshold() -> f64 {
    0.85
}

/// Confidence evaluation configuration (LLM-driven decision engine)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceConfig {
    /// Threshold below which ask_user is triggered (default: 0.7)
    #[serde(default = "default_confidence_threshold")]
    pub threshold: f32,
    /// Enable/disable confidence evaluation (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Per-tool confidence threshold overrides (tool_name → threshold).
    /// Tools not listed here fall back to the global `threshold`.
    #[serde(default)]
    pub tool_overrides: HashMap<String, f32>,
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        Self {
            threshold: default_confidence_threshold(),
            enabled: true,
            tool_overrides: HashMap::new(),
        }
    }
}

/// Todo notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoNotificationConfig {
    #[serde(default = "default_notification_targets")]
    pub targets: Vec<String>,
    #[serde(default = "default_true")]
    pub focus_reminders: bool,
    #[serde(default = "default_true")]
    pub daily_digest: bool,
    #[serde(default = "default_digest_time")]
    pub daily_digest_time: String,
}

impl Default for TodoNotificationConfig {
    fn default() -> Self {
        Self {
            targets: vec!["os_native".to_string()],
            focus_reminders: true,
            daily_digest: true,
            daily_digest_time: default_digest_time(),
        }
    }
}

/// Todo focus mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoFocusConfig {
    #[serde(default = "default_max_slots")]
    pub max_slots: usize,
    #[serde(default = "default_deadline_hours")]
    pub deadline_hours: u64,
}

impl Default for TodoFocusConfig {
    fn default() -> Self {
        Self {
            max_slots: default_max_slots(),
            deadline_hours: default_deadline_hours(),
        }
    }
}

fn default_notification_targets() -> Vec<String> {
    vec!["os_native".to_string()]
}

fn default_confidence_threshold() -> f32 {
    0.7
}

pub(crate) fn default_true() -> bool {
    true
}

fn default_digest_time() -> String {
    "09:00".to_string()
}

/// Learning system configuration (adaptive confidence thresholds).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningConfig {
    /// Enable/disable the learning system (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// How often the background analysis loop runs, in seconds (default: 3600 = 1 hour).
    #[serde(default = "default_learning_analysis_interval")]
    pub analysis_interval_secs: u64,
    /// Lower bound for adaptive threshold (default: 0.4).
    #[serde(default = "default_min_threshold")]
    pub min_threshold: f32,
    /// Upper bound for adaptive threshold (default: 0.9).
    #[serde(default = "default_max_threshold")]
    pub max_threshold: f32,
    /// Minimum outcomes required before threshold adaptation (default: 50).
    #[serde(default = "default_min_outcomes_for_adaptation")]
    pub min_outcomes_for_adaptation: usize,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            analysis_interval_secs: default_learning_analysis_interval(),
            min_threshold: default_min_threshold(),
            max_threshold: default_max_threshold(),
            min_outcomes_for_adaptation: default_min_outcomes_for_adaptation(),
        }
    }
}

fn default_learning_analysis_interval() -> u64 {
    3600
}

fn default_min_threshold() -> f32 {
    0.4
}

fn default_max_threshold() -> f32 {
    0.9
}

fn default_min_outcomes_for_adaptation() -> usize {
    50
}

fn default_max_slots() -> usize {
    3
}

fn default_deadline_hours() -> u64 {
    18
}

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
    "Klyntbot Tasks".to_string()
}

fn default_sync_interval_secs() -> u64 {
    300 // 5 minutes
}

fn default_conflict_resolution() -> String {
    "server_wins".to_string()
}

/// Auto-detect system timezone, fallback to UTC
fn default_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

/// Semantic search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoSearchConfig {
    /// Enable semantic search (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Cosine similarity threshold for semantic search results (0.0-1.0, default: 0.5)
    #[serde(default = "default_semantic_threshold")]
    pub semantic_threshold: f64,

    /// Embedding model name (default: "paraphrase-multilingual-MiniLM-L12-v2")
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,

    /// RRF k parameter for hybrid search (default: 60)
    #[serde(default = "default_rrf_k")]
    pub rrf_k: u32,
}

impl Default for TodoSearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            semantic_threshold: default_semantic_threshold(),
            embedding_model: default_embedding_model(),
            rrf_k: default_rrf_k(),
        }
    }
}

fn default_semantic_threshold() -> f64 {
    0.5
}

fn default_embedding_model() -> String {
    "paraphrase-multilingual-MiniLM-L12-v2".to_string()
}

fn default_rrf_k() -> u32 {
    60
}

fn default_planning_time() -> String {
    "08:00".to_string()
}

/// Daily planning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPlanningConfig {
    /// Enable/disable daily planning feature (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Time to trigger daily planning in HH:MM format (default: "08:00")
    #[serde(default = "default_planning_time")]
    pub planning_time: String,
}

impl Default for DailyPlanningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            planning_time: "08:00".to_string(),
        }
    }
}

/// Project management configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Conversation memory configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationConfig {
    #[serde(default)]
    pub embedding: ConversationEmbeddingConfig,
    #[serde(default)]
    pub search: ConversationSearchConfig,
}

/// Conversation embedding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationEmbeddingConfig {
    /// Enable automatic conversation embedding (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Channels to exclude from conversation embedding (default: [])
    #[serde(default)]
    pub exclude_channels: Vec<String>,
    /// Message roles to exclude from embedding (default: ["system", "tool"])
    #[serde(default = "default_exclude_roles")]
    pub exclude_roles: Vec<String>,
}

impl Default for ConversationEmbeddingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            exclude_channels: Vec::new(),
            exclude_roles: default_exclude_roles(),
        }
    }
}

/// Conversation search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSearchConfig {
    /// Enable conversation search (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Semantic similarity threshold for search results (0.0-1.0, default: 0.5)
    #[serde(default = "default_semantic_threshold")]
    pub semantic_threshold: f64,
    /// Maximum number of search results to return (default: 20)
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

impl Default for ConversationSearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            semantic_threshold: default_semantic_threshold(),
            max_results: default_max_results(),
        }
    }
}

fn default_exclude_roles() -> Vec<String> {
    vec!["system".to_string(), "tool".to_string()]
}

fn default_max_results() -> usize {
    20
}

// ============================================================================
// Finance configuration
// ============================================================================

/// Finance system configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceConfig {
    /// Enable/disable the finance system (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Default ISO 4217 currency code (default: "USD").
    #[serde(default = "default_finance_currency")]
    pub default_currency: String,
    /// Proactivity level: "full", "moderate", or "reactive" (default: "full").
    #[serde(default = "default_proactivity_level")]
    pub proactivity_level: String,
    #[serde(default)]
    pub inflation: FinanceInflationConfig,
    #[serde(default)]
    pub expected_returns: FinanceExpectedReturnsConfig,
    #[serde(default)]
    pub budgeting: FinanceBudgetingConfig,
    #[serde(default)]
    pub price_refresh: FinancePriceRefreshConfig,
    #[serde(default)]
    pub scheduling: FinanceSchedulingConfig,
    #[serde(default)]
    pub categories: FinanceCategoryConfig,
}

impl Default for FinanceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_currency: default_finance_currency(),
            proactivity_level: default_proactivity_level(),
            inflation: Default::default(),
            expected_returns: Default::default(),
            budgeting: Default::default(),
            price_refresh: Default::default(),
            scheduling: Default::default(),
            categories: Default::default(),
        }
    }
}

fn default_finance_currency() -> String {
    "USD".to_string()
}

fn default_proactivity_level() -> String {
    "full".to_string()
}

/// Inflation assumption configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceInflationConfig {
    /// Annual inflation rate as a percentage (default: 3.3).
    #[serde(default = "default_inflation_rate")]
    pub rate: f64,
    /// Source of inflation data: "manual" or "api" (default: "manual").
    #[serde(default = "default_inflation_source")]
    pub source: String,
}

impl Default for FinanceInflationConfig {
    fn default() -> Self {
        Self {
            rate: default_inflation_rate(),
            source: default_inflation_source(),
        }
    }
}

fn default_inflation_rate() -> f64 {
    3.3
}

fn default_inflation_source() -> String {
    "manual".to_string()
}

/// Expected annual return rates by asset class (as percentages).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceExpectedReturnsConfig {
    #[serde(default = "default_stock_return")]
    pub stocks: f64,
    #[serde(default = "default_crypto_return")]
    pub crypto: f64,
    #[serde(default = "default_real_estate_return")]
    pub real_estate: f64,
    #[serde(default = "default_bond_return")]
    pub bonds: f64,
}

impl Default for FinanceExpectedReturnsConfig {
    fn default() -> Self {
        Self {
            stocks: default_stock_return(),
            crypto: default_crypto_return(),
            real_estate: default_real_estate_return(),
            bonds: default_bond_return(),
        }
    }
}

fn default_stock_return() -> f64 {
    10.0
}

fn default_crypto_return() -> f64 {
    15.0
}

fn default_real_estate_return() -> f64 {
    8.0
}

fn default_bond_return() -> f64 {
    5.0
}

/// Budgeting method and alert configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceBudgetingConfig {
    /// Default budgeting method: "standard" or "six_jar" (default: "standard").
    #[serde(default = "default_budget_method")]
    pub default_method: String,
    /// Percentage threshold for budget alerts (0-100, default: 80).
    #[serde(default = "default_alert_threshold")]
    pub alert_threshold: u8,
    #[serde(default)]
    pub six_jar_ratios: SixJarRatios,
}

impl Default for FinanceBudgetingConfig {
    fn default() -> Self {
        Self {
            default_method: default_budget_method(),
            alert_threshold: default_alert_threshold(),
            six_jar_ratios: Default::default(),
        }
    }
}

fn default_budget_method() -> String {
    "standard".to_string()
}

fn default_alert_threshold() -> u8 {
    80
}

/// Six Jar allocation ratios (percentages, should sum to 100).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SixJarRatios {
    #[serde(default = "default_jar_essentials")]
    pub essentials: u8,
    #[serde(default = "default_jar_small")]
    pub savings: u8,
    #[serde(default = "default_jar_small")]
    pub investment: u8,
    #[serde(default = "default_jar_small")]
    pub education: u8,
    #[serde(default = "default_jar_small")]
    pub entertainment: u8,
    #[serde(default = "default_jar_charity")]
    pub charity: u8,
}

impl Default for SixJarRatios {
    fn default() -> Self {
        Self {
            essentials: 55,
            savings: 10,
            investment: 10,
            education: 10,
            entertainment: 10,
            charity: 5,
        }
    }
}

fn default_jar_essentials() -> u8 {
    55
}

fn default_jar_small() -> u8 {
    10
}

fn default_jar_charity() -> u8 {
    5
}

/// Price refresh / cache configuration for market data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancePriceRefreshConfig {
    /// Enable automatic price refresh (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// How often to refresh prices, in hours (default: 4).
    #[serde(default = "default_price_interval_hours")]
    pub interval_hours: u32,
    /// How long to cache prices, in minutes (default: 15).
    #[serde(default = "default_price_cache_ttl")]
    pub cache_ttl_minutes: u32,
}

impl Default for FinancePriceRefreshConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_hours: default_price_interval_hours(),
            cache_ttl_minutes: default_price_cache_ttl(),
        }
    }
}

fn default_price_interval_hours() -> u32 {
    4
}

fn default_price_cache_ttl() -> u32 {
    15
}

/// Finance review and reporting schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceSchedulingConfig {
    /// Time for daily financial review in HH:MM format (default: "21:00").
    #[serde(default = "default_daily_review_time")]
    pub daily_review_time: String,
    /// Day of week for weekly report (default: "monday").
    #[serde(default = "default_weekly_report_day")]
    pub weekly_report_day: String,
    /// Time for daily budget check in HH:MM format (default: "09:00").
    #[serde(default = "default_budget_check_time")]
    pub budget_check_time: String,
    /// IANA timezone override (falls back to system timezone when None).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

impl Default for FinanceSchedulingConfig {
    fn default() -> Self {
        Self {
            daily_review_time: default_daily_review_time(),
            weekly_report_day: default_weekly_report_day(),
            budget_check_time: default_budget_check_time(),
            timezone: None,
        }
    }
}

fn default_daily_review_time() -> String {
    "21:00".to_string()
}

fn default_weekly_report_day() -> String {
    "monday".to_string()
}

fn default_budget_check_time() -> String {
    "09:00".to_string()
}

/// Transaction auto-categorization configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceCategoryConfig {
    /// Enable automatic transaction categorization (default: true).
    #[serde(default = "default_true")]
    pub auto_categorize: bool,
    /// Minimum confidence to auto-apply a category (0.0-1.0, default: 0.8).
    #[serde(default = "default_finance_category_threshold")]
    pub confidence_threshold: f64,
}

impl Default for FinanceCategoryConfig {
    fn default() -> Self {
        Self {
            auto_categorize: true,
            confidence_threshold: default_finance_category_threshold(),
        }
    }
}

fn default_finance_category_threshold() -> f64 {
    0.8
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // ========================================================================
    // CalendarConfig tests
    // ========================================================================

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
    fn test_config_serde_roundtrips() {
        // CalendarConfig serialization
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

        // CalendarConfig roundtrip
        let deserialized: CalendarConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cal_config.providers.len(), deserialized.providers.len());
        assert_eq!(
            cal_config.conflict_resolution,
            deserialized.conflict_resolution
        );

        // Config-level calendar serialization
        let mut config = Config::default();
        config
            .calendar
            .providers
            .push(CalendarProviderConfig::Apple(AppleCalendarConfig {
                enabled: true,
                username: "test@example.com".to_string(),
                password: Secret::new("password123".to_string()),
                ..AppleCalendarConfig::default()
            }));
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("\"calendar\""));
        assert!(json.contains("\"providers\""));
        assert!(json.contains("\"test@example.com\""));

        // DailyPlanningConfig roundtrip
        let daily = DailyPlanningConfig {
            enabled: false,
            planning_time: "09:30".to_string(),
        };
        let json = serde_json::to_string(&daily).unwrap();
        assert!(json.contains("\"planningTime\""));
        assert!(json.contains("\"09:30\""));
        let loaded: DailyPlanningConfig = serde_json::from_str(&json).unwrap();
        assert!(!loaded.enabled);
        assert_eq!(loaded.planning_time, "09:30");
    }

    #[test]
    fn test_secret_is_empty() {
        let empty_secret: Secret<String> = Secret::default();
        assert!(empty_secret.is_empty());

        let non_empty_secret = Secret::new("value".to_string());
        assert!(!non_empty_secret.is_empty());
    }

    // ========================================================================
    // ConversationConfig tests
    // ========================================================================

    #[test]
    fn test_conversation_config_defaults() {
        let config = ConversationConfig::default();

        // Embedding defaults
        assert!(config.embedding.enabled);
        assert!(config.embedding.exclude_channels.is_empty());
        assert_eq!(config.embedding.exclude_roles, vec!["system", "tool"]);

        // Search defaults
        assert!(config.search.enabled);
        assert_eq!(config.search.semantic_threshold, 0.5);
        assert_eq!(config.search.max_results, 20);
    }

    #[test]
    fn test_conversation_config_deserialize() {
        let json = serde_json::json!({
            "embedding": {
                "enabled": false,
                "excludeChannels": ["whatsapp", "telegram"],
                "excludeRoles": ["system"]
            },
            "search": {
                "enabled": true,
                "semanticThreshold": 0.7,
                "maxResults": 50
            }
        });

        let config: ConversationConfig = serde_json::from_value(json).unwrap();

        // Verify embedding config
        assert!(!config.embedding.enabled);
        assert_eq!(config.embedding.exclude_channels.len(), 2);
        assert!(config
            .embedding
            .exclude_channels
            .contains(&"whatsapp".to_string()));
        assert!(config
            .embedding
            .exclude_channels
            .contains(&"telegram".to_string()));
        assert_eq!(config.embedding.exclude_roles, vec!["system"]);

        // Verify search config
        assert!(config.search.enabled);
        assert_eq!(config.search.semantic_threshold, 0.7);
        assert_eq!(config.search.max_results, 50);
    }

    #[test]
    fn test_exclude_channels_config() {
        // Test partial config (only excludeChannels specified)
        let json = serde_json::json!({
            "embedding": {
                "excludeChannels": ["discord", "slack"]
            }
        });

        let config: ConversationConfig = serde_json::from_value(json).unwrap();

        // Verify exclude_channels
        assert_eq!(config.embedding.exclude_channels.len(), 2);
        assert!(config
            .embedding
            .exclude_channels
            .contains(&"discord".to_string()));
        assert!(config
            .embedding
            .exclude_channels
            .contains(&"slack".to_string()));

        // Verify other fields use defaults
        assert!(config.embedding.enabled); // default: true
        assert_eq!(config.embedding.exclude_roles, vec!["system", "tool"]); // default
    }
}
