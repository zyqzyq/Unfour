ALTER TABLE api_history ADD COLUMN request_body_kind TEXT NOT NULL DEFAULT 'json';
