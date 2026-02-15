// Google Calendar provider — CalDAV via OAuth2 Bearer tokens

use async_trait::async_trait;
use chrono::Utc;
use common::{CalendarError, KlyntbotError, Result};
use tokio::sync::RwLock;

use crate::caldav::client::{CalDavAuth, CalDavClient};
use crate::provider::CalendarProvider;
use crate::types::CalendarEvent;

/// Google Calendar provider using CalDAV with OAuth2 Bearer auth.
///
/// Google's CalDAV endpoint: `https://apidata.googleusercontent.com/caldav/v2/{calendar_id}/events/`
/// Tokens are refreshed automatically when expired (5-minute buffer).
pub struct GoogleCalendarProvider {
    client: RwLock<CalDavClient>,
    client_id: String,
    client_secret: String,
    refresh_token: String,
    access_token: RwLock<String>,
    token_expiry: RwLock<Option<chrono::DateTime<Utc>>>,
    /// Kept for diagnostic/status purposes.
    pub calendar_id: String,
}

impl GoogleCalendarProvider {
    pub fn new(
        client_id: String,
        client_secret: String,
        access_token: String,
        refresh_token: String,
        calendar_id: String,
        timezone: String,
    ) -> Self {
        let caldav_url = format!(
            "https://apidata.googleusercontent.com/caldav/v2/{}/events/",
            calendar_id
        );

        let client = CalDavClient::new_with_auth(
            caldav_url,
            CalDavAuth::Bearer {
                token: access_token.clone(),
            },
            timezone,
        );

        Self {
            client: RwLock::new(client),
            client_id,
            client_secret,
            refresh_token,
            access_token: RwLock::new(access_token),
            token_expiry: RwLock::new(None),
            calendar_id,
        }
    }

    /// Ensure the access token is fresh, refreshing if needed.
    async fn ensure_token_fresh(&self) -> Result<()> {
        let needs_refresh = {
            let expiry = self.token_expiry.read().await;
            match *expiry {
                Some(exp) => Utc::now() >= exp - chrono::Duration::minutes(5),
                None => true, // Unknown expiry, refresh to be safe
            }
        };

        if !needs_refresh {
            return Ok(());
        }

        tracing::debug!("Refreshing Google Calendar OAuth2 token");

        let http_client = reqwest::Client::new();
        let form_body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &self.client_id)
            .append_pair("client_secret", &self.client_secret)
            .append_pair("refresh_token", &self.refresh_token)
            .append_pair("grant_type", "refresh_token")
            .finish();

        let resp = http_client
            .post("https://oauth2.googleapis.com/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form_body)
            .send()
            .await
            .map_err(|e| {
                KlyntbotError::Calendar(CalendarError::ConnectionFailed(format!(
                    "Google token refresh failed: {}",
                    e
                )))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(KlyntbotError::Calendar(CalendarError::AuthFailed(
                format!("Google token refresh failed ({}): {}", status, body),
            )));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| {
            KlyntbotError::Calendar(CalendarError::ProtocolError(format!(
                "Failed to parse token response: {}",
                e
            )))
        })?;

        let new_token = json["access_token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        if new_token.is_empty() {
            return Err(KlyntbotError::Calendar(CalendarError::AuthFailed(
                "Google token refresh returned empty access_token".to_string(),
            )));
        }

        let expires_in = json["expires_in"].as_u64().unwrap_or(3600);
        let new_expiry = Utc::now() + chrono::Duration::seconds(expires_in as i64);

        // Update stored token
        {
            let mut token = self.access_token.write().await;
            *token = new_token.clone();
        }
        {
            let mut expiry = self.token_expiry.write().await;
            *expiry = Some(new_expiry);
        }

        // Update the CalDAV client's bearer token
        {
            let mut client = self.client.write().await;
            client.set_bearer_token(new_token);
        }

        tracing::debug!("Google OAuth2 token refreshed, expires at {}", new_expiry);

        Ok(())
    }
}

#[async_trait]
impl CalendarProvider for GoogleCalendarProvider {
    fn name(&self) -> &str {
        "Google Calendar"
    }

    fn provider_id(&self) -> &str {
        "google"
    }

    async fn get_events(
        &self,
        sync_token: Option<&str>,
    ) -> Result<(Vec<CalendarEvent>, Option<String>)> {
        self.ensure_token_fresh().await?;
        let client = self.client.read().await;
        client.get_events(sync_token).await
    }

    async fn put_event(&self, event: &CalendarEvent) -> Result<String> {
        self.ensure_token_fresh().await?;
        let client = self.client.read().await;
        client.put_event(event).await
    }

    async fn delete_event(&self, uid: &str) -> Result<()> {
        self.ensure_token_fresh().await?;
        let client = self.client.read().await;
        client.delete_event(uid).await
    }

    async fn test_connection(&self) -> Result<()> {
        self.ensure_token_fresh().await?;
        // Try fetching events to verify the connection works
        let client = self.client.read().await;
        let _ = client.get_events(None).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_google_provider_creation() {
        let provider = GoogleCalendarProvider::new(
            "client-id".to_string(),
            "client-secret".to_string(),
            "access-token".to_string(),
            "refresh-token".to_string(),
            "primary".to_string(),
            "UTC".to_string(),
        );

        assert_eq!(provider.name(), "Google Calendar");
        assert_eq!(provider.provider_id(), "google");
        assert_eq!(provider.calendar_id, "primary");
    }

    #[test]
    fn test_google_caldav_url_format() {
        let provider = GoogleCalendarProvider::new(
            "id".to_string(),
            "secret".to_string(),
            "token".to_string(),
            "refresh".to_string(),
            "user@gmail.com".to_string(),
            "UTC".to_string(),
        );

        // Verify the CalDAV URL uses the correct format
        // We can check via the client's calendar_url field
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = provider.client.read().await;
            assert!(client.calendar_url.contains("user@gmail.com"));
            assert!(client
                .calendar_url
                .starts_with("https://apidata.googleusercontent.com/caldav/v2/"));
        });
    }
}
