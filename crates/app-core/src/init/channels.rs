use std::sync::Arc;

use bus::MessageBus;
use channels::ChannelManager;
use tokio::sync::Mutex;

/// Initialize the channel manager.
pub(super) fn init_channels(
    config: &config::Config,
    bus: &Arc<MessageBus>,
) -> Result<Arc<Mutex<ChannelManager>>, String> {
    // 9. Channel manager
    let channel_manager = Arc::new(Mutex::new(
        ChannelManager::new(Arc::new(config.clone()), bus.clone())
            .map_err(|e| format!("channel manager init failed: {e}"))?,
    ));

    Ok(channel_manager)
}
