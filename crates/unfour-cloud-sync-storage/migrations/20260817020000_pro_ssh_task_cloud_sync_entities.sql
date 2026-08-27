CREATE TABLE pro_sync_outbox_protocol_v3 (
  account_id TEXT NOT NULL,
  local_workspace_id TEXT NOT NULL,
  cloud_workspace_id TEXT NOT NULL,
  entity_type TEXT NOT NULL CHECK (entity_type IN (
    'workspace', 'workspaceVariable', 'workspaceEnvironment',
    'workspaceEnvironmentVariable', 'apiCollection', 'apiFolder', 'apiRequest',
    'sshTask', 'sshTaskStep'
  )),
  entity_id TEXT NOT NULL,
  operation_id TEXT NOT NULL UNIQUE,
  parent_entity_id TEXT,
  operation TEXT NOT NULL CHECK (operation IN ('upsert', 'delete')),
  base_version INTEGER NOT NULL DEFAULT 0 CHECK (base_version >= 0),
  payload_schema_version INTEGER NOT NULL DEFAULT 1 CHECK (payload_schema_version = 1),
  canonical_payload_json TEXT CHECK (
    canonical_payload_json IS NULL OR json_valid(canonical_payload_json)
  ),
  deleted_at TEXT,
  content_revision INTEGER NOT NULL CHECK (content_revision >= 0),
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
    'pending', 'in_flight', 'uncertain', 'dead'
  )),
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
  next_attempt_at TEXT,
  lease_owner TEXT,
  lease_started_at TEXT,
  lease_expires_at TEXT,
  last_error TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(account_id, cloud_workspace_id, entity_type, entity_id),
  FOREIGN KEY(account_id, local_workspace_id)
    REFERENCES pro_workspace_sync_bindings(account_id, local_workspace_id)
    ON DELETE CASCADE,
  FOREIGN KEY(account_id, cloud_workspace_id)
    REFERENCES pro_workspace_sync_bindings(account_id, cloud_workspace_id)
    ON DELETE CASCADE,
  CHECK (
    (operation = 'upsert' AND canonical_payload_json IS NOT NULL AND deleted_at IS NULL)
    OR (operation = 'delete' AND canonical_payload_json IS NULL AND deleted_at IS NOT NULL)
    OR status = 'dead'
    OR (
      entity_type IN (
        'apiCollection', 'apiFolder', 'apiRequest', 'sshTask', 'sshTaskStep'
      )
      AND status IN ('pending', 'uncertain')
      AND canonical_payload_json IS NULL
      AND deleted_at IS NULL
    )
  )
);

INSERT INTO pro_sync_outbox_protocol_v3 (
  account_id, local_workspace_id, cloud_workspace_id, entity_type, entity_id,
  operation_id, parent_entity_id, operation, base_version,
  payload_schema_version, canonical_payload_json, deleted_at, content_revision,
  status, attempt_count, next_attempt_at, lease_owner, lease_started_at,
  lease_expires_at, last_error, created_at, updated_at
)
SELECT
  account_id, local_workspace_id, cloud_workspace_id, entity_type, entity_id,
  operation_id, parent_entity_id, operation, base_version,
  payload_schema_version, canonical_payload_json, deleted_at, content_revision,
  status, attempt_count, next_attempt_at, lease_owner, lease_started_at,
  lease_expires_at, last_error, created_at, updated_at
FROM pro_sync_outbox;

CREATE TABLE pro_sync_entity_state_protocol_v3 (
  account_id TEXT NOT NULL,
  cloud_workspace_id TEXT NOT NULL,
  entity_type TEXT NOT NULL CHECK (entity_type IN (
    'workspace', 'workspaceVariable', 'workspaceEnvironment',
    'workspaceEnvironmentVariable', 'apiCollection', 'apiFolder', 'apiRequest',
    'sshTask', 'sshTaskStep'
  )),
  entity_id TEXT NOT NULL,
  server_version INTEGER NOT NULL DEFAULT 0 CHECK (server_version >= 0),
  last_operation_id TEXT,
  sync_status TEXT NOT NULL DEFAULT 'synced' CHECK (sync_status IN (
    'synced', 'conflict', 'paused'
  )),
  conflict_remote_payload_json TEXT CHECK (
    conflict_remote_payload_json IS NULL OR json_valid(conflict_remote_payload_json)
  ),
  conflict_remote_operation TEXT CHECK (
    conflict_remote_operation IS NULL OR conflict_remote_operation IN ('upsert', 'delete')
  ),
  conflict_parent_entity_id TEXT,
  conflict_deleted_at TEXT,
  conflict_operation_id TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(account_id, cloud_workspace_id, entity_type, entity_id),
  FOREIGN KEY(account_id, cloud_workspace_id)
    REFERENCES pro_workspace_sync_bindings(account_id, cloud_workspace_id)
    ON DELETE CASCADE
);

INSERT INTO pro_sync_entity_state_protocol_v3 (
  account_id, cloud_workspace_id, entity_type, entity_id, server_version,
  last_operation_id, sync_status, conflict_remote_payload_json,
  conflict_remote_operation, conflict_parent_entity_id, conflict_deleted_at,
  conflict_operation_id, updated_at
)
SELECT
  account_id, cloud_workspace_id, entity_type, entity_id, server_version,
  last_operation_id, sync_status, conflict_remote_payload_json,
  conflict_remote_operation, conflict_parent_entity_id, conflict_deleted_at,
  conflict_operation_id, updated_at
FROM pro_sync_entity_state;

DROP TABLE pro_sync_entity_state;
DROP TABLE pro_sync_outbox;

ALTER TABLE pro_sync_outbox_protocol_v3 RENAME TO pro_sync_outbox;
ALTER TABLE pro_sync_entity_state_protocol_v3 RENAME TO pro_sync_entity_state;

CREATE INDEX idx_pro_sync_outbox_due
ON pro_sync_outbox(account_id, cloud_workspace_id, status, next_attempt_at, created_at);

CREATE INDEX idx_pro_sync_entity_state_conflicts
ON pro_sync_entity_state(account_id, cloud_workspace_id, sync_status, entity_type, entity_id);
