// CalDAV HTTP client for calendar synchronization (RFC 4791)

use crate::caldav::parser::{generate_vevent, parse_vevent};
use crate::types::CalendarEvent;
use common::{CalendarError, KlyntbotError, Result};
use reqwest::{header, Client, RequestBuilder, StatusCode};

/// Authentication method for CalDAV connections.
#[derive(Debug, Clone)]
pub enum CalDavAuth {
    /// HTTP Basic authentication (Apple Calendar, Nextcloud, Fastmail, etc.)
    Basic { username: String, password: String },
    /// HTTP Bearer token authentication (Google Calendar via OAuth2)
    Bearer { token: String },
}

/// CalDAV client for HTTP-based calendar operations
pub struct CalDavClient {
    pub(crate) calendar_url: String,
    auth: CalDavAuth,
    http_client: Client,
    /// IANA timezone name for generating iCalendar events (e.g., "Asia/Bangkok")
    timezone: String,
}

impl CalDavClient {
    /// Create a new CalDAV client with Basic auth (backward-compatible constructor).
    pub fn new(calendar_url: String, username: String, password: String, timezone: String) -> Self {
        Self {
            calendar_url,
            auth: CalDavAuth::Basic { username, password },
            http_client: Client::new(),
            timezone,
        }
    }

    /// Create a new CalDAV client with a specified auth method.
    pub fn new_with_auth(calendar_url: String, auth: CalDavAuth, timezone: String) -> Self {
        Self {
            calendar_url,
            auth,
            http_client: Client::new(),
            timezone,
        }
    }

    /// Apply the configured auth method to an outgoing request.
    fn apply_auth(&self, builder: RequestBuilder) -> RequestBuilder {
        match &self.auth {
            CalDavAuth::Basic { username, password } => {
                builder.basic_auth(username, Some(password))
            }
            CalDavAuth::Bearer { token } => builder.bearer_auth(token),
        }
    }

    /// Update the Bearer token (used after OAuth2 refresh).
    pub fn set_bearer_token(&mut self, token: String) {
        self.auth = CalDavAuth::Bearer { token };
    }

    /// Update the calendar URL (used after discovery)
    pub fn set_calendar_url(&mut self, calendar_url: String) {
        self.calendar_url = calendar_url;
    }

    /// Discover the actual calendar URL for iCloud (and other CalDAV servers)
    /// This implements the CalDAV discovery sequence:
    /// 1. PROPFIND to /.well-known/caldav → get principal URL
    /// 2. PROPFIND to principal URL → get calendar-home-set
    /// 3. PROPFIND to calendar-home-set (Depth:1) → list calendars
    /// 4. Find calendar by name or use first available
    pub async fn discover_calendar_url(
        base_url: &str,
        username: &str,
        password: &str,
        calendar_name: &str,
    ) -> Result<String> {
        let http_client = Client::new();

        // Step 1: Discover principal URL from well-known endpoint
        let well_known_url = format!("{}/.well-known/caldav", base_url.trim_end_matches('/'));
        tracing::debug!("CalDAV discovery: checking {}", well_known_url);

        let principal_url =
            match Self::discover_principal_url(&http_client, &well_known_url, username, password)
                .await
            {
                Ok(url) => url,
                Err(e) => {
                    return Err(e);
                }
            };

        tracing::debug!("CalDAV discovery: found principal URL: {}", principal_url);

        // Step 2: Get calendar-home-set from principal
        let calendar_home_url = match Self::discover_calendar_home(
            &http_client,
            &principal_url,
            username,
            password,
            base_url,
        )
        .await
        {
            Ok(url) => url,
            Err(e) => {
                return Err(e);
            }
        };

        tracing::debug!(
            "CalDAV discovery: found calendar-home-set: {}",
            calendar_home_url
        );

        // Step 3: List calendars and find the one we want
        let calendar_url = Self::find_calendar(
            &http_client,
            &calendar_home_url,
            username,
            password,
            calendar_name,
            base_url,
        )
        .await?;

        // Convert calendar URL to absolute if it's relative
        let full_calendar_url = if calendar_url.starts_with("http") {
            calendar_url
        } else {
            // Use the calendar_home_url host if it's absolute, otherwise use base_url
            if calendar_home_url.starts_with("http") {
                // Extract protocol://host:port from calendar_home_url
                let base_host = calendar_home_url
                    .split('/')
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("/");
                format!("{}{}", base_host, calendar_url)
            } else {
                let base_host = base_url
                    .trim_end_matches('/')
                    .split('/')
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("/");
                format!("{}{}", base_host, calendar_url)
            }
        };

        tracing::info!(
            "CalDAV discovery: selected calendar URL: {}",
            full_calendar_url
        );

        Ok(full_calendar_url)
    }

