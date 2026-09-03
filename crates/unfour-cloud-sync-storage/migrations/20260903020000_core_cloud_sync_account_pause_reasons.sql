-- Preserve the distinction between an account-context pause and an explicit
-- workspace pause. This lets the owning account resume its bindings after
-- re-authentication without changing a user's workspace pause preference.
CREATE TABLE cloud_sync_account_binding_pause_reasons (
  account_id TEXT NOT NULL,
  local_workspace_id TEXT NOT NULL,
  previous_state TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (account_id, local_workspace_id),
  FOREIGN KEY (account_id, local_workspace_id)
    REFERENCES cloud_sync_workspace_bindings(account_id, local_workspace_id)
    ON DELETE CASCADE
);

CREATE INDEX idx_cloud_sync_account_pause_reasons_account
ON cloud_sync_account_binding_pause_reasons(account_id, local_workspace_id);
