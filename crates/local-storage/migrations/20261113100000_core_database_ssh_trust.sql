-- A saved SSH definition's revision does not change when its host key is reset.
-- Negative pins require explicit review. Each subsequent reset changes the pin
-- again so a SQL editor with an older snapshot cannot acknowledge a newer key.
ALTER TABLE database_connections ADD COLUMN ssh_trust_generation INTEGER NOT NULL DEFAULT 0;
UPDATE database_connections SET ssh_trust_generation=-ssh_revision WHERE ssh_revision<0;

CREATE TRIGGER database_ssh_route_trust_reset AFTER DELETE ON ssh_host_keys
BEGIN
  UPDATE database_connections SET ssh_revision = -(ssh_trust_generation + 1),
    ssh_trust_generation = ssh_trust_generation + 1
    WHERE ssh_connection_id IN (
      SELECT id FROM connections WHERE workspace_id=OLD.workspace_id
      AND connection_type='ssh' AND host=OLD.host AND port=OLD.port
    );
END;
CREATE TRIGGER database_ssh_route_trust_changed AFTER UPDATE OF fingerprint ON ssh_host_keys
WHEN OLD.fingerprint IS NOT NEW.fingerprint
BEGIN
  UPDATE database_connections SET ssh_revision = -(ssh_trust_generation + 1),
    ssh_trust_generation = ssh_trust_generation + 1
    WHERE ssh_connection_id IN (
      SELECT id FROM connections WHERE workspace_id=OLD.workspace_id
      AND connection_type='ssh' AND host=OLD.host AND port=OLD.port
    );
END;
