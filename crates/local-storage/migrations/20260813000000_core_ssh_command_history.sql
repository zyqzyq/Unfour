-- Persist user-executed SSH terminal commands independently from the bounded
-- terminal-output buffer. Command history remains local-first and is scoped by
-- both workspace and SSH connection. Nullable execution metadata is reserved
-- for later shell-integration work.

CREATE TABLE IF NOT EXISTS ssh_command_history (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  connection_id TEXT NOT NULL,
  session_id TEXT,
  command TEXT NOT NULL,
  cwd TEXT,
  exit_code INTEGER,
  duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
  redacted INTEGER NOT NULL DEFAULT 0 CHECK (redacted IN (0, 1)),
  executed_at TEXT NOT NULL,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY(workspace_id, connection_id)
    REFERENCES connections(workspace_id, id) ON DELETE CASCADE
);

CREATE TRIGGER IF NOT EXISTS trg_ssh_command_history_connection_type_insert
BEFORE INSERT ON ssh_command_history
WHEN NOT EXISTS (
  SELECT 1
  FROM connections
  WHERE id = NEW.connection_id
    AND workspace_id = NEW.workspace_id
    AND connection_type = 'ssh'
    AND deleted_at IS NULL
)
BEGIN
  SELECT RAISE(ABORT, 'ssh command history connection must be an active SSH connection in the same workspace');
END;

CREATE TRIGGER IF NOT EXISTS trg_ssh_command_history_connection_type_update
BEFORE UPDATE OF workspace_id, connection_id ON ssh_command_history
WHEN NOT EXISTS (
  SELECT 1
  FROM connections
  WHERE id = NEW.connection_id
    AND workspace_id = NEW.workspace_id
    AND connection_type = 'ssh'
    AND deleted_at IS NULL
)
BEGIN
  SELECT RAISE(ABORT, 'ssh command history connection must be an active SSH connection in the same workspace');
END;

CREATE INDEX IF NOT EXISTS idx_ssh_command_history_workspace_executed
ON ssh_command_history(workspace_id, executed_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_ssh_command_history_connection_executed
ON ssh_command_history(workspace_id, connection_id, executed_at DESC, id DESC);
