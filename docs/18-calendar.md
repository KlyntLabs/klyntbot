# Calendar Crate

Crate path: `crates/calendar/`

Layer 2 in the workspace dependency graph. Depends on `common` (Layer 0), `config` (Layer 1), and `storage` (Layer 1.5).

---

## Section 1: Narrative Overview

### What This Crate Does

The `calendar` crate provides CalDAV-based calendar synchronization for Klyntbot. It implements a multi-provider calendar abstraction that lets the agent read, create, update, and delete calendar events across Apple Calendar (iCloud), Google Calendar, and any generic CalDAV server (Nextcloud, Fastmail, Radicale, Zoho, etc.).

The crate is purely a library -- it exposes types, a CalDAV HTTP client, provider implementations, sync state persistence, and conflict resolution utilities. Higher layers (the `agent` crate) drive the actual sync scheduling.

### CalDAV Client Design

`CalDavClient` (`crates/calendar/src/caldav/client.rs`) is the low-level HTTP transport for CalDAV operations conforming to RFC 4791 (CalDAV) and RFC 6578 (WebDAV Sync). It wraps `reqwest::Client` and provides:

1. **Authentication**: Supports both HTTP Basic auth (Apple, Nextcloud, Fastmail) and Bearer token auth (Google OAuth2) via the `CalDavAuth` enum. The `apply_auth` method decorates every outgoing request with the configured credentials.

2. **CalDAV discovery** (lines 74-160 of `client.rs`): A three-step discovery sequence for finding the actual calendar URL from a base server URL:
   - Step 1: `PROPFIND` to `/.well-known/caldav` to discover the `current-user-principal` URL.
   - Step 2: `PROPFIND` to the principal URL to discover the `calendar-home-set`.
   - Step 3: `PROPFIND` with `Depth: 1` on the calendar-home-set to enumerate available calendars and select one by display name, falling back to the first available calendar.

3. **Event operations**:
   - `get_events(sync_token)`: Sends a CalDAV `REPORT` request. With no sync token, issues a full `calendar-query` filtered to `VEVENT` components. With a sync token, issues an RFC 6578 `sync-collection` request for incremental sync. Returns `(Vec<CalendarEvent>, Option<String>)` -- events and a new sync token.
   - `put_event(event)`: HTTP `PUT` of a generated iCalendar `.ics` resource to `{calendar_url}/{uid}.ics`. Returns the server-assigned ETag.
   - `delete_event(uid)`: HTTP `DELETE` of `{calendar_url}/{uid}.ics`. Treats 404 as success (idempotent).

4. **XML parsing** (`client.rs` lines 328-539): All CalDAV responses are WebDAV Multi-Status XML. The client uses `quick-xml` with a streaming event-based parser to extract `href`, `displayname`, `resourcetype`, `calendar-data`, `getetag`, and `sync-token` elements. Namespace prefixes are handled by matching on local name suffixes (e.g., `ends_with("calendar-data")`).

### iCalendar Parser and Generator

`crates/calendar/src/caldav/parser.rs` implements a minimal RFC 5545 iCalendar subset:

- **`parse_vevent(ical_data)`**: Line-by-line parser that extracts `UID`, `SUMMARY`, `DESCRIPTION`, `DTSTART`, `DTEND`, and `STATUS` from a VEVENT block. Handles TZID parameters on datetime properties (e.g., `DTSTART;TZID=Asia/Bangkok:20260215T140000`) by converting to UTC via `chrono-tz`. Also handles UTC `Z` suffix and floating time formats. Required fields: UID, SUMMARY, DTSTART, DTEND.

- **`generate_vevent(event, timezone)`**: Produces a complete VCALENDAR document with optional VTIMEZONE component. For non-UTC timezones, datetimes are formatted with `DTSTART;TZID=...` notation. For UTC, uses the `Z` suffix. The VTIMEZONE component includes STANDARD and DAYLIGHT subcomponents computed from the timezone's offset rules.

