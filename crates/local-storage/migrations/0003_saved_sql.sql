-- Saved SQL snippets (drafts / favorites) for the database query console.
-- Workspace-scoped like every other business record; the optional connection_id
-- records which datasource a snippet was authored against.
CREATE TABLE IF NOT EXISTS saved_sql (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  connection_id TEXT,
  name TEXT NOT NULL,
  sql TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id)
);

CREATE INDEX IF NOT EXISTS idx_saved_sql_workspace_updated ON saved_sql(workspace_id, updated_at DESC);
