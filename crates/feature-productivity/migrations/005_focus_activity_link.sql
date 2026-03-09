-- Link activity events to focus sessions.
-- When a focus session is active, all activity events recorded during that
-- period are stamped with the session ID. This eliminates the need for
-- client-side time-overlap heuristics and enables accurate focus-aware
-- productivity analysis.

ALTER TABLE activity_events ADD COLUMN focus_session_id TEXT REFERENCES focus_sessions(id);

CREATE INDEX IF NOT EXISTS idx_activity_events_focus_session
    ON activity_events(focus_session_id)
    WHERE focus_session_id IS NOT NULL;
