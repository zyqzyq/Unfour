-- Definitions retain deletion intent; runtime history remains device-local.
ALTER TABLE flow_definitions ADD COLUMN deleted_at TEXT;
CREATE INDEX flow_definitions_live_workspace
    ON flow_definitions(workspace_id, updated_at) WHERE deleted_at IS NULL;

-- Earlier workspace soft-deletes did not cascade into Flow storage.
UPDATE flow_definitions
SET deleted_at = (SELECT deleted_at FROM workspaces WHERE id = workspace_id),
    updated_at = (SELECT deleted_at FROM workspaces WHERE id = workspace_id),
    revision = revision + 1,
    definition_json = json_set(definition_json, '$.revision', revision + 1)
WHERE workspace_id IN (SELECT id FROM workspaces WHERE deleted_at IS NOT NULL);

-- Workspace deletion removes local runtime snapshots/outputs for privacy.
DELETE FROM flow_runs
WHERE workspace_id IN (SELECT id FROM workspaces WHERE deleted_at IS NOT NULL);
