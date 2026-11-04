-- Compact commit-visible clocks, not a row-by-row change log. No business values
-- or credentials are stored here. Keep clocks after deletion so open readers
-- can invalidate deleted records; storage grows by domains/workspaces, not writes.
CREATE TABLE local_data_versions (
  workspace_id TEXT NOT NULL,
  domain TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (workspace_id, domain)
) WITHOUT ROWID;

CREATE TRIGGER local_change_workspaces_insert AFTER INSERT ON workspaces
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.id, 'workspaces', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspaces_update AFTER UPDATE ON workspaces
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.id, 'workspaces', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspaces_delete AFTER DELETE ON workspaces
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.id, 'workspaces', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_variables_insert AFTER INSERT ON workspace_variables
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'workspace-variables', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_variables_update AFTER UPDATE ON workspace_variables
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'workspace-variables', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'workspace-variables', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_variables_delete AFTER DELETE ON workspace_variables
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'workspace-variables', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_environments_insert AFTER INSERT ON workspace_environments
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'environments', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_environments_update AFTER UPDATE ON workspace_environments
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'environments', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'environments', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_environments_delete AFTER DELETE ON workspace_environments
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'environments', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_environment_variables_insert AFTER INSERT ON workspace_environment_variables
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'environments', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_environment_variables_update AFTER UPDATE ON workspace_environment_variables
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'environments', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'environments', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_environment_variables_delete AFTER DELETE ON workspace_environment_variables
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'environments', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_local_state_insert AFTER INSERT ON workspace_local_state
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'environments', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_local_state_update AFTER UPDATE ON workspace_local_state
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'environments', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'environments', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_workspace_local_state_delete AFTER DELETE ON workspace_local_state
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'environments', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_api_collections_insert AFTER INSERT ON api_collections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'api-collections', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_api_collections_update AFTER UPDATE ON api_collections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'api-collections', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'api-collections', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_api_collections_delete AFTER DELETE ON api_collections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'api-collections', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_api_collection_folders_insert AFTER INSERT ON api_collection_folders
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'api-folders', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_api_collection_folders_update AFTER UPDATE ON api_collection_folders
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'api-folders', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'api-folders', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_api_collection_folders_delete AFTER DELETE ON api_collection_folders
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'api-folders', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_api_requests_insert AFTER INSERT ON api_requests
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'api-requests', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_api_requests_update AFTER UPDATE ON api_requests
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'api-requests', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'api-requests', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_api_requests_delete AFTER DELETE ON api_requests
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'api-requests', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_task_insert AFTER INSERT ON ssh_task
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'ssh-tasks', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_task_update AFTER UPDATE ON ssh_task
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'ssh-tasks', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'ssh-tasks', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_task_delete AFTER DELETE ON ssh_task
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'ssh-tasks', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_task_step_insert AFTER INSERT ON ssh_task_step
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'ssh-tasks', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_task_step_update AFTER UPDATE ON ssh_task_step
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'ssh-tasks', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'ssh-tasks', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_task_step_delete AFTER DELETE ON ssh_task_step
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'ssh-tasks', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_task_local_binding_insert AFTER INSERT ON ssh_task_local_binding
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'ssh-tasks', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_task_local_binding_update AFTER UPDATE ON ssh_task_local_binding
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'ssh-tasks', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'ssh-tasks', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_task_local_binding_delete AFTER DELETE ON ssh_task_local_binding
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'ssh-tasks', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_saved_sql_insert AFTER INSERT ON saved_sql
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'saved-sql', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_saved_sql_update AFTER UPDATE ON saved_sql
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'saved-sql', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'saved-sql', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_saved_sql_delete AFTER DELETE ON saved_sql
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'saved-sql', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_flow_definitions_insert AFTER INSERT ON flow_definitions
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'flows', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_flow_definitions_update AFTER UPDATE ON flow_definitions
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, 'flows', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, 'flows', 1 WHERE OLD.workspace_id <> NEW.workspace_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_flow_definitions_delete AFTER DELETE ON flow_definitions
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, 'flows', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_connections_insert AFTER INSERT ON connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, NEW.connection_type || '-connections', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_connections_insert AFTER INSERT ON ssh_connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT workspace_id, 'ssh-connections', 1 FROM connections WHERE id = NEW.connection_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_database_connections_insert AFTER INSERT ON database_connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT workspace_id, 'database-connections', 1 FROM connections WHERE id = NEW.connection_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_active_workspace_insert AFTER INSERT ON app_settings
WHEN NEW.key = 'active_workspace_id'
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.value, 'workspaces', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_connections_update AFTER UPDATE ON connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.workspace_id, NEW.connection_type || '-connections', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT OLD.workspace_id, OLD.connection_type || '-connections', 1
  WHERE OLD.workspace_id <> NEW.workspace_id OR OLD.connection_type <> NEW.connection_type
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_connections_update AFTER UPDATE ON ssh_connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT workspace_id, 'ssh-connections', 1 FROM connections WHERE id = NEW.connection_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_database_connections_update AFTER UPDATE ON database_connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT workspace_id, 'database-connections', 1 FROM connections WHERE id = NEW.connection_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_active_workspace_update AFTER UPDATE ON app_settings
WHEN NEW.key = 'active_workspace_id'
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (NEW.value, 'workspaces', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_connections_delete AFTER DELETE ON connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.workspace_id, OLD.connection_type || '-connections', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_ssh_connections_delete AFTER DELETE ON ssh_connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT workspace_id, 'ssh-connections', 1 FROM connections WHERE id = OLD.connection_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_database_connections_delete AFTER DELETE ON database_connections
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  SELECT workspace_id, 'database-connections', 1 FROM connections WHERE id = OLD.connection_id
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;

CREATE TRIGGER local_change_active_workspace_delete AFTER DELETE ON app_settings
WHEN OLD.key = 'active_workspace_id'
BEGIN
  INSERT INTO local_data_versions(workspace_id, domain, version)
  VALUES (OLD.value, 'workspaces', 1)
  ON CONFLICT(workspace_id, domain) DO UPDATE SET version = version + 1;
END;
