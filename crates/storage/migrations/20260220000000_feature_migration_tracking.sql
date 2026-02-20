CREATE TABLE IF NOT EXISTS _feature_migrations (
    feature_name TEXT NOT NULL,
    version BIGINT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (feature_name, version)
);
