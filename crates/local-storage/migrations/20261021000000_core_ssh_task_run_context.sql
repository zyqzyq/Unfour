-- Local execution recall only; excluded from domain sync and workspace bundles.
ALTER TABLE ssh_task_run ADD COLUMN context_json TEXT;
