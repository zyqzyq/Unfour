CREATE TABLE pro_workspace_sync_bindings (
  local_workspace_id TEXT PRIMARY KEY,
  cloud_workspace_id TEXT NOT NULL UNIQUE,
  last_pulled_cursor TEXT NOT NULL DEFAULT '',
  sync_enabled INTEGER NOT NULL DEFAULT 1 CHECK (sync_enabled IN (0, 1)),
  last_success_at TEXT,
  last_error TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(local_workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE pro_sync_outbox (
  operation_id TEXT PRIMARY KEY,
  local_workspace_id TEXT NOT NULL,
  cloud_workspace_id TEXT NOT NULL,
  entity_type TEXT NOT NULL CHECK (entity_type IN (
    'workspace',
    'workspaceVariable',
    'workspaceEnvironment',
    'workspaceEnvironmentVariable'
  )),
  entity_id TEXT NOT NULL,
  parent_entity_id TEXT,
  operation TEXT NOT NULL CHECK (operation IN ('upsert', 'delete')),
  base_version INTEGER NOT NULL DEFAULT 0 CHECK (base_version >= 0),
  payload_schema_version INTEGER NOT NULL DEFAULT 1 CHECK (payload_schema_version = 1),
  content_revision INTEGER NOT NULL CHECK (content_revision >= 0),
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
    'pending', 'in_flight', 'uncertain'
  )),
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
  next_attempt_at TEXT,
  last_error TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(local_workspace_id) REFERENCES pro_workspace_sync_bindings(local_workspace_id)
    ON DELETE CASCADE,
  FOREIGN KEY(cloud_workspace_id) REFERENCES pro_workspace_sync_bindings(cloud_workspace_id)
    ON DELETE CASCADE
);

CREATE UNIQUE INDEX uq_pro_sync_outbox_pending_entity
ON pro_sync_outbox(cloud_workspace_id, entity_type, entity_id)
WHERE status = 'pending';

CREATE INDEX idx_pro_sync_outbox_due
ON pro_sync_outbox(cloud_workspace_id, status, next_attempt_at, created_at);

CREATE TABLE pro_sync_entity_state (
  cloud_workspace_id TEXT NOT NULL,
  entity_type TEXT NOT NULL CHECK (entity_type IN (
    'workspace',
    'workspaceVariable',
    'workspaceEnvironment',
    'workspaceEnvironmentVariable'
  )),
  entity_id TEXT NOT NULL,
  server_version INTEGER NOT NULL DEFAULT 0 CHECK (server_version >= 0),
  last_operation_id TEXT,
  sync_status TEXT NOT NULL DEFAULT 'synced' CHECK (sync_status IN ('synced', 'conflict')),
  conflict_remote_payload_json TEXT CHECK (
    conflict_remote_payload_json IS NULL OR json_valid(conflict_remote_payload_json)
  ),
  conflict_remote_operation TEXT CHECK (
    conflict_remote_operation IS NULL OR conflict_remote_operation IN ('upsert', 'delete')
  ),
  conflict_parent_entity_id TEXT,
  conflict_deleted_at TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(cloud_workspace_id, entity_type, entity_id),
  FOREIGN KEY(cloud_workspace_id) REFERENCES pro_workspace_sync_bindings(cloud_workspace_id)
    ON DELETE CASCADE
);

CREATE INDEX idx_pro_sync_entity_state_conflicts
ON pro_sync_entity_state(cloud_workspace_id, sync_status, entity_type, entity_id);
