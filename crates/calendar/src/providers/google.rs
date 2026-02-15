// Google Calendar provider — REST API v3 with OAuth2
//
// Uses Google Calendar REST API instead of CalDAV because Google restricts
// CalDAV access (returns 403) unless the CalDAV API is explicitly enabled
// in the Cloud Console. The REST API works out of the box with the same
// OAuth2 token and calendar scope.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::{CalendarError, KlyntbotError, Result};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::provider::CalendarProvider;
use crate::types::{CalendarEvent, EventSource};

const BASE_URL: &str = "https://www.googleapis.com/calendar/v3";

/// Google Calendar provider using the REST API v3 with OAuth2 Bearer auth.
///
/// Tokens are refreshed automatically when expired (5-minute buffer).
pub struct GoogleCalendarProvider {
    http_client: reqwest::Client,
    client_id: String,
    client_secret: String,
    refresh_token: String,
    access_token: RwLock<String>,
    token_expiry: RwLock<Option<DateTime<Utc>>>,
    /// The calendar ID (usually "primary" or an email address).
    pub calendar_id: String,
}

impl GoogleCalendarProvider {
    pub fn new(
        client_id: String,
        client_secret: String,
        access_token: String,
        refresh_token: String,
        calendar_id: String,
        _timezone: String,
    ) -> Self {
        Self {
            http_client: reqwest::Client::new(),
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

        let form_body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &self.client_id)
            .append_pair("client_secret", &self.client_secret)
            .append_pair("refresh_token", &self.refresh_token)
            .append_pair("grant_type", "refresh_token")
            .finish();

        let resp = self
            .http_client
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
            return Err(KlyntbotError::Calendar(CalendarError::AuthFailed(format!(
                "Google token refresh failed ({}): {}",
                status, body
            ))));
        }

        let json: Value = resp.json().await.map_err(|e| {
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

        {
            let mut token = self.access_token.write().await;
            *token = new_token;
        }
        {
            let mut expiry = self.token_expiry.write().await;
            *expiry = Some(new_expiry);
        }

        tracing::debug!("Google OAuth2 token refreshed, expires at {}", new_expiry);
        Ok(())
    }

    /// Get the current access token.
    async fn token(&self) -> String {
        self.access_token.read().await.clone()
    }

    /// Build the events endpoint URL.
    fn events_url(&self) -> String {
        format!(
            "{}/calendars/{}/events",
            BASE_URL,
            urlencoding::encode(&self.calendar_id)
        )
    }

    /// Convert a Google Calendar REST API event JSON to a CalendarEvent.
    fn json_to_event(item: &Value) -> Option<CalendarEvent> {
        let uid = item["iCalUID"].as_str().or(item["id"].as_str())?;
        let summary = item["summary"].as_str().unwrap_or("(no title)");
        let description = item["description"].as_str().map(String::from);

        // Google uses dateTime for timed events, date for all-day events
        let start = item["start"]["dateTime"]
            .as_str()
            .or(item["start"]["date"].as_str())?;
        let end = item["end"]["dateTime"]
            .as_str()
            .or(item["end"]["date"].as_str())?;

        let start_dt = DateTime::parse_from_rfc3339(start)
            .ok()?
            .with_timezone(&Utc);
        let end_dt = DateTime::parse_from_rfc3339(end).ok()?.with_timezone(&Utc);

        let etag = item["etag"].as_str().map(String::from);

        Some(CalendarEvent {
            uid: uid.to_string(),
            summary: summary.to_string(),
            description,
            start: start_dt,
            end: end_dt,
            source: EventSource::CalDAV, // Reuse existing variant for remote events
            etag,
        })
    }

    /// Convert a CalendarEvent to Google Calendar REST API JSON body.
    fn event_to_json(event: &CalendarEvent) -> Value {
        let mut body = serde_json::json!({
            "summary": event.summary,
            "start": {
                "dateTime": event.start.to_rfc3339(),
            },
            "end": {
                "dateTime": event.end.to_rfc3339(),
            },
            "iCalUID": event.uid,
        });

        if let Some(ref desc) = event.description {
            body["description"] = Value::String(desc.clone());
        }

        body
    }

    /// Extract a readable error message from a Google API error response.
    fn extract_api_error(body: &str) -> String {
        if let Ok(json) = serde_json::from_str::<Value>(body) {
            if let Some(msg) = json["error"]["message"].as_str() {
                return msg.to_string();
            }
        }
        body.chars().take(200).collect()
    }

    /// Find a Google event ID by iCalUID (needed for update/delete).
    async fn find_event_id_by_uid(&self, uid: &str) -> Result<Option<String>> {
        let token = self.token().await;
        let url = format!(
            "{}?iCalUID={}&maxResults=1",
            self.events_url(),
            urlencoding::encode(uid)
        );

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| KlyntbotError::Calendar(CalendarError::ConnectionFailed(e.to_string())))?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| KlyntbotError::Calendar(CalendarError::ProtocolError(e.to_string())))?;