- **`generate_vtimezone(tzid, year)`** (internal): Computes UTC offsets for January 1 and July 1 of the given year to detect whether the timezone observes DST. Produces a single STANDARD component for non-DST timezones, or both STANDARD and DAYLIGHT components for DST timezones.

### Sync Engine Architecture

The sync engine (`crates/calendar/src/sync_engine.rs`) provides two stateless functions for conflict handling rather than managing full sync orchestration:

- **`detect_conflict(server_event, local_event)`**: Returns `true` if two events share the same UID but differ in any content field (summary, description, start, end, etag, or status).

- **`resolve_conflict(server_event, local_event, strategy)`**: Applies the configured `ConflictResolutionStrategy` to produce a single winning event. The strategy determines which version survives:
  - `ServerWins`: Always returns the server version.
  - `ClientWins`: Always returns the local version.
  - `LastWriteWins`: Uses ETag lexicographic comparison as a recency proxy. Falls back to server version when ordering is ambiguous (missing ETags or equal values).
  - `Manual`: Returns the server version as a safe placeholder. Callers should detect this strategy and surface the conflict to the user.

The actual sync loop orchestration (periodic sync, deciding what to push/pull) is handled by higher layers in the `agent` crate, not by this crate.

### Provider Implementations

The `CalendarProvider` trait (`crates/calendar/src/provider.rs`) defines the uniform interface that all calendar backends implement. Three concrete providers exist:

**AppleCalendarProvider** (`crates/calendar/src/providers/apple.rs`):
- Uses `CalDavClient` with HTTP Basic auth (Apple app-specific passwords).
- Lazy CalDAV discovery: On first use, detects whether the configured URL is a base iCloud URL (e.g., `https://caldav.icloud.com/`) and runs the three-step discovery sequence. Caches the discovered URL for subsequent calls. Skips discovery if the URL already contains `/calendars/`.
- Internal state (`CalDavClient` and `discovered` flag) is guarded by `tokio::sync::RwLock` for safe concurrent access.

**GoogleCalendarProvider** (`crates/calendar/src/providers/google.rs`):
- Does NOT use CalDAV. Uses the Google Calendar REST API v3 (`https://www.googleapis.com/calendar/v3`) because Google restricts CalDAV access unless explicitly enabled in Cloud Console.
- OAuth2 token management: Stores `client_id`, `client_secret`, `refresh_token`, and a mutable `access_token` behind `RwLock`. Automatically refreshes the token when it expires (with a 5-minute buffer) by POSTing to `https://oauth2.googleapis.com/token`.
- Event format translation: `json_to_event` converts Google's REST JSON (which uses `iCalUID`, `dateTime`/`date` in start/end objects, lowercase status) to the internal `CalendarEvent` model. `event_to_json` converts the other direction.
- Uses the `events/import` endpoint for creating events (preserves iCalUID) and the standard `events/{id}` PUT for updates. Looks up the Google-internal event ID via `iCalUID` query parameter before update/delete.
- Handles HTTP 410 Gone (expired sync token) by automatically falling back to a full sync.

**GenericCalDavProvider** (`crates/calendar/src/providers/generic.rs`):
- For any CalDAV server (Nextcloud, Fastmail, Radicale, Zoho, etc.).
- Generates a provider ID from the label (e.g., label "Nextcloud" becomes `generic-nextcloud`).
- Lazy discovery similar to Apple: skips discovery if the URL looks specific (contains `/calendars/`, `/events/`, or ends with `.ics`). If discovery fails, logs a warning and continues with the original URL.
- `test_connection` actually fetches events (unlike Apple which relies on discovery success).

### State Management

Sync state persistence is handled in `crates/calendar/src/state.rs` via two functions that bridge the `SyncState` type to the `storage::CalendarSyncRepo`:

- **`load_provider_sync_state(repo, provider_id)`**: Loads the sync token and last-sync timestamp for a named provider from SQLite. Returns a default (empty) `SyncState` if no row exists.

- **`save_provider_sync_state(repo, provider_id, state)`**: Upserts the sync token and last-sync timestamp for a provider.

