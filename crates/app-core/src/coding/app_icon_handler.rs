//! `app_icon_read` handler — resolve a macOS app's icon to a base64 data URL.
//!
//! Returns `None` on non-macOS, when the app isn't found, or when icon
//! extraction fails. Callers display a fallback glyph in those cases.

use crate::AppCore;
use common::Result;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn app_icon_read(&self, app_name: &str) -> Result<Option<String>> {
        #[cfg(target_os = "macos")]
        {
            let app_name = app_name.to_string();
            let cache_dir = self.config.read().await.data_dir_path().join("icon-cache");
            let result = tokio::task::spawn_blocking(move || {
                let cache = platform_macos::apps::AppIconCache::new(cache_dir);
                let app_path = resolve_app_path(&app_name)?;
                let png = cache.resolve_icon_path(&app_path)?;
                let bytes = std::fs::read(&png).ok()?;
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                Some(format!("data:image/png;base64,{b64}"))
            })
            .await
            .ok()
            .flatten();
            Ok(result)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = app_name;
            Ok(None)
        }
    }
}

#[cfg(target_os = "macos")]
fn resolve_app_path(app_name: &str) -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    // Try the standard application directories. Don't use `mdfind`/Spotlight
    // here — it's slow and brittle in headless contexts.
    let candidates = [
        PathBuf::from("/Applications").join(format!("{app_name}.app")),
        PathBuf::from("/System/Applications").join(format!("{app_name}.app")),
        dirs::home_dir()?
            .join("Applications")
            .join(format!("{app_name}.app")),
    ];
    candidates.into_iter().find(|p| p.exists())
}