    /// Step 1: Discover principal URL from well-known endpoint
    async fn discover_principal_url(
        http_client: &Client,
        well_known_url: &str,
        username: &str,
        password: &str,
    ) -> Result<String> {
        let propfind_body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:current-user-principal/>
  </D:prop>
</D:propfind>"#;

        let response = http_client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
                well_known_url,
            )
            .basic_auth(username, Some(password))
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .header("Depth", "0")
            .body(propfind_body)
            .send()
            .await
            .map_err(|e| {
                KlyntbotError::Calendar(CalendarError::ConnectionFailed(format!(
                    "Failed to connect to well-known endpoint: {}",
                    e
                )))
            })?;

        match response.status() {
            StatusCode::MULTI_STATUS
            | StatusCode::OK
            | StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND => {
                let body = response.text().await.map_err(|e| {
                    KlyntbotError::Calendar(CalendarError::ProtocolError(e.to_string()))
                })?;
                Self::extract_href_from_xml(&body, "current-user-principal")
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(KlyntbotError::Calendar(CalendarError::AuthFailed(
                    "Authentication failed during principal discovery".to_string(),
                )))
            }
            status => Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                format!("Principal discovery failed with status {}", status),
            ))),
        }
    }

    /// Step 2: Get calendar-home-set from principal URL
    async fn discover_calendar_home(
        http_client: &Client,
        principal_url: &str,
        username: &str,
        password: &str,
        base_url: &str,
    ) -> Result<String> {
        let propfind_body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <C:calendar-home-set/>
  </D:prop>
</D:propfind>"#;

        // Convert relative principal_url to absolute URL
        let full_url = if principal_url.starts_with("http") {
            principal_url.to_string()
        } else {
            let base = base_url.trim_end_matches('/');
            let base_host = base.split('/').take(3).collect::<Vec<_>>().join("/");
            format!("{}{}", base_host, principal_url)
        };

        let response = http_client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &full_url)
            .basic_auth(username, Some(password))
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .header("Depth", "0")
            .body(propfind_body)
            .send()
            .await
            .map_err(|e| KlyntbotError::Calendar(CalendarError::ConnectionFailed(e.to_string())))?;

        match response.status() {
            StatusCode::MULTI_STATUS | StatusCode::OK => {
                let body = response.text().await.map_err(|e| {
                    KlyntbotError::Calendar(CalendarError::ProtocolError(e.to_string()))
                })?;
                Self::extract_href_from_xml(&body, "calendar-home-set")
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(KlyntbotError::Calendar(CalendarError::AuthFailed(
                    "Authentication failed during calendar-home discovery".to_string(),
                )))
            }
            status => Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                format!("Calendar-home discovery failed with status {}", status),
            ))),
        }
    }

    /// Step 3: Find a specific calendar by name (or use first available)
    async fn find_calendar(
        http_client: &Client,
        calendar_home_url: &str,
        username: &str,
        password: &str,
        calendar_name: &str,
        base_url: &str,
    ) -> Result<String> {
        let propfind_body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:displayname/>
    <D:resourcetype/>
  </D:prop>
</D:propfind>"#;

        // Convert relative calendar_home_url to absolute URL
        let full_url = if calendar_home_url.starts_with("http") {
            calendar_home_url.to_string()
        } else {
            let base = base_url.trim_end_matches('/');
            let base_host = base.split('/').take(3).collect::<Vec<_>>().join("/");
            format!("{}{}", base_host, calendar_home_url)
        };

        let response = http_client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &full_url)
            .basic_auth(username, Some(password))
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .header("Depth", "1")
            .body(propfind_body)
            .send()
            .await
            .map_err(|e| KlyntbotError::Calendar(CalendarError::ConnectionFailed(e.to_string())))?;

        match response.status() {
            StatusCode::MULTI_STATUS | StatusCode::OK => {
                let body = response.text().await.map_err(|e| {
                    KlyntbotError::Calendar(CalendarError::ProtocolError(e.to_string()))
                })?;
                Self::extract_calendar_from_list(&body, calendar_name)
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(KlyntbotError::Calendar(CalendarError::AuthFailed(
                    "Authentication failed during calendar listing".to_string(),
                )))
            }
            status => Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                format!("Calendar listing failed with status {}", status),
            ))),
        }
    }

    /// Extract href from XML response (for principal and calendar-home-set)
    fn extract_href_from_xml(xml: &str, element_name: &str) -> Result<String> {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut in_target_element = false;
        let mut in_href = false;
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = e.name();
                    let local_name = std::str::from_utf8(name.as_ref()).unwrap_or("");
                    if local_name.ends_with(element_name) {
                        in_target_element = true;
                    } else if in_target_element && local_name.ends_with("href") {
                        in_href = true;
                    }
                }
                Ok(Event::End(e)) => {
                    let name = e.name();
                    let local_name = std::str::from_utf8(name.as_ref()).unwrap_or("");
                    if local_name.ends_with(element_name) {
                        in_target_element = false;
                    } else if local_name.ends_with("href") {
                        in_href = false;
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_target_element && in_href {
                        if let Ok(text) = e.unescape() {
                            let href = text.trim();
                            if !href.is_empty() {
                                return Ok(href.to_string());
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                        format!("XML parse error during {}: {}", element_name, e),
                    )))
                }
                _ => {}
            }
            buf.clear();
        }

        // If we couldn't find it in the XML, look for href in the response element
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut in_href = false;
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = e.name();
                    let local_name = std::str::from_utf8(name.as_ref()).unwrap_or("");
                    if local_name.ends_with("href") {
                        in_href = true;
                    }
                }
                Ok(Event::End(e)) => {
                    let name = e.name();
                    let local_name = std::str::from_utf8(name.as_ref()).unwrap_or("");
                    if local_name.ends_with("href") {
                        in_href = false;
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_href {
                        if let Ok(text) = e.unescape() {
                            let href = text.trim();
                            if !href.is_empty() && href.starts_with('/') {
                                return Ok(href.to_string());
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                        format!("XML parse error: {}", e),
                    )))
                }
                _ => {}
            }
            buf.clear();
        }

        Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
            format!("Could not find {} in XML response", element_name),
        )))
    }

    /// Extract calendar URL from PROPFIND response listing calendars
    fn extract_calendar_from_list(xml: &str, calendar_name: &str) -> Result<String> {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut calendars = Vec::new();
        let mut current_href = None;
        let mut current_displayname = None;
        let mut is_calendar = false;
        let mut in_displayname = false;
        let mut in_resourcetype = false;
        let mut in_href = false;
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = e.name();
                    let local_name = std::str::from_utf8(name.as_ref()).unwrap_or("");

                    if local_name.ends_with("displayname") {
                        in_displayname = true;
                    } else if local_name.ends_with("resourcetype") {
                        in_resourcetype = true;
                    } else if local_name.ends_with("calendar") && in_resourcetype {
                        is_calendar = true;
                    } else if local_name.ends_with("href") && !in_resourcetype {
                        in_href = true;
                    }
                }
                Ok(Event::Empty(e)) => {
                    // Handle self-closing tags like <calendar/>
                    let name = e.name();
                    let local_name = std::str::from_utf8(name.as_ref()).unwrap_or("");

                    if local_name.ends_with("calendar") && in_resourcetype {
                        is_calendar = true;
                    }
                }
                Ok(Event::End(e)) => {
                    let name = e.name();
                    let local_name = std::str::from_utf8(name.as_ref()).unwrap_or("");

                    if local_name.ends_with("displayname") {
                        in_displayname = false;
                    } else if local_name.ends_with("resourcetype") {
                        in_resourcetype = false;
                    } else if local_name.ends_with("href") {
                        in_href = false;
                    } else if local_name.ends_with("response") {
                        // Save this calendar if it's actually a calendar
                        if is_calendar {
                            if let Some(href) = current_href.take() {
                                calendars.push((href, current_displayname.take()));
                            }
                        }
                        // Reset state
                        current_href = None;
                        current_displayname = None;
                        is_calendar = false;
                    }
                }
                Ok(Event::Text(e)) => {
                    if let Ok(text) = e.unescape() {
                        let text_str = text.trim();
                        if in_displayname {
                            current_displayname = Some(text_str.to_string());
                        } else if in_href && current_href.is_none() {
                            // This is the href content
                            current_href = Some(text_str.to_string());
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                        format!("XML parse error: {}", e),
                    )))
                }
                _ => {}
            }
            buf.clear();
        }

        // Try to find calendar by name
        for (href, displayname) in &calendars {
            if let Some(name) = displayname {
                if name.eq_ignore_ascii_case(calendar_name) {
                    tracing::debug!("Found calendar '{}' at {}", name, href);
                    return Ok(href.clone());
                }
            }
        }

        // Fallback: use first calendar
        if let Some((href, displayname)) = calendars.first() {
            tracing::warn!(
                "Calendar '{}' not found, using first available: {:?}",
                calendar_name,
                displayname
            );
            return Ok(href.clone());
        }

        Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
            "No calendars found".to_string(),
        )))
    }

    /// PUT an event to the CalDAV server, returns ETag
    pub async fn put_event(&self, event: &CalendarEvent) -> Result<String> {
        let base_url = self.calendar_url.trim_end_matches('/');
        let event_url = format!("{}/{}.ics", base_url, event.uid);
        let ical_data = generate_vevent(event, &self.timezone)?;

        let request = self
            .http_client
            .put(&event_url)
            .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
            .body(ical_data);
        let response =
            self.apply_auth(request).send().await.map_err(|e| {
                KlyntbotError::Calendar(CalendarError::ConnectionFailed(e.to_string()))
            })?;

        match response.status() {
            StatusCode::CREATED | StatusCode::NO_CONTENT | StatusCode::OK => {
                // Extract ETag from response headers
                let etag = response
                    .headers()
                    .get(header::ETAG)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("unknown")
                    .to_string();
                Ok(etag)
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(KlyntbotError::Calendar(
                CalendarError::AuthFailed("Authentication failed".to_string()),
            )),
            status => Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                format!("PUT failed with status {}", status),
            ))),
        }
    }

    /// DELETE an event from the CalDAV server
    pub async fn delete_event(&self, event_uid: &str) -> Result<()> {
        let base_url = self.calendar_url.trim_end_matches('/');
        let event_url = format!("{}/{}.ics", base_url, event_uid);

        let request = self.http_client.delete(&event_url);
        let response =
            self.apply_auth(request).send().await.map_err(|e| {
                KlyntbotError::Calendar(CalendarError::ConnectionFailed(e.to_string()))
            })?;

        match response.status() {
            StatusCode::NO_CONTENT | StatusCode::OK | StatusCode::NOT_FOUND => Ok(()),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(KlyntbotError::Calendar(
                CalendarError::AuthFailed("Authentication failed".to_string()),
            )),
            status => Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                format!("DELETE failed with status {}", status),
            ))),
        }
    }

    /// Fetch events from CalDAV server with optional sync token for incremental sync
    /// Returns (events, new_sync_token)
    pub async fn get_events(
        &self,
        sync_token: Option<&str>,
    ) -> Result<(Vec<CalendarEvent>, Option<String>)> {
        let report_body = self.build_report_body(sync_token);
        let calendar_url = self.calendar_url.trim_end_matches('/');

        let request = self
            .http_client
            .request(
                reqwest::Method::from_bytes(b"REPORT").unwrap(),
                calendar_url,
            )
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .header("Depth", "1")
            .body(report_body);
        let response =
            self.apply_auth(request).send().await.map_err(|e| {
                KlyntbotError::Calendar(CalendarError::ConnectionFailed(e.to_string()))
            })?;

        match response.status() {
            StatusCode::MULTI_STATUS | StatusCode::OK => {
                let body = response.text().await.map_err(|e| {
                    KlyntbotError::Calendar(CalendarError::ProtocolError(e.to_string()))
                })?;
                self.parse_report_response(&body)
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(KlyntbotError::Calendar(
                CalendarError::AuthFailed("Authentication failed".to_string()),
            )),
            status => Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                format!("REPORT failed with status {}", status),
            ))),
        }
    }

    /// Build calendar-query REPORT XML body with optional sync token
    fn build_report_body(&self, sync_token: Option<&str>) -> String {
        if let Some(token) = sync_token {
            // Incremental sync using sync-collection (RFC 6578)
            format!(
                r#"<?xml version="1.0" encoding="utf-8" ?>
<D:sync-collection xmlns:D="DAV:">
  <D:sync-token>{}</D:sync-token>
  <D:sync-level>1</D:sync-level>
  <D:prop>
    <D:getetag/>
    <C:calendar-data xmlns:C="urn:ietf:params:xml:ns:caldav"/>
  </D:prop>
</D:sync-collection>"#,
                token
            )
        } else {
            // Full sync - fetch all events
            r#"<?xml version="1.0" encoding="utf-8" ?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VEVENT"/>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>"#
                .to_string()
        }
    }

    /// Parse XML REPORT response to extract events and sync token
    fn parse_report_response(&self, xml: &str) -> Result<(Vec<CalendarEvent>, Option<String>)> {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut events = Vec::new();
        let mut current_ical_data = String::new();
        let mut current_etag = None;
        let mut sync_token = None;
        let mut in_calendar_data = false;
        let mut in_sync_token = false;

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = e.name();
                    let local_name = std::str::from_utf8(name.as_ref()).unwrap_or("");

                    if local_name.ends_with("calendar-data") {
                        in_calendar_data = true;
                        current_ical_data.clear();
                    } else if local_name.ends_with("sync-token") {
                        in_sync_token = true;
                    }
                }
                Ok(Event::End(e)) => {
                    let name = e.name();
                    let local_name = std::str::from_utf8(name.as_ref()).unwrap_or("");

                    if local_name.ends_with("sync-token") {
                        in_sync_token = false;
                    } else if local_name.ends_with("calendar-data") {
                        in_calendar_data = false;
                        // Parse the accumulated iCalendar data
                        if !current_ical_data.trim().is_empty() {
                            match parse_vevent(&current_ical_data) {
                                Ok(mut event) => {
                                    event.etag = current_etag.clone();
                                    events.push(event);
                                }
                                Err(e) => {
                                    // Log parse errors but continue processing other events
                                    tracing::warn!("Failed to parse calendar event: {}", e);
                                }
                            }
                            current_ical_data.clear();
                        }
                    } else if local_name.ends_with("response") {
                        // Reset per-response state
                        current_etag = None;
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_calendar_data {
                        if let Ok(text) = e.unescape() {
                            current_ical_data.push_str(&text);
                        }
                    } else if in_sync_token {
                        // Inside sync-token element
                        if let Ok(text) = e.unescape() {
                            let text_str = text.trim();
                            if !text_str.is_empty() {
                                sync_token = Some(text_str.to_string());
                            }
                        }
                    } else {
                        // Check for etag
                        if let Ok(text) = e.unescape() {
                            let text_str = text.trim();
                            // Simple heuristic: if it looks like an etag (quoted string)
                            if text_str.starts_with('"') && text_str.ends_with('"') {
                                current_etag = Some(text_str.to_string());
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(KlyntbotError::Calendar(CalendarError::ProtocolError(
                        format!("XML parse error: {}", e),
                    )))
                }
                _ => {}
            }
            buf.clear();
        }

        Ok((events, sync_token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_caldav_client_creation() {
        let client = CalDavClient::new(
            "https://caldav.example.com/calendar".to_string(),
            "user@example.com".to_string(),
            "password123".to_string(),
            "UTC".to_string(),
        );

        assert_eq!(client.calendar_url, "https://caldav.example.com/calendar");
        match &client.auth {
            CalDavAuth::Basic { username, password } => {
                assert_eq!(username, "user@example.com");
                assert_eq!(password, "password123");
            }
            _ => panic!("Expected Basic auth"),
        }
    }

    #[test]
    fn test_build_report_body_full_sync() {
        let client = CalDavClient::new(
            "https://caldav.example.com/calendar".to_string(),
            "user".to_string(),
            "pass".to_string(),
            "UTC".to_string(),
        );

        let body = client.build_report_body(None);
        assert!(body.contains("calendar-query"));
        assert!(body.contains("VEVENT"));
        assert!(!body.contains("sync-token"));
    }

    #[test]
    fn test_build_report_body_incremental_sync() {
        let client = CalDavClient::new(
            "https://caldav.example.com/calendar".to_string(),
            "user".to_string(),
            "pass".to_string(),
            "UTC".to_string(),
        );

        let body = client.build_report_body(Some("token-abc-123"));
        assert!(body.contains("sync-collection"));
        assert!(body.contains("token-abc-123"));
    }

    #[test]
    fn test_parse_report_response_simple() {
        let client = CalDavClient::new(
            "https://caldav.example.com/calendar".to_string(),
            "user".to_string(),
            "pass".to_string(),
            "UTC".to_string(),
        );

        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/calendar/event1.ics</D:href>
    <D:propstat>
      <D:prop>
        <D:getetag>"etag-123"</D:getetag>
        <C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Test//Test//EN
BEGIN:VEVENT
UID:event-1
SUMMARY:Test Event
DTSTART:20260301T100000Z
DTEND:20260301T110000Z
END:VEVENT
END:VCALENDAR</C:calendar-data>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>"#;

        let result = client.parse_report_response(xml);
        assert!(result.is_ok());

        let (events, _sync_token) = result.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].uid, "event-1");
        assert_eq!(events[0].summary, "Test Event");
        assert_eq!(events[0].etag, Some("\"etag-123\"".to_string()));
    }

    #[test]
    fn test_parse_report_response_empty() {
        let client = CalDavClient::new(
            "https://caldav.example.com/calendar".to_string(),
            "user".to_string(),
            "pass".to_string(),
            "UTC".to_string(),
        );

        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
</D:multistatus>"#;

        let result = client.parse_report_response(xml);
        assert!(result.is_ok());

        let (events, _sync_token) = result.unwrap();
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_parse_report_response_malformed_event() {
        let client = CalDavClient::new(
            "https://caldav.example.com/calendar".to_string(),
            "user".to_string(),
            "pass".to_string(),
            "UTC".to_string(),
        );

        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/calendar/bad.ics</D:href>
    <D:propstat>
      <D:prop>
        <C:calendar-data>INVALID ICAL DATA</C:calendar-data>
      </D:prop>
    </D:propstat>
  </D:response>
</D:multistatus>"#;

        let result = client.parse_report_response(xml);
        // Should succeed but skip the malformed event
        assert!(result.is_ok());
        let (events, _) = result.unwrap();
        assert_eq!(events.len(), 0);
    }
}
