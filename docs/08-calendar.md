# Calendar

## Purpose

The `calendar` crate (Layer 2) provides calendar synchronization with external CalDAV and REST API calendar services. It defines the `CalendarProvider` trait with three implementations (Apple, Google, Generic CalDAV), a CalDAV HTTP client, iCalendar VEVENT parsing and generation, and a sync engine with configurable conflict resolution. Calendar sync runs in the background on a cron schedule and can also be triggered on demand via `CalendarTool::sync_now()`.

## Key Types

### Traits

**`CalendarProvider`** -- async trait implemented by all calendar backends. Methods:

| Method | Signature | Purpose |
|--------|-----------|---------|
| `name()` | `-> &str` | Human-readable name (e.g., `"Apple Calendar"`). |
| `provider_id()` | `-> &str` | Stable identifier for sync state persistence (e.g., `"apple"`, `"google"`, `"generic-nextcloud"`). |
| `get_events()` | `(sync_token: Option<&str>) -> Result<(Vec<CalendarEvent>, Option<String>)>` | Fetch events, optionally using incremental sync. Returns events and a new sync token. |
| `put_event()` | `(event: &CalendarEvent) -> Result<String>` | Create or update an event. Returns the ETag. |
| `delete_event()` | `(uid: &str) -> Result<()>` | Delete an event by UID. |
| `test_connection()` | `-> Result<()>` | Verify connectivity and authentication. |

### Structs

**`CalendarEvent`** -- the unified event representation used across all providers:

| Field | Type | Purpose |
|-------|------|---------|
| `uid` | `String` | iCalendar UID, unique across calendars. |
| `summary` | `String` | Event title. |
| `description` | `Option<String>` | Detailed description. |
| `start` | `DateTime<Utc>` | Start time, always stored in UTC internally. |
| `end` | `DateTime<Utc>` | End time, always stored in UTC internally. |
| `source` | `EventSource` | Where the event came from (`CalDAV` or `TodoItem`). |
| `etag` | `Option<String>` | CalDAV ETag for change detection and conflict resolution. |
| `status` | `Option<String>` | iCal status string: `CONFIRMED`, `CANCELLED`, `TENTATIVE`, or `COMPLETED`. |

**`SyncState`** -- persisted per-provider sync metadata:

| Field | Type | Purpose |
|-------|------|---------|
| `sync_token` | `Option<String>` | Opaque token from the server for incremental sync (RFC 6578). |
| `last_sync` | `Option<DateTime<Utc>>` | Timestamp of the last successful sync. |

**`CalDavClient`** -- HTTP client for CalDAV protocol operations. Holds a `calendar_url`, auth credentials, HTTP client, and timezone string.

**`CalDavAuth`** -- enum with two variants:
- `Basic { username, password }` -- for Apple Calendar, Nextcloud, Fastmail, Radicale, etc.
- `Bearer { token }` -- for Google Calendar OAuth2.

**`AppleCalendarProvider`** -- CalDAV provider for iCloud. Wraps a `CalDavClient` behind `RwLock` and performs lazy CalDAV discovery on first use.

**`GoogleCalendarProvider`** -- REST API provider for Google Calendar v3. Uses OAuth2 with automatic token refresh (5-minute buffer before expiry). Does not use CalDAV because Google restricts CalDAV access unless explicitly enabled in the Cloud Console.

**`GenericCalDavProvider`** -- CalDAV provider for any standards-compliant server (Nextcloud, Fastmail, Zoho, Radicale, etc.). Attempts CalDAV discovery if the URL looks like a base URL; falls back to using the URL as-is if discovery fails.

### Enums

**`EventSource`** -- `CalDAV` (fetched from a remote calendar) or `TodoItem` (converted from a Klyntbot task).

**`ConflictResolutionStrategy`** -- controls how conflicting versions of the same event are resolved:

| Variant | Behavior |
|---------|----------|
| `ServerWins` (default) | Always keep the server version. Safest option. |
| `ClientWins` | Always keep the local version. |
| `LastWriteWins` | Compare ETags lexicographically as a recency proxy. Falls back to `ServerWins` when ordering is ambiguous (e.g., both ETags are absent). |
| `Manual` | Return the server version as a safe placeholder and flag the conflict for user resolution. |

