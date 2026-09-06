-- Distinguishes "remote serverVersion N was received" from "the current
-- entity reader fully materialized that envelope". Existing rows default to 0
-- so an upgraded reader fail-safe replays retained remotes once.
ALTER TABLE cloud_sync_entity_state
ADD COLUMN applied_reader_revision INTEGER NOT NULL DEFAULT 0
CHECK (applied_reader_revision >= 0);
