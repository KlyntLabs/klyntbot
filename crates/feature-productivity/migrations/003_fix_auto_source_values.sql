-- Fix incorrectly written source values for auto-detected sessions.
-- IntelligenceSessionRepo and related modules were inserting "auto"
-- instead of the canonical "auto_detected" expected by FocusSessionRepo.
UPDATE productivity_sessions SET source = 'auto_detected' WHERE source = 'auto';
