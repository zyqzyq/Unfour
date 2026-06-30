CREATE TABLE IF NOT EXISTS api_collection_folders (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  collection_id TEXT NOT NULL,
  parent_folder_id TEXT,
  name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  FOREIGN KEY(workspace_id) REFERENCES workspaces(id),
  FOREIGN KEY(collection_id) REFERENCES api_collections(id),
  FOREIGN KEY(parent_folder_id) REFERENCES api_collection_folders(id)
);

CREATE INDEX IF NOT EXISTS idx_api_collection_folders_collection
ON api_collection_folders(collection_id, deleted_at, sort_order);

CREATE INDEX IF NOT EXISTS idx_api_collection_folders_parent
ON api_collection_folders(collection_id, parent_folder_id, deleted_at, sort_order);

ALTER TABLE api_requests
ADD COLUMN parent_folder_id TEXT;

ALTER TABLE api_requests
ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

INSERT INTO api_collections (
  id, workspace_id, name, description, created_at, updated_at,
  deleted_at, revision, sync_status, remote_id
)
SELECT
  'default-' || missing.workspace_id,
  missing.workspace_id,
  'My Collection',
  NULL,
  datetime('now'),
  datetime('now'),
  NULL,
  1,
  'local',
  NULL
FROM (
  SELECT DISTINCT workspace_id
  FROM api_requests
  WHERE deleted_at IS NULL
    AND (collection_id IS NULL OR trim(collection_id) = '')
) missing
WHERE NOT EXISTS (
  SELECT 1
  FROM api_collections existing
  WHERE existing.workspace_id = missing.workspace_id
    AND existing.deleted_at IS NULL
);

UPDATE api_requests
SET collection_id = (
  SELECT existing.id
  FROM api_collections existing
  WHERE existing.workspace_id = api_requests.workspace_id
    AND existing.deleted_at IS NULL
  ORDER BY existing.created_at ASC, existing.name ASC, existing.id ASC
  LIMIT 1
)
WHERE deleted_at IS NULL
  AND (collection_id IS NULL OR trim(collection_id) = '');

INSERT OR IGNORE INTO api_collection_folders (
  id, workspace_id, collection_id, parent_folder_id, name, sort_order,
  created_at, updated_at, deleted_at
)
WITH RECURSIVE
  legacy_sources(workspace_id, collection_id, folder_path) AS (
    SELECT
      workspace_id,
      collection_id,
      trim(replace(folder_path, char(92), '/'), '/')
    FROM api_requests
    WHERE deleted_at IS NULL
      AND collection_id IS NOT NULL
      AND folder_path IS NOT NULL
      AND trim(replace(folder_path, char(92), '/'), '/') <> ''
    UNION
    SELECT
      collections.workspace_id,
      collections.id,
      trim(replace(json_each.value, char(92), '/'), '/')
    FROM api_collections collections,
      json_each(
        CASE
          WHEN json_valid(collections.folders_json) THEN collections.folders_json
          ELSE '[]'
        END
      )
    WHERE collections.deleted_at IS NULL
      AND trim(replace(json_each.value, char(92), '/'), '/') <> ''
  ),
  split(
    workspace_id,
    collection_id,
    folder_path,
    path,
    parent_path,
    name,
    rest,
    depth
  ) AS (
    SELECT
      workspace_id,
      collection_id,
      folder_path,
      '',
      NULL,
      '',
      folder_path || '/',
      0
    FROM legacy_sources
    UNION ALL
    SELECT
      workspace_id,
      collection_id,
      folder_path,
      CASE
        WHEN path = '' THEN substr(rest, 1, instr(rest, '/') - 1)
        ELSE path || '/' || substr(rest, 1, instr(rest, '/') - 1)
      END,
      NULLIF(path, ''),
      substr(rest, 1, instr(rest, '/') - 1),
      substr(rest, instr(rest, '/') + 1),
      depth + 1
    FROM split
    WHERE rest <> ''
  )
SELECT DISTINCT
  'legacy-folder:' || workspace_id || ':' || collection_id || ':' || path,
  workspace_id,
  collection_id,
  CASE
    WHEN parent_path IS NULL THEN NULL
    ELSE 'legacy-folder:' || workspace_id || ':' || collection_id || ':' || parent_path
  END,
  name,
  0,
  datetime('now'),
  datetime('now'),
  NULL
FROM split
WHERE path <> '';

UPDATE api_requests
SET parent_folder_id =
  'legacy-folder:' || workspace_id || ':' || collection_id || ':' ||
  trim(replace(folder_path, char(92), '/'), '/')
WHERE deleted_at IS NULL
  AND collection_id IS NOT NULL
  AND folder_path IS NOT NULL
  AND trim(replace(folder_path, char(92), '/'), '/') <> '';

CREATE INDEX IF NOT EXISTS idx_api_requests_collection_parent
ON api_requests(collection_id, parent_folder_id, deleted_at, sort_order);
