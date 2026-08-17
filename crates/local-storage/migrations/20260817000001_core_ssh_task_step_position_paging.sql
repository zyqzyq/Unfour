-- External sync pages may split two sides of a position swap. Keep position
-- deterministic through ORDER BY position, id without rejecting the valid
-- intermediate page where two active siblings temporarily share a position.

DROP INDEX IF EXISTS uq_ssh_task_step_active_position;
