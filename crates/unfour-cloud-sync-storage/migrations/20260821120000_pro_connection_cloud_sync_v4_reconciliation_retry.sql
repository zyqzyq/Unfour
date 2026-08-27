-- The first Protocol-v4 bootstrap could mark a binding completed without
-- reconciling the current Connection snapshot. Reopen completed bindings so
-- the corrected bootstrap can recover remote Connection history once.
UPDATE pro_workspace_sync_bindings
SET connection_v4_bootstrap_state = 'pending'
WHERE connection_v4_bootstrap_state = 'completed';
