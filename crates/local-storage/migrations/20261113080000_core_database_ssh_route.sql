-- Device-local route. Retain unresolved jump IDs to prevent a direct fallback.
ALTER TABLE database_connections ADD COLUMN ssh_connection_id TEXT
  CHECK (ssh_connection_id IS NULL OR length(ssh_connection_id) BETWEEN 1 AND 128);
ALTER TABLE database_connections ADD COLUMN ssh_revision INTEGER;
