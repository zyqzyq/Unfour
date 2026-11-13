-- Other windows and desktop processes observe device-local routing changes
-- through the same workspace clocks as their owning requests and environments.
CREATE TRIGGER local_change_api_request_local_routes_insert AFTER INSERT ON api_request_local_routes
BEGIN
  INSERT INTO local_data_versions(workspace_id,domain,version)
  VALUES (NEW.workspace_id,'api-requests',1)
  ON CONFLICT(workspace_id,domain) DO UPDATE SET version=version+1;
END;

CREATE TRIGGER local_change_api_request_local_routes_update AFTER UPDATE OF ssh_connection_id ON api_request_local_routes
WHEN OLD.ssh_connection_id IS NOT NEW.ssh_connection_id
BEGIN
  INSERT INTO local_data_versions(workspace_id,domain,version)
  VALUES (NEW.workspace_id,'api-requests',1)
  ON CONFLICT(workspace_id,domain) DO UPDATE SET version=version+1;
END;

CREATE TRIGGER local_change_api_request_local_routes_delete AFTER DELETE ON api_request_local_routes
BEGIN
  INSERT INTO local_data_versions(workspace_id,domain,version)
  VALUES (OLD.workspace_id,'api-requests',1)
  ON CONFLICT(workspace_id,domain) DO UPDATE SET version=version+1;
END;

CREATE TRIGGER local_change_api_environment_local_routes_insert AFTER INSERT ON api_environment_local_routes
BEGIN
  INSERT INTO local_data_versions(workspace_id,domain,version)
  VALUES (NEW.workspace_id,'environments',1)
  ON CONFLICT(workspace_id,domain) DO UPDATE SET version=version+1;
END;

CREATE TRIGGER local_change_api_environment_local_routes_update AFTER UPDATE OF ssh_connection_id ON api_environment_local_routes
WHEN OLD.ssh_connection_id IS NOT NEW.ssh_connection_id
BEGIN
  INSERT INTO local_data_versions(workspace_id,domain,version)
  VALUES (NEW.workspace_id,'environments',1)
  ON CONFLICT(workspace_id,domain) DO UPDATE SET version=version+1;
END;

CREATE TRIGGER local_change_api_environment_local_routes_delete AFTER DELETE ON api_environment_local_routes
BEGIN
  INSERT INTO local_data_versions(workspace_id,domain,version)
  VALUES (OLD.workspace_id,'environments',1)
  ON CONFLICT(workspace_id,domain) DO UPDATE SET version=version+1;
END;
