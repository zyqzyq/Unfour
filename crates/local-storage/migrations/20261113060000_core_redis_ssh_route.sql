-- A deleted jump host must leave the reference unresolved. Dropping it would
-- silently change how Redis names are resolved and where credentials are sent.
ALTER TABLE redis_connections ADD COLUMN ssh_connection_id TEXT
  CHECK (ssh_connection_id IS NULL OR length(ssh_connection_id) BETWEEN 1 AND 128);
-- A second retarget while the editor is open must invalidate its revision too.
ALTER TABLE redis_connections ADD COLUMN route_review_generation INTEGER NOT NULL DEFAULT 0
  CHECK (route_review_generation >= 0);
CREATE INDEX redis_ssh_dependents ON redis_connections(workspace_id, ssh_connection_id)
  WHERE ssh_connection_id IS NOT NULL;

-- Invalidate in the same transaction as local edits, sync and host-key resets.
-- Keep the credential reference for the credential lifecycle service.
CREATE TRIGGER redis_ssh_route_endpoint AFTER UPDATE OF host,port,deleted_at ON connections
WHEN OLD.connection_type='ssh' AND
  (OLD.host IS NOT NEW.host OR OLD.port IS NOT NEW.port OR OLD.deleted_at IS NOT NEW.deleted_at)
BEGIN
  UPDATE redis_connections SET allow_writes=0,route_review_generation=route_review_generation+1
    WHERE workspace_id=OLD.workspace_id AND ssh_connection_id=OLD.id;
END;
CREATE TRIGGER redis_ssh_route_username AFTER UPDATE OF username ON ssh_connections
WHEN OLD.username IS NOT NEW.username
BEGIN
  UPDATE redis_connections SET allow_writes=0,route_review_generation=route_review_generation+1
    WHERE workspace_id=(SELECT workspace_id FROM connections WHERE id=OLD.connection_id)
      AND ssh_connection_id=OLD.connection_id;
END;
CREATE TRIGGER redis_ssh_route_trust_reset AFTER DELETE ON ssh_host_keys
BEGIN
  UPDATE redis_connections SET allow_writes=0,route_review_generation=route_review_generation+1 WHERE workspace_id=OLD.workspace_id
    AND ssh_connection_id IN (SELECT id FROM connections WHERE workspace_id=OLD.workspace_id
      AND connection_type='ssh' AND host=OLD.host AND port=OLD.port);
END;
CREATE TRIGGER redis_ssh_route_trust_changed AFTER UPDATE OF fingerprint ON ssh_host_keys
WHEN OLD.fingerprint IS NOT NEW.fingerprint
BEGIN
  UPDATE redis_connections SET allow_writes=0,route_review_generation=route_review_generation+1 WHERE workspace_id=OLD.workspace_id
    AND ssh_connection_id IN (SELECT id FROM connections WHERE workspace_id=OLD.workspace_id
      AND connection_type='ssh' AND host=OLD.host AND port=OLD.port);
END;
CREATE TRIGGER redis_ssh_route_local_revision AFTER UPDATE OF route_review_generation ON redis_connections
WHEN NEW.route_review_generation > OLD.route_review_generation
BEGIN
  UPDATE connections SET revision=revision+1 WHERE id=NEW.connection_id AND workspace_id=NEW.workspace_id;
END;
