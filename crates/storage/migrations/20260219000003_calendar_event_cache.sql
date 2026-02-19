-- Calendar event cache: stores events from CalDAV providers locally
-- so reads can be served from cache instead of hitting remote servers.

CREATE TABLE calendar_event_cache (
    uid         TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    summary     TEXT NOT NULL,
    description TEXT,
    start_at    TIMESTAMPTZ NOT NULL,
    end_at      TIMESTAMPTZ NOT NULL,
    source      TEXT NOT NULL DEFAULT 'CalDAV',
    etag        TEXT,
    status      TEXT,
    cached_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (uid, provider_id)
);

CREATE INDEX idx_calendar_event_cache_provider ON calendar_event_cache (provider_id);
CREATE INDEX idx_calendar_event_cache_start ON calendar_event_cache (start_at);
CREATE INDEX idx_calendar_event_cache_uid ON calendar_event_cache (uid);
