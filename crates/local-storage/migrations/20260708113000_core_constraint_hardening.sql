-- Constraint hardening for the core schema.
--
-- Three fixes from the schema review (2026-07-08):
--
-- 1. P2 — Subtype tables must validate the parent `connections.connection_type`.
--    Without this you could insert an `ssh_connections` row pointing at a
--    `connection_type = 'database'` connection (or vice versa), producing rows
--    that no code path ever reads. Additive triggers, no rebuild needed.
--
-- 2. P4 — Unify workspace FK delete behavior to ON DELETE CASCADE.
--    The SSH-side tables (ssh_host_keys, ssh_terminal_history) already cascade;
--    the API/DB-side tables (api_collections, api_collection_folders,
--    api_requests, api_history, db_query_history, api_environments, saved_sql,
--    activity_events) silently default to RESTRICT. Workspaces are soft-deleted
--    today, so this is latent, but a future hard-delete/GC path would then
--    behave inconsistently (SSH data silently wiped, API data rejected). Make
--    every workspace FK cascade so a deleted workspace takes all its data.
--
-- 3. P7 — ssh_host_keys.port lacks the range CHECK that connections.port has.
--    Add `CHECK (port BETWEEN 1 AND 65535)` (NOT NULL, so no NULL branch).
--
-- Items 2 and 3 require table rebuilds: SQLite cannot ALTER a foreign key's
-- ON DELETE clause or add a CHECK to an existing column. We rebuild with
-- PRAGMA legacy_alter_table = ON because foreign_keys is enabled on the
-- connection, and dropping a table that is the target of another table's FK
-- would otherwise fail. Each rebuild preserves every column, CHECK, index, and
-- non-workspace FK exactly, changing only the targeted constraint.
--
-- Dropping a table also drops any triggers attached to it, so the folder-cycle
-- triggers (from 20260707123000) and the connection-workspace triggers are
-- recreated after their owning table is rebuilt.

-- ============================================================
-- P2: connection_type validation triggers (additive)
-- ============================================================

CREATE TRIGGER IF NOT EXISTS trg_ssh_connections_type_insert
BEFORE INSERT ON ssh_connections
WHEN NOT EXISTS (
  SELECT 1 FROM connections
  WHERE id = NEW.connection_id AND connection_type = 'ssh'
)
BEGIN
  SELECT RAISE(ABORT, 'ssh_connections must reference a connection of type ssh');
END;

CREATE TRIGGER IF NOT EXISTS trg_ssh_connections_type_update
BEFORE UPDATE OF connection_id ON ssh_connections
WHEN NOT EXISTS (
  SELECT 1 FROM connections
  WHERE id = NEW.connection_id AND connection_type = 'ssh'
)
BEGIN
  SELECT RAISE(ABORT, 'ssh_connections must reference a connection of type ssh');
END;

CREATE TRIGGER IF NOT EXISTS trg_database_connections_type_insert
BEFORE INSERT ON database_connections
WHEN NOT EXISTS (
  SELECT 1 FROM connections
  WHERE id = NEW.connection_id AND connection_type = 'database'
)
BEGIN
  SELECT RAISE(ABORT, 'database_connections must reference a connection of type database');
END;

CREATE TRIGGER IF NOT EXISTS trg_database_connections_type_update
BEFORE UPDATE OF connection_id ON database_connections
WHEN NOT EXISTS (
  SELECT 1 FROM connections
  WHERE id = NEW.connection_id AND connection_type = 'database'
)
BEGIN
  SELECT RAISE(ABORT, 'database_connections must reference a connection of type database');
END;

-- ============================================================
-- P4 & P7: table rebuilds (legacy_alter_table to bypass FK
-- enforcement during the drop/rename dance)
-- ============================================================

PRAGMA legacy_alter_table = ON;

