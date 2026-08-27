ALTER TABLE pro_workspace_sync_bindings
ADD COLUMN ssh_task_v3_bootstrap_state TEXT NOT NULL DEFAULT 'pending'
CHECK (ssh_task_v3_bootstrap_state IN ('pending', 'completed'));
