-- API entities were added to the sync protocol after some bindings already
-- existed. Those bindings need one idempotent local backfill before normal
-- incremental sync can see their pre-existing API data.
ALTER TABLE cloud_sync_workspace_bindings
ADD COLUMN api_v2_bootstrap_state TEXT NOT NULL DEFAULT 'pending'
CHECK (api_v2_bootstrap_state IN ('pending', 'completed'));
