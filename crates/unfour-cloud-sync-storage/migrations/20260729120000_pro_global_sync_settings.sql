CREATE TABLE pro_sync_account_settings (
  account_id TEXT NOT NULL PRIMARY KEY,
  sync_enabled INTEGER NOT NULL DEFAULT 0 CHECK (sync_enabled IN (0, 1)),
  updated_at TEXT NOT NULL
);

ALTER TABLE pro_workspace_sync_bindings
ADD COLUMN consecutive_failure_count INTEGER NOT NULL DEFAULT 0
CHECK (consecutive_failure_count >= 0);
