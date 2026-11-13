-- Remember the route requirement for copying a historical terminal after restart.
-- The live parent handle and all authentication material stay in memory.
ALTER TABLE ssh_terminal_history ADD COLUMN requires_temporary_route INTEGER NOT NULL DEFAULT 0
    CHECK (requires_temporary_route IN (0, 1));
