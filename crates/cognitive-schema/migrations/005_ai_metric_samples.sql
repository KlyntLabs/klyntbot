-- Unified AI metric samples table.
-- Every metric sample emitted by `#[ai(metric(...))]` on event variants lands here.
-- `MetricHarvestConsumer` inserts; `MetricRepo::aggregate_metric` reads.

CREATE TABLE IF NOT EXISTS ai_metric_samples (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_name TEXT NOT NULL,
    value REAL NOT NULL,
    sample_time INTEGER NOT NULL, -- unix timestamp (seconds)
    source_domain TEXT NOT NULL,
    source_event_kind TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_metric_samples_name_time
    ON ai_metric_samples(metric_name, sample_time);

CREATE INDEX IF NOT EXISTS idx_metric_samples_domain
    ON ai_metric_samples(source_domain);
