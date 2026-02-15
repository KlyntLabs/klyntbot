// Apple Calendar provider — CalDAV via iCloud

use async_trait::async_trait;
use common::Result;
use tokio::sync::RwLock;

use crate::caldav::client::CalDavClient;
use crate::provider::CalendarProvider;
use crate::types::CalendarEvent;

/// Apple Calendar provider using CalDAV with Basic auth.
///
/// Handles iCloud CalDAV discovery (well-known endpoint → principal → calendar-home → calendar).
pub struct AppleCalendarProvider {
    client: RwLock<CalDavClient>,
    calendar_name: String,
    username: String,
    password: String,
    base_url: String,
    discovered: RwLock<bool>,
}

impl AppleCalendarProvider {
    pub fn new(
        caldav_url: String,
        username: String,
        password: String,
        calendar_name: String,
        timezone: String,
    ) -> Self {
        let client = CalDavClient::new(
            caldav_url.clone(),
            username.clone(),
            password.clone(),
            timezone,
        );

        Self {
            client: RwLock::new(client),
            calendar_name,
            username,
            password,
            base_url: caldav_url,
            discovered: RwLock::new(false),
        }
    }

    /// Ensure the calendar URL has been discovered if using a base URL.
    async fn ensure_discovered(&self) -> Result<()> {
        {
            let discovered = self.discovered.read().await;
            if *discovered {
                return Ok(());
            }
        }

        let current_url = &self.base_url;
        let needs_discovery = current_url == "https://caldav.icloud.com/"
            || current_url == "https://caldav.icloud.com"
            || current_url.ends_with("/.well-known/caldav")
            || (!current_url.ends_with(".ics") && !current_url.contains("/calendars/"));

        if !needs_discovery {
            let mut discovered = self.discovered.write().await;
            *discovered = true;
            return Ok(());
        }

        tracing::info!("Apple CalDAV discovery for base URL: {}", current_url);

        let discovered_url = CalDavClient::discover_calendar_url(
            current_url,
            &self.username,
            &self.password,
            &self.calendar_name,
        )
        .await?;

        let full_url = if discovered_url.starts_with("http") {
            discovered_url
        } else {
            let base_host = current_url.split('/').take(3).collect::<Vec<_>>().join("/");
            format!("{}{}", base_host, discovered_url)
        };

        tracing::info!("Discovered Apple calendar URL: {}", full_url);

        {
            let mut client = self.client.write().await;
            client.set_calendar_url(full_url);
        }

        {
            let mut discovered = self.discovered.write().await;
            *discovered = true;
        }

        Ok(())
    }
}

#[async_trait]
impl CalendarProvider for AppleCalendarProvider {
    fn name(&self) -> &str {
        "Apple Calendar"
    }

    fn provider_id(&self) -> &str {
        "apple"
    }

    async fn get_events(
        &self,
        sync_token: Option<&str>,
    ) -> Result<(Vec<CalendarEvent>, Option<String>)> {
        self.ensure_discovered().await?;
        let client = self.client.read().await;
        client.get_events(sync_token).await
    }

    async fn put_event(&self, event: &CalendarEvent) -> Result<String> {
        self.ensure_discovered().await?;
        let client = self.client.read().await;
        client.put_event(event).await
    }

    async fn delete_event(&self, uid: &str) -> Result<()> {
        self.ensure_discovered().await?;
        let client = self.client.read().await;
        client.delete_event(uid).await
    }

    async fn test_connection(&self) -> Result<()> {
        self.ensure_discovered().await?;
        // If discovery succeeded, the connection works
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apple_provider_creation() {
        let provider = AppleCalendarProvider::new(
            "https://caldav.icloud.com".to_string(),
            "user@icloud.com".to_string(),
            "app-password".to_string(),
            "Klyntbot Tasks".to_string(),
            "UTC".to_string(),
        );

        assert_eq!(provider.name(), "Apple Calendar");
        assert_eq!(provider.provider_id(), "apple");
        assert_eq!(provider.username, "user@icloud.com");
    }

    #[test]
    fn test_apple_provider_with_direct_url() {
        let provider = AppleCalendarProvider::new(
            "https://p123-caldav.icloud.com:443/12345/calendars/klyntbot/".to_string(),
            "user@icloud.com".to_string(),
            "app-password".to_string(),
            "Klyntbot Tasks".to_string(),
            "Asia/Bangkok".to_string(),
        );

        assert_eq!(
            provider.base_url,
            "https://p123-caldav.icloud.com:443/12345/calendars/klyntbot/"
        );
    }
}
