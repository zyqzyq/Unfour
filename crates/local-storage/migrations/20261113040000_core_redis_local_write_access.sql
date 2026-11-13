-- Desktop editing is a local opt-in for one saved Redis target.
ALTER TABLE redis_connections ADD COLUMN allow_writes INTEGER NOT NULL
  DEFAULT 0 CHECK (allow_writes IN (0, 1));
