//! Channel manager for orchestrating multiple chat channels.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::bus::MessageBus;
use crate::channels::{
    DiscordChannel, DynChannel, EmailChannel, QQChannel, SlackChannel, TelegramChannel,
    WhatsAppChannel,
};
use crate::config::Config;
use crate::error::Result;

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
    config: Arc<Config>,
}

impl ChannelManager {
    /// Create a new channel manager
    pub fn new(config: Arc<Config>, bus: Arc<MessageBus>) -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            bus,
            config,
        }
    }

    /// Initialize all enabled channels
    pub async fn initialize_channels(&self) -> Result<()> {
        let mut channels = self.channels.write().await;

        // Telegram (needs groq key for voice transcription)
        let groq_key = (!self.config.providers.groq.api_key.is_empty())
            .then(|| self.config.providers.groq.api_key.clone());
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

        // WhatsApp doesn't return Result, so handle separately
        if self.config.channels.whatsapp.enabled {
            info!("Initializing WhatsApp channel");
            let channel = WhatsAppChannel::new(self.config.channels.whatsapp.clone());
            channels.insert("whatsapp".to_string(), Arc::new(channel));
        }

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
    pub async fn start_all(&self) -> Result<()> {
        // Initialize channels first
        self.initialize_channels().await?;

        let channels = self.channels.read().await;

        if channels.is_empty() {
            warn!("No channels enabled");
            return Ok(());
        }

        info!("Starting {} channel(s)", channels.len());

        // Start each channel in its own task
        let mut tasks = Vec::new();
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
        let bus_clone = self.bus.clone();
        let channels_clone = self.channels.clone();
        let dispatcher_task = tokio::spawn(async move {
            debug!("Starting outbound message dispatcher");

            loop {
                match bus_clone.consume_outbound().await {
                    Some(msg) => {
                        let channels = channels_clone.read().await;
                        if let Some(channel) = channels.get(&msg.channel) {
                            if let Err(e) = channel.send(&msg).await {
                                error!("Failed to send message to {}: {}", msg.channel, e);
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
