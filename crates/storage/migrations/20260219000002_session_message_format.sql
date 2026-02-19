-- G-20: Add structured message fields to session_messages.
-- tool_calls stores serialized tool call data (function name, arguments, result).
-- metadata stores extensible message metadata (reasoning, content parts, etc.).

ALTER TABLE session_messages ADD COLUMN tool_calls JSONB;
ALTER TABLE session_messages ADD COLUMN metadata JSONB;
