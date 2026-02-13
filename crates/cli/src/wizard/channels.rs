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
use super::prompts::{self, SelectOption};

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
    let chars = BoxChars::get();

    println!(
        "{} {} Connect klyntbot to your chat platforms.",
        colorize(chars.vertical, BRAND),
        colorize("Channels", BOLD)
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "Each channel can be tested after configuration.",
            DIM
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Channel selection (interactive multi-select)
    let selected = select_channels()?;

    if selected.is_empty() {
        println!(
            "{} {} No channels selected. You can set them up later with:",
            colorize(chars.vertical, BRAND),
            colorize("Skipped.", DIM)
        );
        println!(
            "{}",
            draw_step_line(&colorize("  klyntbot channels login <channel>", DIM))
        );
        return Ok(vec![]);
    }

    let mut configured = Vec::new();

    for &idx in &selected {
        let channel = &CHANNELS[idx];
        println!("{}", colorize(chars.vertical, BRAND));
        println!("{}", draw_separator());
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} {} {}",
            colorize(chars.vertical, BRAND),
            colorize("▸", SUCCESS),
            colorize(&format!("Configure {}", channel.name), BOLD)
        );
        println!(
            "{}",
            draw_step_line(&colorize(
                &format!("Prerequisite: {}", channel.prerequisites),
                DIM
            ))
        );
        println!("{}", colorize(chars.vertical, BRAND));

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
                    "{} {} {} configured successfully",
                    colorize(chars.vertical, BRAND),
                    status_success(),
                    channel.name
                );
            }
            Ok(false) => {
                println!(
                    "{} {} {} configuration skipped",
                    colorize(chars.vertical, BRAND),
                    colorize("○", DIM),
                    channel.name
                );
            }
            Err(e) => {
                println!(
                    "{} {} {} configuration failed: {}",
                    colorize(chars.vertical, BRAND),
                    status_error(),
                    channel.name,
                    e
                );
                println!(
                    "{}",
                    draw_step_line(&colorize("You can configure this channel later.", DIM))
                );
            }
        }
    }

    // Summary
    if !configured.is_empty() {
        println!("{}", colorize(chars.vertical, BRAND));
        println!("{}", draw_separator());
        println!(
            "{} {} {} channel(s) configured: {}",
            colorize(chars.vertical, BRAND),
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

/// Interactive multi-select prompt for choosing which channels to configure.
fn select_channels() -> Result<Vec<usize>> {
    let options: Vec<SelectOption<'_>> = CHANNELS
        .iter()
        .map(|c| SelectOption {
            label: c.name,
            description: c.description,
        })
        .collect();

    prompts::prompt_multi_select("Select channels to configure", &options)
}

// ============================================================================
// Telegram configuration
// ============================================================================

async fn configure_telegram(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();

    println!(
        "{} Get a bot token from {} in Telegram.",
        colorize(chars.vertical, BRAND),
        colorize("@BotFather", UNDERLINE)
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Bot token
    let token = prompts::prompt_secret("Bot Token", 10)?;

    // Validate token via getMe API
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Validating token...");
    spinner.start();

    let validation = validate_telegram_token(&token).await;
    spinner.stop();

    match validation {
        Ok(bot_name) => {
            println!(
                "{} {} Token valid — bot: @{}",
                colorize(chars.vertical, BRAND),
                status_success(),
                bot_name
            );
        }
        Err(e) => {
            println!(
                "{} {} Token validation failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("Telegram")?;

    // Optional proxy
    let proxy = prompts::prompt_optional("Proxy URL (optional, e.g. socks5://...)")?;

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
    let chars = BoxChars::get();

    println!(
        "{} Create a bot at {}",
        colorize(chars.vertical, BRAND),
        colorize("https://discord.com/developers/applications", UNDERLINE)
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Bot token
    let token = prompts::prompt_secret("Bot Token", 10)?;

    // Validate token
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Validating token...");
    spinner.start();

    let validation = oauth::validate_discord_token(&token).await;
    spinner.stop();

    match validation {
        Ok(bot_name) => {
            println!(
                "{} {} Token valid — bot: {}",
                colorize(chars.vertical, BRAND),
                status_success(),
                bot_name
            );

            // Generate invite URL using oauth module's properly URL-encoded helper
            let app_id = get_discord_app_id(&token).await.unwrap_or_default();
            if !app_id.is_empty() {
                let invite_url = oauth::discord_bot_invite_url(&app_id, 274877991936);
                println!(
                    "{} {} Invite your bot to a server:",
                    colorize(chars.vertical, BRAND),
                    colorize("Invite URL:", BOLD)
                );
                println!("{}", draw_step_line(&colorize(&invite_url, UNDERLINE)));
            }
        }
        Err(e) => {
            println!(
                "{} {} Token validation failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
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
    let id = data.get("id").and_then(|i| i.as_str()).unwrap_or("");
    Ok(id.to_string())
}

// ============================================================================
// Slack configuration
// ============================================================================

async fn configure_slack(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();

    println!(
        "{} Create a Slack app at {}",
        colorize(chars.vertical, BRAND),
        colorize("https://api.slack.com/apps", UNDERLINE)
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "You need both a Bot Token (xoxb-) and an App Token (xapp-).",
            DIM
        ))
    );
    println!(
        "{}",
        draw_step_line(&colorize("Enable Socket Mode in your app settings.", DIM))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Bot token
    let bot_token = prompts::prompt_secret("Bot Token (xoxb-...)", 10)?;

    // App token
    let app_token = prompts::prompt_secret("App Token (xapp-...)", 10)?;

    // Validate bot token
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Validating tokens...");
    spinner.start();

    let validation = oauth::validate_slack_token(&bot_token).await;
    spinner.stop();

    match validation {
        Ok((_bot_id, team)) => {
            println!(
                "{} {} Tokens valid — workspace: {}",
                colorize(chars.vertical, BRAND),
                status_success(),
                team
            );
        }
        Err(e) => {
            println!(
                "{} {} Token validation failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
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
    let chars = BoxChars::get();

    println!(
        "{} {}",
        colorize(chars.vertical, BRAND),
        colorize(
            "WhatsApp requires a Node.js bridge (Baileys) running separately.",
            WARNING
        )
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "See: https://github.com/WhiskeySockets/Baileys for bridge setup.",
            DIM
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Bridge URL
    let bridge_url = prompts::prompt_text("Bridge URL", Some("ws://localhost:3001"), false)?;

    // Test bridge connection
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Testing bridge connection...");
    spinner.start();

    let bridge_ok = test_websocket_connection(&bridge_url).await;
    spinner.stop();

    match bridge_ok {
        Ok(()) => {
            println!(
                "{} {} Bridge reachable at {}",
                colorize(chars.vertical, BRAND),
                status_success(),
                bridge_url
            );
            println!(
                "{}",
                draw_step_line(&colorize(
                    "Note: You'll need to scan a QR code when the bridge starts.",
                    DIM
                ))
            );
        }
        Err(e) => {
            println!(
                "{} {} Bridge not reachable: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            println!(
                "{}",
                draw_step_line(&colorize(
                    "The bridge may not be running yet. Configuration will be saved anyway.",
                    DIM
                ))
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
    let chars = BoxChars::get();

    println!(
        "{}",
        draw_step_line(&colorize(
            "Email channel reads your mailbox via IMAP and sends replies via SMTP.",
            DIM
        ))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "This requires IMAP/SMTP credentials and explicit consent.",
            WARNING
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Consent
    println!(
        "{} {} Email access grants klyntbot permission to:",
        colorize(chars.vertical, BRAND),
        colorize("Privacy notice:", BOLD)
    );
    println!(
        "{} - Read unread emails from your IMAP mailbox",
        colorize(chars.vertical, BRAND)
    );
    println!(
        "{} - Send replies via SMTP on your behalf",
        colorize(chars.vertical, BRAND)
    );
    println!(
        "{} - Mark messages as read",
        colorize(chars.vertical, BRAND)
    );
    println!("{}", colorize(chars.vertical, BRAND));

    if !prompts::prompt_yes_no("Do you consent to email access?", false)? {
        return Ok(false);
    }

    println!("{}", colorize(chars.vertical, BRAND));

    // IMAP configuration
    println!(
        "{} {}",
        colorize(chars.vertical, BRAND),
        colorize("IMAP (Incoming Mail)", BOLD)
    );
    let imap_host = prompts::prompt_text("IMAP Host (e.g. imap.gmail.com)", None, true)?;
    let imap_port = prompts::prompt_text("IMAP Port", Some("993"), false)?
        .parse::<u16>()
        .unwrap_or(993);
    let imap_username = prompts::prompt_text("IMAP Username (email)", None, true)?;
    let imap_password = prompts::prompt_secret("IMAP Password", 1)?;
    let imap_use_ssl = prompts::prompt_yes_no("Use SSL?", true)?;

    println!("{}", colorize(chars.vertical, BRAND));

    // SMTP configuration
    println!(
        "{} {}",
        colorize(chars.vertical, BRAND),
        colorize("SMTP (Outgoing Mail)", BOLD)
    );
    let smtp_host = prompts::prompt_text("SMTP Host (e.g. smtp.gmail.com)", None, true)?;
    let smtp_port = prompts::prompt_text("SMTP Port", Some("587"), false)?
        .parse::<u16>()
        .unwrap_or(587);

    // Default SMTP credentials to IMAP values
    let smtp_username = prompts::prompt_text("SMTP Username", Some(&imap_username), false)?;
    let smtp_password_input = prompts::prompt_text(
        "SMTP Password (Enter to use IMAP password)",
        Some(""),
        false,
    )?;
    let smtp_password = if smtp_password_input.is_empty() {
        imap_password.clone()
    } else {
        smtp_password_input
    };
    let smtp_use_tls = prompts::prompt_yes_no("Use TLS?", true)?;

    println!("{}", colorize(chars.vertical, BRAND));

    // From address
    let from_address = prompts::prompt_text("From Address", Some(&smtp_username), false)?;

    // Test connections
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Testing IMAP connection...");
    spinner.start();

    let imap_test = test_imap_connection(
        &imap_host,
        imap_port,
        &imap_username,
        &imap_password,
        imap_use_ssl,
    )
    .await;
    spinner.stop();

    match imap_test {
        Ok(()) => println!(
            "{} {} IMAP connection successful",
            colorize(chars.vertical, BRAND),
            status_success()
        ),
        Err(e) => {
            println!(
                "{} {} IMAP connection failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Testing SMTP connection...");
    spinner.start();

    let smtp_test =
        test_smtp_connection(&smtp_host, smtp_port, &smtp_username, &smtp_password).await;
    spinner.stop();

    match smtp_test {
        Ok(()) => println!(
            "{} {} SMTP connection successful",
            colorize(chars.vertical, BRAND),
            status_success()
        ),
        Err(e) => {
            println!(
                "{} {} SMTP connection failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
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
async fn test_smtp_connection(host: &str, port: u16, username: &str, password: &str) -> Result<()> {
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
    let chars = BoxChars::get();

    println!(
        "{} Register a bot at {}",
        colorize(chars.vertical, BRAND),
        colorize("https://q.qq.com", UNDERLINE)
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // App ID
    let app_id = prompts::prompt_text("App ID", None, true)?;

    // Secret
    let secret = prompts::prompt_secret("App Secret", 1)?;

    // Validate credentials
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Validating credentials...");
    spinner.start();

    let validation = validate_qq_credentials(&app_id, &secret).await;
    spinner.stop();

    match validation {
        Ok(()) => {
            println!(
                "{} {} Credentials valid",
                colorize(chars.vertical, BRAND),
                status_success()
            );
        }
        Err(e) => {
            println!(
                "{} {} Credential validation failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
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

/// Prompt for allowlist entries (comma-separated user IDs).
fn prompt_allowlist(channel_name: &str) -> Result<Vec<String>> {
    let chars = BoxChars::get();

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Restrict who can use klyntbot via {}.",
        colorize(chars.vertical, BRAND),
        colorize("Allowlist:", BOLD),
        channel_name
    );
    println!(
        "{}",
        draw_step_line(&colorize("Leave empty to allow everyone.", DIM))
    );

    print!(
        "{} Allowed IDs (comma-separated): ",
        colorize(chars.vertical, BRAND)
    );
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
