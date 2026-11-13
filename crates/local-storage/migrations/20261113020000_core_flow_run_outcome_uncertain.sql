-- Keep history summaries readable without loading execution snapshots.
ALTER TABLE flow_runs ADD COLUMN outcome_uncertain INTEGER NOT NULL DEFAULT 0;

UPDATE flow_runs SET outcome_uncertain = 1
WHERE status = 'failed'
  AND CASE WHEN json_valid(run_json) THEN json_extract(run_json, '$.error') END = 'FLOW_OUTCOME_UNCERTAIN';

-- Desktop and standalone MCP may finish runs with different binary versions.
-- Derive the flag on the running -> failed transition for either writer.
CREATE TRIGGER flow_runs_mark_uncertain AFTER UPDATE OF status ON flow_runs
WHEN NEW.status = 'failed' AND OLD.status = 'running'
  AND CASE WHEN json_valid(NEW.run_json) THEN json_extract(NEW.run_json, '$.error') END = 'FLOW_OUTCOME_UNCERTAIN'
BEGIN
    UPDATE flow_runs SET outcome_uncertain = 1 WHERE id = NEW.id;
END;
