-- A local workspace has one durable Cloud Sync owner at a time. Historical
-- duplicate bindings are intentionally not deleted; they remain unresolved
-- until an explicit ownership transition exists.
CREATE TABLE cloud_sync_workspace_ownership (
  local_workspace_id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  cloud_workspace_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(local_workspace_id)
    REFERENCES workspaces(id)
    ON DELETE CASCADE,
  FOREIGN KEY(account_id, local_workspace_id)
    REFERENCES cloud_sync_workspace_bindings(account_id, local_workspace_id)
    ON DELETE CASCADE,
  FOREIGN KEY(account_id, cloud_workspace_id)
    REFERENCES cloud_sync_workspace_bindings(account_id, cloud_workspace_id)
    ON DELETE CASCADE
);

CREATE INDEX idx_cloud_sync_workspace_ownership_account
ON cloud_sync_workspace_ownership(account_id, local_workspace_id);

-- Backfill only unambiguous historical workspaces. A duplicate is left
-- without metadata so all runtime resolution paths fail closed.
INSERT INTO cloud_sync_workspace_ownership (
  local_workspace_id, account_id, cloud_workspace_id, created_at, updated_at
)
SELECT binding.local_workspace_id, binding.account_id, binding.cloud_workspace_id,
       binding.created_at, binding.updated_at
FROM cloud_sync_workspace_bindings AS binding
WHERE (
  SELECT COUNT(*)
  FROM cloud_sync_workspace_bindings AS candidate
  WHERE candidate.local_workspace_id = binding.local_workspace_id
) = 1;
