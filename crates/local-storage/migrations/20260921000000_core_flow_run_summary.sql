-- Preserve exact historical durations without reading snapshots for each list.
ALTER TABLE flow_runs ADD COLUMN finished_at TEXT;
UPDATE flow_runs SET finished_at = json_extract(run_json, '$.finishedAt');
