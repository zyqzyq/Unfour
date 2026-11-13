-- Legacy binaries do not hold an owner lock, including when they are still
-- running alongside the upgraded desktop. Preserve those rows for explicit
-- history repair. New launchers acquire ownership before inserting a row.
ALTER TABLE ssh_task_run ADD COLUMN owner_lock INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_ssh_task_run_owners ON ssh_task_run(workspace_id, task_id)
WHERE owner_lock = 1 AND status = 'running';
