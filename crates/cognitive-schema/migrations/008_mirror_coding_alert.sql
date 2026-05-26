-- Add coding alert columns to mirror snippets

ALTER TABLE mirror_snippets ADD COLUMN coding_alert_kind TEXT;
ALTER TABLE mirror_snippets ADD COLUMN coding_alert_severity TEXT;
