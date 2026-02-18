//! Core configuration types: Secret, Config, AgentsConfig, ToolsConfig, GatewayConfig, TodoConfig.

use serde::{Deserialize, Serialize};
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

    /// Get the standardized todo store path (P0 fix for path inconsistency)
    pub fn todo_store_path(&self) -> PathBuf {
        data_dir().join("todos.jsonl")
    }

    /// Get the standardized embedding store path
    pub fn embedding_store_path(&self) -> PathBuf {
        data_dir().join("todos_embeddings.jsonl")
    }

    /// Get the standardized project store path
    pub fn project_store_path(&self) -> PathBuf {
        data_dir().join("projects.jsonl")
    }

    /// Get the standardized goal store path
    pub fn goal_store_path(&self) -> PathBuf {
        data_dir().join("goals.jsonl")
    }

    /// Get the standardized plan store path
    pub fn plan_store_path(&self) -> PathBuf {
        data_dir().join("data").join("plans.jsonl")
    }

    /// Get the learning outcomes JSONL store path.
    pub fn learning_outcomes_path(&self) -> PathBuf {
        data_dir().join("data").join("outcomes.jsonl")
    }

    /// Get the learning state JSON file path.
    pub fn learning_state_path(&self) -> PathBuf {
        data_dir().join("data").join("learning_state.json")
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
    /// Path to decision log file (default: ~/.klyntbot/decision_log.jsonl)
    #[serde(default)]
    pub log_path: Option<PathBuf>,
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        Self {
            threshold: default_confidence_threshold(),
            enabled: true,
            log_path: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // ========================================================================
    // CalendarConfig tests
    // ========================================================================

    #[test]
    fn test_calendar_config_default() {
        let config = CalendarConfig::default();
        assert!(config.providers.is_empty());
        assert!(!config.is_any_enabled());
        assert_eq!(config.conflict_resolution, "server_wins");
        assert!(config.bidirectional_sync); // Defaults to true
    }

    #[test]
    fn test_apple_calendar_config_default() {
        let apple = AppleCalendarConfig::default();
        assert!(!apple.enabled);
        assert!(apple.username.is_empty());
        assert!(apple.password.is_empty());
        assert_eq!(apple.caldav_url, "https://caldav.icloud.com");
        assert_eq!(apple.calendar_name, "Klyntbot Tasks");
        assert_eq!(apple.sync_interval_secs, 300);
        assert!(apple.auto_sync_due_dates);
    }

    #[test]
    fn test_google_calendar_config_default() {
        let google = GoogleCalendarConfig::default();
        assert!(!google.enabled);
        assert!(google.client_id.is_empty());
        assert_eq!(google.calendar_id, "primary");
    }

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
    fn test_calendar_config_serialization_new_format() {
        let config = CalendarConfig {
            providers: vec![CalendarProviderConfig::Apple(AppleCalendarConfig {
                enabled: true,
                username: "user@apple.com".to_string(),
                password: Secret::new("pass".to_string()),
                ..AppleCalendarConfig::default()
            })],
            conflict_resolution: "server_wins".to_string(),
            ..CalendarConfig::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"providers\""));
        assert!(json.contains("\"type\":\"apple\""));
        assert!(json.contains("\"user@apple.com\""));
    }

    #[test]
    fn test_calendar_config_roundtrip() {
        let original = CalendarConfig {
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

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CalendarConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(original.providers.len(), deserialized.providers.len());
        assert_eq!(
            original.conflict_resolution,
            deserialized.conflict_resolution
        );
    }

    #[test]
    fn test_secret_is_empty() {
        let empty_secret: Secret<String> = Secret::default();
        assert!(empty_secret.is_empty());

        let non_empty_secret = Secret::new("value".to_string());
        assert!(!non_empty_secret.is_empty());
    }

    // ========================================================================
    // Config integration tests
    // ========================================================================

    #[test]
    fn test_config_includes_calendar() {
        let config = Config::default();
        assert!(!config.calendar.is_any_enabled());
        assert!(config.calendar.providers.is_empty());
    }

    #[test]
    fn test_config_calendar_serialization() {
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
    }

    // ========================================================================
    // DailyPlanningConfig tests
    // ========================================================================

    #[test]
    fn test_daily_planning_config_defaults_to_true() {
        let config = DailyPlanningConfig::default();
        assert!(config.enabled);
    }

    #[test]
    fn test_daily_planning_config_serde_roundtrip() {
        // Test with enabled: false to verify it doesn't just rely on defaults
        let config = DailyPlanningConfig {
            enabled: false,
            planning_time: "09:30".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();

        // Verify camelCase serialization
        assert!(json.contains("\"enabled\""));
        assert!(json.contains("\"planningTime\""));
        assert!(json.contains("\"09:30\""));

        // Verify roundtrip
        let loaded: DailyPlanningConfig = serde_json::from_str(&json).unwrap();
        assert!(!loaded.enabled);
        assert_eq!(loaded.planning_time, "09:30");
    }

    #[test]
    fn test_daily_planning_config_default() {
        let config = DailyPlanningConfig::default();
        assert!(config.enabled);
        assert_eq!(config.planning_time, "08:00");
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
