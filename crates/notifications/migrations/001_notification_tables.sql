-- Idempotency gate: one row per (alarm, channel) delivery.
CREATE TABLE notification_log (
    alarm_id TEXT NOT NULL,
    channel TEXT NOT NULL,
    sent_at_ms INTEGER NOT NULL,
    ack_at_ms INTEGER,
    error TEXT,
    PRIMARY KEY (alarm_id, channel)
);
CREATE INDEX idx_notification_log_sent ON notification_log(sent_at_ms);

-- Quiet-hours-held notifications awaiting release.
CREATE TABLE held_notifications (
    id TEXT PRIMARY KEY,
    alarm_id TEXT NOT NULL,
    channels TEXT NOT NULL,
    payload TEXT NOT NULL,
    release_at_ms INTEGER NOT NULL,
    released INTEGER NOT NULL DEFAULT 0,
    held_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_held_notifications_pending
    ON held_notifications(release_at_ms) WHERE released = 0;
