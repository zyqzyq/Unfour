ALTER TABLE cloud_sync_diagnostics ADD COLUMN request_id TEXT;
ALTER TABLE cloud_sync_diagnostics ADD COLUMN http_status INTEGER;
ALTER TABLE cloud_sync_diagnostics ADD COLUMN phase TEXT;
ALTER TABLE cloud_sync_diagnostics ADD COLUMN operation_id TEXT;
ALTER TABLE cloud_sync_diagnostics ADD COLUMN operation_index INTEGER;
ALTER TABLE cloud_sync_diagnostics ADD COLUMN source TEXT CHECK (source IN ('domain', 'local', 'remote'));
