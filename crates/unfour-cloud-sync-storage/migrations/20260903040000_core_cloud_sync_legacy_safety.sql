-- Before pause reasons existed, a paused binding could represent either an
-- account-context pause or an explicit workspace pause. Do not auto-resume an
-- ambiguous legacy row; require a deliberate Enable and leave a diagnostic.
UPDATE cloud_sync_workspace_bindings AS binding
SET last_error = COALESCE(last_error, 'cloud_sync_legacy_paused_binding_ambiguous')
WHERE binding.sync_enabled = 0
  AND binding.state = 'paused'
  AND NOT EXISTS (
    SELECT 1
    FROM cloud_sync_account_binding_pause_reasons AS pause
    WHERE pause.account_id = binding.account_id
      AND pause.local_workspace_id = binding.local_workspace_id
  );

INSERT INTO cloud_sync_diagnostics (
  account_id, cloud_workspace_id, category, error_code,
  entity_type, entity_id, occurred_at
)
SELECT binding.account_id, binding.cloud_workspace_id, 'permanent',
       'cloud_sync_legacy_paused_binding_ambiguous', NULL, NULL,
       binding.updated_at
FROM cloud_sync_workspace_bindings AS binding
WHERE binding.sync_enabled = 0
  AND binding.state = 'paused'
  AND NOT EXISTS (
    SELECT 1
    FROM cloud_sync_account_binding_pause_reasons AS pause
    WHERE pause.account_id = binding.account_id
      AND pause.local_workspace_id = binding.local_workspace_id
  );

-- Duplicate historical bindings remain present for data preservation, but no
-- row may be treated as runnable while ownership is unresolved.
UPDATE cloud_sync_workspace_bindings AS binding
SET sync_enabled = 0,
    state = CASE WHEN binding.state = 'paused' THEN 'paused' ELSE 'error' END,
    last_error = COALESCE(last_error, 'cloud_sync_workspace_ownership_ambiguous')
WHERE NOT EXISTS (
    SELECT 1
    FROM cloud_sync_workspace_ownership AS owner
    WHERE owner.local_workspace_id = binding.local_workspace_id
  )
  AND EXISTS (
    SELECT 1
    FROM cloud_sync_workspace_bindings AS duplicate
    WHERE duplicate.local_workspace_id = binding.local_workspace_id
      AND (
        duplicate.account_id <> binding.account_id
        OR duplicate.cloud_workspace_id <> binding.cloud_workspace_id
      )
  );

INSERT INTO cloud_sync_diagnostics (
  account_id, cloud_workspace_id, category, error_code,
  entity_type, entity_id, occurred_at
)
SELECT binding.account_id, binding.cloud_workspace_id, 'permanent',
       'cloud_sync_workspace_ownership_ambiguous', NULL, NULL,
       binding.updated_at
FROM cloud_sync_workspace_bindings AS binding
WHERE NOT EXISTS (
    SELECT 1
    FROM cloud_sync_workspace_ownership AS owner
    WHERE owner.local_workspace_id = binding.local_workspace_id
  )
  AND EXISTS (
    SELECT 1
    FROM cloud_sync_workspace_bindings AS duplicate
    WHERE duplicate.local_workspace_id = binding.local_workspace_id
      AND (
        duplicate.account_id <> binding.account_id
        OR duplicate.cloud_workspace_id <> binding.cloud_workspace_id
      )
  );