**`CalendarError`** -- domain error type with variants `AuthFailed`, `ConnectionFailed`, `SyncFailed`, `NotFound`, `ProtocolError`, `Io`, `Json`. Converts into `KlyntbotError::Calendar(String)`.

### Free Functions

| Function | Purpose |
|----------|---------|
| `parse_vevent(ical_data)` | Parse a VCALENDAR/VEVENT string into a `CalendarEvent`. Handles `DTSTART`/`DTEND` with optional `TZID` parameters and UTC `Z` suffix. |
| `generate_vevent(event, timezone)` | Generate a complete VCALENDAR string from a `CalendarEvent`, including a `VTIMEZONE` component for non-UTC timezones with correct STANDARD/DAYLIGHT subcomponents. |
| `detect_conflict(server, local)` | Returns `true` if two events share the same UID but differ in summary, description, start, end, etag, or status. |
| `resolve_conflict(server, local, strategy)` | Returns the winning event according to the chosen `ConflictResolutionStrategy`. |
| `load_provider_sync_state(repo, provider_id)` | Load `SyncState` from SQLite via `CalendarSyncRepo`. Returns empty state if not found. |
| `save_provider_sync_state(repo, provider_id, state)` | Upsert `SyncState` to SQLite. |

## How It Works

### CalDAV Client

The `CalDavClient` implements the CalDAV protocol (RFC 4791) over HTTP using `reqwest`. It supports two auth methods (`Basic` and `Bearer`) applied uniformly to all requests via `apply_auth()`.

**Discovery sequence** (used by Apple and Generic providers when given a base URL):

1. `PROPFIND /.well-known/caldav` with `Depth: 0` -- extracts `current-user-principal` href from the XML response.
2. `PROPFIND {principal_url}` with `Depth: 0` -- extracts `calendar-home-set` href.
3. `PROPFIND {calendar_home_url}` with `Depth: 1` -- lists all calendars. Finds the target by `displayname` match, or falls back to the first available calendar.

**Event operations:**

- `get_events(sync_token)` -- sends a CalDAV `REPORT` request. Without a sync token, it sends a `calendar-query` filtering for VEVENT components. With a sync token, it sends a `sync-collection` request (RFC 6578) for incremental sync. Parses the multi-status XML response, extracting ETags and iCalendar data, then converts each VEVENT to a `CalendarEvent` via `parse_vevent()`.
- `put_event(event)` -- `PUT {calendar_url}/{uid}.ics` with the generated iCalendar body. Returns the ETag from the response headers.
- `delete_event(uid)` -- `DELETE {calendar_url}/{uid}.ics`. Treats 404 as success (already deleted).

### iCalendar Parsing and Generation

**Parsing** (`parse_vevent`): Reads iCalendar text line-by-line, extracting UID, SUMMARY, DESCRIPTION, DTSTART, DTEND, and STATUS fields from within the VEVENT block. Datetime parsing handles three formats:
- UTC times with Z suffix (e.g., `20260301T100000Z`)
- Timezone-qualified times with TZID parameter (e.g., `DTSTART;TZID=Asia/Bangkok:20260301T090000`)
- Floating times without timezone (treated as UTC)

All datetimes are converted to `DateTime<Utc>` for internal storage.

**Generation** (`generate_vevent`): Produces a complete VCALENDAR document. For non-UTC timezones, it:
1. Generates a VTIMEZONE component with STANDARD and DAYLIGHT subcomponents based on the timezone's UTC offsets at January 1 and July 1 of the event's year.
2. Formats DTSTART/DTEND with `TZID=` parameters and local times.
For UTC, it uses the simpler `Z` suffix format without a VTIMEZONE block.

### Provider Implementations

