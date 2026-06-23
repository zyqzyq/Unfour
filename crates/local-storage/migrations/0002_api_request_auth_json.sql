ALTER TABLE api_requests
ADD COLUMN auth_json TEXT NOT NULL DEFAULT '{"type":"none"}';
