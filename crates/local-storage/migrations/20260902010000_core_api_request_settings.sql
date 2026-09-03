ALTER TABLE api_requests
ADD COLUMN settings_json TEXT NOT NULL DEFAULT '{"timeoutMs":null}';
