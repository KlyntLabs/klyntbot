//! Static OAuth provider definitions.
//!
//! Each entry maps a provider ID to its OAuth endpoints, scopes, and
//! the environment variable name the MCP server subprocess expects.

/// Fixed localhost port for OAuth callbacks.
/// Must match the redirect URI registered with each OAuth provider.
pub const CALLBACK_PORT: u16 = 14321;

/// The redirect URI registered with all OAuth providers.
/// Keep in sync with `CALLBACK_PORT` — verified by the test below.
pub const REDIRECT_URI: &str = "http://localhost:14321/callback";

/// Definition for an OAuth 2.0 provider that can be used with MCP servers.
pub struct OAuthProviderDef {
    pub id: &'static str,
    pub authorize_url: &'static str,
    pub token_url: &'static str,
    pub scopes: &'static str,
    pub env_var: &'static str,
    pub supports_refresh: bool,
    /// Request `access_type=offline` + `prompt=consent` (Google providers).
    pub requires_offline: bool,
    /// Bundled client ID for instant one-click connect.
    pub default_client_id: &'static str,
    /// Bundled client secret (public clients or PKCE flows may leave this empty).
    pub default_client_secret: &'static str,
    /// Env var to override client ID at runtime (e.g. `KLYNT_LINEAR_CLIENT_ID`).
    pub client_id_env: &'static str,
    /// Env var to override client secret at runtime.
    pub client_secret_env: &'static str,
}

impl OAuthProviderDef {
    /// Resolve client ID: env var override → bundled default.
    pub fn client_id(&self) -> String {
        std::env::var(self.client_id_env).unwrap_or_else(|_| self.default_client_id.to_string())
    }

    /// Resolve client secret: env var override → bundled default.
    pub fn client_secret(&self) -> String {
        std::env::var(self.client_secret_env)
            .unwrap_or_else(|_| self.default_client_secret.to_string())
    }
}

/// Const-compatible `unwrap_or` for `option_env!()` results.
const fn env_or_empty(opt: Option<&'static str>) -> &'static str {
    match opt {
        Some(s) => s,
        None => "",
    }
}

/// All supported OAuth providers.
///
/// `default_client_id` / `default_client_secret` are injected at **compile time**
/// via `env_or_empty(option_env!("KLYNT_BUILD_<PROVIDER>_CLIENT_ID")`. Official release builds
/// set these as CI secrets; source code contains only empty fallbacks.
/// Users can always override at runtime via `KLYNT_<PROVIDER>_CLIENT_ID` env vars.
pub static PROVIDERS: &[OAuthProviderDef] = &[
    OAuthProviderDef {
        id: "linear",
        authorize_url: "https://linear.app/oauth/authorize",
        token_url: "https://api.linear.app/oauth/token",
        scopes: "read,write",
        env_var: "LINEAR_API_KEY",
        supports_refresh: false,
        requires_offline: false,
        default_client_id: env_or_empty(option_env!("KLYNT_BUILD_LINEAR_CLIENT_ID")),
        default_client_secret: env_or_empty(option_env!("KLYNT_BUILD_LINEAR_CLIENT_SECRET")),
        client_id_env: "KLYNT_LINEAR_CLIENT_ID",
        client_secret_env: "KLYNT_LINEAR_CLIENT_SECRET",
    },
    OAuthProviderDef {
        id: "github",
        authorize_url: "https://github.com/login/oauth/authorize",
        token_url: "https://github.com/login/oauth/access_token",
        scopes: "repo,read:org",
        env_var: "GITHUB_PERSONAL_ACCESS_TOKEN",
        supports_refresh: true,
        requires_offline: false,
        default_client_id: env_or_empty(option_env!("KLYNT_BUILD_GITHUB_CLIENT_ID")),
        default_client_secret: env_or_empty(option_env!("KLYNT_BUILD_GITHUB_CLIENT_SECRET")),
        client_id_env: "KLYNT_GITHUB_CLIENT_ID",
        client_secret_env: "KLYNT_GITHUB_CLIENT_SECRET",
    },
    OAuthProviderDef {
        id: "notion",
        authorize_url: "https://api.notion.com/v1/oauth/authorize",
        token_url: "https://api.notion.com/v1/oauth/token",
        scopes: "",
        env_var: "NOTION_API_KEY",
        supports_refresh: false,
        requires_offline: false,
        default_client_id: env_or_empty(option_env!("KLYNT_BUILD_NOTION_CLIENT_ID")),
        default_client_secret: env_or_empty(option_env!("KLYNT_BUILD_NOTION_CLIENT_SECRET")),
        client_id_env: "KLYNT_NOTION_CLIENT_ID",
        client_secret_env: "KLYNT_NOTION_CLIENT_SECRET",
    },
    OAuthProviderDef {
        id: "slack",
        authorize_url: "https://slack.com/oauth/v2/authorize",
        token_url: "https://slack.com/api/oauth.v2.access",
        scopes: "channels:history,channels:read,chat:write,users:read",
        env_var: "SLACK_BOT_TOKEN",
        supports_refresh: true,
        requires_offline: false,
        default_client_id: env_or_empty(option_env!("KLYNT_BUILD_SLACK_CLIENT_ID")),
        default_client_secret: env_or_empty(option_env!("KLYNT_BUILD_SLACK_CLIENT_SECRET")),
        client_id_env: "KLYNT_SLACK_CLIENT_ID",
        client_secret_env: "KLYNT_SLACK_CLIENT_SECRET",
    },
    OAuthProviderDef {
        id: "google-drive",
        authorize_url: "https://accounts.google.com/o/oauth2/v2/auth",
        token_url: "https://oauth2.googleapis.com/token",
        scopes: "https://www.googleapis.com/auth/drive.readonly",
        env_var: "GDRIVE_ACCESS_TOKEN",
        supports_refresh: true,
        requires_offline: true,
        default_client_id: env_or_empty(option_env!("KLYNT_BUILD_GOOGLE_DRIVE_CLIENT_ID")),
        default_client_secret: env_or_empty(option_env!("KLYNT_BUILD_GOOGLE_DRIVE_CLIENT_SECRET")),
        client_id_env: "KLYNT_GOOGLE_DRIVE_CLIENT_ID",
        client_secret_env: "KLYNT_GOOGLE_DRIVE_CLIENT_SECRET",
    },
    OAuthProviderDef {
        id: "google-calendar",
        authorize_url: "https://accounts.google.com/o/oauth2/v2/auth",
        token_url: "https://oauth2.googleapis.com/token",
        scopes: "https://www.googleapis.com/auth/calendar",
        env_var: "GOOGLE_CALENDAR_ACCESS_TOKEN",
        supports_refresh: true,
        requires_offline: true,
        default_client_id: env_or_empty(option_env!("KLYNT_BUILD_GOOGLE_CALENDAR_CLIENT_ID")),
        default_client_secret: env_or_empty(option_env!(
            "KLYNT_BUILD_GOOGLE_CALENDAR_CLIENT_SECRET"
        )),
        client_id_env: "KLYNT_GOOGLE_CALENDAR_CLIENT_ID",
        client_secret_env: "KLYNT_GOOGLE_CALENDAR_CLIENT_SECRET",
    },
];

/// Look up an OAuth provider by its ID.
pub fn find_provider(id: &str) -> Option<&'static OAuthProviderDef> {
    PROVIDERS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_uri_matches_port() {
        let expected = format!("http://localhost:{CALLBACK_PORT}/callback");
        assert_eq!(REDIRECT_URI, expected);
    }
}
