-- SSH Tasks P0: sync-ready task templates plus device-local bindings and runs.
-- Only ssh_task and ssh_task_step are future sync candidates. Bindings, runs,
-- logs, transfer progress, and transient inputs remain local to this device.
-- Upload/Download localPath values are persisted only as placeholder-led runtime
-- templates; the engine rejects literal device paths before saving a step.

CREATE TABLE IF NOT EXISTS ssh_task (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  UNIQUE(workspace_id, id),
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS ssh_task_step (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  task_id TEXT NOT NULL,
  name TEXT NOT NULL,
  step_type TEXT NOT NULL CHECK (step_type IN ('command', 'upload', 'download')),
  position INTEGER NOT NULL CHECK (position >= 0),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  config_version INTEGER NOT NULL DEFAULT 1 CHECK (config_version > 0),
  config_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  UNIQUE(workspace_id, id),
  FOREIGN KEY(workspace_id, task_id)
    REFERENCES ssh_task(workspace_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS ssh_task_local_binding (
  task_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  default_connection_id TEXT,
  last_used_connection_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(workspace_id, task_id),
  FOREIGN KEY(workspace_id, task_id)
    REFERENCES ssh_task(workspace_id, id) ON DELETE CASCADE,
  FOREIGN KEY(default_connection_id) REFERENCES connections(id) ON DELETE SET NULL,
  FOREIGN KEY(last_used_connection_id) REFERENCES connections(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS ssh_task_run (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  task_id TEXT NOT NULL,
  connection_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('running', 'success', 'failed', 'cancelled')),
  started_at TEXT NOT NULL,
  finished_at TEXT,
  error_message TEXT,
  log_path TEXT NOT NULL,
  UNIQUE(workspace_id, id),
  FOREIGN KEY(workspace_id, task_id)
    REFERENCES ssh_task(workspace_id, id) ON DELETE CASCADE,
  FOREIGN KEY(connection_id) REFERENCES connections(id) ON DELETE SET NULL
);

CREATE TRIGGER IF NOT EXISTS trg_ssh_task_binding_connection_insert
BEFORE INSERT ON ssh_task_local_binding
WHEN (NEW.default_connection_id IS NOT NULL OR NEW.last_used_connection_id IS NOT NULL)
  AND (
    (NEW.default_connection_id IS NOT NULL AND NOT EXISTS (
      SELECT 1 FROM connections
      WHERE id = NEW.default_connection_id
        AND workspace_id = NEW.workspace_id
        AND connection_type = 'ssh'
        AND deleted_at IS NULL
    ))
    OR
    (NEW.last_used_connection_id IS NOT NULL AND NOT EXISTS (
      SELECT 1 FROM connections
      WHERE id = NEW.last_used_connection_id
        AND workspace_id = NEW.workspace_id
        AND connection_type = 'ssh'
        AND deleted_at IS NULL
    ))
  )
BEGIN
  SELECT RAISE(ABORT, 'SSH task local binding must reference active SSH connections in the same workspace');
END;

CREATE TRIGGER IF NOT EXISTS trg_ssh_task_binding_connection_update
BEFORE UPDATE OF workspace_id, default_connection_id, last_used_connection_id
ON ssh_task_local_binding
WHEN (NEW.default_connection_id IS NOT NULL OR NEW.last_used_connection_id IS NOT NULL)
  AND (
    (NEW.default_connection_id IS NOT NULL AND NOT EXISTS (
      SELECT 1 FROM connections
      WHERE id = NEW.default_connection_id
        AND workspace_id = NEW.workspace_id
        AND connection_type = 'ssh'
        AND deleted_at IS NULL
    ))
    OR
    (NEW.last_used_connection_id IS NOT NULL AND NOT EXISTS (
      SELECT 1 FROM connections
      WHERE id = NEW.last_used_connection_id
        AND workspace_id = NEW.workspace_id
        AND connection_type = 'ssh'
        AND deleted_at IS NULL
    ))
  )
BEGIN
  SELECT RAISE(ABORT, 'SSH task local binding must reference active SSH connections in the same workspace');
END;

CREATE TRIGGER IF NOT EXISTS trg_ssh_task_run_connection_insert
BEFORE INSERT ON ssh_task_run
WHEN NEW.connection_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM connections
    WHERE id = NEW.connection_id
      AND workspace_id = NEW.workspace_id
      AND connection_type = 'ssh'
  )
BEGIN
  SELECT RAISE(ABORT, 'SSH task run connection must be an SSH connection in the same workspace');
END;

CREATE TRIGGER IF NOT EXISTS trg_ssh_task_run_active_task_insert
BEFORE INSERT ON ssh_task_run
WHEN NOT EXISTS (
  SELECT 1 FROM ssh_task
  WHERE id = NEW.task_id
    AND workspace_id = NEW.workspace_id
    AND deleted_at IS NULL
)
BEGIN
  SELECT RAISE(ABORT, 'SSH task run must reference an active task in the same workspace');
END;

CREATE TRIGGER IF NOT EXISTS trg_ssh_task_run_connection_update
BEFORE UPDATE OF workspace_id, connection_id ON ssh_task_run
WHEN NEW.connection_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM connections
    WHERE id = NEW.connection_id
      AND workspace_id = NEW.workspace_id
      AND connection_type = 'ssh'
  )
BEGIN
  SELECT RAISE(ABORT, 'SSH task run connection must be an SSH connection in the same workspace');
END;

CREATE TRIGGER IF NOT EXISTS trg_ssh_task_run_active_task_update
BEFORE UPDATE OF workspace_id, task_id ON ssh_task_run
WHEN NOT EXISTS (
  SELECT 1 FROM ssh_task
  WHERE id = NEW.task_id
    AND workspace_id = NEW.workspace_id
    AND deleted_at IS NULL
)
BEGIN
  SELECT RAISE(ABORT, 'SSH task run must reference an active task in the same workspace');
END;

CREATE INDEX IF NOT EXISTS idx_ssh_task_workspace_updated
ON ssh_task(workspace_id, deleted_at, updated_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS uq_ssh_task_step_active_position
ON ssh_task_step(task_id, position)
WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_ssh_task_step_task_position
ON ssh_task_step(workspace_id, task_id, deleted_at, position);

CREATE INDEX IF NOT EXISTS idx_ssh_task_run_task_started
ON ssh_task_run(workspace_id, task_id, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_ssh_task_run_finished
ON ssh_task_run(finished_at);
