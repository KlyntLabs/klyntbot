// Generic CalDAV provider — any CalDAV server with Basic auth

use async_trait::async_trait;
use common::Result;
use tokio::sync::RwLock;

use crate::caldav::client::CalDavClient;
use crate::provider::CalendarProvider;
use crate::types::CalendarEvent;

/// Generic CalDAV provider for Nextcloud, Fastmail, Zoho, Radicale, etc.
///
/// Users provide the CalDAV URL directly. Optional discovery if needed.
pub struct GenericCalDavProvider {
    client: RwLock<CalDavClient>,
    label: String,
    provider_id_str: String,
    username: String,
    password: String,
    base_url: String,
    calendar_name: String,
    discovered: RwLock<bool>,
}

impl GenericCalDavProvider {
    pub fn new(
        label: String,
        caldav_url: String,
        username: String,
        password: String,
        calendar_name: String,
        timezone: String,
    ) -> Self {
        let provider_id = format!(
            "generic-{}",
            label
                .to_lowercase()
                .replace(|c: char| !c.is_alphanumeric(), "-")
        );

        let client = CalDavClient::new(
            caldav_url.clone(),
            username.clone(),
            password.clone(),
            timezone,
        );

        Self {
            client: RwLock::new(client),
            label,
            provider_id_str: provider_id,
            username,
            password,
            base_url: caldav_url,
            calendar_name,
            discovered: RwLock::new(false),
        }
    }

    /// Try CalDAV discovery if the URL looks like a base URL.
    async fn ensure_discovered(&self) -> Result<()> {
        {
            let discovered = self.discovered.read().await;
            if *discovered {
                return Ok(());
            }
        }

        let url = &self.base_url;
        // If URL ends with /events/ or /calendars/ or .ics, assume it's specific
        let looks_specific = url.ends_with("/events/")
            || url.ends_with("/events")
            || url.contains("/calendars/")
            || url.ends_with(".ics");

        if looks_specific {
            let mut discovered = self.discovered.write().await;
            *discovered = true;
            return Ok(());
        }

        // Try discovery
        tracing::info!(
            "Generic CalDAV discovery for {} at {}",
            self.label,
            self.base_url
        );

        match CalDavClient::discover_calendar_url(
            &self.base_url,
            &self.username,
            &self.password,
            &self.calendar_name,
        )
        .await
        {
            Ok(discovered_url) => {
                let full_url = if discovered_url.starts_with("http") {
                    discovered_url
                } else {
                    let base_host = self
                        .base_url
                        .split('/')
                        .take(3)
                        .collect::<Vec<_>>()
                        .join("/");
                    format!("{}{}", base_host, discovered_url)
                };

                tracing::info!("Discovered {} calendar URL: {}", self.label, full_url);

                let mut client = self.client.write().await;
                client.set_calendar_url(full_url);
            }
            Err(e) => {
                tracing::warn!(
                    "CalDAV discovery failed for {}, using URL as-is: {}",
                    self.label,
                    e
                );
                // Continue without discovery — the URL might work as-is
            }
        }

        let mut discovered = self.discovered.write().await;
        *discovered = true;

        Ok(())
    }
}

#[async_trait]
impl CalendarProvider for GenericCalDavProvider {
    fn name(&self) -> &str {
        &self.label
    }

    fn provider_id(&self) -> &str {
        &self.provider_id_str
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
        let client = self.client.read().await;
        let _ = client.get_events(None).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_provider_creation() {
        let provider = GenericCalDavProvider::new(
            "Nextcloud".to_string(),
            "https://cloud.example.com/remote.php/dav".to_string(),
            "admin".to_string(),
            "password".to_string(),
            "Personal".to_string(),
            "UTC".to_string(),
        );

        assert_eq!(provider.name(), "Nextcloud");
        assert_eq!(provider.provider_id(), "generic-nextcloud");
    }

    #[test]
    fn test_generic_provider_id_sanitization() {
        let provider = GenericCalDavProvider::new(
            "My Fastmail Calendar!".to_string(),
            "https://caldav.fastmail.com/dav/calendars".to_string(),
            "user".to_string(),
            "pass".to_string(),
            "Default".to_string(),
            "UTC".to_string(),
        );

        assert_eq!(provider.provider_id(), "generic-my-fastmail-calendar-");
    }

    #[test]
    fn test_generic_provider_with_specific_url() {
        let provider = GenericCalDavProvider::new(
            "Radicale".to_string(),
            "https://radicale.example.com/user/calendars/tasks/".to_string(),
            "user".to_string(),
            "pass".to_string(),
            "tasks".to_string(),
            "UTC".to_string(),
        );

        assert_eq!(
            provider.base_url,
            "https://radicale.example.com/user/calendars/tasks/"
        );
    }
}
