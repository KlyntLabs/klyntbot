//! Channels command handlers for managing communication channels

use crate::ChannelCommands;
use anyhow::Result;

/// Handle channel commands
pub async fn handle_channels(cmd: ChannelCommands) -> Result<()> {
    match cmd {
        ChannelCommands::List => {
            let config = config::load().await?;

            println!("Available channels:\n");

            // Telegram
            print_channel_info(
                "telegram",
                config.channels.telegram.enabled,
                !config.channels.telegram.token.is_empty(),
            );

            // Discord
            print_channel_info(
                "discord",
                config.channels.discord.enabled,
                !config.channels.discord.token.is_empty(),
            );

            // WhatsApp
            print_channel_info(
                "whatsapp",
                config.channels.whatsapp.enabled,
                !config.channels.whatsapp.bridge_url.is_empty(),
            );

            // Slack
            print_channel_info(
                "slack",
                config.channels.slack.enabled,
                !config.channels.slack.bot_token.is_empty()
                    && !config.channels.slack.app_token.is_empty(),
            );

            // Email
            print_channel_info(
                "email",
                config.channels.email.enabled,
                !config.channels.email.imap_host.is_empty()
                    && !config.channels.email.smtp_host.is_empty(),
            );

            // QQ
            print_channel_info(
                "qq",
                config.channels.qq.enabled,
                !config.channels.qq.app_id.is_empty() && !config.channels.qq.secret.is_empty(),
            );
        }

        ChannelCommands::Login { channel } => {
            let config = config::load().await?;

            match channel.to_lowercase().as_str() {
                "telegram" => {
                    println!("Telegram Login Setup\n");
                    println!("1. Create a bot with @BotFather on Telegram");
                    println!("2. Get your bot token");
                    println!("3. Add to config:");
                    println!("   channels.telegram.token = \"YOUR_BOT_TOKEN\"");
                    println!("   channels.telegram.enabled = true");
                    println!("\n4. Get your chat ID by messaging @userinfobot");
                    println!("5. Add to config:");
                    println!("   channels.telegram.allowFrom = [\"YOUR_CHAT_ID\"]");
                }

                "discord" => {
                    println!("Discord Login Setup\n");
                    println!(
                        "1. Create application at https://discord.com/developers/applications"
                    );
                    println!("2. Create a bot and get the token");
                    println!("3. Add to config:");
                    println!("   channels.discord.token = \"YOUR_BOT_TOKEN\"");
                    println!("   channels.discord.enabled = true");
                    println!("\n4. Invite bot to your server with permissions:");
                    println!("   - Read Messages/View Channels");
                    println!("   - Send Messages");
                    println!("   - Read Message History");
                }

                "whatsapp" => {
                    println!("WhatsApp Login Setup\n");
                    println!("WhatsApp uses a bridge server for authentication.");
                    println!("\n1. Start the bridge server (default: ws://localhost:3001)");
                    println!("2. Configure bridge URL:");
                    println!(
                        "   channels.whatsapp.bridgeUrl = \"{}\"",
                        config.channels.whatsapp.bridge_url
                    );
                    println!("   channels.whatsapp.enabled = true");
                    println!("\n3. Run 'klyntbot serve' to start the gateway");
                    println!("4. The bridge will provide a QR code for WhatsApp Web login");
                    println!("\nBridge URL: {}", config.channels.whatsapp.bridge_url);
                }

                "slack" => {
                    println!("Slack Login Setup\n");
                    println!("1. Create app at https://api.slack.com/apps");
                    println!("2. Enable Socket Mode and get App Token");
                    println!("3. Add Bot Token Scopes:");
                    println!("   - chat:write");
                    println!("   - im:history");
                    println!("   - im:read");
                    println!("4. Install app to workspace and get Bot Token");
                    println!("5. Add to config:");
                    println!("   channels.slack.botToken = \"xoxb-...\"");
                    println!("   channels.slack.appToken = \"xapp-...\"");
                    println!("   channels.slack.enabled = true");
                }

                "email" => {
                    println!("Email Login Setup\n");
                    println!("Configure IMAP and SMTP settings:");
                    println!("\nIMAP (incoming):");
                    println!("  channels.email.imapHost = \"imap.gmail.com\"");
                    println!("  channels.email.imapPort = 993");
                    println!("  channels.email.imapUsername = \"your@email.com\"");
                    println!("  channels.email.imapPassword = \"your_password\"");
                    println!("\nSMTP (outgoing):");
                    println!("  channels.email.smtpHost = \"smtp.gmail.com\"");
                    println!("  channels.email.smtpPort = 587");
                    println!("  channels.email.smtpUsername = \"your@email.com\"");
                    println!("  channels.email.smtpPassword = \"your_password\"");
                    println!("  channels.email.fromAddress = \"your@email.com\"");
                    println!("\n  channels.email.enabled = true");
                    println!("\nNote: For Gmail, use an App Password, not your main password");
                }

                "qq" => {
                    println!("QQ Login Setup\n");
                    println!("1. Register QQ bot at https://q.qq.com/");
                    println!("2. Get your App ID and Secret");
                    println!("3. Add to config:");
                    println!("   channels.qq.appId = \"YOUR_APP_ID\"");
                    println!("   channels.qq.secret = \"YOUR_SECRET\"");
                    println!("   channels.qq.enabled = true");
                }

                _ => {
                    return Err(anyhow::anyhow!(
                        "Unknown channel '{}'\n\nAvailable channels:\n  telegram, discord, whatsapp, slack, email, qq\n\nUsage: klyntbot channels login <channel>",
                        channel
                    ));
                }
            }
        }

        ChannelCommands::Test { channel } => {
            use bus::MessageBus;
            use channels::ChannelManager;
            use std::sync::Arc;

            let config = config::load().await?;

            // Check if the requested channel is configured
            let channel_name = channel.to_lowercase();
            let (enabled, configured) = match channel_name.as_str() {
                "telegram" => (
                    config.channels.telegram.enabled,
                    !config.channels.telegram.token.is_empty(),
                ),
                "discord" => (
                    config.channels.discord.enabled,
                    !config.channels.discord.token.is_empty(),
                ),
                "whatsapp" => (
                    config.channels.whatsapp.enabled,
                    !config.channels.whatsapp.bridge_url.is_empty(),
                ),
                "slack" => (
                    config.channels.slack.enabled,
                    !config.channels.slack.bot_token.is_empty()
                        && !config.channels.slack.app_token.is_empty(),
                ),
                "email" => (
                    config.channels.email.enabled,
                    !config.channels.email.imap_host.is_empty()
                        && !config.channels.email.smtp_host.is_empty(),
                ),
                "qq" => (
                    config.channels.qq.enabled,
                    !config.channels.qq.app_id.is_empty() && !config.channels.qq.secret.is_empty(),
                ),
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unknown channel '{}'\n\nAvailable channels:\n  telegram, discord, whatsapp, slack, email, qq\n\nUsage: klyntbot channels login <channel>",
                        channel
                    ));
                }
            };

            if !configured {
                println!("✗ Channel '{}' is not configured", channel);
                println!(
                    "Run 'klyntbot channels login {}' for setup instructions",
                    channel
                );
                return Ok(());
            }

            if !enabled {
                println!("✗ Channel '{}' is configured but not enabled", channel);
                println!("Set channels.{}.enabled = true in config", channel);
                return Ok(());
            }

            println!("Testing channel '{}'...", channel);

            // Create a temporary channel manager to test initialization
            let bus = Arc::new(MessageBus::new(10));
            let channel_manager = match ChannelManager::new(Arc::new(config), bus) {
                Ok(manager) => manager,
                Err(e) => {
                    println!("✗ Failed to create channel manager: {}", e);
                    return Err(anyhow::Error::from(e));
                }
            };

            // Try to initialize channels (this will create the channel)
            match channel_manager.initialize_channels().await {
                Ok(_) => {
                    println!("✓ Channel '{}' initialized successfully", channel);
                    println!("\nNote: Full connection test requires running 'klyntbot serve'");
                }
                Err(e) => {
                    println!("✗ Channel '{}' failed to initialize: {}", channel, e);
                    return Err(anyhow::Error::from(e));
                }
            }
        }
    }
    Ok(())
}

/// Print channel information
pub fn print_channel_info(name: &str, enabled: bool, configured: bool) {
    let status = if enabled && configured {
        "✓ enabled"
    } else if configured {
        "○ configured"
    } else {
        "✗ not configured"
    };

    println!("  {:<12} {}", name, status);
}
