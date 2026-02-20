//! Channel manager for orchestrating multiple chat channels.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::{
    DiscordChannel, DynChannel, EmailChannel, QQChannel, SlackChannel, TelegramChannel,
    WhatsAppChannel,
};
use bus::{MessageBus, OutboundMessage};
use common::{ChannelError, Result};
use config::Config;

/// Convert an error into a short, user-friendly description.
fn user_facing_error(err: &common::KlyntbotError) -> String {
    match err {
        common::KlyntbotError::Channel(ChannelError::ConnectionFailed(_)) => {
            "Connection issue — the channel may be temporarily unavailable.".to_string()
        }
        common::KlyntbotError::Channel(ChannelError::SendFailed(detail)) => {
            if detail.contains("rate limit") || detail.contains("429") {
                "Rate limited — please wait a moment and try again.".to_string()
            } else {
                "Message delivery failed — please try again.".to_string()
            }
        }
        _ => "An unexpected error occurred.".to_string(),
    }
}

/// Macro to reduce duplication in channel initialization.
/// Handles the common pattern of checking enabled status, logging, creating the channel,
/// and inserting it into the channels map.
macro_rules! init_channel {
    ($channels:expr, $config:expr, $name:literal, $create:expr) => {
        if $config.enabled {
            info!("Initializing {} channel", $name);
            match $create {
                Ok(channel) => {
                    $channels.insert($name.to_string(), Arc::new(channel) as DynChannel);
                }
                Err(e) => error!("Failed to create {} channel: {}", $name, e),
            }
        }
    };
}

/// Channel manager for all chat platforms
pub struct ChannelManager {
    channels: Arc<RwLock<HashMap<String, DynChannel>>>,
    bus: Arc<MessageBus>,
    outbound_rx: Option<mpsc::Receiver<OutboundMessage>>,
    config: Arc<Config>,
}

impl ChannelManager {
    /// Create a new channel manager
    pub fn new(config: Arc<Config>, bus: Arc<MessageBus>) -> Result<Self> {
        // Take ownership of the outbound receiver
        let outbound_rx = bus.take_outbound_rx().ok_or_else(|| {
            ChannelError::InvalidConfig(
                "Outbound receiver already taken - ChannelManager may have been instantiated twice"
                    .to_string(),
            )
        })?;

        Ok(Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            bus,
            outbound_rx: Some(outbound_rx),
            config,
        })
    }

    /// Initialize all enabled channels
    pub async fn initialize_channels(&self) -> Result<()> {
        let mut channels = self.channels.write().await;

        // Telegram (needs groq key for voice transcription)
        let groq_key = (!self.config.providers.groq.api_key.is_empty())
            .then(|| self.config.providers.groq.api_key.expose().clone());
        init_channel!(
            channels,
            self.config.channels.telegram,
            "telegram",
            TelegramChannel::new(self.config.channels.telegram.clone(), groq_key)
        );

        init_channel!(
            channels,
            self.config.channels.discord,
            "discord",
            DiscordChannel::new(self.config.channels.discord.clone())
        );

        init_channel!(
            channels,
            self.config.channels.whatsapp,
            "whatsapp",
            WhatsAppChannel::new(self.config.channels.whatsapp.clone())
        );

        init_channel!(
            channels,
            self.config.channels.qq,
            "qq",
            QQChannel::new(self.config.channels.qq.clone())
        );

        init_channel!(
            channels,
            self.config.channels.slack,
            "slack",
            SlackChannel::new(self.config.channels.slack.clone())
        );

        init_channel!(
            channels,
            self.config.channels.email,
            "email",
            EmailChannel::new(self.config.channels.email.clone())
        );

        Ok(())
    }

    /// Start all enabled channels
    pub async fn start_all(&mut self) -> Result<()> {
        // Initialize channels first
        self.initialize_channels().await?;

        let channels = self.channels.read().await;

        if channels.is_empty() {
            warn!("No channels enabled");
            return Ok(());
        }

        info!("Starting {} channel(s)", channels.len());

        // Start each channel in its own task
        let mut tasks = Vec::with_capacity(channels.len());
        for (name, channel) in channels.iter() {
            let channel = channel.clone();
            let bus = self.bus.clone();
            let name = name.clone();

            let task = tokio::spawn(async move {
                info!("Starting channel: {}", name);
                if let Err(e) = channel.start(bus).await {
                    error!("Channel {} failed: {}", name, e);
                }
            });

            tasks.push(task);
        }

        // Start outbound dispatcher
        let mut outbound_rx = self.outbound_rx.take().ok_or_else(|| {
            ChannelError::InvalidConfig(
                "Outbound receiver already taken - start_all() may have been called twice"
                    .to_string(),
            )
        })?;
        let channels_clone = self.channels.clone();
        let dispatcher_task = tokio::spawn(async move {
            debug!("Starting outbound message dispatcher");

            loop {
                match outbound_rx.recv().await {
                    Some(msg) => {
                        let channels = channels_clone.read().await;
                        if let Some(channel) = channels.get(msg.channel.as_str()) {
                            if let Err(e) = channel.send(&msg).await {
                                error!("Failed to send message to {}: {}", msg.channel, e);

                                // Send user-facing error feedback (avoid infinite loop
                                // by not retrying if the error message itself fails)
                                let error_text = format!(
                                    "Sorry, I encountered an error sending that message. Please try again.\n({})",
                                    user_facing_error(&e)
                                );
                                let error_msg = OutboundMessage {
                                    channel: msg.channel.clone(),
                                    chat_id: msg.chat_id.clone(),
                                    content: error_text,
                                    reply_to: None,
                                    media: vec![],
                                    metadata: Default::default(),
                                };
                                if let Err(e2) = channel.send(&error_msg).await {
                                    error!(
                                        "Failed to send error feedback to {}: {} (giving up)",
                                        msg.channel, e2
                                    );
                                }
                            }
                        } else {
                            warn!("No channel found for: {}", msg.channel);
                        }
                    }
                    None => {
                        warn!("Outbound queue closed");
                        break;
                    }
                }
            }
        });
        tasks.push(dispatcher_task);

        // Wait for all tasks (they should run forever)
        for task in tasks {
            let _ = task.await;
        }

        Ok(())
    }

    /// Stop all channels
    pub async fn stop_all(&self) -> Result<()> {
        let channels = self.channels.read().await;

        for (name, channel) in channels.iter() {
            info!("Stopping channel: {}", name);
            if let Err(e) = channel.stop().await {
                error!("Failed to stop channel {}: {}", name, e);
            }
        }

        Ok(())
    }
}
