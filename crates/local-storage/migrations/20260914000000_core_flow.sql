-- Flow V1 is local-only. History deliberately has no FK to the definition.
CREATE TABLE flow_definitions (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    definition_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX flow_definitions_workspace ON flow_definitions(workspace_id);
CREATE TABLE flow_runs (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    flow_id TEXT NOT NULL,
    status TEXT NOT NULL,
    cancel_requested INTEGER NOT NULL DEFAULT 0,
    run_json TEXT NOT NULL,
    started_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX flow_runs_history ON flow_runs(workspace_id, flow_id, started_at DESC);
