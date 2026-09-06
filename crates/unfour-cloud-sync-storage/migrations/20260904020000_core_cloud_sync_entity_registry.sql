-- Versioned per-entity bootstrap markers are retired. New bindings upload
-- every registered sync entity through the common initial outbox.
ALTER TABLE cloud_sync_workspace_bindings DROP COLUMN api_v2_bootstrap_state;
ALTER TABLE cloud_sync_workspace_bindings DROP COLUMN ssh_task_v3_bootstrap_state;
ALTER TABLE cloud_sync_workspace_bindings DROP COLUMN connection_v4_bootstrap_state;

-- Protocol 5 retains the latest complete remote envelope independently from
-- the local apply state. Entity names are deliberately not constrained so a
-- newer sender cannot make a tolerant reader lose data.
CREATE TABLE cloud_sync_remote_entity (
  account_id TEXT NOT NULL,
  cloud_workspace_id TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  parent_entity_id TEXT,
  server_version INTEGER NOT NULL CHECK (server_version >= 1),
  payload_schema_version INTEGER NOT NULL CHECK (payload_schema_version >= 1),
  operation TEXT NOT NULL CHECK (operation IN ('upsert', 'delete')),
  canonical_payload_json TEXT CHECK (
    canonical_payload_json IS NULL OR json_valid(canonical_payload_json)
  ),
  deleted_at TEXT,
  operation_id TEXT,
  compatibility_state TEXT NOT NULL CHECK (
    compatibility_state IN ('supported', 'deferred_compatibility')
  ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(account_id, cloud_workspace_id, entity_type, entity_id),
  FOREIGN KEY(account_id, cloud_workspace_id)
    REFERENCES cloud_sync_workspace_bindings(account_id, cloud_workspace_id)
    ON DELETE CASCADE,
  CHECK (
    (operation = 'upsert' AND canonical_payload_json IS NOT NULL AND deleted_at IS NULL)
    OR (operation = 'delete' AND canonical_payload_json IS NULL AND deleted_at IS NOT NULL)
  )
);

CREATE INDEX idx_cloud_sync_remote_entity_compatibility
ON cloud_sync_remote_entity(
  account_id, cloud_workspace_id, compatibility_state, entity_type, entity_id
);

-- Conflicts must retain the actual schema carried by the remote entity. NULL
-- remains valid only for historical, pre-Protocol-5 conflict rows.
ALTER TABLE cloud_sync_entity_state
ADD COLUMN conflict_payload_schema_version INTEGER
CHECK (
  conflict_payload_schema_version IS NULL OR conflict_payload_schema_version >= 1
);

-- Snapshot staging must also accept future entity schemas and unknown entity
-- names. It is disposable transport staging, but preserving every page here is
-- required before the new binding and its remote records can commit together.
CREATE TABLE cloud_sync_snapshot_staging_protocol_5 (
  stage_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  cloud_workspace_id TEXT NOT NULL,
  at_cursor INTEGER NOT NULL CHECK (at_cursor >= 0),
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  parent_entity_id TEXT,
  server_version INTEGER NOT NULL CHECK (server_version >= 1),
  payload_schema_version INTEGER NOT NULL CHECK (payload_schema_version >= 1),
  payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
  topology_rank INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(stage_id, entity_type, entity_id)
);

INSERT INTO cloud_sync_snapshot_staging_protocol_5 (
  stage_id, account_id, cloud_workspace_id, at_cursor, entity_type, entity_id,
  parent_entity_id, server_version, payload_schema_version, payload_json,
  topology_rank, created_at
)
SELECT
  stage_id, account_id, cloud_workspace_id, at_cursor, entity_type, entity_id,
  parent_entity_id, server_version, payload_schema_version, payload_json,
  topology_rank, created_at
FROM cloud_sync_snapshot_staging;

DROP TABLE cloud_sync_snapshot_staging;
ALTER TABLE cloud_sync_snapshot_staging_protocol_5
RENAME TO cloud_sync_snapshot_staging;

CREATE INDEX idx_cloud_sync_snapshot_staging_apply
ON cloud_sync_snapshot_staging(stage_id, topology_rank, entity_type, entity_id);