        Ok(json["items"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item["id"].as_str())
            .map(String::from))
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
        let token = self.token().await;

        let url = if let Some(st) = sync_token {
            format!(
                "{}?syncToken={}",
                self.events_url(),
                urlencoding::encode(st)
            )
        } else {
            // Full sync — get events from last 30 days forward
            let time_min = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
            format!(
                "{}?singleEvents=true&timeMin={}&maxResults=250",
                self.events_url(),
                urlencoding::encode(&time_min)
            )
        };

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| KlyntbotError::Calendar(CalendarError::ConnectionFailed(e.to_string())))?;

        let status = resp.status().as_u16();
        match status {
            200 => {}
            401 | 403 => {
                let body = resp.text().await.unwrap_or_default();
                return Err(KlyntbotError::Calendar(CalendarError::AuthFailed(format!(
                    "Google Calendar API error ({}): {}",
                    status,
                    Self::extract_api_error(&body)
                ))));
            }
            410 => {
                // Gone — sync token expired, need full sync
                tracing::warn!("Google sync token expired, performing full sync");
                return self.get_events(None).await;
            }
            status => {
                let body = resp.text().await.unwrap_or_default();
                return Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                    format!("Google Calendar API error ({}): {}", status, body),
                )));
            }
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| KlyntbotError::Calendar(CalendarError::ProtocolError(e.to_string())))?;

        let events: Vec<CalendarEvent> = json["items"]
            .as_array()
            .map(|items| items.iter().filter_map(Self::json_to_event).collect())
            .unwrap_or_default();

        let new_sync_token = json["nextSyncToken"].as_str().map(String::from);

        Ok((events, new_sync_token))
    }

    async fn put_event(&self, event: &CalendarEvent) -> Result<String> {
        self.ensure_token_fresh().await?;
        let token = self.token().await;
        let body = Self::event_to_json(event);

        // Check if event already exists by iCalUID
        if let Some(event_id) = self.find_event_id_by_uid(&event.uid).await? {
            // Update existing event
            let url = format!("{}/{}", self.events_url(), urlencoding::encode(&event_id));
            let resp = self
                .http_client
                .put(&url)
                .bearer_auth(&token)
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    KlyntbotError::Calendar(CalendarError::ConnectionFailed(e.to_string()))
                })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let err_body = resp.text().await.unwrap_or_default();
                return Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                    format!("Google Calendar update failed ({}): {}", status, err_body),
                )));
            }

            let result: Value = resp.json().await.unwrap_or_default();
            Ok(result["etag"].as_str().unwrap_or("updated").to_string())
        } else {
            // Import new event (preserves iCalUID)
            let url = format!("{}/import", self.events_url());
            let resp = self
                .http_client
                .post(&url)
                .bearer_auth(&token)
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    KlyntbotError::Calendar(CalendarError::ConnectionFailed(e.to_string()))
                })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let err_body = resp.text().await.unwrap_or_default();
                return Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                    format!("Google Calendar import failed ({}): {}", status, err_body),
                )));
            }

            let result: Value = resp.json().await.unwrap_or_default();
            Ok(result["etag"].as_str().unwrap_or("created").to_string())
        }
    }

    async fn delete_event(&self, uid: &str) -> Result<()> {
        self.ensure_token_fresh().await?;
        let token = self.token().await;

        // Find the Google event ID from the iCalUID
        let event_id = match self.find_event_id_by_uid(uid).await? {
            Some(id) => id,
            None => return Ok(()), // Already deleted or never existed
        };

        let url = format!("{}/{}", self.events_url(), urlencoding::encode(&event_id));
        let resp = self
            .http_client
            .delete(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| KlyntbotError::Calendar(CalendarError::ConnectionFailed(e.to_string())))?;

        match resp.status().as_u16() {
            204 | 200 | 404 | 410 => Ok(()),
            401 | 403 => Err(KlyntbotError::Calendar(CalendarError::AuthFailed(
                "Authentication failed".to_string(),
            ))),
            status => {
                let body = resp.text().await.unwrap_or_default();
                Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                    format!("Google Calendar delete failed ({}): {}", status, body),
                )))
            }
        }
    }

    async fn test_connection(&self) -> Result<()> {
        self.ensure_token_fresh().await?;
        let token = self.token().await;

        let url = format!("{}?maxResults=1", self.events_url());
        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| KlyntbotError::Calendar(CalendarError::ConnectionFailed(e.to_string())))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            Err(KlyntbotError::Calendar(CalendarError::AuthFailed(format!(
                "Google Calendar connection test failed ({}): {}",
                status, body
            ))))
        }
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
    fn test_events_url_primary() {
        let provider = GoogleCalendarProvider::new(
            "id".to_string(),
            "secret".to_string(),
            "token".to_string(),
            "refresh".to_string(),
            "primary".to_string(),
            "UTC".to_string(),
        );

        assert_eq!(
            provider.events_url(),
            "https://www.googleapis.com/calendar/v3/calendars/primary/events"
        );
    }

    #[test]
    fn test_events_url_email() {
        let provider = GoogleCalendarProvider::new(
            "id".to_string(),
            "secret".to_string(),
            "token".to_string(),
            "refresh".to_string(),
            "user@gmail.com".to_string(),
            "UTC".to_string(),
        );

        assert!(provider.events_url().contains("user%40gmail.com"));
    }

    #[test]
    fn test_event_to_json() {
        let event = CalendarEvent {
            uid: "test-uid".to_string(),
            summary: "Test Event".to_string(),
            description: Some("A description".to_string()),
            start: Utc::now(),
            end: Utc::now() + chrono::Duration::hours(1),
            source: EventSource::TodoItem,
            etag: None,
        };

        let json = GoogleCalendarProvider::event_to_json(&event);
        assert_eq!(json["summary"], "Test Event");
        assert_eq!(json["description"], "A description");
        assert_eq!(json["iCalUID"], "test-uid");
        assert!(json["start"]["dateTime"].is_string());
        assert!(json["end"]["dateTime"].is_string());
    }

    #[test]
    fn test_json_to_event() {
        let json = serde_json::json!({
            "id": "google-id-123",
            "iCalUID": "ical-uid-456",
            "summary": "Parsed Event",
            "description": "Some description",
            "start": { "dateTime": "2026-03-01T10:00:00Z" },
            "end": { "dateTime": "2026-03-01T11:00:00Z" },
            "etag": "\"etag-789\""
        });

        let event = GoogleCalendarProvider::json_to_event(&json).unwrap();
        assert_eq!(event.uid, "ical-uid-456");
        assert_eq!(event.summary, "Parsed Event");
        assert_eq!(event.description, Some("Some description".to_string()));
        assert_eq!(event.etag, Some("\"etag-789\"".to_string()));
    }

    #[test]
    fn test_json_to_event_missing_times() {
        let json = serde_json::json!({
            "id": "no-times",
            "summary": "Missing times"
        });

        assert!(GoogleCalendarProvider::json_to_event(&json).is_none());
    }

    #[test]
    fn test_event_to_json_no_description() {
        let event = CalendarEvent {
            uid: "uid".to_string(),
            summary: "No Desc".to_string(),
            description: None,
            start: Utc::now(),
            end: Utc::now() + chrono::Duration::hours(1),
            source: EventSource::TodoItem,
            etag: None,
        };

        let json = GoogleCalendarProvider::event_to_json(&event);
        assert!(json.get("description").is_none());
    }
}
