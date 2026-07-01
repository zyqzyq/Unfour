ALTER TABLE workspaces
ADD COLUMN environment_type TEXT NOT NULL DEFAULT 'dev';

ALTER TABLE workspaces
ADD COLUMN mcp_policy TEXT NOT NULL DEFAULT 'auto';
