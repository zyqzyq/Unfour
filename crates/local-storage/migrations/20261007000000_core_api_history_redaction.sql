-- The HTTP owner repairs old payloads before exposing history. New writes
-- explicitly use version 1; legacy writers remain detectable on next startup.
ALTER TABLE api_history ADD COLUMN redaction_version INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_api_history_redaction_pending ON api_history(redaction_version)
WHERE redaction_version = 0;