-- ---- api_collections (workspace FK -> CASCADE) ----
DROP TABLE IF EXISTS api_collections_new;
CREATE TABLE api_collections_new (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  revision INTEGER NOT NULL DEFAULT 1,
  sync_status TEXT NOT NULL DEFAULT 'local',
  remote_id TEXT,
  UNIQUE(workspace_id, id),
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);
INSERT INTO api_collections_new SELECT * FROM api_collections;
DROP TABLE api_collections;
ALTER TABLE api_collections_new RENAME TO api_collections;
CREATE INDEX IF NOT EXISTS idx_api_collections_workspace
  ON api_collections(workspace_id, deleted_at);

-- ---- api_collection_folders (workspace FK -> CASCADE) ----
DROP TABLE IF EXISTS api_collection_folders_new;
CREATE TABLE api_collection_folders_new (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  collection_id TEXT NOT NULL,
  parent_folder_id TEXT,
  name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  revision INTEGER NOT NULL DEFAULT 1,
  sync_status TEXT NOT NULL DEFAULT 'local',
  remote_id TEXT,
  UNIQUE(workspace_id, id),
  UNIQUE(workspace_id, collection_id, id),
  CHECK(parent_folder_id IS NULL OR parent_folder_id <> id),
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY(workspace_id, collection_id) REFERENCES api_collections(workspace_id, id),
  FOREIGN KEY(workspace_id, collection_id, parent_folder_id)
    REFERENCES api_collection_folders(workspace_id, collection_id, id)
);
INSERT INTO api_collection_folders_new SELECT * FROM api_collection_folders;
DROP TABLE api_collection_folders;
ALTER TABLE api_collection_folders_new RENAME TO api_collection_folders;
CREATE INDEX IF NOT EXISTS idx_api_collection_folders_collection
  ON api_collection_folders(workspace_id, collection_id, deleted_at, sort_order);
CREATE INDEX IF NOT EXISTS idx_api_collection_folders_parent
  ON api_collection_folders(workspace_id, collection_id, parent_folder_id, deleted_at, sort_order);
-- Recreated: dropped with the table above (defined in 20260707123000).
CREATE TRIGGER IF NOT EXISTS trg_api_collection_folders_no_cycle_insert
BEFORE INSERT ON api_collection_folders
WHEN NEW.parent_folder_id IS NOT NULL
BEGIN
  WITH RECURSIVE ancestors(ancestor_id) AS (
    SELECT NEW.parent_folder_id
    UNION ALL
    SELECT f.parent_folder_id
    FROM api_collection_folders f
    JOIN ancestors ON f.id = ancestors.ancestor_id
    WHERE f.parent_folder_id IS NOT NULL
  )
  SELECT CASE
    WHEN EXISTS(SELECT 1 FROM ancestors WHERE ancestor_id = NEW.id)
    THEN RAISE(ABORT, 'api_collection_folders would create a cycle')
  END;
END;
CREATE TRIGGER IF NOT EXISTS trg_api_collection_folders_no_cycle_update
BEFORE UPDATE OF parent_folder_id ON api_collection_folders
WHEN NEW.parent_folder_id IS NOT NULL
BEGIN
  WITH RECURSIVE ancestors(ancestor_id) AS (
    SELECT NEW.parent_folder_id
    UNION ALL
    SELECT f.parent_folder_id
    FROM api_collection_folders f
    JOIN ancestors ON f.id = ancestors.ancestor_id
    WHERE f.parent_folder_id IS NOT NULL
  )
  SELECT CASE
    WHEN EXISTS(SELECT 1 FROM ancestors WHERE ancestor_id = NEW.id)
    THEN RAISE(ABORT, 'api_collection_folders would create a cycle')
  END;
END;