**AppleCalendarProvider**: Wraps a `CalDavClient` with lazy discovery. On first `get_events()`, `put_event()`, or `delete_event()` call, it checks whether the URL needs discovery (base iCloud URL, well-known endpoint, or missing `/calendars/` path). If so, it runs the full CalDAV discovery sequence and updates the client's calendar URL. The `discovered` flag (behind `RwLock`) prevents repeated discovery.

**GoogleCalendarProvider**: Uses the Google Calendar REST API v3 (`https://www.googleapis.com/calendar/v3`) instead of CalDAV. OAuth2 token management:
- Tokens are refreshed automatically when expired or within 5 minutes of expiry.
- Refresh calls `POST https://oauth2.googleapis.com/token` with `client_id`, `client_secret`, `refresh_token`, and `grant_type=refresh_token`.
- The new access token and expiry are stored behind `RwLock`.

Event conversion handles the differences between Google's JSON format and Klyntbot's `CalendarEvent`: Google uses `iCalUID` (mapped to `uid`), RFC 3339 datetime strings (vs. iCalendar format), and lowercase status values (vs. iCalendar uppercase). The provider uses the `import` endpoint for new events (which preserves the iCalUID) and standard `PUT` for updates, looking up the Google-internal event ID by iCalUID first.

Incremental sync uses Google's `syncToken` parameter. When a sync token expires (HTTP 410 Gone), the provider automatically falls back to a full sync.

**GenericCalDavProvider**: Works with any CalDAV-compliant server. Attempts discovery if the URL does not look like a specific calendar path (no `/calendars/`, `/events/`, or `.ics` suffix). If discovery fails, it proceeds with the URL as-is. The `provider_id` is generated from the label (e.g., `"Nextcloud"` becomes `"generic-nextcloud"`).

### Sync Engine (Conflict Detection and Resolution)

The sync engine provides two pure functions used by the sync adapter in the agent layer:

**`detect_conflict(server_event, local_event)`** -- returns `true` when both events share the same UID but differ in any of: summary, description, start, end, etag, or status. Events with different UIDs never conflict.

**`resolve_conflict(server_event, local_event, strategy)`** -- returns the winning event:
- `ServerWins`: always returns the server copy.
- `ClientWins`: always returns the local copy.
- `LastWriteWins`: compares ETags lexicographically. If the local ETag is greater, local wins; if only the local has an ETag, local wins (it was previously confirmed by the server); otherwise server wins.
- `Manual`: returns the server copy as a safe placeholder. Callers should check the strategy beforehand and surface the conflict to the user.

### Sync State Persistence

Sync state (sync token + last sync timestamp) is persisted per provider via `CalendarSyncRepo` in the `storage` crate. The `load_provider_sync_state()` and `save_provider_sync_state()` functions map between the `SyncState` domain type and the SQL row type.

### How Calendar Sync Runs

Calendar sync is triggered in three ways:

1. **Cron-scheduled** -- the `calendar_sync` system job in the scheduling crate fires periodically, publishing a bus message that the agent processes as a calendar sync request.
2. **On-demand** -- `CalendarTool::sync_now()` triggers an immediate sync via the `CalendarHandler` trait (defined in `tools`, implemented in `agent`).
3. **Background** -- the sync adapter in the agent layer runs each provider's `get_events()` with the stored sync token, processes conflicts via the sync engine, updates local state, and saves the new sync token.

## Connections

**Depends on:**
- `common` (Layer 0) -- `Result`, `KlyntbotError`, `ToolError`
- `storage` (Layer 1.5) -- `CalendarSyncRepo` for sync state persistence
- External crates: `reqwest` (HTTP client), `quick_xml` (XML parsing for CalDAV responses), `chrono` / `chrono_tz` (datetime and timezone handling), `url` / `urlencoding` (URL manipulation for Google API)

**Depended on by:**
- `agent` (Layer 5) -- creates calendar providers from config, runs the sync adapter, implements the `CalendarHandler` trait
- `tools` (Layer 3) -- `CalendarHandler` trait (defined in tools, implemented in agent) bridges the calendar operations to the tool layer so `CalendarTool` can list events, sync, and manage calendar state without directly depending on the agent
