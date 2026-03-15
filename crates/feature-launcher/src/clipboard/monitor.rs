use crate::repos::ClipboardRepo;
use tokio_util::sync::CancellationToken;

pub struct ClipboardMonitor {
    repo: ClipboardRepo,
    max_entries: i64,
}

impl ClipboardMonitor {
    pub fn new(repo: ClipboardRepo, max_entries: i64) -> Self {
        Self { repo, max_entries }
    }

    #[cfg(target_os = "macos")]
    pub async fn start(&self, cancel: CancellationToken) {
        use tokio::time::{interval, Duration};

        let mut last_change_count: i64 = -1;
        let mut tick = interval(Duration::from_millis(500));

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tick.tick() => {
                    let current_count = self.get_change_count();

                    if current_count != last_change_count && last_change_count != -1 {
                        if let Some(content) = self.read_pasteboard() {
                            let source = self.get_frontmost_app_name();
                            if let Err(e) = self.repo.insert(
                                &content,
                                "text",
                                source.as_deref(),
                                None,
                            ).await {
                                tracing::error!("Failed to store clipboard entry: {}", e);
                            }
                            let _ = self.repo.evict_to_max(self.max_entries).await;
                        }
                    }
                    last_change_count = current_count;
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn get_change_count(&self) -> i64 {
        platform_macos::pasteboard::pasteboard_change_count()
    }

    #[cfg(target_os = "macos")]
    fn read_pasteboard(&self) -> Option<String> {
        platform_macos::pasteboard::read_pasteboard_string()
    }

    #[cfg(target_os = "macos")]
    fn get_frontmost_app_name(&self) -> Option<String> {
        platform_macos::window::get_frontmost_window().map(|w| w.app_name)
    }

    #[cfg(not(target_os = "macos"))]
    pub async fn start(&self, cancel: CancellationToken) {
        cancel.cancelled().await;
    }
}