-- ---- api_requests (workspace FK -> CASCADE) ----
DROP TABLE IF EXISTS api_requests_new;
CREATE TABLE api_requests_new (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL,
  collection_id TEXT NOT NULL,
  parent_folder_id TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  auth_json TEXT NOT NULL DEFAULT '{"type":"none"}',
  method TEXT NOT NULL,
  url TEXT NOT NULL,
  headers_json TEXT NOT NULL DEFAULT '[]',
  query_json TEXT NOT NULL DEFAULT '[]',
  body TEXT,
  body_kind TEXT NOT NULL DEFAULT 'json',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  revision INTEGER NOT NULL DEFAULT 1,
  sync_status TEXT NOT NULL DEFAULT 'local',
  remote_id TEXT,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY(workspace_id, collection_id) REFERENCES api_collections(workspace_id, id),
  FOREIGN KEY(workspace_id, collection_id, parent_folder_id)
    REFERENCES api_collection_folders(workspace_id, collection_id, id)
);
INSERT INTO api_requests_new SELECT * FROM api_requests;
DROP TABLE api_requests;
ALTER TABLE api_requests_new RENAME TO api_requests;
CREATE INDEX IF NOT EXISTS idx_api_requests_workspace_tree
  ON api_requests(workspace_id, deleted_at, collection_id, parent_folder_id, sort_order, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_api_requests_collection_parent
  ON api_requests(workspace_id, collection_id, parent_folder_id, deleted_at, sort_order);

-- ---- api_history (workspace FK -> CASCADE) ----
DROP TABLE IF EXISTS api_history_new;
CREATE TABLE api_history_new (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  name TEXT,
  method TEXT NOT NULL,
  url TEXT NOT NULL,
  request_headers_json TEXT NOT NULL DEFAULT '[]',
  request_query_json TEXT NOT NULL DEFAULT '[]',
  request_body TEXT,
  status INTEGER,
  duration_ms INTEGER,
  response_headers_json TEXT NOT NULL DEFAULT '[]',
  response_body_preview TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);
INSERT INTO api_history_new SELECT * FROM api_history;
DROP TABLE api_history;
ALTER TABLE api_history_new RENAME TO api_history;
CREATE INDEX IF NOT EXISTS idx_api_history_workspace_created
  ON api_history(workspace_id, created_at DESC);

-- ---- db_query_history (workspace FK -> CASCADE; keep connection FK SET NULL) ----
DROP TABLE IF EXISTS db_query_history_new;
CREATE TABLE db_query_history_new (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  connection_id TEXT,
  connection_name TEXT NOT NULL,
  sql TEXT NOT NULL,
  status TEXT NOT NULL,
  classification TEXT,
  row_count INTEGER,
  affected_rows INTEGER,
  duration_ms INTEGER,
  error TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY(connection_id) REFERENCES connections(id) ON DELETE SET NULL
);
INSERT INTO db_query_history_new SELECT * FROM db_query_history;
DROP TABLE db_query_history;
ALTER TABLE db_query_history_new RENAME TO db_query_history;
CREATE INDEX IF NOT EXISTS idx_db_query_history_workspace_created
  ON db_query_history(workspace_id, created_at DESC);
-- Recreated: dropped with the table above.
CREATE TRIGGER IF NOT EXISTS trg_db_query_history_connection_workspace_insert
BEFORE INSERT ON db_query_history
WHEN NEW.connection_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1
    FROM connections
    WHERE id = NEW.connection_id
      AND workspace_id = NEW.workspace_id
  )
BEGIN
  SELECT RAISE(ABORT, 'db_query_history connection must belong to the same workspace');
END;
CREATE TRIGGER IF NOT EXISTS trg_db_query_history_connection_workspace_update
BEFORE UPDATE OF workspace_id, connection_id ON db_query_history
WHEN NEW.connection_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1
    FROM connections
    WHERE id = NEW.connection_id
      AND workspace_id = NEW.workspace_id
  )
BEGIN
  SELECT RAISE(ABORT, 'db_query_history connection must belong to the same workspace');
END;

-- ---- api_environments (workspace FK -> CASCADE) ----
DROP TABLE IF EXISTS api_environments_new;
CREATE TABLE api_environments_new (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL,
  variables_json TEXT NOT NULL DEFAULT '[]',
  is_active INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  revision INTEGER NOT NULL DEFAULT 1,
  sync_status TEXT NOT NULL DEFAULT 'local',
  remote_id TEXT,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);
