-- SSH Task definition rows participate in Core Domain Sync through a local
-- monotonic revision. Pro-owned remote ids and sync status stay out of Core.

ALTER TABLE ssh_task
ADD COLUMN revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0);

ALTER TABLE ssh_task_step
ADD COLUMN revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0);
