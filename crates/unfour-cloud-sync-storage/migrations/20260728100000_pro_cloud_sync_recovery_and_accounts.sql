CREATE TABLE pro_workspace_sync_bindings_v2 (
  account_id TEXT NOT NULL,
  local_workspace_id TEXT NOT NULL,
  cloud_workspace_id TEXT NOT NULL,
  last_pulled_cursor INTEGER NOT NULL DEFAULT 0 CHECK (last_pulled_cursor >= 0),
  sync_enabled INTEGER NOT NULL DEFAULT 1 CHECK (sync_enabled IN (0, 1)),
  state TEXT NOT NULL DEFAULT 'preparing' CHECK (state IN (
    'preparing', 'uploading', 'downloading', 'reconciling', 'active',
    'paused', 'conflict', 'error'
  )),
  initial_cursor INTEGER,
  initial_total INTEGER NOT NULL DEFAULT 0 CHECK (initial_total >= 0),
  initial_confirmed INTEGER NOT NULL DEFAULT 0 CHECK (initial_confirmed >= 0),
  initialization_checkpoint TEXT,
  generation INTEGER NOT NULL DEFAULT 0 CHECK (generation >= 0),
  last_success_at TEXT,
  last_error TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(account_id, local_workspace_id),
  UNIQUE(account_id, cloud_workspace_id),
  FOREIGN KEY(local_workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

INSERT INTO pro_workspace_sync_bindings_v2 (
  account_id, local_workspace_id, cloud_workspace_id, last_pulled_cursor,
  sync_enabled, state, initial_cursor, initial_total, initial_confirmed,
  initialization_checkpoint, generation, last_success_at, last_error,
  created_at, updated_at
)
SELECT
  'unclaimed', local_workspace_id, cloud_workspace_id,
  CASE
    WHEN last_pulled_cursor GLOB '[0-9]*' AND last_pulled_cursor <> ''
      THEN CAST(last_pulled_cursor AS INTEGER)
    ELSE 0
  END,
  0, 'paused', NULL, 0, 0, 'legacy-unclaimed', 0,
  last_success_at, 'legacy_binding_unclaimed', created_at, updated_at
FROM pro_workspace_sync_bindings;

CREATE TABLE pro_sync_outbox_v2 (
  account_id TEXT NOT NULL,
  local_workspace_id TEXT NOT NULL,
  cloud_workspace_id TEXT NOT NULL,
  entity_type TEXT NOT NULL CHECK (entity_type IN (
    'workspace', 'workspaceVariable', 'workspaceEnvironment',
    'workspaceEnvironmentVariable'
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
    REFERENCES pro_workspace_sync_bindings_v2(account_id, local_workspace_id)
    ON DELETE CASCADE,
  FOREIGN KEY(account_id, cloud_workspace_id)
    REFERENCES pro_workspace_sync_bindings_v2(account_id, cloud_workspace_id)
    ON DELETE CASCADE,
  CHECK (
    (operation = 'upsert' AND canonical_payload_json IS NOT NULL AND deleted_at IS NULL)
    OR (operation = 'delete' AND canonical_payload_json IS NULL AND deleted_at IS NOT NULL)
    OR status = 'dead'
  )
);

INSERT INTO pro_sync_outbox_v2 (
  account_id, local_workspace_id, cloud_workspace_id, entity_type, entity_id,
  operation_id, parent_entity_id, operation, base_version,
  payload_schema_version, canonical_payload_json, deleted_at, content_revision,
  status, attempt_count, next_attempt_at, lease_owner, lease_started_at,
  lease_expires_at, last_error, created_at, updated_at
)
SELECT
  'unclaimed', local_workspace_id, cloud_workspace_id, entity_type, entity_id,
  operation_id, parent_entity_id, operation, base_version,
  payload_schema_version, NULL,
  CASE WHEN operation = 'delete' THEN updated_at ELSE NULL END,
  content_revision, 'dead', attempt_count, NULL, NULL, NULL, NULL,
  'legacy_binding_unclaimed', created_at, updated_at
FROM pro_sync_outbox;

CREATE TABLE pro_sync_entity_state_v2 (
  account_id TEXT NOT NULL,
  cloud_workspace_id TEXT NOT NULL,
  entity_type TEXT NOT NULL CHECK (entity_type IN (
    'workspace', 'workspaceVariable', 'workspaceEnvironment',
    'workspaceEnvironmentVariable'
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
    REFERENCES pro_workspace_sync_bindings_v2(account_id, cloud_workspace_id)
    ON DELETE CASCADE
);

INSERT INTO pro_sync_entity_state_v2 (
  account_id, cloud_workspace_id, entity_type, entity_id, server_version,
  last_operation_id, sync_status, conflict_remote_payload_json,
  conflict_remote_operation, conflict_parent_entity_id, conflict_deleted_at,
  conflict_operation_id, updated_at
)
SELECT
  'unclaimed', cloud_workspace_id, entity_type, entity_id, server_version,
  last_operation_id, 'paused', conflict_remote_payload_json,
  conflict_remote_operation, conflict_parent_entity_id, conflict_deleted_at,
  NULL, updated_at
FROM pro_sync_entity_state;

DROP TABLE pro_sync_entity_state;
DROP TABLE pro_sync_outbox;
DROP TABLE pro_workspace_sync_bindings;

ALTER TABLE pro_workspace_sync_bindings_v2 RENAME TO pro_workspace_sync_bindings;
ALTER TABLE pro_sync_outbox_v2 RENAME TO pro_sync_outbox;
ALTER TABLE pro_sync_entity_state_v2 RENAME TO pro_sync_entity_state;

CREATE INDEX idx_pro_sync_bindings_account_enabled
ON pro_workspace_sync_bindings(account_id, sync_enabled, state, updated_at);

-- A local mutation has no account identity of its own. This single-row gate is
-- updated at the account boundary so the transactional command hook can only
-- append an intent for the currently authenticated account.
CREATE TABLE pro_sync_runtime_context (
  singleton INTEGER NOT NULL PRIMARY KEY CHECK (singleton = 1),
  active_account_id TEXT,
  generation INTEGER NOT NULL DEFAULT 0 CHECK (generation >= 0),
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_pro_sync_outbox_due
ON pro_sync_outbox(account_id, cloud_workspace_id, status, next_attempt_at, created_at);

CREATE TABLE pro_sync_attempts (
  account_id TEXT NOT NULL,
  cloud_workspace_id TEXT NOT NULL,
  operation_id TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  base_version INTEGER NOT NULL CHECK (base_version >= 0),
  status TEXT NOT NULL CHECK (status IN ('in_flight', 'uncertain', 'applied', 'no_op', 'failed')),
  lease_owner TEXT,
  started_at TEXT NOT NULL,
  lease_expires_at TEXT,
  finished_at TEXT,
  result_server_version INTEGER,
  result_cursor INTEGER,
  error_code TEXT,
  PRIMARY KEY(account_id, cloud_workspace_id, operation_id),
  FOREIGN KEY(account_id, cloud_workspace_id)
    REFERENCES pro_workspace_sync_bindings(account_id, cloud_workspace_id)
    ON DELETE CASCADE
);

CREATE INDEX idx_pro_sync_attempts_recovery
ON pro_sync_attempts(account_id, status, lease_expires_at, started_at);

CREATE TABLE pro_sync_snapshot_staging (
  stage_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  cloud_workspace_id TEXT NOT NULL,
  at_cursor INTEGER NOT NULL CHECK (at_cursor >= 0),
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  parent_entity_id TEXT,
  server_version INTEGER NOT NULL CHECK (server_version >= 1),
  payload_schema_version INTEGER NOT NULL CHECK (payload_schema_version = 1),
  payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
  topology_rank INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(stage_id, entity_type, entity_id)
);

CREATE INDEX idx_pro_sync_snapshot_staging_apply
ON pro_sync_snapshot_staging(stage_id, topology_rank, entity_type, entity_id);

CREATE TABLE pro_sync_diagnostics (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  account_id TEXT NOT NULL,
  cloud_workspace_id TEXT,
  category TEXT NOT NULL CHECK (category IN ('retryable', 'permanent', 'dead_letter', 'conflict')),
  error_code TEXT NOT NULL,
  entity_type TEXT,
  entity_id TEXT,
  occurred_at TEXT NOT NULL
);

CREATE INDEX idx_pro_sync_diagnostics_recent
ON pro_sync_diagnostics(account_id, occurred_at DESC);
