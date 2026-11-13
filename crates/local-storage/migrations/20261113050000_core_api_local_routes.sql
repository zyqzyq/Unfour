-- Network preferences apply on this device. Keep a missing jump-host reference
-- so removing a saved SSH connection can never turn a routed request into a direct one.
CREATE UNIQUE INDEX IF NOT EXISTS idx_api_requests_workspace_id ON api_requests(workspace_id,id);
CREATE TABLE api_request_local_routes (
    workspace_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    ssh_connection_id TEXT, -- a row with NULL represents an explicit direct override
    PRIMARY KEY (workspace_id, request_id),
    FOREIGN KEY(workspace_id,request_id) REFERENCES api_requests(workspace_id,id) ON DELETE CASCADE
);
CREATE TABLE api_environment_local_routes (
    workspace_id TEXT NOT NULL,
    environment_id TEXT NOT NULL,
    ssh_connection_id TEXT NOT NULL,
    PRIMARY KEY (workspace_id, environment_id),
    FOREIGN KEY(workspace_id,environment_id) REFERENCES workspace_environments(workspace_id,id) ON DELETE CASCADE
);
