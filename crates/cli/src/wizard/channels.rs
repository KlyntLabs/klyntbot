//! Channel configuration wizard for interactive channel setup.
//!
//! Provides guided configuration for all supported chat channels:
//! Telegram, Discord, Slack, WhatsApp, Email, and QQ.

use std::io::{self, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::schema::{
    DiscordConfig, EmailConfig, QQConfig, Secret, SlackConfig, TelegramConfig, WhatsAppConfig,
};
use config::Config;

use super::oauth;

/// Channel metadata for the selection UI
struct ChannelInfo {
    name: &'static str,
    key: &'static str,
    description: &'static str,
    prerequisites: &'static str,
}

const CHANNELS: &[ChannelInfo] = &[
    ChannelInfo {
        name: "Telegram",
        key: "telegram",
        description: "Bot API with long polling",
        prerequisites: "Bot token from @BotFather",
    },
    ChannelInfo {
        name: "Discord",
        key: "discord",
        description: "Bot via WebSocket Gateway",
        prerequisites: "Bot token from Discord Developer Portal",
    },
    ChannelInfo {
        name: "Slack",
        key: "slack",
        description: "Socket Mode bot integration",
        prerequisites: "Bot Token (xoxb-) and App Token (xapp-)",
    },
    ChannelInfo {
        name: "WhatsApp",
        key: "whatsapp",
        description: "Via Baileys Node.js bridge",
        prerequisites: "Running WhatsApp bridge at ws://localhost:3001",
    },
    ChannelInfo {
        name: "Email",
        key: "email",
        description: "IMAP polling + SMTP replies",
        prerequisites: "IMAP/SMTP server credentials",
    },
    ChannelInfo {
        name: "QQ",
        key: "qq",
        description: "QQ Bot via official API",
        prerequisites: "App ID and Secret from QQ Bot Platform",
    },
];

// ============================================================================
// Public entry point
// ============================================================================

/// Run the channel configuration wizard step.
/// Returns the list of channel names that were successfully configured.
pub async fn configure_channels(config: &mut Config) -> Result<Vec<String>> {
    println!(
        "\n  {} Connect klyntbot to your chat platforms.",
        colorize("Channels", BOLD)
    );
    println!(
        "  {}",
        colorize(
            "Each channel can be tested after configuration.",
            DIM
        )
    );
    println!();

    // Channel selection (multi-select)
    let selected = select_channels()?;

    if selected.is_empty() {
        println!(
            "\n  {} No channels selected. You can set them up later with:",
            colorize("Skipped.", DIM)
        );
        println!(
            "  {}",
            colorize("  klyntbot channels login <channel>", DIM)
        );
        return Ok(vec![]);
    }

    let mut configured = Vec::new();

    for &idx in &selected {
        let channel = &CHANNELS[idx];
        println!("\n{}", draw_separator());
        println!(
            "\n  {} {}",
            colorize("▸", SUCCESS),
            colorize(&format!("Configure {}", channel.name), BOLD)
        );
        println!(
            "  {}",
            colorize(&format!("Prerequisite: {}", channel.prerequisites), DIM)
        );
        println!();

        let result = match channel.key {
            "telegram" => configure_telegram(config).await,
            "discord" => configure_discord(config).await,
            "slack" => configure_slack(config).await,
            "whatsapp" => configure_whatsapp(config).await,
            "email" => configure_email(config).await,
            "qq" => configure_qq(config).await,
            _ => Ok(false),
        };

        match result {
            Ok(true) => {
                configured.push(channel.key.to_string());
                println!(
                    "\n  {} {} configured successfully",
                    status_success(),
                    channel.name
                );
            }
            Ok(false) => {
                println!(
                    "\n  {} {} configuration skipped",
                    colorize("○", DIM),
                    channel.name
                );
            }
            Err(e) => {
                println!(
                    "\n  {} {} configuration failed: {}",
                    status_error(),
                    channel.name,
                    e
                );
                println!(
                    "  {}",
                    colorize("You can configure this channel later.", DIM)
                );
            }
        }
    }

    // Summary
    if !configured.is_empty() {
        println!("\n{}", draw_separator());
        println!(
            "\n  {} {} channel(s) configured: {}",
            status_success(),
            configured.len(),
            configured.join(", ")
        );
    }

    Ok(configured)
}

// ============================================================================
// Channel selection UI
// ============================================================================

/// Multi-select prompt for choosing which channels to configure.
fn select_channels() -> Result<Vec<usize>> {
    println!("  Select channels to configure (comma-separated numbers, or Enter to skip):\n");

    for (idx, channel) in CHANNELS.iter().enumerate() {
        println!(
            "    {}. {} - {}",
            colorize(&(idx + 1).to_string(), BOLD),
            channel.name,
            colorize(channel.description, DIM)
        );
    }
    println!();

    loop {
        print!("  Channels []: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            return Ok(vec![]);
        }

        let mut selected = Vec::new();
        let mut valid = true;

        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.parse::<usize>() {
                Ok(n) if n >= 1 && n <= CHANNELS.len() => {
                    if !selected.contains(&(n - 1)) {
                        selected.push(n - 1);
                    }
                }
                _ => {
                    println!(
                        "  {}",
                        colorize(
                            &format!(
                                "Invalid selection '{}'. Enter numbers 1-{}.",
                                part,
                                CHANNELS.len()
                            ),
                            ERROR
                        )
                    );
                    valid = false;
                    break;
                }
            }
        }

        if valid {
            return Ok(selected);
        }
    }
}

