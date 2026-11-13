-- Widen just the connection discriminator. Dropping this parent with foreign
-- keys enabled would cascade into terminal history and null saved SQL links.
-- SQLite supports editing the schema text for CHECK-only changes that leave
-- the on-disk representation intact (lang_altertable.html, section 8).
-- Verify the exact historical constraint before enabling schema writes.
CREATE TEMP TABLE redis_connection_schema_guard (ok INTEGER NOT NULL CHECK (ok = 1));
INSERT INTO redis_connection_schema_guard VALUES ((
  SELECT instr(sql, 'CHECK (connection_type IN (''database'', ''ssh''))') > 0
  FROM sqlite_schema WHERE type = 'table' AND name = 'connections'
));
DROP TABLE redis_connection_schema_guard;
PRAGMA writable_schema = ON;
UPDATE sqlite_schema
SET sql = replace(sql,
  'CHECK (connection_type IN (''database'', ''ssh''))',
  'CHECK (connection_type IN (''database'', ''ssh'', ''redis''))')
WHERE type = 'table' AND name = 'connections';
PRAGMA writable_schema = RESET;

-- This DDL also advances the schema cookie for other open connections.
CREATE TABLE redis_connections (
  connection_id TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL,
  username TEXT,
  db_index INTEGER NOT NULL CHECK (db_index BETWEEN 0 AND 4294967295),
  tls INTEGER NOT NULL CHECK (tls IN (0, 1)),
  FOREIGN KEY (workspace_id, connection_id) REFERENCES connections(workspace_id, id)
    ON DELETE CASCADE
);
CREATE TRIGGER redis_connections_type_insert BEFORE INSERT ON redis_connections
WHEN NOT EXISTS (
  SELECT 1 FROM connections WHERE workspace_id = NEW.workspace_id
    AND id = NEW.connection_id AND connection_type = 'redis'
)
BEGIN
  SELECT RAISE(ABORT, 'redis_connections must reference a Redis connection');
END;
CREATE TRIGGER redis_connections_type_update
BEFORE UPDATE OF workspace_id, connection_id ON redis_connections
WHEN NOT EXISTS (
  SELECT 1 FROM connections WHERE workspace_id = NEW.workspace_id
    AND id = NEW.connection_id AND connection_type = 'redis'
)
BEGIN
  SELECT RAISE(ABORT, 'redis_connections must reference a Redis connection');
END;

-- Redis is a second data source in the existing Database connections list.
DROP TRIGGER local_change_connections_insert;
CREATE TRIGGER local_change_connections_insert AFTER INSERT ON connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, CASE NEW.connection_type WHEN 'redis' THEN 'database-connections'
    ELSE NEW.connection_type || '-connections' END, 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;
DROP TRIGGER local_change_connections_update;
CREATE TRIGGER local_change_connections_update AFTER UPDATE ON connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, CASE NEW.connection_type WHEN 'redis' THEN 'database-connections'
    ELSE NEW.connection_type || '-connections' END, 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, CASE OLD.connection_type WHEN 'redis' THEN 'database-connections'
    ELSE OLD.connection_type || '-connections' END, 1
  WHERE OLD.workspace_id <> NEW.workspace_id OR OLD.connection_type <> NEW.connection_type
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;
DROP TRIGGER local_change_connections_delete;
CREATE TRIGGER local_change_connections_delete AFTER DELETE ON connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, CASE OLD.connection_type WHEN 'redis' THEN 'database-connections'
    ELSE OLD.connection_type || '-connections' END, 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;