INSERT INTO api_environments_new SELECT * FROM api_environments;
DROP TABLE api_environments;
ALTER TABLE api_environments_new RENAME TO api_environments;
CREATE INDEX IF NOT EXISTS idx_api_environments_workspace_deleted
  ON api_environments(workspace_id, deleted_at);
CREATE UNIQUE INDEX IF NOT EXISTS uq_api_environments_active_per_workspace
  ON api_environments(workspace_id)
  WHERE is_active = 1 AND deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uq_api_environments_name_per_workspace
  ON api_environments(workspace_id, name COLLATE NOCASE)
  WHERE deleted_at IS NULL;

-- ---- saved_sql (workspace FK -> CASCADE; keep connection FK SET NULL) ----
DROP TABLE IF EXISTS saved_sql_new;
CREATE TABLE saved_sql_new (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  connection_id TEXT,
  name TEXT NOT NULL,
  sql TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  revision INTEGER NOT NULL DEFAULT 1,
  sync_status TEXT NOT NULL DEFAULT 'local',
  remote_id TEXT,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY(connection_id) REFERENCES connections(id) ON DELETE SET NULL
);
INSERT INTO saved_sql_new SELECT * FROM saved_sql;
DROP TABLE saved_sql;
ALTER TABLE saved_sql_new RENAME TO saved_sql;
CREATE INDEX IF NOT EXISTS idx_saved_sql_workspace_updated
  ON saved_sql(workspace_id, deleted_at, updated_at DESC);
-- Recreated: dropped with the table above.
CREATE TRIGGER IF NOT EXISTS trg_saved_sql_connection_workspace_insert
BEFORE INSERT ON saved_sql
WHEN NEW.connection_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1
    FROM connections
    WHERE id = NEW.connection_id
      AND workspace_id = NEW.workspace_id
      AND deleted_at IS NULL
  )
BEGIN
  SELECT RAISE(ABORT, 'saved_sql connection must belong to the same workspace');
END;
CREATE TRIGGER IF NOT EXISTS trg_saved_sql_connection_workspace_update
BEFORE UPDATE OF workspace_id, connection_id ON saved_sql
WHEN NEW.connection_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1
    FROM connections
    WHERE id = NEW.connection_id
      AND workspace_id = NEW.workspace_id
      AND deleted_at IS NULL
  )
BEGIN
  SELECT RAISE(ABORT, 'saved_sql connection must belong to the same workspace');
END;

-- ---- activity_events (workspace FK -> CASCADE) ----
DROP TABLE IF EXISTS activity_events_new;
CREATE TABLE activity_events_new (
  id TEXT PRIMARY KEY,
  workspace_id TEXT,
  action TEXT NOT NULL,
  target TEXT,
  details_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);
INSERT INTO activity_events_new SELECT * FROM activity_events;
DROP TABLE activity_events;
ALTER TABLE activity_events_new RENAME TO activity_events;
CREATE INDEX IF NOT EXISTS idx_activity_events_workspace_created_id
  ON activity_events(workspace_id, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_activity_events_created_id
  ON activity_events(created_at DESC, id DESC);

-- ---- ssh_host_keys (port range CHECK) ----
DROP TABLE IF EXISTS ssh_host_keys_new;
CREATE TABLE ssh_host_keys_new (
  workspace_id TEXT NOT NULL,
  host TEXT NOT NULL,
  port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
  fingerprint TEXT NOT NULL,
  key_type TEXT,
  public_key_data TEXT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, host, port),
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);
INSERT INTO ssh_host_keys_new SELECT * FROM ssh_host_keys;
DROP TABLE ssh_host_keys;
ALTER TABLE ssh_host_keys_new RENAME TO ssh_host_keys;

PRAGMA legacy_alter_table = OFF;
