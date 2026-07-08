-- Enforce unique workspace names (global, case-insensitive).
--
-- Workspaces are top-level entities (there is no parent workspace_id), so
-- uniqueness is global across all non-deleted workspaces. We match the
-- existing api_environments convention and ignore case (COLLATE NOCASE) so
-- that "Dev" and "dev" cannot both exist.
--
-- Safety net for already-running dev databases that may hold duplicate names
-- from before this constraint existed: soft-delete all but the first row per
-- name (a `dedup-<id>` sentinel in deleted_at) before creating the unique
-- index. The app only ever checks `deleted_at IS NULL`, so the sentinel is
-- harmless and is never parsed as a date.

UPDATE workspaces
SET deleted_at = 'dedup-' || id
WHERE deleted_at IS NULL
  AND id NOT IN (
    SELECT MIN(id)
    FROM workspaces
    WHERE deleted_at IS NULL
    GROUP BY name COLLATE NOCASE
  );

CREATE UNIQUE INDEX IF NOT EXISTS uq_workspaces_name
  ON workspaces(name COLLATE NOCASE)
  WHERE deleted_at IS NULL;