// ============================================================================
// Telegram configuration
// ============================================================================

async fn configure_telegram(config: &mut Config) -> Result<bool> {
    println!(
        "  Get a bot token from {} in Telegram.\n",
        colorize("@BotFather", UNDERLINE)
    );

    // Bot token
    let token = prompt_secret("  Bot Token: ")?;
    if token.is_empty() {
        return Ok(false);
    }

    // Validate token via getMe API
    print!("  ");
    let mut spinner = Spinner::new("Validating token...");
    spinner.start();

    let validation = validate_telegram_token(&token).await;
    spinner.stop();

    match validation {
        Ok(bot_name) => {
            println!(
                "  {} Token valid — bot: @{}",
                status_success(),
                bot_name
            );
        }
        Err(e) => {
            println!("  {} Token validation failed: {}", status_warning(), e);
            if !prompt_yes_no_inline("  Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("Telegram")?;

    // Optional proxy
    let proxy = prompt_optional("  Proxy URL (optional, e.g. socks5://...): ")?;

    // Apply config
    config.channels.telegram = TelegramConfig {
        enabled: true,
        token: Secret::new(token),
        allow_from,
        proxy,
    };

    Ok(true)
}

/// Validate a Telegram bot token by calling the getMe API.
async fn validate_telegram_token(token: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let url = format!("https://api.telegram.org/bot{}/getMe", token);
    let resp = client.get(&url).send().await?;
    let data: serde_json::Value = resp.json().await?;

    if data.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        let username = data
            .get("result")
            .and_then(|r| r.get("username"))
            .and_then(|u| u.as_str())
            .unwrap_or("unknown");
        Ok(username.to_string())
    } else {
        let description = data
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("Unknown error");
        anyhow::bail!("{}", description);
    }
}

// ============================================================================
// Discord configuration
// ============================================================================

async fn configure_discord(config: &mut Config) -> Result<bool> {
    println!(
        "  Create a bot at {}\n",
        colorize("https://discord.com/developers/applications", UNDERLINE)
    );

    // Bot token
    let token = prompt_secret("  Bot Token: ")?;
    if token.is_empty() {
        return Ok(false);
    }

    // Validate token
    print!("  ");
    let mut spinner = Spinner::new("Validating token...");
    spinner.start();

    let validation = oauth::validate_discord_token(&token).await;
    spinner.stop();

    match validation {
        Ok(bot_name) => {
            println!("  {} Token valid — bot: {}", status_success(), bot_name);

            // Generate invite URL using oauth module's properly URL-encoded helper
            let app_id = get_discord_app_id(&token).await.unwrap_or_default();
            if !app_id.is_empty() {
                let invite_url = oauth::discord_bot_invite_url(&app_id, 274877991936);
                println!(
                    "\n  {} Invite your bot to a server:",
                    colorize("Invite URL:", BOLD)
                );
                println!("  {}", colorize(&invite_url, UNDERLINE));
            }
        }
        Err(e) => {
            println!("  {} Token validation failed: {}", status_warning(), e);
            if !prompt_yes_no_inline("  Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("Discord")?;

    // Apply config
    config.channels.discord = DiscordConfig {
        enabled: true,
        token: Secret::new(token),
        allow_from,
        ..DiscordConfig::default()
    };

    Ok(true)
}

/// Get Discord application ID from the bot token.
async fn get_discord_app_id(token: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp = client
        .get("https://discord.com/api/v10/oauth2/applications/@me")
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await?;

    let data: serde_json::Value = resp.json().await?;
    let id = data
        .get("id")
        .and_then(|i| i.as_str())
        .unwrap_or("");
    Ok(id.to_string())
}

// ============================================================================
// Slack configuration
// ============================================================================

async fn configure_slack(config: &mut Config) -> Result<bool> {
    println!(
        "  Create a Slack app at {}\n",
        colorize("https://api.slack.com/apps", UNDERLINE)
    );
    println!(
        "  {}",
        colorize("You need both a Bot Token (xoxb-) and an App Token (xapp-).", DIM)
    );
    println!(
        "  {}",
        colorize("Enable Socket Mode in your app settings.\n", DIM)
    );

    // Bot token
    let bot_token = prompt_secret("  Bot Token (xoxb-...): ")?;
    if bot_token.is_empty() {
        return Ok(false);
    }

    // App token
    let app_token = prompt_secret("  App Token (xapp-...): ")?;
    if app_token.is_empty() {
        return Ok(false);
    }

    // Validate bot token
    print!("  ");
    let mut spinner = Spinner::new("Validating tokens...");
    spinner.start();

    let validation = oauth::validate_slack_token(&bot_token).await;
    spinner.stop();

    match validation {
        Ok((_bot_id, team)) => {
            println!("  {} Tokens valid — workspace: {}", status_success(), team);
        }
        Err(e) => {
            println!("  {} Token validation failed: {}", status_warning(), e);
            if !prompt_yes_no_inline("  Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("Slack")?;

    // Apply config
    config.channels.slack = SlackConfig {
        enabled: true,
        bot_token: Secret::new(bot_token),
        app_token: Secret::new(app_token),
        allow_from,
        ..SlackConfig::default()
    };

    Ok(true)
}

// ============================================================================
// WhatsApp configuration
// ============================================================================

async fn configure_whatsapp(config: &mut Config) -> Result<bool> {
    println!(
        "  {}",
        colorize(
            "WhatsApp requires a Node.js bridge (Baileys) running separately.",
            WARNING
        )
    );
    println!(
        "  {}",
        colorize(
            "See: https://github.com/WhiskeySockets/Baileys for bridge setup.\n",
            DIM
        )
    );

    // Bridge URL
    let default_url = "ws://localhost:3001";
    print!("  Bridge URL [{}]: ", default_url);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let bridge_url = if input.trim().is_empty() {
        default_url.to_string()
    } else {
        input.trim().to_string()
    };

    // Test bridge connection
    print!("  ");
    let mut spinner = Spinner::new("Testing bridge connection...");
    spinner.start();

    let bridge_ok = test_websocket_connection(&bridge_url).await;
    spinner.stop();

    match bridge_ok {
        Ok(()) => {
            println!("  {} Bridge reachable at {}", status_success(), bridge_url);
            println!(
                "\n  {}",
                colorize(
                    "Note: You'll need to scan a QR code when the bridge starts.",
                    DIM
                )
            );
        }
        Err(e) => {
            println!(
                "  {} Bridge not reachable: {}",
                status_warning(),
                e
            );
            println!(
                "  {}",
                colorize(
                    "The bridge may not be running yet. Configuration will be saved anyway.",
                    DIM
                )
            );
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("WhatsApp")?;

    // Apply config
    config.channels.whatsapp = WhatsAppConfig {
        enabled: true,
        bridge_url,
        allow_from,
    };

    Ok(true)
}

/// Test a WebSocket connection (just attempt to connect, then disconnect).
async fn test_websocket_connection(url: &str) -> Result<()> {
    let connect_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio_tungstenite::connect_async(url),
    )
    .await;

    match connect_result {
        Ok(Ok((_ws_stream, _))) => Ok(()),
        Ok(Err(e)) => anyhow::bail!("{}", e),
        Err(_) => anyhow::bail!("Connection timed out"),
    }
}

// ============================================================================
// Email configuration
// ============================================================================

async fn configure_email(config: &mut Config) -> Result<bool> {
    println!(
        "  {}",
        colorize(
            "Email channel reads your mailbox via IMAP and sends replies via SMTP.",
            DIM
        )
    );
    println!(
        "  {}\n",
        colorize(
            "This requires IMAP/SMTP credentials and explicit consent.",
            WARNING
        )
    );

    // Consent
    println!("  {} Email access grants klyntbot permission to:", colorize("Privacy notice:", BOLD));
    println!("    - Read unread emails from your IMAP mailbox");
    println!("    - Send replies via SMTP on your behalf");
    println!("    - Mark messages as read\n");

    if !prompt_yes_no_inline("  Do you consent to email access?", false)? {
        return Ok(false);
    }

    println!();

    // IMAP configuration
    println!("  {}", colorize("IMAP (Incoming Mail)", BOLD));
    let imap_host = prompt_required("  IMAP Host (e.g. imap.gmail.com): ")?;
    let imap_port = prompt_with_default("  IMAP Port", "993")?
        .parse::<u16>()
        .unwrap_or(993);
    let imap_username = prompt_required("  IMAP Username (email): ")?;
    let imap_password = prompt_secret("  IMAP Password: ")?;
    let imap_use_ssl = prompt_yes_no_inline("  Use SSL?", true)?;

    println!();

    // SMTP configuration
    println!("  {}", colorize("SMTP (Outgoing Mail)", BOLD));
    let smtp_host = prompt_required("  SMTP Host (e.g. smtp.gmail.com): ")?;
    let smtp_port = prompt_with_default("  SMTP Port", "587")?
        .parse::<u16>()
        .unwrap_or(587);

    // Default SMTP credentials to IMAP values
    let smtp_user_default = &imap_username;
    let smtp_username = prompt_with_default("  SMTP Username", smtp_user_default)?;
    let smtp_password_input = prompt_optional_secret("  SMTP Password (Enter to use IMAP password): ")?;
    let smtp_password = if smtp_password_input.is_empty() {
        imap_password.clone()
    } else {
        smtp_password_input
    };
    let smtp_use_tls = prompt_yes_no_inline("  Use TLS?", true)?;

    println!();

    // From address
    let from_default = &smtp_username;
    let from_address = prompt_with_default("  From Address", from_default)?;

    // Test connections
    print!("  ");
    let mut spinner = Spinner::new("Testing IMAP connection...");
    spinner.start();

    let imap_test = test_imap_connection(&imap_host, imap_port, &imap_username, &imap_password, imap_use_ssl).await;
    spinner.stop();

    match imap_test {
        Ok(()) => println!("  {} IMAP connection successful", status_success()),
        Err(e) => {
            println!("  {} IMAP connection failed: {}", status_warning(), e);
            if !prompt_yes_no_inline("  Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    print!("  ");
    let mut spinner = Spinner::new("Testing SMTP connection...");
    spinner.start();

    let smtp_test = test_smtp_connection(&smtp_host, smtp_port, &smtp_username, &smtp_password).await;
    spinner.stop();

    match smtp_test {
        Ok(()) => println!("  {} SMTP connection successful", status_success()),
        Err(e) => {
            println!("  {} SMTP connection failed: {}", status_warning(), e);
            if !prompt_yes_no_inline("  Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("Email")?;

    // Apply config
    config.channels.email = EmailConfig {
        enabled: true,
        consent_granted: true,
        imap_host,
        imap_port,
        imap_username,
        imap_password: Secret::new(imap_password),
        imap_use_ssl,
        imap_mailbox: "INBOX".to_string(),
        smtp_host,
        smtp_port,
        smtp_username,
        smtp_password: Secret::new(smtp_password),
        smtp_use_tls,
        from_address,
        allow_from,
        ..EmailConfig::default()
    };

    Ok(true)
}

/// Test IMAP connection by attempting login.
async fn test_imap_connection(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    use_ssl: bool,
) -> Result<()> {
    use tokio::net::TcpStream;

    let connect_result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        TcpStream::connect((host, port)),
    )
    .await;

    let tcp_stream = match connect_result {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => anyhow::bail!("TCP connection failed: {}", e),
        Err(_) => anyhow::bail!("Connection timed out"),
    };

    if use_ssl {
        let tls_connector = native_tls::TlsConnector::builder().build()?;
        let tls_connector = tokio_native_tls::TlsConnector::from(tls_connector);
        let tls_stream = tls_connector.connect(host, tcp_stream).await?;

        let client = async_imap::Client::new(tls_stream);
        let mut session = client
            .login(username, password)
            .await
            .map_err(|(e, _)| anyhow::anyhow!("Login failed: {}", e))?;
        let _ = session.logout().await;
    } else {
        let client = async_imap::Client::new(tcp_stream);
        let mut session = client
            .login(username, password)
            .await
            .map_err(|(e, _)| anyhow::anyhow!("Login failed: {}", e))?;
        let _ = session.logout().await;
    }

    Ok(())
}

/// Test SMTP connection by attempting relay setup.
async fn test_smtp_connection(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
) -> Result<()> {
    let host = host.to_string();
    let username = username.to_string();
    let password = password.to_string();

    tokio::task::spawn_blocking(move || {
        use lettre::transport::smtp::authentication::Credentials;
        use lettre::SmtpTransport;

        let creds = Credentials::new(username, password);
        let mailer = SmtpTransport::relay(&host)?
            .credentials(creds)
            .port(port)
            .build();

        mailer.test_connection()?;
        Ok::<(), anyhow::Error>(())
    })
    .await??;

    Ok(())
}

// ============================================================================
// QQ configuration
// ============================================================================

async fn configure_qq(config: &mut Config) -> Result<bool> {
    println!(
        "  Register a bot at {}\n",
        colorize("https://q.qq.com", UNDERLINE)
    );

    // App ID
    let app_id = prompt_required("  App ID: ")?;

    // Secret
    let secret = prompt_secret("  App Secret: ")?;
    if secret.is_empty() {
        return Ok(false);
    }

    // Validate credentials
    print!("  ");
    let mut spinner = Spinner::new("Validating credentials...");
    spinner.start();

    let validation = validate_qq_credentials(&app_id, &secret).await;
    spinner.stop();

    match validation {
        Ok(()) => {
            println!("  {} Credentials valid", status_success());
        }
        Err(e) => {
            println!(
                "  {} Credential validation failed: {}",
                status_warning(),
                e
            );
            if !prompt_yes_no_inline("  Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("QQ")?;

    // Apply config
    config.channels.qq = QQConfig {
        enabled: true,
        app_id,
        secret: Secret::new(secret),
        allow_from,
    };

    Ok(true)
}

/// Validate QQ credentials by attempting to get an access token.
async fn validate_qq_credentials(app_id: &str, secret: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp = client
        .post("https://api.sgroup.qq.com/app/getAppAccessToken")
        .json(&serde_json::json!({
            "appId": app_id,
            "clientSecret": secret,
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }

    let data: serde_json::Value = resp.json().await?;
    if data.get("access_token").is_some() {
        Ok(())
    } else {
        let msg = data
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Invalid credentials");
        anyhow::bail!("{}", msg);
    }
}

// ============================================================================
// Shared prompt helpers
// ============================================================================

/// Draw a separator line (matches mod.rs style).
fn draw_separator() -> String {
    let chars = BoxChars::get();
    format!(
        "{}{}{}",
        color(SEPARATOR),
        chars.horizontal.repeat(60),
        color(RESET)
    )
}

/// Prompt for a required non-empty value.
fn prompt_required(prompt: &str) -> Result<String> {
    loop {
        print!("{}", prompt);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if !input.is_empty() {
            return Ok(input);
        }
        println!(
            "  {}",
            colorize("This field is required.", ERROR)
        );
    }
}

/// Prompt for a secret value (displayed as-is since we can't hide stdin easily).
fn prompt_secret(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

/// Prompt for an optional secret value.
fn prompt_optional_secret(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

/// Prompt with a default value.
fn prompt_with_default(prompt: &str, default: &str) -> Result<String> {
    print!("{} [{}]: ", prompt, default);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input.to_string())
    }
}

/// Prompt for an optional value (returns None-wrapped in Option via String).
fn prompt_optional(prompt: &str) -> Result<Option<String>> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_string();

    if input.is_empty() {
        Ok(None)
    } else {
        Ok(Some(input))
    }
}

/// Inline yes/no prompt.
fn prompt_yes_no_inline(prompt: &str, default: bool) -> Result<bool> {
    let default_str = if default { "Y/n" } else { "y/N" };
    print!("{} [{}]: ", prompt, default_str);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input.is_empty() {
        return Ok(default);
    }

    Ok(input == "y" || input == "yes")
}

/// Prompt for allowlist entries (comma-separated user IDs).
fn prompt_allowlist(channel_name: &str) -> Result<Vec<String>> {
    println!(
        "\n  {} Restrict who can use klyntbot via {}.",
        colorize("Allowlist:", BOLD),
        channel_name
    );
    println!(
        "  {}",
        colorize("Leave empty to allow everyone.", DIM)
    );
    print!("  Allowed IDs (comma-separated): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        Ok(vec![])
    } else {
        Ok(input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_info_count() {
        assert_eq!(CHANNELS.len(), 6);
    }

    #[test]
    fn test_channel_keys_unique() {
        let keys: Vec<&str> = CHANNELS.iter().map(|c| c.key).collect();
        let mut unique_keys = keys.clone();
        unique_keys.sort();
        unique_keys.dedup();
        assert_eq!(keys.len(), unique_keys.len());
    }

    #[test]
    fn test_channel_names_match_keys() {
        for channel in CHANNELS {
            assert_eq!(channel.key, channel.name.to_lowercase());
        }
    }
}