This design delegates all SQL to the `storage` crate's `CalendarSyncRepo` (which manages the `calendar_sync_state` table) while keeping the calendar crate focused on CalDAV logic.

### Event Types and Calendar Event Model

All calendar data flows through `CalendarEvent` (`crates/calendar/src/types.rs`), which represents both remote CalDAV events and locally-generated events from todo items. The `EventSource` enum tracks origin (`CalDAV` or `TodoItem`). Events carry an optional `etag` for concurrency control and an optional `status` field for iCalendar status values (CONFIRMED, CANCELLED, TENTATIVE, COMPLETED).

The `SyncState` type holds the incremental sync token (from CalDAV sync-collection or Google's nextSyncToken) and the timestamp of the last successful sync.

### How the Sync Engine Interacts With Storage

The calendar crate has a narrow interface with storage:

1. State functions (`load_provider_sync_state` / `save_provider_sync_state`) call `CalendarSyncRepo::get` and `CalendarSyncRepo::upsert` to persist sync tokens.
2. Provider implementations are stateless with respect to storage -- they only talk to remote servers. The caller (agent layer) is responsible for loading sync state before calling `get_events` and saving the new sync token afterward.
3. `CalendarEvent` structs are the exchange format between the CalDAV layer and the rest of the system. They are serializable via serde for JSON transport.

### Calendar Reconciliation

The reconciliation engine (`crates/agent/src/calendar_reconcile.rs`) keeps calendar-linked todos in sync with their corresponding calendar events after each sync cycle. It is a pure decision layer -- the `determine_action()` function examines a `CalendarEvent` and its linked `Todo` and returns a `ReconcileAction` with no side effects. The async `reconcile_calendar_events()` function drives the full reconciliation loop.

**Algorithm:**

1. Build a `HashMap<String, CalendarEvent>` from the fetched events for O(1) lookup by UID.
2. Query all todos from the repository and filter to those with a `calendar_event_uid`.
3. For each linked todo, look up the event by UID. If the event is missing from the map, treat it as deleted.
4. Apply a priority-ordered decision chain via `determine_action()`:
   - **Priority 1 -- Cancelled event**: If `event.status == "CANCELLED"`, return `ClearCalendarLink` (remove the todo's calendar link).
   - **Priority 2 -- Completed event**: If `event.status == "COMPLETED"` and the todo is not already done, return `CompleteTodo`.
   - **Priority 3 -- Due date mismatch**: If `event.start` differs from `todo.due_date` (or the todo has no due date), return `UpdateDueDate`.
   - **No changes**: If none of the above apply, return `NoChange`.
5. Apply each action via `TodoRepo::update()` with a `TodoPatch`. Errors are collected in the report rather than aborting the run.
6. Return a `ReconcileReport` summarizing counts and any errors.

**`ReconcileAction` enum:**

| Variant | Fields | Description |
|---------|--------|-------------|
| `UpdateDueDate` | `todo_id`, `old_due`, `new_due` | Sync todo's due date to match the event's start time |
| `CompleteTodo` | `todo_id` | Mark the todo as done because the event is completed |
| `ClearCalendarLink` | `todo_id`, `event_uid` | Remove the calendar link because the event was cancelled or deleted |
| `NoChange` | `todo_id` | No action needed |

**`ReconcileReport` struct:**

| Field | Type | Description |
|-------|------|-------------|
| `due_dates_updated` | `u32` | Number of todos whose due dates were updated |
| `todos_completed` | `u32` | Number of todos marked done |
| `links_cleared` | `u32` | Number of calendar links removed |
| `errors` | `Vec<String>` | Error messages from failed updates |
| `checked` | `u32` | Total calendar-linked todos examined |
| `timestamp` | `DateTime<Utc>` | When the reconciliation ran |

Derives: `Debug`, `Clone`, `Default`, `Serialize`, `Deserialize`. Uses `#[serde(rename_all = "camelCase")]`.

---

## Section 2: API Reference

### `CalDavClient`

**File**: `crates/calendar/src/caldav/client.rs`, line 19

```rust
pub struct CalDavClient {
    pub(crate) calendar_url: String,
    auth: CalDavAuth,
    http_client: Client,
    timezone: String,
}
```

**Constructors**:

| Method | Line | Signature | Description |
|--------|------|-----------|-------------|
| `new` | 29 | `(calendar_url: String, username: String, password: String, timezone: String) -> Self` | Create client with Basic auth |
| `new_with_auth` | 39 | `(calendar_url: String, auth: CalDavAuth, timezone: String) -> Self` | Create client with any auth method |

**Methods**:

| Method | Line | Signature | Description |
|--------|------|-----------|-------------|
| `set_bearer_token` | 59 | `(&mut self, token: String)` | Update Bearer token (post-OAuth2 refresh) |
| `set_calendar_url` | 64 | `(&mut self, calendar_url: String)` | Update calendar URL (post-discovery) |
| `discover_calendar_url` | 74 | `async (base_url, username, password, calendar_name) -> Result<String>` | Static. Three-step CalDAV discovery returning the resolved calendar URL |
| `put_event` | 543 | `async (&self, event: &CalendarEvent) -> Result<String>` | PUT event to server, returns ETag |
| `delete_event` | 582 | `async (&self, event_uid: &str) -> Result<()>` | DELETE event by UID |
| `get_events` | 608 | `async (&self, sync_token: Option<&str>) -> Result<(Vec<CalendarEvent>, Option<String>)>` | REPORT query; full or incremental sync |

### `CalDavAuth`

**File**: `crates/calendar/src/caldav/client.rs`, line 11

```rust
pub enum CalDavAuth {
    Basic { username: String, password: String },
    Bearer { token: String },
}
```

### `parse_vevent` / `generate_vevent`

**File**: `crates/calendar/src/caldav/parser.rs`

| Function | Line | Signature | Description |
|----------|------|-----------|-------------|
| `parse_vevent` | 10 | `(ical_data: &str) -> Result<CalendarEvent>` | Parse iCalendar VEVENT data into a `CalendarEvent`. Handles TZID parameters and UTC Z suffix. |
| `generate_vevent` | 90 | `(event: &CalendarEvent, timezone: &str) -> Result<String>` | Generate iCalendar VCALENDAR string with VTIMEZONE component for the given timezone. |

### `CalendarProvider` Trait

**File**: `crates/calendar/src/provider.rs`, line 13

```rust
#[async_trait]
pub trait CalendarProvider: Send + Sync {
    fn name(&self) -> &str;
    fn provider_id(&self) -> &str;
    async fn get_events(&self, sync_token: Option<&str>) -> Result<(Vec<CalendarEvent>, Option<String>)>;
    async fn put_event(&self, event: &CalendarEvent) -> Result<String>;
    async fn delete_event(&self, uid: &str) -> Result<()>;
    async fn test_connection(&self) -> Result<()>;
}
```

| Method | Description |
|--------|-------------|
| `name()` | Human-readable provider name (e.g., "Apple Calendar") |
| `provider_id()` | Unique ID for sync state file naming (e.g., "apple", "google", "generic-nextcloud") |
| `get_events(sync_token)` | Fetch events; returns events and optional new sync token |
| `put_event(event)` | Create or update event remotely; returns ETag |
| `delete_event(uid)` | Delete event by UID |
| `test_connection()` | Verify credentials and connectivity |

### `AppleCalendarProvider`

**File**: `crates/calendar/src/providers/apple.rs`, line 14

```rust
pub struct AppleCalendarProvider {
    client: RwLock<CalDavClient>,
    calendar_name: String,
    username: String,
    password: String,
    base_url: String,
    discovered: RwLock<bool>,
}
```

**Constructor**: `new(caldav_url, username, password, calendar_name, timezone) -> Self` (line 24)

Implements `CalendarProvider`. All trait methods call `ensure_discovered()` first to run CalDAV discovery if needed. Provider ID: `"apple"`. Name: `"Apple Calendar"`.

### `GoogleCalendarProvider`

**File**: `crates/calendar/src/providers/google.rs`, line 23

```rust
pub struct GoogleCalendarProvider {
    http_client: reqwest::Client,
    client_id: String,
    client_secret: String,
    refresh_token: String,
    access_token: RwLock<String>,
    token_expiry: RwLock<Option<DateTime<Utc>>>,
    pub calendar_id: String,
}
```

**Constructor**: `new(client_id, client_secret, access_token, refresh_token, calendar_id, _timezone) -> Self` (line 35)

Implements `CalendarProvider`. All trait methods call `ensure_token_fresh()` first. Provider ID: `"google"`. Name: `"Google Calendar"`.

**Internal methods** (not part of trait):

| Method | Line | Description |
|--------|------|-------------|
| `ensure_token_fresh` | 55 | Refresh OAuth2 token if expired (5-minute buffer) |
| `token` | 131 | Get current access token |
| `events_url` | 136 | Build REST API events endpoint URL |
| `json_to_event` | 145 | Convert Google JSON to `CalendarEvent` (static) |
| `event_to_json` | 181 | Convert `CalendarEvent` to Google JSON (static) |
| `extract_api_error` | 206 | Extract readable message from Google API error (static) |
| `find_event_id_by_uid` | 216 | Look up Google-internal event ID from iCalUID |

### `GenericCalDavProvider`

**File**: `crates/calendar/src/providers/generic.rs`, line 14

```rust
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
```

**Constructor**: `new(label, caldav_url, username, password, calendar_name, timezone) -> Self` (line 26)

Provider ID is derived from label: `generic-{sanitized_label}`. Name: the label string. Implements `CalendarProvider` with lazy discovery.

### `CalendarEvent`

**File**: `crates/calendar/src/types.rs`, line 39

```rust
pub struct CalendarEvent {
    pub uid: String,
    pub summary: String,
    pub description: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub source: EventSource,
    pub etag: Option<String>,
    pub status: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `uid` | `String` | iCalendar UID, unique event identifier |
| `summary` | `String` | Event title |
| `description` | `Option<String>` | Detailed description |
| `start` | `DateTime<Utc>` | Start time in UTC |
| `end` | `DateTime<Utc>` | End time in UTC |
| `source` | `EventSource` | Origin of the event (`CalDAV` or `TodoItem`) |
| `etag` | `Option<String>` | CalDAV ETag for concurrency control |
| `status` | `Option<String>` | iCalendar status: CONFIRMED, CANCELLED, TENTATIVE, COMPLETED |

Derives: `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize`.

### `EventSource`

**File**: `crates/calendar/src/types.rs`, line 61

```rust
pub enum EventSource {
    CalDAV,
    TodoItem,
}
```

Derives: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`.

### `SyncState`

**File**: `crates/calendar/src/types.rs`, line 69

```rust
pub struct SyncState {
    pub sync_token: Option<String>,
    pub last_sync: Option<DateTime<Utc>>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `sync_token` | `Option<String>` | Server-issued sync token for incremental sync (RFC 6578 or Google nextSyncToken) |
| `last_sync` | `Option<DateTime<Utc>>` | Timestamp of last successful sync |

Derives: `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize`.

### `ConflictResolutionStrategy`

**File**: `crates/calendar/src/types.rs`, line 8

```rust
pub enum ConflictResolutionStrategy {
    ServerWins,   // default
    ClientWins,
    LastWriteWins,
    Manual,
}
```

Serializes to camelCase (`"serverWins"`, `"clientWins"`, `"lastWriteWins"`, `"manual"`). Implements `FromStr` with support for camelCase, snake_case, and PascalCase inputs. Default: `ServerWins`.

### Sync State Persistence Functions

**File**: `crates/calendar/src/state.rs`

| Function | Line | Signature | Description |
|----------|------|-----------|-------------|
| `load_provider_sync_state` | 7 | `async (repo: &CalendarSyncRepo, provider_id: &str) -> Result<SyncState>` | Load sync state from SQLite; returns empty state if not found |
| `save_provider_sync_state` | 25 | `async (repo: &CalendarSyncRepo, provider_id: &str, state: &SyncState) -> Result<()>` | Upsert sync state to SQLite |

### Conflict Resolution Functions

**File**: `crates/calendar/src/sync_engine.rs`

| Function | Line | Signature | Description |
|----------|------|-----------|-------------|
| `detect_conflict` | 6 | `(server_event: &CalendarEvent, local_event: &CalendarEvent) -> bool` | True if same UID but differing content |
| `resolve_conflict` | 30 | `(server_event: &CalendarEvent, local_event: &CalendarEvent, strategy: ConflictResolutionStrategy) -> CalendarEvent` | Apply strategy to pick winning version |

### `ReconcileAction`

**File**: `crates/agent/src/calendar_reconcile.rs`, line 17

```rust
pub enum ReconcileAction {
    UpdateDueDate { todo_id: String, old_due: Option<DateTime<Utc>>, new_due: DateTime<Utc> },
    CompleteTodo { todo_id: String },
    ClearCalendarLink { todo_id: String, event_uid: String },
    NoChange { todo_id: String },
}
```

Derives: `Debug`, `Clone`, `PartialEq`.

### `ReconcileReport`

**File**: `crates/agent/src/calendar_reconcile.rs`, line 33

```rust
pub struct ReconcileReport {
    pub due_dates_updated: u32,
    pub todos_completed: u32,
    pub links_cleared: u32,
    pub errors: Vec<String>,
    pub checked: u32,
    pub timestamp: DateTime<Utc>,
}
```

Derives: `Debug`, `Clone`, `Default`, `Serialize`, `Deserialize`. Serde: `rename_all = "camelCase"`.

### Reconciliation Functions

**File**: `crates/agent/src/calendar_reconcile.rs`

| Function | Line | Signature | Description |
|----------|------|-----------|-------------|
| `determine_action` | 47 | `(event: &CalendarEvent, todo: &Todo) -> ReconcileAction` | Pure function: decides the reconcile action for one event-todo pair |
| `reconcile_calendar_events` | 89 | `async (todo_repo: &TodoRepo, events: Vec<CalendarEvent>) -> Result<ReconcileReport>` | Reconcile all calendar-linked todos against fetched events, returning a summary report |

### `CalendarError`

**File**: `crates/calendar/src/error.rs`, line 7

```rust
pub enum CalendarError {
    AuthFailed(String),
    ConnectionFailed(String),
    SyncFailed(String),
    NotFound(String),
    ProtocolError(String),
    Io(std::io::Error),
    Json(serde_json::Error),
}
```

| Variant | Description |
|---------|-------------|
| `AuthFailed` | HTTP 401/403 or OAuth2 token refresh failure |
| `ConnectionFailed` | Network-level failure (DNS, timeout, TLS) |
| `SyncFailed` | Sync-specific logic failure |
| `NotFound` | Calendar or event not found |
| `ProtocolError` | Malformed XML, invalid datetime, missing required fields |
| `Io` | File system errors (auto-converted via `From<std::io::Error>`) |
| `Json` | JSON serialization/deserialization errors (auto-converted via `From<serde_json::Error>`) |

Implements `From<CalendarError> for common::KlyntbotError` (line 30), mapping to `KlyntbotError::Calendar(String)`.

### Public Re-exports

**File**: `crates/calendar/src/lib.rs`

The crate root re-exports all public API items:

```rust
pub use caldav::{generate_vevent, parse_vevent, CalDavAuth, CalDavClient};
pub use error::CalendarError;
pub use provider::CalendarProvider;
pub use providers::{AppleCalendarProvider, GenericCalDavProvider, GoogleCalendarProvider};
pub use state::{load_provider_sync_state, save_provider_sync_state};
pub use sync_engine::{detect_conflict, resolve_conflict};
pub use types::{CalendarEvent, ConflictResolutionStrategy, EventSource, SyncState};
```
